//! What a computed field can come to, and the claims rulec answers for the lowered route
//! (DESIGN 4.1, 4.2, 6.4).
//!
//! - [`reach`]: the values a computation can come to, over the ranges of what it reads (a field it
//!   reads that is computed and writes a range is read within it: a request whose value falls
//!   outside is refused, DESIGN 4.1). E103 is a range nothing the computation comes to is in: no
//!   request can be made.
//! - [`only_when`]: `X is Y only when C`. rulec is asked what the stage's output comes to over each
//!   part of the inputs where C does not hold (`Rules::outputs_over`); an input that reaches Y there
//!   is the example of E306.
//! - [`hidden`]: a field asked for on one condition and shown on another. rulec is asked whether
//!   the question rule [`crate::lower::form::hidden`] can answer true; an input that does is the
//!   example of E105.

use crate::ast::{BinOp, ClaimBody, Expr, ExprKind, Field, TypeExpr};
use crate::diag::Diag;
use crate::lower::words::{self, Cell, ColKind};
use crate::lower::{Ctx, Lowered, Part, RouteNames};
use crate::names::Scope;
use ritsu_base::text::Text;
use ritsu_ports::{Found, Rules, Value};
use ritsu_units::Rat;
use std::path::Path;

/// The least and the most of an interval.
type Span = (Rat, Rat);

fn min(a: Rat, b: Rat) -> Rat {
    if a.cmp_to(b) == std::cmp::Ordering::Greater { b } else { a }
}

fn max(a: Rat, b: Rat) -> Rat {
    if a.cmp_to(b) == std::cmp::Ordering::Less { b } else { a }
}

/// What the computation of a computed field can come to, over the ranges of what it reads; None
/// for a field that is not a computed number, or a computation this does not follow.
pub fn reach(s: &Scope, f: &Field) -> Option<Span> {
    reached(s, f, Reach::Held)
}

/// What the computation of a computed field can come to before any range a computed field writes
/// holds it: over the ranges of the fields a person fills, a computed field it reads taken over all
/// it comes to in the same way. The code ringi generates holds each value to its range after
/// working the form out (DESIGN 4.4), at the ends this passes ([`crate::lower::form::held`]).
pub fn reach_open(s: &Scope, f: &Field) -> Option<Span> {
    reached(s, f, Reach::Open)
}

/// [`reach_open`], a total over any number of its rows up to the most, from none: what the form
/// can work out while its rows are being filled in, before there are as many as it asks for. The
/// copy the code ringi generates works a form out with (`values.rule`, [`crate::lower::form`])
/// takes its totals and what reads them over this, so that a screen can show them for the rows
/// filled in so far (DESIGN 12.2).
pub fn reach_filling(s: &Scope, f: &Field) -> Option<Span> {
    reached(s, f, Reach::Filling)
}

/// What a computation's values are taken over: [`reach`], [`reach_open`] or [`reach_filling`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reach {
    Held,
    Open,
    Filling,
}

fn reached(s: &Scope, f: &Field, how: Reach) -> Option<Span> {
    let e = f.computed.as_ref()?;
    if !matches!(f.ty, TypeExpr::Num(_)) {
        return None;
    }
    let row = row_of(s, f);
    let v = interval(s, row, e, 0, how)?;
    Some(round(f, v))
}

/// The `rows` field `f` is a field of a row of, if any.
fn row_of<'s>(s: &Scope<'s>, f: &Field) -> Option<&'s [Field]> {
    s.fields().iter().find(|r| r.children.iter().any(|c| c.name.text == f.name.text && c.line == f.line)).map(|r| r.children.as_slice())
}

/// What the field's rounding takes the interval to. A rounding keeps the order of two values, so it
/// takes an interval to the one between what it takes the ends to: `down` both ends down, `up` both
/// up, and the ways that round to the nearest the least with a tie taken down and the most with a
/// tie taken up (which holds for each of them). Below zero the interval is widened to the step on
/// both sides.
fn round(f: &Field, (lo, hi): Span) -> Span {
    let Some(r) = &f.round else { return (lo, hi) };
    let Some(step) = words::constant_in(&r.step, &f.ty).map(|v| v.rat()) else { return (lo, hi) };
    if step.num == 0 {
        return (lo, hi);
    }
    let floor = |x: Rat| {
        let q = x.div(step);
        let n = q.num.div_euclid(q.den);
        Rat::int(n).mul(step)
    };
    let ceil = |x: Rat| {
        let q = x.div(step);
        let n = -((-q.num).div_euclid(q.den));
        Rat::int(n).mul(step)
    };
    if lo.cmp_to(Rat::int(0)) == std::cmp::Ordering::Less {
        return (floor(lo), ceil(hi));
    }
    let half = step.div(Rat::int(2));
    match r.mode.text.as_str() {
        "down" => (floor(lo), floor(hi)),
        "up" => (ceil(lo), ceil(hi)),
        _ => (ceil(lo.sub(half)), floor(hi.add(half))),
    }
}

