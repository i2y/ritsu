//! The Python sekisho generates (`gen --target python`, DESIGN 5.3, 6.3), as Python's own tools
//! read it and as cedarpy answers it. Every `.gate` of the examples and the tests that passes its
//! check is generated twice — asking cedarpy in the process (`--authorizer cedar`) and Verified
//! Permissions through boto3 (`--authorizer avp`) — into a package laid out as `ritsu gen` lays one
//! out: `rules/` and `dates/` beside `authz/`, the rules' and the dates' modules written by rulec's
//! and koyomi's own generators. Then:
//!
//! - `mypy --strict` reads every package, both authorizers (the code of `avp` is only type-checked:
//!   the tests never call AWS, DESIGN 6.3);
//! - the code of `cedar` is run on the raw values of every combination the check walks, and on the
//!   faults it is to refuse (the store's records, the input, the time; `sekisho::raw`), and each
//!   answer — allowed or not, the policies that decide as a set, the kind of a refusal, the context
//!   Cedar was given — is held to what sekisho's reference evaluation answers for the same values,
//!   through the ports of rulec and koyomi (`Raw::compare`);
//! - the entry by the action's name (`authorize`, DESIGN 5.7) is given each case too, the
//!   resource's id under its type and an id of every other type the gate's actions take beside it,
//!   and answers as the action's own function does (an action with inputs of its own, refused as
//!   `input`); a name the gate has no action of is refused as `action`, and no id, or ids of two of
//!   an action's types, as `resource`.
//!
//! The Python is tools/runner-py's: cedarpy 4.12.1, boto3 and its stubs, mypy 2.4.0, made with
//! `uv venv --python 3.13 tools/runner-py/.venv` and `uv pip sync --python
//! tools/runner-py/.venv/bin/python --require-hashes tools/runner-py/requirements.txt`. Without it,
//! a test says SKIP and passes.

mod common;

use ritsu_testkit::{Need, TempDir};
use sekisho::r#gen::Authorizer;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The runner's Python, or None with the SKIP line saying why.
fn python() -> Option<PathBuf> {
    if !ritsu_testkit::need(Need::Python) {
        return None;
    }
    // the venv's own python, not the interpreter its link points to: the packages are the venv's
    let py = Some(std::env::current_dir().unwrap().join("tools/runner-py/.venv/bin/python")).filter(|p| p.is_file());
    if py.is_none() {
        ritsu_testkit::skip("tools/runner-py/.venv is missing; make it with `uv venv --python 3.13 tools/runner-py/.venv` and `uv pip sync --python tools/runner-py/.venv/bin/python --require-hashes tools/runner-py/requirements.txt`; the generated Python is not checked");
    }
    py
}

