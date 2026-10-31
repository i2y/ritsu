//! Ranges: what numbers a value can be, checked where a value is given to a place that has a
//! range — an input of a rule, a parameter of a task, a field of a record written out, an output
//! of the workflow. A value can be outside the place's range (E014), or nothing says what it can
//! be (W104). What a variable can be is gathered from every value put in it anywhere in the flow,
//! so a variable that is given 1 in one arm and 40 in another is `>=1 <=40` everywhere.
//!
//! A place of an ordered enum may hold its value to a range too (`range >=basic <=gold`): an input
//! of a rule (which rulec hands over with the values it lets in), and in the `.flow` an input, an
//! output, a field of a record, a parameter or the answer of a task. The values of the enum a value
//! can be are gathered the same way, a value written out being itself, and an input, a task's
//! answer or a field the values its range lets in, every value of its enum without one. A value
//! that can be a value the range of where it goes does not let in is E014, said in those values;
//! the type always says what an enum's value can be, so there is no W104 for one.
//!
//! A date and a time may be held to a range too (`range >=2026-01-01 <=2027-12-31`, DESIGN 20):
//! a date's range is its days (day numbers), a time's its seconds, and they are checked as numbers
//! are, said in days and moments. `now` is a moment of the period the workflow runs in, when it
//! says one (`now range`, which every statement that reads `now` is held to: `TExpr::NowAt`), and
//! says nothing of what it is otherwise, nor in `on failure` and `on cancel`, which the period does
//! not hold. The day of a koyomi date (`due.day`) is one of the days koyomi counts it to over the
//! whole range of its inputs, and its time (`due.at`) that day's time at the calendar's offset: for
//! what dandori itself holds to a range (E014), not for the values it hands ritsu at a rule's call,
//! whose days ritsu decides itself (X2, X3 (a)).
//!
//! Where a value comes in (an input, the answer of a task or a rule) its range is checked when
//! the workflow runs, by the code each platform is given; here, the ranges are taken as true.

use crate::diag::{Diag, Text};
use crate::model::*;
use std::collections::{BTreeMap, BTreeSet};

/// What is known of the numbers a value can be, and of the values of an enum.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Est {
    /// the numbers that come from places with a range; None when none does (`none`, `[]`)
    pub(crate) known: Option<Range>,
    /// a place without a range the value can come from, named in Japanese and in English
    pub(crate) unknown: Option<Text>,
    /// for a value of an enum, the values it can be, by name: the value written, or every value of
    /// the enum for an input, a task's answer or a field (empty for a value that is no enum's)
    pub(crate) values: BTreeSet<String>,
}

impl Est {
    fn range(r: Range) -> Est {
        Est { known: Some(r), ..Est::default() }
    }

    fn unknown(from: Text) -> Est {
        Est { unknown: Some(from), ..Est::default() }
    }

    fn or(r: Option<Range>, from: Text) -> Est {
        match r {
            Some(r) => Est::range(r),
            None => Est::unknown(from),
        }
    }

    /// What a value read from a place of the type `t` with the range `rg` (named `from` in case it
    /// has none) can be: the numbers of its range, and for a value of an enum the values the range
    /// lets in, every value of the enum without one. An enum's range is not numbers, and a place
    /// of an enum is named as one without a range of numbers, as it always was.
    fn at(m: &Model, t: &Ty, rg: Option<Range>, from: Text) -> Est {
        let numbers = if t.enum_inside().is_some() { None } else { rg };
        let mut est = Est::or(numbers, from);
        est.values = place_values(m, t, rg);
        est
    }

    fn join(&self, o: &Est) -> Est {
        Est {
            known: match (self.known, o.known) {
                (Some(a), Some(b)) => Some(a.hull(&b)),
                (a, b) => a.or(b),
            },
            unknown: self.unknown.clone().or(o.unknown.clone()),
            values: self.values.union(&o.values).cloned().collect(),
        }
    }
}

