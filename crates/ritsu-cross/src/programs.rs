//! The checks across the borders where a module of kaname calls a rule, a koyomi date or a chobo
//! transfer (kaname's DESIGN 14.3): the same checks as where a workflow calls them (DESIGN 7.4–7.8),
//! with the same codes, over what kaname hands over through its port (`Programs::crossings`).
//!
//! - X2: each precondition of each rule a module calls. A relation kaname keeps between the values
//!   given (a condition in force, the running totals of `running_totals`) shows it; else rulec is
//!   asked over the ranges kaname finds, as for a workflow ([`crate::preconditions`]); a bound on the
//!   total or the length of a list the rule walks is asked with the most elements kaname knows the
//!   list can have; the days of a koyomi date are held to the days kaname finds the value can be.
//! - X3 (a): the day given to a rule's date input that declares a range, held to that range: the
//!   days of the koyomi dates it is carried from unchanged, and the days kaname finds it can be.
//! - X4: every amount given to a transfer, wherever it comes from, held to what chobo takes; and the
//!   refusals each `do` and `hold` can meet at the amounts given, as chobo's search finds them. A
//!   module does not handle a refusal (the host applies the transfer), so the refusals are an answer
//!   to hand over, and only one that cannot be decided is said (W204).
//! - X6: the day given to a koyomi date's date input, held to its range.
//!
//! kaname's ranges are an estimate from above, not a range each value takes whatever the others
//! are: two values can move together, and an end can be one no input reaches. Where the ranges do
//! not show a check, the function's vectors are asked for (`Programs::seen`), and an example is a
//! run of them that breaks it (kaname's DESIGN 6.4); with none, the check is undecided, and the code
//! kaname generates checks it when it runs. [`ProgramVerdicts`] is what kaname reads of all this
//! (ritsu's port `Verdicts`), and [`check`] what `ritsu check` says.

use crate::{borders, Borders};
use ritsu_base::diag::Diag;
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_ports::{
    day_text, Answer, At, Books, CallArg, CallVerdict, ColumnType, Crossing, Dates, DaySet, Finding, Found, Given, Origin, Precondition, ProgramDateCall, ProgramRuleCall, ProgramTransfer, Programs, RuleFacts,
    Rules, Seen, Value,
};
use ritsu_project::{Joined, Project};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// The most days of a range kaname finds a date can be that are counted a day at a time: past it,
/// the range has no end to hold it to (a range of dates kaname finds is a parameter's, a few years).
const MOST_DAYS: i128 = 1_000_000;

/// One decision at one place of a module: the verdict kaname reads, and the notes ritsu says it with
/// when it does not hold.
#[derive(Clone)]
struct Decided {
    verdict: CallVerdict,
    notes: Vec<Text>,
}

/// What the checks across the borders came to at the calls of kaname's modules (ritsu's port
/// `Verdicts`, kaname's DESIGN 14.5): kaname decides the effects of its functions with it.
/// `ritsu kaname` and `ritsu check` hand it to kaname; each module is decided once in a run, and what
/// `ritsu check` says of it ([`check`]) is the same decision.
pub struct ProgramVerdicts {
    ports: ritsu_ports::Ports,
    programs: Rc<dyn Programs>,
    kept: RefCell<BTreeMap<PathBuf, Rc<Vec<Decided>>>>,
}

impl ProgramVerdicts {
    /// The verdicts of the modules' calls, asked of rulec, koyomi, chobo and kaname as `joined` joins
    /// them.
    pub fn new(joined: &Joined) -> ProgramVerdicts {
        ProgramVerdicts { ports: joined.ports(), programs: joined.kaname.clone(), kept: RefCell::default() }
    }

    /// Every decision at the calls of the module at `file`, once in a run.
    fn decided(&self, file: &Path) -> Rc<Vec<Decided>> {
        if let Some(d) = self.kept.borrow().get(file) {
            return d.clone();
        }
        let d = Rc::new(self.decide(file));
        self.kept.borrow_mut().insert(file.to_path_buf(), d.clone());
        d
    }

    fn decide(&self, file: &Path) -> Vec<Decided> {
        let Ok(cx) = self.programs.crossings(file, &self.ports) else { return Vec::new() };
        let seen = Seeing { file, ports: &self.ports, programs: self.programs.as_ref(), kept: RefCell::default() };
        let (rules, dates, books) = (self.ports.rules.as_ref(), self.ports.dates.as_ref(), self.ports.books.as_ref());
        let mut out = Vec::new();
        for call in &cx.rules {
            out.extend(rule_call(rules, dates, &seen, call));
        }
        for call in &cx.dates {
            out.extend(date_call(dates, &seen, call));
        }
        for t in &cx.transfers {
            out.extend(transfer(books, &seen, t));
        }
        out
    }
}

