//! One claim at a time: its `when`s through the adapters, its checks against what
//! they observed, what it keeps for later steps (DESIGN §22), and the journal events. A
//! `when` that gets no observation ends the claim as an error with a code (E030-E033),
//! the command and its stderr as notes, and the run that gets there. In `map`, every
//! process a claim starts gets the coverage switches and a directory of its own
//! (DESIGN §7.3).

pub use crate::check::CheckResult;
use ritsu_base::text::Text;
use crate::check::{self, trim_one_newline};
use crate::cover;
use crate::diag::{self, Diag, DiagExt};
use crate::drift;
use crate::gui;
use crate::http;
use crate::json;
use crate::keep::{self, Held, NotRead, View};
use crate::model::*;
use crate::proc::{self, AutoPort, Env, Failure, Launch, Server};
use crate::sched;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The longest observation a step of the run that gets there shows.
const OBSERVED: usize = 100;

#[derive(Clone)]
pub enum Obs {
    Proc { stdout: String, stderr: String, exit: i32 },
    Http { status: u16, headers: Vec<(String, String)>, body: String },
    /// What a GUI target shows after an action (DESIGN §8).
    Screen(crate::screen::Node),
}

impl Obs {
    /// What a step of a run shows: `exit 1, stdout "…", stderr "…"` for a process,
    /// `200, body "…"` for HTTP, in at most 100 characters.
    pub fn summary(&self) -> String {
        let s = match self {
            Obs::Proc { stdout, stderr, exit } => {
                let mut parts = vec![format!("exit {exit}")];
                if !stdout.is_empty() {
                    parts.push(format!("stdout {}", json::quote(trim_one_newline(stdout))));
                }
                if !stderr.is_empty() {
                    parts.push(format!("stderr {}", json::quote(trim_one_newline(stderr))));
                }
                parts.join(", ")
            }
            Obs::Http { status, body, .. } => {
                if body.is_empty() {
                    status.to_string()
                } else {
                    format!("{status}, body {}", json::quote(trim_one_newline(body)))
                }
            }
            Obs::Screen(screen) => screen.summary(),
        };
        diag::cut(&s, OBSERVED)
    }
}

pub struct ObsRec {
    /// The `when`'s place among the `when`s of the run it is in.
    pub idx: usize,
    /// For a claim with `in any order`, the number of the order it ran in (DESIGN §23).
    pub order: Option<usize>,
    /// Where the `when` is written among the claim's steps, whatever order it ran in.
    pub step: usize,
    pub line: usize,
    pub target: String,
    /// The call as written after the target: `get("/total")`.
    pub call: String,
    /// The call as a baseline snapped before its strings were escaped holds it, where that differs
    /// (`Call::unescaped`, DESIGN §22.1).
    pub unescaped: Option<String>,
    pub obs: Obs,
    /// How the `when` waited, when it has `within`.
    pub waited: Option<Waited>,
    /// What the claim holds once the `keep`s under this `when` have read its answer: what the
    /// journal, the baseline and drift write as `{name}` in it (DESIGN §22).
    pub held: Held,
}

/// How a `when` with `within` waited (DESIGN §18): the time it was given, how many calls it
/// made, how long from the first call to the last observation, and whether every check under
/// the `when` held on the observation kept (else the time ran out, and it is the last one).
#[derive(Clone, Debug, PartialEq)]
pub struct Waited {
    pub wait: Wait,
    pub tries: usize,
    pub ms: u64,
    pub held: bool,
}

impl Waited {
    /// The calls and the time, as a person reads them: `47 calls in 5.0 s`, `5.0 秒で 47 回`.
    pub fn spent(&self) -> Text {
        let secs = format!("{:.1}", self.ms as f64 / 1000.0);
        Text::new(format!("{secs} 秒で {} 回", self.tries), format!("{} in {secs} s", diag::count(self.tries, "call", "calls")))
    }
}

/// One step of the run that gets there: its line, `api.get("/")` or `keep body as b`, and what it
/// observed or read, or None for the step the claim stopped at; and its `within`, if it waits.
#[derive(Clone)]
pub struct RunLine {
    pub line: usize,
    /// `when` or `keep`.
    pub word: &'static str,
    /// What follows the word, as the spec writes it. What the `--json` of a run calls `call`, a
    /// `keep` with its word.
    pub when: String,
    pub observed: Option<String>,
    pub waited: Option<Waited>,
}