/// What a computation reaches, within the range its field writes (an end the range does not write
/// is the computation's): a request outside the range is refused, and a part of the range the
/// computation does not reach never comes (DESIGN 4.1). None when the two do not meet (E103).
pub fn within(f: &Field, (lo, hi): Span) -> Option<Span> {
    let (wlo, whi) = words::ends(f);
    let lo = wlo.map_or(lo, |w| max(w, lo));
    let hi = whi.map_or(hi, |w| min(w, hi));
    (lo.cmp_to(hi) != std::cmp::Ordering::Greater).then_some((lo, hi))
}

/// The values a field is read with: a field a person fills, its range as written; a computed field,
/// what its computation reaches, within its range if it writes one (unless it is taken open).
fn read_as(s: &Scope, row: Option<&[Field]>, f: &Field, depth: usize, how: Reach) -> Option<Span> {
    let reached = match &f.computed {
        Some(e) => interval(s, row, e, depth + 1, how).map(|v| round(f, v)),
        None => None,
    };
    if how != Reach::Held && f.computed.is_some() {
        return reached;
    }
    if f.range.is_some() {
        if let Some(v) = reached.and_then(|v| within(f, v)) {
            return Some(v);
        }
        let (lo, hi) = words::ends(f);
        return Some((lo?, hi?));
    }
    reached
}

fn interval(s: &Scope, row: Option<&[Field]>, e: &Expr, depth: usize, how: Reach) -> Option<Span> {
    if depth > 32 {
        return None;
    }
    Some(match &e.kind {
        ExprKind::Num(n) => (words::literal(n), words::literal(n)),
        ExprKind::Neg(x) => {
            let (lo, hi) = interval(s, row, x, depth, how)?;
            (Rat::int(0).sub(hi), Rat::int(0).sub(lo))
        }
        ExprKind::Path(p) if p.len() == 1 => {
            let f = row.and_then(|r| r.iter().find(|c| c.name.text == p[0].text)).or_else(|| s.field(&p[0].text))?;
            read_as(s, row, f, depth, how)?
        }
        ExprKind::Sum(p) if p.len() == 2 => {
            let rows = s.field(&p[0].text)?;
            let child = rows.children.iter().find(|c| c.name.text == p[1].text)?;
            let (clo, chi) = read_as(s, Some(&rows.children), child, depth, how)?;
            // a row whose cell (or what its computation reads) is empty adds nothing (DESIGN 4.9)
            let (clo, chi) = if crate::lower::empty_reads_of_row(rows, child).is_empty() { (clo, chi) } else { (min(clo, Rat::int(0)), max(chi, Rat::int(0))) };
            let b = rows.count.as_ref()?;
            // the rows the form asks for at least; while they are filled in, from none
            let nlo = match how {
                Reach::Filling => Rat::int(0),
                _ => b.lo.as_ref().and_then(|l| words::constant(&l.value)).map(|v| v.rat()).unwrap_or(Rat::int(0)),
            };
            let nhi = b.hi.as_ref().and_then(|h| words::constant(&h.value)).map(|v| v.rat())?;
            let c = [clo.mul(nlo), clo.mul(nhi), chi.mul(nlo), chi.mul(nhi)];
            (c.iter().copied().fold(c[0], min), c.iter().copied().fold(c[0], max))
        }
        ExprKind::Bin(op, a, b) => {
            let (alo, ahi) = interval(s, row, a, depth, how)?;
            let (blo, bhi) = interval(s, row, b, depth, how)?;
            match op {
                BinOp::Add => (alo.add(blo), ahi.add(bhi)),
                BinOp::Sub => (alo.sub(bhi), ahi.sub(blo)),
                BinOp::Mul => {
                    let c = [alo.mul(blo), alo.mul(bhi), ahi.mul(blo), ahi.mul(bhi)];
                    (c.iter().copied().fold(c[0], min), c.iter().copied().fold(c[0], max))
                }
                BinOp::Div => {
                    let zero = Rat::int(0);
                    if blo.cmp_to(zero) != std::cmp::Ordering::Greater && bhi.cmp_to(zero) != std::cmp::Ordering::Less {
                        return None;
                    }
                    let c = [alo.div(blo), alo.div(bhi), ahi.div(blo), ahi.div(bhi)];
                    (c.iter().copied().fold(c[0], min), c.iter().copied().fold(c[0], max))
                }
            }
        }
        _ => return None,
    })
}