/// The values of the enum a value of the type `t` is (inside a `?` or a list) that a place with
/// the range `rg` lets in, every value of it without one; none for a type that is not an enum.
fn place_values(m: &Model, t: &Ty, rg: Option<Range>) -> BTreeSet<String> {
    match t.enum_inside() {
        Some(e) => m.values_in(e, rg).into_iter().collect(),
        None => BTreeSet::new(),
    }
}

/// What a place takes: the numbers of a range, or of a place that holds its enum to a range, the
/// values the range lets in (in the enum's order).
enum Takes {
    Numbers(Range),
    Values(Vec<String>),
}

/// What a place of the `.flow` of the type `t` with the range `rg` takes: for an enum, the values
/// its range lets in; for a number, its range. None for a place without a range.
fn takes(m: &Model, t: &Ty, rg: Option<Range>) -> Option<Takes> {
    match t.enum_inside() {
        Some(_) => m.held_values(t, rg).map(Takes::Values),
        None => rg.map(Takes::Numbers),
    }
}

pub fn check(m: &Model) -> Vec<Diag> {
    let vars = variables_by(m, Written::Itself, true);
    let mut out = Vec::new();
    for s in m.all_stmts() {
        let mut give = |e: &TExpr, want: Option<Takes>, en: String, ja: String| {
            let d = match want {
                Some(Takes::Numbers(want)) => fit(m, &vars, e, want, s.line, &en, &ja),
                Some(Takes::Values(takes)) => fit_values(m, &vars, e, &takes, s.line, &en, &ja),
                None => None,
            };
            if let Some(d) = d {
                out.push(d);
            }
        };
        match &s.kind {
            TK::Call { callee, args, .. } => {
                for (p, e) in args {
                    match callee {
                        Callee::Task(t) => {
                            let t = &m.tasks[*t];
                            let want = t.params.iter().find(|(n, _)| n == p).and_then(|(_, pt)| takes(m, pt, t.param_ranges.get(p).copied()));
                            give(e, want, format!("the parameter `{p}` of `{}`", t.name), format!("`{}` の引数 `{p}`", t.name));
                        }
                        Callee::Rule(r) => {
                            let rn = &m.rules[*r].name;
                            let want = m.rule_input_range(*r, p).map(Takes::Numbers).or_else(|| m.rule_input_values(*r, p).map(|v| Takes::Values(v.to_vec())));
                            give(e, want, format!("`{p}` of the rule `{rn}`"), format!("規則 `{rn}` の `{p}`"));
                        }
                    }
                }
            }
            TK::Succeed { fields } => {
                for (f, e) in fields {
                    let want = m.outputs.iter().find(|(n, _)| n == f).and_then(|(_, t)| takes(m, t, m.output_ranges.get(f).copied()));
                    give(e, want, format!("the output `{f}`"), format!("出力 `{f}`"));
                }
            }
            _ => {}
        }
        // every record written out, wherever it is
        for e in exprs(&s.kind) {
            records(e, &mut |fields, r| {
                for (f, x) in fields {
                    let rd = &m.records[r];
                    let want = rd.fields.iter().find(|(n, _)| n == f).and_then(|(_, ft)| takes(m, ft, rd.ranges.get(f).copied()));
                    give(x, want, format!("the field `{f}` of `{}`", rd.name), format!("`{}` のフィールド `{f}`", rd.name));
                }
            });
        }
    }
    out
}

/// Every call of a rule in the flow (ritsu's port of flows, DESIGN 7.4 X2): its line, the rule,
/// and for each input it gives, the value as written, the range that value can be in when every
/// place it comes from has one, and a place it comes from that has none.
#[allow(clippy::type_complexity)]
pub fn rule_calls(m: &Model) -> Vec<(usize, usize, Vec<(String, String, Option<Range>, Option<Text>)>)> {
    let vars = variables(m);
    let mut out = Vec::new();
    for s in m.all_stmts() {
        if let TK::Call { callee: Callee::Rule(r), args, .. } = &s.kind {
            let given = args
                .iter()
                .map(|(p, e)| {
                    let est = estimate(m, &vars, e);
                    let range = if est.unknown.is_none() { est.known } else { None };
                    (p.clone(), e.show(), range, est.unknown.clone())
                })
                .collect();
            out.push((s.line, *r, given));
        }
    }
    out
}