impl RunLine {
    pub fn step(&self) -> diag::Step {
        let wait = self.waited.as_ref().map(|w| format!(" {}", w.wait.display())).unwrap_or_default();
        let head = if self.word == "keep" { self.when.clone() } else { format!("{} {}", self.word, self.when) };
        let text = match &self.observed {
            Some(o) => format!("{head}{wait}  →  {o}"),
            None => format!("{head}{wait}"),
        };
        diag::Step { line: self.line, text: Text::same(text) }
    }
}

pub enum ClaimStatus {
    Ok,
    Fail,
    Error(Diag),
}

pub struct ClaimResult {
    pub name: String,
    pub line: usize,
    pub status: ClaimStatus,
    /// Of a claim with `in any order`, the checks of the order it shows ([`Orders::shown`]).
    pub checks: Vec<CheckResult>,
    /// Of every order it ran in, in their order.
    pub observations: Vec<ObsRec>,
    /// For a claim that is not ok: its steps up to the problem, in the order it shows.
    pub run: Vec<RunLine>,
    /// Every `when` that observed something and every `keep` that read a value, in order, in the
    /// order it shows.
    pub trail: Vec<RunLine>,
    /// Every process the claim started, in order, in every order it ran in.
    pub started: Vec<Started>,
    /// For a claim with `in any order`, how its orders went (DESIGN §23).
    pub orders: Option<Orders>,
}

/// How the orders of a claim with `in any order` went (DESIGN §23).
#[derive(Clone, Debug)]
pub struct Orders {
    /// How many orders the block makes.
    pub count: usize,
    /// The line `in` is written on.
    pub line: usize,
    /// The orders in which a check failed, in their order, each by its number with the lines of the
    /// block's `when`s as they ran.
    pub failed: Vec<(usize, Vec<usize>)>,
    /// The order whose checks and run the result holds: the one that ended in an error, else the
    /// first that failed, else the first.
    pub shown: usize,
}

/// A process a claim started, for `map` to read what it ran.
pub struct Started {
    pub target: String,
    /// The place of the `when` that started it.
    pub line: usize,
    pub col: usize,
    pub pid: u32,
    pub words: Vec<String>,
    /// The program its first word starts, resolved as the OS would (in `map`).
    pub program: Option<PathBuf>,
    /// The directory its coverage goes to (in `map`).
    pub dir: Option<PathBuf>,
    /// A service that did not exit within 5 s of SIGTERM and was killed.
    pub killed: bool,
}

/// How a claim is run: in `map`, with the coverage switches of a session, the
/// claim's number naming its directory; and what its GUI targets share, as seen by
/// the worker running it.
#[derive(Clone, Copy)]
pub struct Opts<'a> {
    pub cover: Option<&'a cover::Session>,
    pub gui: &'a gui::Shared,
    pub worker: usize,
}

/// Runs every claim, up to `jobs` at once (DESIGN §10); the results and the
/// journal in claim order, whatever finished first. `geas_dir` is the spec's
/// `.geas/`, absolute, and `stem` the spec's name, for what GUI targets write.
pub fn run_spec(spec: &Spec, dir: &Path, geas_dir: &Path, stem: &str, jobs: usize) -> (Vec<ClaimResult>, Vec<String>) {
    let shared = gui::Shared::new(geas_dir, stem, jobs);
    let runs = sched::each(spec, jobs, |worker, i| {
        run_claim(spec, &spec.claims[i], i + 1, dir, Opts { cover: None, gui: &shared, worker })
    });
    let mut journal = Vec::new();
    let results = runs
        .into_iter()
        .map(|(r, j)| {
            journal.extend(j);
            r
        })
        .collect();
    (results, journal)
}

/// A service a claim started: its port, and the port it holds from the registry
/// when the target has `port auto`, given back once the service is stopped.
struct Instance {
    server: Server,
    /// Its place in the claim's started processes.
    index: usize,
    port: u16,
    auto: Option<AutoPort>,
}

/// The environment of the k-th process of a claim: what its target's pins give
/// it, and in `map` the coverage switches, with the directory they point into.
fn environment(tg: &Target, port: Option<u16>, opts: Opts, claim_no: usize, k: usize) -> Result<(Option<PathBuf>, Env), Failure> {
    let mut env = Env::of(&tg.pins, port);
    let Some(session) = opts.cover else {
        return Ok((None, env));
    };
    match session.process(claim_no, k) {
        Ok(d) => {
            session.switch(&mut env, &d);
            Ok((Some(d), env))
        }
        Err(e) => Err(Failure {
            code: "E081",
            msg: tr!(
                "このプロセスのカバレッジを書くディレクトリを作れません: {e}",
                "cannot make the directory for the coverage of this process: {e}",
            ),
            notes: vec![],
        }),
    }
}

