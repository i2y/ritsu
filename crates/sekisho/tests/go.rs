//! The Go sekisho generates (`gen --target go`, DESIGN 5.3, 6.3), as Go's own tools read it and as
//! cedar-go answers it. Every `.gate` of the examples and the tests that passes its check is
//! generated twice — asking cedar-go in the process (`--authorizer cedar`) and Verified
//! Permissions through the AWS SDK (`--authorizer avp`) — into a module laid out as `ritsu gen`
//! lays a package out: `rules/` and `dates/` beside `authz/`, the rules' and the dates' packages
//! written by rulec's and koyomi's own generators. Then:
//!
//! - every file is as gofmt writes it, and `go vet` passes every package, both authorizers (the
//!   code of `avp` is only built and vetted: the tests never call AWS, DESIGN 6.3);
//! - the code of `cedar` is run on the raw values of every combination the check walks, and on the
//!   faults it is to refuse (`sekisho::raw`), and each answer is held to what sekisho's reference
//!   evaluation answers for the same values (`Raw::compare`).
//!
//! The module is tools/runner-go's (go.mod and go.sum: cedar-go v1.8.0, the AWS SDK's client of
//! Verified Permissions), its modules fetched with `go mod download` in tools/runner-go. Without
//! Go, a test says SKIP and passes.

mod common;

use ritsu_testkit::{Need, TempDir};
use sekisho::r#gen::Authorizer;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The module of the runner, or None with the SKIP line saying why.
fn runner() -> Option<PathBuf> {
    if !ritsu_testkit::need(Need::Go) {
        return None;
    }
    let dir = std::env::current_dir().unwrap().join("tools/runner-go");
    let go = Command::new("go").arg("version").output().map(|o| o.status.success()).unwrap_or(false);
    if !go || !dir.join("go.mod").is_file() {
        ritsu_testkit::skip("go or tools/runner-go is missing; the generated Go is not checked");
        return None;
    }
    Some(dir)
}

/// `go`, run in `dir` as every Go command here runs: outside any workspace, with -trimpath (the
/// build cache keys a package by its directory otherwise), and with no network (the modules are
/// the ones tools/runner-go fetched).
fn go_in(dir: &Path) -> Command {
    let mut c = Command::new("go");
    c.current_dir(dir).env("GOWORK", "off").env("GOFLAGS", "-trimpath").env("GOPROXY", "off").env("GOTOOLCHAIN", "local");
    c
}

/// Every `.gate` under `examples/` and `tests/`, sorted.
fn gates() -> Vec<String> {
    fn walk(d: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "gate") {
                out.push(p.to_string_lossy().to_string());
            }
        }
    }
    let mut out = Vec::new();
    walk(Path::new("examples"), &mut out);
    walk(Path::new("tests"), &mut out);
    out.sort();
    out
}

fn write(p: &Path, text: &str) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

/// A module holding the runner's go.mod and go.sum.
fn module(runner: &Path, dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    for f in ["go.mod", "go.sum"] {
        std::fs::copy(runner.join(f), dir.join(f)).unwrap();
    }
}

/// The module path of the runner's go.mod.
fn module_path(runner: &Path) -> String {
    std::fs::read_to_string(runner.join("go.mod")).unwrap().lines().find_map(|l| l.strip_prefix("module ").map(|m| m.trim().to_string())).unwrap()
}

