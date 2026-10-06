//! The Cedar sekisho writes, held to the official Cedar CLI (`cedar-policy-cli` 4.13.0; DESIGN
//! 6.1). Every `.gate` of the examples and the tests that passes its check is generated
//! (`sekisho gen --target cedar`, every language joined as `ritsu sekisho` joins them), and the
//! CLI is asked:
//!
//! - `validate`: the policies against the schema, strict, with no error and no warning, in the
//!   Cedar formats and in the JSON formats (the schema's is what Verified Permissions takes, the
//!   policies' what cedar-wasm, cedarpy and cedar-go are given; DESIGN 5.1, 5.2);
//! - `format --check`: the text is laid out as `cedar format` lays it out;
//! - `translate-policy` and `translate-schema`: the JSON formats are what they print, to the byte;
//! - `run-tests`: the tests `sekisho vectors` writes, every combination the check walks (a number
//!   at both ends of its cell), with the decision and the determining policies of sekisho's
//!   reference evaluation, and no evaluation error. `run-tests` holds the determining policies it
//!   is given to be among those it finds, not to be all of them
//!   (`cedar-policy-cli/src/command/run_test.rs`); so each policy is run again alone, as a permit,
//!   on every test whose answer says whether it holds. The two together make the determining
//!   policies the same;
//! - and the schema refuses a test with an attribute it requires left out (`run-tests` says it is
//!   an error, and answers none): Verified Permissions holds a request to the schema it is given,
//!   and a schema that took such a request would answer it as if the value were absent.
//!
//! Then the generated text is changed at one place at a time (a constant moved by one, a `has`
//! taken away, a forbid made a permit, a role, an action or a type put in another's place, …),
//! each change read and written back with ritsu-base's Cedar, so every one is Cedar the CLI reads
//! and lays out as it would: every change must fail one of the checks. (That the English and the
//! Japanese versions of a gate write the same Cedar but for the names is `tests/gen.rs`'s.)
//!
//! The CLI is found by ritsu-testkit (`RITSU_CEDAR`, or `cedar` on the PATH, at 4.13.0); without
//! it, a test that runs it says SKIP and passes.

mod common;

use ritsu_base::cedar::{self, ActionScope, BinOp, CondKind, Effect, EntityKind, EntityOrSlot, EntityUid, Expr, ExprKind, Name, Policy, PolicySet, Schema, Scope, Type};
use ritsu_base::json::{self, Json};
use ritsu_testkit::TempDir;
use ritsu_testkit::run::Ran;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// The longest one run of the CLI may take: `run-tests` on the 2,134 tests of the example takes a
/// third of a second on the machine the test was written on.
const LIMIT: Duration = Duration::from_secs(300);