/// Runs one claim; `claim_no` (from 1) names its coverage directory in `map`. The
/// result, and the claim's lines of the journal. A claim with `in any order` runs once in
/// each of its orders, one after another, each with its services started afresh; it holds
/// when it holds in every order, and an order that ends in an error ends it (DESIGN §23).
pub fn run_claim(spec: &Spec, claim: &Claim, claim_no: usize, dir: &Path, opts: Opts) -> (ClaimResult, Vec<String>) {
    let (Some(block), Some(orders)) = (&claim.any_order, claim.orders()) else {
        return run_once(spec, claim, None, claim_no, dir, opts, 0);
    };
    let mut journal = Vec::new();
    let mut started: Vec<Started> = Vec::new();
    let mut observations: Vec<ObsRec> = Vec::new();
    let mut failed: Vec<(usize, Vec<usize>)> = Vec::new();
    let (mut first, mut first_failed, mut broke) = (None, None, None);
    for order in &orders {
        let ordered = Claim { name: claim.name.clone(), steps: order.steps.clone(), pos: claim.pos, any_order: None };
        let (mut r, lines) = run_once(spec, &ordered, Some((order.number, &order.places)), claim_no, dir, opts, started.len());
        journal.extend(lines.into_iter().map(|l| in_order(l, &claim.name, order.number)));
        started.append(&mut r.started);
        observations.append(&mut r.observations);
        match &mut r.status {
            ClaimStatus::Error(d) => {
                let lines: Vec<String> = order.lines.iter().map(usize::to_string).collect();
                let (n, count) = (order.number, orders.len());
                d.notes.push(tr!(
                    "`in any order` の {count} 通りの順のうち、{n} 番目の順（{} 行目の順）で起きました。そのあとの順は走らせていません",
                    "this is order {n} of the {count} of `in any order`, the steps on lines {} in that order; geas ran no order after it",
                    lines.join("、");
                    lines.join(", "),
                ));
                broke = Some((r, order.number));
                break;
            }
            ClaimStatus::Fail => {
                failed.push((order.number, order.lines.clone()));
                if first_failed.is_none() {
                    first_failed = Some((r, order.number));
                }
            }
            ClaimStatus::Ok => {
                if first.is_none() {
                    first = Some((r, order.number));
                }
            }
        }
        // the run is ending on a signal: no order more
        if proc::signalled().is_some() {
            break;
        }
    }
    let (mut r, shown) = broke.or(first_failed).or(first).expect("one order ran");
    r.observations = observations;
    r.started = started;
    r.orders = Some(Orders { count: orders.len(), line: block.pos.line, failed, shown });
    (r, journal)
}

/// A line of the journal with the number of the order it ran in, right after its claim.
fn in_order(line: String, claim: &str, n: usize) -> String {
    let head = format!("{{\"claim\":{},", json::quote(claim));
    match line.strip_prefix(&head) {
        Some(rest) => format!("{head}\"order\":{n},{rest}"),
        None => line,
    }
}