/// The diagnostic for giving `e` to a place whose range is `want`, if it may not fit. A date's and
/// a time's ranges are said in days and moments.
fn fit(m: &Model, vars: &BTreeMap<String, Est>, e: &TExpr, want: Range, line: usize, place_en: &str, place_ja: &str) -> Option<Diag> {
    let est = estimate_by(m, vars, e, Written::Itself, true);
    let ty = e.ty();
    let x = e.show();
    let w = m.range_text(&ty, &want);
    if let Some(have) = est.known {
        if !have.within(&want) {
            let h = m.range_text(&ty, &have);
            return Some(match (have.lo, have.hi) {
                _ if matches!(e, TExpr::Int(_)) => Diag::error("E014", line, 1, Text::new(spaced(format!("`{x}`は{place_ja}の範囲`{w}`の外です")), format!("`{x}` is outside `{w}`, the range of {place_en}"))),
                (Some(a), Some(b)) if a == b => {
                    let a = one_text(&ty, a);
                    Diag::error("E014", line, 1, Text::new(spaced(format!("`{x}`は {a} で、{place_ja}の範囲`{w}`の外です")), format!("`{x}` is {a}, outside `{w}`, the range of {place_en}")))
                }
                _ => Diag::error("E014", line, 1, Text::new(spaced(format!("`{x}`は`{h}`で、{place_ja}の範囲`{w}`を外れることがあります")), format!("`{x}` can be outside `{w}`, the range of {place_en}: it is `{h}`"))),
            });
        }
    }
    let Text { en: from_en, ja: from_ja } = est.unknown?;
    let d = Diag::warning(
        "W104",
        line,
        1,
        Text::new(spaced(format!("`{x}`の範囲が分かりません（{from_ja}に範囲がありません）。{place_ja}が受け取るのは`{w}`です")), format!("nothing says what range `{x}` is in ({from_en} has no range), and {place_en} takes `{w}`")),
    );
    // `now` with no period says nothing of what it is: the period is where to say it
    Some(if from_en == now_text(m).en {
        d.note(match &m.period {
            None => Text::new(
                spaced("`now`は、ワークフローが動く期間を書かなければ、どの時刻にもなりえます。`workflow`の下に`now range >=2026-01-01 <=2027-12-31`のように期間を書くと、`now`はその中の時刻になります".into()),
                "`now` can be any moment unless the workflow says the period it runs in: with `now range >=2026-01-01 <=2027-12-31` under `workflow`, `now` is a moment in it".to_string(),
            ),
            Some(_) => Text::new(
                spaced("`on failure`と`on cancel`の中の`now`は、後始末を時計で止めないよう、動く期間で確かめません。どの時刻にもなりえます".into()),
                "`now` in `on failure` and `on cancel` is not held to the period, so that what settles a run is not stopped by the clock: it can be any moment".to_string(),
            ),
        })
    } else {
        d
    })
}

/// One value of a range of the type `t`, as the range is written: a number, a day or a moment.
fn one_text(t: &Ty, n: i64) -> String {
    let mut inner = t;
    while let Ty::Opt(x) | Ty::List(x) = inner {
        inner = x;
    }
    match inner {
        Ty::Date => day_text(n),
        Ty::Timestamp => moment_text(n),
        _ => n.to_string(),
    }
}

/// Where `now` comes from when nothing holds it to a period: a workflow that says none, or `on
/// failure` and `on cancel`, which the period does not hold.
fn now_text(m: &Model) -> Text {
    match m.period {
        None => tr!("動く期間を書いていないワークフローの `now`", "`now` in a workflow that says no period it runs in"),
        Some(_) => tr!("`on failure` と `on cancel` の中の `now`", "`now` in `on failure` and `on cancel`"),
    }
}