/// E103: a computed field whose written range holds none of the values its computation comes to,
/// so that no request can be made (DESIGN 4.1). A range that is wider than what it reaches, or
/// takes in only part of it, is no mistake: the part outside is refused at the door.
pub fn unsatisfiable(s: &Scope, out: &mut Vec<Diag>) {
    let all: Vec<&Field> = s.fields().iter().flat_map(|f| std::iter::once(f).chain(f.children.iter())).collect();
    for f in all {
        let (Some(_), Some(b)) = (&f.computed, &f.range) else { continue };
        let Some((lo, hi)) = reach(s, f) else { continue };
        let (wlo, whi) = words::ends(f);
        let below = whi.is_some_and(|w| w.cmp_to(lo) == std::cmp::Ordering::Less);
        let above = wlo.is_some_and(|w| w.cmp_to(hi) == std::cmp::Ordering::Greater);
        if !(below || above) {
            continue;
        }
        let reach_text = format!(">={} <={}", words::written(words::Val::Num(lo), &f.ty), words::written(words::Val::Num(hi), &f.ty));
        out.push(no_value_in_range(s, f, b.span, Some(reach_text)));
    }
}

/// E103 at the range of the computed field `f`: with what the computation comes to, when it is
/// known as ringi writes values.
pub fn no_value_in_range(s: &Scope, f: &Field, at: crate::ast::Span, reach_text: Option<String>) -> Diag {
    let fd = &f.name.text;
    let range_text = words::range_text(f);
    let comes_to = match reach_text {
        Some(reach_text) => tr!("計算した値がとるのは `{reach_text}` です（読む値の範囲から）。この範囲では、どの申請も出せません。", "The computation comes to `{reach_text}` (from the ranges of what it reads). With this range no request can be made."),
        None => tr!("計算した値は、読む値がどの範囲にあっても、この範囲に入りません。この範囲では、どの申請も出せません。", "Whatever the values it reads, the computation comes to no value in this range. With this range no request can be made."),
    };
    s.err("E103", at, tr!("フィールド {fd} の範囲 `{range_text}` に、計算した値は一度も入りません", "The range `{range_text}` of the field {fd} holds no value its computation comes to")).note(comes_to).note(tr!(
        "範囲を、計算した値と重なるように直してください。範囲の外の値になる申請は出せず、範囲が計算した値より広くても狭くても誤りではありません。",
        "Correct the range so that it meets what the computation comes to. A request whose value falls outside the range is refused; a range wider or narrower than that is no mistake."
    ))
}

/// The values of the inputs as the request names them: each field's name and its value, a choice
/// by the name written in the file, in both languages (`金額 = 30_001円、種別 = 経費`).
pub fn shown_inputs(s: &Scope, names: &RouteNames, inputs: &[(String, Value)]) -> Text {
    let each: Vec<Text> = inputs
        .iter()
        .map(|(n, v)| {
            let (reads, kind) = names.inputs.iter().find(|i| i.1 == *n).map(|i| (i.0.clone(), i.2.clone())).unwrap_or((vec![n.clone()], ColKind::Bool));
            let shown = reads.join(".");
            let field = (reads.len() == 1).then(|| s.field(reads[0].trim_end_matches('?'))).flatten();
            let value = value_text(s, field, &kind, v);
            match reads.first() {
                Some(r) if r.ends_with('?') => {
                    let fd = r.trim_end_matches('?');
                    if matches!(v, Value::Bool(true)) { tr!("{fd} は入っている", "{fd} given") } else { tr!("{fd} は空", "{fd} = none") }
                }
                _ => {
                    let t = format!("{shown} = {value}");
                    Text::new(t.clone(), t)
                }
            }
        })
        .collect();
    Text::new(each.iter().map(|t| t.ja.clone()).collect::<Vec<_>>().join("、"), each.iter().map(|t| t.en.clone()).collect::<Vec<_>>().join(", "))
}