/// One run of a claim's steps, with its services started for it. `order` is, for a claim with `in
/// any order`, the number of the order and where each step is written among the claim's; `prior`
/// how many processes the claim's earlier orders started, so that each has a coverage directory of
/// its own in `map`.
fn run_once(spec: &Spec, claim: &Claim, order: Option<(usize, &[usize])>, claim_no: usize, dir: &Path, opts: Opts, prior: usize) -> (ClaimResult, Vec<String>) {
    let mut journal: Vec<String> = Vec::new();
    let term = opts.cover.is_some();
    let mut servers: HashMap<String, Instance> = HashMap::new();
    let mut checks: Vec<CheckResult> = Vec::new();
    // a step that gets no observation: what it says, where, its target, and its line of the run
    let mut error: Option<(Failure, Pos, String, RunLine)> = None;
    let mut obs: Option<Obs> = None;
    let mut observations: Vec<ObsRec> = Vec::new();
    let mut started: Vec<Started> = Vec::new();
    let mut when_idx = 0usize;
    // what the claim has kept so far (DESIGN §22)
    let mut held = Held::default();
    // the steps that observed or read something, in order
    let mut trail: Vec<RunLine> = Vec::new();
    // how far along that the last failed check had come: its run
    let mut failed_after: Option<usize> = None;

    // the targets this claim has used, for the journal's `env` event
    let mut used: Vec<&str> = Vec::new();
    let mut gui = gui::ClaimGui::new(opts.gui, opts.worker, claim, claim_no, dir);

    for (i, step) in claim.steps.iter().enumerate() {
        match step {
            Step::When { target, call, pos, wait } => {
                let tg = spec.target(target).expect("the parser resolved every target");
                if !used.contains(&target.as_str()) {
                    used.push(target);
                    if !tg.pins.is_empty() {
                        journal.push(format!(
                            "{{\"claim\":{},\"event\":\"env\",\"target\":{},\"pins\":{}}}",
                            json::quote(&claim.name),
                            json::quote(target),
                            tg.pins.json()
                        ));
                    }
                }
                // the steps under this `when`, which a wait holds each observation to (DESIGN §18)
                let under: Vec<&Step> = claim.steps[i + 1..].iter().take_while(|s| !matches!(s, Step::When { .. })).collect();
                // a wait counts from its first call: the start of a service it is the first to use
                // is not counted
                let mut begun: Option<Instant> = None;
                let mut tries = 0usize;
                let mut waited: Option<Waited> = None;
                // the path a request went out on, for its contract
                let mut sent_path: Option<String> = None;
                let result = loop {
                    tries += 1;
                    let k = prior + started.len() + 1;
                    let record = |pid: u32, words: &[String], d: Option<PathBuf>| Started {
                        target: target.clone(),
                        line: pos.line,
                        col: pos.col,
                        pid,
                        // `map` shows them
                        words: words.iter().map(|w| held.text(w, View::Report)).collect(),
                        program: opts.cover.and_then(|_| proc::resolve(&words[0], dir)),
                        dir: d,
                        killed: false,
                    };
                    let attempt = match (&tg.kind, call) {
                        (TargetKind::Run(base), Call::Run(args)) => match environment(tg, None, opts, claim_no, k) {
                            Err(f) => Err(f),
                            Ok((d, env)) => {
                                let launch = Launch { dir, env: &env, term };
                                let mut s = None;
                                begun.get_or_insert_with(Instant::now);
                                let r = proc::run_process(target, base, &held.fill_args(args), &launch, &mut |pid, words| {
                                    s = Some(record(pid, words, d.clone()));
                                });
                                started.extend(s);
                                r
                            }
                        },
                        (TargetKind::Serve { words: cmd, port }, _) => {
                            let up = if servers.contains_key(target.as_str()) {
                                Ok(())
                            } else {
                                match take_port(target, *port) {
                                    Err(f) => Err(f),
                                    Ok((p, auto)) => match environment(tg, Some(p), opts, claim_no, k) {
                                        Err(f) => Err(f),
                                        Ok((d, env)) => {
                                            let launch = Launch { dir, env: &env, term };
                                            let mut s = None;
                                            let r = proc::start_server(target, cmd, p, auto.is_some(), &launch, &mut |pid, words| {
                                                s = Some(record(pid, words, d.clone()));
                                            });
                                            let index = started.len();
                                            started.extend(s);
                                            r.map(|server| {
                                                servers.insert(target.clone(), Instance { server, index, port: p, auto });
                                            })
                                        }
                                    },
                                }
                            };
                            match up {
                                Err(e) => Err(e),
                                Ok(()) => {
                                    let inst = &servers[target.as_str()];
                                    let auto = inst.auto.is_some();
                                    begun.get_or_insert_with(Instant::now);
                                    let answer = match held.sent(call, auto.then_some(inst.port)) {
                                        Err(f) => Err(f),
                                        Ok(Call::Http { method, path, body, headers }) => {
                                            let a = http::exchange(target, inst.port, auto, method.name(), &path, body.as_deref(), &headers);
                                            sent_path = Some(path);
                                            a
                                        }
                                        Ok(Call::Run(_)) => unreachable!("E006 refuses `run` on a service"),
                                        // an action on the service's page in Chrome
                                        Ok(sent) => gui.act(tg, &sent, Some((inst.port, auto))).map(Obs::Screen),
                                    };
                                    // the port belongs to the run, not to the program (DESIGN §10)
                                    answer.map(|o| if auto { http::port_written(o, inst.port) } else { o })
                                }
                            }
                        }
                        // E017 keeps kept names out of a pixie app's calls, which run as one script
                        (TargetKind::Pixie(_), _) => gui.act(tg, call, None).map(Obs::Screen),
                        (TargetKind::Driver(_), _) => held.sent(call, None).and_then(|sent| gui.act(tg, &sent, None)).map(Obs::Screen),
                        _ => unreachable!("E006 refuses a call its target does not take"),
                    };
                    let Some(w) = wait else {
                        break attempt;
                    };
                    let spent = begun.map_or(Duration::ZERO, |b| b.elapsed());
                    let o = match attempt {
                        Ok(o) => o,
                        // an error is not an observation: the claim ends as it does on any `when`
                        Err(mut f) => {
                            if tries > 1 {
                                let secs = format!("{:.1}", spent.as_secs_f64());
                                f.notes.push(tr!(
                                    "待ちの {tries} 回目の呼び出しで、一回目から {secs} 秒後でした",
                                    "call {tries} of the wait, {secs} s after the first",
                                ));
                            }
                            waited = Some(Waited { wait: *w, tries, ms: spent.as_millis() as u64, held: false });
                            break Err(f);
                        }
                    };
                    let settled = settles(&under, &o, &held);
                    let limit = Duration::from_secs(w.secs);
                    // the run is ending on a signal: nothing more is sent
                    if settled == Some(true) || settled.is_none() || spent >= limit || proc::signalled().is_some() {
                        waited = Some(Waited { wait: *w, tries, ms: spent.as_millis() as u64, held: settled != Some(false) });
                        break Ok(o);
                    }
                    std::thread::sleep(Wait::EVERY.min(limit - spent));
                };
                match result {
                    Ok(o) => {
                        // the record of this answer has the values kept from it (DESIGN §22): they
                        // are read first, up to the first that cannot be
                        for s in &under {
                            let Step::Keep(k) = s else { continue };
                            match keep::read(k, Some(&o)) {
                                Ok((value, secret)) => held.push(&k.name, value, secret),
                                Err(_) => break,
                            }
                        }
                        journal.push(journal_when(&claim.name, pos.line, target, call, waited.as_ref(), &drift::recorded(&o, &spec.masks, &held)));
                        trail.push(RunLine {
                            line: pos.line,
                            word: "when",
                            when: format!("{}.{}", target, call.display()),
                            observed: Some(held.obs(&o, View::Report).summary()),
                            waited: waited.clone(),
                        });
                        observations.push(ObsRec {
                            idx: when_idx,
                            order: order.map(|(n, _)| n),
                            step: order.map_or(i, |(_, places)| places[i]),
                            line: pos.line,
                            target: target.clone(),
                            call: call.display(),
                            unescaped: call.unescaped(),
                            obs: o.clone(),
                            waited,
                            held: held.clone(),
                        });
                        when_idx += 1;
                        // what it answered, held to its contract as one more check (DESIGN §20);
                        // of a `when` that waits, what it kept
                        if let Some(cr) = crate::contract::held_to(tg.contract.as_ref(), call, sent_path.as_deref(), &held, pos.line, &o) {
                            journal.push(journal_check(&claim.name, &cr));
                            if !cr.ok {
                                failed_after = Some(trail.len());
                            }
                            checks.push(cr);
                        }
                        obs = Some(o);
                    }
                    Err(f) => {
                        journal.push(journal_error(&claim.name, pos.line, &f, &held));
                        let line = RunLine { line: pos.line, word: "when", when: format!("{}.{}", target, call.display()), observed: None, waited };
                        error = Some((f, *pos, target.clone(), line));
                        break;
                    }
                }
            }
            Step::Then(check) => {
                let o = obs.as_ref().expect("E005 refuses a check before any `when`");
                let cr = check::eval(check, o, &held);
                journal.push(journal_check(&claim.name, &cr));
                if !cr.ok {
                    failed_after = Some(trail.len());
                }
                checks.push(cr);
            }
            Step::Keep(k) => {
                // under a `when`, it was read with that `when`'s answer
                if held.get(&k.name).is_none() {
                    match keep::read(k, obs.as_ref()) {
                        Ok((value, secret)) => held.push(&k.name, value, secret),
                        // the rest of the claim goes on from this value, so it stops here
                        Err(NotRead::Answer(why)) => {
                            let cr = CheckResult {
                                line: k.pos.line,
                                label: k.label(),
                                expected: format!("as {}", k.name),
                                recorded: held.text(&why.en, View::Record),
                                actual: held.words(&why, View::Report),
                                ok: false,
                                screen: None,
                                values: vec![],
                            };
                            journal.push(journal_check(&claim.name, &cr));
                            failed_after = Some(trail.len());
                            checks.push(cr);
                            break;
                        }
                        Err(NotRead::Env(f)) => {
                            journal.push(journal_error(&claim.name, k.pos.line, &f, &held));
                            let line = RunLine { line: k.pos.line, word: "keep", when: k.display(), observed: None, waited: None };
                            error = Some((f, k.pos, String::new(), line));
                            break;
                        }
                    }
                }
                let kept = held.get(&k.name).expect("kept above");
                let placeholder = format!("{{{}}}", k.name);
                // a value kept from an answer can hold what came from the environment
                let told = if kept.secret { placeholder.clone() } else { json::quote(&held.text(&kept.value, View::Report)) };
                let cr = CheckResult {
                    line: k.pos.line,
                    label: k.label(),
                    expected: format!("as {}", k.name),
                    actual: Text::same(told.clone()),
                    recorded: placeholder,
                    ok: true,
                    screen: None,
                    values: vec![],
                };
                journal.push(journal_check(&claim.name, &cr));
                checks.push(cr);
                trail.push(RunLine { line: k.pos.line, word: "keep", when: k.display(), observed: Some(diag::cut(&told, OBSERVED)), waited: None });
            }
        }
    }

    // A claim ends with its GUIs, then its services, each asked to stop with SIGTERM
    // first: a service cleans up as it does whenever it is stopped, and in `map` its
    // runtime writes what it ran.
    gui.close();
    let mut server_stderr: HashMap<String, String> = HashMap::new();
    let mut names: Vec<String> = servers.keys().cloned().collect();
    names.sort();
    for name in names {
        let inst = servers.remove(&name).expect("a started service");
        let (stderr, killed) = proc::stop_server(inst.server);
        started[inst.index].killed = killed;
        server_stderr.insert(name, stderr);
        // an auto port goes back to the registry once its service is gone
        drop(inst.auto);
    }

    let (status, run) = if let Some((mut f, pos, target, line)) = error {
        if f.code == "E033"
            && let Some(err) = server_stderr.get(&target)
        {
            f.notes.extend(proc::stderr_tail(err));
        }
        // what came from the environment is told to nobody, whatever the program did with it
        let f = held.hide_failure(f);
        // every step in the trail came before the one that failed
        let mut run = trail.clone();
        run.push(line);
        let mut d = diag::error(f.code, pos.line, pos.col, f.msg).with_path(run.iter().map(RunLine::step).collect());
        d.notes = f.notes;
        (ClaimStatus::Error(d), run)
    } else if let Some(upto) = failed_after {
        (ClaimStatus::Fail, trail[..upto].to_vec())
    } else {
        (ClaimStatus::Ok, vec![])
    };
    (ClaimResult { name: claim.name.clone(), line: claim.pos.line, status, checks, observations, run, trail, started, orders: None }, journal)
}