/// The diagnostic for giving `e` to a place that takes only the values `takes` of its enum (the
/// values its `range` lets in, in the enum's order: a rule's input, or a place of the `.flow`), if
/// `e` can be another: E014, said in the values of the enum. The range is written as a rule writes
/// it, by its two ends (`>=basic <=gold`).
fn fit_values(m: &Model, vars: &BTreeMap<String, Est>, e: &TExpr, takes: &[String], line: usize, place_en: &str, place_ja: &str) -> Option<Diag> {
    let est = estimate(m, vars, e);
    // the values `e` can be that the range does not let in, in the enum's order
    let order: Vec<String> = match e.ty().enum_inside() {
        Some(id) => m.enums[id].values.clone(),
        None => est.values.iter().cloned().collect(),
    };
    let outside: Vec<&String> = order.iter().filter(|v| est.values.contains(*v) && !takes.contains(v)).collect();
    if outside.is_empty() {
        return None;
    }
    let x = e.show();
    let w = ends_text(takes);
    let quoted: Vec<String> = outside.iter().map(|v| format!("`{v}`")).collect();
    let (one_en, one_ja) = match quoted.as_slice() {
        [one] => (one.clone(), one.clone()),
        [a, b] => (format!("{a} or {b}"), format!("{a}か{b}")),
        [init @ .., last] => (format!("{} or {last}", init.join(", ")), format!("{}、{last} のどれか", init.join("、"))),
        [] => unreachable!("a value outside was found"),
    };
    // A value of an enum is written out only where its type is known (a call's argument), so a
    // variable, an input, a field or an answer can be any value its range lets in, every value of its
    // enum without one: a `match` does not narrow it.
    let can_be = || {
        Diag::error(
            "E014",
            line,
            1,
            Text::new(spaced(format!("`{x}`は{one_ja}になることがあり、{place_ja}の範囲`{w}`を外れます")), format!("`{x}` can be outside `{w}`, the range of {place_en}: it can be {one_en}")),
        )
    };
    Some(match e {
        TExpr::Enum(..) => Diag::error("E014", line, 1, Text::new(spaced(format!("`{x}`は{place_ja}の範囲`{w}`の外です")), format!("`{x}` is outside `{w}`, the range of {place_en}"))),
        // a list of values written out: what to write is in the values themselves
        TExpr::List { items, .. } if items.iter().all(|i| matches!(i, TExpr::Enum(..))) => can_be(),
        _ => can_be().note(Text::new(
            spaced(format!("値の出どころ（ワークフローの入力、レコードのフィールド、タスクの結果）に`range {w}`を書くか、`match`で分けて、範囲に入る値を書いて渡してください")),
            format!("Write `range {w}` where the value comes from (an input of the workflow, a field of a record, the answer of a task), or branch with `match` and give a value the range lets in, written out"),
        )),
    })
}

/// Japanese with a space on each side of the code in it, where it meets a word.
fn spaced(text: String) -> String {
    let bare = |c: Option<char>| c.is_some_and(|c| !c.is_whitespace() && !"（）、。「」".contains(c));
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut open = false;
    for (i, c) in chars.iter().enumerate() {
        if *c == '`' {
            if !open && bare(out.chars().last()) {
                out.push(' ');
            }
            out.push('`');
            if open && bare(chars.get(i + 1).copied()) {
                out.push(' ');
            }
            open = !open;
        } else {
            out.push(*c);
        }
    }
    out
}

/// How a value of an enum written out counts: as itself (E014), or, for what a `match` can find, as
/// any value of its enum, which the check of a `match` does not narrow by (`held`).
#[derive(Clone, Copy, PartialEq)]
enum Written {
    Itself,
    Any,
}

/// What each variable can be: every value put in it, joined, until nothing more changes. An input
/// and a task's answer of an enum can be any value its range lets in, every value of it without one.
pub(crate) fn variables(m: &Model) -> BTreeMap<String, Est> {
    variables_by(m, Written::Itself, false)
}