/// The CLI's words without the colours it prints even into a pipe (`run-tests` colours `ok`, and
/// `NO_COLOR` does not stop it).
fn plain(s: &str) -> String {
    let esc = char::from(27u8);
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == esc && it.peek() == Some(&'[') {
            it.next();
            for d in it.by_ref() {
                if d.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// The official CLI.
struct Cli {
    bin: PathBuf,
}

impl Cli {
    /// The CLI at 4.13.0, or None with the SKIP line saying why.
    fn find() -> Option<Cli> {
        ritsu_testkit::cedar::cli().map(|bin| Cli { bin })
    }

    /// The CLI run in `dir` on the files there.
    fn run(&self, dir: &Path, args: &[&str]) -> Ran {
        let mut c = Command::new(&self.bin);
        c.current_dir(dir).args(args).env("CEDAR_ERROR_FORMAT", "plain").env("NO_COLOR", "1");
        let mut r = ritsu_testkit::run(&mut c, LIMIT);
        r.stdout = plain(&r.stdout);
        r.stderr = plain(&r.stderr);
        r
    }

    /// `cedar validate`, strict, failing on a warning too: in the Cedar formats, or in the JSON
    /// formats.
    fn validate(&self, dir: &Path, policies: &str, schema: &str, json: bool) -> Result<(), String> {
        let mut args = vec!["validate", "--schema", schema, "--policies", policies, "--validation-mode", "strict", "--deny-warnings"];
        if json {
            args.extend(["--schema-format", "json", "--policy-format", "json"]);
        }
        let r = self.run(dir, &args);
        if r.ok && r.both().contains("no errors or warnings") { Ok(()) } else { Err(format!("cedar {}: exit {:?}\n{}", args.join(" "), r.code, r.both())) }
    }

    /// `cedar format --check`: it prints the text as it lays it out, which is the text written.
    fn format_check(&self, dir: &Path, policies: &str, text: &str) -> Result<(), String> {
        let r = self.run(dir, &["format", "--check", "--policies", policies]);
        if r.ok && r.stdout == text {
            Ok(())
        } else {
            Err(format!("cedar format --check --policies {policies}: exit {:?}\n{}{}", r.code, ritsu_testkit::golden::line_diff(text, &r.stdout), r.stderr))
        }
    }

    /// A `translate-*` of the CLI prints what was written, to the byte.
    fn translates_to(&self, dir: &Path, args: &[&str], written: &str) -> Result<(), String> {
        let r = self.run(dir, args);
        if r.ok && r.stdout == written {
            Ok(())
        } else {
            Err(format!("cedar {}: exit {:?}, and what it prints differs from the file {}\n{}", args.join(" "), r.code, first_difference(&r.stdout, written), r.stderr))
        }
    }

    /// `cedar run-tests`: every one of the `n` cases passes.
    fn run_tests(&self, dir: &Path, policies: &str, schema: &str, tests: &str, n: usize, json: bool) -> Result<(), String> {
        let mut args = vec!["run-tests", "--policies", policies, "--schema", schema, "--tests", tests];
        if json {
            args.extend(["--schema-format", "json", "--policy-format", "json"]);
        }
        let r = self.run(dir, &args);
        let all = format!("results: {n} passed, 0 failed");
        if r.ok && r.stdout.lines().any(|l| l == all) {
            return Ok(());
        }
        let failed: Vec<&str> = r.stdout.lines().filter(|l| l.starts_with("  test ") && !l.ends_with(" ok")).take(6).collect();
        let last = r.stdout.lines().rfind(|l| l.starts_with("results: ")).unwrap_or("no results line");
        Err(format!("cedar {}: exit {:?}, {last} (of {n})\n{}\n{}", args.join(" "), r.code, failed.join("\n"), r.stderr))
    }

    /// `cedar run-tests` on tests the schema is to refuse: each is said to be an error, and none
    /// is answered.
    fn refuses(&self, dir: &Path, policies: &str, schema: &str, tests: &str, names: &[String], json: bool) -> Result<(), String> {
        let mut args = vec!["run-tests", "--policies", policies, "--schema", schema, "--tests", tests];
        if json {
            args.extend(["--schema-format", "json", "--policy-format", "json"]);
        }
        let r = self.run(dir, &args);
        let answered: Vec<String> = names.iter().filter(|n| !r.stdout.lines().any(|l| l == format!("  test {n} ... error:"))).map(|n| format!("`{n}`")).collect();
        if answered.is_empty() {
            Ok(())
        } else {
            Err(format!("cedar {}: the schema takes {} of the {} tests that each leave out an attribute it requires: {}\n{}", args.join(" "), answered.len(), names.len(), answered.join(", "), r.stdout))
        }
    }
}

/// Where two texts first differ, with a little of each around it.
fn first_difference(cli: &str, file: &str) -> String {
    let at = cli.char_indices().zip(file.chars()).find(|((_, a), b)| a != b).map(|((i, _), _)| i).unwrap_or(cli.len().min(file.len()));
    let around = |s: &str| {
        let from = s.floor_char_boundary(at.saturating_sub(60));
        let to = s.ceil_char_boundary((at + 60).min(s.len()));
        format!("…{}…", &s[from..to])
    };
    format!("at byte {at}:\n  the CLI: {}\n  the file: {}", around(cli), around(file))
}

/// One test of `sekisho vectors`: the case as `run-tests` reads it, its name, and its answer.
struct Case {
    json: Json,
    name: String,
    allow: bool,
    reason: Vec<String>,
}

impl Case {
    /// The combination the test is of: its name without the end of the cells it is tried at
    /// (`refund_order 12 (amount 50)` is of `refund_order 12`).
    fn combination(&self) -> &str {
        match self.name.rfind(" (") {
            Some(i) if self.name.ends_with(')') => &self.name[..i],
            _ => &self.name,
        }
    }
}

/// The cases of the vectors, each held to say no evaluation error.
fn cases(text: &str) -> Result<Vec<Case>, String> {
    let v = json::parse(text).map_err(|e| format!("the vectors are not JSON: {e:?}"))?;
    let arr = v.as_arr().ok_or("the vectors are not an array")?;
    let mut out = Vec::new();
    let mut names = std::collections::BTreeSet::new();
    for c in arr {
        let name = c.get("name").and_then(Json::as_str).ok_or("a case without a name")?;
        if !names.insert(name.to_string()) {
            return Err(format!("two cases are named `{name}`"));
        }
        let allow = match c.get("decision").and_then(Json::as_str) {
            Some("allow") => true,
            Some("deny") => false,
            d => return Err(format!("`{name}`: the decision is {d:?}")),
        };
        let reason = c.get("reason").and_then(Json::as_arr).ok_or_else(|| format!("`{name}`: no reason"))?;
        let reason: Vec<String> = reason.iter().map(|r| r.as_str().map(str::to_string).ok_or_else(|| format!("`{name}`: a reason that is not an id"))).collect::<Result<_, _>>()?;
        match c.get("num_errors").and_then(Json::as_int) {
            Some(0) => {}
            e => return Err(format!("`{name}`: num_errors is {e:?}, where sekisho's Cedar is to evaluate without an error (DESIGN 3.8)")),
        }
        out.push(Case { json: c.clone(), name: name.to_string(), allow, reason });
    }
    Ok(out)
}

/// An object without one of its keys.
fn without(o: &Json, key: &str) -> Json {
    match o {
        Json::Obj(pairs) => Json::Obj(pairs.iter().filter(|(k, _)| k != key).cloned().collect()),
        _ => o.clone(),
    }
}

/// An object with one of its keys given another value.
fn with(o: &Json, key: &str, v: Json) -> Json {
    match o {
        Json::Obj(pairs) => Json::Obj(pairs.iter().map(|(k, x)| (k.clone(), if k == key { v.clone() } else { x.clone() })).collect()),
        _ => o.clone(),
    }
}

/// For each attribute the schema requires (of an entity type, or of an action's context) that a
/// test of the vectors holds: the first such test with the attribute left out, which the schema
/// is to refuse. Its name, and the test.
fn lacking(schema: &Schema, cases: &[Case]) -> Vec<(String, Json)> {
    let mut out = Vec::new();
    for ns in &schema.namespaces {
        let q = |id: &str| match &ns.name {
            Some(n) => format!("{n}::{id}"),
            None => id.to_string(),
        };
        for et in &ns.entity_types {
            let EntityKind::Standard { shape: Type::Record(rec), .. } = &et.kind else { continue };
            let ty = q(&et.name);
            for a in rec.attrs.iter().filter(|a| a.required) {
                let found = cases.iter().find_map(|c| {
                    let entities = c.json.get("entities")?.as_arr()?;
                    let k = entities.iter().position(|e| e.get("uid").and_then(|u| u.get("type")).and_then(Json::as_str) == Some(ty.as_str()) && e.get("attrs").and_then(|x| x.get(&a.name)).is_some())?;
                    Some((c, entities, k))
                });
                let Some((c, entities, k)) = found else { continue };
                let mut es = entities.to_vec();
                es[k] = with(&es[k], "attrs", without(es[k].get("attrs").unwrap(), &a.name));
                let name = format!("{} without {ty}.{}", c.name, a.name);
                out.push((name.clone(), with(&with(&c.json, "entities", Json::Arr(es)), "name", Json::str(name))));
            }
        }
        for act in &ns.actions {
            let Some(cedar::AppliesTo { context: Type::Record(rec), .. }) = &act.applies_to else { continue };
            let uid = format!("{}::\"{}\"", q("Action"), act.name);
            for a in rec.attrs.iter().filter(|a| a.required) {
                let found = cases.iter().find(|c| {
                    let r = c.json.get("request");
                    r.and_then(|r| r.get("action")).and_then(Json::as_str) == Some(uid.as_str()) && r.and_then(|r| r.get("context")).and_then(|x| x.get(&a.name)).is_some()
                });
                let Some(c) = found else { continue };
                let request = c.json.get("request").unwrap();
                let request = with(request, "context", without(request.get("context").unwrap(), &a.name));
                let name = format!("{} without context.{}", c.name, a.name);
                out.push((name.clone(), with(&with(&c.json, "request", request), "name", Json::str(name))));
            }
        }
    }
    out
}

/// The tests that leave out a required attribute, written beside the others: their file and
/// their names.
fn write_lacking(dir: &Path, lacking: &[(String, Json)]) -> (String, Vec<String>) {
    let file = dir.join("lacking.tests.json");
    std::fs::write(&file, Json::arr(lacking.iter().map(|(_, t)| t.clone())).compact()).unwrap();
    (file.to_string_lossy().to_string(), lacking.iter().map(|(n, _)| n.clone()).collect())
}

/// A case with another answer: the same request and entities.
fn answered(c: &Case, allow: bool, reason: Option<&str>) -> Json {
    let Json::Obj(pairs) = &c.json else { unreachable!("a case is an object") };
    Json::Obj(
        pairs
            .iter()
            .map(|(k, v)| {
                let v = match k.as_str() {
                    "decision" => Json::str(if allow { "allow" } else { "deny" }),
                    "reason" => Json::arr(reason.map(Json::str)),
                    _ => v.clone(),
                };
                (k.clone(), v)
            })
            .collect(),
    )
}

/// Each policy alone, made a permit, on the tests whose answer says whether it holds: a forbid
/// holds where it is a determining policy of a deny, and nowhere else; a permit holds where it is
/// one of an allow, and not where the answer is an allow without it or a deny that no forbid
/// decides (where a forbid decides, the answer does not say whether a permit holds). The number of
/// policies run.
fn each_alone(cli: &Cli, dir: &Path, set: &PolicySet, schema: &str, cases: &[Case]) -> Result<usize, String> {
    for (i, p) in set.policies.iter().enumerate() {
        let forbid = p.effect == Effect::Forbid;
        let mut alone = p.clone();
        alone.effect = Effect::Permit;
        let text = cedar::write_policies(&PolicySet { policies: vec![alone] }).map_err(|e| format!("{}: ritsu-base does not write it: {e:?}", p.id))?;
        let picked: Vec<Json> = cases
            .iter()
            .filter(|c| forbid || c.allow || c.reason.is_empty())
            .map(|c| {
                let holds = c.reason.contains(&p.id);
                answered(c, holds, holds.then_some(p.id.as_str()))
            })
            .collect();
        let (pf, tf, n) = (format!("alone-{i}.cedar"), format!("alone-{i}.tests.json"), picked.len());
        std::fs::write(dir.join(&pf), text).unwrap();
        std::fs::write(dir.join(&tf), Json::arr(picked).compact()).unwrap();
        cli.run_tests(dir, &pf, schema, &tf, n, false).map_err(|e| format!("`{}` alone, made a permit: {e}", p.id))?;
    }
    Ok(set.policies.len())
}

/// The four files `sekisho gen --target cedar` writes for a `.gate` under `<out>/cedar/` (DESIGN
/// 5.1, 5.2, 5.6), named after the file's alias.
struct Written {
    dir: PathBuf,
    alias: String,
    policies: String,
    schema: String,
    schema_json: String,
    policies_json: String,
}

impl Written {
    /// What is in `dir`: `<alias>.cedar`, `<alias>.cedarschema`, `<alias>.cedarschema.json` and
    /// `<alias>.policies.json`, and nothing else.
    fn read(dir: &Path) -> Result<Written, String> {
        let mut names: Vec<String> = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?.map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
        names.sort();
        let alias = names.iter().find_map(|n| n.strip_suffix(".cedarschema")).ok_or_else(|| format!("no .cedarschema among {names:?}"))?.to_string();
        let want: Vec<String> = ["cedar", "cedarschema", "cedarschema.json", "policies.json"].iter().map(|e| format!("{alias}.{e}")).collect();
        let mut sorted = want.clone();
        sorted.sort();
        if names != sorted {
            return Err(format!("{} holds {names:?}, where gen writes {want:?}", dir.display()));
        }
        let read = |n: &str| std::fs::read_to_string(dir.join(n)).map_err(|e| format!("{n}: {e}"));
        Ok(Written { dir: dir.to_path_buf(), policies: read(&want[0])?, schema: read(&want[1])?, schema_json: read(&want[2])?, policies_json: read(&want[3])?, alias })
    }

    fn file(&self, ext: &str) -> String {
        format!("{}.{ext}", self.alias)
    }
}

/// The tests `sekisho vectors` writes for a `.gate` (every combination, its numbers at both ends
/// of their cells), in the file `run-tests` reads.
struct Vectors {
    file: PathBuf,
    text: String,
}

/// What the CLI answered for one `.gate`, for the line the test prints: the tests, the
/// combinations they are of, the combinations allowed, the policies run alone, and the tests
/// that leave out a required attribute, refused.
struct Held {
    tests: usize,
    combinations: usize,
    allowed: usize,
    policies: usize,
    lacking: usize,
}

/// The CLI on the text of what was written: validated, strict, in both formats; laid out as
/// `cedar format` lays it out; translated to the JSON formats written beside it.
fn hold_text(cli: &Cli, w: &Written) -> Result<(), String> {
    let (p, s, sj, pj) = (w.file("cedar"), w.file("cedarschema"), w.file("cedarschema.json"), w.file("policies.json"));
    let dir = &w.dir;
    cli.validate(dir, &p, &s, false)?;
    cli.validate(dir, &pj, &sj, true)?;
    cli.format_check(dir, &p, &w.policies)?;
    cli.translates_to(dir, &["translate-policy", "--direction", "cedar-to-json", "--policies", &p], &w.policies_json)?;
    cli.translates_to(dir, &["translate-schema", "--direction", "cedar-to-json", "--schema", &s], &w.schema_json)?;
    Ok(())
}

/// The CLI's answers on every test of the vectors, in both formats, and each policy alone; and
/// the schema refuses each test with an attribute it requires left out (in both formats too:
/// Verified Permissions holds a request to the schema it is given).
fn hold_answers(cli: &Cli, w: &Written, v: &Vectors) -> Result<Held, String> {
    let (p, s, sj, pj) = (w.file("cedar"), w.file("cedarschema"), w.file("cedarschema.json"), w.file("policies.json"));
    let dir = &w.dir;
    let cases = cases(&v.text)?;
    let vf = v.file.to_string_lossy().to_string();
    cli.run_tests(dir, &p, &s, &vf, cases.len(), false)?;
    cli.run_tests(dir, &pj, &sj, &vf, cases.len(), true)?;
    let set = cedar::parse_policies(&w.policies).map_err(|e| format!("ritsu-base does not read {p}: {e:?}"))?;
    let scratch = dir.join("../alone");
    std::fs::create_dir_all(&scratch).unwrap();
    std::fs::copy(dir.join(&s), scratch.join(&s)).unwrap();
    let policies = each_alone(cli, &scratch, &set, &s, &cases)?;
    let schema = cedar::parse_schema(&w.schema).map_err(|e| format!("ritsu-base does not read {s}: {e:?}"))?;
    let lacking = lacking(&schema, &cases);
    let (lf, names) = write_lacking(&scratch, &lacking);
    cli.refuses(dir, &p, &s, &lf, &names, false)?;
    cli.refuses(dir, &pj, &sj, &lf, &names, true)?;
    // the tests of one combination (its cells tried at both ends) answer alike
    let mut combos: std::collections::BTreeMap<&str, (bool, &[String])> = std::collections::BTreeMap::new();
    for c in &cases {
        if let Some((allow, reason)) = combos.insert(c.combination(), (c.allow, &c.reason))
            && (allow, reason) != (c.allow, &c.reason[..])
        {
            return Err(format!("the tests of `{}` answer otherwise: `{}` is one of them", c.combination(), c.name));
        }
    }
    let allowed = combos.values().filter(|(allow, _)| *allow).count();
    Ok(Held { tests: cases.len(), combinations: combos.len(), allowed, policies, lacking: lacking.len() })
}

/// The command run as a function, every language joined: its exit code, what it printed, and
/// what it printed on stderr.
fn sekisho(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = sekisho::run::run(&args, common::joined(), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// What gen writes for a `.gate` in `lang` (the language of the heads and of the `@doc` sekisho
/// writes), in a directory of its own. None when the check finds an error: gen then writes
/// nothing.
fn generate(gate: &str, lang: &str) -> Result<Option<(TempDir, Written)>, String> {
    let t = TempDir::new("cedar");
    let out = t.path().join("out");
    let (code, stdout, stderr) = sekisho(&["gen", gate, "--target", "cedar", "--out", out.to_str().unwrap(), "--lang", lang]);
    match code {
        0 => {}
        1 => return if out.exists() { Err(format!("{gate}: gen finds an error, and writes {}", out.display())) } else { Ok(None) },
        c => return Err(format!("{gate}: gen exits {c}\n{stdout}{stderr}")),
    }
    let w = Written::read(&out.join("cedar")).map_err(|e| format!("{gate}: {e}"))?;
    Ok(Some((t, w)))
}

/// What `sekisho vectors` writes for a `.gate`, kept in `dir/vectors.json`.
fn vectors(gate: &str, dir: &Path) -> Result<Vectors, String> {
    let (code, text, stderr) = sekisho(&["vectors", gate]);
    if code != 0 {
        return Err(format!("{gate}: vectors exits {code}\n{stderr}"));
    }
    let file = dir.join("vectors.json");
    std::fs::write(&file, &text).unwrap();
    Ok(Vectors { file, text })
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

/// `f` on each of `items`, on as many threads as the machine has cores (at most eight); the
/// answers in the order of the items.
fn each_on_threads<T: Sync, R: Send>(items: &[T], f: impl Fn(usize, &T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let answers: Mutex<Vec<(usize, R)>> = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 8);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    if i >= items.len() {
                        break;
                    }
                    let r = f(i, &items[i]);
                    answers.lock().unwrap().push((i, r));
                }
            });
        }
    });
    let mut a = answers.into_inner().unwrap();
    a.sort_by_key(|(i, _)| *i);
    a.into_iter().map(|(_, r)| r).collect()
}