/// A value as the request writes it.
pub fn value_text(s: &Scope, field: Option<&Field>, kind: &ColKind, v: &Value) -> String {
    match v {
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => match (field, kind) {
            (Some(f), ColKind::Ordered { step, ty, .. }) => {
                if matches!(ty, TypeExpr::Date) {
                    crate::lex::date_text(*n as i64)
                } else {
                    words::written(words::Val::Num(Rat::int(*n).mul(*step)), &f.ty)
                }
            }
            (_, ColKind::Ordered { step, ty, .. }) => words::written(words::Val::Num(Rat::int(*n).mul(*step)), ty),
            _ => n.to_string(),
        },
        Value::Enum(e) => match field {
            Some(f) => words::choice_written(s, f, e).map(|o| o.name.text.clone()).unwrap_or_else(|| e.clone()),
            None => e.clone(),
        },
        Value::Str(t) => format!("\"{t}\""),
        Value::Date(d) => d.clone(),
        Value::None => "none".into(),
        Value::List(_) => String::new(),
    }
}

/// The ranges of `outputs_over` for a row of cells over the route's inputs: an ordered input
/// between its ends (as integers on the wire), an enum by the places of its values, a bool as 0
/// and 1.
fn ranges(cols: &[(String, ColKind)], cells: &[Cell]) -> Vec<(String, Option<i128>, Option<i128>)> {
    let mut out = Vec::new();
    for ((name, kind), cell) in cols.iter().zip(cells) {
        match (kind, cell) {
            (_, Cell::Any) => {}
            (ColKind::Bool, Cell::Bool(b)) => out.push((name.clone(), Some(*b as i128), Some(*b as i128))),
            (ColKind::Values(all), Cell::Values(vs)) => {
                for v in vs {
                    if let Some(i) = all.iter().position(|x| x == v) {
                        out.push((name.clone(), Some(i as i128), Some(i as i128)));
                    }
                }
            }
            // the values a `not:` cell lets in (a column that may have no value is read whole
            // instead, [`fits`])
            (ColKind::Values(all), Cell::NotValues(vs)) => {
                for (i, _) in all.iter().enumerate().filter(|(_, v)| !vs.contains(v)) {
                    out.push((name.clone(), Some(i as i128), Some(i as i128)));
                }
            }
            (ColKind::Ordered { step, .. }, Cell::Between(lo, hi)) => {
                let wire = |x: Rat| x.div(*step);
                let lo = lo.map(|(x, inc)| if inc { wire(x) } else { wire(x).add(Rat::int(1)) });
                let hi = hi.map(|(y, inc)| if inc { wire(y) } else { wire(y).sub(Rat::int(1)) });
                let whole = |r: Rat, up: bool| if r.is_int() { r.num } else if up { r.num.div_euclid(r.den) + 1 } else { r.num.div_euclid(r.den) };
                out.push((name.clone(), lo.map(|r| whole(r, true)), hi.map(|r| whole(r, false))));
            }
            _ => {}
        }
    }
    out
}

/// Whether the inputs `ex` rulec found over the ranges of a row ([`ranges`]) are in the row where a
/// column may have no value (DESIGN 4.9): `none` where the cell is `none`, and where a `not:` cell
/// lets every value of its column in but some, not one of those. No range holds `none`, so rulec
/// is asked with the column whole there, and what it finds may be outside the row.
fn fits(cols: &[(String, ColKind, bool)], cells: &[Cell], ex: &ritsu_ports::Values) -> bool {
    cols.iter().zip(cells).all(|((name, _, opt), cell)| {
        let v = ex.iter().find(|(n, _)| n == name).map(|(_, v)| v).filter(|v| **v != Value::None);
        match cell {
            Cell::None if *opt => v.is_none(),
            Cell::NotValues(vs) if *opt => !matches!(v, Some(Value::Enum(e)) if vs.contains(e)),
            _ => true,
        }
    })
}

