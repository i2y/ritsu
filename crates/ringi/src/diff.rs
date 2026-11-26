//! `ringi diff` (DESIGN 11.2, 11.4): what two versions of a file say and answer differently, in the
//! words of the pages, for the people who approve and manage the requests.
//!
//! - The route of a request: the lowered `route.rule` of each version, compared over every input
//!   the two take ([`ritsu_ports::Rules::diff`], what `rulec diff` finds), each box where the stages
//!   differ said again with the form's fields and the stages' ways to choose. With the
//!   organization's data, each requester whose approvers that box changes, version by version of
//!   the organization (`simulate` of every requester, without a form, on each side).
//! - Everything else the page of the file says ([`crate::doc::page`]), section by section: a row of
//!   a table by its first cell, a sentence by its words. What the page of a version says is what
//!   the version is, so the two say no more and no less than the pages.
//! - The approvers that change where the route does not: a way to choose written otherwise, or
//!   other data.
//! - How each reaches the tickets in progress (DESIGN 7.7, 8.7).
//!
//! `ringi diff --versions <date> <date> <file>` holds one request to two versions of the
//! organization: the requesters whose approvers change, stage by stage, who may submit it, and the
//! stages nobody answers.

use crate::ast::FileKind;
use crate::check::{Options, Outcome, Run};
use crate::doc::words::{self, Say};
use crate::doc::{Block, Page};
use crate::names::Scope;
use crate::org::Version;
use crate::simulate::{Answer, On, StageSim, WaySim};
use crate::suite::Suite;
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text};
use ritsu_ports::{DiffHeld, DiffSide};
use std::path::Path;
use std::rc::Rc;

/// How many requesters a line names before it counts the rest (the JSON holds them all).
pub const NAMED: usize = 10;

/// One side: the path the run reads, the files it reads it through (a module's store, a git
/// revision; None for the files as they are), and the file as the output names it.
#[derive(Clone)]
pub struct Side {
    pub read: String,
    pub files: Option<Rc<dyn ritsu_base::fs::Files>>,
    pub shown: String,
}

/// Why two files are not compared: a sentence, or a side whose check fails, as the output names it,
/// with how many errors it has and its diagnostics (in the language of the text).
#[derive(Debug)]
pub enum Refused {
    Said(Text),
    Check { shown: String, errors: usize, said: String },
}

fn within<T>(files: &Option<Rc<dyn ritsu_base::fs::Files>>, f: impl FnOnce() -> T) -> T {
    match files {
        Some(fs) => ritsu_base::fs::with(fs.clone(), f),
        None => f(),
    }
}

/// A side, read and checked, with its page in the language of the output.
struct Read {
    run: Run,
    o: Rc<Outcome>,
    side: Side,
    page: Page,
}

impl Read {
    fn file(&self) -> &crate::ast::File {
        self.o.file.as_ref().expect("a side that passes its check")
    }

    /// The first day and the last the request can be used on, as `check` says.
    fn usable(&self) -> (Option<i64>, Option<i64>) {
        match &self.o.usable {
            Some(u) => (u.from.as_ref().map(|f| f.day), u.through.as_ref().map(|t| t.day)),
            None => (None, None),
        }
    }

    /// `f` with the scope of the file, read through the side's files.
    fn scoped<T>(&self, f: impl FnOnce(&Scope) -> T) -> T {
        within(&self.side.files, || {
            let file = self.o.file.as_ref().expect("a side that passes its check");
            let src = ritsu_base::fs::read_to_string(&self.o.path).unwrap_or_default();
            let mut said = Vec::new();
            let got = crate::names::read_uses(file, &self.o.path, &src, &self.run, &mut said);
            let s = Scope { file, path: &self.o.path, src: &src, got };
            f(&s)
        })
    }

    /// The labels of the stages and the ways the answers name, as the page of the side says them.
    fn labels<'x>(&self, lang: Lang, answers: impl Iterator<Item = &'x Answer>) -> Labels {
        let (mut ways, mut stages): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
        for a in answers {
            for st in a.sims.iter().flat_map(|s| &s.stages) {
                if !stages.contains(&st.stage) {
                    stages.push(st.stage.clone());
                }
                let named = st.ways.iter().map(|w| w.way.clone()).chain((!st.shown.contains(' ')).then(|| st.shown.clone()));
                for w in named {
                    if !ways.contains(&w) {
                        ways.push(w);
                    }
                }
            }
        }
        self.scoped(|s| {
            let say = Say::new(s, lang);
            Labels {
                lang,
                names: crate::engine::Names::of(s),
                ways: ways.into_iter().map(|w| (say.way(&w), w)).map(|(l, w)| (w, l)).collect(),
                specs: specs(s, lang),
                stages: stages.into_iter().map(|st| (say.stage(&st), st)).map(|(l, st)| (st, l)).collect(),
            }
        })
    }

    /// `simulate` of every requester without a form, on the day: every stage the route can come to,
    /// and who answers it. None where the request cannot be used that day, or reads no data.
    fn sims(&self, day: i64) -> Option<Answer> {
        let ask = crate::simulate::Ask { requester: None, every: true, form: None, on: On::Day(day) };
        within(&self.side.files, || crate::simulate::simulate(&self.o, &self.run, &ask).ok())
    }
}

fn read(side: &Side, suite: &Suite, opts: &Options, lang: Lang, made: Lang) -> Result<Read, Refused> {
    within(&side.files, || {
        let run = Run::with(suite.clone(), opts.clone());
        run.set_lang(lang);
        let f = &side.shown;
        let o = run.file(&side.read).map_err(|e| Refused::Said(ritsu_base::cli::cannot_read(f, &e)))?;
        if o.file.is_none() || o.has_errors() {
            let errors = o.diags.iter().filter(|d| d.is_error()).count();
            return Err(Refused::Check { shown: f.clone(), errors, said: crate::check::render(&o, lang) });
        }
        let page = crate::doc::page(&o, &run, &side.shown, made, &crate::doc::Ask::default()).map_err(Refused::Said)?;
        Ok(Read { run, o, side: side.clone(), page })
    })
}

// ── What changes ────────────────────────────────────────────────────────

/// What two versions of a file say and answer differently.
pub struct Diff {
    pub lang: Lang,
    pub kind: FileKind,
    pub name: String,
    /// The title of the new version, as its page writes it.
    pub title: String,
    pub old: Head,
    pub new: Head,
    /// The boxes of the route's inputs where the stages differ.
    pub route: Vec<Region>,
    /// The boxes the comparison of the route could not settle, each with why.
    pub unsettled: Vec<(String, String)>,
    /// Whether "every other input comes to the same stages" was earned.
    pub total: bool,
    pub sections: Vec<Section>,
    /// The approvers that change where the route does not, version by version of the organization.
    pub approvers: Vec<Group>,
    /// Whether those come from the data the request reads alone: its ways to choose say the same.
    pub from_data: bool,
    /// Why no approver is named: the request reads no data of the organization.
    pub no_people: Option<String>,
    pub in_progress: Vec<String>,
}

/// One side, as the output names it.
pub struct Head {
    pub file: String,
    pub version: u32,
    pub sha: String,
}

/// A box of the route's inputs where the stages differ.
pub struct Region {
    /// What the box holds of each input it narrows, by the lowered rule's name.
    pub held: Vec<(String, DiffHeld)>,
    pub case: Vec<Held>,
    pub stages: Vec<StageChange>,
    pub text: String,
    /// One input of the box, as the lowered rule names it.
    pub example: Vec<(String, String)>,
    pub people: Vec<Group>,
}

/// What a box holds of one input: its name, its label (the field's), and the values in words.
pub struct Held {
    pub name: String,
    pub label: String,
    pub is: String,
}

/// A stage a box answers otherwise: the stage as written and in ASCII (DESIGN 3.4), its label, and
/// how it is written before and after (None: it does not come), as written, in ASCII and in words.
pub struct StageChange {
    pub stage: String,
    pub stage_ascii: String,
    pub label: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub before_ascii: Option<String>,
    pub after_ascii: Option<String>,
    pub before_words: Option<String>,
    pub after_words: Option<String>,
}

/// The requesters whose approvers change, in one version of the organization.
pub struct Group {
    /// The first day of the version.
    pub from: String,
    /// How many requesters were compared.
    pub of: usize,
    pub changes: Vec<Person>,
}

/// One requester's stage, answered otherwise: the stage as the file names it, how it is written
/// where the stage is written more than one way (None in a box of the route, which says it), the
/// stage in the words of the page, and who answers it before and after, in words.
#[derive(Clone)]
pub struct Person {
    pub id: String,
    pub by: Option<String>,
    pub named: String,
    pub stage: String,
    pub shown: Option<String>,
    /// The stage and how it is written, in ASCII (DESIGN 3.4).
    pub stage_ascii: String,
    pub shown_ascii: Option<String>,
    pub label: String,
    pub before: String,
    pub after: String,
}

/// A section of the page that says something else.
pub struct Section {
    pub id: String,
    pub title: String,
    pub changes: Vec<Change>,
}

/// One thing a section says otherwise: what (a row's first cell and column, the heading above it),
/// and the words before and after (None: not there).
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub item: Option<String>,
    pub before: Option<String>,
    pub after: Option<String>,
}

impl Diff {
    /// Whether anything differs (`diff` exits 1): a box of the route, a section, an approver.
    pub fn differs(&self) -> bool {
        !self.route.is_empty() || !self.sections.is_empty() || self.approvers.iter().any(|g| !g.changes.is_empty())
    }

    /// How many things differ: the boxes of the route, the changes of the sections, and the
    /// requesters' stages answered otherwise where the route does not change.
    pub fn count(&self) -> usize {
        self.route.len() + self.sections.iter().map(|s| s.changes.len()).sum::<usize>() + self.approvers.iter().map(|g| g.changes.len()).sum::<usize>()
    }
}