// ---------------------------------------------------------------------------------------------
// The generated text changed at one place.

/// One change of the generated Cedar: what it is, and the policies or the schema it makes.
struct Change {
    what: String,
    policies: Option<PolicySet>,
    schema: Option<Schema>,
}

fn with_kind(e: &Expr, kind: ExprKind) -> Expr {
    Expr { kind, line: e.line, col: e.col }
}

/// The operator put in another's place: the one that reads otherwise beside it.
fn other_ops(op: BinOp) -> &'static [BinOp] {
    match op {
        BinOp::Eq => &[BinOp::NotEq],
        BinOp::NotEq => &[BinOp::Eq],
        BinOp::Less => &[BinOp::LessEq],
        BinOp::LessEq => &[BinOp::Less],
        BinOp::Greater => &[BinOp::GreaterEq],
        BinOp::GreaterEq => &[BinOp::Greater],
        BinOp::And => &[BinOp::Or],
        BinOp::Or => &[BinOp::And],
        BinOp::In => &[BinOp::Eq],
        BinOp::Add | BinOp::Sub | BinOp::Mul => &[],
    }
}

/// Each change of an expression at one place, and what it is: a number moved by one, a string
/// given another letter, a role put in another's place, an operator put in another's place, a
/// `has` taken away from the `&&` it guards, a `!` taken away.
fn expr_changes(e: &Expr, roles: &[EntityUid]) -> Vec<(String, Expr)> {
    let mut out = Vec::new();
    match &e.kind {
        ExprKind::Long(n) => {
            for m in [n.checked_sub(1), n.checked_add(1)].into_iter().flatten() {
                out.push((format!("{n} made {m}"), with_kind(e, ExprKind::Long(m))));
            }
        }
        ExprKind::Str(s) => out.push((format!("\"{s}\" made \"{s}_\""), with_kind(e, ExprKind::Str(format!("{s}_"))))),
        ExprKind::Bool(b) => out.push((format!("{b} made {}", !b), with_kind(e, ExprKind::Bool(!b)))),
        ExprKind::Entity(u) => {
            for r in roles.iter().filter(|r| r.ty == u.ty && r.id != u.id) {
                out.push((format!("{}::\"{}\" made \"{}\"", u.ty, u.id, r.id), with_kind(e, ExprKind::Entity(r.clone()))));
            }
        }
        ExprKind::Binary { op, left, right } => {
            for o in other_ops(*op) {
                out.push((format!("`{}` made `{}`", op.as_str(), o.as_str()), with_kind(e, ExprKind::Binary { op: *o, left: left.clone(), right: right.clone() })));
            }
            if *op == BinOp::And {
                if matches!(left.kind, ExprKind::Has { .. }) {
                    out.push((format!("`{}` taken away", cedar::write_expr(left)), (**right).clone()));
                }
                if matches!(right.kind, ExprKind::Has { .. }) {
                    out.push((format!("`{}` taken away", cedar::write_expr(right)), (**left).clone()));
                }
            }
        }
        ExprKind::Not(inner) => out.push(("`!` taken away".to_string(), (**inner).clone())),
        _ => {}
    }
    // and the same at a place inside
    match &e.kind {
        ExprKind::Binary { op, left, right } => {
            for (w, l) in expr_changes(left, roles) {
                out.push((w, with_kind(e, ExprKind::Binary { op: *op, left: Box::new(l), right: right.clone() })));
            }
            for (w, r) in expr_changes(right, roles) {
                out.push((w, with_kind(e, ExprKind::Binary { op: *op, left: left.clone(), right: Box::new(r) })));
            }
        }
        ExprKind::Not(inner) => {
            for (w, x) in expr_changes(inner, roles) {
                out.push((w, with_kind(e, ExprKind::Not(Box::new(x)))));
            }
        }
        ExprKind::If { cond, then, els } => {
            for (w, x) in expr_changes(cond, roles) {
                out.push((w, with_kind(e, ExprKind::If { cond: Box::new(x), then: then.clone(), els: els.clone() })));
            }
            for (w, x) in expr_changes(then, roles) {
                out.push((w, with_kind(e, ExprKind::If { cond: cond.clone(), then: Box::new(x), els: els.clone() })));
            }
            for (w, x) in expr_changes(els, roles) {
                out.push((w, with_kind(e, ExprKind::If { cond: cond.clone(), then: then.clone(), els: Box::new(x) })));
            }
        }
        _ => {}
    }
    out
}