impl ritsu_ports::Verdicts for ProgramVerdicts {
    fn verdicts(&self, file: &Path) -> Vec<CallVerdict> {
        self.decided(file).iter().map(|d| d.verdict.clone()).collect()
    }
}

/// What the vectors of a module's functions give its places across the borders, asked of kaname for
/// a function the first time one of its checks is not shown by the ranges.
struct Seeing<'a> {
    file: &'a Path,
    ports: &'a ritsu_ports::Ports,
    programs: &'a dyn Programs,
    kept: RefCell<BTreeMap<String, Rc<Vec<Seen>>>>,
}

impl Seeing<'_> {
    /// What the runs of `function` gave the place at `at`.
    fn at(&self, function: &str, at: At) -> Vec<Seen> {
        let kept = self.kept.borrow().get(function).cloned();
        let all = match kept {
            Some(s) => s,
            None => {
                let s = Rc::new(self.programs.seen(self.file, self.ports, function));
                self.kept.borrow_mut().insert(function.to_string(), s.clone());
                s
            }
        };
        all.iter().filter(|s| s.at == at).cloned().collect()
    }
}

// ── What a check comes to ───────────────────────────────────────────────────────────────────────

/// What the ranges came to at a place: shown; or not shown, with what is said of it when no run of
/// the function's vectors breaks it either.
enum ByRanges {
    Shown,
    Open(Text),
}

/// The first run of the function's vectors that breaks a check, else undecided for `why`.
fn by_runs(found: Option<Seen>, why: Text) -> Answer<Seen> {
    match found {
        Some(s) => Answer::Fails(s),
        None => Answer::Undecided(why),
    }
}

/// A sentence that says why nothing could be said from the ranges, with what the runs found added.
fn no_run_breaks(why: &Text) -> Text {
    let (ja, en) = (why.ja.trim_end_matches('。'), why.en.trim_end_matches('.'));
    tr!("{ja}。関数のベクタにも、これを破る入力はありませんでした。", "{en}, and no input among the function's vectors breaks it either.")
}

/// A day number from a value on the wire.
fn day_of(v: &Value) -> Option<i64> {
    match v {
        Value::Date(d) => ritsu_ports::day_of(d),
        Value::Int(i) => i64::try_from(*i).ok(),
        _ => None,
    }
}

/// A whole number from a value on the wire (a date as its day number).
fn int_of(v: &Value) -> Option<i128> {
    match v {
        Value::Int(i) => Some(*i),
        Value::Date(_) => day_of(v).map(i128::from),
        _ => None,
    }
}

/// A value on the wire as a note writes it.
fn value_text(v: &Value) -> String {
    match v {
        Value::Int(i) => i.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Str(s) => format!("\"{s}\""),
        Value::Date(d) | Value::Enum(d) => d.clone(),
        Value::None => "none".into(),
        Value::List(xs) => format!("[{} elements]", xs.len()),
    }
}

/// A range on the wire as a note writes it: `>=0 <=90000000`, or for a date `>=2026-01-01`.
fn range_text(r: (Option<i128>, Option<i128>), date: bool) -> String {
    let end = |v: i128| if date { day_text(v as i64) } else { v.to_string() };
    let mut parts = Vec::new();
    if let Some(lo) = r.0 {
        parts.push(format!(">={}", end(lo)));
    }
    if let Some(hi) = r.1 {
        parts.push(format!("<={}", end(hi)));
    }
    if parts.is_empty() { "any".into() } else { parts.join(" ") }
}

/// Whether an input of the rule is a date (a date that may be none too).
fn is_date(facts: &RuleFacts, input: &str) -> bool {
    facts.inputs.iter().any(|c| c.name == input && (c.ty == ColumnType::Date || c.ty == ColumnType::Opt(Box::new(ColumnType::Date))))
}

/// A value a module gives, as a flow's is handed over to the decisions of X2.
fn as_flows(g: &Given) -> CallArg {
    CallArg { input: g.to.clone(), shown: g.shown.clone(), range: Some(g.range), unknown: None, from: g.from.clone() }
}

