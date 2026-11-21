//! ringi's reference interpreter, held to `RingiCore` (DESIGN 11.3).
//!
//! The machine of a ticket, `lower/ticket.kn` in ringi's crate, is written in Lean as `RingiCore`.
//! What a request gives that machine, the data and the functions of its glue, the model does not
//! hold: it reads the data from a file this writes (`ringi::model::Model::spec`), and asks the
//! answers of the functions as it needs them. Every request `ritsu gen` makes code for, ringi's
//! examples and its tests of the machine (the same list as `crates/ritsu/tests/gen_requests.rs`):
//! each vector `ringi vectors` writes for it is given to the model without its answer, with the
//! answers to the questions the model has asked so far. The model asks the next one
//! (`{"need": …}`), the reference interpreter answers it (`ringi::model::Asking`), and the vector is
//! given again with that answer too, until the model answers it (`{"answer": …}`): the ticket and
//! the things to do the same as the vector's, or a refusal of the same kind. A vector the model asks
//! more than [`MOST`] times of is taken to ask without end, and differs.
//!
//! Every request is run before the test fails, and a request that differs shows how many of its
//! lines do; the test then shows the first five lines that differ of the first such request.
//!
//! `-- --nocapture` shows a `compared ringi <request>: <n> lines` line for each request.
//! `RINGI_MODEL_ONLY=<request>` runs that request alone, `RINGI_MODEL_DUMP=<directory>` writes
//! there each request's spec (`<request>.spec.json`) and every line given to the model and
//! answered by it (`<request>.trips.jsonl`), and `RINGI_MODEL_PROGRAM=<file>` runs another build of
//! the program (a model changed in one place, to see the comparison fail).

use ringi::model::{self, Model};
use ringi::suite::Suite;
use ritsu_base::json::{self, Json};
use ritsu_model::{Compared, program};
use ritsu_testkit::{Need, TempDir, ready};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::rc::Rc;
use std::sync::Arc;
use std::thread::JoinHandle;

/// ringi's examples, five kinds of request each in English and in Japanese.
const EXAMPLES: &[&str] = &[
    "examples/account.ringi",
    "examples/account.ja.ringi",
    "examples/contract.ringi",
    "examples/contract.ja.ringi",
    "examples/expense.ringi",
    "examples/expense.ja.ringi",
    "examples/leave.ringi",
    "examples/leave.ja.ringi",
    "examples/purchase.ringi",
    "examples/purchase.ja.ringi",
];

/// ringi's tests of the machine that have a scenario to make vectors from.
const MACHINE: &[&str] = &[
    "tests/machine/absent.ringi",
    "tests/machine/ahead.ringi",
    "tests/machine/ahead.ja.ringi",
    "tests/machine/apart.ringi",
    "tests/machine/behind.ringi",
    "tests/machine/behind.ja.ringi",
    "tests/machine/deep.ringi",
    "tests/machine/desk.ringi",
    "tests/machine/desk.ja.ringi",
    "tests/machine/flows.ringi",
    "tests/machine/held.ringi",
    "tests/machine/held.ja.ringi",
    "tests/machine/last.ringi",
    "tests/machine/last.ja.ringi",
    "tests/machine/late.ringi",
    "tests/machine/once.ringi",
    "tests/machine/ranges.ringi",
    "tests/machine/reasons.ringi",
    "tests/machine/reminds.ringi",
    "tests/machine/seat.ringi",
    "tests/machine/steps.ringi",
    "tests/machine/supplies.ringi",
    "tests/machine/supplies.ja.ringi",
    "tests/machine/top.ringi",
    "tests/machine/waits.ringi",
    "tests/machine/who.ringi",
];

/// The most lines one vector is given to the model.
const MOST: usize = 400;

fn ringi_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../ringi")
}

/// Every language joined, as ringi's own tests join them (`crates/ringi/tests/common`): rulec,
/// koyomi, chobo, dandori, sekisho and kaname, each through its own engine.
fn joined() -> Suite {
    let rulec = Rc::new(rulec::ports::Engine::with_dates(Arc::new(koyomi::ports::Engine)));
    let koyomi = Rc::new(koyomi::ports::Engine);
    let chobo = Rc::new(chobo::ports::Engine);
    let dandori = Rc::new(dandori::ports::Engine::default());
    let gates = ritsu_ports::GatePorts { rules: rulec.clone(), dates: koyomi.clone(), books: chobo.clone(), flows: dandori.clone(), items: dandori.clone() };
    let sekisho = Rc::new(sekisho::ports::Engine::new(gates.into()));
    Suite { rules: Some(rulec), dates: Some(koyomi), books: Some(chobo), flows: Some(dandori), gates: Some(sekisho), programs: Some(Rc::new(kaname::ports::Engine)) }
}