/// The entities a policy set names in its scopes and conditions whose type is `Role` (DESIGN
/// 5.1): each may be put in another's place.
fn roles_of(set: &PolicySet) -> Vec<EntityUid> {
    let mut out: Vec<EntityUid> = Vec::new();
    let mut add = |u: &EntityUid| {
        if u.ty.id == "Role" && !out.iter().any(|o| o.ty == u.ty && o.id == u.id) {
            out.push(EntityUid::new(u.ty.clone(), u.id.clone()));
        }
    };
    for p in &set.policies {
        for s in [&p.principal, &p.resource] {
            if let Scope::Eq(EntityOrSlot::Entity(u)) | Scope::In(EntityOrSlot::Entity(u)) | Scope::IsIn(_, EntityOrSlot::Entity(u)) = s {
                add(u);
            }
        }
        for c in &p.conditions {
            cedar::walk(&c.body, &mut |e| {
                if let ExprKind::Entity(u) = &e.kind {
                    add(u);
                }
            });
        }
    }
    out
}

/// The actions the policies name in their scopes.
fn actions_of(set: &PolicySet) -> Vec<EntityUid> {
    let mut out: Vec<EntityUid> = Vec::new();
    for p in &set.policies {
        let named: Vec<&EntityUid> = match &p.action {
            ActionScope::Eq(a) | ActionScope::In(a) => vec![a],
            ActionScope::InList(l) => l.iter().collect(),
            ActionScope::Any => vec![],
        };
        for a in named {
            if !out.iter().any(|o| o.ty == a.ty && o.id == a.id) {
                out.push(EntityUid::new(a.ty.clone(), a.id.clone()));
            }
        }
    }
    out
}

