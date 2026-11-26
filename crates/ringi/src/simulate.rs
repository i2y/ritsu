//! `ringi simulate` (DESIGN 10.3): a request, a requester (or every one), the values of its form
//! and a day, and who answers each stage of the route in the version of the organization in effect
//! that day, with the deadline and the reminder of the stage the ticket starts at.
//!
//! The stages come from the lowered route (`route.rule`, through `Rules::eval`), with the form's
//! computations first (`form.rule`) and the outputs of the rules the route reads; the deadlines from
//! the lowered `terms.cal` (`Dates::eval`), the day given as the day the first stage starts. Without
//! `--form`, every stage the route can come to is shown with every way to choose it can name. Who
//! answers is solved as the check solves it ([`crate::org::resolve`]), the approve proxies in effect
//! that day beside them, and what a stage that goes up when it is late does where it has nowhere to
//! go up to ([`listing::nowhere`]). With `submitted by` (DESIGN 5.13), a requester who may not
//! submit the request is refused, and every requester is every one who may.

use crate::ast::{At, AtTop, Deadline, Deadlines, File, FileKind, Route, Spec, WhenEmpty};
use crate::check::{Outcome, Run};
use crate::lower::{Lowered, Part};
use crate::names::Scope;
use crate::org::listing::{self, Occ, Once, Out};
use crate::org::resolve::{Found, Requester, Solver, Up};
use crate::org::{Data, Org, ProxyKind, Version};
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text};
use ritsu_ports::{Value, Values};
use std::rc::Rc;

/// What `simulate` is asked.
pub struct Ask {
    /// The requester's ID; None with `every`.
    pub requester: Option<String>,
    pub every: bool,
    /// `--form`, as written.
    pub form: Option<String>,
    /// The day the request is made, and its first stage starts.
    pub on: On,
}

/// The day `simulate` is asked of (DESIGN 10.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum On {
    /// The day `--on` gives.
    Day(i64),
    /// Today, at the instant given in seconds since 1970-01-01T00:00:00Z: the day it is in the UTC
    /// offset of the calendars the request uses, as their deadlines are read.
    Now(i64),
}

/// How today was read, without `--on`: in the UTC offset (minutes east of UTC) the calendars the
/// request uses say, with their aliases; in UTC, with no aliases, where none says one.
#[derive(Clone, Debug, PartialEq)]
pub struct Today {
    pub offset: i32,
    /// The aliases as written, which the sentence says.
    pub calendars: Vec<String>,
    /// The aliases in ASCII, which the JSON takes.
    pub ascii: Vec<String>,
}

/// What the form computes for a field.
#[derive(Clone, Debug, PartialEq)]
pub struct Computed {
    /// The field in ASCII, and as written.
    pub name: String,
    pub written: String,
    /// The value as the JSON of a ticket writes it (a number counted in its field's step), and as
    /// the request writes it (`1_200_000JPY`).
    pub value: Value,
    pub shown: String,
}

/// The day the instant `now` (seconds since 1970-01-01T00:00:00Z) is in, at `offset` minutes east of
/// UTC.
pub fn day_at(now: i64, offset: i32) -> i64 {
    (now + i64::from(offset) * 60).div_euclid(86_400)
}

/// An offset as a calendar writes it: `+09:00`, `-05:30`, `+00:00`.
pub fn offset_text(offset: i32) -> String {
    let sign = if offset < 0 { '-' } else { '+' };
    let m = offset.unsigned_abs();
    format!("{sign}{:02}:{:02}", m / 60, m % 60)
}

/// Today for the request, at the instant `now`: in the offset its calendars say (the request's own
/// `use calendar` lines and its shared route's), in UTC where none says one. Calendars that say
/// different offsets read different days, and which is the requester's is not written: that is
/// refused, for `--on` to give the day.
fn today(cals: &[crate::lower::UsedCal], now: i64) -> Result<(i64, Today), Text> {
    let said: Vec<(&str, i32)> = cals.iter().filter_map(|c| c.offset.map(|o| (c.alias.as_str(), o))).collect();
    let Some(&(_, offset)) = said.first() else {
        return Ok((day_at(now, 0), Today { offset: 0, calendars: Vec::new(), ascii: Vec::new() }));
    };
    if said.iter().any(|(_, o)| *o != offset) {
        let shown: Vec<String> = said.iter().map(|(a, o)| format!("`{a}` {}", offset_text(*o))).collect();
        let (ja, en) = (shown.join("、"), shown.join(", "));
        return Err(tr!(
            "申請が使うカレンダーの UTC オフセットが違い（{ja}）、今日がどの日かが決まりません。`--on <日付>` で日を選んでください",
            "the calendars the request uses say different UTC offsets ({en}), and which day today is is not decided: give the day with `--on <date>`"
        ));
    }
    let ascii = cals.iter().filter(|c| c.offset.is_some()).map(|c| c.ascii.clone()).collect();
    Ok((day_at(now, offset), Today { offset, calendars: said.iter().map(|(a, _)| a.to_string()).collect(), ascii }))
}

/// A way to choose of a stage, solved for a requester.
#[derive(Clone, Debug, PartialEq)]
pub struct WaySim {
    pub way: String,
    /// The row of a master, the value of a rule's output, it is in this case.
    pub label: Option<Text>,
    pub people: Vec<String>,
    /// Each approve proxy in effect that day, with the person they answer for.
    pub proxies: Vec<(String, String)>,
    /// Why nobody answers, and what happens then.
    pub nobody: Option<Text>,
    /// What happens where nobody answers, in no language and by IDs alone: `up e0102`, `to board:
    /// e0201 e0202`, `skip`, `wait`, `cannot`, or `open` where a person chooses when the ticket
    /// runs. `ringi diff` compares it: a team or a person named otherwise in another version of the
    /// organization does not change who answers.
    pub goes: Option<String>,
    /// The quorum, when the stage waits for more than one approval.
    pub needed: Option<usize>,
    /// What it comes to, as `same approver approves once` reads it.
    pub out: Out,
    /// An agent's way: what it comes to once the agent doubts and the way after `else` answers it,
    /// as `same approver approves once` reads it (DESIGN 5.12); None for a way with no agent.
    pub doubted: Option<Out>,
    /// The stage whose approval does it, with `same approver approves once` (DESIGN 5.12): the one
    /// right before it, or, with `, at the last stage`, the later stage it waits for.
    pub once_with: Option<String>,
    /// `, at the last stage`: `once_with` is a later stage, which it waits for.
    pub once_waits: bool,
    /// `, at the last stage`, without the form: it waits where the later stage comes, and comes to
    /// the same person; and the stages that can be, the last of which that does is the one (it is
    /// `once_with` where they all do).
    pub once_if_it_comes: bool,
    pub once_among: Vec<String>,
    /// Without the form: whether it is done so only where no other stage comes between them.
    pub once_if_nothing_between: bool,
    /// `once_with` holds only once an agent doubts: with `same approver approves once`, the agent of
    /// the stage `once_with`, which the same person answers then; with `, at the last stage`, the
    /// agent of this way, whose stage then waits for `once_with` (DESIGN 5.12).
    pub once_if_doubted: bool,
    /// What the stage does when it is late and has nowhere to go up to (`when late escalate`, DESIGN
    /// 5.10): `it cannot go up, and expires at the deadline`.
    pub late: Option<Text>,
}

/// A stage as it comes to the requester.
#[derive(Clone, Debug, PartialEq)]
pub struct StageSim {
    pub stage: String,
    /// How the stage is written: `manager`, `dept_head and accounting`, `circulate to finance`.
    pub shown: String,
    pub ways: Vec<WaySim>,
    /// The deadline and the reminder of the stage the ticket starts at; how long the others have.
    pub due: Option<Text>,
    /// Whether it may come or not, without `--form`, when a condition decides it.
    pub may: bool,
}

/// What `simulate` comes to for one requester.
#[derive(Clone, Debug)]
pub struct Sim {
    pub requester: Requester,
    pub named: Text,
    pub stages: Vec<StageSim>,
    /// The holds made at `submit` that cannot be made for the requester, whose `submit` the machine
    /// refuses (DESIGN 7.2, the refusal `book`): an argument reads a kind of team the requester has
    /// none of above them.
    pub no_hold: Vec<NoHold>,
}