/// `ringi diff <old> <new>`: the two files, each read through its own files, compared in the
/// language `made` (the text of the run in `lang`), the route's walk held to `budget`.
pub fn diff(old: &Side, new: &Side, suite: &Suite, opts: &Options, budget: Option<usize>, lang: Lang, made: Lang) -> Result<Diff, Refused> {
    let o = read(old, suite, opts, lang, made)?;
    let n = read(new, suite, opts, lang, made)?;
    let (of, nf) = (o.file(), n.file());
    if of.kind != nf.kind {
        let (a, b) = (kind_words(of.kind), kind_words(nf.kind));
        return Err(Refused::Said(tr!(
            "二つのファイルの種類が違います（{}と{}）。同じファイルの二つの版を渡してください",
            "the two files are of different kinds ({} and {}): give two versions of one file",
            a.ja, b.ja; a.en, b.en
        )));
    }
    if of.name.ascii() != nf.name.ascii() {
        let (a, b) = (of.name.text.clone(), nf.name.text.clone());
        return Err(Refused::Said(tr!(
            "二つのファイルの名前が違います（`{a}` と `{b}`）。同じファイルの二つの版を渡してください",
            "the two files have different names (`{a}` and `{b}`): give two versions of one file"
        )));
    }
    let say_t = |en: &str, ja: &str| if made == Lang::Ja { ja.to_string() } else { en.to_string() };
    // the route, compared as the lowered rules answer
    let mut route = Vec::new();
    let mut unsettled = Vec::new();
    let mut total = true;
    // the routes compared case by case could not be given the same requests: the route's lines are
    // compared as written instead
    let mut apart = false;
    let routes = alike(lowered_route(&o), lowered_route(&n));
    let compared = match routes {
        (Some(a), Some(b)) if nf.kind == FileKind::Request => {
            let Some(rules) = n.run.suite.rules.clone() else {
                return Err(Refused::Said(tr!("rulec がつながっていないので、経路を比べられません", "rulec is not joined: the routes cannot be compared")));
            };
            let found = rules.diff(&a.0, &b.0, budget).map_err(|said| {
                let first = said.first().map(|s| s.message.clone()).unwrap_or_default();
                Refused::Said(tr!("経路を比べられません: {}", "the routes cannot be compared: {}", first.ja; first.en))
            })?;
            match found {
                ritsu_ports::Found::Value(d) => {
                    total = d.total();
                    let names = (a.1, b.1);
                    let (old_specs, new_specs) = (o.scoped(|s| specs(s, made)), n.scoped(|s| specs(s, made)));
                    let ascii = (o.scoped(crate::engine::Names::of), n.scoped(crate::engine::Names::of));
                    n.scoped(|s| {
                        let say = Say::new(s, made);
                        for (held, moves, witness) in boxes(&say, &d.columns, &d.changes, &names) {
                            route.push(region(&say, &d.columns, &held, moves, &witness, &names, (&old_specs, &new_specs), &ascii));
                        }
                        for u in &d.unknown {
                            unsettled.push((case_text(&say, &held_of(&say, &d.columns, &u.held, &names.1, &names.0)), u.why.get(made).to_string()));
                        }
                        if d.blocked.is_some() {
                            apart = true;
                            unsettled.push((say_t("Every request", "どの申請も"), reads_apart(&d.declared, &names, made)));
                        }
                    });
                    // a stage only the new version has: rulec's diff gives an output only the old rule
                    // has in its boxes (its new value `-`), and one only the new rule has among what the
                    // two declare differently alone; its boxes are the diff's the other way round
                    let added: Vec<&str> = d.declared.iter().filter(|x| x.what == "output_added").map(|x| x.name.as_str()).collect();
                    if !added.is_empty() {
                        let back = rules.diff(&b.0, &a.0, budget).map_err(|said| {
                            let first = said.first().map(|s| s.message.clone()).unwrap_or_default();
                            Refused::Said(tr!("経路を比べられません: {}", "the routes cannot be compared: {}", first.ja; first.en))
                        })?;
                        match back {
                            ritsu_ports::Found::Value(r) => {
                                total = total && r.total();
                                let turned: Vec<ritsu_ports::DiffChange> = r.changes.iter().map(|c| turned_back(c, &added)).collect();
                                let (old_specs, new_specs) = (o.scoped(|s| specs(s, made)), n.scoped(|s| specs(s, made)));
                                let ascii = (o.scoped(crate::engine::Names::of), n.scoped(crate::engine::Names::of));
                                n.scoped(|s| {
                                    let say = Say::new(s, made);
                                    for (held, moves, witness) in boxes(&say, &r.columns, &turned, &names) {
                                        route.push(region(&say, &r.columns, &held, moves, &witness, &names, (&old_specs, &new_specs), &ascii));
                                    }
                                    // what the walk this way could not settle, where the other could
                                    if d.unknown.is_empty() && d.blocked.is_none() {
                                        for u in &r.unknown {
                                            unsettled.push((case_text(&say, &held_of(&say, &r.columns, &u.held, &names.1, &names.0)), u.why.get(made).to_string()));
                                        }
                                        if let Some(b) = &r.blocked {
                                            unsettled.push((say_t("Every request", "どの申請も"), b.get(made).to_string()));
                                        }
                                    }
                                });
                            }
                            ritsu_ports::Found::Undecided(t) => {
                                total = false;
                                unsettled.push((say_t("Every request", "どの申請も"), t.get(made).to_string()));
                            }
                        }
                    }
                    true
                }
                ritsu_ports::Found::Undecided(t) => {
                    total = false;
                    unsettled.push((say_t("Every request", "どの申請も"), t.get(made).to_string()));
                    true
                }
            }
        }
        _ => false,
    };
    // what the pages say otherwise
    let skip: &[&str] = if compared && !apart { &["route", "examples", "scenarios"] } else { &["examples", "scenarios"] };
    let mut sections = sections_of(&o.page, &n.page, skip, made);
    sections.extend(calendar_sections(&o, &n, made)?);
    // the approvers, version by version of the organization
    let mut approvers = Vec::new();
    let mut no_people = None;
    let from_data = !sections.iter().any(|s| s.id == "ways");
    if nf.kind == FileKind::Request {
        let days = days(&o, &n);
        if days.is_empty() {
            no_people = Some(n.scoped(|s| no_org_line(s, made)).unwrap_or_else(|| say_t("No data of the organization is read: no approver is named.", "組織のデータを読んでいないので、承認者を名前では言いません。")));
        }
        let sims: Vec<(Answer, Answer)> = days.into_iter().filter_map(|day| Some((o.sims(day)?, n.sims(day)?))).collect();
        let (lo, ln) = (o.labels(made, sims.iter().map(|x| &x.0)), n.labels(made, sims.iter().map(|x| &x.1)));
        for (a, b) in &sims {
            let Some(v) = b.org_version() else { continue };
            let vo = a.org_version().unwrap_or(v);
            let from = v.day();
            for r in route.iter_mut() {
                let g = region_people(&r.stages, a, vo, b, v, &lo, &ln, &from);
                if !g.changes.is_empty() {
                    r.people.push(g);
                }
            }
            let g = way_people(a, vo, b, v, &lo, &ln, &from);
            if !g.changes.is_empty() {
                approvers.push(g);
            }
        }
    }
    // the title and the description, and the head of the page but its own line
    let mut head = Vec::new();
    if o.page.title != n.page.title {
        head.push(Change { item: Some(say_t("Title", "題")), before: Some(o.page.title.clone()), after: Some(n.page.title.clone()) });
    }
    if o.page.description != n.page.description {
        head.push(Change { item: Some(say_t("Description", "説明")), before: o.page.description.clone(), after: n.page.description.clone() });
    }
    if !head.is_empty() {
        let at = 0;
        match sections.iter_mut().find(|s| s.id == "head") {
            Some(s) => {
                for (i, c) in head.into_iter().enumerate() {
                    s.changes.insert(at + i, c);
                }
            }
            None => sections.insert(0, Section { id: "head".to_string(), title: say_t("The file", "ファイル"), changes: head }),
        }
    }
    let mut d = Diff {
        lang: made,
        kind: nf.kind,
        name: nf.name.text.clone(),
        title: n.page.title.clone(),
        old: Head { file: old.shown.clone(), version: of.version, sha: o.page.sha.clone() },
        new: Head { file: new.shown.clone(), version: nf.version, sha: n.page.sha.clone() },
        route,
        unsettled,
        total,
        sections,
        approvers,
        from_data,
        no_people,
        in_progress: Vec::new(),
    };
    d.in_progress = in_progress(&d, nf.effective_from.map(|(day, _)| day), made);
    Ok(d)
}

/// The calendars the two requests count their deadlines by, compared as koyomi compares two
/// versions of one calendar. A calendar whose answer changes is one section of the page; without
/// it, a request whose own lines do not change would read as unchanged while its dates move.
fn calendar_sections(o: &Read, n: &Read, made: Lang) -> Result<Vec<Section>, Refused> {
    use crate::ast::UseKind;
    let mut out = Vec::new();
    let Some(dates) = o.run.suite.dates.clone() else { return Ok(out) };
    for ou in o.file().uses.iter().filter(|u| u.kind == UseKind::Calendar) {
        let Some(nu) = n.file().uses.iter().find(|u| u.kind == UseKind::Calendar && u.alias.as_ref().map(|a| a.ascii()) == ou.alias.as_ref().map(|a| a.ascii()) && u.path == ou.path) else { continue };
        let (op, np) = match (crate::names::beside(&o.side.read, &ou.path), crate::names::beside(&n.side.read, &nu.path)) {
            (Ok(a), Ok(b)) => (a, b),
            _ => continue,
        };
        let read = |side: &Read, path: &Path| -> Result<ritsu_ports::CalendarFacts, Refused> {
            within(&side.side.files, || dates.calendar(path)).map_err(|said| {
                let first = said.first().map(|x| x.message.clone()).unwrap_or_default();
                Refused::Said(first)
            })
        };
        let (a, b) = (read(o, &op)?, read(n, &np)?);
        if a == b {
            continue;
        }
        let mut changes = Vec::new();
        if a.data != b.data {
            let range = |x: (i64, i64)| {
                if made == Lang::Ja { format!("{} から {}", ritsu_ports::day_text(x.0), ritsu_ports::day_text(x.1)) } else { format!("{} to {}", ritsu_ports::day_text(x.0), ritsu_ports::day_text(x.1)) }
            };
            changes.push(Change { item: Some(if made == Lang::Ja { "データの範囲".into() } else { "The days the calendar knows".into() }), before: Some(range(a.data)), after: Some(range(b.data)) });
        }
        if a.offset != b.offset {
            let off = |x: Option<i32>| x.map(|m| format!("{m:+}")).unwrap_or_else(|| "UTC".into());
            changes.push(Change { item: Some(if made == Lang::Ja { "UTC との差".into() } else { "UTC offset".into() }), before: Some(off(a.offset)), after: Some(off(b.offset)) });
        }
        let opened: Vec<_> = a.closed.difference(&b.closed).collect();
        let closed: Vec<_> = b.closed.difference(&a.closed).collect();
        if !closed.is_empty() || !opened.is_empty() {
            let days = |xs: Vec<&i64>| xs.into_iter().take(20).map(|d| ritsu_ports::day_text(*d)).collect::<Vec<_>>().join(", ");
            let more_closed = closed.len().saturating_sub(20);
            let more_opened = opened.len().saturating_sub(20);
            changes.push(Change {
                item: Some(if made == Lang::Ja { "休みの日".into() } else { "Closed days".into() }),
                before: Some(if opened.is_empty() { String::new() } else { format!("{}{}", days(opened), if more_opened > 0 { format!(" +{more_opened}") } else { String::new() }) }),
                after: Some(if closed.is_empty() { String::new() } else { format!("{}{}", days(closed), if more_closed > 0 { format!(" +{more_closed}") } else { String::new() }) }),
            });
        }
        if changes.is_empty() {
            changes.push(Change { item: None, before: None, after: Some(if made == Lang::Ja { "カレンダーが変わります".into() } else { "The calendar changes".into() }) });
        }
        out.push(Section {
            id: format!("calendar-{}", ou.alias.as_ref().map(|a| a.ascii().to_string()).unwrap_or_else(|| ou.path.replace('/', "_"))),
            title: if made == Lang::Ja { format!("カレンダー `{}`", ou.path) } else { format!("The calendar `{}`", ou.path) },
            changes,
        });
    }
    Ok(out)
}

/// The lowered route of a request, as a side of [`ritsu_ports::Rules::diff`] (read through the
/// lowered files laid over the side's), with what it names things by.
fn lowered_route(r: &Read) -> Option<(DiffSide, crate::lower::RouteNames, String)> {
    let l = r.o.lowered.as_ref()?;
    let f = l.files.iter().find(|f| f.part == crate::lower::Part::Route)?;
    let laid = within(&r.side.files, || crate::engine::eval::overlay(l));
    Some((DiffSide::new(l.path(f), Some(laid)), l.route.clone().unwrap_or_default(), f.text.clone()))
}

type Route = (DiffSide, crate::lower::RouteNames);