/// Each change of the policies at one place.
fn policy_changes(set: &PolicySet) -> Vec<Change> {
    let roles = roles_of(set);
    let actions = actions_of(set);
    let mut out = Vec::new();
    for (i, p) in set.policies.iter().enumerate() {
        let mut push = |what: String, q: Policy| {
            let mut s = set.clone();
            s.policies[i] = q;
            out.push(Change { what: format!("{}: {what}", p.id), policies: Some(s), schema: None });
        };
        let mut q = p.clone();
        q.effect = if p.effect == Effect::Forbid { Effect::Permit } else { Effect::Forbid };
        push(if p.effect == Effect::Forbid { "forbid made permit".into() } else { "permit made forbid".into() }, q);
        if let Scope::In(EntityOrSlot::Entity(u)) = &p.principal {
            let mut q = p.clone();
            q.principal = Scope::Eq(EntityOrSlot::Entity(u.clone()));
            push(format!("principal in {}::\"{}\" made ==", u.ty, u.id), q);
            for r in roles.iter().filter(|r| r.ty == u.ty && r.id != u.id) {
                let mut q = p.clone();
                q.principal = Scope::In(EntityOrSlot::Entity(r.clone()));
                push(format!("principal in \"{}\" made in \"{}\"", u.id, r.id), q);
            }
        }
        match &p.action {
            ActionScope::Eq(a) => {
                for b in actions.iter().filter(|b| b.id != a.id) {
                    let mut q = p.clone();
                    q.action = ActionScope::Eq(b.clone());
                    push(format!("action == \"{}\" made \"{}\"", a.id, b.id), q);
                }
            }
            ActionScope::InList(l) if l.len() > 1 => {
                for (j, a) in l.iter().enumerate() {
                    let mut q = p.clone();
                    let mut l2 = l.clone();
                    l2.remove(j);
                    q.action = ActionScope::InList(l2);
                    push(format!("\"{}\" taken out of the actions", a.id), q);
                }
            }
            _ => {}
        }
        for (j, c) in p.conditions.iter().enumerate() {
            let shown = format!("{} {{ {} }}", if c.kind == CondKind::When { "when" } else { "unless" }, cedar::write_expr(&c.body));
            let mut q = p.clone();
            q.conditions[j].kind = if c.kind == CondKind::When { CondKind::Unless } else { CondKind::When };
            push(format!("`{shown}` made {}", if c.kind == CondKind::When { "unless" } else { "when" }), q);
            let mut q = p.clone();
            q.conditions.remove(j);
            push(format!("`{shown}` taken away"), q);
            for (w, e) in expr_changes(&c.body, &roles) {
                let mut q = p.clone();
                q.conditions[j].body = e;
                push(format!("in `{shown}`, {w}"), q);
            }
        }
        let mut s = set.clone();
        s.policies.remove(i);
        out.push(Change { what: format!("{}: the policy taken away", p.id), policies: Some(s), schema: None });
    }
    out
}