/// Whether an answer settles a wait (DESIGN §18): every check under the `when` holds on it and every
/// `keep` under it can read its value, each check with the values kept above it. None when no later
/// answer can: a `keep` wants what geas's environment does not have.
fn settles(under: &[&Step], o: &Obs, held: &Held) -> Option<bool> {
    let mut trial = held.clone();
    for s in under {
        match s {
            Step::Then(c) if !check::eval(c, o, &trial).ok => return Some(false),
            Step::Keep(k) => match keep::read(k, Some(o)) {
                Ok((value, secret)) => trial.push(&k.name, value, secret),
                Err(NotRead::Answer(_)) => return Some(false),
                Err(NotRead::Env(_)) => return None,
            },
            _ => {}
        }
    }
    Some(true)
}

/// The port of a new instance: the one written in the spec, or a free one from the
/// registry, held until the instance is stopped.
fn take_port(target: &str, port: Port) -> Result<(u16, Option<AutoPort>), Failure> {
    match port {
        Port::Fixed(p) => Ok((p, None)),
        Port::Auto => match AutoPort::take() {
            Ok(a) => Ok((a.port, Some(a))),
            // ritsu built for WASI (its npm package) has no sockets, nor any program to give one to
            Err(e) if ritsu_base::wasi::unsupported(&e) => Err(Failure { code: "E030", msg: ritsu_base::wasi::cannot_start_programs(), notes: vec![] }),
            Err(e) => Err(Failure {
                code: "E030",
                msg: tr!(
                    "`{target}` に渡す 127.0.0.1 の空きポートが見つかりません: {e}",
                    "cannot find a free port on 127.0.0.1 for `{target}`: {e}",
                ),
                notes: vec![],
            }),
        },
    }
}