/// A hold that cannot be made for a requester.
#[derive(Clone, Debug)]
pub struct NoHold {
    /// The hold, as written: `budget.reserve`.
    pub hold: String,
    /// The argument with no value, and what it reads (`requester.department`).
    pub argument: String,
    pub reads: String,
    /// The kind of team it reads above the requester.
    pub kind: String,
}

impl NoHold {
    fn text(&self) -> Text {
        let (hold, argument, reads, kind) = (&self.hold, &self.argument, &self.reads, &self.kind);
        tr!(
            "`submit` は断られます: 仮押さえ `{hold}` の引数 `{argument}` は `{reads}` を読みますが、申請者の上に種類が `{kind}` のチームがありません",
            "`submit` is refused: the hold `{hold}` takes `{argument}` from `{reads}`, and no team of the kind `{kind}` is above the requester"
        )
    }
}

/// What `simulate` answers: the day, the version, and each requester's stages.
pub struct Answer {
    /// The day asked of.
    pub on: i64,
    /// How today was read, without `--on`.
    pub today: Option<Today>,
    /// The organization the request read.
    pub data: Rc<Data>,
    /// The first day of the version in effect on the day asked.
    pub version: i64,
    pub sims: Vec<Sim>,
    /// What the form comes to (its computations), when one is given.
    pub computed: Vec<Computed>,
    /// Whether the stages are the ones the route comes to for `--form` (else every one it can).
    pub chosen: bool,
    /// With `submitted by`: the line as written after it, and the people of the version who may not
    /// submit the request, by their IDs.
    pub submitted: Option<(String, Vec<String>)>,
    /// Whether every requester is asked for.
    pub every: bool,
    /// The ASCII names the JSON takes.
    pub names: crate::engine::Names,
}

impl Answer {
    /// The version in effect on the day asked.
    pub fn org_version(&self) -> Option<&Version> {
        match self.data.as_ref() {
            Data::Org(o) => o.versions.iter().find(|v| v.from == self.version),
            _ => None,
        }
    }
}

/// The values of the form, by the lowered names, from `--form`: read and held to their fields as
/// every value of a form is ([`crate::taken`]).
fn values(s: &Scope, written: &str, when: &crate::taken::When) -> Result<Values, Text> {
    let read = crate::parse::form_values("--form", written).map_err(|ds| {
        let d = ds.into_iter().find(|d| d.is_error());
        let m = d.map(|d| d.message).unwrap_or_default();
        tr!("`--form` が読めません: {}", "`--form` does not read: {}", m.ja; m.en)
    })?;
    crate::taken::form(s, &written.replace('\n', " "), &read, when).map_err(|(_, no)| no.text)
}

/// The stages the route comes to for the values of the form (each stage, and the value its output
/// takes in the lowered route; None for a stage that is not there), and what the form computes, as
/// the lowered files answer ([`crate::engine::eval`], which a ticket asks too). A value of a field
/// the form's `shown when` hides for the values is refused in the machine's words (the refusal
/// `form`; [`crate::taken::hidden`]).
fn routed(s: &Scope, run: &Run, lowered: &Lowered, given: &Values) -> Result<(Vec<(String, Option<String>)>, Vec<Computed>), Text> {
    use crate::engine::eval::{self, Unrouted};
    let given: Values = given.iter().filter(|(_, v)| *v != Value::None).cloned().collect();
    let r = ritsu_base::fs::with(eval::overlay(lowered), || eval::routed(s, &run.suite, lowered, &given)).map_err(|e| match e {
        Unrouted::Unjoined(tool) => tr!("{tool} がつながっていません", "{tool} is not joined"),
        Unrouted::Unread(m) => tr!("下ろした規則を読めません: {}", "a lowered rule does not read: {}", m.ja; m.en),
        Unrouted::Lacks { name, route: true } => tr!("経路が読む `{name}` が、`--form` にありません", "the route reads `{name}`, which `--form` does not give"),
        Unrouted::Lacks { name, route: false } => tr!("`--form` に `{name}` がありません", "`--form` does not give `{name}`"),
        Unrouted::Date(field, t) => tr!("`{field}` を数えられません: {}", "`{field}` cannot be counted: {}", t.ja; t.en),
        Unrouted::Outside(t, _) => t,
        Unrouted::Rule(e, what) => failed(&e, what),
    })?;
    // a value given for a field the form hides for the values is refused, as the machine refuses it
    let ff = lowered.files.iter().find(|f| f.part == crate::lower::Part::Form);
    let named = |want: &str| ff.and_then(|f| f.renamed.iter().find(|(_, w)| w == want).map(|(g, _)| g.clone())).unwrap_or_else(|| want.to_string());
    if let Some(t) = crate::taken::hidden(s, &given, &r.outs, &named) {
        return Err(t.text);
    }
    let computed = r
        .computed
        .iter()
        .filter_map(|(n, v)| s.fields().iter().find(|f| f.name.ascii() == n).map(|f| Computed { name: n.clone(), written: f.name.text.clone(), value: v.clone(), shown: crate::taken::shown(f, v) }))
        .collect();
    Ok((r.stages, computed))
}

/// Why the rule `what` is lowered to gives no answer for the values: one it does not take (a
/// value of `--form` outside its field's range), none or two (a rule that passes its check never
/// does this), or the lowered file unread.
fn failed(e: &ritsu_ports::RuleError, what: Text) -> Text {
    use ritsu_ports::RuleError;
    let w = &what;
    match e {
        RuleError::Input(t) => tr!("{}が、`--form` の値を受け取りません: {}", "{} does not take the values of `--form`: {}", w.ja, t.ja; w.en, t.en),
        RuleError::Contradiction(t) => tr!("{}の答えが一つに決まりません: {}", "{} gives no one answer: {}", w.ja, t.ja; w.en, t.en),
        RuleError::Unread(said) => {
            let first = said.first().map(|x| x.message.clone()).unwrap_or_default();
            tr!("{}を下ろしたファイルを読めません: {}", "what {} is lowered to cannot be read: {}", w.ja, first.ja; w.en, first.en)
        }
    }
}

/// The deadlines of the request: the `every stage` block, and each stage line's own.
fn deadlines_of<'f>(s: &'f Scope) -> (Option<&'f Deadlines>, Vec<(String, &'f Deadlines)>) {
    let mut every = s.file.every_stage.as_ref();
    let mut own = Vec::new();
    let mut add = |f: &'f File| {
        if let Some(Route::Stages(ls)) = &f.route {
            for l in ls {
                if let Some(d) = &l.deadlines {
                    own.push((l.name.text.clone(), d));
                }
            }
        }
    };
    add(s.file);
    for sh in s.shared_routes().into_iter().flatten() {
        if every.is_none() {
            every = sh.every_stage.as_ref();
        }
        add(sh);
    }
    (every, own)
}

fn due_line(d: Option<&Deadlines>) -> Option<(u32, At)> {
    d?.lines.iter().find_map(|l| match l {
        Deadline::Due { days, at, .. } => Some((*days, *at)),
        _ => None,
    })
}

/// The time of day of the reminder: `09:00`.
fn remind_at(d: Option<&Deadlines>) -> Option<u32> {
    d?.lines.iter().find_map(|l| match l {
        Deadline::Remind { at, .. } => Some(*at),
        _ => None,
    })
}

fn at_text(at: At) -> Text {
    match at {
        At::EndOfDay => tr!(" の終わり", " at the end of the day"),
        At::Time(t) => {
            let t = crate::lex::time_text(t);
            tr!(" {t}", " at {t}")
        }
    }
}

