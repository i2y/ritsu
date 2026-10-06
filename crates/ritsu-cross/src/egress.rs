//! Where a flow sends a secret value, against the map (DESIGN 16.8, X14). A contract marks a value
//! secret (`debug_redact` in a `.proto`, `x-data-classification` and its kin in an OpenAPI
//! document, `secret` in the `.flow` itself); dandori follows the value to every call that gives it
//! to a file of the project — an OpenAPI document a task calls, a `.proto` a `connect` task calls, a
//! rule at its Connect service, a child `.flow`, a book, a dates file (`Flows::sends`); sakai says
//! which context of a map each file belongs to, and how the contexts are related (`Maps`). The
//! parties outside the project (a model's provider, Jev, a URL, an AWS service) are dandori's own
//! check (its E906), which needs no map.
//!
//! A flow that belongs to no context of any map is not looked at: the map says nothing of it. For
//! one that does, each call is a border:
//!
//! - every secret it gives goes to the context that marked it, to one the map relates to that
//!   context (any relationship but `separate ways`), or under the task's `discloses`: shown to
//!   hold, and nothing is said;
//! - a secret goes to a file no context of the map holds, or to a context the map does not relate
//!   to the one that marked it (E905);
//! - otherwise undecided (W905): sakai cannot say which context a file belongs to, its map not
//!   passing its check.
//!
//! The context that marked a secret is the one the file of the mark belongs to; a mark the `.flow`
//! writes (`secret`), or one in a file no context holds, is the flow's own context's.

use crate::{Borders, named, then_ja};
use ritsu_base::diag::Diag;
use ritsu_base::text::{Lang, Text, capitalize};
use ritsu_base::tr;
use ritsu_ports::{Destination, Finding, Flows, MapFacts, Maps, Ports, Said, Secret, Send};
use ritsu_project::{File, Joined, Project};
use std::path::Path;

/// What one secret a call gives comes to (P5).
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Sent to the context that marked it.
    Same,
    /// Sent to a context the map relates to the one that marked it: the words of the relationship.
    Related(String),
    /// Sent under the task's `discloses`, with the reason written.
    Disclosed(String),
    /// Sent to a file no context of the map holds (E905).
    Outside,
    /// Sent to a context the map does not relate to the one that marked it (E905).
    Unrelated,
    /// Which context a file belongs to cannot be said (W905): what sakai says.
    Undecided(Vec<Said>),
}

impl Outcome {
    pub fn holds(&self) -> bool {
        matches!(self, Outcome::Same | Outcome::Related(_) | Outcome::Disclosed(_))
    }

    pub fn fails(&self) -> bool {
        matches!(self, Outcome::Outside | Outcome::Unrelated)
    }
}

/// One secret a call gives, judged.
#[derive(Clone, Debug, PartialEq)]
pub struct Judged {
    /// The parameter that carries it, and the secret.
    pub param: String,
    pub secret: Secret,
    /// The context that marked it, and the one it is sent to (None: no context holds the file, or it
    /// cannot be said).
    pub marked_by: Option<String>,
    pub sent_to: Option<String>,
    pub outcome: Outcome,
}

/// One call, judged: the border it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub send: Send,
    /// The destination's file, from the root (None when it is outside the root).
    pub to: Option<String>,
    pub secrets: Vec<Judged>,
}

impl Call {
    /// What the border comes to: failed if a secret fails, else undecided if one is, else held.
    pub fn count(&self, borders: &mut Borders) {
        if self.secrets.iter().any(|j| j.outcome.fails()) {
            borders.failed += 1;
        } else if self.secrets.iter().any(|j| matches!(j.outcome, Outcome::Undecided(_))) {
            borders.undecided += 1;
        } else {
            borders.held += 1;
        }
    }
}

/// Whether the map relates two contexts: a relationship between them, either way, that is not
/// `separate ways`; the words it is written with.
pub fn related(map: &MapFacts, a: &str, b: &str) -> Option<String> {
    map.relationships.iter().find(|r| !r.separate && ((r.from == a && r.to == b) || (r.from == b && r.to == a))).map(|r| r.words.clone())
}