/// E306: for each `X is Y only when C`, the inputs where C does not hold and the stage comes to Y.
pub fn only_when(cx: &Ctx, lowered: &Lowered, rules: &dyn Rules, out: &mut Vec<Diag>) {
    let s = cx.s;
    let (Some(names), Some(route)) = (&lowered.route, lowered.files.iter().find(|f| f.part == Part::Route)) else { return };
    let path = lowered.path(route);
    for c in &s.file.claims {
        let ClaimBody::OnlyWhen { stage, spec, when } = &c.body else { continue };
        let Some((_, output, values)) = names.stages.iter().find(|(n, ..)| *n == stage.text) else { continue };
        let Some((want, _)) = words::spec_value(s, spec) else { continue };
        // the value the route writes for it: by the request's name of each value the file renamed
        let asked = |v: &str| route.renamed.iter().find(|(g, _)| g == v).map(|(_, w)| w.clone()).unwrap_or_else(|| v.to_string());
        let Some((value, _)) = values.iter().find(|(v, _)| asked(v) == want) else {
            continue;
        };
        // the inputs where C does not hold, as rows over the columns it reads
        let mut inputs = crate::lower::route::Inputs::new(cx);
        let Ok(cond) = inputs.cond(when, &[], c.line) else { continue };
        let (used, rows) = crate::lower::form::rows_of(&inputs, &cond);
        let all = inputs.cols();
        // the columns as the route's rule names them; one the route does not read holds nothing
        let cols: Vec<(String, ColKind, bool)> = used
            .iter()
            .map(|i| {
                let reads = &inputs.list[*i].reads;
                let name = names.inputs.iter().find(|x| x.0 == *reads).map(|x| x.1.clone()).unwrap_or_default();
                (name, all[*i].kind.clone(), all[*i].opt)
            })
            .collect();
        let mut found: Option<ritsu_ports::Values> = None;
        let mut undecided: Option<Text> = None;
        for (cells, holds) in rows.iter().filter(|(_, h)| !h) {
            let _ = holds;
            let keep: Vec<(String, ColKind, bool)> = cols.iter().filter(|(n, ..)| !n.is_empty()).cloned().collect();
            let kept_cells: Vec<Cell> = cols.iter().zip(cells).filter(|((n, ..), _)| !n.is_empty()).map(|(_, c)| c.clone()).collect();
            // a column that may have no value: `none`, which no range holds, is read whole
            let held: Vec<(String, ColKind)> = keep.iter().zip(&kept_cells).filter(|((_, _, opt), c)| !(*opt && matches!(c, Cell::None | Cell::NotValues(_)))).map(|((n, k, _), _)| (n.clone(), k.clone())).collect();
            let held_cells: Vec<Cell> = keep.iter().zip(&kept_cells).filter(|((_, _, opt), c)| !(*opt && matches!(c, Cell::None | Cell::NotValues(_)))).map(|(_, c)| c.clone()).collect();
            match rules.outputs_over(&path, output, &ranges(&held, &held_cells)) {
                Ok(Found::Value(vals)) => {
                    if let Some((_, ex)) = vals.iter().find(|(v, _)| matches!(v, Value::Enum(e) if e == value)) {
                        if !fits(&keep, &kept_cells, ex) {
                            // what rulec found is outside the row: whether a request inside it
                            // comes to the value is not settled
                            undecided = Some(tr!("値の無いことがある列で、条件の外の申請を一つに限れません", "a column that may have no value cannot be held to the requests outside the condition"));
                            continue;
                        }
                        found = Some(ex.clone());
                        break;
                    }
                }
                Ok(Found::Undecided(why)) => undecided = Some(why),
                Err(_) => return,
            }
        }
        let name = &c.name.text;
        let st = &stage.text;
        let y = spec.shown();
        let at = crate::ast::Span { line: c.line, col: c.name.span.col, end: c.name.span.end };
        if let Some(ex) = found {
            let shown = shown_inputs(s, names, &ex);
            let (sja, sen) = (shown.ja.clone(), shown.en.clone());
            out.push(
                s.err("E306", at, tr!("主張 {name} が成り立ちません。ステップ {st} が `{y}` になる申請が、条件の外にあります", "The claim {name} does not hold: some request outside the condition comes to `{y}` at the stage {st}"))
                    .note(tr!("その申請: {sja}", "Such a request: {sen}"))
                    .note(tr!("経路を直すか、条件を直してください。", "Correct the route, or the condition."))
                    .with(crate::diag::Spaced { witness: Some(witness(&ex)), rows: None, from: Some("rulec".into()) }),
            );
        } else if let Some(why) = undecided {
            let (ja, en) = (why.ja.clone(), why.en.clone());
            out.push(s.warn("W201", at, tr!("主張 {name} を決められません（{ja}）", "The claim {name} cannot be settled ({en})")).note(tr!("rulec が、条件の外でステップがとる値を決められませんでした。", "rulec could not settle what the stage comes to outside the condition.")));
        }
    }
}