/// The two lowered routes, each declaring the inputs the other reads: a version whose route reads a
/// field the other's does not (a stage made conditional on the amount) would leave rulec no request
/// to give both, and nothing compared. An input a route does not read changes none of its answers,
/// so the route that does not read it is declared to take it, and the two are compared case by
/// case. An input typed with an enum of the other route's is not declared (the route has no such
/// enum): rulec then says the two take different inputs ([`reads_apart`]).
fn alike(o: Option<(DiffSide, crate::lower::RouteNames, String)>, n: Option<(DiffSide, crate::lower::RouteNames, String)>) -> (Option<Route>, Option<Route>) {
    let (Some((os, on, ot)), Some((ns, nn, nt))) = (o.clone(), n.clone()) else {
        return (o.map(|(s, r, _)| (s, r)), n.map(|(s, r, _)| (s, r)));
    };
    let (oi, ni) = (input_lines(&ot), input_lines(&nt));
    let missing = |have: &[(String, String)], other: &[(String, String)], other_text: &str| -> Vec<String> {
        other
            .iter()
            .filter(|(name, _)| !have.iter().any(|(h, _)| h == name))
            .filter(|(_, line)| !line.contains(" fed ") && !enum_typed(line, other_text))
            .map(|(_, line)| line.clone())
            .collect()
    };
    let laid = |side: DiffSide, text: &str, add: Vec<String>| -> DiffSide {
        if add.is_empty() {
            return side;
        }
        let mut ov = ritsu_base::fs::Overlay::new(side.files.clone());
        ov.put(&ritsu_base::paths::opened(&side.rule), with_inputs(text, &add));
        DiffSide::new(side.rule.clone(), Some(Rc::new(ov)))
    };
    let (to_old, to_new) = (missing(&oi, &ni, &nt), missing(&ni, &oi, &ot));
    (Some((laid(os, &ot, to_old), on)), Some((laid(ns, &nt, to_new), nn)))
}

/// The lines under `inputs` of a lowered rule, each with the name it declares.
fn input_lines(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if line == "inputs" {
            inside = true;
        } else if inside && line.starts_with("  ") {
            if let Some((name, _)) = line.split_once(" : ") {
                out.push((name.trim().to_string(), line.to_string()));
            }
        } else {
            inside = false;
        }
    }
    out
}

/// Whether an input's line types it with an enum the rule declares or imports.
fn enum_typed(line: &str, text: &str) -> bool {
    let ty = line.split_once(" : ").map(|(_, t)| t.split_whitespace().next().unwrap_or("").trim_end_matches('?')).unwrap_or("");
    text.lines().any(|l| {
        let l = l.trim_start();
        l.strip_prefix("enum ").or_else(|| l.strip_prefix("import enum ")).is_some_and(|rest| rest.split([' ', '=']).next() == Some(ty))
    })
}

/// A lowered rule's text with the input lines `add` declared too: under its `inputs`, or in a new
/// `inputs` before its `outputs`.
fn with_inputs(text: &str, add: &[String]) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    if let Some(i) = lines.iter().position(|l| l == "inputs") {
        let mut j = i + 1;
        while j < lines.len() && lines[j].starts_with("  ") {
            j += 1;
        }
        lines.splice(j..j, add.iter().cloned());
    } else if let Some(i) = lines.iter().position(|l| l == "outputs") {
        let mut put = vec!["inputs".to_string()];
        put.extend(add.iter().cloned());
        put.push(String::new());
        lines.splice(i..i, put);
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// What is said of two routes that read different inputs, which no request can be given both of
/// ([`alike`] could not declare them alike): the fields each reads that the other does not, by what
/// the route reads in the request, and that the route's lines are compared as written instead.
fn reads_apart(declared: &[ritsu_ports::DiffDeclared], names: &(crate::lower::RouteNames, crate::lower::RouteNames), lang: Lang) -> String {
    let field = |name: &str| -> String {
        let reads = names.1.inputs.iter().chain(&names.0.inputs).find(|(_, n, _)| n == name).map(|(r, _, _)| r.join(".")).unwrap_or_else(|| name.to_string());
        format!("`{reads}`")
    };
    let of = |what: &str| -> Vec<String> { declared.iter().filter(|x| x.what == what).map(|x| field(&x.name)).collect() };
    let (added, removed) = (of("input_added"), of("input_removed"));
    let sep = if lang == Lang::Ja { "、" } else { ", " };
    let mut parts = Vec::new();
    if !added.is_empty() {
        let list = added.join(sep);
        parts.push(if lang == Lang::Ja { format!("新しい版の経路だけが {list} を読みます") } else { format!("only the new version's route reads {list}") });
    }
    if !removed.is_empty() {
        let list = removed.join(sep);
        parts.push(if lang == Lang::Ja { format!("前の版の経路だけが {list} を読みます") } else { format!("only the old version's route reads {list}") });
    }
    if lang == Lang::Ja {
        let why = if parts.is_empty() { "二つの版の経路は、読む値が違います".to_string() } else { parts.join("。") };
        format!("{why}。同じ申請を両方に渡せないので、経路は場合ごとには比べず、書いてある行どうしを比べます")
    } else {
        let why = if parts.is_empty() { "the two versions' routes read different values".to_string() } else { parts.join("; ") };
        format!("{why}: no request can be given to both, so the route is not compared case by case, but line by line as written")
    }
}

/// Each stage and each way it can be answered by, as written, with the way in words: the stages of
/// every path of the route.
fn specs(s: &Scope, lang: Lang) -> Vec<(String, String, String, String)> {
    let say = Say::new(s, lang);
    let mut out: Vec<(String, String, String, String)> = Vec::new();
    for o in crate::org::listing::paths(s).iter().flatten() {
        let shown = o.spec.shown();
        if !out.iter().any(|(st, sh, _, _)| *st == o.stage && *sh == shown) {
            out.push((o.stage.clone(), shown, say.spec(o.spec), say.stage(&o.stage)));
        }
    }
    out
}

/// What a box holds of each input it narrows, in words: the inputs as the route reads them in the
/// request, by the names of `names` (the new version's), else of `older`.
fn held_of(say: &Say, columns: &[ritsu_ports::DiffColumn], held: &[DiffHeld], names: &crate::lower::RouteNames, older: &crate::lower::RouteNames) -> Vec<Held> {
    let mut out = Vec::new();
    for (col, h) in columns.iter().zip(held) {
        if matches!(h, DiffHeld::Any) {
            continue;
        }
        let reads = names.inputs.iter().chain(&older.inputs).find(|(_, n, _)| *n == col.name).map(|(r, _, _)| r.clone()).unwrap_or_else(|| vec![col.name.clone()]);
        let path: Vec<&str> = reads.iter().map(String::as_str).collect();
        let field = words::find_field(&say.s.file.form, &path);
        let label = match (&field, path.as_slice()) {
            (Some(_), _) => say.field(&path),
            (None, [r, o]) => say.t(format!("the answer of the rule `{r}` in `{o}`"), format!("規則 `{r}` の `{o}` の答え")),
            (None, _) => format!("`{}`", col.name),
        };
        let q = |x: &str| say.t(format!("“{x}”"), format!("「{x}」"));
        let is = match h {
            DiffHeld::Any => continue,
            DiffHeld::Words { words: ws, other } => {
                let mut vals: Vec<String> = ws
                    .iter()
                    .map(|w| match w.as_str() {
                        "true" => say.t("yes", "はい"),
                        "false" => say.t("no", "いいえ"),
                        _ if field.is_some() => say.value(&path, w),
                        _ => w.clone(),
                    })
                    .map(|w| q(&w))
                    .collect();
                if *other {
                    vals.push(say.t("another value", "ほかの値"));
                }
                if say.ja() { vals.join("か") } else { words::or_list(&vals) }
            }
            DiffHeld::Runs(runs) => {
                let ty = field.map(|f| f.ty.clone());
                let one = |(lo, hi): &(Option<ritsu_units::Rat>, Option<ritsu_units::Rat>)| match &ty {
                    Some(t) => say.span(t, *lo, *hi),
                    None => say.between(lo.map(|r| crate::lex::decimal(r)), hi.map(|r| crate::lex::decimal(r))),
                };
                let parts: Vec<String> = runs.iter().map(one).collect();
                if say.ja() { parts.join("か、") } else { words::or_list(&parts) }
            }
        };
        out.push(Held { name: col.name.clone(), label, is });
    }
    out
}

/// The requests a box holds, in words.
fn case_text(say: &Say, held: &[Held]) -> String {
    if held.is_empty() {
        return say.t("Every request", "どの申請も");
    }
    if say.ja() {
        let parts: Vec<String> = held.iter().map(|h| format!("「{}」が{}", h.label, h.is)).collect();
        format!("{}の申請", parts.join("で、"))
    } else {
        let parts: Vec<String> = held.iter().map(|h| format!("“{}” is {}", h.label, h.is)).collect();
        format!("Requests where {}", parts.join(" and "))
    }
}

/// The stages a box answers otherwise, each with how it is written before and after (None: it does
/// not come): an output's value, as the stage's way it stands for. An output a version does not
/// have (`-`, a stage taken out or put in) is a stage that does not come, as `none` is.
type Moves = Vec<(String, Option<String>, Option<String>)>;

fn moves(outputs: &[(String, String, String)], names: &(crate::lower::RouteNames, crate::lower::RouteNames)) -> Moves {
    let mut out = Vec::new();
    for (o, before, after) in outputs {
        let stage_of = |n: &crate::lower::RouteNames| n.stages.iter().find(|(_, x, _)| x == o).map(|(st, _, _)| st.clone());
        let stage = stage_of(&names.1).or_else(|| stage_of(&names.0)).unwrap_or_else(|| o.clone());
        let shown = |n: &crate::lower::RouteNames, v: &str| -> Option<String> {
            let at = n.stages.iter().find(|(_, x, _)| x == o);
            if v == "none" || (v == "-" && at.is_none()) {
                return None;
            }
            Some(at.and_then(|(_, _, vs)| vs.iter().find(|(x, _)| x == v).map(|(_, s)| s.clone())).unwrap_or_else(|| v.to_string()))
        };
        let (b, a) = (shown(&names.0, before), shown(&names.1, after));
        if b != a {
            out.push((stage, b, a));
        }
    }
    out
}

/// What a column of the route reads in the form: the field, where it is one.
fn field_of<'a>(say: &Say<'_, 'a>, col: &ritsu_ports::DiffColumn, names: &(crate::lower::RouteNames, crate::lower::RouteNames)) -> Option<&'a crate::ast::Field> {
    let reads = names.1.inputs.iter().chain(&names.0.inputs).find(|(_, n, _)| *n == col.name).map(|(r, _, _)| r.clone())?;
    let path: Vec<&str> = reads.iter().map(String::as_str).collect();
    words::find_field(&say.s.file.form, &path)
}

/// The union of what two boxes hold of one column: the words of both, the runs of both, a run
/// joined to the next where no value is between them (the field's step says which are next to each
/// other); every value where that is all the field has.
fn union(a: &DiffHeld, b: &DiffHeld, field: Option<&crate::ast::Field>, scope: &Scope) -> DiffHeld {
    match (a, b) {
        (DiffHeld::Words { words: x, other: ox }, DiffHeld::Words { words: y, other: oy }) => {
            let mut ws = x.clone();
            for w in y {
                if !ws.contains(w) {
                    ws.push(w.clone());
                }
            }
            let every = field.is_some_and(|f| matches!(f.ty, crate::ast::TypeExpr::Choice) && f.options.iter().all(|o| ws.iter().any(|w| w == o.name.ascii())));
            if every && !(*ox || *oy) { DiffHeld::Any } else { DiffHeld::Words { words: ws, other: *ox || *oy } }
        }
        (DiffHeld::Runs(x), DiffHeld::Runs(y)) => {
            let step = field.and_then(|f| match &f.ty {
                crate::ast::TypeExpr::Num(u) => Some(crate::lower::words::step(u)),
                crate::ast::TypeExpr::Date => Some(ritsu_units::Rat::int(1)),
                _ => None,
            });
            let mut runs: Vec<(Option<ritsu_units::Rat>, Option<ritsu_units::Rat>)> = x.iter().chain(y).cloned().collect();
            runs.sort_by(|p, q| match (p.0, q.0) {
                (None, None) => std::cmp::Ordering::Equal,
                (None, _) => std::cmp::Ordering::Less,
                (_, None) => std::cmp::Ordering::Greater,
                (Some(l), Some(m)) => l.cmp_to(m),
            });
            let mut out: Vec<(Option<ritsu_units::Rat>, Option<ritsu_units::Rat>)> = Vec::new();
            for r in runs {
                if let Some(last) = out.last_mut() {
                    let next = match (last.1, r.0) {
                        (None, _) => true,
                        (Some(h), Some(l)) => l.cmp_to(h) != std::cmp::Ordering::Greater || step.is_some_and(|s| l == h.add(s)),
                        (Some(_), None) => false,
                    };
                    if next {
                        last.1 = match (last.1, r.1) {
                            (None, _) | (_, None) => None,
                            (Some(p), Some(q)) => Some(if q.cmp_to(p) == std::cmp::Ordering::Greater { q } else { p }),
                        };
                        continue;
                    }
                }
                out.push(r);
            }
            // one run from end to end of what the field can hold holds every request, as an open one
            // does: the case is not narrowed by it
            let (lo, hi) = field.map(|f| crate::lower::route::reach_or_range(scope, f)).unwrap_or((None, None));
            let every = |r: &(Option<ritsu_units::Rat>, Option<ritsu_units::Rat>)| {
                r.0.is_none_or(|l| lo.is_some_and(|x| l.cmp_to(x) != std::cmp::Ordering::Greater)) && r.1.is_none_or(|h| hi.is_some_and(|x| h.cmp_to(x) != std::cmp::Ordering::Less))
            };
            if out.len() == 1 && every(&out[0]) { DiffHeld::Any } else { DiffHeld::Runs(out) }
        }
        _ => DiffHeld::Any,
    }
}

/// The boxes where the stages differ, each with the stages it changes and one input of it: a box
/// whose outputs differ only where both mean the stage does not come is no change of the route, and
/// two boxes that change the same stages alike and hold the same of every column but one are one
/// box, the union of the two (the walk cuts where other stages, which do not change, differ).
fn boxes(say: &Say, columns: &[ritsu_ports::DiffColumn], changes: &[ritsu_ports::DiffChange], names: &(crate::lower::RouteNames, crate::lower::RouteNames)) -> Vec<(Vec<DiffHeld>, Moves, Vec<(String, String)>)> {
    let fields: Vec<Option<&crate::ast::Field>> = columns.iter().map(|c| field_of(say, c, names)).collect();
    let mut out: Vec<(Vec<DiffHeld>, Moves, Vec<(String, String)>, bool)> = changes.iter().map(|c| (c.held.clone(), moves(&c.outputs, names), c.witness.clone(), c.uniform)).filter(|(_, m, _, _)| !m.is_empty()).collect();
    'again: loop {
        for i in 0..out.len() {
            for j in i + 1..out.len() {
                let (a, b) = (&out[i], &out[j]);
                if !(a.3 && b.3) || a.1 != b.1 {
                    continue;
                }
                let differ: Vec<usize> = (0..columns.len()).filter(|&k| a.0[k] != b.0[k]).collect();
                if let [k] = differ[..] {
                    let joined = union(&a.0[k], &b.0[k], fields[k], say.s);
                    out[i].0[k] = joined;
                    out.remove(j);
                    continue 'again;
                }
            }
        }
        break;
    }
    out.into_iter().map(|(h, m, w, _)| (h, m, w)).collect()
}