/// The calls of the flow at `flow` (from the root) that give secrets, judged against `map`, whose
/// contexts `context_of` gives for a file from the root (sakai's answer); `from_root` turns a path
/// as dandori reaches it into one from the root. None when the flow belongs to no context of the
/// map; Err when sakai cannot say whether it does (what it says).
pub fn judge(
    flow: &str,
    sends: &[Send],
    map: &MapFacts,
    context_of: &dyn Fn(&str) -> Result<Option<String>, Vec<Said>>,
    from_root: &dyn Fn(&Path) -> Option<String>,
) -> Result<Option<Vec<Call>>, Vec<Said>> {
    let Some(own) = context_of(flow)? else { return Ok(None) };
    let mut out = Vec::new();
    for s in sends {
        let Destination::File(dest) = &s.to;
        let to = from_root(dest);
        // where the call sends: a context, no context, or what sakai says
        let sent_to: Result<Option<String>, Vec<Said>> = match &to {
            Some(t) => context_of(t),
            None => Ok(None),
        };
        let mut secrets = Vec::new();
        for (param, secret) in &s.secrets {
            let marked_in = from_root(&secret.marked_in);
            let marked_by: Result<String, Vec<Said>> = match &marked_in {
                Some(m) if m == flow => Ok(own.clone()),
                Some(m) => context_of(m).map(|c| c.unwrap_or_else(|| own.clone())),
                None => Ok(own.clone()),
            };
            let disclosed = s.disclosed.iter().find(|(p, _)| p == param).map(|(_, why)| why.clone());
            let (outcome, by, at) = match (disclosed, &marked_by, &sent_to) {
                (Some(why), by, at) => (Outcome::Disclosed(why), by.clone().ok(), at.clone().ok().flatten()),
                (None, Err(said), at) => (Outcome::Undecided(said.clone()), None, at.clone().ok().flatten()),
                (None, Ok(by), Err(said)) => (Outcome::Undecided(said.clone()), Some(by.clone()), None),
                (None, Ok(by), Ok(None)) => (Outcome::Outside, Some(by.clone()), None),
                (None, Ok(by), Ok(Some(at))) if at == by => (Outcome::Same, Some(by.clone()), Some(at.clone())),
                (None, Ok(by), Ok(Some(at))) => match related(map, by, at) {
                    Some(words) => (Outcome::Related(words), Some(by.clone()), Some(at.clone())),
                    None => (Outcome::Unrelated, Some(by.clone()), Some(at.clone())),
                },
            };
            secrets.push(Judged { param: param.clone(), secret: secret.clone(), marked_by: by, sent_to: at, outcome });
        }
        out.push(Call { send: s.clone(), to, secrets });
    }
    Ok(Some(out))
}

/// A map of the project: its file from the root, and its facts or what sakai says of it.
struct Map {
    rel: String,
    facts: Result<MapFacts, Vec<Said>>,
}

/// The maps of the project: each `.ctx` sakai answers for as a map, and each that does not pass
/// its check (whether it is a map cannot be told then; it counts as one that cannot answer).
fn maps(project: &Project, sakai: &dyn Maps) -> Vec<Map> {
    project
        .of(ritsu_base::naming::Tool::Sakai)
        .into_iter()
        .filter_map(|f| match sakai.map(&project.root, &f.rel) {
            Ok(Some(facts)) => Some(Map { rel: f.rel.clone(), facts: Ok(facts) }),
            Ok(None) => None,
            Err(said) => Some(Map { rel: f.rel.clone(), facts: Err(said) }),
        })
        .collect()
}

/// X14 over the project: E905 for each secret sent where the map does not allow it, W905 for each
/// one whose place cannot be decided, and a border for each call.
pub(crate) fn check(project: &Project, joined: &Joined, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    check_with(project, &*joined.dandori, &*joined.maps(), &joined.ports(), lang, borders)
}