/// A request by its file's name without `.ringi`: `leave`, `leave.ja`.
fn name_of(path: &str) -> String {
    let file = Path::new(path).file_name().unwrap().to_string_lossy();
    file.strip_suffix(".ringi").unwrap_or(&file).to_string()
}

/// `ritsu-model ringi <spec>`, running: a line written to it, a line read back.
struct Lean {
    child: Child,
    to: BufWriter<ChildStdin>,
    from: BufReader<ChildStdout>,
    said: Option<JoinHandle<String>>,
}

impl Lean {
    fn start(bin: &Path, spec: &Path) -> Result<Lean, String> {
        let mut child = Command::new(bin)
            .arg("ringi")
            .arg(spec)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot run {}: {e}", bin.display()))?;
        let to = BufWriter::new(child.stdin.take().expect("piped stdin"));
        let from = BufReader::new(child.stdout.take().expect("piped stdout"));
        let mut err = child.stderr.take().expect("piped stderr");
        let said = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = err.read_to_string(&mut s);
            s
        });
        Ok(Lean { child, to, from, said: Some(said) })
    }

    /// The model's answer to the line; None where it stopped before it answered.
    fn trip(&mut self, line: &str) -> Option<String> {
        writeln!(self.to, "{line}").ok()?;
        self.to.flush().ok()?;
        let mut back = String::new();
        match self.from.read_line(&mut back) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(back.trim_end_matches(['\n', '\r']).to_string()),
        }
    }

    /// The program let go of: what it wrote to its standard error, and Err where it did not exit
    /// well.
    fn end(mut self) -> Result<String, String> {
        drop(self.to);
        let status = self.child.wait().map_err(|e| format!("ritsu-model did not end: {e}"))?;
        let said = self.said.take().and_then(|h| h.join().ok()).unwrap_or_default();
        if status.success() { Ok(said) } else { Err(format!("ritsu-model ringi exited with {status}:\n{said}")) }
    }
}

/// Whether the model's answer is the vector's: both `ok` with the same ticket and the same things
/// to do (the keys of an object in any order), or both `err` with a refusal of the same kind (its
/// sentences are not compared).
fn same(want: &Json, got: &Json) -> bool {
    match (want.get("ok"), got.get("ok"), want.get("err"), got.get("err")) {
        (Some(a), Some(b), ..) => ["ticket", "effects"].iter().all(|k| a.get(k).is_some() && a.get(k).map(model::sorted) == b.get(k).map(model::sorted)),
        (None, None, Some(a), Some(b)) => a.get("refused").is_some() && a.get("refused") == b.get("refused"),
        _ => false,
    }
}

/// Each line given to the model and each it gave back, as `RINGI_MODEL_DUMP` writes them: kept only
/// where they are to be written.
struct Trips(Option<String>);

impl Trips {
    fn sent(&mut self, j: &Json) {
        if let Some(t) = &mut self.0 {
            t.push_str(&Json::obj([("sent", j.clone())]).compact());
            t.push('\n');
        }
    }

    fn got(&mut self, line: &str) {
        if let Some(t) = &mut self.0 {
            let j = json::parse(line).unwrap_or_else(|_| Json::str(line));
            t.push_str(&Json::obj([("got", j)]).compact());
            t.push('\n');
        }
    }

    fn keep(&self, dump: Option<&Path>, name: &str, spec: &Json) {
        let (Some(dir), Some(t)) = (dump, &self.0) else { return };
        std::fs::write(dir.join(format!("{name}.spec.json")), spec.pretty() + "\n").unwrap();
        std::fs::write(dir.join(format!("{name}.trips.jsonl")), t).unwrap();
    }
}