/// The packages of one gate under `dir/<name>/`, as `ritsu gen` lays them out: the rules' and the
/// dates' packages the gate reads and the gate's own, for `authorizer`, importing each other by
/// `<module>/<name>/…`. The outcome of its check, and the gate's package's text; None for a gate
/// whose check finds an error.
fn package(gate: &str, dir: &Path, module: &str, name: &str, authorizer: Authorizer, lang: ritsu_base::text::Lang) -> Option<(sekisho::check::Outcome, String)> {
    let suite = common::joined();
    let o = sekisho::check::check_file(gate, &suite, &sekisho::check::Options::default()).unwrap();
    if o.has_errors() {
        return None;
    }
    let base = dir.join(name);
    let scope = o.scope.as_ref().unwrap();
    for read in scope.rules.values() {
        let src = std::fs::read_to_string(&read.path).unwrap();
        let p = read.path.to_string_lossy().to_string();
        let (rel, body) = rulec::codegen::package_module(&src, &p, &p, "go").unwrap();
        write(&base.join("rules").join(rel), &body);
    }
    let checked = o.walked.as_ref().unwrap();
    let mut loader = koyomi::calendar::Loader::default();
    for u in &checked.gate.uses {
        if !matches!(u.kind, sekisho::model::UseKind::Dates | sekisho::model::UseKind::Calendar) {
            continue;
        }
        let p = u.file.to_string_lossy().to_string();
        let k = koyomi::check::check_file(&p, &koyomi::check::Options::default(), &mut loader).unwrap();
        let unit = koyomi::codegen::unit_shown(k.checked.as_ref().unwrap(), ritsu_base::text::Lang::En, Some(&p));
        let pkg = ritsu_emit::ident::go_package(&unit.alias);
        write(&base.join("dates").join(&pkg).join(format!("{pkg}.go")), &koyomi::codegen::go::module(&unit));
    }
    let target = sekisho::r#gen::Target { name: "go", authorizer, go_module: format!("{module}/{name}") };
    let shown = ritsu_emit::header::file_name(gate);
    let mut text = String::new();
    for (rel, body) in sekisho::r#gen::code(&o, &suite, &target, &shown, lang).unwrap() {
        let rel = rel.strip_prefix("go/").unwrap_or(&rel).to_string();
        write(&base.join(rel), &body);
        text = body;
    }
    Some((o, text))
}

/// Every `.gate` that passes its check, generated for both authorizers, and with its comments in
/// Japanese: every file is as gofmt writes it, and `go vet` passes every package.
#[test]
fn every_package_is_formatted_and_vets() {
    let Some(runner) = runner() else { return };
    let t = TempDir::new("sekisho-go");
    let m = t.path().join("m");
    module(&runner, &m);
    let path = module_path(&runner);
    let mut made = Vec::new();
    let (en, ja) = (ritsu_base::text::Lang::En, ritsu_base::text::Lang::Ja);
    for (i, g) in gates().iter().enumerate() {
        for (k, (authorizer, lang, said)) in [(Authorizer::Cedar, en, "cedar"), (Authorizer::Avp, en, "avp"), (Authorizer::Cedar, ja, "cedar, --lang ja")].into_iter().enumerate() {
            if package(g, &m, &path, &format!("g{i}_{k}"), authorizer, lang).is_some() {
                made.push(format!("{g} ({said})"));
            }
        }
    }
    if std::env::var_os("SEKISHO_KEEP").is_some() {
        let keep = std::env::temp_dir().join("sekisho-go-kept");
        let _ = std::fs::remove_dir_all(&keep);
        let _ = Command::new("cp").arg("-R").arg(&m).arg(&keep).status();
    }
    let out = Command::new("gofmt").arg("-l").arg(".").current_dir(&m).output().unwrap();
    assert!(out.status.success() && out.stdout.is_empty(), "gofmt -l lists files it would change:\n{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let out = go_in(&m).args(["vet", "./..."]).output().unwrap();
    assert!(out.status.success(), "go vet:\n{}", String::from_utf8_lossy(&out.stderr));
    println!("gofmt and go vet pass {} packages: {}", made.len(), made.join(", "));
    assert!(made.iter().any(|m| m.starts_with("examples/refunds/refunds.gate")));
}

/// The harness: the cases of the raw values (`Raw::json`) run through the generated package, an
/// answer a line, as `Raw::compare` reads them. `REGISTRY` is put in by the test: the gate's
/// package, the store (a method a type, as the package's `Store` asks), the record types and the
/// actions' functions.
const HARNESS: &str = r#"package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"reflect"
	"strings"
	"time"
	"unicode"

	gate "IMPORT"
)

REGISTRY