/// [`check`], with what dandori says the flows send (`flows`, read with the other languages through
/// `ports`) and what sakai says of the maps (`sakai`) given: the ports, which a test can answer.
pub fn check_with(project: &Project, flows: &dyn Flows, sakai: &dyn Maps, ports: &Ports, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    // the flows that give a secret to a file of the project; a flow that does not pass dandori's
    // check is dandori's to say
    let sending: Vec<(&File, Vec<Send>)> = project
        .of(ritsu_base::naming::Tool::Dandori)
        .into_iter()
        .filter_map(|f| {
            let sends = flows.sends(&ritsu_base::paths::on_disk(&project.root, &f.rel), ports).ok()?;
            (!sends.is_empty()).then_some((f, sends))
        })
        .collect();
    if sending.is_empty() {
        return Vec::new();
    }
    let maps = maps(project, sakai);
    let from_root = |p: &Path| ritsu_base::paths::from_root(&project.root, p);
    let mut out = Vec::new();
    for (f, sends) in &sending {
        let disk = ritsu_base::paths::on_disk(&project.root, &f.rel);
        let src = ritsu_base::fs::read_to_string(&disk).unwrap_or_default();
        // the first map, in the order of the paths, that holds the flow; else what a map that
        // cannot answer says
        let mut judged: Option<(&Map, Vec<Call>)> = None;
        let mut cannot: Option<(&Map, Vec<Said>)> = None;
        for m in &maps {
            match &m.facts {
                Ok(facts) => {
                    let context_of = |file: &str| sakai.context_of(&project.root, &m.rel, file);
                    match judge(&f.rel, sends, facts, &context_of, &from_root) {
                        Ok(Some(calls)) => {
                            judged = Some((m, calls));
                            break;
                        }
                        Ok(None) => {}
                        Err(said) => {
                            cannot.get_or_insert((m, said));
                        }
                    }
                }
                Err(said) => {
                    cannot.get_or_insert((m, said.clone()));
                }
            }
        }
        match (judged, cannot) {
            (Some((m, calls)), _) => {
                let facts = m.facts.as_ref().expect("a map that answered");
                for c in &calls {
                    c.count(borders);
                    for j in &c.secrets {
                        if let Some(d) = said(project, f, &src, &m.rel, facts, c, j) {
                            out.push(Finding::of(&d, Some(f.rel.clone()), lang));
                        }
                    }
                }
            }
            (None, Some((m, said))) => {
                for s in sends {
                    borders.undecided += 1;
                    for (_, secret) in &s.secrets {
                        out.push(Finding::of(&undecided(f, &src, &m.rel, s, secret, &said), Some(f.rel.clone()), lang));
                    }
                }
            }
            // no map holds the flow: the maps say nothing of it
            (None, None) => {}
        }
    }
    out
}

/// What is said of one secret a call gives: E905 when it fails, W905 when it is undecided, nothing
/// when it holds.
fn said(project: &Project, f: &File, src: &str, map: &str, facts: &MapFacts, c: &Call, j: &Judged) -> Option<Diag> {
    let s = &c.send;
    let who = caller(s, c.to.as_deref());
    let secret = &j.secret.shown;
    let param = &j.param;
    let fix = |relationship: bool| {
        if relationship {
            tr!(
                "値の代わりに参照を送るか、地図に関係を足すか、そこへ送ることを意図しているなら、タスクの下に `discloses {param} \"<理由>\"` と書いてください。",
                "Send a reference instead, add the relationship to the map, or, if sending it there is intended, write `discloses {param} \"<why>\"` under the task."
            )
        } else {
            tr!(
                "値の代わりに参照を送るか、そのファイルを地図のコンテキストに入れるか（`owns`）、そこへ送ることを意図しているなら、タスクの下に `discloses {param} \"<理由>\"` と書いてください。",
                "Send a reference instead, give the file to a context of the map (`owns`), or, if sending it there is intended, write `discloses {param} \"<why>\"` under the task."
            )
        }
    };
    match &j.outcome {
        Outcome::Same | Outcome::Related(_) | Outcome::Disclosed(_) => None,
        Outcome::Undecided(said) => Some(undecided(f, src, map, s, &j.secret, said)),
        Outcome::Outside => {
            let by = j.marked_by.clone().unwrap_or_default();
            let to = c.to.clone().unwrap_or_else(|| s_path(project, s));
            Some(
                Diag::at(
                    "E905",
                    &f.shown,
                    s.line,
                    0,
                    tr!(
                        "{}、「{by}」が印を付けた秘密の値 `{secret}` を、地図 {map} のどのコンテキストにも属さない \"{to}\" に送ります",
                        "{} sends the secret `{secret}`, marked by {by}, to \"{to}\", which no context of the map {map} holds",
                        then_ja(&who.ja, "が"); capitalize(&who.en)
                    ),
                )
                .source(src)
                .rel(&f.rel)
                .note(mark_note(project, &j.secret))
                .note(fix(false)),
            )
        }
        Outcome::Unrelated => {
            let by = j.marked_by.clone().unwrap_or_default();
            let at = j.sent_to.clone().unwrap_or_default();
            let others: Vec<String> = facts
                .relationships
                .iter()
                .filter(|r| !r.separate && (r.from == at || r.to == at))
                .map(|r| if r.from == at { r.to.clone() } else { r.from.clone() })
                .fold(Vec::new(), |mut v, c| {
                    if !v.contains(&c) {
                        v.push(c);
                    }
                    v
                });
            // `separate ways` between the two is written in the map, and says they have nothing to do
            // with each other
            let apart = facts.relationships.iter().any(|r| r.separate && ((r.from == at && r.to == by) || (r.from == by && r.to == at)));
            let relates = if others.is_empty() && apart {
                tr!(
                    "地図 {map} は、「{at}」をどのコンテキストとも関係づけていません（「{by}」とは `separate ways` で、関係が無いことを書いています）。",
                    "The map {map} relates {at} with no other context (with {by}, it writes `separate ways`: the two have nothing to do with each other)."
                )
            } else if others.is_empty() {
                tr!("地図 {map} は、「{at}」をどのコンテキストとも関係づけていません。", "The map {map} relates {at} with no other context.")
            } else {
                let ja: Vec<String> = others.iter().map(|o| format!("「{o}」")).collect();
                let list = Text::list(&others.iter().map(|o| Text::same(o.clone())).collect::<Vec<_>>());
                tr!("地図 {map} は、「{at}」を{}とだけ関係づけています。", "The map {map} relates {at} with {} only.", ja.join("、"); list.en)
            };
            Some(
                Diag::at(
                    "E905",
                    &f.shown,
                    s.line,
                    0,
                    tr!(
                        "{}、「{by}」が印を付けた秘密の値 `{secret}` を、「{by}」と関係の無い「{at}」に送ります",
                        "{} sends the secret `{secret}`, marked by {by}, to {at}, which has no relationship with {by}",
                        then_ja(&who.ja, "が"); capitalize(&who.en)
                    ),
                )
                .source(src)
                .rel(&f.rel)
                .note(mark_note(project, &j.secret))
                .note(relates)
                .note(fix(true)),
            )
        }
    }
}