/// A box of the diff of the new route against the old, turned back: of the outputs, those only the
/// new rule has (`added`, whose value there is `-` the other way round), each from none to its value.
fn turned_back(c: &ritsu_ports::DiffChange, added: &[&str]) -> ritsu_ports::DiffChange {
    ritsu_ports::DiffChange {
        held: c.held.clone(),
        outputs: c.outputs.iter().filter(|(o, _, _)| added.contains(&o.as_str())).map(|(o, x, y)| (o.clone(), y.clone(), x.clone())).collect(),
        uniform: c.uniform,
        old_rows: c.new_rows.clone(),
        new_rows: c.old_rows.clone(),
        witness: c.witness.clone(),
        cells: c.cells,
    }
}

/// A box where the stages differ, in words.
#[allow(clippy::too_many_arguments)]
fn region(
    say: &Say,
    columns: &[ritsu_ports::DiffColumn],
    held: &[DiffHeld],
    moves: Moves,
    witness: &[(String, String)],
    names: &(crate::lower::RouteNames, crate::lower::RouteNames),
    (old_specs, new_specs): (&[(String, String, String, String)], &[(String, String, String, String)]),
    ascii: &(crate::engine::Names, crate::engine::Names),
) -> Region {
    let case = held_of(say, columns, held, &names.1, &names.0);
    let mut stages = Vec::new();
    for (stage, b, a) in moves {
        let in_words = |specs: &[(String, String, String, String)], sh: &Option<String>| sh.as_ref().map(|sh| specs.iter().find(|(st, s, _, _)| *st == stage && s == sh).map(|(_, _, w, _)| w.clone()).unwrap_or_else(|| format!("`{sh}`")));
        let label = new_specs.iter().chain(old_specs).find(|(st, _, _, _)| *st == stage).map(|(_, _, _, l)| l.clone()).unwrap_or_else(|| say.stage(&stage));
        // the stage in ASCII as the version that has it names it
        let stage_ascii = if new_specs.iter().any(|(st, _, _, _)| *st == stage) { ascii.1.stage(&stage) } else { ascii.0.stage(&stage) }.to_string();
        stages.push(StageChange {
            before_words: in_words(old_specs, &b),
            after_words: in_words(new_specs, &a),
            before_ascii: b.as_ref().map(|x| ascii.0.spec(x)),
            after_ascii: a.as_ref().map(|x| ascii.1.spec(x)),
            stage_ascii,
            stage,
            label,
            before: b,
            after: a,
        });
    }
    let said: Vec<String> = stages
        .iter()
        .map(|c| {
            let st = &c.label;
            match (&c.before_words, &c.after_words) {
                (Some(b), None) => say.t(format!("the stage “{st}” ({b}) no longer comes"), format!("ステップ「{st}」（{b}）が来なくなります")),
                (None, Some(a)) => say.t(format!("the stage “{st}” ({a}) comes"), format!("ステップ「{st}」（{a}）が来るようになります")),
                (Some(b), Some(a)) => say.t(format!("the stage “{st}” is answered by {a} instead of {b}"), format!("ステップ「{st}」に答えるのが、{b}から{a}に変わります")),
                (None, None) => String::new(),
            }
        })
        .collect();
    let case_said = case_text(say, &case);
    let text = if say.ja() { format!("{case_said}：{}。", said.join("。")) } else { format!("{case_said}: {}.", said.join("; ")) };
    let narrowed = columns.iter().zip(held).filter(|(_, h)| !matches!(h, DiffHeld::Any)).map(|(c, h)| (c.name.clone(), h.clone())).collect();
    Region { held: narrowed, case, stages, text, example: witness.to_vec(), people: Vec::new() }
}

/// The days the approvers are compared on: the first day of each version of the organization
/// either side reads, moved up to the first day both can be used on, through the last.
fn days(o: &Read, n: &Read) -> Vec<i64> {
    let firsts = |r: &Read| r.scoped(|s| s.org_data().map(|org| org.versions.iter().map(|v| v.from).collect::<Vec<i64>>()).unwrap_or_default());
    let mut all = firsts(o);
    all.extend(firsts(n));
    if all.is_empty() {
        return all;
    }
    let ((fa, ta), (fb, tb)) = (o.usable(), n.usable());
    let from = fa.into_iter().chain(fb).max();
    let through = ta.into_iter().chain(tb).min();
    let mut out: Vec<i64> = all.into_iter().map(|d| from.map_or(d, |f| d.max(f))).filter(|d| through.is_none_or(|t| *d <= t)).collect();
    out.sort();
    out.dedup();
    out
}

/// The line a page says of the organization with no data, when the request reads one.
fn no_org_line(s: &Scope, lang: Lang) -> Option<String> {
    let d = s.org_rc()?;
    let crate::org::Data::NoOrg(none) = d.as_ref() else { return None };
    let line = none.line.get(lang).to_string();
    Some(if lang == Lang::Ja {
        format!("組織のデータを読んでいません。{line}。承認者を名前では言いません。")
    } else {
        format!("No data of the organization is read: {line}. No approver is named.")
    })
}

/// Who answers a stage, in words: each way, with the people it comes to; of a stage both sides
/// answer by the same ways, only the ways answered otherwise (`only`).
fn answered(v: &Version, st: &StageSim, only: Option<&[usize]>, l: &Labels) -> String {
    let lang = l.lang;
    let parts: Vec<String> = st.ways.iter().enumerate().filter(|(k, _)| only.is_none_or(|o| o.contains(k))).map(|(_, w)| crate::simulate::way_alone(v, &l.way_name(st, w), w).get(lang).to_string()).collect();
    parts.join(if lang == Lang::Ja { "／" } else { "; " })
}

/// The ways of a stage answered otherwise, where both sides answer it by the same ways in the same
/// order; None where they do not (every way is said then).
fn changed_ways(a: &StageSim, b: &StageSim) -> Option<Vec<usize>> {
    let same = a.ways.len() == b.ways.len() && a.ways.iter().zip(&b.ways).all(|(x, y)| x.way == y.way && x.label == y.label);
    let (ka, kb) = (who_key(a), who_key(b));
    same.then(|| (0..a.ways.len()).filter(|&k| ka[k] != kb[k]).collect())
}

/// How the page of a side names the stages and the ways of its request, in its language.
struct Labels {
    lang: Lang,
    names: crate::engine::Names,
    ways: Vec<(String, String)>,
    /// Each stage and each way it is written, with the way in words and the stage's label
    /// ([`specs`]).
    specs: Vec<(String, String, String, String)>,
    stages: Vec<(String, String)>,
}

impl Labels {
    fn way(&self, n: &str) -> String {
        self.ways.iter().find(|(w, _)| w == n).map(|(_, l)| l.clone()).unwrap_or_else(|| n.to_string())
    }

    fn stage(&self, n: &str) -> String {
        self.specs.iter().find(|(st, _, _, _)| st == n).map(|(_, _, _, l)| l.clone()).or_else(|| self.stages.iter().find(|(st, _)| st == n).map(|(_, l)| l.clone())).unwrap_or_else(|| n.to_string())
    }

    /// A stage in words, with the way it is written where it is written more than one way.
    fn stage_as(&self, stage: &str, shown: &str) -> String {
        let label = self.stage(stage);
        // a row where the stage does not come writes nothing
        let ways = self.specs.iter().filter(|(st, sh, _, _)| st == stage && !sh.is_empty()).count();
        if ways < 2 {
            return label;
        }
        let words = self.specs.iter().find(|(st, sh, _, _)| st == stage && sh == shown).map(|(_, _, w, _)| w.clone()).unwrap_or_else(|| format!("`{shown}`"));
        if self.lang == Lang::Ja { format!("{label}（{words}）") } else { format!("{label} ({words})") }
    }

    /// [`crate::simulate::way_name`], in the labels of the page.
    fn way_name(&self, st: &StageSim, w: &WaySim) -> Text {
        let (shown, way) = (&st.shown, &w.way);
        if st.ways.len() == 1 && shown != way && !shown.contains(' ') {
            let (a, b) = (self.way(shown), self.way(way));
            let to = crate::simulate::agent_doubts_ja(&b);
            tr!("{a}（{to}）", "{a} ({b} when the agent doubts)")
        } else {
            Text::same(self.way(way))
        }
    }
}

/// Who answers a stage, to compare: each way's people, the quorum, the proxies and, where nobody
/// answers, what happens then, by IDs ([`crate::simulate::WaySim::goes`]). The way's name is left
/// out (another way that comes to the same person does not change the approver), and so are the
/// names of the people and the teams (a version that names them otherwise does not either).
type WhoKey = Vec<(Option<String>, Vec<String>, Option<usize>, Vec<(String, String)>, Option<String>)>;

