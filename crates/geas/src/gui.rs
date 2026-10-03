//! What the GUI drivers share (DESIGN §8): a claim's actions on its GUI targets, the
//! screen after each, and the notes of a refused action. A live driver (a driver
//! program, a page in Chrome) keeps the app running for the claim and acts as each
//! `when` comes; the replayed one (pixie) runs the claim's actions on an app as one
//! script at the first of them, and hands each `when` its screen.

use crate::cdp::{Browsers, Pages};
use crate::diag;
use ritsu_base::text::Text;
use crate::driver::Live;
use crate::model::{Call, Claim, Step, Target, TargetKind};
use crate::pixie;
use crate::proc::Failure;
use crate::screen::{self, Node};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

/// What the GUI targets of one run share: where pixie's transcripts go, and the
/// browsers, one per worker.
pub struct Shared {
    geas_dir: PathBuf,
    stem: String,
    pub browsers: Browsers,
}

impl Shared {
    /// `geas_dir` is the spec's `.geas/`, absolute.
    pub fn new(geas_dir: &Path, stem: &str, workers: usize) -> Shared {
        Shared { geas_dir: geas_dir.to_path_buf(), stem: stem.to_string(), browsers: Browsers::new(geas_dir, workers) }
    }

    /// Where the transcript of a claim's pixie script goes (PLAN C7).
    fn dump(&self, claim_no: usize) -> PathBuf {
        self.geas_dir.join(format!("{}.pixie-{claim_no}.txt", self.stem))
    }
}

/// The notes of a refused action: the screen the app was on, and what on it the
/// action could have reached, so the message says what was there instead.
pub fn refused_notes(screen: Option<&Node>, call: &Call) -> Vec<Text> {
    let Some(s) = screen else {
        return vec![];
    };
    let summary = diag::cut(&s.summary(), 200);
    let mut notes = vec![tr!("そのときの画面: {summary}", "the screen it was on: {summary}")];
    let pick = |roles: &[&str]| -> Vec<String> {
        s.walk().iter().filter(|(n, _)| roles.contains(&n.role.as_str())).map(|(n, _)| n.line()).collect()
    };
    match call {
        Call::Click { .. } => {
            let found = pick(screen::CLICKABLE);
            notes.push(if found.is_empty() {
                tr!("その画面にクリックできるものはありません", "nothing on it can be clicked")
            } else {
                tr!("クリックできるもの: {}", "what it has to click: {}", found.join("、"); found.join(", "))
            });
        }
        Call::Input { .. } | Call::Submit(_) => {
            let found = pick(screen::FIELDS);
            notes.push(if found.is_empty() {
                tr!("その画面にテキストフィールドはありません", "it has no text field")
            } else {
                tr!("テキストフィールド: {}", "its text fields: {}", found.join("、"); found.join(", "))
            });
        }
        _ => {}
    }
    notes
}

/// A claim's GUI targets while it runs.
pub struct ClaimGui<'a> {
    shared: &'a Shared,
    worker: usize,
    claim: &'a Claim,
    claim_no: usize,
    dir: &'a Path,
    drivers: HashMap<String, Live>,
    replays: HashMap<String, VecDeque<Result<Node, Failure>>>,
    pages: Option<Pages<'a>>,
}

impl<'a> ClaimGui<'a> {
    pub fn new(shared: &'a Shared, worker: usize, claim: &'a Claim, claim_no: usize, dir: &'a Path) -> ClaimGui<'a> {
        ClaimGui { shared, worker, claim, claim_no, dir, drivers: HashMap::new(), replays: HashMap::new(), pages: None }
    }

    /// One action, and the screen after it. A service's `port` (and whether geas
    /// gave it) is there for a page; the claim has started the service already.
    pub fn act(&mut self, tg: &Target, call: &Call, service: Option<(u16, bool)>) -> Result<Node, Failure> {
        match &tg.kind {
            TargetKind::Driver(words) => {
                if !self.drivers.contains_key(&tg.name) {
                    let live = Live::start(tg, words, self.dir)?;
                    self.drivers.insert(tg.name.clone(), live);
                }
                self.drivers.get_mut(&tg.name).expect("started").act(call)
            }
            TargetKind::Pixie(words) => {
                if !self.replays.contains_key(&tg.name) {
                    // every action of the claim on the app, as one script (E012
                    // keeps other targets' `when`s from coming between them)
                    let calls: Vec<&Call> = self
                        .claim
                        .steps
                        .iter()
                        .filter_map(|s| match s {
                            Step::When { target, call, .. } if *target == tg.name => Some(call),
                            _ => None,
                        })
                        .collect();
                    let app = pixie::App { target: tg, words, dir: self.dir, dump: self.shared.dump(self.claim_no) };
                    self.replays.insert(tg.name.clone(), app.replay(&calls).into());
                }
                match self.replays.get_mut(&tg.name).and_then(VecDeque::pop_front) {
                    Some(r) => r,
                    None => unreachable!("a replay has a result for every action up to the first that fails, and the claim stops there"),
                }
            }
            TargetKind::Serve { .. } => {
                let (port, auto) = service.expect("the claim starts the service first");
                if self.pages.is_none() {
                    self.pages = Some(self.shared.browsers.pages(self.worker)?);
                }
                let pages = self.pages.as_mut().expect("made");
                match call {
                    Call::Open(path) => pages.open(tg, port, auto, path.as_deref().unwrap_or("/")),
                    _ => pages.act(tg, call),
                }
            }
            TargetKind::Run(_) => unreachable!("E006 refuses an action on a command"),
        }
    }

    /// Ends the claim's GUIs: drivers told to close, the browser context disposed.
    pub fn close(self) {
        for (_, d) in self.drivers {
            let _ = d.close();
        }
        if let Some(p) = self.pages {
            p.close();
        }
    }
}
