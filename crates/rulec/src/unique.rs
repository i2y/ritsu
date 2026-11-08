//! The `unique` mark on a field of `elements` (§15.290): where it may be written (E148), and what it
//! caps a `sum` over the sequence at (E147).
//!
//! A sequence whose elements each hold a value of their own in a field of an enum or a truth value
//! holds each value once at most. A `sum` over it then comes to at most what the element of each
//! value adds at most, added up over the values — the largest that many of them, where a `count`
//! caps how many elements the sequence has. Past the top of the range the `sum` declares, the
//! generated code refuses at its door a sequence the caller may well send, and until this check
//! nothing said so: a contribution that adds a row with a large value to an open table did it in
//! silence.

use crate::ast::{AggDecl, AggKind, Item, RuleFile, VarDecl};
use crate::diag::{Diag, Severity, Span};
use crate::eval::Val;
use crate::num::Rat;
use crate::types::{Checked, RowBound, Ty};
use std::cmp::Ordering;

/// E148 for every `unique` written where it cannot stand, then E147 for every `sum` a field marked
/// `unique` caps past the top of its range. The second waits for a rule that types: the cap is read
/// off the ranges and the rows the types check works out.
pub fn check(f: &RuleFile, c: &Checked, path: &str) -> Vec<Diag> {
    let mut out = marks(f, c, path);
    if !out.is_empty() || c.diags.iter().any(|d| d.severity == Severity::Error) {
        return out;
    }
    out.extend(reaches(f, c, path));
    out
}

/// Where a line is, in the file it was written in: the rule's, or a contribution's (§15.231).
fn at(sp: &Span, path: &str) -> String {
    match sp.file_path() {
        Some(p) => format!("{p}:{}", sp.line),
        None => format!("{path}:{}", sp.line),
    }
}

fn mark_title() -> String {
    tr!("`unique` は `elements` の、必ずあるフィールドにだけ書けます", "Only a field of `elements` that is always there can be `unique`")
}

fn random_note() -> String {
    tr!(
        "`random` の値は、一つずつ独立に引きます。引いた二つの値が同じにならない保証は無いので、`unique` とは両立しません。",
        "A `random` value is drawn on its own each time, and nothing keeps two draws apart, so it cannot be `unique` as well."
    )
}

/// E148: `unique` on an input's line, on a field that may be absent, or on a `random` one.
fn marks(f: &RuleFile, c: &Checked, path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    for i in &f.inputs {
        let Some(sp) = &i.unique else { continue };
        let mut d = Diag::error("E148", mark_title())
            .at(at(sp, path))
            .mark(sp.clone(), tr!("{} は入力です", "{} is an input", i.name.text))
            .note(tr!(
                "`unique` は、並びの要素どうしでそのフィールドが同じ値にならないという約束です。入力は一回の呼び出しに一つの値なので、比べる相手がありません。",
                "`unique` promises that no two elements of a sequence have the same value of the field. An input holds one value per call, and has nothing to be compared with."
            ));
        if i.random.is_some() {
            d = d.note(random_note());
        }
        out.push(d.note(tr!(
            "`unique` を外してください。並びの要素どうしで違う値をとるという約束なら、`elements` のそのフィールドの行に書きます。",
            "Take `unique` off the line. To say that the elements of a sequence differ in a value, write it on that field of the `elements`."
        )));
    }
    for fd in f.element_fields() {
        let Some(sp) = &fd.unique else { continue };
        let optional = matches!(c.ty_of(&fd.name.text), Some(Ty::Opt(_)));
        if !optional && fd.random.is_none() {
            continue;
        }
        let what = if optional {
            tr!("{} は値の無いことがあるフィールドです（`?`）", "{} may be absent (`?`)", fd.name.text)
        } else {
            tr!("{} は `random` です", "{} is `random`", fd.name.text)
        };
        let mut d = Diag::error("E148", mark_title()).at(at(sp, path)).mark(sp.clone(), what);
        if optional {
            d = d.note(tr!(
                "値の無い要素どうしを同じとみなすかが決まらず、値の無い要素が何件来るかで、合計の上限も決まりません。必ずあるフィールドにするか、`unique` を外してください。",
                "Whether two elements without a value count as the same is not settled, and how many come without one leaves the cap on a sum unsettled too. Make the field one that is always there, or take `unique` off."
            ));
        }
        if fd.random.is_some() {
            d = d.note(random_note());
        }
        out.push(d);
    }
    out
}