fn who_key(st: &StageSim) -> WhoKey {
    st.ways.iter().map(|w| (w.label.as_ref().map(|l| l.en.clone()), w.people.clone(), w.needed, w.proxies.clone(), w.goes.clone())).collect()
}

fn not_coming(lang: Lang) -> String {
    if lang == Lang::Ja { "来ません".to_string() } else { "does not come".to_string() }
}

fn stage_sim<'a>(a: &'a Answer, i: usize, stage: &str, shown: &str) -> Option<&'a StageSim> {
    a.sims[i].stages.iter().find(|st| st.stage == stage && st.shown == shown)
}

/// The requesters both sides have, as indices into each.
fn both(a: &Answer, b: &Answer) -> Vec<(usize, usize)> {
    a.sims.iter().enumerate().filter_map(|(i, x)| b.sims.iter().position(|y| y.requester == x.requester).map(|j| (i, j))).collect()
}

/// The requesters whose approvers a box changes: each changed stage, who answers it in the old
/// version (in the version `vo` of its organization) and in the new.
#[allow(clippy::too_many_arguments)]
fn region_people(stages: &[StageChange], a: &Answer, vo: &Version, b: &Answer, v: &Version, lo: &Labels, ln: &Labels, from: &str) -> Group {
    let lang = ln.lang;
    let pairs = both(a, b);
    let mut changes = Vec::new();
    for (i, j) in &pairs {
        for c in stages {
            let (sa, sb) = (c.before.as_ref().and_then(|sh| stage_sim(a, *i, &c.stage, sh)), c.after.as_ref().and_then(|sh| stage_sim(b, *j, &c.stage, sh)));
            if sa.map(who_key) != sb.map(who_key) {
                let only = match (sa, sb) {
                    (Some(x), Some(y)) => changed_ways(x, y),
                    _ => None,
                };
                let before = sa.map_or_else(|| not_coming(lang), |st| answered(vo, st, only.as_deref(), lo));
                let after = sb.map_or_else(|| not_coming(lang), |st| answered(v, st, only.as_deref(), ln));
                let sim = &b.sims[*j];
                changes.push(Person { id: sim.requester.id.clone(), by: sim.requester.by.clone(), named: sim.named.get(lang).to_string(), stage: c.stage.clone(), shown: None, stage_ascii: c.stage_ascii.clone(), shown_ascii: None, label: c.label.clone(), before, after });
            }
        }
    }
    Group { from: from.to_string(), of: pairs.len(), changes }
}

/// The requesters whose approvers change at a stage both sides have, answered by the same way as
/// written: a way to choose written otherwise, or other data.
fn way_people(a: &Answer, vo: &Version, b: &Answer, v: &Version, lo: &Labels, ln: &Labels, from: &str) -> Group {
    let lang = ln.lang;
    let pairs = both(a, b);
    let mut changes = Vec::new();
    for (i, j) in &pairs {
        for st in &b.sims[*j].stages {
            let Some(old) = stage_sim(a, *i, &st.stage, &st.shown) else { continue };
            if who_key(old) != who_key(st) {
                let only = changed_ways(old, st);
                let (before, after) = (answered(vo, old, only.as_deref(), lo), answered(v, st, only.as_deref(), ln));
                let sim = &b.sims[*j];
                changes.push(Person {
                    id: sim.requester.id.clone(),
                    by: sim.requester.by.clone(),
                    named: sim.named.get(lang).to_string(),
                    stage: st.stage.clone(),
                    shown: Some(st.shown.clone()),
                    stage_ascii: ln.names.stage(&st.stage).to_string(),
                    shown_ascii: Some(ln.names.spec(&st.shown)),
                    label: ln.stage_as(&st.stage, &st.shown),
                    before,
                    after,
                });
            }
        }
    }
    Group { from: from.to_string(), of: pairs.len(), changes }
}

/// How the changes reach the tickets in progress (DESIGN 7.7, 8.7, 11.2). A ticket keeps the version
/// of the request it was submitted with, and no version of a file the request reads: a change of
/// the organization or of the masters reaches it at the next stage it starts (it keeps the key of
/// the row of a master it chose), and a shared route is part of each request that uses it.
fn in_progress(d: &Diff, effective: Option<i64>, lang: Lang) -> Vec<String> {
    let t = |en: String, ja: String| if lang == Lang::Ja { ja } else { en };
    let mut out = Vec::new();
    if !d.differs() {
        return out;
    }
    let (a, b) = (d.old.version, d.new.version);
    let own = !d.route.is_empty() || !d.sections.is_empty() || (!d.from_data && !d.approvers.is_empty());
    match d.kind {
        FileKind::Request => {}
        FileKind::Route => out.push(t(
            "A shared route is part of each request that uses it, and a ticket finishes with the version of the request it was submitted with: raise the version of each request that uses it, so that the tickets in progress finish as they were submitted.".to_string(),
            "共有の経路は、それを使う申請の種類の一部です。チケットは、出したときの申請の種類の版で終わります。進行中のチケットが出したときのとおりに終わるように、この経路を使う申請の種類の版を、それぞれ上げてください。".to_string(),
        )),
        FileKind::Organization => out.push(t(
            "A change of the organization reaches a ticket in progress at the next stage it starts: a stage whose approvers are decided keeps them.".to_string(),
            "組織の変更は、進行中のチケットには、次に始まるステップから効きます。承認者が決まったステップは、そのまま進みます。".to_string(),
        )),
        FileKind::Masters => out.push(t(
            "A ticket in progress keeps the key of the row it chose: a stage whose approvers are decided keeps them, and a stage that starts after the change reads the row as the masters then say.".to_string(),
            "進行中のチケットは、フォームで選んだマスタの行のキーを持ったままです。承認者が決まったステップはそのまま進み、変更のあとに始まるステップは、そのときのマスタの行を読みます。".to_string(),
        )),
    }
    if own && d.kind == FileKind::Request {
        if a != b {
            let from = effective.map(|e| crate::lex::date_text(e));
            out.push(t(
                format!(
                    "A ticket finishes with the version of the request it was submitted with: tickets submitted with version {a} finish as version {a} says, and what changes here applies to requests submitted with version {b}{}.",
                    from.as_ref().map(|f| format!(", from {f}")).unwrap_or_default()
                ),
                format!(
                    "チケットは、出したときの申請の種類の版で終わります。版 {a} で出したチケットは版 {a} のとおりに終わり、ここに並べた変更は、版 {b} で出す申請{}に効きます。",
                    from.as_ref().map(|f| format!("（{f} から）")).unwrap_or_default()
                ),
            ));
        } else {
            out.push(t(
                format!("Both are version {a}. A ticket in progress is told from the requests submitted after the change only by the version it was submitted with: raise the version, so that the tickets in progress finish as they were submitted."),
                format!("どちらも版 {a} です。進行中のチケットと、変更のあとに出す申請は、出したときの版でしか見分けられません。進行中のチケットが出したときのとおりに終わるように、版を上げてください。"),
            ));
        }
    }
    if d.from_data && d.approvers.iter().any(|g| !g.changes.is_empty()) {
        out.push(t(
            "Where the request itself does not change, the approvers that change come from the data it reads: a stage whose approvers are decided keeps them, and a ticket in progress takes the change at the next stage it starts.".to_string(),
            "申請の種類が変わっていない所で承認者が変わるのは、申請の種類が読むデータが変わったからです。承認者が決まったステップはそのまま進み、この変更は、進行中のチケットには次に始まるステップから効きます。".to_string(),
        ));
    }
    out
}

fn kind_words(k: FileKind) -> Text {
    match k {
        FileKind::Request => tr!("申請の種類", "a request"),
        FileKind::Route => tr!("共有の経路", "a shared route"),
        FileKind::Organization => tr!("組織", "an organization"),
        FileKind::Masters => tr!("マスタ", "masters"),
    }
}

fn kind_key(k: FileKind) -> &'static str {
    match k {
        FileKind::Request => "request",
        FileKind::Route => "route",
        FileKind::Organization => "organization",
        FileKind::Masters => "masters",
    }
}

// ── What the pages say otherwise ────────────────────────────────────────

/// The sections of a page: the head (the blocks before the first section, but the line that names
/// the file), then each section by its id and title.
fn split(p: &Page, lang: Lang) -> Vec<(String, String, Vec<Block>)> {
    let mut out: Vec<(String, String, Vec<Block>)> = vec![("head".to_string(), if lang == Lang::Ja { "ファイル".to_string() } else { "The file".to_string() }, Vec::new())];
    for b in &p.blocks {
        match b {
            Block::Heading { level: 2, text, id } => out.push((id.clone(), text.clone(), Vec::new())),
            // the line that names the file, its version and its digest
            Block::Para(t) if out.len() == 1 && t.contains(&format!("`{}`", p.file)) => {}
            b => out.last_mut().expect("the head").2.push(b.clone()),
        }
    }
    out
}

/// One thing a section says: a row of a table, under the heading above it, or a sentence.
enum Item {
    Row { ctx: String, head: Vec<String>, cells: Vec<String> },
    Text { ctx: String, text: String },
}

fn items(bs: &[Block]) -> Vec<Item> {
    let mut ctx = String::new();
    let mut out = Vec::new();
    for b in bs {
        match b {
            Block::Heading { text, .. } => ctx = text.clone(),
            Block::Table { head, rows } => out.extend(rows.iter().map(|r| Item::Row { ctx: ctx.clone(), head: head.clone(), cells: r.clone() })),
            Block::List(xs) => out.extend(xs.iter().map(|x| Item::Text { ctx: ctx.clone(), text: x.clone() })),
            Block::Para(t) | Block::Note(t) => out.push(Item::Text { ctx: ctx.clone(), text: t.clone() }),
        }
    }
    out
}

/// What the sections of the new page say otherwise than the old's, but those of `skip`.
fn sections_of(old: &Page, new: &Page, skip: &[&str], lang: Lang) -> Vec<Section> {
    let (a, b) = (split(old, lang), split(new, lang));
    let mut ids: Vec<(String, String)> = b.iter().map(|(i, t, _)| (i.clone(), t.clone())).collect();
    for (i, t, _) in &a {
        if !ids.iter().any(|(x, _)| x == i) {
            ids.push((i.clone(), t.clone()));
        }
    }
    let mut out = Vec::new();
    for (id, title) in ids {
        if skip.contains(&id.as_str()) {
            continue;
        }
        let blocks = |s: &[(String, String, Vec<Block>)]| s.iter().find(|(i, _, _)| *i == id).map(|(_, _, bs)| bs.clone()).unwrap_or_default();
        let changes = compare(&items(&blocks(&a)), &items(&blocks(&b)), lang);
        if !changes.is_empty() {
            out.push(Section { id, title, changes });
        }
    }
    out
}