/// The run of a claim up to and with the `when` on `line`, each step with what it
/// observed or read: the path of a diagnostic about something that `when` started. In
/// an order of `in any order`, the lines do not rise with the steps, so the run goes up
/// to that `when` as it ran.
pub fn run_up_to(r: &ClaimResult, line: usize) -> Vec<diag::Step> {
    match r.trail.iter().position(|s| s.line == line && s.word == "when") {
        Some(at) => r.trail[..=at].iter().map(RunLine::step).collect(),
        None => r.trail.iter().filter(|s| s.line <= line).map(RunLine::step).collect(),
    }
}

/// A check in the journal.
fn journal_check(claim: &str, cr: &CheckResult) -> String {
    format!(
        "{{\"claim\":{},\"event\":\"check\",\"line\":{},\"check\":{},\"expected\":{},\"actual\":{},\"ok\":{}}}",
        json::quote(claim),
        cr.line,
        json::quote(&cr.label),
        json::quote(&cr.expected),
        json::quote(&cr.recorded),
        cr.ok
    )
}

/// An error in the journal, the values the claim holds written `{name}` in what it says.
fn journal_error(claim: &str, line: usize, f: &Failure, held: &Held) -> String {
    format!(
        "{{\"claim\":{},\"event\":\"error\",\"line\":{},\"code\":\"{}\",\"message\":{}}}",
        json::quote(claim),
        line,
        f.code,
        json::quote(&held.text(&f.msg.en, View::Record))
    )
}