type rawCase struct {
	Name      string            `json:"name"`
	Action    string            `json:"action"`
	Principal map[string]string `json:"principal"`
	Store     []rawEntity       `json:"store"`
	Input     map[string]any    `json:"input"`
	Now       string            `json:"now"`
}

type rawEntity struct {
	Type     string              `json:"type"`
	ID       string              `json:"id"`
	Roles    []string            `json:"roles"`
	MemberOf []map[string]string `json:"member_of"`
	Attrs    map[string]any      `json:"attrs"`
}

type rawData struct {
	Types map[string]struct {
		Attrs map[string]string `json:"attrs"`
	} `json:"types"`
	Actions map[string]struct {
		Resource string            `json:"resource"`
		Inputs   map[string]string `json:"inputs"`
	} `json:"actions"`
	Cases []rawCase `json:"cases"`
}

// exported is a name as the generated Go exports it.
func exported(name string) string {
	clean := strings.Map(func(r rune) rune {
		if r < 128 && (unicode.IsLetter(r) || unicode.IsDigit(r)) {
			return r
		}
		return '_'
	}, name)
	out := ""
	for _, part := range strings.Split(clean, "_") {
		if part != "" {
			out += strings.ToUpper(part[:1]) + part[1:]
		}
	}
	if out == "" || unicode.IsDigit(rune(out[0])) {
		out = "X" + out
	}
	return out
}

// set puts a raw value into a field, by its kind; a pointer for a field that may be absent.
func set(f reflect.Value, kind string, v any) {
	if v == nil {
		return
	}
	t := f.Type()
	ptr := t.Kind() == reflect.Pointer
	if ptr {
		t = t.Elem()
	}
	x := reflect.New(t).Elem()
	switch kind {
	case "number":
		x.SetInt(int64(v.(float64)))
	case "bool":
		x.SetBool(v.(bool))
	case "date":
		var y, m, d int
		fmt.Sscanf(v.(string), "%d-%d-%d", &y, &m, &d)
		x.FieldByName("Year").SetInt(int64(y))
		x.FieldByName("Month").SetInt(int64(m))
		x.FieldByName("Day").SetInt(int64(d))
	default:
		x.SetString(v.(string))
	}
	if ptr {
		p := reflect.New(t)
		p.Elem().Set(x)
		f.Set(p)
	} else {
		f.Set(x)
	}
}

func main() {
	b, err := os.ReadFile(os.Args[1])
	if err != nil {
		panic(err)
	}
	var data rawData
	if err := json.Unmarshal(b, &data); err != nil {
		panic(err)
	}
	w := bufio.NewWriter(os.Stdout)
	defer w.Flush()
	for _, c := range data.Cases {
		held := map[[2]string]any{}
		for _, e := range c.Store {
			t, ok := records[e.Type]
			if !ok {
				continue
			}
			r := reflect.New(t)
			kinds := data.Types[e.Type].Attrs
			for i := 0; i < t.NumField(); i++ {
				f := t.Field(i)
				switch {
				case f.Name == "Roles":
					r.Elem().Field(i).Set(reflect.ValueOf(append([]string{}, e.Roles...)))
				case strings.HasPrefix(f.Name, "MemberOf"):
					ids := []string{}
					for _, m := range e.MemberOf {
						if "MemberOf"+m["type"] == f.Name {
							ids = append(ids, m["id"])
						}
					}
					r.Elem().Field(i).Set(reflect.ValueOf(ids))
				default:
					for alias, kind := range kinds {
						if exported(alias) == f.Name {
							set(r.Elem().Field(i), kind, e.Attrs[alias])
						}
					}
				}
			}
			held[[2]string{e.Type, e.ID}] = r.Interface()
		}
		fn := reflect.ValueOf(actions[c.Action])
		p := reflect.New(fn.Type().In(1)).Elem()
		p.FieldByName("Type").SetString(c.Principal["type"])
		p.FieldByName("ID").SetString(c.Principal["id"])
		in := reflect.New(fn.Type().In(2)).Elem()
		a := data.Actions[c.Action]
		for k, v := range c.Input {
			kind := a.Inputs[k]
			if k == a.Resource || k == "resource_type" {
				kind = "id"
			}
			set(in.FieldByName(exported(k)), kind, v)
		}
		now, err := time.Parse(time.RFC3339, c.Now)
		if err != nil {
			panic(err)
		}
		ans := fn.Call([]reflect.Value{reflect.ValueOf(store{held}), p, in, reflect.ValueOf(now)})[0]
		var kind any
		if e := ans.FieldByName("Err"); !e.IsNil() {
			kind = e.Elem().FieldByName("Kind").String()
		}
		line, err := json.Marshal(map[string]any{"name": c.Name, "allowed": ans.FieldByName("Allowed").Bool(), "policies": ans.FieldByName("Policies").Interface(), "error": kind, "context": ans.FieldByName("Context").Interface()})
		if err != nil {
			panic(err)
		}
		fmt.Fprintln(w, string(line))
	}
}
"#;

