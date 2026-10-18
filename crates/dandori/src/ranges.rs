//! Ranges: what numbers a value can be, checked where a value is given to a place that has a
//! range — an input of a rule, a parameter of a task, a field of a record written out, an output
//! of the workflow. A value can be outside the place's range (E014), or nothing says what it can
//! be (W104). What a variable can be is gathered from every value put in it anywhere in the flow,
//! so a variable that is given 1 in one arm and 40 in another is `>=1 <=40` everywhere.
//!
//! An input of a rule of an ordered enum may declare a range over its values too (`range >=basic
//! <=gold`, which rulec hands over with the values it lets in): the values of the enum a value can
//! be are gathered the same way, a value written out being itself, and an input, a task's answer or
//! a field any value of its enum, which its type says. A value that can be a value the range does
//! not let in is E014, said in those values; the type always says what an enum's value can be, so
//! there is no W104 for one.
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

    /// The same, a value of the type `t`: of an enum, it can be any of the enum's values.
    fn of_type(mut self, m: &Model, t: &Ty) -> Est {
        self.values = every_value(m, t);
        self
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

/// Every value of the enum a value of the type `t` is (inside a `?` or a list); none for a type
/// that is not an enum.
fn every_value(m: &Model, t: &Ty) -> BTreeSet<String> {
    let mut inner = t;
    while let Ty::Opt(x) | Ty::List(x) = inner {
        inner = x;
    }
    match inner {
        Ty::Enum(e) => m.enums[*e].values.iter().cloned().collect(),
        _ => BTreeSet::new(),
    }
}

/// What a place takes: the numbers of a range, or of a rule's input that holds its enum to a
/// range, the values the range lets in (in the enum's order).
enum Takes<'a> {
    Numbers(Range),
    Values(&'a [String]),
}

pub fn check(m: &Model) -> Vec<Diag> {
    let vars = variables(m);
    let mut out = Vec::new();
    for s in m.all_stmts() {
        let mut give = |e: &TExpr, want: Option<Takes>, en: String, ja: String| {
            let d = match want {
                Some(Takes::Numbers(want)) => fit(m, &vars, e, want, s.line, &en, &ja),
                Some(Takes::Values(takes)) => fit_values(m, &vars, e, takes, s.line, &en, &ja),
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
                            give(e, t.param_ranges.get(p).copied().map(Takes::Numbers), format!("the parameter `{p}` of `{}`", t.name), format!("`{}` の引数 `{p}`", t.name));
                        }
                        Callee::Rule(r) => {
                            let rn = &m.rules[*r].name;
                            let want = m.rule_input_range(*r, p).map(Takes::Numbers).or_else(|| m.rule_input_values(*r, p).map(Takes::Values));
                            give(e, want, format!("`{p}` of the rule `{rn}`"), format!("規則 `{rn}` の `{p}`"));
                        }
                    }
                }
            }
            TK::Succeed { fields } => {
                for (f, e) in fields {
                    give(e, m.output_ranges.get(f).copied().map(Takes::Numbers), format!("the output `{f}`"), format!("出力 `{f}`"));
                }
            }
            _ => {}
        }
        // every record written out, wherever it is
        for e in exprs(&s.kind) {
            records(e, &mut |fields, r| {
                for (f, x) in fields {
                    let rd = &m.records[r];
                    give(x, rd.ranges.get(f).copied().map(Takes::Numbers), format!("the field `{f}` of `{}`", rd.name), format!("`{}` のフィールド `{f}`", rd.name));
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

/// The diagnostic for giving `e` to a place whose range is `want`, if it may not fit.
fn fit(m: &Model, vars: &BTreeMap<String, Est>, e: &TExpr, want: Range, line: usize, place_en: &str, place_ja: &str) -> Option<Diag> {
    let est = estimate(m, vars, e);
    let x = e.show();
    let w = want.show();
    if let Some(have) = est.known {
        if !have.within(&want) {
            return Some(match (have.lo, have.hi) {
                _ if matches!(e, TExpr::Int(_)) => Diag::error("E014", line, 1, Text::new(spaced(format!("`{x}`は{place_ja}の範囲`{w}`の外です")), format!("`{x}` is outside `{w}`, the range of {place_en}"))),
                (Some(a), Some(b)) if a == b => Diag::error("E014", line, 1, Text::new(spaced(format!("`{x}`は {a} で、{place_ja}の範囲`{w}`の外です")), format!("`{x}` is {a}, outside `{w}`, the range of {place_en}"))),
                _ => Diag::error(
                    "E014",
                    line,
                    1,
                    Text::new(spaced(format!("`{x}`は`{}`で、{place_ja}の範囲`{w}`を外れることがあります", have.show())), format!("`{x}` can be outside `{w}`, the range of {place_en}: it is `{}`", have.show())),
                ),
            });
        }
    }
    let Text { en: from_en, ja: from_ja } = est.unknown?;
    Some(Diag::warning(
        "W104",
        line,
        1,
        Text::new(spaced(format!("`{x}`の範囲が分かりません（{from_ja}に範囲がありません）。{place_ja}が受け取るのは`{w}`です")), format!("nothing says what range `{x}` is in ({from_en} has no range), and {place_en} takes `{w}`")),
    ))
}

/// The diagnostic for giving `e` to a rule's input that takes only the values `takes` of its enum
/// (the values its `range` lets in, in the enum's order), if `e` can be another: E014, said in the
/// values of the enum. The range is written as a rule writes it, by its two ends (`>=basic <=gold`).
fn fit_values(m: &Model, vars: &BTreeMap<String, Est>, e: &TExpr, takes: &[String], line: usize, place_en: &str, place_ja: &str) -> Option<Diag> {
    let est = estimate(m, vars, e);
    // the values `e` can be that the range does not let in, in the enum's order
    let order: Vec<String> = match e.ty().inner() {
        Ty::Enum(id) => m.enums[*id].values.clone(),
        _ => est.values.iter().cloned().collect(),
    };
    let outside: Vec<&String> = order.iter().filter(|v| est.values.contains(*v) && !takes.contains(v)).collect();
    if outside.is_empty() {
        return None;
    }
    let x = e.show();
    let w = match (takes.first(), takes.last()) {
        (Some(lo), Some(hi)) => format!(">={lo} <={hi}"),
        _ => String::new(),
    };
    let quoted: Vec<String> = outside.iter().map(|v| format!("`{v}`")).collect();
    let (one_en, one_ja) = match quoted.as_slice() {
        [one] => (one.clone(), one.clone()),
        [a, b] => (format!("{a} or {b}"), format!("{a}か{b}")),
        [init @ .., last] => (format!("{} or {last}", init.join(", ")), format!("{}、{last} のどれか", init.join("、"))),
        [] => unreachable!("a value outside was found"),
    };
    // A value of an enum is written out only where its type is known (a call's argument), so a
    // variable, an input, a field or an answer can be any value of its enum: nothing narrows one.
    Some(match e {
        TExpr::Enum(..) => Diag::error("E014", line, 1, Text::new(spaced(format!("`{x}`は{place_ja}の範囲`{w}`の外です")), format!("`{x}` is outside `{w}`, the range of {place_en}"))),
        _ => Diag::error(
            "E014",
            line,
            1,
            Text::new(spaced(format!("`{x}`は{one_ja}になることがあり、{place_ja}の範囲`{w}`を外れます")), format!("`{x}` can be outside `{w}`, the range of {place_en}: it can be {one_en}")),
        )
        .note(Text::new(
            spaced("`.flow`の列挙には範囲を書けません。`match`で分けて、規則の範囲に入る値を書いて渡してください".to_string()),
            "An enum in a `.flow` takes no range: branch with `match`, and give the rule a value its range lets in, written out".to_string(),
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

/// What each variable can be: every value put in it, joined, until nothing more changes. An input
/// and a task's answer of an enum can be any value of it.
pub(crate) fn variables(m: &Model) -> BTreeMap<String, Est> {
    let mut vars: BTreeMap<String, Est> = BTreeMap::new();
    for (n, t) in &m.inputs {
        vars.insert(n.clone(), Est::or(m.input_ranges.get(n).copied(), tr!("入力 `{n}`", "the input `{n}`")).of_type(m, t));
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
                    let answer = Est::or(t.result_range, tr!("`{}` の結果", "the answer of `{}`", t.name));
                    put(x, match &t.result {
                        Some(ty) => answer.of_type(m, ty),
                        None => answer,
                    });
                }
                TK::Assign { name, expr } => put(name, estimate(m, &vars, expr)),
                TK::For { var, list, result, .. } => {
                    put(var, estimate(m, &vars, list));
                    if let Some((r, y)) = result {
                        put(r, estimate(m, &vars, y));
                    }
                }
                TK::Match { expr, arms } => {
                    for a in arms {
                        if let Some(x) = &a.some {
                            put(x, estimate(m, &vars, expr));
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
    match e {
        TExpr::Int(n) => Est::range(Range::exactly(*n)),
        TExpr::Enum(v, _) => Est { values: BTreeSet::from([v.clone()]), ..Est::default() },
        TExpr::List { items, .. } => items.iter().fold(Est::default(), |a, x| a.join(&estimate(m, vars, x))),
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
            let from = match &rd.origin {
                RecordOrigin::RuleOutputs(ix) => tr!("規則 `{}` の出力 `{f}`", "the output `{f}` of the rule `{}`", m.rules[*ix].name),
                RecordOrigin::Local | RecordOrigin::Proto { .. } | RecordOrigin::Hold { .. } => tr!("`{}` のフィールド `{f}`", "the field `{f}` of `{}`", rd.name),
            };
            Est::or(m.field_range(r, f), from).of_type(m, &ty)
        }
        _ => Est::default(),
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