/// What the values of an enum each variable holds can be, as the places they come from hold them
/// (the check of a `match` reads it, `held_values`): a place's range lets in its values, a place
/// without one every value of its enum, and so does a value written out.
pub(crate) fn held(m: &Model) -> BTreeMap<String, Est> {
    variables_by(m, Written::Any, false)
}

/// The values of its enum `e` can be where a `match` reads it, in the enum's order, when the places
/// it comes from hold it to fewer than its enum has: the ranges a value is checked against where
/// it comes in (an input, a task's answer, a field) keep it there. None when it can be any of them.
pub(crate) fn held_values(m: &Model, held: &BTreeMap<String, Est>, e: &TExpr) -> Option<Vec<String>> {
    let en = e.ty().enum_inside()?;
    let est = estimate_by(m, held, e, Written::Any, false);
    let order = &m.enums[en].values;
    let values: Vec<String> = order.iter().filter(|v| est.values.contains(*v)).cloned().collect();
    (values.len() < order.len()).then_some(values)
}

/// `days`: the day and the time of a koyomi date are what koyomi counts them to (`koyomi_days`),
/// for what dandori holds to a range itself; else they say nothing of what they are, as in what
/// dandori hands ritsu, which decides them over koyomi's days itself.
fn variables_by(m: &Model, w: Written, days: bool) -> BTreeMap<String, Est> {
    let mut vars: BTreeMap<String, Est> = BTreeMap::new();
    for (n, t) in &m.inputs {
        vars.insert(n.clone(), Est::at(m, t, m.input_ranges.get(n).copied(), tr!("入力 `{n}`", "the input `{n}`")));
    }
    let stmts = m.all_stmts();
    for _ in 0..=m.vars.len() + 1 {
        let mut next = vars.clone();
        let mut put = |name: &str, est: Est| {
            let e = next.entry(name.to_string()).or_default();
            *e = e.join(&est);
        };
        for s in &stmts {
            match &s.kind {
                TK::Call { target: Some(Target::Let(x)), callee: Callee::Task(t), .. } => {
                    let t = &m.tasks[*t];
                    let from = tr!("`{}` の結果", "the answer of `{}`", t.name);
                    put(x, match &t.result {
                        Some(ty) => Est::at(m, ty, t.result_range, from),
                        None => Est::or(t.result_range, from),
                    });
                }
                TK::Assign { name, expr } => put(name, estimate_by(m, &vars, expr, w, days)),
                TK::For { var, list, result, .. } => {
                    put(var, estimate_by(m, &vars, list, w, days));
                    if let Some((r, y)) = result {
                        put(r, estimate_by(m, &vars, y, w, days));
                    }
                }
                TK::Match { expr, arms } => {
                    for a in arms {
                        if let Some(x) = &a.some {
                            put(x, estimate_by(m, &vars, expr, w, days));
                        }
                    }
                }
                _ => {}
            }
        }
        if next == vars {
            break;
        }
        vars = next;
    }
    vars
}

/// What numbers `e` can be, or the numbers inside it when it is a `?` or a list; and for a value of
/// an enum, the values it can be.
pub(crate) fn estimate(m: &Model, vars: &BTreeMap<String, Est>, e: &TExpr) -> Est {
    estimate_by(m, vars, e, Written::Itself, false)
}