/// The deadline and the reminder of the first stage, through the lowered `terms.cal`: the
/// reminder where it comes before the deadline, as the machine sets it (DESIGN 5.10).
fn first_due(s: &Scope, run: &Run, lowered: &Lowered, stage: &str, on: i64) -> Option<Text> {
    let dates = run.suite.dates.as_ref()?;
    let (every, own) = deadlines_of(s);
    let mine = own.iter().find(|(n, _)| n == stage).map(|(_, d)| *d);
    let (_, at) = due_line(mine).or_else(|| due_line(every))?;
    // the stage's own reminder, else the one of every stage (as its date is found below)
    let r_at = remind_at(mine).or_else(|| remind_at(every));
    let ascii = s.stages().unwrap_or_default().iter().find(|n| n.text == stage).map(|n| n.ascii().to_string()).unwrap_or_else(|| stage.to_string());
    let tf = lowered.files.iter().find(|f| matches!(f.part, Part::Terms(_)))?;
    let got = lowered.fs(|| dates.eval(&lowered.path(tf), &[("started".to_string(), on)])).ok()?;
    // a name koyomi reserves is written another way in the file
    let day = |name: &str| {
        let written = tf.renamed.iter().find(|(_, want)| want == name).map(|(given, _)| given.as_str()).unwrap_or(name);
        got.iter().find(|(n, _)| n == written).and_then(|(_, v)| match v {
            ritsu_ports::DateValue::Day(d) => Some(*d),
            _ => None,
        })
    };
    let due = day(&format!("{ascii}_due")).or_else(|| day("due"))?;
    let due_minutes = match at {
        At::EndOfDay => 1440,
        At::Time(t) => t,
    };
    let remind = day(&format!("{ascii}_remind")).or_else(|| day("remind")).filter(|r| (*r, r_at.unwrap_or(0)) < (due, due_minutes));
    let (dd, a) = (crate::lex::date_text(due), at_text(at));
    let r = remind.map(|r| match r_at {
        Some(t) => format!("{} {}", crate::lex::date_text(r), crate::lex::time_text(t)),
        None => crate::lex::date_text(r),
    });
    Some(match r {
        Some(r) => tr!("期限は {dd}{}、催促は {r}", "due {dd}{}, reminder on {r}", a.ja; a.en),
        None => tr!("期限は {dd}{}", "due {dd}{}", a.ja; a.en),
    })
}

/// How long a later stage has: `3 business days from when it starts`.
fn later_due(s: &Scope, stage: &str) -> Option<Text> {
    let (every, own) = deadlines_of(s);
    let mine = own.iter().find(|(n, _)| n == stage).map(|(_, d)| *d);
    let (days, at) = due_line(mine).or_else(|| due_line(every))?;
    Some(match at {
        At::EndOfDay => tr!("期限は、始まってから {days} 営業日目の終わり", "due {days} business days after it starts, at the end of the day"),
        At::Time(t) => {
            let t = crate::lex::time_text(t);
            tr!("期限は、始まってから {days} 営業日目の {t}", "due {days} business days after it starts, at {t}")
        }
    })
}

/// The row a lookup way looks up for the values of the form, when the form gives the field it looks
/// up by; None when it gives no such value (every row is a case then).
fn looked_up(sv: &Solver, o: &Occ, way: &str, given: &Values) -> Option<String> {
    let w = Solver::who(o.file, way).or_else(|| Solver::who(sv.s.file, way))?;
    let crate::ast::How::Lookup { by, .. } = &w.how else { return None };
    let ascii = sv.s.field(&by.text).map(|f| f.name.ascii().to_string()).unwrap_or_else(|| by.text.clone());
    given.iter().find(|(n, _)| *n == ascii).and_then(|(_, v)| match v {
        Value::Enum(k) | Value::Str(k) => Some(k.clone()),
        _ => None,
    })
}

/// One stage solved for a requester; with the values of the form, a lookup is of the row it picks.
fn stage_sim(sv: &Solver, org: &Org, v: &Version, o: &Occ, on: i64, request: &File, given: Option<&Values>) -> StageSim {
    let mut ways = Vec::new();
    let shown = o.spec.shown();
    let parts: Vec<(String, Option<crate::ast::Quorum>, Vec<crate::org::resolve::Case>)> = match o.spec {
        Spec::Circulate(w) => vec![(w.text.clone(), None, sv.way(o.file, &w.text))],
        _ => listing::stage_cases(sv, o),
    };
    for (name, q, mut cases) in parts {
        if let Some(given) = given {
            cases.retain(|c| c.row.is_none() || looked_up(sv, o, &c.way, given).is_none_or(|k| c.row.as_deref() == Some(k.as_str())));
        }
        for c in cases {
            let out = if matches!(o.spec, Spec::Circulate(_)) { Out::Pass } else { listing::out_of(sv, o, q, &c) };
            // late with nowhere to go up to: what it does then, once for each way it does it
            let mut late: Vec<Text> = Vec::new();
            if !matches!(o.spec, Spec::Circulate(_)) {
                for n in listing::nowhere(sv, o, &name, &c) {
                    if !late.contains(&n.what) {
                        late.push(n.what);
                    }
                }
            }
            let late = (!late.is_empty()).then(|| Text::new(late.iter().map(|t| t.ja.clone()).collect::<Vec<_>>().join("。"), late.iter().map(|t| t.en.clone()).collect::<Vec<_>>().join("; ")));
            let doubted = c.agent.then(|| listing::out_doubted(sv, o, q, &c));
            let mut w = WaySim { way: c.way.clone(), label: c.label.clone(), people: Vec::new(), proxies: Vec::new(), nobody: None, goes: None, needed: None, out, doubted, once_with: None, once_waits: false, once_if_it_comes: false, once_among: Vec::new(), once_if_nothing_between: false, once_if_doubted: false, late };
            match &c.found {
                Found::People(ps) => {
                    w.people = ps.clone();
                    w.needed = q.map(|q| listing::needed(q, ps.len()));
                    for p in ps {
                        // a proxy who is the requester, or the one who made the request, does not
                        // answer it where self approval is not allowed, as the machine refuses them
                        // (DESIGN 5.5, 5.8)
                        for x in org.proxies.iter().filter(|x| x.kind == ProxyKind::Approve && &x.person == p && !sv.left_out(&x.proxy) && (x.on(on, &request.name.text) || x.on(on, request.name.ascii()))) {
                            w.proxies.push((x.proxy.clone(), p.clone()));
                        }
                    }
                }
                Found::Open => {
                    w.nobody = Some(tr!("その時に人が決めます", "a person chooses when the ticket runs"));
                    w.goes = Some("open".to_string());
                }
                Found::Nobody(why, up) => {
                    w.goes = Some(goes(sv, o, why, up));
                    let mut t = why_said(v, why);
                    let then = match &o.empty {
                        WhenEmpty::Stay => tr!("チケットは、管理する人が承認者を足すまで待ちます", "the ticket waits until an administrator adds an approver"),
                        WhenEmpty::Skip => tr!("ステップは飛ばされます", "the stage is skipped"),
                        WhenEmpty::Escalate(_) if *up == Up::Cannot && matches!(why, crate::org::resolve::Why::PersonNotIn(_) | crate::org::resolve::Why::TeamNotIn(_)) => tr!("上へ回せません", "it cannot go up"),
                        WhenEmpty::Escalate(top) => match (sv.up(up), top) {
                            (Some(h), _) => {
                                let h = v.who(&h);
                                tr!("{}へ回ります", "it goes up to {}", crate::org::ja_word(&h.ja); h.en)
                            }
                            // past the top: skipped with `then skip`, to the way `then` names, else it
                            // waits (DESIGN 5.4); where the reason says nobody is above, it is not
                            // said again
                            (None, Some(AtTop::To(tw))) => {
                                let said = matches!(why, crate::org::resolve::Why::NoneAbove | crate::org::resolve::Why::NoTeamOfKind(_));
                                let goes = at_top_said(sv, v, o, &tw.text);
                                if said { goes } else { tr!("上に回せる人がいないので、{}", "nobody is above to go up to, and {}", goes.ja; goes.en) }
                            }
                            (None, _) => {
                                let said = matches!(why, crate::org::resolve::Why::NoneAbove | crate::org::resolve::Why::NoTeamOfKind(_));
                                match (top == &Some(AtTop::Skip), said) {
                                    (true, true) => tr!("ステップは飛ばされます", "the stage is skipped"),
                                    (true, false) => tr!("上に回せる人がいないので、ステップは飛ばされます", "nobody is above to go up to, and the stage is skipped"),
                                    (false, true) => tr!("チケットは、管理する人が承認者を足すまで待ちます", "the ticket waits until an administrator adds an approver"),
                                    (false, false) => tr!(
                                        "上に回せる人がいないので、チケットは、管理する人が承認者を足すまで待ちます",
                                        "nobody is above to go up to, and the ticket waits until an administrator adds an approver"
                                    ),
                                }
                            }
                        },
                    };
                    t = tr!("{}。{}", "{}; {}", t.ja, then.ja; t.en, then.en);
                    w.nobody = Some(t);
                }
            }
            ways.push(w);
        }
    }
    StageSim { stage: o.stage.clone(), shown, ways, due: None, may: false }
}

