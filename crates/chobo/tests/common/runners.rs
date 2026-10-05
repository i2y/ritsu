//! The clients of the test books, the runners that drive them (tools/runner), and how what they
//! answer and send is compared with the reference interpreter and `chobo run --show` (PLAN C5).
#![allow(dead_code)]

use super::servers::{Pg, Postgres};
use super::*;
use chobo::interp::Op;
use chobo::model::{Book, Expiry};
use chobo::scenario::{self, Scenario, Step};
use chobo::target::{self, Target};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The expiry of every hold in the test copy of a book: `pass` is waited for, for real.
pub const EXPIRY: u64 = 3;

/// One book as the backend tests take it.
pub struct Case {
    pub stem: String,
    /// the book as it is written
    pub book: Book,
    /// the same book with every expiry at EXPIRY seconds: what the clients and the SQL are built from
    pub copy: Book,
    pub scenarios: Vec<Scenario>,
    /// what the reference interpreter answers for each scenario (PLAN 0.3)
    pub reference: Vec<Value>,
    /// the copy's file as the head of what is built from it names it
    pub origin: chobo::target::Origin,
}

/// The book with every `pending expires after …` line at EXPIRY seconds.
pub fn test_copy(src: &str) -> String {
    src.lines()
        .map(|l| {
            let t = l.trim_start();
            if t.starts_with("pending expires after ") {
                format!("{}pending expires after {EXPIRY} seconds", &l[..l.len() - t.len()])
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

pub fn case(path: &Path) -> Case {
    let src = std::fs::read_to_string(path).unwrap();
    let (book, d) = chobo::model::load(&src);
    assert!(d.is_empty(), "{}", path.display());
    let book = book.unwrap();
    let copy_src = test_copy(&src);
    let (copy, d) = chobo::model::load(&copy_src);
    assert!(d.is_empty(), "{}: the test copy does not load", path.display());
    let copy = copy.unwrap();
    for (a, b) in book.transfers.iter().zip(&copy.transfers) {
        if matches!(a.pending, Some(Expiry::After(_))) {
            assert_eq!(b.pending, Some(Expiry::After(EXPIRY)), "{}: the test copy did not find where {}'s expiry is written", path.display(), a.name);
        }
    }
    let mut scenarios = chobo::scenarios::generate(&book);
    // hand-written scenarios beside the book, for what the generated ones do not reach
    if let Ok(text) = std::fs::read_to_string(path.with_file_name(format!("{}.more.json", stem(path)))) {
        let more: Vec<Value> = serde_json::from_str(&text).unwrap();
        scenarios.extend(more.iter().map(|v| scenario::from_json(&book, v).unwrap_or_else(|e| panic!("{}: {e}", path.display()))));
    }
    let mut reference = Vec::new();
    for s in &scenarios {
        let r = scenario::run_json(&book, s).unwrap();
        // the copy answers the same: every `pass` is past every expiry, at 3 seconds as at 30 minutes
        let sc = scenario::from_json(&copy, &scenario::to_json(&book, s)).unwrap();
        assert_eq!(scenario::run_json(&copy, &sc).unwrap(), r, "{}: {}: the test copy answers otherwise", path.display(), s.name);
        reference.push(r);
    }
    let origin = chobo::target::Origin::named(&path.display().to_string(), copy_src.as_bytes());
    Case { stem: stem(path), book, copy, scenarios, reference, origin }
}

/// The books the backends run: those of tests/books, and the examples when there are some.
pub fn cases() -> Vec<Case> {
    let mut paths = books_in("tests/books");
    let examples = root().join("examples");
    if examples.is_dir() {
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(&examples).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        dirs.sort();
        for d in dirs {
            let mut more: Vec<PathBuf> = std::fs::read_dir(&d).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "book")).collect();
            more.sort();
            paths.extend(more);
        }
    }
    paths.iter().map(|p| case(p)).collect()
}

// ── the tools ─────────────────────────────────────────────────────────────

fn runs(cmd: &str, arg: &str) -> bool {
    ritsu_testkit::tools::runs(cmd, &[arg])
}

pub fn runner_dir() -> PathBuf {
    root().join("tools/runner")
}

pub fn node() -> Result<(), String> {
    let nm = runner_dir().join("node_modules");
    if !runs("node", "--version") || !nm.join("pg").is_dir() || !nm.join("tigerbeetle-node").is_dir() || !nm.join("typescript").is_dir() {
        return Err("node or tools/runner/node_modules is missing; run `npm ci --prefix tools/runner`".into());
    }
    Ok(())
}

pub fn python() -> Result<PathBuf, String> {
    let py = runner_dir().join(".venv/bin/python");
    if !py.is_file() {
        return Err("tools/runner/.venv is missing; make it with `uv venv --python 3.13 tools/runner/.venv` and `uv pip install --python tools/runner/.venv/bin/python -r tools/runner/requirements.txt`".into());
    }
    Ok(py)
}

pub fn go() -> Result<(), String> {
    if !runs("go", "version") || !runner_dir().join("go/go.mod").is_file() {
        return Err("go or tools/runner/go is missing".into());
    }
    Ok(())
}

/// `go`, run in `dir`, as every Go command here runs: outside any workspace, and with -trimpath
/// (Go's build cache keys a package by its directory otherwise, and grows by the same packages
/// at every run).
pub fn go_in(dir: &Path) -> Command {
    let mut c = Command::new("go");
    c.current_dir(dir).env("GOWORK", "off").env("GOFLAGS", "-trimpath");
    c
}

// ── the clients ───────────────────────────────────────────────────────────

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Build `target` of the test copy of each book into `dir`; the path of each book's client.
pub fn build_all(cases: &[Case], target: Target, dir: &Path) -> Vec<PathBuf> {
    cases
        .iter()
        .map(|c| {
            let files = target::build(&c.copy, &c.stem, target, &c.origin).unwrap_or_else(|d| panic!("{}: {target:?}: {}", c.stem, d[0].message.en));
            let mut first = None;
            for (rel, text) in &files {
                let p = dir.join(rel);
                write(&p, text);
                first.get_or_insert(p);
            }
            first.unwrap()
        })
        .collect()
}

/// A directory where the TypeScript clients find tools/runner/node_modules, as a project of their own would.
pub fn ts_dir(work: &Path) -> PathBuf {
    let d = work.join("ts");
    std::fs::create_dir_all(&d).unwrap();
    let link = d.join("node_modules");
    if !link.exists() {
        std::os::unix::fs::symlink(runner_dir().join("node_modules"), &link).unwrap();
    }
    std::fs::write(d.join("package.json"), "{\"type\": \"module\"}\n").unwrap();
    d
}

/// The Go runner: one binary of tools/runner/go/harness and the Go clients of every book, for both
/// databases (building a binary a book would cost far more time). Each package is known by a key.
pub struct GoRunner {
    pub bin: PathBuf,
    pub keys: BTreeMap<(String, &'static str), String>,
}

fn go_adapter(alias: &str, book: &Book, pg: bool) -> String {
    use chobo::client::go::{account_fields, balance_methods, fields, go_name, members};
    let ty = format!("a_{alias}");
    let mut o = format!("type {ty} struct{{ b *{alias}.Book }}\n\n");
    o.push_str(&format!("func (a {ty}) Call(ctx context.Context, kind, op string, args, amounts map[string]any) (string, string, error) {{\n\tswitch kind + \".\" + op {{\n"));
    let arg = |t: &chobo::model::TransferKind, i: usize, f: &str, m: &str| {
        let p = &t.params[i];
        let read = if matches!(p.ty, chobo::model::Ty::Amount(_)) { "Amt" } else { "Str" };
        format!("{f}: harness.{read}({m}, {:?})", p.name)
    };
    for (t, member) in book.transfers.iter().zip(members(book)) {
        let g = go_name(&t.name);
        let all: Vec<usize> = (0..t.params.len()).collect();
        let lit = |which: &[usize], m: &str| fields(t, which).iter().zip(which).map(|(f, i)| arg(t, *i, f, m)).collect::<Vec<_>>().join(", ");
        let keys = chobo::postgres::key_params(t);
        if t.is_pending() {
            o.push_str(&format!("\tcase {:?}:\n\t\tr, err := a.b.{member}.Hold(ctx, {alias}.{g}Args{{{}}})\n\t\treturn r.Outcome, r.Reason, err\n", format!("{}.hold", t.name), lit(&all, "args")));
            let ap = t.amount_params();
            if ap.is_empty() {
                o.push_str(&format!("\tcase {:?}:\n\t\tr, err := a.b.{member}.Post(ctx, {alias}.{g}Key{{{}}})\n\t\treturn r.Outcome, r.Reason, err\n", format!("{}.post", t.name), lit(&keys, "args")));
            } else {
                o.push_str(&format!(
                    "\tcase {:?}:\n\t\tvar am *{alias}.{g}Amounts\n\t\tif amounts != nil {{\n\t\t\tam = &{alias}.{g}Amounts{{{}}}\n\t\t}}\n\t\tr, err := a.b.{member}.Post(ctx, {alias}.{g}Key{{{}}}, am)\n\t\treturn r.Outcome, r.Reason, err\n",
                    format!("{}.post", t.name),
                    lit(&ap, "amounts"),
                    lit(&keys, "args")
                ));
            }
            o.push_str(&format!("\tcase {:?}:\n\t\tr, err := a.b.{member}.Void(ctx, {alias}.{g}Key{{{}}})\n\t\treturn r.Outcome, r.Reason, err\n", format!("{}.void", t.name), lit(&keys, "args")));
        } else {
            o.push_str(&format!("\tcase {:?}:\n\t\tr, err := a.b.{member}.Do(ctx, {alias}.{g}Args{{{}}})\n\t\treturn r.Outcome, r.Reason, err\n", format!("{}.do", t.name), lit(&all, "args")));
        }
    }
    o.push_str("\t}\n\treturn \"\", \"\", fmt.Errorf(\"no %s.%s\", kind, op)\n}\n\n");
    o.push_str(&format!("func (a {ty}) Balance(ctx context.Context, account string, args map[string]any) (int64, int64, int64, error) {{\n\tswitch account {{\n"));
    for (acc, m) in book.accounts.iter().zip(balance_methods(book)) {
        let call = if acc.params.is_empty() {
            format!("a.b.Balance.{m}(ctx)")
        } else {
            let fs: Vec<String> = account_fields(acc).iter().zip(&acc.params).map(|(f, p)| format!("{f}: harness.Str(args, {p:?})")).collect();
            format!("a.b.Balance.{m}(ctx, {alias}.{}Account{{{}}})", go_name(&acc.name), fs.join(", "))
        };
        o.push_str(&format!("\tcase {:?}:\n\t\tb, err := {call}\n\t\treturn b.Posted, b.HeldIn, b.HeldOut, err\n", acc.name));
    }
    o.push_str("\t}\n\treturn 0, 0, 0, fmt.Errorf(\"no account %s\", account)\n}\n\n");
    o.push_str(&format!("func (a {ty}) Status(ctx context.Context, kind string, args map[string]any) (string, error) {{\n\tswitch kind {{\n"));
    for (t, member) in book.transfers.iter().zip(members(book)) {
        if !t.is_pending() {
            continue;
        }
        let keys = chobo::postgres::key_params(t);
        let lit = fields(t, &keys).iter().zip(&keys).map(|(f, i)| arg(t, *i, f, "args")).collect::<Vec<_>>().join(", ");
        o.push_str(&format!("\tcase {:?}:\n\t\ts, err := a.b.{member}.Status(ctx, {alias}.{}Key{{{lit}}})\n\t\treturn string(s), err\n", t.name, go_name(&t.name)));
    }
    o.push_str("\t}\n\treturn \"\", fmt.Errorf(\"no %s\", kind)\n}\n\n");
    if pg {
        o.push_str(&format!("func (a {ty}) Expire(ctx context.Context) error {{\n\t_, err := a.b.Expire(ctx)\n\treturn err\n}}\n\n"));
    } else {
        o.push_str(&format!("func (a {ty}) Expire(ctx context.Context) error {{ return nil }}\n\n"));
    }
    o
}

/// Copy a directory's files (not its subdirectories).
fn copy_files(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        if e.path().is_file() {
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

pub fn build_go(cases: &[Case], work: &Path) -> Result<GoRunner, String> {
    let module = work.join("go");
    copy_files(&runner_dir().join("go"), &module);
    copy_files(&runner_dir().join("go/harness"), &module.join("harness"));
    let mut imports = vec!["\"context\"".to_string(), "\"fmt\"".to_string(), String::new(), "\"chobo-runner/harness\"".to_string()];
    let mut adapters = String::new();
    let mut entries = String::new();
    let mut keys = BTreeMap::new();
    for (i, c) in cases.iter().enumerate() {
        for (backend, target, pg) in [("postgres", Target::PostgresGo, true), ("tigerbeetle", Target::TigerBeetleGo, false)] {
            let key = format!("{}{i}", if pg { "pg" } else { "tb" });
            let files = target::build(&c.copy, &c.stem, target, &c.origin).map_err(|d| d[0].message.en.clone())?;
            for (rel, text) in &files {
                let name = Path::new(rel).file_name().unwrap();
                write(&module.join("books").join(&key).join(name), text);
            }
            imports.push(format!("{key} \"chobo-runner/books/{key}\""));
            adapters.push_str(&go_adapter(&key, &c.copy, pg));
            let make = if pg {
                format!("Postgres: func(q harness.Querier, tenant string) harness.Book {{ return a_{key}{{{key}.Postgres(q, tenant)}} }}")
            } else {
                format!("TigerBeetle: func(c harness.TBClient, tenant string) harness.Book {{ return a_{key}{{{key}.TigerBeetle(c, tenant)}} }}")
            };
            entries.push_str(&format!("\t\t{key:?}: {{{make}}},\n"));
            keys.insert((c.stem.clone(), backend), key);
        }
    }
    let main = format!(
        "package main\n\nimport (\n{}\n)\n\n{adapters}func main() {{\n\tharness.Main(map[string]harness.Entry{{\n{entries}\t}})\n}}\n",
        imports.iter().map(|i| if i.is_empty() { String::new() } else { format!("\t{i}") }).collect::<Vec<_>>().join("\n")
    );
    write(&module.join("main.go"), &main);
    let bin = work.join("go-runner");
    let started = Instant::now();
    let out = go_in(&module).args(["build", "-mod=readonly", "-o"]).arg(&bin).arg(".").output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("the Go runner did not build:\n{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)));
    }
    eprintln!("built the Go runner of {} package(s) in {:.0} s", keys.len(), started.elapsed().as_secs_f64());
    Ok(GoRunner { bin, keys })
}

// ── the runners ───────────────────────────────────────────────────────────

/// What a runner is given for one book: its client, and each scenario with its tenant, the
/// accounts to read at the end, and the holds to ask about.
pub fn book_input(c: &Case, client: &str, combo: &str) -> Value {
    book_input_of(c, client, combo, None)
}

/// The same, for the scenarios `only` names (every one when None), in that order.
pub fn book_input_of(c: &Case, client: &str, combo: &str, only: Option<&[usize]>) -> Value {
    let mut keys = serde_json::Map::new();
    for t in &c.book.transfers {
        keys.insert(t.name.clone(), json!(t.key.iter().map(|i| t.params[*i].name.clone()).collect::<Vec<_>>()));
    }
    let all: Vec<usize> = (0..c.scenarios.len()).collect();
    let scenarios: Vec<Value> = only
        .unwrap_or(&all)
        .iter()
        .map(|&i| {
            let s = &c.scenarios[i];
            let accounts: Vec<Value> = scenario::named_accounts(&c.book, s)
                .iter()
                .map(|id| {
                    let a = &c.book.accounts[id.kind];
                    let params: serde_json::Map<String, Value> = a.params.iter().zip(&id.args).map(|(p, v)| (p.clone(), json!(v))).collect();
                    json!({"account": a.name, "args": id.args, "params": params})
                })
                .collect();
            let mut holds: Vec<(usize, Vec<String>, Value)> = Vec::new();
            let mut add = |call: &chobo::interp::Call| {
                if call.op != Op::Hold {
                    return;
                }
                let t = &c.book.transfers[call.kind];
                let key = call.key(t);
                if holds.iter().any(|(k, kk, _)| *k == call.kind && *kk == key) {
                    return;
                }
                let args: serde_json::Map<String, Value> = t.key.iter().map(|i| (t.params[*i].name.clone(), json!(call.args[*i].as_ref().map(|v| v.text())))).collect();
                holds.push((call.kind, key.clone(), json!({"kind": t.name, "key": key, "args": args})));
            };
            for st in &s.steps {
                match st {
                    Step::Call(call) => add(call),
                    Step::Together(cs) => cs.iter().flatten().for_each(&mut add),
                    Step::Pass(..) => {}
                }
            }
            holds.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
            let steps = scenario::to_json(&c.book, s)["steps"].clone();
            json!({"tenant": tenant(combo, c, i), "steps": steps, "accounts": accounts, "holds": holds.into_iter().map(|h| h.2).collect::<Vec<_>>()})
        })
        .collect();
    json!({"name": c.stem, "client": client, "expiry": EXPIRY, "keys": keys, "scenarios": scenarios})
}

pub fn tenant(combo: &str, c: &Case, i: usize) -> String {
    format!("{combo}/{}/{}", c.stem, i + 1)
}

/// Run a runner on its input; what it printed, or why it failed.
pub fn run_runner(mut cmd: Command, input: &Value, work: &Path, combo: &str) -> Result<Value, String> {
    let file = work.join(format!("{}.json", combo.replace('/', "-")));
    std::fs::write(&file, serde_json::to_string(input).unwrap()).unwrap();
    let out = cmd.arg(&file).output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("{combo}: the runner failed:\n{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("{combo}: the runner's output is not JSON ({e}):\n{}", String::from_utf8_lossy(&out.stderr)))
}

// ── the SQL as it is, through psql ───────────────────────────────────────

/// One psql session, fed a statement at a time.
struct Psql {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    stderr: Arc<Mutex<String>>,
}

const END: &str = "__chobo_end__";

static EXPIRING: Mutex<()> = Mutex::new(());

impl Psql {
    fn open(pg: &Postgres) -> Psql {
        let mut child = pg.db().args(["-A", "-t", "-F", "|", "-v", "ON_ERROR_STOP=0"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut err = BufReader::new(child.stderr.take().unwrap());
        let stderr = Arc::new(Mutex::new(String::new()));
        let sink = stderr.clone();
        std::thread::spawn(move || {
            let mut line = String::new();
            while err.read_line(&mut line).is_ok_and(|n| n > 0) {
                sink.lock().unwrap().push_str(&line);
                line.clear();
            }
        });
        Psql { child, stdin, stdout, stderr }
    }

    /// The rows a statement prints.
    fn rows(&mut self, sql: &str) -> Result<Vec<String>, String> {
        writeln!(self.stdin, "{sql};\n\\echo {END}").map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())?;
        let mut rows = Vec::new();
        loop {
            let mut line = String::new();
            if self.stdout.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                return Err(format!("psql ended: {}", self.stderr.lock().unwrap()));
            }
            let line = line.trim_end_matches('\n').to_string();
            if line == END {
                break;
            }
            rows.push(line);
        }
        Ok(rows)
    }

    fn one(&mut self, sql: &str) -> Result<String, String> {
        let rows = self.rows(sql)?;
        match rows.as_slice() {
            [r] => Ok(r.clone()),
            _ => Err(format!("{sql}: {rows:?}\n{}", self.stderr.lock().unwrap())),
        }
    }
}

impl Drop for Psql {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "\\q");
        let _ = self.child.wait();
    }
}

/// A value as SQL writes it.
fn sql_value(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::String(s) => chobo::postgres::lit(s),
        other => other.to_string(),
    }
}

/// The SQL with its parameters in place.
fn inline(sql: &str, params: &[Value], amounts: &[bool]) -> String {
    let mut s = sql.to_string();
    for (i, p) in params.iter().enumerate().rev() {
        let v = if amounts.get(i).copied().unwrap_or(false) { p.as_str().map(|x| x.to_string()).unwrap_or_else(|| "null".into()) } else { sql_value(p) };
        s = s.replace(&format!("${}", i + 1), &v);
    }
    s
}

fn sql_scenario(pg: &Postgres, c: &Case, combo: &str, i: usize) -> Result<Value, String> {
    let book = &c.copy;
    let s = scenario::from_json(book, &scenario::to_json(&c.book, &c.scenarios[i])).unwrap();
    let tenant = tenant(combo, c, i);
    let schema = chobo::postgres::ident(&book.name);
    let mut main = Psql::open(pg);
    let mut held: Vec<Instant> = Vec::new();
    // when the first hold since the last `pass` was asked for, and whether a step or a read came
    // after its expiry (tools/runner/runner.ts says why)
    let mut held_since: Option<Instant> = None;
    let mut late = false;
    let outlived = |since: Option<Instant>| since.is_some_and(|t| t.elapsed() >= Duration::from_secs(EXPIRY));
    let call = |p: &mut Psql, call: &chobo::interp::Call| -> Result<(Value, bool), String> {
        let (sql, params) = chobo::postgres::call(book, &tenant, call);
        // which parameters are amounts: the tenant's is not; then the call's, as postgres::call lays them out
        let t = &book.transfers[call.kind];
        let mut amounts = vec![false];
        match call.op {
            Op::Do | Op::Hold => amounts.extend(t.params.iter().map(|p| matches!(p.ty, chobo::model::Ty::Amount(_)))),
            Op::Post | Op::Void => {
                amounts.extend(chobo::postgres::key_params(t).iter().map(|_| false));
                if call.op == Op::Post {
                    amounts.extend(t.amount_params().iter().map(|_| true));
                }
            }
        }
        let row = p.one(&inline(&sql, &params, &amounts))?;
        let (result, reason) = row.split_once('|').ok_or_else(|| format!("{sql}: {row}"))?;
        let mut v = json!({"op": call.op.name(), "kind": t.name, "result": result});
        if result == "refused" {
            v["reason"] = json!(reason);
        }
        Ok((v, result == "done" && call.op == Op::Hold))
    };
    let mut steps = Vec::new();
    for st in &s.steps {
        match st {
            Step::Call(cl) => {
                let asked = Instant::now();
                let (v, h) = call(&mut main, cl)?;
                if h {
                    held.push(Instant::now());
                    held_since.get_or_insert(asked);
                }
                steps.push(v);
            }
            Step::Pass(..) => {
                late |= outlived(held_since);
                let until = held.iter().max().copied().unwrap_or_else(Instant::now) + Duration::from_secs(EXPIRY) + Duration::from_millis(300);
                std::thread::sleep(until.saturating_duration_since(Instant::now()));
                // one expire() at a time, as one job would call it: an expire() skips the holds
                // another is giving back, and returns before that one commits
                let _one = EXPIRING.lock().unwrap();
                main.one(&format!("select {schema}.expire()"))?;
                held_since = None;
                steps.push(json!({"op": "pass"}));
            }
            Step::Together(cs) => {
                let asked = Instant::now();
                let outs: Vec<Result<Vec<(Value, bool)>, String>> = std::thread::scope(|sc| {
                    let handles: Vec<_> = cs
                        .iter()
                        .map(|ops| {
                            sc.spawn(|| {
                                let mut p = Psql::open(pg);
                                p.one("select 1")?;
                                ops.iter().map(|cl| call(&mut p, cl)).collect::<Result<Vec<_>, String>>()
                            })
                        })
                        .collect();
                    handles.into_iter().map(|h| h.join().unwrap()).collect()
                });
                let mut callers = Vec::new();
                for o in outs {
                    let o = o?;
                    for (_, h) in &o {
                        if *h {
                            held.push(Instant::now());
                            held_since.get_or_insert(asked);
                        }
                    }
                    callers.push(o.into_iter().map(|(v, _)| v).collect::<Vec<_>>());
                }
                steps.push(json!({"op": "together", "callers": callers}));
            }
        }
    }
    let mut accounts = Vec::new();
    for id in scenario::named_accounts(book, &s) {
        let a = &book.accounts[id.kind];
        let mut params = vec![json!(tenant)];
        params.extend(id.args.iter().map(|x| json!(x)));
        let sql = inline(&chobo::postgres::balance_sql(book, a), &params, &[]);
        let row = main.one(&sql)?;
        let n: Vec<i64> = row.split('|').map(|x| x.parse().unwrap_or(i64::MIN)).collect();
        accounts.push(json!({"account": a.name, "args": id.args, "posted": n[0], "held_in": n[1], "held_out": n[2]}));
    }
    let mut holds = Vec::new();
    let mut seen: Vec<(usize, Vec<String>)> = Vec::new();
    let mut calls: Vec<&chobo::interp::Call> = Vec::new();
    for st in &s.steps {
        match st {
            Step::Call(cl) => calls.push(cl),
            Step::Together(cs) => calls.extend(cs.iter().flatten()),
            Step::Pass(..) => {}
        }
    }
    for cl in calls {
        if cl.op == Op::Hold {
            let key = cl.key(&book.transfers[cl.kind]);
            if !seen.contains(&(cl.kind, key.clone())) {
                seen.push((cl.kind, key));
            }
        }
    }
    seen.sort();
    for (k, key) in seen {
        let t = &book.transfers[k];
        let mut params = vec![json!(tenant)];
        for i in chobo::postgres::key_params(t) {
            params.push(json!(t.key.iter().position(|x| *x == i).map(|j| key[j].clone())));
        }
        let state = main.one(&inline(&chobo::postgres::status_sql(book, t), &params, &[]))?;
        if !state.is_empty() {
            holds.push(json!({"kind": t.name, "key": key, "state": state}));
        }
    }
    late |= outlived(held_since);
    Ok(json!({"tenant": tenant, "result": {"steps": steps, "accounts": accounts, "holds": holds}, "sent": [], "late": late, "error": null}))
}

/// The scenarios of every book on the SQL itself, through psql: what the functions answer, with
/// no client in between.
pub fn run_sql(pg: &Postgres, cases: &[Case], combo: &str) -> Value {
    run_sql_of(pg, cases, combo, None)
}

/// The same, for the scenarios `only` names of each book (every one when None), in that order.
pub fn run_sql_of(pg: &Postgres, cases: &[Case], combo: &str, only: Option<&[Vec<usize>]>) -> Value {
    let which = |ci: usize| -> Vec<usize> { only.map(|o| o[ci].clone()).unwrap_or_else(|| (0..cases[ci].scenarios.len()).collect()) };
    let jobs: Vec<(usize, usize)> = (0..cases.len()).flat_map(|ci| which(ci).into_iter().map(move |i| (ci, i))).collect();
    let results: Mutex<BTreeMap<(usize, usize), Value>> = Mutex::new(BTreeMap::new());
    let next = Mutex::new(0usize);
    std::thread::scope(|sc| {
        for _ in 0..48 {
            sc.spawn(|| {
                loop {
                    let j = {
                        let mut n = next.lock().unwrap();
                        let j = *n;
                        *n += 1;
                        j
                    };
                    let Some(&(ci, i)) = jobs.get(j) else { break };
                    let c = &cases[ci];
                    let v = sql_scenario(pg, c, combo, i).unwrap_or_else(|e| json!({"tenant": tenant(combo, c, i), "result": null, "sent": [], "error": e}));
                    results.lock().unwrap().insert((ci, i), v);
                }
            });
        }
    });
    let results = results.into_inner().unwrap();
    let books: Vec<Value> = cases
        .iter()
        .enumerate()
        .map(|(ci, c)| json!({"name": c.stem, "scenarios": which(ci).into_iter().map(|i| results[&(ci, i)].clone()).collect::<Vec<_>>()}))
        .collect();
    json!({"books": books})
}

// ── the comparison ────────────────────────────────────────────────────────

/// Does the runner's result agree with the reference: the same, or, with `together`, one of
/// the ways it can come out.
pub fn agrees(reference: &Value, got: &Value) -> bool {
    match reference.get("outcomes").and_then(|o| o.as_array()) {
        Some(outcomes) => outcomes.contains(got),
        None => reference == got,
    }
}

/// The fields of what is sent, without the notes `chobo run --show` adds for a person.
fn wire(v: &Value) -> Value {
    let mut v = v.clone();
    if let Some(o) = v.as_object_mut() {
        for k in ["role", "of", "move"] {
            o.remove(k);
        }
    }
    v
}

/// Is `got` the same transfer or account as `want`: every field, an ID of "random" any ID.
fn same(want: &Value, got: &Value) -> bool {
    let (w, g) = (wire(want), wire(got));
    let (Some(wo), Some(go)) = (w.as_object(), g.as_object()) else { return w == g };
    wo.len() == go.len() && wo.iter().all(|(k, x)| (k == "id" && x == "random") || go.get(k) == Some(x))
}

/// Is `got` a part of `want`, in its order.
fn subsequence(want: &[Value], got: &[Value]) -> bool {
    let mut it = want.iter();
    got.iter().all(|g| it.any(|w| same(w, g)))
}

/// The SQL a client sent, its placeholders as PostgreSQL's (psycopg's `%s` are `$n`).
fn dollars(sql: &str) -> String {
    let mut n = 0;
    let mut out = String::new();
    let mut rest = sql;
    while let Some(at) = rest.find("%s") {
        n += 1;
        out.push_str(&rest[..at]);
        out.push_str(&format!("${n}"));
        rest = &rest[at + 2..];
    }
    out.push_str(rest);
    out
}

fn order(v: &Value) -> (u64, u64) {
    (v["step"].as_u64().unwrap_or(0), v["caller"].as_u64().unwrap_or(0))
}

/// Compare what a client sent, operation by operation, with `chobo run --show`: for PostgreSQL
/// the SQL and its parameters, for TigerBeetle the hold it reads first, the accounts and
/// openings it makes sure of (each the first time this value of the book needs it), and the
/// chain, field by field. Every account the show names is sent at least once.
pub fn compare_sent(c: &Case, backend: &str, i: usize, tenant: &str, got: &Value) -> Result<(), String> {
    let s = scenario::from_json(&c.copy, &scenario::to_json(&c.book, &c.scenarios[i])).unwrap();
    let want = if backend == "postgres" { chobo::render::postgres_json(&c.copy, tenant, &s) } else { chobo::render::tigerbeetle_json(&c.copy, tenant, &s) };
    let want = want.as_array().unwrap();
    let mut got: Vec<Value> = got.as_array().cloned().unwrap_or_default();
    got.sort_by_key(order);
    if want.len() != got.len() {
        return Err(format!("{} operations were sent, and the show has {}", got.len(), want.len()));
    }
    let mut sent_accounts: Vec<String> = Vec::new();
    let mut shown_accounts: Vec<String> = Vec::new();
    for (w, g) in want.iter().zip(&got) {
        let at = format!("step {} {}.{}", w["step"], w["kind"].as_str().unwrap_or(""), w["op"].as_str().unwrap_or(""));
        if order(w) != order(g) || w["op"] != g["op"] || w["kind"] != g["kind"] {
            return Err(format!("{at}: the client sent {}.{} at step {}", g["kind"], g["op"], g["step"]));
        }
        let requests = g["requests"].as_array().cloned().unwrap_or_default();
        if backend == "postgres" {
            let [r] = requests.as_slice() else { return Err(format!("{at}: {} requests", requests.len())) };
            let sql = dollars(r["sql"].as_str().unwrap_or(""));
            if sql != w["sql"] || r["params"] != w["params"] {
                return Err(format!("{at}: sent {sql} {}, and the show has {} {}", r["params"], w["sql"], w["params"]));
            }
            continue;
        }
        let mut rest: &[Value] = &requests;
        if let Some(lookup) = w.get("lookup") {
            match rest.first() {
                Some(r) if r.get("lookup_transfers") == Some(lookup) => rest = &rest[1..],
                other => return Err(format!("{at}: the hold is read as {other:?}, and the show reads {lookup}")),
            }
        }
        let sent = &w["sent"];
        if sent.is_null() {
            if !rest.is_empty() {
                return Err(format!("{at}: the show sends nothing, and the client sent {rest:?}"));
            }
            continue;
        }
        let shown = |k: &str| sent[k].as_array().cloned().unwrap_or_default();
        let (accounts, openings, transfers) = (shown("accounts"), shown("openings"), shown("transfers"));
        shown_accounts.extend(accounts.iter().filter_map(|a| a["id"].as_str().map(String::from)));
        let mut sent_a = Vec::new();
        while let Some(r) = rest.first().and_then(|r| r.get("create_accounts")) {
            sent_a.extend(r.as_array().cloned().unwrap_or_default());
            rest = &rest[1..];
        }
        if !subsequence(&accounts, &sent_a) {
            return Err(format!("{at}: the accounts sent are not the show's:\n  sent {sent_a:?}\n  show {accounts:?}"));
        }
        sent_accounts.extend(sent_a.iter().filter_map(|a| a["id"].as_str().map(String::from)));
        let Some((last, before)) = rest.split_last() else { return Err(format!("{at}: no chain was sent")) };
        let mut sent_o = Vec::new();
        for r in before {
            match r.get("create_transfers") {
                Some(x) => sent_o.extend(x.as_array().cloned().unwrap_or_default()),
                None => return Err(format!("{at}: an unexpected request {r}")),
            }
        }
        if !subsequence(&openings, &sent_o) {
            return Err(format!("{at}: the openings sent are not the show's:\n  sent {sent_o:?}\n  show {openings:?}"));
        }
        let chain = last.get("create_transfers").and_then(|x| x.as_array()).cloned().unwrap_or_default();
        if chain.len() != transfers.len() || !transfers.iter().zip(&chain).all(|(w, g)| same(w, g)) {
            return Err(format!("{at}: the chain sent is not the show's:\n  sent {chain:?}\n  show {transfers:?}"));
        }
    }
    for a in &shown_accounts {
        if !sent_accounts.contains(a) {
            return Err(format!("the account {a} is never sent"));
        }
    }
    Ok(())
}