fn estimate_by(m: &Model, vars: &BTreeMap<String, Est>, e: &TExpr, w: Written, days: bool) -> Est {
    match e {
        TExpr::Int(n) => Est::range(Range::exactly(*n)),
        TExpr::Enum(v, _) if w == Written::Itself => Est { values: BTreeSet::from([v.clone()]), ..Est::default() },
        TExpr::Enum(_, en) => Est { values: m.enums[*en].values.iter().cloned().collect(), ..Est::default() },
        TExpr::List { items, .. } => items.iter().fold(Est::default(), |a, x| a.join(&estimate_by(m, vars, x, w, days))),
        // the moment a check of the period read: one in the period (DESIGN 20)
        TExpr::NowAt(_) => match &m.period {
            Some(p) => Est::range(p.range),
            None => Est::unknown(now_text(m)),
        },
        TExpr::Now => Est::unknown(now_text(m)),
        TExpr::Var { name, fields, .. } if fields.is_empty() => vars.get(name).cloned().unwrap_or_default(),
        TExpr::Var { name, fields, .. } => {
            // the last field's range, in the record it belongs to
            let mut ty = match m.var_ty(name) {
                Some(t) => t.clone(),
                None => return Est::default(),
            };
            let mut at = None;
            for f in fields {
                let r = match ty.inner() {
                    Ty::Record(r) => *r,
                    _ => return Est::default(),
                };
                at = Some((r, f));
                ty = match m.field_ty(r, f) {
                    Some(t) => t.clone(),
                    None => return Est::default(),
                };
            }
            let (r, f) = at.expect("a field was read");
            let rd = &m.records[r];
            if let (true, RecordOrigin::RuleOutputs(ix)) = (days, &rd.origin) {
                if let Some(k) = koyomi_days(m, *ix, f) {
                    return Est::range(k);
                }
            }
            let from = match &rd.origin {
                RecordOrigin::RuleOutputs(ix) => tr!("規則 `{}` の出力 `{f}`", "the output `{f}` of the rule `{}`", m.rules[*ix].name),
                RecordOrigin::Local | RecordOrigin::Proto { .. } | RecordOrigin::Hold { .. } => tr!("`{}` のフィールド `{f}`", "the field `{f}` of `{}`", rd.name),
            };
            Est::at(m, &ty, m.field_range(r, f), from)
        }
        _ => Est::default(),
    }
}

/// The day of a koyomi date (`day`), or its time (`at`), over every day koyomi counts it to on the
/// whole range of its inputs (`Dates::values`): its fewest and most days, and their times at the
/// calendar's offset. None for a field of no date, and when koyomi does not count the days.
fn koyomi_days(m: &Model, ix: usize, field: &str) -> Option<Range> {
    let d = m.rules[ix].date()?;
    let path = &m.rules[ix].info.path;
    let days = match crate::sources::date_values(path, &d.date) {
        Ok(ritsu_ports::Found::Value(set)) => set,
        _ => return None,
    };
    let (lo, hi) = (*days.first()?, *days.last()?);
    match field {
        "day" => Some(Range { lo: Some(lo), hi: Some(hi) }),
        "at" => {
            let facts = crate::sources::dates(path).ok()?;
            let at = facts.functions.iter().find(|f| f.name == d.date)?.at? as i64;
            let off = d.offset.unwrap_or(0) as i64;
            let moment = |day: i64| day * 86_400 + at * 60 - off * 60;
            Some(Range { lo: Some(moment(lo)), hi: Some(moment(hi)) })
        }
        _ => None,
    }
}

/// The values a statement holds, not looking into them.
fn exprs(k: &TK) -> Vec<&TExpr> {
    match k {
        TK::Call { args, .. } => args.iter().map(|(_, e)| e).collect(),
        TK::Assign { expr, .. } => vec![expr],
        TK::Match { expr, .. } => vec![expr],
        TK::WaitUntil { at } => vec![at],
        TK::For { list, result, .. } => std::iter::once(list).chain(result.as_ref().map(|(_, y)| y)).collect(),
        TK::Succeed { fields } => fields.iter().map(|(_, e)| e).collect(),
        TK::Fail { cause, .. } => cause.iter().collect(),
        _ => vec![],
    }
}

/// Every record of a record type written out in `e`, with its fields.
fn records<'a>(e: &'a TExpr, f: &mut dyn FnMut(&'a [(String, TExpr)], RecordId)) {
    match e {
        TExpr::Record { fields, ty } => {
            if let Ty::Record(r) = ty.inner() {
                f(fields, *r);
            }
            for (_, x) in fields {
                records(x, f);
            }
        }
        TExpr::List { items, .. } => items.iter().for_each(|x| records(x, f)),
        _ => {}
    }
}