/// Every vector of the request run by the model, a vector's lines one at a time.
fn run(bin: &Path, tmp: &TempDir, name: &str, m: &Model, dump: Option<&Path>) -> Result<Compared, String> {
    let spec = m.spec().map_err(|e| e.en)?;
    let file = tmp.write(&format!("{name}.spec.json"), spec.compact());
    let mut lean = Lean::start(bin, &file)?;
    let mut trips = Trips(dump.map(|_| String::new()));
    let mut out = Compared::default();
    for v in &m.vectors {
        let (line, want) = (m.line(v), m.answer(v));
        let mut asking = m.asking(v);
        let mut answers: Vec<Json> = Vec::new();
        let mut sent = String::new();
        let mut got: Result<Json, String> = Err(format!("no answer in {MOST} lines"));
        for _ in 0..MOST {
            let given = Json::obj([("vector", line.clone()), ("answers", Json::Arr(answers.clone()))]);
            sent = given.compact();
            trips.sent(&given);
            let Some(back) = lean.trip(&sent) else {
                trips.keep(dump, name, &spec);
                let said = lean.end().unwrap_or_else(|e| e);
                return Err(format!("ritsu-model stopped before it answered `{}`:\n{said}", v.from));
            };
            trips.got(&back);
            let j = match json::parse(&back) {
                Ok(j) => j,
                Err(_) => {
                    got = Err(format!("not JSON: {back}"));
                    break;
                }
            };
            if let Some(q) = j.get("need") {
                match asking.answer(q) {
                    Ok(a) => {
                        answers.push(Json::obj([("q", q.clone()), ("a", a)]));
                        continue;
                    }
                    Err(e) => {
                        got = Err(format!("a question the test cannot answer: {e}"));
                        break;
                    }
                }
            }
            got = match (j.get("answer"), j.get("error")) {
                (Some(a), _) => Ok(a.clone()),
                (None, Some(e)) => Err(format!("error: {}", e.as_str().map_or_else(|| e.compact(), str::to_string))),
                _ => Err(format!("neither a question nor an answer: {back}")),
            };
            break;
        }
        out.lines += 1;
        if !got.as_ref().is_ok_and(|g| same(&want, g)) {
            out.differ += 1;
            if out.first.len() < 5 {
                out.first.push((format!("{}: {sent}", v.from), want.compact(), got.map_or_else(|e| e, |g| g.compact())));
            }
        }
    }
    trips.keep(dump, name, &spec);
    lean.end()?;
    Ok(out)
}

#[test]
fn every_vector_of_every_request_runs_as_the_model_says() {
    let bin = std::env::var_os("RINGI_MODEL_PROGRAM").map_or_else(program, PathBuf::from);
    if !ready(Need::Lean, || bin.exists(), "proofs/ is not built (lake build makes ritsu-model)") {
        return;
    }
    let only = std::env::var("RINGI_MODEL_ONLY").ok();
    let dump = std::env::var_os("RINGI_MODEL_DUMP").map(PathBuf::from);
    if let Some(d) = &dump {
        std::fs::create_dir_all(d).unwrap();
    }
    let tmp = TempDir::new("model-ringi");
    let (mut requests, mut lines, mut differ) = (0, 0usize, 0usize);
    let mut failed: Option<String> = None;
    for path in EXAMPLES.iter().chain(MACHINE) {
        let name = name_of(path);
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        let full = ringi_dir().join(path).display().to_string();
        let c = model::with(&full, joined(), |m| run(&bin, &tmp, &name, m, dump.as_deref())).unwrap_or_else(|e| panic!("{path}: {}", e.en)).unwrap_or_else(|e| panic!("{path}: {e}"));
        if c.differ == 0 {
            println!("compared ringi {name}: {} lines", c.lines);
        } else {
            println!("compared ringi {name}: {} lines, {} differ", c.lines, c.differ);
            failed.get_or_insert_with(|| c.report(&format!("ringi {name}")));
        }
        requests += 1;
        lines += c.lines;
        differ += c.differ;
    }
    if let Some(report) = failed {
        panic!("{differ} of {lines} lines of {requests} requests differ from the model; the first request that differs:\n{report}");
    }
    if let Some(o) = &only {
        assert_eq!(requests, 1, "no request is named {o}");
        return;
    }
    assert!(requests >= 34, "the requests are fewer than they were: {requests}");
    assert!(lines >= 10_000, "the vectors are fewer than they were: {lines}");
}