/// A `when` in the journal, its observation as recorded: masked values written as
/// `<masked>`, so two runs write the same bytes where the spec declares noise. A `when` that
/// waits has its `within`, and the calls and the time only when the time ran out: a wait that
/// held writes the same bytes however many calls it took (DESIGN §18).
fn journal_when(claim: &str, line: usize, target: &str, call: &Call, waited: Option<&Waited>, obs: &Obs) -> String {
    let obs_s = match obs {
        Obs::Proc { stdout, stderr, exit } => format!(
            "{{\"stdout\":{},\"stderr\":{},\"exit\":{}}}",
            json::quote(stdout),
            json::quote(stderr),
            exit
        ),
        Obs::Http { status, headers, body } => format!(
            "{{\"status\":{},\"headers\":{},\"body\":{}}}",
            status,
            drift::headers_json(headers),
            json::quote(body)
        ),
        Obs::Screen(screen) => format!("{{\"screen\":{}}}", screen.json()),
    };
    let wait = match waited {
        None => String::new(),
        Some(w) if w.held => format!(",\"within\":{}", w.wait.secs),
        Some(w) => format!(",\"within\":{},\"tries\":{},\"ms\":{}", w.wait.secs, w.tries, w.ms),
    };
    format!(
        "{{\"claim\":{},\"event\":\"when\",\"line\":{},\"target\":{},\"call\":{}{wait},\"obs\":{}}}",
        json::quote(claim),
        line,
        json::quote(target),
        json::quote(&call.display()),
        obs_s
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summaries_fit_one_line() {
        let p = Obs::Proc { stdout: "5\n".into(), stderr: String::new(), exit: 0 };
        assert_eq!(p.summary(), "exit 0, stdout \"5\"");
        let p = Obs::Proc { stdout: String::new(), stderr: "no\nway\n".into(), exit: 1 };
        assert_eq!(p.summary(), "exit 1, stderr \"no\\nway\"");
        let h = Obs::Http { status: 204, headers: vec![], body: String::new() };
        assert_eq!(h.summary(), "204");
        let h = Obs::Http { status: 200, headers: vec![], body: "x".repeat(200) };
        let s = h.summary();
        assert_eq!(s.chars().count(), 100);
        assert!(s.starts_with("200, body \"xxx") && s.ends_with('…'));
    }
}