/// Every `.gate` under `examples/` and `tests/`, sorted.
fn gates() -> Vec<String> {
    fn walk(d: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                // not into another module of ritsu's, as the commands' walks go (ritsu's DESIGN 17.6): its
                // gates read their dependencies' files, and ritsu's `tests/modules_gen.rs` generates them
                if ritsu_base::paths::not_entered(&p).is_none() {
                    walk(&p, out);
                }
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

/// The package of one gate, as `ritsu gen` lays it out under `dir/<name>/`: the rules' and the
/// dates' modules the gate reads (each written by its own language's generator), the types of the
/// units the rules take amounts in, and the gate's own, for `authorizer`. None for a gate whose check finds an error.
fn package(gate: &str, dir: &Path, name: &str, authorizer: Authorizer, lang: ritsu_base::text::Lang) -> Option<sekisho::check::Outcome> {
    let suite = common::joined();
    let o = sekisho::check::check_file(gate, &suite, &sekisho::check::Options::default()).unwrap();
    if o.has_errors() || !sekisho::spellings::stops(&o, "python").is_empty() {
        return None;
    }
    let base = dir.join(name);
    for sub in ["", "rules", "dates", "authz"] {
        write(&base.join(sub).join("__init__.py"), "");
    }
    write(&base.join("py.typed"), "");
    let scope = o.scope.as_ref().unwrap();
    let mut units = Vec::new();
    for read in scope.rules.values() {
        let src = std::fs::read_to_string(&read.path).unwrap();
        let p = read.path.to_string_lossy().to_string();
        let (rel, body, used) = rulec::codegen::package_parts(&src, &p, &p, "python", "").unwrap();
        units.extend(used);
        write(&base.join("rules").join(rel), &body);
    }
    // the types of the units the rules take amounts in, as the package's `units` module
    if let Some((rel, body)) = rulec::codegen::units_module("python", &units) {
        write(&base.join(rel), &body);
    }
    let checked = o.walked.as_ref().unwrap();
    let mut loader = koyomi::calendar::Loader::default();
    for u in &checked.gate.uses {
        if !matches!(u.kind, sekisho::model::UseKind::Dates | sekisho::model::UseKind::Calendar) {
            continue;
        }
        let p = u.file.to_string_lossy().to_string();
        let k = koyomi::check::check_file(&p, &koyomi::check::Options::default(), &mut loader).unwrap();
        let kc = k.checked.as_ref().unwrap();
        let unit = koyomi::codegen::unit_shown(kc, ritsu_base::text::Lang::En, Some(&p));
        write(&base.join("dates").join(format!("{}.py", unit.alias)), &koyomi::codegen::python::module(&unit));
    }
    let target = sekisho::r#gen::Target { name: "python", authorizer, go_module: String::new(), place: ritsu_emit::package::Place::here() };
    let shown = ritsu_emit::header::file_name(gate);
    for (rel, body) in sekisho::r#gen::code(&o, &suite, &target, &shown, lang).unwrap() {
        let rel = rel.strip_prefix("python/").unwrap_or(&rel).to_string();
        write(&base.join(rel), &body);
    }
    Some(o)
}

/// `mypy --strict` over `dir`, as the runner's mypy reads it.
fn mypy(py: &Path, dir: &Path) -> Result<String, String> {
    let out = Command::new(py).args(["-m", "mypy", "--strict", "--no-incremental", "--cache-dir", "/dev/null", "--no-color-output", "--no-error-summary", "."]).current_dir(dir).output().unwrap();
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if out.status.success() { Ok(text) } else { Err(text) }
}

/// Every `.gate` that passes its check, generated for both authorizers, and with its comments in
/// Japanese: `mypy --strict` reads each package without an error.
#[test]
fn every_package_type_checks() {
    let Some(py) = python() else { return };
    let t = TempDir::new("sekisho-py");
    let mut made = Vec::new();
    let (en, ja) = (ritsu_base::text::Lang::En, ritsu_base::text::Lang::Ja);
    for (i, g) in gates().iter().enumerate() {
        for (k, (authorizer, lang, said)) in [(Authorizer::Cedar, en, "cedar"), (Authorizer::Avp, en, "avp"), (Authorizer::Cedar, ja, "cedar, --lang ja")].into_iter().enumerate() {
            let name = format!("g{i}_{k}");
            if package(g, t.path(), &name, authorizer, lang).is_some() {
                made.push(format!("{g} ({said})"));
            }
        }
    }
    if std::env::var_os("SEKISHO_KEEP").is_some() {
        let keep = std::env::temp_dir().join("sekisho-py-kept");
        let _ = std::fs::remove_dir_all(&keep);
        let _ = Command::new("cp").arg("-R").arg(t.path()).arg(&keep).status();
    }
    match mypy(&py, t.path()) {
        Ok(_) => println!("mypy --strict passes {} packages: {}", made.len(), made.join(", ")),
        Err(said) => panic!("mypy --strict:\n{said}"),
    }
    assert!(made.iter().any(|m| m.starts_with("examples/refunds/refunds.gate")));
}

/// The harness: the cases of the raw values (`Raw::json`) run through the generated module, an
/// answer a line, as `Raw::compare` reads them. The store holds the records the cases give, each
/// made as the module's dataclass of its type; a type with no record (one the code knows by its id
/// alone) is not held.
const HARNESS: &str = r#"import dataclasses
import importlib
import json
import sys
from datetime import date, datetime

m = importlib.import_module(sys.argv[1])
with open(sys.argv[2], encoding="utf-8") as f:
    raw = json.load(f)
types, actions = raw["types"], raw["actions"]
# for the entry by the action's name: the types each action takes, the other types the gate's
# actions take, and the actions with inputs of their own
with open(sys.argv[3], encoding="utf-8") as f:
    by_name = json.load(f)
takes, others, own = by_name["takes"], by_name["others"], set(by_name["own"])
# the names roles_of is given: the gate's types, and one it does not declare
names = by_name["types"]


def snake(alias):
    out = ""
    for i, c in enumerate(alias):
        if c.isupper():
            if i > 0 and not out.endswith("_"):
                out += "_"
            out += c.lower()
        else:
            out += c
    return out


def value(kind, v):
    if v is None:
        return None
    return date.fromisoformat(v) if kind == "date" else v


class Store:
    def __init__(self, held):
        self.held = held

    def __getattr__(self, method):
        return lambda id: self.held.get((method, id))


def record(e):
    cls = getattr(m, e["type"], None)
    if cls is None or not dataclasses.is_dataclass(cls):
        return None
    kinds = types[e["type"]]["attrs"]
    args = {}
    for f in dataclasses.fields(cls):
        # the fields of the roles and the groups, moved aside from an attribute of their name
        if f.name not in kinds and f.name.rstrip("_") == "roles":
            args[f.name] = list(e["roles"])
        elif f.name not in kinds and f.name.startswith("member_of_"):
            args[f.name] = [x["id"] for x in e["member_of"] if "member_of_" + snake(x["type"]) == f.name]
        else:
            args[f.name] = value(kinds.get(f.name), e["attrs"].get(f.name))
    return cls(**args)


def said(a):
    return [a.allowed, sorted(a.policies), a.error.kind if a.error else None, a.context]


same, refused, narrowed = 0, 0, 0
for case in raw["cases"]:
    held = {}
    for e in case["store"]:
        r = record(e)
        if r is not None:
            held[(snake(e["type"]), e["id"])] = r
    kinds = actions[case["action"]]["inputs"]
    given = {k: value(kinds.get(k), v) for k, v in case["input"].items()}
    principal = m.Principal(case["principal"]["type"], case["principal"]["id"])
    now = datetime.fromisoformat(case["now"])
    a = getattr(m, "authorize_" + case["action"])(Store(held), principal, given, now)
    print(json.dumps({"name": case["name"], "allowed": a.allowed, "policies": a.policies, "error": a.error.kind if a.error else None, "context": a.context}, ensure_ascii=False))
    # the entry by the action's name, given the same: the resource's id under its type, beside an
    # id of each type of the gate's the action does not take
    resources = {t: "other-" + t for t in others[case["action"]]}
    ts = takes[case["action"]]
    rt = ts[0] if len(ts) == 1 else case["input"].get("resource_type") if isinstance(case["input"].get("resource_type"), str) else ""
    rid = case["input"].get(actions[case["action"]]["resource"])
    if isinstance(rid, str) and rt:
        resources[rt] = rid
    b = m.authorize(Store(held), principal, case["action"], resources, now)
    if case["action"] in own:
        if b.error is None or b.error.kind != "input" or b.allowed or b.policies:
            print(f"by name differs: {case['name']}: {said(b)}, not a refusal of the kind input")
        else:
            refused += 1
    elif said(b) != said(a):
        print(f"by name differs: {case['name']}: {said(b)}, not {said(a)}")
    else:
        same += 1
    # the roles a sign-in carries, every role of the gate's and one it does not declare, passed
    # through roles_of of the principal's type as a service does: asked again, the entry does not
    # refuse the principal
    key = (snake(case["principal"]["type"]), case["principal"]["id"])
    r = held.get(key)
    field = None
    if r is not None:
        kinds_of = types[case["principal"]["type"]]["attrs"]
        field = next((f.name for f in dataclasses.fields(r) if f.name not in kinds_of and f.name.rstrip("_") == "roles"), None)
    if field is not None and case["action"] not in own and b.error is None:
        may = m.roles_of(case["principal"]["type"])
        kept = [x for x in m.roles() + ["no_such_role"] if x in may]
        again = dict(held)
        again[key] = dataclasses.replace(r, **{field: kept})
        d = m.authorize(Store(again), principal, case["action"], resources, now)
        if d.error is not None and d.error.kind == "principal":
            print(f"roles of differs: {case['name']}: {kept} refused as the principal: {d.error}")
        else:
            narrowed += 1

# what the entry refuses before any function is asked
probes = 0
none = datetime.fromisoformat("2026-01-01T00:00:00+00:00")
nobody = m.Principal("", "")


def probe(what, b, kind, message):
    global probes
    probes += 1
    if b.error is None or b.error.kind != kind or b.allowed or b.policies or (message and str(b.error) != message):
        print(f"by name differs: {what}: {said(b)}, not a refusal of the kind {kind} {message}")


probe("a name of no action", m.authorize(Store({}), nobody, "no such action", {}, none), "action", 'the gate has no action "no such action"')
for action, ts in takes.items():
    if action in own:
        continue
    probe(f"{action} with no id", m.authorize(Store({}), nobody, action, {}, none), "resource", "")
    if len(ts) > 1:
        probe(f"{action} with an id of each of its types", m.authorize(Store({}), nobody, action, {t: "x" for t in ts}, none), "resource", "")
print(f"by name: {same} answered as the action's own function answers, {refused} refused as taking inputs of their own, {probes} refusals before any function")
# the list of the roles the gate declares, a new list each call
listed = m.roles()
again = m.roles()
if again:
    again[0] = "changed"
print("roles: " + json.dumps(listed, separators=(",", ":")) + " " + json.dumps(m.roles(), separators=(",", ":")))
# the roles of each type, and of a name the gate does not declare
print(f"roles of: {narrowed} asked again with the roles roles_of leaves")
for t in names:
    print(f"roles of {t}: " + json.dumps(m.roles_of(t), separators=(",", ":")))
"#;

/// What the harness reads for the entry by the action's name, as JSON: the types each action
/// takes, the other types the gate's actions take, and the actions with inputs of their own.
fn by_name(g: &sekisho::model::Gate) -> String {
    let q = |x: &str| ritsu_base::json::quote(x);
    let every: std::collections::BTreeSet<usize> = g.actions.iter().flat_map(|a| a.resources.iter().copied()).collect();
    let types = |ts: &mut dyn Iterator<Item = usize>| ts.map(|t| q(&g.types[t].named.alias)).collect::<Vec<_>>().join(", ");
    let takes: Vec<String> = g.actions.iter().map(|a| format!("{}: [{}]", q(&a.named.alias), types(&mut a.resources.iter().copied()))).collect();
    let others: Vec<String> = g.actions.iter().map(|a| format!("{}: [{}]", q(&a.named.alias), types(&mut every.iter().copied().filter(|t| !a.resources.contains(t))))).collect();
    let own: Vec<String> = g.actions.iter().filter(|a| !a.inputs.is_empty()).map(|a| q(&a.named.alias)).collect();
    let names: Vec<String> = g.types.iter().map(|t| q(&t.named.alias)).chain([q("no such type")]).collect();
    format!("{{\"takes\": {{{}}}, \"others\": {{{}}}, \"own\": [{}], \"types\": [{}]}}", takes.join(", "), others.join(", "), own.join(", "), names.join(", "))
}

/// What the harness prints of `roles_of` for each type of the gate and for a name it does not
/// declare: the aliases of the roles of the type's `roles` line, in the order the gate declares the
/// roles, as `json.dumps` writes them with no spaces.
fn lines_of_roles_of(g: &sekisho::model::Gate) -> Vec<String> {
    let mut out: Vec<String> = g
        .types
        .iter()
        .map(|t| {
            let mut rs = t.roles.clone();
            rs.sort_unstable();
            rs.dedup();
            let each: Vec<String> = rs.iter().map(|&r| ritsu_base::json::quote(&g.roles[r].named.alias)).collect();
            format!("roles of {}: [{}]", t.named.alias, each.join(","))
        })
        .collect();
    out.push("roles of no such type: []".to_string());
    out
}

/// The aliases of the roles a gate declares, in its order, as `json.dumps` writes a list of them
/// with no spaces.
fn roles_of(g: &sekisho::model::Gate) -> String {
    let each: Vec<String> = g.roles.iter().map(|r| ritsu_base::json::quote(&r.named.alias)).collect();
    format!("[{}]", each.join(","))
}

/// The answers of the generated Python for every case of the raw values of every `.gate` that
/// passes its check, held to the reference evaluation.
#[test]
fn every_case_is_answered_as_sekisho_answers_it() {
    let Some(py) = python() else { return };
    let suite = common::joined();
    let t = TempDir::new("sekisho-py-cases");
    std::fs::write(t.path().join("harness.py"), HARNESS).unwrap();
    let (mut wrong, mut lines) = (Vec::new(), Vec::new());
    let (mut total, mut narrowed) = (0usize, 0usize);
    for (i, g) in gates().iter().enumerate() {
        let name = format!("g{i}");
        let Some(o) = package(g, t.path(), &name, Authorizer::Cedar, ritsu_base::text::Lang::En) else { continue };
        let (scope, checked) = (o.scope.as_ref().unwrap(), o.walked.as_ref().unwrap());
        let r = sekisho::raw::raw(scope, checked, &suite);
        let data = t.path().join(format!("{name}.json"));
        std::fs::write(&data, r.json(&checked.gate)).unwrap();
        let named = t.path().join(format!("{name}.by-name.json"));
        std::fs::write(&named, by_name(&checked.gate)).unwrap();
        let module = format!("{name}.authz.{}", checked.gate.named.alias);
        let out = Command::new(&py).arg(t.path().join("harness.py")).arg(&module).arg(&data).arg(&named).current_dir(t.path()).env("PYTHONPATH", t.path()).env("PYTHON_COLORS", "0").output().unwrap();
        if !out.status.success() {
            wrong.push(format!("{g}: the harness fails:\n{}", String::from_utf8_lossy(&out.stderr)));
            continue;
        }
        let printed = String::from_utf8_lossy(&out.stdout);
        match r.compare(&printed) {
            Ok(0) => lines.push(format!("{g}: no action, so no case")),
            Ok(n) => {
                total += n;
                let (refused, allowed) = r.counts();
                lines.push(format!("{g}: {n} cases ({allowed} allowed, {refused} refused) answered as sekisho answers them"));
            }
            Err(e) => wrong.push(format!("{g}: {e}")),
        }
        // the entry by the action's name
        let differs: Vec<&str> = printed.lines().filter(|l| l.starts_with("by name differs: ")).collect();
        if !differs.is_empty() {
            wrong.push(format!("{g}: {} answers of authorize differ:\n{}", differs.len(), differs.iter().take(20).copied().collect::<Vec<_>>().join("\n")));
        }
        match printed.lines().find(|l| l.starts_with("by name: ")) {
            Some(l) => lines.push(format!("{g}: {}", l.trim_start_matches("by name: "))),
            None => wrong.push(format!("{g}: the harness does not say what authorize answered")),
        }
        // roles() is the gate's roles as it declares them, and a change to what it returned is not
        // in the next
        let declared = roles_of(&checked.gate);
        match printed.lines().find_map(|l| l.strip_prefix("roles: ")) {
            Some(l) if l == format!("{declared} {declared}") => {}
            other => wrong.push(format!("{g}: roles() gave {other:?}, not {declared} twice")),
        }
        // roles_of of each type, and the cases asked again with the roles it leaves
        for want in lines_of_roles_of(&checked.gate) {
            if !printed.lines().any(|l| l == want) {
                wrong.push(format!("{g}: no line {want:?}"));
            }
        }
        let differs: Vec<&str> = printed.lines().filter(|l| l.starts_with("roles of differs: ")).collect();
        if !differs.is_empty() {
            wrong.push(format!("{g}: {} cases refused with the roles roles_of leaves:\n{}", differs.len(), differs.iter().take(20).copied().collect::<Vec<_>>().join("\n")));
        }
        match printed.lines().find_map(|l| l.strip_prefix("roles of: ")).and_then(|l| l.split(' ').next()?.parse::<usize>().ok()) {
            Some(n) => {
                narrowed += n;
                if g.ends_with("tests/gen/roles_of.gate") && n == 0 {
                    wrong.push(format!("{g}: no case asked again with the roles roles_of leaves"));
                }
            }
            None => wrong.push(format!("{g}: the harness does not say what it asked with the roles roles_of leaves")),
        }
    }
    println!("{narrowed} cases asked again with every role of the gate's and one more, passed through roles_of, and none refused as the principal");
    for l in &lines {
        println!("{l}");
    }
    println!("{} gates, {total} cases, through cedarpy 4.12.1", lines.len());
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

/// `gen --target python` (DESIGN 5, 11): the module of each gate under `<out>/python/authz/`, its
/// head naming the gate; `--check` finds it current; `--authorizer avp` writes another module,
/// which asks Verified Permissions; `--lang ja` writes its comments in Japanese.
#[test]
fn gen_writes_the_python_of_each_gate() {
    let t = TempDir::new("sekisho-gen-py");
    let out = t.path().join("out").to_string_lossy().to_string();
    let (code, said, err) = sekisho(&["gen", "examples/refunds/refunds.gate", "examples/refunds/refunds.ja.gate", "--target", "python", "--out", &out]);
    assert_eq!(code, 0, "{said}{err}");
    for alias in ["refunds", "refunds_ja"] {
        assert!(said.contains(&format!("generated: {out}/python/authz/{alias}.py\n")), "{said}");
    }
    let text = std::fs::read_to_string(format!("{out}/python/authz/refunds.py")).unwrap();
    let head = format!("# Code generated by sekisho {}. DO NOT EDIT.\n# Source: refunds.gate (gate refunds v1, sha256:", env!("CARGO_PKG_VERSION"));
    assert!(text.starts_with(&head), "{}", &text[..200.min(text.len())]);
    assert!(text.contains("import cedarpy") && text.contains("def authorize_refund_order(store: Store, principal: Principal, input: RefundOrderInput, now: datetime | None = None) -> Answer:"));
    let (code, said, _) = sekisho(&["gen", "examples/refunds/refunds.gate", "examples/refunds/refunds.ja.gate", "--target", "python", "--out", &out, "--check"]);
    assert_eq!((code, said.as_str()), (0, ""));
    // the code of Verified Permissions, in place of cedarpy's
    let (code, said, _) = sekisho(&["gen", "examples/refunds/refunds.gate", "--target", "python", "--authorizer", "avp", "--out", &out, "--check"]);
    assert_eq!(code, 1);
    assert_eq!(said, format!("differs from what gen writes: {out}/python/authz/refunds.py\n"));
    let (code, _, _) = sekisho(&["gen", "examples/refunds/refunds.gate", "--target", "python", "--authorizer", "avp", "--out", &out]);
    assert_eq!(code, 0);
    let avp = std::fs::read_to_string(format!("{out}/python/authz/refunds.py")).unwrap();
    assert!(!avp.contains("import cedarpy") && avp.contains("def authorize_refund_order(avp: VerifiedPermissions, store: Store,"), "{avp}");
    // the comments in Japanese
    let (code, _, _) = sekisho(&["gen", "examples/refunds/refunds.gate", "--target", "python", "--out", &out, "--lang", "ja"]);
    assert_eq!(code, 0);
    let ja = std::fs::read_to_string(format!("{out}/python/authz/refunds.py")).unwrap();
    assert!(ja.contains("# もと: refunds.gate（gate refunds v1、sha256:") && ja.contains("組み立てられなければ SekishoError を投げます"), "{}", &ja[..400.min(ja.len())]);
    // an authorizer it does not know
    let (code, _, err) = sekisho(&["gen", "examples/refunds/refunds.gate", "--target", "python", "--authorizer", "opa", "--out", &out]);
    assert_eq!(code, 2, "{err}");
}