/// What a `sum` comes to at most with each value of one field marked `unique` once.
struct Reach {
    field: String,
    total: Rat,
    /// The values that make the total, largest first, each with the most its element adds and the
    /// contributed row that adds it, when one does: the contribution's name and where the row is.
    parts: Vec<(String, Rat, Option<(String, String)>)>,
    /// The cap a `count` puts on the sequence, when it took fewer values than add something.
    capped: Option<i128>,
}

/// E147: a `sum` over a sequence with a field of an enum or a truth value marked `unique`, which can
/// come past the top of its range with every value of that field once.
fn reaches(f: &RuleFile, c: &Checked, path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    for d in f.aggs().filter(|d| d.kind == AggKind::Sum) {
        let Some(el) = f.elements.iter().find(|el| el.name.text == d.over) else { continue };
        let Some(hi) = c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi) else { continue };
        let cap = crate::door::count_cap(f, c, &d.over);
        // Each field marked `unique` caps the sum on its own; the smallest cap is the one that holds.
        let mut best: Option<Reach> = None;
        for fd in el.fields.iter().filter(|fd| fd.unique.is_some()) {
            let Some(r) = reach(f, c, d, fd, cap, path) else { continue };
            if best.as_ref().is_none_or(|b| r.total.cmp_to(b.total) == Ordering::Less) {
                best = Some(r);
            }
        }
        let Some(r) = best else { continue };
        if r.total.cmp_to(hi) != Ordering::Greater {
            continue;
        }
        let sty = c.ty_of(&d.name.text).unwrap_or(Ty::Number);
        let cty = c.ty_of(&d.column.text).unwrap_or(Ty::Number);
        let show = |v: Rat, ty: &Ty| crate::types::fmt_val(v, ty);
        let (field, sum) = (&r.field, &d.name.text);
        let (reach, top) = (show(r.total, &sty), show(hi, &sty));
        let mut listed: Vec<String> = r.parts.iter().take(10).map(|(v, m, _)| format!("{v} {}", show(*m, &cty))).collect();
        if r.parts.len() > 10 {
            listed.push(tr!("ほか {} 個", "and {} more", r.parts.len() - 10));
        }
        let mut diag = Diag::error(
            "E147",
            tr!(
                "{field} が一つずつでも、{sum} は {reach} に届き、範囲の {top} を越えます",
                "With each `{field}` once, `{sum}` can reach {reach}, past the {top} its range takes"
            ),
        )
        .at(at(&d.span, path))
        .mark(d.span.clone(), tr!("{reach} に届きます", "can reach {reach}"))
        .note(tr!(
            "{field} の値ごとに、その要素が足しうる最大を、大きい順に: {}。",
            "The most the element of each `{field}` can add, largest first: {}.",
            listed.join(if crate::i18n::ja() { "、" } else { ", " })
        ));
        if let Some(n) = r.capped {
            diag = diag.note(tr!(
                "`count` の上限で、並びの要素は {n} 個までなので、大きいほうから {n} 個を足しました。",
                "A `count` caps the sequence at {n} elements, so the {n} largest are added."
            ));
        }
        for (v, m, whose) in &r.parts {
            if let Some((name, row)) = whose {
                diag = diag.note(tr!(
                    "  {v} {}: 寄与 {name} が足した行です（{row}）",
                    "  {v} {}: the row contribution {name} adds ({row})",
                    show(*m, &cty)
                ));
            }
        }
        out.push(
            diag.note(tr!(
                "要素どうしで {field} が違う並びでも合計がこの範囲を出るので、生成コードはそういう並びを入口で受け付けません（「{}」）。",
                "The total leaves the range even with no two elements sharing a `{field}`, and the generated code refuses such a sequence at its door (`{}`).",
                crate::door::out_of_range(sum)
            ))
            .note(tr!(
                "{sum} の範囲を広げてその合計のときの答えを決めるか（読む表は広げた範囲で検査し直されます）、行が足す値を下げるか、`count` で並びの件数を絞ってください。どれにするかは業務の判断です。",
                "Widen the range of `{sum}` and decide the answer for such a total (the tables that read it are then checked over the wider range), lower what the rows add, or cap how many elements the sequence has with a `count`. Which is a business decision."
            )),
        );
    }
    out
}