/// The type put in an attribute's place: another of Cedar's, or a string for an entity.
fn other_type(t: &Type) -> Option<Type> {
    let named = |n: &str| Type::EntityOrCommon(Name::new(n));
    match t {
        Type::Long => Some(Type::String),
        Type::String => Some(Type::Long),
        Type::Bool => Some(Type::String),
        Type::Entity(_) => Some(Type::String),
        Type::EntityOrCommon(n) if n.path.is_empty() && n.id == "Long" => Some(named("String")),
        Type::EntityOrCommon(n) if n.path.is_empty() && (n.id == "String" || n.id == "Bool") => Some(named(if n.id == "String" { "Long" } else { "String" })),
        Type::EntityOrCommon(_) => Some(named("String")),
        _ => None,
    }
}

/// The entity types that have parents in some test of the vectors (`Shop::User` has the roles
/// it holds).
fn parented(cases: &[Case]) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for c in cases {
        for e in c.json.get("entities").and_then(Json::as_arr).unwrap_or_default() {
            if e.get("parents").and_then(Json::as_arr).is_some_and(|p| !p.is_empty())
                && let Some(t) = e.get("uid").and_then(|u| u.get("type")).and_then(Json::as_str)
            {
                out.insert(t.to_string());
            }
        }
    }
    out
}

/// Each change of the schema at one place: the parents of an entity type taken away (when an
/// entity of the type has parents in the tests: when none has, the schema reads the same for
/// them), an attribute made optional or required, given another type, or taken away, and a
/// principal or resource type taken out of an action's.
fn schema_changes(schema: &Schema, parented: &std::collections::BTreeSet<String>) -> Vec<Change> {
    let mut out = Vec::new();
    let mut push = |what: String, s: Schema| out.push(Change { what, policies: None, schema: Some(s) });
    for (n, ns) in schema.namespaces.iter().enumerate() {
        for (e, et) in ns.entity_types.iter().enumerate() {
            let EntityKind::Standard { member_of, shape, tags } = &et.kind else { continue };
            let qualified = match &ns.name {
                Some(n) => format!("{n}::{}", et.name),
                None => et.name.clone(),
            };
            if !member_of.is_empty() && parented.contains(&qualified) {
                let mut s = schema.clone();
                s.namespaces[n].entity_types[e].kind = EntityKind::Standard { member_of: vec![], shape: shape.clone(), tags: tags.clone() };
                push(format!("entity {}: its parents taken away", et.name), s);
            }
            let Type::Record(rec) = shape else { continue };
            for (a, attr) in rec.attrs.iter().enumerate() {
                let edit = |f: &dyn Fn(&mut Vec<cedar::Attr>)| {
                    let mut s = schema.clone();
                    if let EntityKind::Standard { shape: Type::Record(r), .. } = &mut s.namespaces[n].entity_types[e].kind {
                        f(&mut r.attrs);
                    }
                    s
                };
                push(format!("entity {}: {} made {}", et.name, attr.name, if attr.required { "optional" } else { "required" }), edit(&|v| v[a].required = !v[a].required));
                if let Some(t) = other_type(&attr.ty) {
                    push(format!("entity {}: {} given another type", et.name, attr.name), edit(&|v| v[a].ty = t.clone()));
                }
                push(format!("entity {}: {} taken away", et.name, attr.name), edit(&|v| {
                    v.remove(a);
                }));
            }
        }
        for (k, act) in ns.actions.iter().enumerate() {
            let Some(applies) = &act.applies_to else { continue };
            for (which, list) in [("principal", &applies.principal_types), ("resource", &applies.resource_types)] {
                if list.len() < 2 {
                    continue;
                }
                for (j, t) in list.iter().enumerate() {
                    let mut s = schema.clone();
                    let a = s.namespaces[n].actions[k].applies_to.as_mut().unwrap();
                    if which == "principal" {
                        a.principal_types.remove(j);
                    } else {
                        a.resource_types.remove(j);
                    }
                    push(format!("action {}: {t} taken out of its {which} types", act.name), s);
                }
            }
            let Type::Record(rec) = &applies.context else { continue };
            for (a, attr) in rec.attrs.iter().enumerate() {
                let edit = |f: &dyn Fn(&mut Vec<cedar::Attr>)| {
                    let mut s = schema.clone();
                    if let Some(cedar::AppliesTo { context: Type::Record(r), .. }) = &mut s.namespaces[n].actions[k].applies_to {
                        f(&mut r.attrs);
                    }
                    s
                };
                push(format!("action {}: context.{} made {}", act.name, attr.name, if attr.required { "optional" } else { "required" }), edit(&|v| v[a].required = !v[a].required));
                if let Some(t) = other_type(&attr.ty) {
                    push(format!("action {}: context.{} given another type", act.name, attr.name), edit(&|v| v[a].ty = t.clone()));
                }
                push(format!("action {}: context.{} taken away", act.name, attr.name), edit(&|v| {
                    v.remove(a);
                }));
            }
        }
    }
    out
}

