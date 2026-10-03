//! Ranges: what numbers a value can be, checked where a value is given to a place that has a
//! range — an input of a rule, a parameter of a task, a field of a record written out, an output
//! of the workflow. A value can be outside the place's range (E014), or nothing says what it can
//! be (W104). What a variable can be is gathered from every value put in it anywhere in the flow,
//! so a variable that is given 1 in one arm and 40 in another is `>=1 <=40` everywhere.
//!
//! Where a value comes in (an input, the answer of a task or a rule) its range is checked when
//! the workflow runs, by the code each platform is given; here, the ranges are taken as true.

use crate::diag::Diag;
use crate::model::*;
use std::collections::BTreeMap;

/// What is known of the numbers a value can be.
#[derive(Clone, Debug, Default, PartialEq)]
struct Est {
    /// the numbers that come from places with a range; None when none does (`none`, `[]`)
    known: Option<Range>,
    /// a place without a range the value can come from, named in English and in Japanese
    unknown: Option<(String, String)>,
}

impl Est {
    fn range(r: Range) -> Est {
        Est { known: Some(r), unknown: None }
    }

    fn unknown(en: String, ja: String) -> Est {
        Est { known: None, unknown: Some((en, ja)) }
    }

    fn or(r: Option<Range>, en: String, ja: String) -> Est {
        match r {
            Some(r) => Est::range(r),
            None => Est::unknown(en, ja),
        }
    }

    fn join(&self, o: &Est) -> Est {
        Est {
            known: match (self.known, o.known) {
                (Some(a), Some(b)) => Some(a.hull(&b)),
                (a, b) => a.or(b),
            },
            unknown: self.unknown.clone().or(o.unknown.clone()),
        }
    }
}

pub fn check(m: &Model) -> Vec<Diag> {
    let vars = variables(m);
    let mut out = Vec::new();
    for s in m.all_stmts() {
        let mut give = |e: &TExpr, want: Option<Range>, en: String, ja: String| {
            if let Some(want) = want {
                if let Some(d) = fit(m, &vars, e, want, s.line, &en, &ja) {
                    out.push(d);
                }
            }
        };
        match &s.kind {
            TK::Call { callee, args, .. } => {
                for (p, e) in args {
                    match callee {
                        Callee::Task(t) => {
                            let t = &m.tasks[*t];
                            give(e, t.param_ranges.get(p).copied(), format!("the parameter `{p}` of `{}`", t.name), format!("`{}` の引数 `{p}`", t.name));
                        }
                        Callee::Rule(r) => {
                            let rn = &m.rules[*r].name;
                            give(e, m.rule_input_range(*r, p), format!("`{p}` of the rule `{rn}`"), format!("規則 `{rn}` の `{p}`"));
                        }
                    }
                }
            }
            TK::Succeed { fields } => {
                for (f, e) in fields {
                    give(e, m.output_ranges.get(f).copied(), format!("the output `{f}`"), format!("出力 `{f}`"));
                }
            }
            _ => {}
        }
        // every record written out, wherever it is
        for e in exprs(&s.kind) {
            records(e, &mut |fields, r| {
                for (f, x) in fields {
                    let rd = &m.records[r];
                    give(x, rd.ranges.get(f).copied(), format!("the field `{f}` of `{}`", rd.name), format!("`{}` のフィールド `{f}`", rd.name));
                }
            });
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
                _ if matches!(e, TExpr::Int(_)) => Diag::error("E014", line, 1, format!("`{x}` is outside `{w}`, the range of {place_en}"), spaced(format!("`{x}`は{place_ja}の範囲`{w}`の外です"))),
                (Some(a), Some(b)) if a == b => Diag::error("E014", line, 1, format!("`{x}` is {a}, outside `{w}`, the range of {place_en}"), spaced(format!("`{x}`は {a} で、{place_ja}の範囲`{w}`の外です"))),
                _ => Diag::error(
                    "E014",
                    line,
                    1,
                    format!("`{x}` can be outside `{w}`, the range of {place_en}: it is `{}`", have.show()),
                    spaced(format!("`{x}`は`{}`で、{place_ja}の範囲`{w}`を外れることがあります", have.show())),
                ),
            });
        }
    }
    let (from_en, from_ja) = est.unknown?;
    Some(Diag::warning(
        "W104",
        line,
        1,
        format!("nothing says what range `{x}` is in ({from_en} has no range), and {place_en} takes `{w}`"),
        spaced(format!("`{x}`の範囲が分かりません（{from_ja}に範囲がありません）。{place_ja}が受け取るのは`{w}`です")),
    ))
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

/// What each variable can be: every value put in it, joined, until nothing more changes.
fn variables(m: &Model) -> BTreeMap<String, Est> {
    let mut vars: BTreeMap<String, Est> = BTreeMap::new();
    for (n, _) in &m.inputs {
        vars.insert(n.clone(), Est::or(m.input_ranges.get(n).copied(), format!("the input `{n}`"), format!("入力 `{n}`")));
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
                    put(x, Est::or(t.result_range, format!("the answer of `{}`", t.name), format!("`{}` の結果", t.name)));
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

/// What numbers `e` can be, or the numbers inside it when it is a `?` or a list.
fn estimate(m: &Model, vars: &BTreeMap<String, Est>, e: &TExpr) -> Est {
    match e {
        TExpr::Int(n) => Est::range(Range::exactly(*n)),
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
            let (en, ja) = match &rd.origin {
                RecordOrigin::RuleOutputs(ix) => (format!("the output `{f}` of the rule `{}`", m.rules[*ix].name), format!("規則 `{}` の出力 `{f}`", m.rules[*ix].name)),
                RecordOrigin::Local | RecordOrigin::Proto { .. } => (format!("the field `{f}` of `{}`", rd.name), format!("`{}` のフィールド `{f}`", rd.name)),
            };
            Est::or(m.field_range(r, f), en, ja)
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