/// What happens where a way finds nobody, by IDs alone ([`WaySim::goes`]): the same branches as
/// the sentence `stage_sim` writes.
fn goes(sv: &Solver, o: &Occ, why: &crate::org::resolve::Why, up: &Up) -> String {
    use crate::org::resolve::Why;
    match &o.empty {
        WhenEmpty::Stay => "wait".to_string(),
        WhenEmpty::Skip => "skip".to_string(),
        WhenEmpty::Escalate(_) if *up == Up::Cannot && matches!(why, Why::PersonNotIn(_) | Why::TeamNotIn(_)) => "cannot".to_string(),
        WhenEmpty::Escalate(top) => match (sv.up(up), top) {
            (Some(h), _) => format!("up {h}"),
            (None, Some(AtTop::To(tw))) => {
                let cases = sv.way(o.empty_in, &tw.text);
                let people: Vec<String> = cases
                    .iter()
                    .filter_map(|c| match &c.found {
                        Found::People(ps) => Some(format!("{}{}", c.label.as_ref().map(|l| format!("{} ", l.en)).unwrap_or_default(), ps.join(" "))),
                        Found::Open => Some("open".to_string()),
                        Found::Nobody(..) => None,
                    })
                    .collect();
                if people.is_empty() { "wait".to_string() } else { format!("to {}: {}", tw.text, people.join("; ")) }
            }
            (None, Some(AtTop::Skip)) => "skip".to_string(),
            (None, _) => "wait".to_string(),
        },
    }
}

/// Why a way finds nobody, as `simulate` says it: `no head is above the requester`.
pub(crate) fn why_said(v: &Version, why: &crate::org::resolve::Why) -> Text {
    use crate::org::resolve::Why;
    match why {
        Why::PersonNotIn(id) => tr!("{id} は組織にいません", "{id} is not in the organization"),
        Why::TeamNotIn(id) => tr!("チーム `{id}` は組織にありません", "the team `{id}` is not in the organization"),
        Why::HeadEmpty(t) => {
            let t = v.team_named(t);
            tr!("{}の長が空いています", "the head of {} is empty", crate::org::ja_word(&t.ja); t.en)
        }
        Why::OnlyRequester(_) => tr!("当たるのは申請者だけです", "it comes to the requester alone"),
        Why::NoTeamOfKind(k) => tr!("種類 `{k}` のチームが、申請者の上にありません", "no team of the kind `{k}` is above the requester"),
        Why::NoneAbove => tr!("申請者の上に長がいません", "no head is above the requester"),
        Why::NoMembers(t) => {
            let t = v.team_named(t);
            tr!("{}に人がいません", "{} has no member", crate::org::ja_word(&t.ja); t.en)
        }
    }
}

/// Where a stage goes where nobody is above, when it says `then <way>` (DESIGN 5.4): `it goes to
/// `board`: one of e0201 (Director 1), …`, or that the way finds nobody either and the ticket waits.
fn at_top_said(sv: &Solver, v: &Version, o: &Occ, way: &str) -> Text {
    let said: Vec<Text> = sv
        .way(o.empty_in, way)
        .iter()
        .map(|c| {
            let label = c.label.as_ref().map(|l| tr!("（{}）", " ({})", l.ja; l.en)).unwrap_or_default();
            match &c.found {
                Found::People(ps) => {
                    let people: Vec<Text> = ps.iter().map(|p| v.who(p)).collect();
                    let ja = people.iter().map(|t| t.ja.clone()).collect::<Vec<_>>().join("、");
                    let en = people.iter().map(|t| t.en.clone()).collect::<Vec<_>>().join(", ");
                    // (`board` の e0201（取締役 1）へ: a space after a name, none after a bracket)
                    let of = if label.ja.is_empty() { " の" } else { "の" };
                    if ps.len() == 1 {
                        tr!("`{way}`{}{of} {}へ回ります", "it goes to `{way}`{}: {en}", label.ja, crate::org::ja_word(&ja); label.en)
                    } else {
                        tr!("`{way}`{}{of} {}のだれかへ回ります", "it goes to `{way}`{}: one of {en}", label.ja, crate::org::ja_word(&ja); label.en)
                    }
                }
                Found::Nobody(why, _) => {
                    let w = why_said(v, why);
                    tr!(
                        "`{way}`{} へ回しますが、{}。チケットは、管理する人が承認者を足すまで待ちます",
                        "it goes to `{way}`{}, but {}; the ticket waits until an administrator adds an approver",
                        label.ja, w.ja; label.en, w.en
                    )
                }
                Found::Open => tr!("`{way}`{} へ回ります。その時に人が決めます", "it goes to `{way}`{}; a person chooses when the ticket runs", label.ja; label.en),
            }
        })
        .collect();
    Text::new(said.iter().map(|t| t.ja.clone()).collect::<Vec<_>>().join("。"), said.iter().map(|t| t.en.clone()).collect::<Vec<_>>().join("; "))
}