/// The registry of the harness for a gate's package: the store, a method for each `Store` method
/// the package declares, the record types, and each action's function.
fn registry(text: &str, actions: &[String]) -> String {
    let mut methods: Vec<(String, String)> = Vec::new();
    let mut inside = false;
    for l in text.lines() {
        if l.starts_with("type Store interface {") {
            inside = true;
            continue;
        }
        if inside && l == "}" {
            break;
        }
        if inside && let Some((m, rest)) = l.trim().split_once("(id string) (*") {
            methods.push((m.to_string(), rest.trim_end_matches(", error)").to_string()));
        }
    }
    let mut r = String::from("type store struct{ held map[[2]string]any }\n\n");
    for (m, ty) in &methods {
        r.push_str(&format!("func (s store) {m}(id string) (*gate.{ty}, error) {{\n\tx, _ := s.held[[2]string{{\"{m}\", id}}].(*gate.{ty})\n\treturn x, nil\n}}\n\n"));
    }
    r.push_str("var records = map[string]reflect.Type{\n");
    for (m, ty) in &methods {
        r.push_str(&format!("\t\"{m}\": reflect.TypeOf(gate.{ty}{{}}),\n"));
    }
    r.push_str("}\n\nvar actions = map[string]any{\n");
    for a in actions {
        r.push_str(&format!("\t\"{a}\": gate.Authorize{},\n", sekisho::r#gen::go::exported(a)));
    }
    r.push_str("}\n");
    r
}

/// The answers of the generated Go for every case of the raw values of every `.gate` that passes
/// its check, held to the reference evaluation.
#[test]
fn every_case_is_answered_as_sekisho_answers_it() {
    let Some(runner) = runner() else { return };
    let suite = common::joined();
    let t = TempDir::new("sekisho-go-cases");
    let m = t.path().join("m");
    module(&runner, &m);
    let path = module_path(&runner);
    let mut runs: Vec<(String, String, sekisho::raw::Raw)> = Vec::new();
    for (i, g) in gates().iter().enumerate() {
        let name = format!("g{i}");
        let Some((o, text)) = package(g, &m, &path, &name, Authorizer::Cedar, ritsu_base::text::Lang::En) else { continue };
        let (scope, checked) = (o.scope.as_ref().unwrap(), o.walked.as_ref().unwrap());
        let r = sekisho::raw::raw(scope, checked, &suite);
        let pkg = ritsu_emit::ident::go_package(&checked.gate.named.alias);
        let actions: Vec<String> = checked.gate.actions.iter().map(|a| a.named.alias.clone()).collect();
        let main = HARNESS.replace("IMPORT", &format!("{path}/{name}/authz/{pkg}")).replace("REGISTRY", &registry(&text, &actions));
        write(&m.join(&name).join("cmd/run/main.go"), &main);
        let data = t.path().join(format!("{name}.json"));
        std::fs::write(&data, r.json(&checked.gate)).unwrap();
        runs.push((g.clone(), name, r));
    }
    // each harness built, then run
    let bin = t.path().join("bin");
    let (mut wrong, mut lines) = (Vec::new(), Vec::new());
    let mut total = 0usize;
    for (g, name, r) in &runs {
        if r.cases.is_empty() {
            // no action: the package declares its types, and nothing asks it
            lines.push(format!("{g}: no action, so no case"));
            continue;
        }
        let built = go_in(&m).args(["build", "-o"]).arg(bin.join(name)).arg(format!("./{name}/cmd/run")).output().unwrap();
        if !built.status.success() {
            wrong.push(format!("{g}: go build: {}", String::from_utf8_lossy(&built.stderr)));
            continue;
        }
        let out = Command::new(bin.join(name)).arg(t.path().join(format!("{name}.json"))).output().unwrap();
        if !out.status.success() {
            wrong.push(format!("{g}: the harness fails:\n{}", String::from_utf8_lossy(&out.stderr)));
            continue;
        }
        match r.compare(&String::from_utf8_lossy(&out.stdout)) {
            Ok(n) => {
                total += n;
                let (refused, allowed) = r.counts();
                lines.push(format!("{g}: {n} cases ({allowed} allowed, {refused} refused) answered as sekisho answers them"));
            }
            Err(e) => wrong.push(format!("{g}: {e}")),
        }
    }
    for l in &lines {
        println!("{l}");
    }
    println!("{} gates, {total} cases, through cedar-go v1.8.0", lines.len());
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
    assert!(lines.iter().any(|l| l.starts_with("examples/refunds/refunds.gate: ")), "the example is run");
}

/// The command run as a function, every language joined: its exit code, and what it printed on
/// both outputs.
fn sekisho(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = sekisho::run::run(&args, common::joined(), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// `gen --target go` (DESIGN 5, 11): the package of each gate under `<out>/go/authz/`, importing the
/// rules' and the dates' packages from the module `--module` names; `--check` finds it current;
/// `--authorizer avp` writes another package, which asks Verified Permissions.
#[test]
fn gen_writes_the_go_of_each_gate() {
    let t = TempDir::new("sekisho-gen-go");
    let out = t.path().join("out").to_string_lossy().to_string();
    let (code, said, err) = sekisho(&["gen", "examples/refunds/refunds.gate", "examples/refunds/refunds.ja.gate", "--target", "go", "--module", "example.com/shop", "--out", &out]);
    assert_eq!(code, 0, "{said}{err}");
    for pkg in ["refunds", "refundsja"] {
        assert!(said.contains(&format!("generated: {out}/go/authz/{pkg}/{pkg}.go\n")), "{said}");
    }
    let text = std::fs::read_to_string(format!("{out}/go/authz/refunds/refunds.go")).unwrap();
    let head = format!("// Code generated by sekisho {}. DO NOT EDIT.\n// Source: refunds.gate (gate refunds v1, sha256:", env!("CARGO_PKG_VERSION"));
    assert!(text.starts_with(&head));
    assert!(text.contains("\trulesrefundlimit \"example.com/shop/rules/refundlimit\"\n") && text.contains("func AuthorizeRefundOrder(store Store, p Principal, in RefundOrderInput, now time.Time) Answer {"), "{text}");
    let (code, said, _) = sekisho(&["gen", "examples/refunds/refunds.gate", "examples/refunds/refunds.ja.gate", "--target", "go", "--module", "example.com/shop", "--out", &out, "--check"]);
    assert_eq!((code, said.as_str()), (0, ""));
    // another module, another import path
    let (code, said, _) = sekisho(&["gen", "examples/refunds/refunds.gate", "--target", "go", "--out", &out, "--check"]);
    assert_eq!(code, 1);
    assert_eq!(said, format!("differs from what gen writes: {out}/go/authz/refunds/refunds.go\n"));
    let (code, _, _) = sekisho(&["gen", "examples/refunds/refunds.gate", "--target", "go", "--authorizer", "avp", "--out", &out]);
    assert_eq!(code, 0);
    let avp = std::fs::read_to_string(format!("{out}/go/authz/refunds/refunds.go")).unwrap();
    assert!(avp.contains("\"generated/rules/refundlimit\"") && avp.contains("func AuthorizeRefundOrder(ctx context.Context, avp VerifiedPermissions, store Store,") && !avp.contains("cedar-go"), "{avp}");
}