/// The days a value can be (X2's days, X3 (a), X6): the days of each koyomi date it is carried from
/// unchanged, as koyomi counts them, within the range kaname finds; and the range kaname finds,
/// where it can come from somewhere else.
fn days_of(dates: &dyn Dates, g: &Given) -> (Vec<Found<DaySet>>, Option<Text>) {
    let (lo, hi) = g.range;
    let within = |d: &i64| lo.is_none_or(|l| i128::from(*d) >= l) && hi.is_none_or(|h| i128::from(*d) <= h);
    let mut sets = Vec::new();
    let mut other = None;
    for o in &g.from {
        match o {
            Origin::Day { file, date } => sets.push(match dates.values(file, date) {
                Ok(Found::Value(set)) => Found::Value(set.into_iter().filter(within).collect()),
                Ok(Found::Undecided(why)) => Found::Undecided(tr!("koyomi は日付 {date} の日を数えません（{}）", "koyomi does not count the days of the date {date}: {}", why.ja; why.en)),
                Err(_) => Found::Undecided(tr!("koyomi から日付 {date} の情報を得られません", "koyomi does not answer for the date {date}")),
            }),
            _ => match (lo, hi) {
                (Some(l), Some(h)) if h - l < MOST_DAYS => sets.push(Found::Value((l..=h).filter_map(|d| i64::try_from(d).ok()).collect())),
                _ => {
                    other.get_or_insert(tr!("範囲の解析では、`{}` がとる日の範囲に端がありません", "The analysis of ranges finds no end of the days `{}` can be", g.shown; g.shown));
                }
            },
        }
    }
    (sets, other)
}

/// Whether the days a value can be all lie in `range`; what is said where one may not.
fn days_in(dates: &dyn Dates, g: &Given, range: (Option<i64>, Option<i64>)) -> ByRanges {
    let (sets, other) = days_of(dates, g);
    match borders::days_given(&sets, other.as_ref(), range) {
        Answer::Holds => ByRanges::Shown,
        Answer::Fails((_, day)) => ByRanges::Open(tr!(
            "範囲の解析では、`{}` がとる日に、範囲の外の {} が入ります。関数のベクタには、範囲の外の日を渡す入力はありませんでした。",
            "The days the analysis of ranges finds for `{}` include {}, outside the range, and no input among the function's vectors gives a day outside.",
            g.shown, day_text(day); g.shown, day_text(day)
        )),
        Answer::Undecided(why) => ByRanges::Open(no_run_breaks(&why)),
    }
}

/// The first run that gives the place a day outside `range`.
fn day_outside(seen: &[Seen], to: &str, range: (Option<i64>, Option<i64>)) -> Option<Seen> {
    seen.iter()
        .find(|s| s.given.iter().any(|(n, v)| n == to && day_of(v).is_some_and(|d| range.0.is_some_and(|lo| d < lo) || range.1.is_some_and(|hi| d > hi))))
        .cloned()
}

/// Inputs each held to a range on the wire, as rulec is asked about its preconditions.
type Ranges = Vec<(String, Option<i128>, Option<i128>)>;

/// What a run gave the inputs of a precondition, on the wire, held to each as a range of one value;
/// None when it gave one of them nothing.
fn points(s: &Seen, names: &[&String]) -> Option<Ranges> {
    names
        .iter()
        .map(|n| {
            let v = s.given.iter().find(|(m, _)| m == *n).and_then(|(_, v)| int_of(v))?;
            Some(((*n).clone(), Some(v), Some(v)))
        })
        .collect()
}

/// rulec's answer for the precondition `p` over these ranges and this most of a list.
fn asked(rules: &dyn Rules, rule: &Path, p: &Precondition, ranges: &[(String, Option<i128>, Option<i128>)], max_len: Option<i128>) -> Option<Answer<ritsu_ports::Values>> {
    rules.preconditions_hold(rule, ranges, max_len).ok().and_then(|xs| xs.into_iter().find(|(q, _)| q == p)).map(|(_, a)| a)
}

/// Whether rulec says a run breaks the precondition `p` (asked over the values the run gave, each
/// held to itself). A total over a list is asked over its largest element as many times as it has
/// elements, so a run breaks it only when every element gives the same.
fn run_breaks(rules: &dyn Rules, rule: &Path, p: &Precondition, s: &Seen) -> bool {
    let fails = |ranges: &[(String, Option<i128>, Option<i128>)], max_len: Option<i128>| matches!(asked(rules, rule, p, ranges, max_len), Some(Answer::Fails(_)));
    match p {
        Precondition::Relation { left, right, .. } => points(s, &[left, right]).is_some_and(|r| fails(&r, None)),
        Precondition::SumRelation { left, right, .. } => points(s, &left.iter().chain(right.iter()).collect::<Vec<_>>()).is_some_and(|r| fails(&r, None)),
        Precondition::Sum { over, of, .. } => {
            let Some(Value::List(xs)) = s.given.iter().find(|(n, _)| n == over).map(|(_, v)| v) else { return false };
            let each: Vec<i128> = xs.iter().filter_map(|el| el.iter().find(|(n, _)| n == of).and_then(|(_, v)| int_of(v))).collect();
            let (Some(top), true) = (each.iter().copied().max(), each.windows(2).all(|w| w[0] == w[1])) else { return false };
            fails(&[(of.clone(), Some(top), Some(top))], Some(xs.len() as i128))
        }
        Precondition::Length { sequence, .. } => match s.given.iter().find(|(n, _)| n == sequence).map(|(_, v)| v) {
            Some(Value::List(xs)) => fails(&[], Some(xs.len() as i128)),
            _ => false,
        },
        Precondition::Days { input, days, .. } => s.given.iter().any(|(n, v)| n == input && day_of(v).is_some_and(|d| !days.contains(&d))),
    }
}