/// What `simulate` answers for the checked request `o`.
pub fn simulate(o: &Outcome, run: &Run, ask: &Ask) -> Result<Answer, Text> {
    let Some(file) = &o.file else { return Err(tr!("検査が通りません", "the file does not pass its check")) };
    if file.kind != FileKind::Request {
        return Err(tr!("`simulate` が試すのは申請の種類のファイルです", "`simulate` tries a request's file"));
    }
    let src = ritsu_base::fs::read_to_string(&o.path).map_err(|e| {
        let e = e.to_string();
        tr!("読めません: {e}", "cannot read it: {e}")
    })?;
    let mut said = Vec::new();
    let got = crate::names::read_uses(file, &o.path, &src, run, &mut said);
    let s = Scope { file, path: &o.path, src: &src, got };
    let data = s.org_rc().ok_or_else(|| tr!("申請の種類が組織を読みません", "the request reads no organization"))?;
    let org = match data.as_ref() {
        Data::Org(org) => org,
        Data::NoOrg(none) => return Err(tr!("{}（{}）", "{} ({})", none.what.ja, none.why.ja; none.what.en, none.why.en)),
        Data::Masters(_) => return Err(Text::default()),
    };
    let (on, today) = match ask.on {
        On::Day(d) => (d, None),
        On::Now(now) => {
            let (d, t) = today(&crate::lower::context(&s, &run.suite).calendars, now)?;
            (d, Some(t))
        }
    };
    let Some(v) = org.in_effect(on) else {
        let d = crate::lex::date_text(on);
        return Err(tr!("{d} に効いている組織の版がありません", "no version of the organization is in effect on {d}"));
    };
    if let Some(u) = &o.usable {
        let d = crate::lex::date_text(on);
        if let Some(f) = u.from.as_ref().filter(|f| on < f.day) {
            let fd = crate::lex::date_text(f.day);
            return Err(tr!("この申請の種類は {fd} から使います。{d} には使えません", "the request is used from {fd}, not on {d}"));
        }
        if let Some(t) = u.through.as_ref().filter(|t| on > t.day) {
            let td = crate::lex::date_text(t.day);
            return Err(tr!("この申請の種類は {td} まで使えます。{d} には使えません", "the request can be used through {td}, not on {d}"));
        }
    }
    // the stages: as the route comes to them for the form, or every one it can come to
    let paths = listing::paths(&s);
    let mut given: Option<Values> = None;
    let (chosen, computed): (Option<Vec<(String, Option<String>)>>, Vec<Computed>) = match (&ask.form, &o.lowered) {
        (Some(w), Some(l)) => {
            let calendars = crate::engine::calendars(&s, &run.suite);
            let g = values(&s, w, &crate::taken::When { day: Some(on), versions: Some(vec![v]), calendars: &calendars, rows: None })?;
            let (r, c) = routed(&s, run, l, &g)?;
            given = Some(g);
            (Some(r), c)
        }
        (Some(_), None) => return Err(tr!("下ろした経路がありません", "the route is not lowered")),
        (None, _) => (None, Vec::new()),
    };
    let once = listing::once(file);
    let submit = crate::org::submit::of(file);
    let may = |id: &str| submit.as_ref().is_none_or(|l| l.may(org, v, id));
    let mut requesters: Vec<Requester> = match (&ask.requester, ask.every) {
        (_, true) => listing::requesters(org, v, file),
        (Some(id), false) => {
            if v.person(id).is_none() {
                let d = v.day();
                return Err(tr!("{id} は、{d} からの組織にいません", "{id} is not in the organization from {d}"));
            }
            if let Some(l) = submit.as_ref().filter(|_| !may(id)) {
                return Err(l.refused(v, id, &o.path));
            }
            vec![Requester { id: id.clone(), by: None }]
        }
        // the command stops before this (ritsu's DESIGN 6.2, item 10); a caller of the library hears it alike
        (None, false) => return Err(crate::cli::table().one_of("simulate", &["--requester", "--every-requester"])),
    };
    requesters.retain(|r| may(&r.id));
    let submitted = submit.as_ref().map(|l| (l.shown(), v.people.iter().filter(|p| !may(&p.id)).map(|p| p.id.clone()).collect::<Vec<_>>()));
    let allowed = listing::self_approval(file);
    let ticket_lines: &[crate::ast::TicketLine] = file.ticket.as_ref().map(|t| t.lines.as_slice()).unwrap_or(&[]);
    let gaps = crate::ticket::hold_gaps(ticket_lines);
    let mut sims = Vec::new();
    for r in &requesters {
        let sv = Solver { s: &s, org, v, requester: r, self_approval: allowed };
        let mut stages: Vec<StageSim> = Vec::new();
        match &chosen {
            Some(ch) => {
                for (stage, val) in ch {
                    let Some(val) = val else { continue };
                    let occ = paths.iter().flatten().find(|o| &o.stage == stage && crate::lower::words::spec_value(&s, o.spec).is_some_and(|(x, _)| &x == val));
                    let occ = occ.or_else(|| paths.iter().flatten().find(|o| &o.stage == stage && matches!(o.spec, Spec::RuleOutput(_))));
                    let Some(occ) = occ else { continue };
                    let mut st = stage_sim(&sv, org, v, occ, on, file, given.as_ref());
                    if let Spec::RuleOutput(_) = occ.spec {
                        st.ways.retain(|w| crate::lower::words::ascii_of_who(&s, &w.way) == *val || &w.way == val);
                        st.shown = val.clone();
                    }
                    stages.push(st);
                }
            }
            None => {
                let mut seen: Vec<(String, String)> = Vec::new();
                for occ in paths.iter().flatten() {
                    let key = (occ.stage.clone(), occ.spec.shown());
                    if seen.contains(&key) || matches!(occ.spec, Spec::Empty | Spec::Route(_)) {
                        continue;
                    }
                    seen.push(key);
                    let mut st = stage_sim(&sv, org, v, occ, on, file, None);
                    st.may = true;
                    stages.push(st);
                }
                // in the order of the stages
                let order: Vec<String> = s.stages().unwrap_or_default().iter().map(|n| n.text.clone()).collect();
                stages.sort_by_key(|st| order.iter().position(|o| *o == st.stage).unwrap_or(usize::MAX));
            }
        }
        match (once, chosen.is_some()) {
            (Once::No, _) => {}
            (Once::Before, true) => once_on_the_route(&mut stages),
            (Once::Before, false) => once_on_each_path(&sv, &paths, &mut stages),
            (Once::Last, true) => last_on_the_route(&mut stages),
            (Once::Last, false) => last_on_each_path(&sv, &paths, &mut stages),
        }
        // a stage done with another stage's approval has no clock, and is never late (one done so
        // only once an agent doubts is answered when the agent does not)
        for w in stages.iter_mut().flat_map(|st| st.ways.iter_mut()) {
            if w.once_with.is_some() && !w.once_if_doubted && (chosen.is_some() || !(w.once_if_nothing_between || w.once_if_it_comes)) {
                w.late = None;
            }
        }
        // the deadline of the first stage that waits for a person, and how long the others have; a
        // stage `same approver approves once` does has no clock
        let mut first = true;
        for st in stages.iter_mut() {
            // (a way that waits only where a later stage comes to the same person may be answered here)
            let done = !st.ways.is_empty() && st.ways.iter().all(|w| (w.once_with.is_some() && !w.once_if_doubted && !(w.once_if_it_comes && chosen.is_none())) || w.out == Out::Pass);
            let waits = !st.shown.starts_with("circulate") && !st.shown.starts_with("note") && !done;
            if !waits {
                continue;
            }
            st.due = if first && chosen.is_some() { o.lowered.as_ref().and_then(|l| first_due(&s, run, l, &st.stage, on)) } else { later_due(&s, &st.stage) };
            first = false;
        }
        let no_hold = gaps
            .iter()
            .filter(|g| !crate::ticket::has_kind(v, &r.id, g.kind))
            .map(|g| NoHold { hold: format!("{}.{}", g.alias, g.transfer), argument: g.arg.name.text.clone(), reads: crate::types::written(&src, g.arg.value.span), kind: g.kind.to_string() })
            .collect();
        sims.push(Sim { requester: r.clone(), named: r.named(v), stages, no_hold });
    }
    Ok(Answer { on, today, data: data.clone(), version: v.from, sims, computed, chosen: chosen.is_some(), submitted, every: ask.every, names: crate::engine::Names::of(&s) })
}

/// `same approver approves once` on the route the form comes to (DESIGN 5.12): a part of a stage
/// that one person alone answers is done with the approval of the stage right before it, when that
/// person approved it, every part of it; circulations and stages skipped for lack of an approver are
/// passed over. A stage done so stands for the approval it is done with. Each agent answers as it
/// is asked; where the stage right before is an agent's that the person answers once its agent
/// doubts, the part is done with that approval then ([`WaySim::once_if_doubted`]).
fn once_on_the_route(stages: &mut [StageSim]) {
    // the one person who approved the stage right before, and the stage they approved
    let mut last: Option<(String, String)> = None;
    // the one person who answers every part of the agent's stage right before once its agent
    // doubts, and that stage
    let mut doubted: Option<(String, String)> = None;
    for st in stages.iter_mut() {
        if st.shown.starts_with("circulate ") || st.ways.iter().all(|w| w.out == Out::Pass) {
            continue;
        }
        if st.shown.starts_with("note ") {
            (last, doubted) = (None, None);
            continue;
        }
        let mut by: Option<String> = None;
        let (mut one, mut done) = (true, true);
        for w in st.ways.iter_mut() {
            match &w.out {
                Out::Pass => {}
                Out::By(x) => {
                    match (&last, &doubted) {
                        (Some((p, with)), _) if p == x => w.once_with = Some(with.clone()),
                        (_, Some((p, with))) if p == x => {
                            (w.once_with, w.once_if_doubted) = (Some(with.clone()), true);
                            done = false;
                        }
                        _ => done = false,
                    }
                    one &= by.get_or_insert_with(|| x.clone()) == x;
                }
                Out::Other => (one, done) = (false, false),
            }
        }
        doubted = doubted_by(st).map(|x| (x, st.stage.clone()));
        last = match (one, by) {
            (true, Some(x)) => {
                let with = if done { last.map(|(_, w)| w).unwrap_or_else(|| st.stage.clone()) } else { st.stage.clone() };
                Some((x, with))
            }
            _ => None,
        };
    }
}

/// The one person who answers every part of an agent's stage once its agents doubt: None where a
/// part comes to another, or to others, or no part is an agent's.
fn doubted_by(st: &StageSim) -> Option<String> {
    let mut by: Option<&String> = None;
    let mut agent = false;
    for w in &st.ways {
        agent |= w.doubted.is_some();
        match w.doubted.as_ref().unwrap_or(&w.out) {
            Out::Pass => {}
            Out::By(x) if *by.get_or_insert(x) == x => {}
            _ => return None,
        }
    }
    by.filter(|_| agent).cloned()
}