/// Which check failed a change: the first of validate, run-tests, each policy alone, and the
/// tests that leave out a required attribute (in `../lacking.tests.json`).
fn caught_by(cli: &Cli, dir: &Path, set: &PolicySet, policies: &str, schema_text: &str, cases: &[Case], lacking: &[String]) -> Option<&'static str> {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("p.cedar"), policies).unwrap();
    std::fs::write(dir.join("s.cedarschema"), schema_text).unwrap();
    if cli.validate(dir, "p.cedar", "s.cedarschema", false).is_err() {
        return Some("validate");
    }
    if cli.run_tests(dir, "p.cedar", "s.cedarschema", "../vectors.json", cases.len(), false).is_err() {
        return Some("run-tests");
    }
    if each_alone(cli, dir, set, "s.cedarschema", cases).is_err() {
        return Some("each policy alone");
    }
    if cli.refuses(dir, "p.cedar", "s.cedarschema", "../lacking.tests.json", lacking, false).is_err() {
        return Some("a required attribute left out");
    }
    None
}

/// What gen and vectors wrote for a `.gate`, with what Cedar does not evaluate put aside (the
/// namespace, the start of the ids, every annotation but `@id`): two files with the same are
/// changed alike.
fn same_but_for_names(w: &Written, v: &Vectors) -> String {
    let mut set = cedar::parse_policies(&w.policies).unwrap();
    for p in &mut set.policies {
        p.annotations.retain(|a| a.key == "id");
    }
    let mut schema = cedar::parse_schema(&w.schema).unwrap();
    let ns = schema.namespaces.iter().find_map(|n| n.name.as_ref().map(Name::to_string)).unwrap_or_default();
    for n in &mut schema.namespaces {
        n.annotations.clear();
        n.name = n.name.as_ref().map(|_| Name::new("N"));
        let attrs = |t: &mut Type| {
            if let Type::Record(r) = t {
                r.attrs.iter_mut().for_each(|a| a.annotations.clear());
            }
        };
        for e in &mut n.entity_types {
            e.annotations.clear();
            if let EntityKind::Standard { shape, .. } = &mut e.kind {
                attrs(shape);
            }
        }
        for a in &mut n.actions {
            a.annotations.clear();
            if let Some(ap) = &mut a.applies_to {
                attrs(&mut ap.context);
            }
        }
    }
    let renamed = |s: String| s.replace(&format!("{ns}::"), "N::").replace(&format!("\"{}/", w.alias), "\"a/");
    format!("{}\n{}\n{}", renamed(cedar::policies_to_json(&set).compact()), renamed(cedar::schema_to_json(&schema).compact()), renamed(v.text.clone()))
}