/// What is said of a relation between inputs the ranges do not show: the ranges kaname finds, and
/// the corner of them that breaks it, when rulec says one.
fn relation_open(rules: &dyn Rules, facts: &RuleFacts, call: &ProgramRuleCall, p: &Precondition, names: &[String], pre: &str) -> Text {
    let given: Vec<&Given> = names.iter().filter_map(|n| call.args.iter().find(|a| a.to == *n)).collect();
    if given.len() < names.len() {
        let missing = names.iter().find(|n| !call.args.iter().any(|a| a.to == **n)).cloned().unwrap_or_default();
        return no_run_breaks(&tr!("呼び出しが `{missing}` を渡していません", "the call does not give `{missing}`"));
    }
    let each_ja: Vec<String> = given.iter().map(|g| format!("`{}` は {}", g.shown, range_text(g.range, is_date(facts, &g.to)))).collect();
    let each_en: Vec<String> = given.iter().map(|g| format!("`{}` in {}", g.shown, range_text(g.range, is_date(facts, &g.to)))).collect();
    let ranges: Vec<(String, Option<i128>, Option<i128>)> = given.iter().map(|g| (g.to.clone(), g.range.0, g.range.1)).collect();
    match asked(rules, &call.rule, p, &ranges, None) {
        Some(Answer::Fails(corner)) => {
            let at = corner.iter().map(|(n, v)| format!("{n} = {}", value_text(v))).collect::<Vec<_>>().join(", ");
            tr!(
                "範囲の解析では、{} です。範囲の端の {at} では `{pre}` が成り立ちませんが、関数のベクタに、そういう値を渡す入力はありませんでした。",
                "The analysis of ranges finds {}; at the ends of the ranges, {at}, `{pre}` does not hold, and no input among the function's vectors gives such values.",
                each_ja.join("、"); join_en(&each_en)
            )
        }
        Some(Answer::Undecided(why)) => no_run_breaks(&why),
        _ => no_run_breaks(&tr!("rulec から、この前提が成り立つかどうかの結果を得られません", "rulec does not answer for this precondition")),
    }
}