/// What `d` comes to at most with each value of `fd` once (§15.290), or `None` when `fd` is not of
/// an enum or a truth value, or when what an element adds is not bounded above.
fn reach(f: &RuleFile, c: &Checked, d: &AggDecl, fd: &VarDecl, cap: Option<i128>, path: &str) -> Option<Reach> {
    let name = &fd.name.text;
    let ty = c.ty_of(name)?;
    let values: Vec<(String, Val)> = match &ty {
        Ty::Enum(en) => c.enum_ranges.get(name).or_else(|| c.enums.get(en))?.iter().map(|v| (v.clone(), Val::Enum(v.clone()))).collect(),
        Ty::Bool => vec![(crate::kw::TRUE.to_string(), Val::Bool(true)), (crate::kw::FALSE.to_string(), Val::Bool(false))],
        _ => return None,
    };
    let col = &d.column.text;
    let cty = c.ty_of(col)?;
    let whole = || c.ranges.get(col).and_then(|(_, hi)| *hi);
    let tables: Vec<&crate::ast::Table> = f
        .items
        .iter()
        .filter_map(|it| match it {
            Item::Table(t) if t.outputs.iter().any(|o| o.name.text == *col) => Some(t),
            _ => None,
        })
        .collect();
    let mut each: Vec<(String, Rat, Option<(String, String)>)> = Vec::new();
    for (shown, v) in &values {
        let most: Option<(Rat, Option<(String, String)>)> = if tables.is_empty() {
            // A field of the element, a `define`, an open value: the top of the whole column.
            Some((whole()?, None))
        } else {
            let mut most: Option<(Rat, Option<(String, String)>)> = None;
            for t in &tables {
                let oi = t.outputs.iter().position(|o| o.name.text == *col)?;
                let ci = t.inputs.iter().position(|(n, _)| n == name);
                for row in &t.rows {
                    if let Some(ci) = ci
                        && !row.cells.get(ci).is_some_and(|cell| crate::eval::cell_matches(c, cell, v, &ty))
                    {
                        continue;
                    }
                    let top = match c.row_bound(t, row, oi, &cty) {
                        RowBound::Value { bound, .. } => bound.1,
                        RowBound::Nothing => continue,
                        RowBound::Unknown => whole()?,
                    };
                    if most.as_ref().is_none_or(|(m, _)| top.cmp_to(*m) == Ordering::Greater) {
                        let whose = t.contributed.as_ref().map(|(cname, _)| (cname.clone(), at(&row.span, path)));
                        most = Some((top, whose));
                    }
                }
            }
            most
        };
        // A value no row takes adds nothing here; the table that has no row for it is E101's.
        if let Some((m, whose)) = most
            && m.cmp_to(Rat::zero()) == Ordering::Greater
        {
            each.push((shown.clone(), m, whose));
        }
    }
    each.sort_by(|a, b| b.1.cmp_to(a.1));
    let capped = cap.filter(|n| (each.len() as i128) > *n);
    if let Some(n) = capped {
        each.truncate(n.max(0) as usize);
    }
    let total = each.iter().fold(Rat::zero(), |t, (_, m, _)| t.add(*m));
    Some(Reach { field: name.clone(), total, parts: each, capped })
}