/// E105: a field the question rule says can be asked for and hidden.
pub fn hidden(cx: &Ctx, lowered: &Lowered, rules: &dyn Rules, out: &mut Vec<Diag>) {
    let s = cx.s;
    for lf in lowered.files.iter() {
        let Part::Hidden(field) = &lf.part else { continue };
        let Some(f) = s.field(field) else { continue };
        let path = lowered.path(lf);
        let fd = &f.name.text;
        let at = f.required_when.as_ref().map(|e| e.span).unwrap_or(f.name.span);
        let vals = match rules.outputs_over(&path, "hidden", &[]) {
            Ok(Found::Value(vals)) => vals,
            // the question is left open: said, as a claim left open is (`only_when`), in the words of
            // the form, since the question rule is no file anyone sees
            Ok(Found::Undecided(_)) => {
                out.push(
                    s.warn("W201", at, tr!("フィールド {fd} が、必須なのに見せていないことがあるかを決められません", "Whether the field {fd} may be asked for while it is hidden cannot be settled"))
                        .note(tr!(
                            "`required when` が成り立ち、`shown when` が成り立たない申請があるかを、rulec は決められませんでした。計算した値と、その計算が読む値を、二つの条件が別々に読むと、こうなることがあります。",
                            "Whether there is a request where `required when` holds and `shown when` does not, rulec could not settle. This can happen when the two conditions read a computed value and a value the computation reads, one each."
                        ))
                        .note(tr!(
                            "二つの条件を同じ値で書けるなら（計算した値の代わりに、その計算が読む値で）、そう書き直してください。",
                            "If both conditions can be written over the same value (the value the computation reads, instead of the computed value), rewrite them so."
                        )),
                );
                continue;
            }
            Err(_) => continue,
        };
        let Some((_, ex)) = vals.iter().find(|(v, _)| matches!(v, Value::Bool(true))) else { continue };
        let Some(q) = crate::lower::form::question(cx, f) else { continue };
        // the example's inputs, and what the computations the conditions read come to at it
        let mut said = ex.clone();
        if !q.defines.is_empty()
            && let Ok(got) = rules.eval(&path, ex)
        {
            for d in &q.defines {
                if let Some(v) = got.iter().find(|(n, _)| *n == d.name) {
                    said.push(v.clone());
                }
            }
        }
        let names = RouteNames { inputs: q.inputs.list.iter().map(|i| (i.reads.clone(), i.name.clone(), i.col.kind.clone())).collect(), ..Default::default() };
        let shown = shown_inputs(s, &names, &said);
        let (sja, sen) = (shown.ja.clone(), shown.en.clone());
        out.push(
            s.err("E105", at, tr!("フィールド {fd} は、必須なのに見せていないことがあります", "The field {fd} may be asked for while it is hidden"))
                .note(tr!("その申請: {sja}", "Such a request: {sen}"))
                .note(tr!(
                    "見せていないフィールドは入れられません。`required when` の条件が成り立つときは、`shown when` の条件も成り立つように直してください。",
                    "A hidden field cannot be filled. Correct the conditions so that the field is shown whenever `required when` holds."
                ))
                .with(crate::diag::Spaced { witness: Some(witness(ex)), rows: None, from: Some("rulec".into()) }),
        );
    }
}

/// The example input as the JSON writes it: each input by the lowered rule's name.
pub fn witness(ex: &ritsu_ports::Values) -> ritsu_base::json::Json {
    use ritsu_base::json::Json;
    let inputs: Vec<(String, Json)> = ex
        .iter()
        .map(|(n, v)| {
            let j = match v {
                Value::Bool(b) => Json::from(*b),
                Value::Int(i) => Json::Int(*i),
                Value::Str(t) | Value::Date(t) | Value::Enum(t) => Json::str(t),
                Value::None => Json::Null,
                Value::List(_) => Json::Null,
            };
            (n.clone(), j)
        })
        .collect();
    Json::Obj(vec![("inputs".into(), Json::Obj(inputs))])
}

/// The path the question is asked at, for a test.
pub fn at(lowered: &Lowered, part: &Part) -> Option<std::path::PathBuf> {
    lowered.files.iter().find(|f| f.part == *part).map(|f| lowered.path(f))
}

#[allow(dead_code)]
fn exists(p: &Path) -> bool {
    ritsu_base::fs::exists(p)
}