/// W905: where a call sends a secret cannot be decided, with what sakai says.
fn undecided(f: &File, src: &str, map: &str, s: &Send, secret: &Secret, said: &[Said]) -> Diag {
    let who = caller(s, None);
    let shown = &secret.shown;
    let why = match said.first() {
        Some(x) if !x.code.is_empty() => tr!("sakai の {}: {}", "sakai's {}: {}", x.code, x.message.ja; x.code, x.message.en),
        Some(x) => x.message.clone(),
        None => Text::default(),
    };
    let mut d: Diag = Diag::at(
        "W905",
        &f.shown,
        s.line,
        0,
        tr!(
            "{}秘密の値 `{shown}` を送る先が、地図 {map} のどこかを決められません",
            "Where in the map {map} {} sends the secret `{shown}` cannot be decided",
            then_ja(&who.ja, "が"); who.en
        ),
    )
    .source(src)
    .rel(&f.rel);
    if !why.is_empty() {
        d = d.note(tr!("地図が sakai の検査を通らないので、ファイルがどのコンテキストに属するかが分かりません（{}）。", "The map does not pass sakai's check, so which context a file belongs to is not known ({}).", why.ja; why.en));
    }
    d.note(tr!("`sakai check` が通るよう地図を直してください。", "Correct the map so that `sakai check` passes."))
}

/// Who sends: a task, or the call of a rule or of a date.
fn caller(s: &Send, to: Option<&str>) -> Text {
    let task = &s.task;
    let ext = to.map(|t| t.rsplit('.').next().unwrap_or("")).or_else(|| {
        let Destination::File(p) = &s.to;
        p.extension().and_then(|e| e.to_str())
    });
    match ext {
        Some("rule") => tr!("規則 `{task}` の呼び出し", "the call of the rule `{task}`"),
        Some("cal") => tr!("`{task}` の呼び出し", "the call of `{task}`"),
        _ => tr!("タスク `{task}`", "the task `{task}`"),
    }
}

/// The destination as the project names it.
fn s_path(project: &Project, s: &Send) -> String {
    let Destination::File(p) = &s.to;
    named(project, p)
}

/// Where the mark is: the file (from the root) and its line, and the mark as the file writes it.
fn mark_note(project: &Project, secret: &Secret) -> Text {
    let file = named(project, &secret.marked_in);
    let at = if secret.line > 0 { format!("{file}:{}", secret.line) } else { file };
    let mark = &secret.mark;
    tr!("印は {at} の `{mark}` です。", "The mark is `{mark}` at {at}.")
}