/// The changes between the items of a section: a row by its first cell where that is one row's
/// alone (else by the whole row), each cell that differs; a sentence by its words, the ones taken
/// out paired, in order, with the ones put in under the same heading.
fn compare(old: &[Item], new: &[Item], lang: Lang) -> Vec<Change> {
    let mut out = Vec::new();
    let rows = |xs: &[Item]| -> Vec<(String, Vec<String>, Vec<String>)> {
        xs.iter().filter_map(|i| if let Item::Row { ctx, head, cells } = i { Some((ctx.clone(), head.clone(), cells.clone())) } else { None }).collect()
    };
    let (ro, rn) = (rows(old), rows(new));
    let unique = |rs: &[(String, Vec<String>, Vec<String>)], ctx: &str, first: &str| rs.iter().filter(|(c, _, cells)| c == ctx && cells.first().map(String::as_str) == Some(first)).count() == 1;
    let key = |rs: &[(String, Vec<String>, Vec<String>)], others: &[(String, Vec<String>, Vec<String>)], (ctx, _, cells): &(String, Vec<String>, Vec<String>)| {
        let first = cells.first().cloned().unwrap_or_default();
        if unique(rs, ctx, &first) && (unique(others, ctx, &first) || !others.iter().any(|(c, _, x)| c == ctx && x.first() == Some(&first))) {
            (ctx.clone(), vec![first])
        } else {
            (ctx.clone(), cells.clone())
        }
    };
    let item = |ctx: &str, what: &str| if ctx.is_empty() { what.to_string() } else if lang == Lang::Ja { format!("{ctx}：{what}") } else { format!("{ctx}: {what}") };
    let whole = |head: &[String], cells: &[String]| -> String {
        head.iter().zip(cells).skip(1).map(|(h, c)| if lang == Lang::Ja { format!("{h}：{c}") } else { format!("{h}: {c}") }).collect::<Vec<_>>().join(if lang == Lang::Ja { "、" } else { "; " })
    };
    for r in &ro {
        let k = key(&ro, &rn, r);
        match rn.iter().find(|x| key(&rn, &ro, x) == k) {
            Some((ctx, head, cells)) => {
                for (h, c) in head.iter().zip(cells).skip(1) {
                    let at = r.1.iter().position(|x| x == h);
                    let before = at.and_then(|i| r.2.get(i)).cloned();
                    if before.as_ref() != Some(c) {
                        let first = &cells[0];
                        let what = if lang == Lang::Ja { format!("{first}の{h}") } else { format!("{first} — {h}") };
                        out.push(Change { item: Some(item(ctx, &what)), before, after: Some(c.clone()) });
                    }
                }
            }
            None => out.push(Change { item: Some(item(&r.0, r.2.first().map(String::as_str).unwrap_or_default())), before: Some(whole(&r.1, &r.2)), after: None }),
        }
    }
    for r in &rn {
        let k = key(&rn, &ro, r);
        if !ro.iter().any(|x| key(&ro, &rn, x) == k) {
            out.push(Change { item: Some(item(&r.0, r.2.first().map(String::as_str).unwrap_or_default())), before: None, after: Some(whole(&r.1, &r.2)) });
        }
    }
    // the sentences, under each heading
    let texts = |xs: &[Item]| -> Vec<(String, String)> { xs.iter().filter_map(|i| if let Item::Text { ctx, text } = i { Some((ctx.clone(), text.clone())) } else { None }).collect() };
    let (to, tn) = (texts(old), texts(new));
    let mut ctxs: Vec<String> = Vec::new();
    for (c, _) in tn.iter().chain(&to) {
        if !ctxs.contains(c) {
            ctxs.push(c.clone());
        }
    }
    for ctx in ctxs {
        let of = |xs: &[(String, String)]| xs.iter().filter(|(c, _)| *c == ctx).map(|(_, t)| t.clone()).collect::<Vec<String>>();
        let (a, b) = (of(&to), of(&tn));
        let mut gone: Vec<String> = Vec::new();
        let mut rest = b.clone();
        for t in &a {
            match rest.iter().position(|x| x == t) {
                Some(i) => {
                    rest.remove(i);
                }
                None => gone.push(t.clone()),
            }
        }
        let mut came = Vec::new();
        let mut left = a.clone();
        for t in &b {
            match left.iter().position(|x| x == t) {
                Some(i) => {
                    left.remove(i);
                }
                None => came.push(t.clone()),
            }
        }
        let n = gone.len().max(came.len());
        for i in 0..n {
            let what = (!ctx.is_empty()).then(|| ctx.clone());
            out.push(Change { item: what, before: gone.get(i).cloned(), after: came.get(i).cloned() });
        }
    }
    out
}

// ── Writing it ──────────────────────────────────────────────────────────

fn dash() -> String {
    "—".to_string()
}

/// The names of a group, at most [`NAMED`], and how many more.
fn named(g: &Group) -> (Vec<&Person>, usize) {
    let shown: Vec<&Person> = g.changes.iter().take(NAMED).collect();
    let more = g.changes.len() - shown.len();
    (shown, more)
}

/// The requesters whose approvers change in a group, in a sentence.
fn group_line(g: &Group, lang: Lang) -> String {
    let people: Vec<&str> = {
        let mut ids: Vec<&str> = Vec::new();
        for p in &g.changes {
            if !ids.contains(&p.named.as_str()) {
                ids.push(&p.named);
            }
        }
        ids
    };
    let n = people.len();
    if lang == Lang::Ja {
        format!("{} からの組織で、{} 人のうち {n} 人の承認者が変わります。", g.from, g.of)
    } else {
        format!("In the organization from {}, the approvers change for {n} of {} requesters.", g.from, g.of)
    }
}

fn more_line(more: usize, lang: Lang) -> Option<String> {
    (more > 0).then(|| if lang == Lang::Ja { format!("ほかに {more} 件あります。すべては `--format json` で出ます。") } else { format!("And {more} more: `--format json` lists them all.") })
}

fn person_cells(p: &Person) -> Vec<String> {
    vec![p.named.clone(), p.label.clone(), p.before.clone(), p.after.clone()]
}

/// The head of the output: what the two files are.
fn head_line(d: &Diff) -> String {
    let k = match d.kind {
        FileKind::Request => tr!("申請の種類", "The request"),
        FileKind::Route => tr!("共有の経路", "The shared route"),
        FileKind::Organization => tr!("組織", "The organization"),
        FileKind::Masters => tr!("マスタ", "The masters"),
    };
    let k = k.get(d.lang);
    let (a, b) = (&d.old, &d.new);
    let version = if a.version == b.version { format!("v{}", a.version) } else { format!("v{} → v{}", a.version, b.version) };
    if d.lang == Lang::Ja {
        format!("{k} `{}`（{version}）：`{}` → `{}`", d.name, a.file, b.file)
    } else {
        format!("{k} `{}` ({version}): `{}` → `{}`", d.name, a.file, b.file)
    }
}

/// The blocks of the page of the changes.
fn blocks(d: &Diff) -> Vec<Block> {
    let ja = d.lang == Lang::Ja;
    let t = |en: &str, ja_: &str| if ja { ja_.to_string() } else { en.to_string() };
    let mut out = vec![Block::Para(head_line(d))];
    if !d.differs() {
        out.push(Block::Para(if d.total { t("The two say the same and answer alike.", "二つは同じことを言い、同じに答えます。") } else { t("No difference is shown, but some cases were not settled.", "違いは見つかりませんでした。ただし、調べきれなかった場合があります。") }));
    }
    let people_head = vec![t("Requester", "申請者"), t("Stage", "ステップ"), t("Before", "前"), t("After", "後")];
    if !d.route.is_empty() || !d.unsettled.is_empty() {
        out.push(crate::doc::h2(t("Route", "経路"), "route"));
        for (k, r) in d.route.iter().enumerate() {
            out.push(crate::doc::h3(r.text.trim_end_matches(['.', '。']).to_string(), format!("case-{}", k + 1)));
            for g in &r.people {
                out.push(Block::Para(group_line(g, d.lang)));
                let (shown, more) = named(g);
                out.push(Block::Table { head: people_head.clone(), rows: shown.into_iter().map(person_cells).collect() });
                out.extend(more_line(more, d.lang).map(Block::Para));
            }
        }
        if !d.unsettled.is_empty() {
            out.push(Block::Para(t("These were not settled:", "次の場合は、調べきれませんでした。")));
            out.push(Block::List(d.unsettled.iter().map(|(c, w)| if ja { format!("{c}：{w}") } else { format!("{c}: {w}") }).collect()));
        } else if !d.route.is_empty() && d.total {
            out.push(Block::Para(t("Every other request comes to the same stages.", "ほかの申請は、どれも同じステップに進みます。")));
        }
    }
    for s in &d.sections {
        out.push(crate::doc::h2(s.title.clone(), &format!("changed-{}", s.id)));
        let head = vec![t("What", "何が"), t("Before", "前"), t("After", "後")];
        let rows = s.changes.iter().map(|c| vec![c.item.clone().unwrap_or_else(dash), c.before.clone().unwrap_or_else(dash), c.after.clone().unwrap_or_else(dash)]).collect();
        out.push(Block::Table { head, rows });
    }
    if d.approvers.iter().any(|g| !g.changes.is_empty()) {
        out.push(crate::doc::h2(t("Approvers", "承認者"), "approvers"));
        out.push(Block::Para(t("Where the route does not change, these requesters' stages are answered by others:", "経路は変わらず、次の申請者のステップに答える人が変わります。")));
        for g in &d.approvers {
            out.push(Block::Para(group_line(g, d.lang)));
            let (shown, more) = named(g);
            out.push(Block::Table { head: people_head.clone(), rows: shown.into_iter().map(person_cells).collect() });
            out.extend(more_line(more, d.lang).map(Block::Para));
        }
    }
    if let Some(n) = &d.no_people {
        out.push(Block::Note(n.clone()));
    }
    if !d.in_progress.is_empty() {
        out.push(crate::doc::h2(t("Tickets in progress", "進行中のチケット"), "in-progress"));
        out.extend(d.in_progress.iter().cloned().map(Block::Para));
    }
    out
}

fn page_of(d: &Diff) -> Page {
    let title = if d.lang == Lang::Ja { format!("{}の変更", d.title) } else { format!("{}: what changes", d.title) };
    Page { file: format!("{} → {}", d.old.file, d.new.file), sha: format!("{} → {}", d.old.sha, d.new.sha), lang: d.lang, title, description: None, blocks: blocks(d) }
}

pub fn markdown(d: &Diff) -> String {
    crate::doc::write::markdown(&page_of(d))
}

/// The page of the changes as Markdown that another holds (`ritsu mod diff`, `ritsu mod report` and
/// the module site): the title a heading of the third level, as the diffs of the other languages
/// begin, and no comment naming the two files, which the holder names. In the module site, a comment
/// would be text on the page, and the names of a module's files hold an `@` outside code.
pub fn markdown_held(d: &Diff) -> String {
    crate::doc::write::markdown_held(&page_of(d), 2)
}

pub fn html(d: &Diff) -> String {
    crate::doc::write::html(&page_of(d))
}

/// The page of the changes as text for a terminal: the same blocks, a heading as a line, a
/// sentence as a line, a row of a table as its cells.
pub fn text(d: &Diff) -> String {
    plain(&blocks(d), d.lang)
}

fn plain(bs: &[Block], lang: Lang) -> String {
    let mut out = String::new();
    for b in bs {
        match b {
            // a section's title ends in a colon; a box of the route, a sentence, in its period
            Block::Heading { level, text, .. } => {
                let pad = "  ".repeat(usize::from(level.saturating_sub(2)));
                let end = match (*level == 2, lang == Lang::Ja) {
                    (true, true) => "：",
                    (true, false) => ":",
                    (false, true) => "。",
                    (false, false) => ".",
                };
                out.push_str(&format!("{pad}{text}{end}\n"));
            }
            Block::Para(t) | Block::Note(t) => out.push_str(&format!("  {t}\n")),
            Block::List(xs) => {
                for x in xs {
                    out.push_str(&format!("  - {x}\n"));
                }
            }
            // the last two columns are before and after: `what: before → after`
            Block::Table { rows, .. } => {
                for r in rows {
                    let n = r.len();
                    let what = r[..n.saturating_sub(2)].join(if lang == Lang::Ja { "、" } else { ", " });
                    let sep = if lang == Lang::Ja { "：" } else { ": " };
                    // a sentence under no heading of its section: no "what"
                    if what == dash() {
                        out.push_str(&format!("    {} → {}\n", r[n - 2], r[n - 1]));
                    } else {
                        out.push_str(&format!("    {what}{sep}{} → {}\n", r[n - 2], r[n - 1]));
                    }
                }
            }
        }
    }
    out
}