/// `a`, `a and b`, `a, b and c`.
fn join_en(xs: &[String]) -> String {
    match xs {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// X2: one precondition at one call of a rule.
fn precondition(rules: &dyn Rules, dates: &dyn Dates, seen: &Seeing, facts: &RuleFacts, call: &ProgramRuleCall, p: &Precondition) -> Answer<Seen> {
    let pre = crate::preconditions::said(p);
    let ranges = match p {
        Precondition::Relation { left, op, right } => {
            if kept(&call.relations, left, op, right) {
                ByRanges::Shown
            } else {
                let flows: Vec<CallArg> = call.args.iter().map(as_flows).collect();
                match crate::preconditions::decide(rules, dates, &call.rule, p, &flows) {
                    Answer::Holds => ByRanges::Shown,
                    _ => ByRanges::Open(relation_open(rules, facts, call, p, &[left.clone(), right.clone()], &pre)),
                }
            }
        }
        Precondition::SumRelation { left, right, .. } => {
            let flows: Vec<CallArg> = call.args.iter().map(as_flows).collect();
            match crate::preconditions::decide(rules, dates, &call.rule, p, &flows) {
                Answer::Holds => ByRanges::Shown,
                _ => ByRanges::Open(relation_open(rules, facts, call, p, &left.iter().chain(right.iter()).cloned().collect::<Vec<_>>(), &pre)),
            }
        }
        // kaname knows how many elements a list can have: the bound is asked with it
        Precondition::Sum { over, .. } | Precondition::Length { sequence: over, .. } => {
            let fields: Vec<(String, Option<i128>, Option<i128>)> = call.args.iter().find(|a| a.to == *over).map(|a| a.fields.iter().map(|(n, (lo, hi))| (n.clone(), *lo, *hi)).collect()).unwrap_or_default();
            match asked(rules, &call.rule, p, &fields, call.max_len) {
                Some(Answer::Holds) => ByRanges::Shown,
                Some(Answer::Fails(_)) => ByRanges::Open(match call.max_len {
                    Some(n) => tr!(
                        "並び `{over}` に渡すリストは {n} 件までです。その長さと要素の範囲の端では `{pre}` が成り立ちませんが、関数のベクタに、そういうリストを渡す入力はありませんでした。",
                        "The list given to `{over}` has up to {n} elements; at that length and the ends of its elements' ranges `{pre}` does not hold, and no input among the function's vectors gives such a list."
                    ),
                    None => no_run_breaks(&tr!("並び `{over}` に渡すリストの長さの上限が分かりません", "nothing says how long the list given to `{over}` can be")),
                }),
                Some(Answer::Undecided(why)) => ByRanges::Open(no_run_breaks(&why)),
                None => ByRanges::Open(no_run_breaks(&tr!("rulec から、この前提が成り立つかどうかの結果を得られません", "rulec does not answer for this precondition"))),
            }
        }
        Precondition::Days { input, file, date, days } => match call.args.iter().find(|a| a.to == *input) {
            None => ByRanges::Open(no_run_breaks(&tr!("呼び出しが `{input}` を渡していません", "the call does not give `{input}`"))),
            Some(a) => {
                let (sets, other) = days_of(dates, a);
                let outside = sets.iter().find_map(|s| match s {
                    Found::Value(set) => set.iter().copied().find(|d| !days.contains(d)),
                    Found::Undecided(_) => None,
                });
                let open = sets.iter().find_map(|s| match s {
                    Found::Undecided(why) => Some(why.clone()),
                    Found::Value(_) => None,
                });
                match (outside, other.or(open)) {
                    (Some(d), _) => ByRanges::Open(tr!(
                        "範囲の解析では、`{}` がとる日に、koyomi \"{file}\" date {date} の日でない {} が入ります。関数のベクタには、そういう日を渡す入力はありませんでした。",
                        "The days the analysis of ranges finds for `{}` include {}, which is not a day of koyomi \"{file}\" date {date}, and no input among the function's vectors gives such a day.",
                        a.shown, day_text(d); a.shown, day_text(d)
                    )),
                    (None, Some(why)) => ByRanges::Open(no_run_breaks(&why)),
                    (None, None) => ByRanges::Shown,
                }
            }
        },
    };
    match ranges {
        ByRanges::Shown => Answer::Holds,
        ByRanges::Open(why) => {
            let found = seen.at(&call.function, call.at).into_iter().find(|s| run_breaks(rules, &call.rule, p, s));
            by_runs(found, why)
        }
    }
}

/// Whether a relation kaname keeps at a call says the precondition `left op right`: the same two
/// inputs in its direction, `<` saying `<=` too, `=` saying `<=` and `>=` (kaname's DESIGN 14.3).
fn kept(relations: &[(String, String, String)], left: &str, op: &str, right: &str) -> bool {
    let (small, large, strict) = match op {
        "<" => (left, right, true),
        "<=" => (left, right, false),
        ">" => (right, left, true),
        ">=" => (right, left, false),
        _ => return false,
    };
    relations.iter().any(|(a, r, b)| match r.as_str() {
        "<" => a == small && b == large,
        "<=" => a == small && b == large && !strict,
        "=" => ((a == small && b == large) || (a == large && b == small)) && !strict,
        _ => false,
    })
}

/// A decision, with the code ritsu says it with and its headline where it does not hold.
fn decided(at: At, check: Crossing, about: String, outcome: Answer<Seen>, codes: (&str, &str), headline: Text, notes: Vec<Text>) -> Decided {
    let said = match &outcome {
        Answer::Holds => None,
        Answer::Fails(_) => Some((codes.0.to_string(), headline)),
        Answer::Undecided(_) => Some((codes.1.to_string(), headline)),
    };
    Decided { verdict: CallVerdict { at, check, about, outcome, said, refusals: None }, notes }
}

/// The example of a check, as a note says it: the function's input, and what it gives the place.
fn example_note(s: &Seen, what: &str) -> Text {
    let call = &s.call;
    tr!("たとえば `{call}` のとき、{what} になります。", "For example, `{call}` gives it {what}.")
}

/// What kaname's generated code does at a place a check is not shown: it checks it when it runs, and
/// the function fails there with `kind` where the call is refused (`refuser` says who refuses it).
fn checked_when_run(refuser: Text, kind: &str) -> Text {
    tr!(
        "kaname の生成したコードが、走らせたときにその場所で確かめます。{}、関数はそこで `{kind}` で失敗します。",
        "kaname's generated code checks it there when it runs: {}, the function fails there with `{kind}`.",
        refuser.ja; refuser.en
    )
}

/// X2 and X3 (a) at one call of a rule.
fn rule_call(rules: &dyn Rules, dates: &dyn Dates, seen: &Seeing, call: &ProgramRuleCall) -> Vec<Decided> {
    let Ok(facts) = rules.facts(&call.rule) else { return Vec::new() };
    let rule = &call.name;
    let refused = tr!("規則から生成したコードが入口で受け付けなければ", "where the rule's generated code refuses the call at its door");
    let mut out = Vec::new();
    for p in &facts.preconditions {
        let pre = crate::preconditions::said(p);
        let outcome = precondition(rules, dates, seen, &facts, call, p);
        let headline = match &outcome {
            Answer::Fails(_) => tr!("規則 {rule} を呼ぶところで、前提 `{pre}` を破る値を渡すことがあります", "The call of the rule {rule} can give it values that break its precondition `{pre}`"),
            _ => tr!("規則 {rule} を呼ぶところで、前提 `{pre}` が保たれるかを決められません", "Whether the call of the rule {rule} keeps its precondition `{pre}` cannot be decided"),
        };
        let notes = match &outcome {
            Answer::Holds => Vec::new(),
            Answer::Fails(s) => {
                let names = match p {
                    Precondition::Relation { left, right, .. } => vec![left.clone(), right.clone()],
                    Precondition::SumRelation { left, right, .. } => left.iter().chain(right.iter()).cloned().collect(),
                    Precondition::Sum { over, .. } => vec![over.clone()],
                    Precondition::Length { sequence, .. } => vec![sequence.clone()],
                    Precondition::Days { input, .. } => vec![input.clone()],
                };
                let at = names.iter().filter_map(|n| s.given.iter().find(|(m, _)| m == n).map(|(_, v)| format!("{n} = {}", value_text(v)))).collect::<Vec<_>>().join(", ");
                let call_text = &s.call;
                vec![
                    tr!("たとえば `{call_text}` のとき、規則に {at} を渡します。", "For example, `{call_text}` gives the rule {at}."),
                    tr!(
                        "規則から生成したコードは、前提を破る呼び出しを入口で受け付けません。kaname の関数は、そこで `rule` で失敗します。",
                        "The rule's generated code refuses a call that breaks a precondition at its door, and the kaname function fails there with `rule`."
                    ),
                    tr!("呼ぶ前に前提を `if` で確かめて分けるか、渡す値の範囲を直してください。", "Branch on the precondition with an `if` before the call, or correct the ranges of the values given."),
                ]
            }
            Answer::Undecided(why) => vec![
                why.clone(),
                checked_when_run(refused.clone(), "rule"),
                tr!(
                    "呼ぶ前に前提を `if` で確かめて分けるか、kaname が関係を知っている値（`running_totals` の途中までの合計など）を渡すと、示せるようになります。",
                    "Branch on the precondition with an `if` before the call, or give values whose relation kaname keeps (the running totals of `running_totals`), and it is shown."
                ),
            ],
        };
        out.push(decided(call.at, Crossing::Precondition, pre, outcome, ("E201", "W201"), headline, notes));
    }
    // X3 (a): a day given to a date input with a declared range; an input whose range is a koyomi
    // date takes the date's days as a precondition, above
    for a in &call.args {
        if !is_date(&facts, &a.to) || facts.preconditions.iter().any(|p| matches!(p, Precondition::Days { input, .. } if *input == a.to)) {
            continue;
        }
        let range = rules.date_range(&call.rule, &a.to).unwrap_or((None, None));
        if range == (None, None) {
            continue;
        }
        let input = &a.to;
        let shown_range = range_text((range.0.map(i128::from), range.1.map(i128::from)), true);
        let outcome = match days_in(dates, a, range) {
            ByRanges::Shown => Answer::Holds,
            ByRanges::Open(why) => by_runs(day_outside(&seen.at(&call.function, a.at), input, range), why),
        };
        let headline = match &outcome {
            Answer::Fails(_) => tr!("規則 `{rule}` の `{input}` に渡す日が、規則の宣言した範囲を外れることがあります", "The day given to `{input}` of the rule `{rule}` can be outside the range the rule declares"),
            _ => tr!("規則 `{rule}` の `{input}` に渡す日が、範囲に収まるかを決められません", "Whether the day given to `{input}` of the rule `{rule}` stays inside its range cannot be decided"),
        };
        let takes = tr!("`{input}` が受け取るのは `{shown_range}` です。", "`{input}` takes `{shown_range}`.");
        let notes = match &outcome {
            Answer::Holds => Vec::new(),
            Answer::Fails(s) => {
                let day = s.given.iter().find(|(n, _)| n == input).and_then(|(_, v)| day_of(v)).map(day_text).unwrap_or_default();
                vec![
                    example_note(s, &day),
                    takes,
                    tr!(
                        "規則から生成したコードは、範囲の外の日を入口で受け付けません。kaname の関数は、そこで `rule` で失敗します。",
                        "The rule's generated code refuses a day outside its range at its door, and the kaname function fails there with `rule`."
                    ),
                    tr!("呼ぶ前に `if` で日を範囲に狭めるか、規則の入力の範囲を広げてください。", "Narrow the day to the range with an `if` before the call, or widen the rule input's range."),
                ]
            }
            Answer::Undecided(why) => vec![why.clone(), takes, checked_when_run(refused.clone(), "rule")],
        };
        out.push(decided(a.at, Crossing::RuleDay, input.clone(), outcome, ("E202", "W202"), headline, notes));
    }
    out
}

/// X6 at one call of a koyomi date.
fn date_call(dates: &dyn Dates, seen: &Seeing, call: &ProgramDateCall) -> Vec<Decided> {
    let Ok(facts) = dates.facts(&call.file) else { return Vec::new() };
    let Some((lo, hi)) = borders::input_range(&facts, &call.day.to) else { return Vec::new() };
    let (date, input, file) = (&call.name, &call.day.to, call.file.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default());
    let (min, max) = (day_text(lo), day_text(hi));
    let range = (Some(lo), Some(hi));
    let outcome = match days_in(dates, &call.day, range) {
        ByRanges::Shown => Answer::Holds,
        ByRanges::Open(why) => by_runs(day_outside(&seen.at(&call.function, call.day.at), input, range), why),
    };
    let headline = match &outcome {
        Answer::Fails(_) => tr!("koyomi の日付 `{date}` の `{input}` に渡す日が、入力の範囲を外れることがあります", "The day given to `{input}` of the koyomi date `{date}` can be outside the input's range"),
        _ => tr!("koyomi の日付 `{date}` の `{input}` に渡す日が、入力の範囲に収まるかを決められません", "Whether the day given to `{input}` of the koyomi date `{date}` stays inside the input's range cannot be decided"),
    };
    let takes = tr!("\"{file}\" が `{input}` に受け取るのは {min} から {max} までです。", "\"{file}\" takes {min} to {max} for `{input}`.");
    let notes = match &outcome {
        Answer::Holds => Vec::new(),
        Answer::Fails(s) => {
            let day = s.given.iter().find(|(n, _)| n == input).and_then(|(_, v)| day_of(v)).map(day_text).unwrap_or_default();
            vec![
                example_note(s, &day),
                takes,
                tr!(
                    "koyomi が生成したコードは、範囲の外の日を受け付けません。kaname の関数は、そこで `date` で失敗します。",
                    "koyomi's generated code refuses a day outside the range, and the kaname function fails there with `date`."
                ),
                tr!(
                    "呼ぶ前に `if` で日を範囲に狭めるか、\"{file}\" の `{input}` の範囲を広げてください（カレンダーのデータも足してください）。",
                    "Narrow the day with an `if` before the call, or widen the range of `{input}` in \"{file}\" (and the data of its calendar)."
                ),
            ]
        }
        Answer::Undecided(why) => vec![why.clone(), takes, checked_when_run(tr!("koyomi が生成したコードが受け付けなければ", "where koyomi's generated code refuses the day"), "date")],
    };
    vec![decided(call.day.at, Crossing::DateDay, input.clone(), outcome, ("E205", "W205"), headline, notes)]
}

/// X4 at one transfer a module builds: each amount, and the refusals of a `do` or a `hold`.
fn transfer(books: &dyn Books, seen: &Seeing, t: &ProgramTransfer) -> Vec<Decided> {
    // the operation as the book names it, which the verdict is about, and as the module writes it
    let (op, written) = (format!("{}.{}", t.transfer, t.op), &t.name);
    let mut out = Vec::new();
    for a in &t.amounts {
        let (param, shown) = (&a.to, &a.shown);
        let outside = |s: &Seen| s.given.iter().any(|(n, v)| n == param && int_of(v).is_some_and(|x| !(0..=borders::MAX_AMOUNT).contains(&x)));
        let outcome = match borders::amounts_given(&[], &[a.range], None) {
            Answer::Holds => Answer::Holds,
            Answer::Fails(_) => by_runs(
                seen.at(&t.function, a.at).into_iter().find(outside),
                tr!(
                    "範囲の解析では、`{shown}` は {} です。関数のベクタにも、chobo が受け取らない額になる入力はありませんでした。",
                    "The analysis of ranges finds `{shown}` in {}, and no input among the function's vectors takes it out of what chobo takes.",
                    range_text(a.range, false); range_text(a.range, false)
                ),
            ),
            Answer::Undecided(why) => by_runs(seen.at(&t.function, a.at).into_iter().find(outside), no_run_breaks(&why)),
        };
        let headline = match &outcome {
            Answer::Fails(_) => tr!("振替 `{written}` の `{param}` に渡す `{shown}` が、chobo の受け取らない額になることがあります", "`{shown}`, given to `{param}` of the transfer `{written}`, can be an amount chobo does not take"),
            _ => tr!("振替 `{written}` の `{param}` に渡す `{shown}` を、chobo が額として受け取るかを決められません", "Whether chobo takes `{shown}`, given to `{param}` of the transfer `{written}`, as an amount cannot be decided"),
        };
        let takes = tr!("chobo が受け取る額は、単位のいちばん小さい刻みで 0 から 9223372036854775807 までです。", "chobo takes an amount from 0 to 9223372036854775807 in the unit's smallest step.");
        let notes = match &outcome {
            Answer::Holds => Vec::new(),
            Answer::Fails(s) => {
                let v = s.given.iter().find(|(n, _)| n == param).map(|(_, v)| value_text(v)).unwrap_or_default();
                vec![
                    example_note(s, &v),
                    takes,
                    tr!(
                        "kaname の生成したコードは、振替を組み立てるところで額を確かめます。関数は、そこで `range` で失敗します。",
                        "kaname's generated code checks the amount where it builds the transfer, and the function fails there with `range`."
                    ),
                    tr!("振替の前に `if` で額を狭めるか、負の額は向きの違う振替に分けてください。", "Narrow the amount with an `if` before the transfer, or make a negative amount a transfer the other way."),
                ]
            }
            Answer::Undecided(why) => vec![why.clone(), takes, checked_when_run(tr!("chobo が受け取らない額なら", "for an amount chobo does not take"), "range")],
        };
        out.push(decided(a.at, Crossing::Amount, param.clone(), outcome, ("E203", "W203"), headline, notes));
    }
    // the refusals the amounts can meet, for the operations whose refusals turn on them
    if t.op == "do" || t.op == "hold" {
        let hull = borders::amounts_hull(&[], &t.amounts.iter().map(|a| a.range).collect::<Vec<_>>());
        let found = match hull {
            None => Found::Undecided(tr!("渡す額の範囲が分からないか、chobo が受け取る額を含みません", "the range of the amounts given is not known, or holds no amount chobo takes")),
            Some(range) => match books.refusals(&t.book, &t.transfer, range) {
                Ok(v) => v,
                Err(_) => Found::Undecided(tr!("chobo から帳簿の情報を得られません", "chobo does not answer for the book")),
            },
        };
        let (outcome, refusals) = match found {
            Found::Value(ops) => match ops.into_iter().find(|(o, _)| *o == t.op) {
                Some((_, reasons)) => (Answer::Holds, Some(reasons)),
                None => (Answer::Undecided(tr!("振替に操作 `{}` がありません", "the transfer has no operation `{}`", t.op)), None),
            },
            Found::Undecided(why) => (Answer::Undecided(why), None),
        };
        let headline = tr!("ここで渡す額で、帳簿が `{written}` をどの理由で拒否しうるかを決められません", "Which reasons the book can refuse `{written}` with, at the amounts given here, cannot be decided");
        let notes = match &outcome {
            Answer::Undecided(why) => vec![
                why.clone(),
                tr!(
                    "kaname が生成する振替の型には、帳簿の境界の理由を全部載せます。拒否を扱うのは、振替を当てるホストです。",
                    "The type kaname generates for the transfer lists every reason of the book's bounds; the host that applies the transfer handles the refusal."
                ),
            ],
            _ => Vec::new(),
        };
        let mut d = decided(t.at, Crossing::Refusals, op, outcome, ("W204", "W204"), headline, notes);
        d.verdict.refusals = refusals;
        out.push(d);
    }
    out
}

/// What `ritsu check` says of the calls of the project's modules across the borders (X2, X3 (a), X4,
/// X6), as `verdicts` decided them: each decision a border, and each that does not hold said at the
/// place, in ritsu's codes. A module whose names or types do not pass is kaname's to say.
pub(crate) fn check(project: &Project, verdicts: &ProgramVerdicts, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    let mut out = Vec::new();
    for f in project.of(Tool::Kaname) {
        let decided = verdicts.decided(Path::new(&f.shown));
        if decided.is_empty() {
            continue;
        }
        let src = ritsu_base::fs::read_to_string(ritsu_base::paths::on_disk(&project.root, &f.rel)).unwrap_or_default();
        for d in decided.iter() {
            let v = &d.verdict;
            match &v.outcome {
                Answer::Holds => borders.held += 1,
                Answer::Fails(_) => borders.failed += 1,
                Answer::Undecided(_) => borders.undecided += 1,
            }
            let Some((code, headline)) = &v.said else { continue };
            // the codes of ritsu's ledger these checks say, as the diagnostics take them
            let code: &'static str = match code.as_str() {
                "E201" => "E201",
                "W201" => "W201",
                "E202" => "E202",
                "W202" => "W202",
                "E203" => "E203",
                "W203" => "W203",
                "W204" => "W204",
                "E205" => "E205",
                _ => "W205",
            };
            let mut diag: Diag = Diag::at(code, &f.shown, v.at.line, v.at.col, headline.clone()).source(&src).rel(&f.rel);
            for n in &d.notes {
                diag = diag.note(n.clone());
            }
            out.push(Finding::of(&diag, Some(f.rel.clone()), lang));
        }
    }
    out
}