/// `same approver approves once, at the last stage` on the route the form comes to (DESIGN 5.12): a
/// part of a stage that one person alone answers waits for the last stage after it that the same
/// person alone answers a part of, and is done with the approval given there. An agent's part waits
/// so once its agent doubts, as the way after `else` comes to the person then
/// ([`WaySim::once_if_doubted`]); a stage an agent answers first is waited for by none.
fn last_on_the_route(stages: &mut [StageSim]) {
    let alone: Vec<Vec<String>> = stages.iter().map(|st| st.ways.iter().filter_map(|w| if let Out::By(x) = &w.out { Some(x.clone()) } else { None }).collect()).collect();
    for a in 0..stages.len() {
        let after = stages[a + 1..].iter().map(|st| st.stage.clone()).collect::<Vec<_>>();
        for w in stages[a].ways.iter_mut() {
            let (x, doubted) = match (&w.out, &w.doubted) {
                (Out::By(x), _) => (x.clone(), false),
                (_, Some(Out::By(x))) => (x.clone(), true),
                _ => continue,
            };
            if let Some(k) = alone[a + 1..].iter().rposition(|ps| ps.contains(&x)) {
                (w.once_with, w.once_waits, w.once_if_doubted) = (Some(after[k].clone()), true, doubted);
            }
        }
    }
}

/// `same approver approves once, at the last stage` without the form (DESIGN 10.3): on each path of
/// the route, a way of a stage that one person alone answers waits for the last stage after it that
/// the same person alone answers a part of ([`listing::later`]). Said as sure where it is one stage
/// on every path the way is on, and that stage comes to them whatever the form; else with the
/// stages it can be, the last of which that comes, and to them, is the one. An agent's way waits so
/// once its agent doubts, as the way after `else` comes to the person then.
fn last_on_each_path(sv: &Solver, paths: &[Vec<Occ>], stages: &mut [StageSim]) {
    // each way of a stage as written, by its place among the stage's ways: what each path says
    let mut said: std::collections::BTreeMap<(String, String, usize), Vec<(Vec<String>, bool)>> = std::collections::BTreeMap::new();
    for path in paths {
        let cases: Vec<_> = path.iter().map(|o| listing::stage_cases(sv, o)).collect();
        let seen: Vec<listing::Seen> = path.iter().zip(&cases).map(|(o, cs)| listing::seen_of(sv, o, cs)).collect();
        for (ia, o) in path.iter().enumerate() {
            let listing::Seen::Parts(parts) = &seen[ia] else { continue };
            let mut at = 0;
            for ((_, q, cs), (_, outs)) in cases[ia].iter().zip(parts) {
                for (n, c) in cs.iter().enumerate() {
                    let out = if c.agent { Some(listing::out_doubted(sv, o, *q, c)) } else { outs.get(n).filter(|_| outs.len() == cs.len()).cloned() };
                    if let Some(Out::By(x)) = &out {
                        let (among, sure) = listing::later(path, &seen, ia, x);
                        said.entry((o.stage.clone(), o.spec.shown(), at)).or_default().push((among.iter().map(|k| path[*k].stage.clone()).collect(), sure));
                    }
                    at += 1;
                }
            }
        }
    }
    let order = |name: &String| paths.iter().flatten().position(|o| o.stage == *name);
    for st in stages.iter_mut() {
        for (at, w) in st.ways.iter_mut().enumerate() {
            let Some(rs) = said.get(&(st.stage.clone(), st.shown.clone(), at)) else { continue };
            let mut among: Vec<String> = Vec::new();
            for name in rs.iter().flat_map(|(names, _)| names) {
                if !among.contains(name) {
                    among.push(name.clone());
                }
            }
            among.sort_by_key(|name| order(name));
            let Some(with) = among.last().cloned() else { continue };
            let sure = among.len() == 1 && rs.iter().all(|(names, s)| *s && names.len() == 1);
            (w.once_with, w.once_waits, w.once_if_doubted) = (Some(with), true, w.doubted.is_some());
            if !sure {
                (w.once_if_it_comes, w.once_among) = (true, among);
            }
        }
    }
}

/// `same approver approves once` without the form (DESIGN 10.3): on each path of the route (a row
/// of the table, or every stage line), a way of a stage that one person alone answers is done with
/// the approval of the stage before it that the same person alone approves, every part of it, where
/// the stages between are passed over. Said as sure where it is so on every path the way is on, and
/// else where no other stage comes between them (a stage whose condition decides it may come). A
/// way no path does so is done so once the agents of the stages before it doubt, where they come
/// to the person then: with the approval of the agent's stage, which is never done with an approval
/// before it.
fn once_on_each_path(sv: &Solver, paths: &[Vec<Occ>], stages: &mut [StageSim]) {
    use listing::Before;
    // each way of a stage as written, by its place among the stage's ways: what each path says, as
    // the agents answer and once they doubt
    type Said = std::collections::BTreeMap<(String, String, usize), Vec<Option<(String, bool)>>>;
    let (mut said, mut doubted): (Said, Said) = Default::default();
    for path in paths {
        let cases: Vec<_> = path.iter().map(|o| listing::stage_cases(sv, o)).collect();
        let seen: Vec<listing::Seen> = path.iter().zip(&cases).map(|(o, cs)| listing::seen_of(sv, o, cs)).collect();
        let seen_doubted: Vec<listing::Seen> = path.iter().zip(&cases).map(|(o, cs)| listing::seen_doubted(sv, o, cs)).collect();
        for (ib, o) in path.iter().enumerate() {
            let listing::Seen::Parts(parts) = &seen[ib] else { continue };
            let mut at = 0;
            for ((_, _, cs), (_, outs)) in cases[ib].iter().zip(parts) {
                for (n, _) in cs.iter().enumerate() {
                    if let Some(Out::By(x)) = outs.get(n).filter(|_| outs.len() == cs.len()) {
                        let r = match listing::before(path, &seen, ib, x) {
                            Before::Sure(k) => Some((path[listing::origin(path, &seen, k, x)].stage.clone(), true)),
                            Before::Maybe(k) => Some((path[k].stage.clone(), false)),
                            Before::Not => None,
                        };
                        let d = match listing::before(path, &seen_doubted, ib, x) {
                            _ if r.is_some() => None,
                            Before::Sure(k) => Some((path[k].stage.clone(), true)),
                            Before::Maybe(k) => Some((path[k].stage.clone(), false)),
                            Before::Not => None,
                        };
                        let key = (o.stage.clone(), o.spec.shown(), at);
                        doubted.entry(key.clone()).or_default().push(d);
                        said.entry(key).or_default().push(r);
                    }
                    at += 1;
                }
            }
        }
    }
    for st in stages.iter_mut() {
        for (at, w) in st.ways.iter_mut().enumerate() {
            let key = (st.stage.clone(), st.shown.clone(), at);
            let Some(rs) = said.get(&key) else { continue };
            let (rs, if_doubted) = match (rs.iter().flatten().next(), doubted.get(&key)) {
                (Some(_), _) => (rs, false),
                (None, Some(ds)) => (ds, true),
                (None, None) => continue,
            };
            let Some((with, _)) = rs.iter().flatten().next() else { continue };
            let sure = rs.iter().all(|r| r.as_ref().is_some_and(|(o, s)| *s && o == with));
            w.once_with = Some(with.clone());
            w.once_if_nothing_between = !sure;
            w.once_if_doubted = if_doubted;
        }
    }
}

/// A way solved, as a line shows it: `manager: e1101 (Head of Sales Section 1A)`. `name` is the
/// way as the stage names it; `chosen` says whether the stages are the ones the form comes to.
fn way_text(v: &Version, name: &Text, w: &WaySim, chosen: bool) -> Text {
    let t = way_alone(v, name, w);
    let once = match &w.once_with {
        None => None,
        Some(_) if w.once_if_it_comes && !chosen && w.once_among.len() > 1 => {
            let among = Text::list(&w.once_among.iter().map(|n| Text::same(format!("`{n}`"))).collect::<Vec<_>>());
            Some(tr!("{} のうち、来て同じ人に当たる最後のステップで、一度だけ答えます", "answers once, at the last of {} that comes, and to them", among.ja; among.en))
        }
        Some(s) if w.once_if_it_comes && !chosen => Some(tr!("`{s}` が来て同じ人に当たるなら、そこで一度だけ答えます", "answers once, at `{s}`, where it comes, and to them")),
        Some(s) if w.once_waits => Some(tr!("`{s}` で一度だけ答えます（`same approver approves once, at the last stage`）", "answers once, at `{s}` (`same approver approves once, at the last stage`)")),
        Some(s) if chosen || !w.once_if_nothing_between => Some(tr!("`{s}` の承認で済みます（`same approver approves once`）", "done with the approval of `{s}` (`same approver approves once`)")),
        Some(s) => Some(tr!("あいだにほかのステップが来なければ、`{s}` の承認で済みます", "done with the approval of `{s}` where no other stage comes between them")),
    };
    // only once an agent doubts: the agent of this way, or of the stage whose approval does it
    let t = match once {
        None => t,
        Some(o) if w.once_if_doubted => tr!("{}。エージェントが迷ったときは、{}", "{}; when the agent doubts, {}", t.ja, o.ja; t.en, o.en),
        Some(o) => tr!("{}。{}", "{}; {}", t.ja, o.ja; t.en, o.en),
    };
    match &w.late {
        Some(l) => tr!("{}。期限を過ぎると、{}", "{}; when it is late, {}", t.ja, l.ja; t.en, l.en),
        None => t,
    }
}