/// What the changes of one `.gate`'s Cedar came to: how many each check failed first, the
/// changes only the run of each policy alone failed, and how many changes were not Cedar
/// ritsu-base writes (none is expected).
struct Failed {
    counts: Vec<(&'static str, usize)>,
    alone: Vec<String>,
    not_cedar: usize,
}

/// Every change of what was written for one `.gate`, each held to the checks; an error names the
/// changes no check failed.
fn changes_fail(cli: &Cli, w: &Written, v: &Vectors, scratch: &Path) -> Result<Failed, String> {
    let set = cedar::parse_policies(&w.policies).map_err(|e| format!("ritsu-base does not read the policies: {e:?}"))?;
    let schema = cedar::parse_schema(&w.schema).map_err(|e| format!("ritsu-base does not read the schema: {e:?}"))?;
    let schema_text = cedar::write_schema(&schema).map_err(|e| format!("ritsu-base does not write the schema: {e:?}"))?;
    let policies_text = cedar::write_policies(&set).map_err(|e| format!("ritsu-base does not write the policies: {e:?}"))?;
    let cases = cases(&v.text)?;
    std::fs::write(scratch.join("vectors.json"), &v.text).unwrap();
    let (_, lacking) = write_lacking(scratch, &lacking(&schema, &cases));
    // unchanged, written back: what every change is held against passes every check
    if let Some(check) = caught_by(cli, &scratch.join("unchanged"), &set, &policies_text, &schema_text, &cases, &lacking) {
        return Err(format!("the generated Cedar, read and written back unchanged, fails {check}"));
    }
    let mut all = policy_changes(&set);
    all.extend(schema_changes(&schema, &parented(&cases)));
    // each change as the text the CLI reads: one ritsu-base does not write is not Cedar, and is
    // left out
    let mut changes = Vec::new();
    let mut not_cedar = 0;
    for c in all {
        let p = match &c.policies {
            Some(p) => cedar::write_policies(p),
            None => Ok(policies_text.clone()),
        };
        let s = match &c.schema {
            Some(s) => cedar::write_schema(s),
            None => Ok(schema_text.clone()),
        };
        match (p, s) {
            (Ok(p), Ok(s)) => changes.push((c, p, s)),
            _ => not_cedar += 1,
        }
    }
    let found = each_on_threads(&changes, |i, (c, p, s)| caught_by(cli, &scratch.join(format!("c{i}")), c.policies.as_ref().unwrap_or(&set), p, s, &cases, &lacking));
    let missed: Vec<&str> = changes.iter().zip(&found).filter(|(_, f)| f.is_none()).map(|((c, _, _), _)| c.what.as_str()).collect();
    if !missed.is_empty() {
        return Err(format!("{} of {} changes fail no check:\n  {}", missed.len(), changes.len(), missed.join("\n  ")));
    }
    let mut counts: Vec<(&'static str, usize)> = Vec::new();
    for f in found.iter().flatten() {
        match counts.iter_mut().find(|(k, _)| k == f) {
            Some((_, n)) => *n += 1,
            None => counts.push((f, 1)),
        }
    }
    counts.sort();
    let alone = changes.iter().zip(&found).filter(|(_, f)| **f == Some("each policy alone")).map(|((c, _, _), _)| c.what.clone()).collect();
    Ok(Failed { counts, alone, not_cedar })
}

// ---------------------------------------------------------------------------------------------
// The tests.

/// What the CLI says of one `.gate`: None when its check finds an error (gen then writes
/// nothing), else the line the test prints. What gen writes in English and in Japanese is
/// validated, laid out and translated alike; the answers do not depend on the language.
fn held_line(cli: &Cli, gate: &str) -> Result<Option<String>, String> {
    let checked = sekisho::check::check_file(gate, &common::joined(), &sekisho::check::Options::default()).map_err(|e| format!("{gate}: {e}"))?;
    match (checked.exit() == 0, generate(gate, "en")?) {
        (false, None) => Ok(None),
        (false, Some(_)) => Err(format!("{gate}: its check finds an error, and gen writes Cedar for it")),
        (true, None) => Err(format!("{gate}: its check passes, and gen finds an error")),
        (true, Some((t, w))) => {
            hold_text(cli, &w).map_err(|e| format!("{gate}: {e}"))?;
            let (_tj, ja) = generate(gate, "ja")?.ok_or_else(|| format!("{gate}: gen --lang ja finds an error"))?;
            hold_text(cli, &ja).map_err(|e| format!("{gate}, gen --lang ja: {e}"))?;
            let v = vectors(gate, t.path())?;
            let h = hold_answers(cli, &w, &v).map_err(|e| format!("{gate}: {e}"))?;
            // the vectors are every combination the check walks, with its answers
            let actions = &checked.walked.as_ref().ok_or_else(|| format!("{gate}: the check walked nothing"))?.report.actions;
            let walked: u128 = actions.iter().map(|a| a.combinations).sum();
            let allowed: u128 = actions.iter().map(|a| a.allowed).sum();
            if (h.combinations as u128, h.allowed as u128) != (walked, allowed) {
                return Err(format!("{gate}: the vectors hold {} combinations, {} allowed; the check walks {walked}, {allowed} allowed", h.combinations, h.allowed));
            }
            if h.tests == 0 {
                return Ok(Some(format!("{gate}: no action, so no combination; what gen writes is validated, formatted and translated")));
            }
            let n = |k: usize, one: &str, many: &str| format!("{k} {}", if k == 1 { one } else { many });
            let refused = match h.lacking {
                0 => "no test holds an attribute the schema requires".to_string(),
                1 => "the schema refuses the test that leaves out an attribute it requires".to_string(),
                k => format!("the schema refuses the {k} tests that each leave out an attribute it requires"),
            };
            Ok(Some(format!(
                "{gate}: {} of {} ({} allowed) {} run-tests, and so does each of the {} alone; {refused}",
                n(h.tests, "test", "tests"),
                n(h.combinations, "combination", "combinations"),
                h.allowed,
                if h.tests == 1 { "passes" } else { "pass" },
                n(h.policies, "policy", "policies")
            )))
        }
    }
}

/// Every `.gate` of the examples and the tests that passes its check: what gen writes for it,
/// held to the CLI. gen writes nothing for one whose check finds an error.
#[test]
fn every_gate_that_passes_its_check_is_held_to_the_cli() {
    let Some(cli) = Cli::find() else { return };
    let gates = gates();
    let said = each_on_threads(&gates, |_, g| held_line(&cli, g));
    let (mut held, mut wrong) = (Vec::new(), Vec::new());
    for s in said {
        match s {
            Ok(Some(line)) => held.push(line),
            Ok(None) => {}
            Err(e) => wrong.push(e),
        }
    }
    for line in &held {
        println!("{line}");
    }
    println!("{} of the {} .gate files of the examples and the tests pass their check; for each, the CLI validates what gen writes, finds it formatted, translates it to its JSON, and answers every combination as sekisho does", held.len(), gates.len());
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
    for g in ["examples/refunds/refunds.gate", "examples/refunds/refunds.ja.gate"] {
        assert!(held.iter().any(|l| l.starts_with(&format!("{g}: "))), "{g} is generated and held");
    }
}

/// What gen writes for every `.gate` that passes its check, changed at one place at a time: every
/// change fails one of the checks. The changes only the run of each policy alone fails are the
/// ones `run-tests` alone would pass.
#[test]
fn every_change_of_the_generated_cedar_fails_a_check() {
    let Some(cli) = Cli::find() else { return };
    let mut wrong = Vec::new();
    let (mut gates_changed, mut changes) = (0, 0);
    let gates = gates();
    let generated = each_on_threads(&gates, |_, g| generate(g, "en"));
    // a file whose Cedar and vectors are another's but for the names, as the Japanese version of a
    // file is, is changed once
    let mut seen: Vec<(String, String)> = Vec::new();
    for (gate, g) in gates.iter().zip(generated) {
        let Some((t, w)) = g.unwrap_or_else(|e| panic!("{e}")) else { continue };
        let v = vectors(gate, t.path()).unwrap_or_else(|e| panic!("{e}"));
        if cases(&v.text).unwrap_or_else(|e| panic!("{gate}: {e}")).is_empty() {
            // a file of declarations that other files read: no combination answers for its Cedar
            println!("{gate}: no action, so no combination to hold a change to");
            continue;
        }
        let key = same_but_for_names(&w, &v);
        if let Some((_, first)) = seen.iter().find(|(k, _)| *k == key) {
            println!("{gate}: the same Cedar and vectors as {first}, but for the names");
            continue;
        }
        seen.push((key, gate.clone()));
        let scratch = t.path().join("changes");
        std::fs::create_dir_all(&scratch).unwrap();
        match changes_fail(&cli, &w, &v, &scratch) {
            Ok(f) => {
                let n: usize = f.counts.iter().map(|(_, n)| n).sum();
                let counts: Vec<String> = f.counts.iter().map(|(k, n)| format!("{k} {n}")).collect();
                let left_out = if f.not_cedar > 0 { format!("; {} more are not Cedar, and are left out", f.not_cedar) } else { String::new() };
                println!("{gate}: {n} changes, each failing a check (the first to fail: {}){left_out}", counts.join(", "));
                for a in &f.alone {
                    println!("  only each policy alone fails: {a}");
                }
                gates_changed += 1;
                changes += n;
            }
            Err(e) => wrong.push(format!("{gate}: {e}")),
        }
    }
    println!("{changes} changes of the Cedar of {gates_changed} .gate files, every one failing a check");
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
    assert!(gates_changed >= 2, "the two versions of the example are changed");
}
