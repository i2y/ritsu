//! One claim at a time: its `when`s through the adapters, its checks against what
//! they observed, and the journal events. A `when` that gets no observation ends the
//! claim as an error with a code (E030-E033), the command and its stderr as notes,
//! and the run that gets there. In `map`, every process a claim starts gets the
//! coverage switches and a directory of its own (DESIGN §7.3).

pub use crate::check::CheckResult;
use ritsu_base::text::Text;
use crate::check::{self, trim_one_newline};
use crate::cover;
use crate::diag::{self, Diag, DiagExt};
use crate::drift;
use crate::gui;
use crate::http;
use crate::json;
use crate::model::*;
use crate::proc::{self, AutoPort, Env, Failure, Launch, Server};
use crate::sched;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
    pub idx: usize,
    pub line: usize,
    pub target: String,
    /// The call as written after the target: `get("/total")`.
    pub call: String,
    pub obs: Obs,
}

/// One `when` of the run that gets there: its line, `api.get("/")`, and what it
/// observed, or None for the `when` the claim stopped at.
pub struct RunLine {
    pub line: usize,
    pub when: String,
    pub observed: Option<String>,
}

impl RunLine {
    pub fn step(&self) -> diag::Step {
        let text = match &self.observed {
            Some(o) => format!("when {}  →  {}", self.when, o),
            None => format!("when {}", self.when),
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
    pub checks: Vec<CheckResult>,
    pub observations: Vec<ObsRec>,
    /// For a claim that is not ok: the `when`s up to the problem.
    pub run: Vec<RunLine>,
    /// Every process the claim started, in order.
    pub started: Vec<Started>,
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
/// result, and the claim's lines of the journal.
pub fn run_claim(spec: &Spec, claim: &Claim, claim_no: usize, dir: &Path, opts: Opts) -> (ClaimResult, Vec<String>) {
    let mut journal: Vec<String> = Vec::new();
    let term = opts.cover.is_some();
    let mut servers: HashMap<String, Instance> = HashMap::new();
    let mut checks: Vec<CheckResult> = Vec::new();
    let mut error: Option<(Failure, Pos, String, String)> = None;
    let mut obs: Option<Obs> = None;
    let mut observations: Vec<ObsRec> = Vec::new();
    let mut started: Vec<Started> = Vec::new();
    let mut when_idx = 0usize;
    // how many observations the last failed check had seen: its run
    let mut failed_after: Option<usize> = None;

    // the targets this claim has used, for the journal's `env` event
    let mut used: Vec<&str> = Vec::new();
    let mut gui = gui::ClaimGui::new(opts.gui, opts.worker, claim, claim_no, dir);

    for step in &claim.steps {
        match step {
            Step::When { target, call, pos } => {
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
                let k = started.len() + 1;
                let record = |pid: u32, words: &[String], d: Option<PathBuf>| Started {
                    target: target.clone(),
                    line: pos.line,
                    col: pos.col,
                    pid,
                    words: words.to_vec(),
                    program: opts.cover.and_then(|_| proc::resolve(&words[0], dir)),
                    dir: d,
                    killed: false,
                };
                let result = match (&tg.kind, call) {
                    (TargetKind::Run(base), Call::Run(args)) => match environment(tg, None, opts, claim_no, k) {
                        Err(f) => Err(f),
                        Ok((d, env)) => {
                            let launch = Launch { dir, env: &env, term };
                            let mut s = None;
                            let r = proc::run_process(target, base, args, &launch, &mut |pid, words| {
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
                                let answer = match call {
                                    Call::Get(path) => http::exchange(target, inst.port, auto, "GET", path, None),
                                    Call::Post { path, body } => {
                                        http::exchange(target, inst.port, auto, "POST", path, body.as_deref())
                                    }
                                    Call::Run(_) => unreachable!("E006 refuses `run` on a service"),
                                    // an action on the service's page in Chrome
                                    _ => gui.act(tg, call, Some((inst.port, auto))).map(Obs::Screen),
                                };
                                // the port belongs to the run, not to the program (DESIGN §10)
                                answer.map(|o| if auto { http::port_written(o, inst.port) } else { o })
                            }
                        }
                    }
                    (TargetKind::Pixie(_) | TargetKind::Driver(_), _) => gui.act(tg, call, None).map(Obs::Screen),
                    _ => unreachable!("E006 refuses a call its target does not take"),
                };
                match result {
                    Ok(o) => {
                        journal.push(journal_when(&claim.name, pos.line, target, call, &drift::recorded(&o, &spec.masks)));
                        observations.push(ObsRec {
                            idx: when_idx,
                            line: pos.line,
                            target: target.clone(),
                            call: call.display(),
                            obs: o.clone(),
                        });
                        when_idx += 1;
                        obs = Some(o);
                    }
                    Err(f) => {
                        journal.push(format!(
                            "{{\"claim\":{},\"event\":\"error\",\"line\":{},\"code\":\"{}\",\"message\":{}}}",
                            json::quote(&claim.name),
                            pos.line,
                            f.code,
                            json::quote(&f.msg.en)
                        ));
                        error = Some((f, *pos, target.clone(), format!("{}.{}", target, call.display())));
                        break;
                    }
                }
            }
            Step::Then(check) => {
                let o = obs.as_ref().expect("E005 refuses a check before any `when`");
                let cr = check::eval(check, o);
                journal.push(format!(
                    "{{\"claim\":{},\"event\":\"check\",\"line\":{},\"check\":{},\"expected\":{},\"actual\":{},\"ok\":{}}}",
                    json::quote(&claim.name),
                    cr.line,
                    json::quote(&cr.label),
                    json::quote(&cr.expected),
                    json::quote(&cr.actual.en),
                    cr.ok
                ));
                if !cr.ok {
                    failed_after = Some(observations.len());
                }
                checks.push(cr);
            }
        }
    }

    // A claim ends with its GUIs, then its services: killed at once, or in `map`
    // asked to stop with SIGTERM first, so that their runtimes write what they ran.
    gui.close();
    let mut server_stderr: HashMap<String, String> = HashMap::new();
    let mut names: Vec<String> = servers.keys().cloned().collect();
    names.sort();
    for name in names {
        let inst = servers.remove(&name).expect("a started service");
        let (stderr, killed) = proc::stop_server(inst.server, term);
        started[inst.index].killed = killed;
        server_stderr.insert(name, stderr);
        // an auto port goes back to the registry once its service is gone
        drop(inst.auto);
    }

    let seen = |upto: usize| -> Vec<RunLine> {
        observations
            .iter()
            .take(upto)
            .map(|o| RunLine {
                line: o.line,
                when: format!("{}.{}", o.target, o.call),
                observed: Some(o.obs.summary()),
            })
            .collect()
    };
    let (status, run) = if let Some((mut f, pos, target, when)) = error {
        if f.code == "E033"
            && let Some(err) = server_stderr.get(&target)
        {
            f.notes.extend(proc::stderr_tail(err));
        }
        // every observation came before the `when` that failed
        let mut run = seen(observations.len());
        run.push(RunLine { line: pos.line, when, observed: None });
        let mut d = diag::error(f.code, pos.line, pos.col, f.msg).with_path(run.iter().map(RunLine::step).collect());
        d.notes = f.notes;
        (ClaimStatus::Error(d), run)
    } else if let Some(upto) = failed_after {
        (ClaimStatus::Fail, seen(upto))
    } else {
        (ClaimStatus::Ok, vec![])
    };
    (ClaimResult { name: claim.name.clone(), line: claim.pos.line, status, checks, observations, run, started }, journal)
}

/// The port of a new instance: the one written in the spec, or a free one from the
/// registry, held until the instance is stopped.
fn take_port(target: &str, port: Port) -> Result<(u16, Option<AutoPort>), Failure> {
    match port {
        Port::Fixed(p) => Ok((p, None)),
        Port::Auto => match AutoPort::take() {
            Ok(a) => Ok((a.0, Some(a))),
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

/// The run of a claim up to and with the `when` on `line`, each `when` with what it
/// observed: the path of a diagnostic about something that `when` started.
pub fn run_up_to(r: &ClaimResult, line: usize) -> Vec<diag::Step> {
    r.observations
        .iter()
        .filter(|o| o.line <= line)
        .map(|o| {
            RunLine { line: o.line, when: format!("{}.{}", o.target, o.call), observed: Some(o.obs.summary()) }.step()
        })
        .collect()
}

/// A `when` in the journal, its observation as recorded: masked values written as
/// `<masked>`, so two runs write the same bytes where the spec declares noise.
fn journal_when(claim: &str, line: usize, target: &str, call: &Call, obs: &Obs) -> String {
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
    format!(
        "{{\"claim\":{},\"event\":\"when\",\"line\":{},\"target\":{},\"call\":{},\"obs\":{}}}",
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