fn group_json(g: &Group) -> Json {
    Json::Obj(vec![
        ("organization_from".to_string(), Json::Str(g.from.clone())),
        ("requesters".to_string(), Json::Int(g.of as i128)),
        (
            "changes".to_string(),
            Json::Arr(
                g.changes
                    .iter()
                    .map(|p| {
                        Json::Obj(vec![
                            ("requester".to_string(), Json::Str(p.id.clone())),
                            ("made_by".to_string(), p.by.clone().map_or(Json::Null, Json::Str)),
                            ("named".to_string(), Json::Str(p.named.clone())),
                            ("stage".to_string(), Json::Str(p.stage_ascii.clone())),
                            ("stage_written".to_string(), Json::Str(p.stage.clone())),
                            ("spec".to_string(), opt(&p.shown_ascii)),
                            ("spec_written".to_string(), opt(&p.shown)),
                            ("label".to_string(), Json::Str(p.label.clone())),
                            ("before".to_string(), Json::Str(p.before.clone())),
                            ("after".to_string(), Json::Str(p.after.clone())),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

fn opt(s: &Option<String>) -> Json {
    s.clone().map_or(Json::Null, Json::Str)
}

/// The changes as JSON (DESIGN 16.2): the keys in English, the words in the language of the page.
pub fn json(d: &Diff) -> Json {
    let side = |h: &Head| Json::Obj(vec![("file".to_string(), Json::Str(h.file.clone())), ("version".to_string(), Json::Int(i128::from(h.version))), ("sha256".to_string(), Json::Str(h.sha.clone()))]);
    // what a change reaches (DESIGN 16.2): a ticket keeps the version of its request, and no version
    // of a file the request reads, as `in_progress` says
    let applies = match d.kind {
        FileKind::Request if d.old.version != d.new.version => "new_requests",
        FileKind::Request => "same_version",
        FileKind::Route => "requests_that_use_it",
        FileKind::Organization | FileKind::Masters => "next_stage",
    };
    let route = d
        .route
        .iter()
        .map(|r| {
            Json::Obj(vec![
                ("case".to_string(), Json::Arr(r.case.iter().map(|h| Json::Obj(vec![("name".to_string(), Json::Str(h.name.clone())), ("label".to_string(), Json::Str(h.label.clone())), ("is".to_string(), Json::Str(h.is.clone()))])).collect())),
                (
                    "stages".to_string(),
                    Json::Arr(
                        r.stages
                            .iter()
                            .map(|s| {
                                Json::Obj(vec![
                                    ("stage".to_string(), Json::Str(s.stage_ascii.clone())),
                                    ("stage_written".to_string(), Json::Str(s.stage.clone())),
                                    ("label".to_string(), Json::Str(s.label.clone())),
                                    ("before".to_string(), opt(&s.before_ascii)),
                                    ("before_written".to_string(), opt(&s.before)),
                                    ("after".to_string(), opt(&s.after_ascii)),
                                    ("after_written".to_string(), opt(&s.after)),
                                    ("before_words".to_string(), opt(&s.before_words)),
                                    ("after_words".to_string(), opt(&s.after_words)),
                                ])
                            })
                            .collect(),
                    ),
                ),
                ("text".to_string(), Json::Str(r.text.clone())),
                ("example".to_string(), Json::Obj(r.example.iter().map(|(k, v)| (k.clone(), Json::Str(v.clone()))).collect())),
                ("people".to_string(), Json::Arr(r.people.iter().map(group_json).collect())),
                ("applies_to".to_string(), Json::Str(applies.to_string())),
            ])
        })
        .collect();
    let sections = d
        .sections
        .iter()
        .map(|s| {
            Json::Obj(vec![
                ("section".to_string(), Json::Str(s.id.clone())),
                ("title".to_string(), Json::Str(s.title.clone())),
                ("changes".to_string(), Json::Arr(s.changes.iter().map(|c| Json::Obj(vec![("item".to_string(), opt(&c.item)), ("before".to_string(), opt(&c.before)), ("after".to_string(), opt(&c.after))])).collect())),
                ("applies_to".to_string(), Json::Str(applies.to_string())),
            ])
        })
        .collect();
    Json::Obj(vec![
        ("ringi".to_string(), Json::Str(env!("CARGO_PKG_VERSION").to_string())),
        ("kind".to_string(), Json::Str(kind_key(d.kind).to_string())),
        ("name".to_string(), Json::Str(d.name.clone())),
        ("old".to_string(), side(&d.old)),
        ("new".to_string(), side(&d.new)),
        ("differs".to_string(), Json::Bool(d.differs())),
        ("total".to_string(), Json::Bool(d.total)),
        ("route".to_string(), Json::Arr(route)),
        ("unsettled".to_string(), Json::Arr(d.unsettled.iter().map(|(c, w)| Json::Obj(vec![("case".to_string(), Json::Str(c.clone())), ("why".to_string(), Json::Str(w.clone()))])).collect())),
        ("sections".to_string(), Json::Arr(sections)),
        ("approvers".to_string(), Json::Arr(d.approvers.iter().map(group_json).collect())),
        ("approvers_apply_to".to_string(), Json::Str(if d.from_data { "next_stage" } else { applies }.to_string())),
        ("no_people".to_string(), opt(&d.no_people)),
        ("in_progress".to_string(), Json::Arr(d.in_progress.iter().cloned().map(Json::Str).collect())),
    ])
}

// ── Two versions of the organization ───────────────────────────────────

/// What two versions of the organization change for one request (DESIGN 11.4).
pub struct Versions {
    pub lang: Lang,
    pub file: String,
    pub sha: String,
    pub name: String,
    pub title: String,
    /// The two days asked, and the first day of the version in effect on each.
    pub asked: (String, String),
    pub from: (String, String),
    /// The requesters both versions have, and how many.
    pub of: usize,
    pub changes: Vec<Person>,
    /// The requesters only one version has.
    pub left: Vec<String>,
    pub joined: Vec<String>,
    /// With `submitted by`: the line, and who may now submit it and who may no longer, of the people
    /// both versions have.
    pub submitted_by: Option<String>,
    pub may_now: Vec<String>,
    pub may_not: Vec<String>,
    /// Each stage nobody answers for some requesters, by what happens then, with how many in each
    /// version.
    pub nobody: Vec<Nobody>,
    pub in_progress: String,
}

/// A stage, as it is written, that finds nobody for some requesters, by what happens then: for how
/// many in each version.
pub struct Nobody {
    pub stage: String,
    pub shown: String,
    pub stage_ascii: String,
    pub shown_ascii: String,
    pub label: String,
    pub then: Then,
    pub before: usize,
    pub after: usize,
}

/// What happens where a stage finds nobody: it goes up or to the way `then` names, it is skipped, or
/// the ticket waits for an approver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Then {
    Elsewhere,
    Skipped,
    Waits,
}

impl Then {
    fn of(goes: &str) -> Option<Then> {
        match goes {
            "open" => None,
            "skip" => Some(Then::Skipped),
            "wait" | "cannot" => Some(Then::Waits),
            _ => Some(Then::Elsewhere),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Then::Elsewhere => "elsewhere",
            Then::Skipped => "skipped",
            Then::Waits => "waits",
        }
    }

    fn words(self, lang: Lang) -> String {
        let t = match self {
            Then::Elsewhere => tr!("上へ回るか、ほかへ回ります", "goes up or elsewhere"),
            Then::Skipped => tr!("飛ばされます", "is skipped"),
            Then::Waits => tr!("管理する人が承認者を足すまで待ちます", "waits until an approver is added"),
        };
        t.get(lang).to_string()
    }
}

impl Versions {
    pub fn differs(&self) -> bool {
        !self.changes.is_empty() || !self.left.is_empty() || !self.joined.is_empty() || !self.may_now.is_empty() || !self.may_not.is_empty() || !self.nobody.is_empty()
    }

    pub fn same_version(&self) -> bool {
        self.from.0 == self.from.1
    }
}

/// `ringi diff --versions <day> <day> <file>`.
pub fn versions(side: &Side, days: (i64, i64), suite: &Suite, opts: &Options, lang: Lang, made: Lang) -> Result<Versions, Refused> {
    let r = read(side, suite, opts, lang, made)?;
    let f = r.file();
    if f.kind != FileKind::Request {
        return Err(Refused::Said(tr!("`--versions` が比べるのは申請の種類の承認者です。申請の種類のファイルを渡してください", "`--versions` compares the approvers of a request: give a request's file")));
    }
    let org = r.scoped(|s| s.org_rc());
    let org = match org.as_deref() {
        Some(crate::org::Data::Org(o)) => o.clone(),
        Some(crate::org::Data::NoOrg(none)) => {
            let line = &none.line;
            return Err(Refused::Said(tr!("組織のデータを読んでいません。{}", "no data of the organization is read: {}", line.ja; line.en)));
        }
        _ => return Err(Refused::Said(tr!("申請の種類が組織を読みません", "the request reads no organization"))),
    };
    let version = |d: i64| {
        org.in_effect(d).cloned().ok_or_else(|| {
            let day = crate::lex::date_text(d);
            let first = org.versions.first().map(|v| v.day()).unwrap_or_default();
            Refused::Said(tr!("{day} に効いている組織の版がありません（最初の版は {first} から）", "no version of the organization is in effect on {day} (the first is from {first})"))
        })
    };
    let (va, vb) = (version(days.0)?, version(days.1)?);
    let t = |en: String, ja: String| if made == Lang::Ja { ja } else { en };
    let asked = (crate::lex::date_text(days.0), crate::lex::date_text(days.1));
    let mut out = Versions {
        lang: made,
        file: side.shown.clone(),
        sha: r.page.sha.clone(),
        name: f.name.text.clone(),
        title: r.page.title.clone(),
        asked: asked.clone(),
        from: (va.day(), vb.day()),
        of: 0,
        changes: Vec::new(),
        left: Vec::new(),
        joined: Vec::new(),
        submitted_by: None,
        may_now: Vec::new(),
        may_not: Vec::new(),
        nobody: Vec::new(),
        in_progress: String::new(),
    };
    if out.same_version() {
        return Ok(out);
    }
    let sim = |d: i64| within(&r.side.files, || crate::simulate::simulate(&r.o, &r.run, &crate::simulate::Ask { requester: None, every: true, form: None, on: On::Day(d) })).map_err(Refused::Said);
    let (a, b) = (sim(days.0)?, sim(days.1)?);
    let labels = r.labels(made, [&a, &b].into_iter());
    let g = way_people(&a, &va, &b, &vb, &labels, &labels, &vb.day());
    out.of = g.of;
    out.changes = g.changes;
    out.left = a.sims.iter().filter(|x| !b.sims.iter().any(|y| y.requester == x.requester)).map(|x| x.named.get(made).to_string()).collect();
    out.joined = b.sims.iter().filter(|y| !a.sims.iter().any(|x| x.requester == y.requester)).map(|y| y.named.get(made).to_string()).collect();
    // who may submit it, of the people both versions have
    if let Some(l) = crate::org::submit::of(f) {
        out.submitted_by = Some(l.shown());
        for p in &vb.people {
            if va.person(&p.id).is_none() {
                continue;
            }
            let (before, after) = (l.may(&org, &va, &p.id), l.may(&org, &vb, &p.id));
            if before != after {
                let named = vb.who(&p.id).get(made).to_string();
                if after { out.may_now.push(named) } else { out.may_not.push(named) }
            }
        }
        // the requesters only one version has, which `submitted by` lets in or keeps out, are said
        // there
        out.left.retain(|n| !out.may_not.contains(n));
        out.joined.retain(|n| !out.may_now.contains(n));
    }
    // the stages nobody answers, for how many requesters
    let mut keys: Vec<(String, String)> = Vec::new();
    for st in a.sims.iter().chain(&b.sims).flat_map(|s| &s.stages) {
        let k = (st.stage.clone(), st.shown.clone());
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    let count = |x: &Answer, (stage, shown): &(String, String), then: Then| {
        x.sims.iter().filter(|s| s.stages.iter().any(|st| st.stage == *stage && st.shown == *shown && st.ways.iter().any(|w| w.goes.as_deref().and_then(Then::of) == Some(then)))).count()
    };
    for k in keys {
        for then in [Then::Elsewhere, Then::Skipped, Then::Waits] {
            let (x, y) = (count(&a, &k, then), count(&b, &k, then));
            if x != y {
                out.nobody.push(Nobody { label: labels.stage_as(&k.0, &k.1), stage_ascii: labels.names.stage(&k.0).to_string(), shown_ascii: labels.names.spec(&k.1), stage: k.0.clone(), shown: k.1.clone(), then, before: x, after: y });
            }
        }
    }
    let to = &vb.day();
    out.in_progress = t(
        format!("A stage whose approvers are decided keeps them on and after {to}; each stage that starts on or after {to} takes its approvers from the version from {to}. Where an approver a stage waits for is no longer in the organization, an administrator hands the ticket over; a late stage goes up as the version in effect then says."),
        format!("承認者が決まったステップは、{to} のあともそのまま進みます。{to} 以降に始まるステップは、{to} からの版で承認者が決まります。待っている承認者が組織からいなくなったチケットは、管理する人が引き継ぎます。期限を過ぎて上へ回るときは、そのとき効いている版で回る先が決まります。"),
    );
    Ok(out)
}

fn versions_blocks(v: &Versions) -> Vec<Block> {
    let ja = v.lang == Lang::Ja;
    let t = |en: String, ja_: String| if ja { ja_ } else { en };
    let (a, b) = (&v.from.0, &v.from.1);
    let (da, db) = (&v.asked.0, &v.asked.1);
    let mut out = vec![Block::Para(t(
        format!("The request `{}` (`{}`), read in the organization in effect on {da} (the version from {a}) and on {db} (the version from {b}).", v.name, v.file),
        format!("申請の種類 `{}`（`{}`）を、{da} に効いている組織（{a} からの版）と、{db} に効いている組織（{b} からの版）で読み比べます。", v.name, v.file),
    ))];
    if v.same_version() {
        out.push(Block::Para(t(format!("Both days are in the version from {a}: nothing changes."), format!("どちらの日も {a} からの版にあります。変わるものはありません。"))));
        return out;
    }
    if !v.differs() {
        out.push(Block::Para(t(format!("No approver changes for the {} requesters both versions have.", v.of), format!("二つの版にいる {} 人の申請者のだれも、承認者が変わりません。", v.of))));
    }
    if !v.changes.is_empty() {
        out.push(crate::doc::h2(t("Approvers".to_string(), "承認者".to_string()), "approvers"));
        let people: Vec<&str> = {
            let mut ids: Vec<&str> = Vec::new();
            for p in &v.changes {
                if !ids.contains(&p.named.as_str()) {
                    ids.push(&p.named);
                }
            }
            ids
        };
        out.push(Block::Para(t(format!("The approvers change for {} of the {} requesters both versions have.", people.len(), v.of), format!("二つの版にいる {} 人の申請者のうち、{} 人の承認者が変わります。", v.of, people.len()))));
        let head = vec![t("Requester".into(), "申請者".into()), t("Stage".into(), "ステップ".into()), t(format!("From {a}"), format!("{a} からの版")), t(format!("From {b}"), format!("{b} からの版"))];
        let shown: Vec<Vec<String>> = v.changes.iter().take(NAMED).map(person_cells).collect();
        out.push(Block::Table { head, rows: shown });
        out.extend(more_line(v.changes.len().saturating_sub(NAMED), v.lang).map(Block::Para));
    }
    let list = |xs: &[String]| -> String {
        let shown: Vec<String> = xs.iter().take(NAMED).cloned().collect();
        let more = xs.len() - shown.len();
        let mut s = if ja { shown.join("、") } else { shown.join(", ") };
        if more > 0 {
            s.push_str(&if ja { format!("、ほか {more} 人") } else { format!(", and {more} more") });
        }
        s
    };
    if let Some(by) = &v.submitted_by
        && (!v.may_now.is_empty() || !v.may_not.is_empty())
    {
        out.push(crate::doc::h2(t("Who may submit it".into(), "出せる人".into()), "submitters"));
        let mut items = Vec::new();
        if !v.may_now.is_empty() {
            items.push(t(format!("May submit it from {b} (`submitted by {by}`): {}", list(&v.may_now)), format!("{b} から出せるようになる人（`submitted by {by}`）：{}", list(&v.may_now))));
        }
        if !v.may_not.is_empty() {
            items.push(t(format!("May no longer submit it from {b}: {}", list(&v.may_not)), format!("{b} から出せなくなる人：{}", list(&v.may_not))));
        }
        out.push(Block::List(items));
    }
    if !v.joined.is_empty() || !v.left.is_empty() {
        out.push(crate::doc::h2(t("Requesters".into(), "申請者".into()), "requesters"));
        let mut items = Vec::new();
        if !v.joined.is_empty() {
            items.push(t(format!("Only in the version from {b}: {}", list(&v.joined)), format!("{b} からの版にだけいる人：{}", list(&v.joined))));
        }
        if !v.left.is_empty() {
            items.push(t(format!("Only in the version from {a}: {}", list(&v.left)), format!("{a} からの版にだけいる人：{}", list(&v.left))));
        }
        out.push(Block::List(items));
    }
    if !v.nobody.is_empty() {
        out.push(crate::doc::h2(t("Stages nobody answers".into(), "答える人がいないステップ".into()), "nobody"));
        out.push(Block::Para(t(
            "How many requesters each stage finds nobody for, by what happens then as the request says, where the two versions differ:".into(),
            "答える人が見つからない申請者の数が、二つの版で違うステップです。そのときどうなるかは、申請の種類の決まりによります。".into(),
        )));
        let head = vec![t("Stage".into(), "ステップ".into()), t("Then".into(), "そのとき".into()), t(format!("From {a}"), format!("{a} からの版")), t(format!("From {b}"), format!("{b} からの版"))];
        out.push(Block::Table { head, rows: v.nobody.iter().map(|n| vec![n.label.clone(), n.then.words(v.lang), n.before.to_string(), n.after.to_string()]).collect() });
    }
    out.push(crate::doc::h2(t("Tickets in progress".into(), "進行中のチケット".into()), "in-progress"));
    out.push(Block::Para(v.in_progress.clone()));
    out
}

fn versions_page(v: &Versions) -> Page {
    let (a, b) = (&v.from.0, &v.from.1);
    let title = if v.lang == Lang::Ja { format!("{}：{a} からの組織と {b} からの組織", v.title) } else { format!("{}: the organization from {a} and from {b}", v.title) };
    Page { file: v.file.clone(), sha: v.sha.clone(), lang: v.lang, title, description: None, blocks: versions_blocks(v) }
}

pub fn versions_markdown(v: &Versions) -> String {
    crate::doc::write::markdown(&versions_page(v))
}

pub fn versions_html(v: &Versions) -> String {
    crate::doc::write::html(&versions_page(v))
}

pub fn versions_text(v: &Versions) -> String {
    plain(&versions_blocks(v), v.lang)
}

pub fn versions_json(v: &Versions) -> Json {
    let strs = |xs: &[String]| Json::Arr(xs.iter().cloned().map(Json::Str).collect());
    Json::Obj(vec![
        ("ringi".to_string(), Json::Str(env!("CARGO_PKG_VERSION").to_string())),
        ("file".to_string(), Json::Str(v.file.clone())),
        ("name".to_string(), Json::Str(v.name.clone())),
        ("days".to_string(), Json::Arr(vec![Json::Str(v.asked.0.clone()), Json::Str(v.asked.1.clone())])),
        ("versions".to_string(), Json::Arr(vec![Json::Str(v.from.0.clone()), Json::Str(v.from.1.clone())])),
        ("same_version".to_string(), Json::Bool(v.same_version())),
        ("differs".to_string(), Json::Bool(v.differs())),
        ("requesters".to_string(), Json::Int(v.of as i128)),
        ("changes".to_string(), group_json(&Group { from: v.from.1.clone(), of: v.of, changes: v.changes.clone() }).get("changes").cloned().unwrap_or(Json::Arr(Vec::new()))),
        ("only_before".to_string(), strs(&v.left)),
        ("only_after".to_string(), strs(&v.joined)),
        ("submitted_by".to_string(), v.submitted_by.clone().map_or(Json::Null, Json::Str)),
        ("may_now".to_string(), strs(&v.may_now)),
        ("may_no_longer".to_string(), strs(&v.may_not)),
        (
            "nobody".to_string(),
            Json::Arr(
                v.nobody
                    .iter()
                    .map(|n| {
                        Json::Obj(vec![
                            ("stage".to_string(), Json::Str(n.stage_ascii.clone())),
                            ("stage_written".to_string(), Json::Str(n.stage.clone())),
                            ("spec".to_string(), Json::Str(n.shown_ascii.clone())),
                            ("spec_written".to_string(), Json::Str(n.shown.clone())),
                            ("label".to_string(), Json::Str(n.label.clone())),
                            ("then".to_string(), Json::Str(n.then.key().to_string())),
                            ("before".to_string(), Json::Int(n.before as i128)),
                            ("after".to_string(), Json::Int(n.after as i128)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("in_progress".to_string(), if v.same_version() { Json::Null } else { Json::Str(v.in_progress.clone()) }),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "rule r v1\n\nenum a_by = x \"x\"\n\n\noutputs\n  a : a_by\n\ntable a\n| -> a |\n| x    |\n";
    const NEW: &str = "rule r v1\n\nenum a_by = x \"x\"\nimport enum kind from \"choices.rule\"\n\ninputs\n  amount : money[JPY]  range >=1JPY <=10JPY\n  kind   : kind\n\noutputs\n  a : a_by?\n";

    /// An input the other route reads is declared in a route that does not read it: under its
    /// `inputs`, or in a new `inputs` before its `outputs`; one typed with an enum is not.
    #[test]
    fn an_input_the_other_route_reads_is_declared_where_it_is_not() {
        let (old, new) = (input_lines(OLD), input_lines(NEW));
        assert!(old.is_empty());
        assert_eq!(new.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["amount", "kind"]);
        assert!(!enum_typed(&new[0].1, NEW) && enum_typed(&new[1].1, NEW));
        let added = with_inputs(OLD, &[new[0].1.clone()]);
        assert!(added.contains("\ninputs\n  amount : money[JPY]  range >=1JPY <=10JPY\n\noutputs\n"), "{added}");
        let more = with_inputs(NEW, &["  days : int".to_string()]);
        assert_eq!(input_lines(&more).iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["amount", "kind", "days"]);
    }

    /// Two routes that cannot be given the same requests are said by the fields each reads that
    /// the other does not, in both languages, with no word of a list that is not there.
    #[test]
    fn routes_that_read_apart_are_said_by_their_fields() {
        let declared = vec![
            ritsu_ports::DiffDeclared { what: "input_added".into(), name: "region_value".into(), old: None, new: None },
            ritsu_ports::DiffDeclared { what: "output_added".into(), name: "review".into(), old: None, new: None },
        ];
        let mut names = (crate::lower::RouteNames::default(), crate::lower::RouteNames::default());
        names.1.inputs.push((vec!["region".into()], "region_value".into(), crate::lower::words::ColKind::Bool));
        let en = reads_apart(&declared, &names, Lang::En);
        assert_eq!(en, "only the new version's route reads `region`: no request can be given to both, so the route is not compared case by case, but line by line as written");
        let ja = reads_apart(&declared, &names, Lang::Ja);
        assert_eq!(ja, "新しい版の経路だけが `region` を読みます。同じ申請を両方に渡せないので、経路は場合ごとには比べず、書いてある行どうしを比べます");
        assert!(!reads_apart(&[], &names, Lang::En).contains("above") && !reads_apart(&[], &names, Lang::Ja).contains("上の"));
    }
}