/// A way and who answers it, as a line of `simulate` and of `diff` says it: `manager: e1101
/// (Manager of Section 1A)`, in Japanese with the full-width colon a Japanese list writes
/// (`上長：e1101（営業一課長）`).
pub(crate) fn way_alone(v: &Version, name: &Text, w: &WaySim) -> Text {
    let label = w.label.as_ref().map(|l| tr!("（{}）", " ({})", l.ja; l.en)).unwrap_or_default();
    let (nja, nen) = (&name.ja, &name.en);
    if let Some(n) = &w.nobody {
        return tr!("{nja}{}：{}", "{nen}{}: {}", label.ja, n.ja; label.en, n.en);
    }
    let people: Vec<Text> = w.people.iter().map(|p| v.who(p)).collect();
    let ja = people.iter().map(|t| t.ja.clone()).collect::<Vec<_>>().join("、");
    let en = people.iter().map(|t| t.en.clone()).collect::<Vec<_>>().join(", ");
    let k = w.people.len();
    let mut t = match w.needed {
        Some(n) if n > k => tr!(
            "{nja}{}：承認が {n} 人要りますが、答えられるのは {k} 人だけです（{ja}）",
            "{nen}{}: {n} approvals are needed, and only {k} can give one ({en})",
            label.ja;
            label.en
        ),
        Some(n) if w.people.len() > 1 => tr!("{nja}{}：{}のうち {n} 人", "{nen}{}: {n} of {en}", label.ja, crate::org::ja_word(&ja); label.en),
        _ if w.people.len() > 1 => tr!("{nja}{}：{}のだれか", "{nen}{}: one of {en}", label.ja, crate::org::ja_word(&ja); label.en),
        _ => tr!("{nja}{}：{ja}", "{nen}{}: {en}", label.ja; label.en),
    };
    // in brackets of their own, after the people they stand in for
    for (px, p) in &w.proxies {
        let a = v.who(px);
        t = tr!("{}［{p} の代理で {}も］", "{} [{} too, as the proxy of {p}]", t.ja, crate::org::ja_word(&a.ja); t.en, a.en);
    }
    t
}

/// Where the agent's stage goes when the agent doubts, in Japanese: `エージェントが迷えば上長へ`,
/// with a space on each side of a name written in ASCII (`エージェントが迷えば manager へ`).
pub(crate) fn agent_doubts_ja(way: &str) -> String {
    let before = if way.starts_with(|c: char| c.is_ascii()) { " " } else { "" };
    format!("エージェントが迷えば{before}{}へ", crate::org::ja_word(way))
}

/// The name a way is shown by in a stage: the way itself, or the agent's way with the one that
/// answers when it doubts (`check (manager when the agent doubts)`).
pub(crate) fn way_name(st: &StageSim, w: &WaySim) -> Text {
    let (shown, way) = (&st.shown, &w.way);
    if st.ways.len() == 1 && shown != way && !shown.contains(' ') {
        let to = agent_doubts_ja(way);
        tr!("{shown}（{to}）", "{shown} ({way} when the agent doubts)")
    } else {
        Text::same(way.clone())
    }
}

/// The lines of one requester's stages. For the route `--form` comes to: each stage and its way to
/// choose (or how its ways go together, and each way under it), then its deadline. Without
/// `--form`: each stage with every way to choose it can name, each once.
fn stage_lines(v: &Version, sim: &Sim, chosen: bool) -> Vec<Text> {
    let width = sim.stages.iter().map(|s| ritsu_base::text::width(&s.stage)).max().unwrap_or(0);
    let under = " ".repeat(width + 2);
    let mut out = Vec::new();
    let mut last: Option<&str> = None;
    let mut seen: Vec<Text> = Vec::new();
    for st in &sim.stages {
        let pad = " ".repeat(width - ritsu_base::text::width(&st.stage));
        let stage = &st.stage;
        if !chosen {
            if last != Some(stage.as_str()) {
                seen.clear();
            }
            for w in &st.ways {
                let line = way_text(v, &way_name(st, w), w, chosen);
                if seen.contains(&line) {
                    continue;
                }
                let head = if last == Some(stage.as_str()) { under.clone() } else { format!("{stage}{pad}  ") };
                out.push(tr!("{head}{}", "{head}{}", line.ja; line.en));
                seen.push(line);
                last = Some(stage.as_str());
            }
            continue;
        }
        match st.ways.as_slice() {
            [one] => {
                let w = way_text(v, &way_name(st, one), one, chosen);
                out.push(tr!("{stage}{pad}  {}", "{stage}{pad}  {}", w.ja; w.en));
            }
            ways => {
                let shown = &st.shown;
                out.push(tr!("{stage}{pad}  {shown}", "{stage}{pad}  {shown}"));
                for w in ways {
                    let w = way_text(v, &Text::same(w.way.clone()), w, chosen);
                    out.push(tr!("{under}  {}", "{under}  {}", w.ja; w.en));
                }
            }
        }
        if let Some(d) = &st.due {
            out.push(tr!("{under}{}", "{under}{}", d.ja; d.en));
        }
    }
    out
}

/// What `simulate` prints: the day and the version, how today was read without `--on`, then each
/// requester (or each group of requesters who come to the same approvers) with the stages.
pub fn render(path: &str, org_version: &Version, a: &Answer, lang: Lang) -> String {
    let d = crate::lex::date_text(a.on);
    let vd = org_version.day();
    let mut out = String::new();
    out.push_str(&format!("{}\n", tr!("{path}（{d}、{vd} からの組織）", "{path} on {d}, in the organization from {vd}").get(lang)));
    if let Some(t) = &a.today {
        out.push_str(&format!("  {}\n", today_text(t).get(lang)));
    }
    if !a.computed.is_empty() {
        let cs = a.computed.iter().map(|c| format!("{} = {}", c.written, c.shown)).collect::<Vec<_>>().join(", ");
        out.push_str(&format!("  {}\n", tr!("計算：{cs}", "computed: {cs}").get(lang)));
    }
    if !a.chosen {
        out.push_str(&format!(
            "  {}\n",
            tr!(
                "`--form` が無いので、経路が来うるステップを、承認者の決め方ごとに見せます。どれが来るかはフォームの値で決まります",
                "without `--form`: every stage the route can come to, by each way to choose its approver; which come depends on the form"
            )
            .get(lang)
        ));
    }
    // with every requester, the people who may not submit the request
    if let Some((shown, ids)) = a.submitted.as_ref().filter(|(_, ids)| a.every && !ids.is_empty()) {
        let n = ids.len();
        let names: Vec<Text> = ids.iter().map(|id| org_version.who(id)).collect();
        let first: Vec<&Text> = names.iter().take(3).collect();
        let rest = n - first.len();
        let ja = first.iter().map(|t| t.ja.clone()).collect::<Vec<_>>().join("、");
        let en_items: Vec<String> = first.iter().map(|t| t.en.clone()).collect();
        let en = if rest > 0 {
            format!("{} and {rest} more", en_items.join(", "))
        } else if en_items.len() > 1 {
            format!("{} and {}", en_items[..en_items.len() - 1].join(", "), en_items[en_items.len() - 1])
        } else {
            en_items.join("")
        };
        let ja = if rest > 0 { format!("{ja}、ほか {rest} 人") } else { ja };
        let people = if n == 1 { "person" } else { "people" };
        out.push_str(&format!("  {}\n", tr!("出せない人 {n} 人（`submitted by {shown}`）：{ja}", "{n} {people} may not submit it (`submitted by {shown}`): {en}").get(lang)));
    }
    // requesters who come to the same lines, together
    let mut groups: Vec<(Vec<String>, Vec<Text>)> = Vec::new();
    for sim in &a.sims {
        // a requester whose `submit` is refused says so first
        let mut lines: Vec<Text> = sim.no_hold.iter().map(NoHold::text).collect();
        let stages = stage_lines(org_version, sim, a.chosen);
        let none = stages.is_empty();
        lines.extend(stages);
        if none {
            lines.push(tr!("ステップはありません", "no stage"));
        }
        let key: Vec<String> = lines.iter().map(|l| l.get(lang).to_string()).collect();
        let who = sim.named.get(lang).to_string();
        match groups.iter_mut().find(|(_, ls)| ls.iter().map(|l| l.get(lang).to_string()).collect::<Vec<_>>() == key) {
            Some((ws, _)) => ws.push(who),
            None => groups.push((vec![who], lines)),
        }
    }
    for (who, lines) in &groups {
        let n = who.len();
        let head = if n == 1 {
            who[0].clone()
        } else {
            let shown: Vec<String> = who.iter().take(3).cloned().collect();
            let rest = n - shown.len();
            let list = shown.join(if lang == Lang::Ja { "、" } else { ", " });
            if rest > 0 { tr!("{list}、ほか {rest} 人（{n} 人）", "{list} and {rest} more ({n} requesters)").get(lang).to_string() } else { tr!("{list}（{n} 人）", "{list} ({n} requesters)").get(lang).to_string() }
        };
        let head = if n == 1 { tr!("申請者 {head}", "requester {head}") } else { tr!("申請者 {head}", "requesters {head}") };
        out.push_str(&format!("{}\n", head.get(lang)));
        for l in lines {
            out.push_str(&format!("  {}\n", l.get(lang)));
        }
    }
    out
}

/// How today was read: in the offset of the calendars, or in UTC; and that `--on` gives another day.
fn today_text(t: &Today) -> Text {
    let o = offset_text(t.offset);
    if t.calendars.is_empty() {
        return tr!(
            "今日を UTC で読みました（オフセットを書いたカレンダーを使わない申請です）。ほかの日は `--on <日付>` で選べます",
            "today, read in UTC: the request uses no calendar that says its UTC offset; `--on <date>` gives another day"
        );
    }
    let shown: Vec<String> = t.calendars.iter().map(|c| format!("`{c}`")).collect();
    let (ja, en) = (shown.join("、"), shown.join(", "));
    let s = if t.calendars.len() == 1 { "" } else { "s" };
    tr!(
        "今日を、カレンダー {ja} の UTC オフセット {o} で読みました（期限の時刻と同じ読み方です）。ほかの日は `--on <日付>` で選べます",
        "today, read in the UTC offset of the calendar{s} {en} ({o}), as the deadlines are; `--on <date>` gives another day"
    )
}

/// The JSON of `simulate`: `ringi`, `file`, `on`, how the day was decided (`on_given`, and
/// `today_in`, the offset and the calendars today was read in without `--on`), `version`,
/// `computed`, the people of the version who may not submit the request (`may_not_submit`, empty
/// without `submitted by`), and each requester with the holds its `submit` is refused for
/// (`holds_refused`, empty where every hold can be made) and its stages, each stage with its ways.
///
/// A name is its ASCII name, as the JSON of `run`, the vectors and `api` have it (DESIGN 3.4), with
/// the name as written beside it under the same key and `_written` (`stage` and `stage_written`);
/// a value the form computes is the value a ticket's JSON writes, with the value as the request
/// writes it beside it (`value` and `value_written`). What is a sentence for a person (`due`,
/// `case`, `nobody`, `late`) says the names as written.
pub fn to_json(file: &str, a: &Answer, lang: Lang) -> Json {
    let names = &a.names;
    Json::obj([
        ("ringi", Json::str(env!("CARGO_PKG_VERSION"))),
        ("file", Json::str(file)),
        ("on", Json::str(crate::lex::date_text(a.on))),
        ("on_given", Json::from(a.today.is_none())),
        (
            "today_in",
            match &a.today {
                Some(t) => Json::obj([("offset", Json::str(offset_text(t.offset))), ("calendars", Json::arr(t.ascii.iter().map(Json::str))), ("calendars_written", Json::arr(t.calendars.iter().map(Json::str)))]),
                None => Json::Null,
            },
        ),
        ("version", Json::str(crate::lex::date_text(a.version))),
        (
            "computed",
            Json::arr(a.computed.iter().map(|c| Json::obj([("name", Json::str(&c.name)), ("written", Json::str(&c.written)), ("value", crate::engine::state::value_json(&c.value)), ("value_written", Json::str(&c.shown))]))),
        ),
        ("may_not_submit", Json::arr(a.submitted.iter().flat_map(|(_, ids)| ids.iter().map(Json::str)))),
        (
            "requesters",
            Json::arr(a.sims.iter().map(|sim| {
                Json::obj([
                    ("requester", Json::str(&sim.requester.id)),
                    ("made_by", Json::opt_str(sim.requester.by.clone())),
                    (
                        "holds_refused",
                        Json::arr(sim.no_hold.iter().map(|n| {
                            Json::obj([
                                ("hold", Json::str(names.spec(&n.hold))),
                                ("hold_written", Json::str(&n.hold)),
                                ("argument", Json::str(&n.argument)),
                                ("reads", Json::str(format!("requester.{}", names.team_kind(&n.kind)))),
                                ("reads_written", Json::str(&n.reads)),
                                ("team_kind", Json::str(names.team_kind(&n.kind))),
                                ("team_kind_written", Json::str(&n.kind)),
                            ])
                        })),
                    ),
                    (
                        "stages",
                        Json::arr(sim.stages.iter().map(|st| {
                            Json::obj([
                                ("stage", Json::str(names.stage(&st.stage))),
                                ("stage_written", Json::str(&st.stage)),
                                ("spec", Json::str(names.spec(&st.shown))),
                                ("spec_written", Json::str(&st.shown)),
                                ("if_its_condition_holds", Json::from(st.may)),
                                ("due", Json::opt_str(st.due.as_ref().map(|d| d.get(lang).to_string()))),
                                (
                                    "ways",
                                    Json::arr(st.ways.iter().map(|w| {
                                        Json::obj([
                                            ("way", Json::str(names.spec(&w.way))),
                                            ("way_written", Json::str(&w.way)),
                                            ("case", Json::opt_str(w.label.as_ref().map(|l| l.get(lang).to_string()))),
                                            ("people", Json::arr(w.people.iter().map(Json::str))),
                                            ("needed", w.needed.map(|n| Json::int(n as i128)).unwrap_or(Json::Null)),
                                            ("proxies", Json::arr(w.proxies.iter().map(|(x, p)| Json::obj([("proxy", Json::str(x)), ("for", Json::str(p))])))),
                                            ("nobody", Json::opt_str(w.nobody.as_ref().map(|n| n.get(lang).to_string()))),
                                            ("once_with", Json::opt_str(w.once_with.as_ref().map(|x| names.stage(x).to_string()))),
                                            ("once_with_written", Json::opt_str(w.once_with.clone())),
                                            ("once_if_no_stage_between", Json::from(w.once_if_nothing_between)),
                                            // `, at the last stage`: whether the way waits for
                                            // `once_with` only where that stage comes, and to the
                                            // same person, and the stages it can be; `false` and
                                            // empty with the line without it and with no line
                                            ("once_if_it_comes", Json::from(w.once_if_it_comes)),
                                            ("once_among", Json::arr(w.once_among.iter().map(|x| Json::str(names.stage(x))))),
                                            ("once_among_written", Json::arr(w.once_among.iter().map(Json::str))),
                                            // whether `once_with` holds only once an agent doubts:
                                            // the agent of the stage `once_with`, or, with `, at
                                            // the last stage`, of this way
                                            ("once_if_the_agent_doubts", Json::from(w.once_if_doubted)),
                                            ("late", Json::opt_str(w.late.as_ref().map(|l| l.get(lang).to_string()))),
                                        ])
                                    })),
                                ),
                            ])
                        })),
                    ),
                ])
            })),
        ),
    ])
}
