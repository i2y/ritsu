//! The form lowered (DESIGN 4.3, 4.4, 4.5, 6.4): the values of every choice to `choices.rule` (an
//! `enums` file the other lowered rules read with `import enum`, so that the form, the route and a
//! table read one enum); the totals, the computations and the conditions of showing and asking for
//! a field to `form.rule`; a computation of business days to `form.cal`; and, for a field that is
//! asked for on a condition and shown on another, the question whether it may be asked for and
//! hidden (E105), as a rule of its own.
//!
//! A total is rulec's `sum … over … of … range`, whose range is the field's written range, and any
//! other computation is a `define … range` with the range its field writes: the lowered rule
//! refuses a request whose total or computation is outside it, at the total and where the value is
//! worked out (DESIGN 4.1, 4.3, 4.4). The check and the reference interpreter read that rule.
//!
//! The code ringi generates reads a copy of it, `values.rule`, written by the same function
//! ([`form_rule`] with `open`): no computation in it is held to a range, a total's range is what it
//! can reach, and a computation of a row that writes a range has its most and its least worked out
//! too (`max`, `min`), so that the glue gets every value back and holds each to its range itself, in
//! the order the rule's door would ([`Held`], DESIGN 12.1). The two differ in those lines only, and
//! in a condition's table that reads a computed field, which the copy writes over all the values the
//! field can come to.

use super::lines::Out;
use super::route::Inputs;
use super::words::{self, Cond, Names};
use super::{Ctx, LFile, Part, what};
use crate::ast::{BinOp, Expr, ExprKind, Field, TypeExpr};
use ritsu_base::text::Text;
use ritsu_ports::LoweredTool;

/// What each name a lowered rule of the form declares reads ([`LFile::reads`]), as it is declared:
/// an input reads itself, a field of an element the element's field (`lines.price`), and a name
/// worked out from others what they read.
#[derive(Default)]
struct Reads(Vec<(String, Vec<String>)>);

impl Reads {
    /// What `name` reads: itself, for an input.
    fn of(&self, name: &str) -> Vec<String> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, r)| r.clone()).unwrap_or_else(|| vec![name.to_string()])
    }

    /// `name`, a field of the element `el`.
    fn field(&mut self, el: &str, name: &str) {
        self.0.push((name.to_string(), vec![format!("{el}.{name}")]));
    }

    /// `name`, worked out from the names `from`.
    fn put<'n>(&mut self, name: &str, from: impl IntoIterator<Item = &'n str>) {
        let mut all: Vec<String> = Vec::new();
        for f in from {
            for r in self.of(f) {
                if !all.contains(&r) {
                    all.push(r);
                }
            }
        }
        self.0.retain(|(n, _)| n != name);
        self.0.push((name.to_string(), all));
    }
}

/// The names an expression of the form reads, each once: the paths of one name in it.
fn names_in(e: &Expr) -> Vec<&str> {
    let mut all = Vec::new();
    e.walk(&mut all);
    let mut out: Vec<&str> = Vec::new();
    for x in all {
        if let ExprKind::Path(p) = &x.kind
            && p.len() == 1
            && !out.contains(&p[0].text.as_str())
        {
            out.push(&p[0].text);
        }
    }
    out
}

/// `choices.rule`: an enum for each `choice` of the form, and for each `choice from` a rule's enum
/// (the rule's values, copied as rulec says them now), and for each enum of a rule's output the
/// route reads or a condition of the form reads. `applied` names the rules the route brings in
/// with `apply`: an output of one is a column of the route's rule, not a `fed by` input, so no
/// file reads the enum and it is not written (written, it would only say the apply's
/// `authority:decider` twice in the Swift the rules make, rulec's W121). None when the form has
/// none.
pub fn choices(cx: &Ctx, applied: &[String]) -> Option<LFile> {
    let s = cx.s;
    let mut out = Out::default();
    let none_w = Text::default();
    let mut enums: Vec<(String, usize, Text)> = Vec::new();
    let named = super::choice_enums(cx);
    let enum_name = |f: &Field| named.iter().find(|(n, _)| *n == f.name.text).map(|(_, e)| e.clone()).unwrap_or_else(|| f.name.ascii().to_string());
    for f in s.file.form.iter() {
        let fd = &f.name.text;
        let w = what(format!("フィールド `{fd}` の選択"), format!("the choices of the field `{fd}`"));
        match &f.ty {
            TypeExpr::Choice => {
                let lowered = words::lowered_values(s, f);
                let vs: Vec<String> = f
                    .options
                    .iter()
                    .zip(&lowered)
                    .map(|(o, (_, v))| match &o.title {
                        Some(t) => format!("{v} \"{}\"", t.replace('"', "'")),
                        None => v.clone(),
                    })
                    .collect();
                enums.push((format!("enum {} = {}", enum_name(f), vs.join(" | ")), f.line, w));
            }
            TypeExpr::ChoiceFrom(p) if p.len() == 2 => {
                let Some(r) = cx.rule(&p[0].text) else { continue };
                let Some(e) = r.facts.enums.iter().find(|e| e.name == p[1].text || e.public == p[1].text) else { continue };
                let vs: Vec<String> = e.values.iter().map(|v| v.public.clone()).collect();
                enums.push((format!("enum {} = {}", enum_name(f), vs.join(" | ")), f.line, w));
            }
            _ => {}
        }
    }
    // the enums of the rules' outputs the route and the conditions read, an optional one's too
    // (`decider : decider?`), which the route reads as an input of that enum that may be `none`;
    // none for an output the route's `apply` answers itself
    let mut seen: Vec<String> = Vec::new();
    for (alias, output, line, condition) in rule_outputs(cx) {
        if !condition && applied.iter().any(|a| *a == alias) {
            continue;
        }
        let Some(r) = cx.rule(&alias) else { continue };
        let Some(o) = r.facts.outputs.iter().find(|o| o.name == output || o.alias == output) else { continue };
        let ty = match &o.ty {
            ritsu_ports::ColumnType::Opt(t) => t.as_ref(),
            t => t,
        };
        let ritsu_ports::ColumnType::Enum { name: en, .. } = ty else { continue };
        let Some(e) = r.facts.enums.iter().find(|e| &e.name == en) else { continue };
        let Some(name) = output_enum(cx, &alias, &o.alias) else { continue };
        if seen.contains(&name) {
            continue;
        }
        seen.push(name.clone());
        let vs: Vec<String> = e.values.iter().map(|v| v.public.clone()).collect();
        enums.push((format!("enum {name} = {}", vs.join(" | ")), line, what(format!("規則の出力 `{alias}.{output}`"), format!("the rule's output `{alias}.{output}`"))));
    }
    if enums.is_empty() {
        return None;
    }
    let name = cx.name();
    out.put(format!("enums {name}_choices v1"), 0, &none_w);
    out.put(format!("description \"What ringi writes from the choices of {}: one enum for each, which the form, the route and the tables read\"", cx.shown()), 0, &none_w);
    out.blank();
    for (l, from, w) in &enums {
        out.put(l, *from, w);
    }
    Some(LFile { name: "choices.rule".into(), tool: LoweredTool::Rulec, part: Part::Choices, text: out.text, lines: out.lines, renamed: super::choices_renamed(cx), given: Vec::new(), reads: Vec::new() })
}

/// The enums `choices.rule` holds for the rules' outputs the route and the conditions read, each
/// by the rule's alias as written and the output's alias, with its name: the ASCII names of the two
/// (`authority_level`), unless rulec reserves it, reads it as a type, or the code it generates makes
/// a type of that name, or another of them is the same, or the same in Pascal case (`a.b_c` and
/// `a_b.c` are both `a_b_c`), when `_value` follows it. The fields' enums are named out of the way of
/// them.
pub fn output_enums(cx: &Ctx) -> Vec<(String, String, String)> {
    let mut names = Names::enums().avoiding(&words::declared(cx.s));
    let mut out: Vec<(String, String, String)> = Vec::new();
    for (alias, output, ..) in rule_outputs(cx) {
        let Some(r) = cx.rule(&alias) else { continue };
        let Some(o) = r.facts.outputs.iter().find(|o| o.name == output || o.alias == output) else { continue };
        let ty = match &o.ty {
            ritsu_ports::ColumnType::Opt(t) => t.as_ref(),
            t => t,
        };
        let ritsu_ports::ColumnType::Enum { .. } = ty else { continue };
        if out.iter().any(|(a, x, _)| *a == alias && *x == o.alias) {
            continue;
        }
        let name = names.take(&format!("{}_{}", words::alias_ascii(cx.s, &alias), o.alias));
        out.push((alias, o.alias.clone(), name));
    }
    out
}

/// The name of the enum `choices.rule` holds for the output `output` (its alias) of the rule the
/// request reads by `alias`.
pub fn output_enum(cx: &Ctx, alias: &str, output: &str) -> Option<String> {
    output_enums(cx).into_iter().find(|(a, o, _)| a == alias && o == output).map(|(.., n)| n)
}

/// Every rule's output (`authority.decider`) the route and the conditions read, with the line,
/// and whether the read is a condition of the form's (`shown when`, `required when`).
fn rule_outputs(cx: &Ctx) -> Vec<(String, String, usize, bool)> {
    let s = cx.s;
    let mut out = Vec::new();
    let add = |out: &mut Vec<(String, String, usize, bool)>, e: &Expr, line: usize| {
        for p in e.paths() {
            if p.len() == 2 && cx.rule(&p[0].text).is_some() {
                out.push((p[0].text.clone(), p[1].text.clone(), line, false));
            }
        }
    };
    match &s.file.route {
        Some(crate::ast::Route::Stages(lines)) => {
            for l in lines {
                if let Some(e) = &l.when {
                    add(&mut out, e, l.line);
                }
                if let crate::ast::Spec::RuleOutput(p) = &l.spec
                    && p.len() == 2
                {
                    out.push((p[0].text.clone(), p[1].text.clone(), l.line, false));
                }
            }
        }
        Some(crate::ast::Route::Table(t)) => {
            for p in &t.inputs {
                if p.len() == 2 {
                    out.push((p[0].text.clone(), p[1].text.clone(), t.line, false));
                }
            }
        }
        _ => {}
    }
    for f in &s.file.form {
        for e in f.shown_when.iter().chain(&f.required_when) {
            for p in e.paths() {
                if p.len() == 2 && cx.rule(&p[0].text).is_some() {
                    out.push((p[0].text.clone(), p[1].text.clone(), f.line, true));
                }
            }
        }
    }
    out
}

/// A computation as rulec writes it: the names of the fields in ASCII (a field the rule names
/// otherwise by its name there), numbers in their units; None where it is not one rulec computes.
fn expr_text(e: &Expr, name_of: &dyn Fn(&str) -> Option<(String, Option<TypeExpr>)>) -> Option<String> {
    Some(match &e.kind {
        ExprKind::Num(n) => {
            let ty = TypeExpr::Num(match n.unit.as_str() {
                "" => ritsu_units::Unit { dim: ritsu_units::Dim::Number, unit: String::new(), tax: None, step: None },
                "%" => ritsu_units::Unit { dim: ritsu_units::Dim::Rate, unit: String::new(), tax: None, step: None },
                u => ritsu_units::Unit { dim: ritsu_units::Dim::Money(u.to_string()), unit: u.to_string(), tax: None, step: None },
            });
            words::written(words::Val::Num(words::literal(n)), &ty)
        }
        ExprKind::Path(p) if p.len() == 1 => name_of(&p[0].text)?.0,
        ExprKind::Neg(x) => format!("-{}", expr_text(x, name_of)?),
        ExprKind::Bin(op, a, b) => {
            let wrap = |x: &Expr| -> Option<String> {
                let t = expr_text(x, name_of)?;
                Some(if matches!(x.kind, ExprKind::Bin(BinOp::Add | BinOp::Sub, ..)) && matches!(op, BinOp::Mul | BinOp::Div) { format!("({t})") } else { t })
            };
            format!("{} {} {}", wrap(a)?, op.sign(), wrap(b)?)
        }
        _ => return None,
    })
}

/// The `round` a computed field writes as rulec's function around its computation:
/// `down(amount * 3%, 1JPY)`.
fn rounded(f: &Field, body: String) -> String {
    match &f.round {
        Some(r) => {
            let step = match (&r.step.kind, &f.ty) {
                (_, TypeExpr::Num(_)) => words::constant_in(&r.step, &f.ty).map(|v| words::written(v, &f.ty)).unwrap_or_default(),
                _ => String::new(),
            };
            format!("{}({body}, {step})", r.mode.text)
        }
        None => body,
    }
}

/// The `round` of a numeric output: the field's, else down to one step (a total of whole yen is on
/// the step already; rulec asks every numeric output for one, its E104).
fn output_round(f: &Field) -> String {
    let TypeExpr::Num(u) = &f.ty else { return String::new() };
    match &f.round {
        Some(r) => {
            let step = words::constant_in(&r.step, &f.ty).map(|v| words::written(v, &f.ty)).unwrap_or_default();
            format!("  round {}({step})", r.mode.text)
        }
        None => format!("  round down({})", words::number(words::step(u), u)),
    }
}

/// `form.rule` and `form.cal`, for what the form has of them.
pub fn form(cx: &Ctx) -> Vec<LFile> {
    let mut out = Vec::new();
    if let Some(f) = form_rule(cx, false) {
        out.push(f);
    }
    if let Some(f) = form_dates(cx) {
        out.push(f);
    }
    out
}

/// `values.rule`, the copy of `form.rule` the code ringi generates reads: None when there is no
/// `form.rule`.
pub fn values(cx: &Ctx) -> Option<LFile> {
    form_rule(cx, true)
}

/// A computed number the glue holds to its range itself, as the door of `form.rule` would: the
/// field, its row's field when it is a computation of a row, the ends it is held to, and the output
/// of `values.rule` each end is compared with (a computation of a row by its most and its least).
#[derive(Clone, Debug)]
pub struct Held {
    pub field: String,
    pub rows: Option<String>,
    /// The least and the most it may be, of what it can come to: the ends the route reads it within
    /// ([`super::route::reach_or_range`]); None for an end it cannot pass.
    pub lo: Option<(String, ritsu_units::Rat)>,
    pub hi: Option<(String, ritsu_units::Rat)>,
    /// The step of its unit, which a value in kaname is written with.
    pub ty: TypeExpr,
    /// The refusal's words when it is outside ([`out_of_range_said`]); for a computation of a row,
    /// with [`INDEX`] where the row it is outside on goes ([`Held::said_at`]).
    pub said: Text,
    /// The stage of rulec's door it is held at: 0 before the rows are walked, 1 in the walk (a
    /// computation of a row), 2 after (a total, and a computation that reads one).
    pub stage: u8,
}

/// Where the index of the row goes in the words of a computation of a row outside its range
/// ([`Held::said`]), until the row is known: `lines[1].fee`, the rows counted from 0.
pub const INDEX: &str = "\u{2}";

impl Held {
    /// The refusal of a computation of a row outside its range on the row `index` (counted from
    /// 0), and the place it names: `lines[1].fee`.
    pub fn said_at(&self, index: usize) -> (Text, String) {
        let k = index.to_string();
        let said = Text::new(self.said.ja.replace(INDEX, &k), self.said.en.replace(INDEX, &k));
        match &self.rows {
            Some(r) => (said, format!("{r}[{k}].{}", self.field)),
            None => (said, self.field.clone()),
        }
    }
}

/// The name of the most (or the least) of the computation of a row `c` in `values.rule`:
/// `fee_most`, or with a number after it when a field of the form, or a condition's output, is
/// named so.
fn extreme(s: &crate::names::Scope, c: &Field, most: bool) -> String {
    let wanted: Vec<String> = s.file.form.iter().flat_map(|f| std::iter::once(f).chain(f.children.iter())).flat_map(|f| [f.name.ascii().to_string(), format!("{}_shown", f.name.ascii()), format!("{}_required", f.name.ascii())]).collect();
    let base = format!("{}_{}", c.name.ascii(), if most { "most" } else { "least" });
    (1..100).map(|k| if k == 1 { base.clone() } else { format!("{base}_{k}") }).find(|n| !wanted.contains(n)).unwrap_or(base)
}

/// The refusal of a computed field outside its range, in the words of the request (DESIGN 4.4):
/// the field as `whole` names it (a computation of a row at its row, `lines[1].fee`), the range it
/// writes, else what it can reach, and the fields it is worked out from, for the requester to
/// change one of them ([`fix_said`]). `f` is a computation of the rows `rows`, or of the form;
/// `values` is the request's lowered `values.rule`, by which the kind's description says what a
/// computation reads ([`super::computed_reads`]). The reference interpreter says it when the door
/// of `form.rule` refuses, and the glue when it holds the value itself.
pub fn out_of_range_said(s: &crate::names::Scope, values: Option<&LFile>, f: &Field, rows: Option<&Field>, whole: &str) -> Text {
    let r = if f.range.is_some() {
        words::range_text(f)
    } else {
        let (lo, hi) = super::route::reach_or_range(s, f);
        super::route::range_of(f, lo, hi)
    };
    let (mut ja, mut en) = (format!("計算した `{whole}` が、範囲（`{r}`）の外です"), format!("the `{whole}` the form works out is outside its range (`{r}`)"));
    if let Some(fix) = fix_said(s, &super::computed_reads(s, values, f, rows)) {
        ja = format!("{ja}。{}", fix.ja);
        en = format!("{en}; {}", fix.en);
    }
    Text::new(ja, en)
}

/// The fields of the form `reads` names (a field by its ASCII name, a field of a row by the rows'
/// and its: `lines.price`), for the requester to change one of them, as the request names them
/// and the screens say them (DESIGN 13.7): `` change `price` (`lines`) ``, `` `明細` の `単価` を
/// 直してください ``; the fields of one rows, when they are all it reads, with the rows once:
/// `` change `quantity` or `price` (`lines`) ``, `` `明細` の `数量` か `単価` を直してください ``;
/// three or more as `` `a`, `b` or `c` ``, `` `a`、`b`、`c` のどれか ``. None where it names no
/// field of the form.
pub fn fix_said(s: &crate::names::Scope, reads: &[String]) -> Option<Text> {
    let mut named: Vec<(&Field, Option<&Field>)> = Vec::new();
    for r in reads {
        let (of, cell) = match r.split_once('.') {
            Some((of, cell)) => (of, Some(cell)),
            None => (r.as_str(), None),
        };
        let Some(top) = s.file.form.iter().find(|f| f.name.ascii() == of) else { continue };
        let read = match cell {
            Some(cell) => match top.children.iter().find(|c| c.name.ascii() == cell) {
                Some(c) => (c, Some(top)),
                None => continue,
            },
            None => (top, None),
        };
        if !named.iter().any(|(x, _)| std::ptr::eq(*x, read.0)) {
            named.push(read);
        }
    }
    let first = named.first()?;
    // the rows of them all, or none
    let rows = first.1.filter(|r| named.iter().all(|(_, of)| of.is_some_and(|o| std::ptr::eq(o, *r))));
    let one = |c: &Field, of: Option<&Field>, ja: bool| match of.filter(|_| rows.is_none()) {
        Some(o) if ja => format!("`{}` の `{}`", o.name.text, c.name.text),
        Some(o) => format!("`{}` (`{}`)", c.name.text, o.name.text),
        None => format!("`{}`", c.name.text),
    };
    let (ja, en): (Vec<String>, Vec<String>) = named.iter().map(|(c, of)| (one(c, *of, true), one(c, *of, false))).unzip();
    let ja = match ja.as_slice() {
        [a] => a.clone(),
        [a, b] => format!("{a} か {b}"),
        all => format!("{} のどれか", all.join("、")),
    };
    let en = match en.as_slice() {
        [] => String::new(),
        [a] => a.clone(),
        [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
    };
    Some(match rows {
        Some(r) => Text::new(format!("`{}` の {ja} を直してください", r.name.text), format!("change {en} (`{}`)", r.name.text)),
        None => Text::new(format!("{ja} を直してください"), format!("change {en}")),
    })
}

/// The computed numbers the glue holds to their ranges, in the order rulec's door holds them: the
/// computations before the rows are walked, those of each row, then the totals and what reads them;
/// each in the order the form writes it. Only the ends what a value can come to passes. `lf` is
/// the request's `values.rule`, whose names the outputs are read by.
pub fn held(s: &crate::names::Scope, lf: &LFile) -> Vec<Held> {
    let named = |want: &str| lf.renamed.iter().find(|(_, w)| w == want).map(|(g, _)| g.clone()).unwrap_or_else(|| want.to_string());
    // whether a computation reads a total, itself or through another computation
    fn reads_total(s: &crate::names::Scope, e: &Expr, depth: usize) -> bool {
        let mut all = Vec::new();
        e.walk(&mut all);
        all.iter().any(|x| match &x.kind {
            ExprKind::Sum(_) => true,
            ExprKind::Path(p) if p.len() == 1 && depth < 32 => s.field(&p[0].text).and_then(|g| g.computed.as_ref()).is_some_and(|c| reads_total(s, c, depth + 1)),
            _ => false,
        })
    }
    let ends = |f: &Field, most: String, least: String| -> (Option<(String, ritsu_units::Rat)>, Option<(String, ritsu_units::Rat)>) {
        let (lo, hi) = super::route::reach_or_range(s, f);
        let open = crate::claims::reach_open(s, f);
        let below = |l: ritsu_units::Rat| open.is_none_or(|(olo, _)| olo.cmp_to(l) == std::cmp::Ordering::Less);
        let above = |h: ritsu_units::Rat| open.is_none_or(|(_, ohi)| ohi.cmp_to(h) == std::cmp::Ordering::Greater);
        (lo.filter(|l| below(*l)).map(|l| (least, l)), hi.filter(|h| above(*h)).map(|h| (most, h)))
    };
    let mut out = Vec::new();
    for f in s.file.form.iter().filter(|f| computed_number(f)) {
        let name = named(f.name.ascii());
        let (lo, hi) = ends(f, name.clone(), name.clone());
        let stage = if reads_total(s, f.computed.as_ref().unwrap(), 0) { 2 } else { 0 };
        out.push(Held { field: f.name.ascii().to_string(), rows: None, lo, hi, ty: f.ty.clone(), said: out_of_range_said(s, Some(lf), f, None, &f.name.text), stage });
    }
    for rows in s.file.form.iter().filter(|f| f.children.iter().any(computed_number)) {
        for c in rows.children.iter().filter(|c| computed_number(c) && c.range.is_some()) {
            let (most, least) = (named(&extreme(s, c, true)), named(&extreme(s, c, false)));
            let (lo, hi) = ends(c, most, least);
            let whole = format!("{}[{INDEX}].{}", rows.name.text, c.name.text);
            out.push(Held { field: c.name.ascii().to_string(), rows: Some(rows.name.ascii().to_string()), lo, hi, ty: c.ty.clone(), said: out_of_range_said(s, Some(lf), c, Some(rows), &whole), stage: 1 });
        }
    }
    out.retain(|h| h.lo.is_some() || h.hi.is_some());
    out.sort_by_key(|h| h.stage);
    out
}

/// Whether a field is computed by rulec (a number), not koyomi (a date).
fn computed_number(f: &Field) -> bool {
    f.computed.is_some() && matches!(f.ty, TypeExpr::Num(_))
}

/// `form.rule`, or with `open` its copy `values.rule`, which the code ringi generates reads: no
/// computation held to a range, a total's range what it can reach, and the most and the least of a
/// computation of a row that writes a range as outputs ([`Held`]).
fn form_rule(cx: &Ctx, open: bool) -> Option<LFile> {
    let s = cx.s;
    let has_rows_computed = s.file.form.iter().any(|f| f.children.iter().any(computed_number));
    let conds: Vec<&Field> = s.file.form.iter().filter(|f| f.shown_when.is_some() || f.required_when.is_some()).collect();
    if !s.file.form.iter().any(computed_number) && !has_rows_computed && conds.is_empty() {
        return None;
    }
    let mut inputs = Inputs::new(cx);
    inputs.open = open;
    inputs.computes = true;
    let none_w = Text::default();
    let mut body = Out::default();
    let mut reads = Reads::default();
    // whether each field of a row that may be empty is filled in, as a field of the element, by the
    // rows' and the field's ASCII names (`items.price`), which the reference and the glue give
    let mut row_given: Vec<(String, String)> = Vec::new();
    // the most and the least of a computation of a row, which the copy writes as outputs
    let mut extremes: Vec<(String, String, usize, Text)> = Vec::new();
    // the rows a total or a computation reads, with their fields
    let mut elements: Vec<&Field> = Vec::new();
    for f in s.file.form.iter().filter(|f| computed_number(f)) {
        let mut all = Vec::new();
        f.computed.as_ref().unwrap().walk(&mut all);
        for e in all {
            if let ExprKind::Sum(p) = &e.kind
                && let Some(rows) = s.field(&p[0].text)
                && !elements.iter().any(|x| x.name.text == rows.name.text)
            {
                elements.push(rows);
            }
        }
    }
    // the names the rule declares: the elements and their fields, the totals and computations. A
    // field of a row is a name of the rule's, as an element is (rulec has one space of names).
    let mut lowered_rows: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for rows in &elements {
        let el = inputs.names.take(rows.name.ascii());
        let children = rows.children.iter().map(|c| (c.name.text.clone(), inputs.names.take(c.name.ascii()))).collect();
        lowered_rows.push((el, children));
    }
    // for each rows field, the name of the field of an element that says a row counts for a cell or a
    // computation of a row: none where it always does
    let mut counted: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for (rows, (el, children)) in elements.iter().zip(&lowered_rows) {
        let rd = &rows.name.text;
        let child = |c: &Field| children.iter().find(|(n, _)| *n == c.name.text).map(|(_, l)| l.clone()).unwrap_or_else(|| c.name.ascii().to_string());
        body.put(format!("elements {el}"), rows.line, &what(format!("フィールド `{rd}`"), format!("the field `{rd}`")));
        let plain: Vec<&Field> = rows.children.iter().filter(|c| c.computed.is_none() && matches!(c.ty, TypeExpr::Num(_) | TypeExpr::Date | TypeExpr::Bool)).collect();
        // a cell that may be empty is its value and whether it is filled in (DESIGN 4.9)
        let given: Vec<(String, String)> = plain.iter().filter(|c| super::may_be_empty(c)).map(|c| (c.name.text.clone(), inputs.names.take(&format!("{}_given", c.name.ascii())))).collect();
        let fields: Vec<(String, String, usize, Text)> = plain
            .iter()
            .flat_map(|c| {
                let cd = &c.name.text;
                let w = what(format!("フィールド `{rd}.{cd}`"), format!("the field `{rd}.{cd}`"));
                let range = words::range_text(c);
                let range = if range.is_empty() { String::new() } else { format!("  range {range}") };
                let mut one = vec![(child(c), format!("{}{range}", super::route::field_type(c)), c.line, w.clone())];
                if let Some((_, g)) = given.iter().find(|(n, _)| *n == c.name.text) {
                    one.push((g.clone(), "bool".to_string(), c.line, w));
                }
                one
            })
            .collect();
        let w = fields.iter().map(|f| f.0.chars().count()).max().unwrap_or(0);
        for (n, ty, line, wh) in &fields {
            body.put(format!("  {n}{} : {ty}", " ".repeat(w - n.chars().count())), *line, wh);
            reads.field(el, n);
        }
        body.blank();
        for (c, g) in &given {
            if let Some(x) = rows.children.iter().find(|x| x.name.text == *c) {
                row_given.push((g.clone(), format!("{}.{}", rows.name.ascii(), x.name.ascii())));
            }
        }
        // the cells of a row that may be empty each cell and computation reads: the row counts for it
        // where they are all filled in, which a table of the element says for two or more
        let mut flags: Vec<(String, String)> = Vec::new();
        for c in rows.children.iter().filter(|c| c.computed.is_none() || computed_number(c)) {
            let empty = row_empty_reads(rows, c);
            let gs: Vec<String> = empty.iter().filter_map(|r| given.iter().find(|(n, _)| *n == r.name.text).map(|(_, g)| g.clone())).collect();
            match gs.as_slice() {
                [] => {}
                [one] => flags.push((c.name.text.clone(), one.clone())),
                more => {
                    let cd = &c.name.text;
                    let flag = inputs.names.take(&format!("{}_there", c.name.ascii()));
                    let w = what(format!("フィールド `{rd}.{cd}` が値を持つ行"), format!("the rows the field `{rd}.{cd}` has a value in"));
                    let cols: Vec<words::Col> = more.iter().map(|g| words::Col { name: g.clone(), kind: words::ColKind::Bool, opt: false }).collect();
                    let all = more.iter().enumerate().skip(1).fold(Cond::Atom(words::Atom::True(0)), |acc, (i, _)| Cond::And(Box::new(acc), Box::new(Cond::Atom(words::Atom::True(i)))));
                    let rows_of: Vec<(Vec<String>, usize, Text)> = words::rows(&cols, &all)
                        .into_iter()
                        .map(|(cells, holds)| {
                            let mut t: Vec<String> = cells.iter().zip(&cols).map(|(c, col)| words::cell_text(col, c)).collect();
                            t.push(holds.to_string());
                            (t, c.line, w.clone())
                        })
                        .collect();
                    body.put(format!("table {flag}_by"), c.line, &w);
                    body.table(more, &[format!("{flag} : bool")], (c.line, &w), &rows_of);
                    body.blank();
                    reads.put(&flag, more.iter().map(String::as_str));
                    flags.push((c.name.text.clone(), flag));
                }
            }
        }
        // a computation of a row: a `define` on each element; one that may have no value is held to
        // its range where it has one, by the reference and the glue (DESIGN 4.9)
        for c in rows.children.iter().filter(|c| computed_number(c)) {
            let cd = &c.name.text;
            let name_of = |n: &str| rows.children.iter().find(|x| x.name.text == n).map(|x| (child(x), Some(x.ty.clone())));
            let Some(t) = expr_text(c.computed.as_ref().unwrap(), &name_of) else { continue };
            let held = if open || flags.iter().any(|(n, _)| *n == c.name.text) { String::new() } else { accepted(c) };
            body.put(format!("define {} : {} = {}{held}", child(c), super::route::field_type(c), rounded(c, t)), c.line, &what(format!("フィールド `{rd}.{cd}` の計算"), format!("the computation of the field `{rd}.{cd}`")));
            body.blank();
            let from: Vec<String> = names_in(c.computed.as_ref().unwrap()).into_iter().filter_map(|n| name_of(n).map(|(x, _)| x)).collect();
            reads.put(&child(c), from.iter().map(String::as_str));
        }
        // what a total adds of a row: the cell, or zero where the row does not count for it
        let summed: Vec<String> = s
            .file
            .form
            .iter()
            .filter_map(|f| match f.computed.as_ref().map(|e| &e.kind) {
                Some(ExprKind::Sum(p)) if p.len() == 2 && p[0].text == rows.name.text => Some(p[1].text.clone()),
                _ => None,
            })
            .collect();
        let mut sums: Vec<(String, String)> = Vec::new();
        for (cn, flag) in &flags {
            let Some(c) = rows.children.iter().find(|x| x.name.text == *cn && summed.contains(&x.name.text)) else { continue };
            let TypeExpr::Num(u) = &c.ty else { continue };
            let cd = &c.name.text;
            let n = inputs.names.take(&format!("{}_counted", c.name.ascii()));
            let w = what(format!("合計が数えるフィールド `{rd}.{cd}`"), format!("the field `{rd}.{cd}` as the total counts it"));
            let zero = words::number(ritsu_units::Rat::int(0), u);
            let rows_of = vec![(vec!["true".to_string(), child(c)], c.line, w.clone()), (vec!["false".to_string(), zero], c.line, w.clone())];
            body.put(format!("table {n}_by"), c.line, &w);
            body.table(std::slice::from_ref(flag), &[format!("{n} : {}", super::route::field_type(c))], (c.line, &w), &rows_of);
            body.blank();
            reads.put(&n, [flag.as_str(), child(c).as_str()]);
            sums.push((cn.clone(), n));
        }
        counted.push((rows.name.text.clone(), sums));
        // the copy: the most and the least of each computation of the row that writes a range, which
        // the glue holds to it
        if open {
            let mut any = false;
            for c in rows.children.iter().filter(|c| computed_number(c) && c.range.is_some()) {
                let cd = &c.name.text;
                let (lo, hi) = super::route::reach_or_range(s, c);
                let reach = crate::claims::reach_open(s, c);
                let empty = lo.or(hi).map(|v| words::written(words::Val::Num(v), &c.ty)).unwrap_or_default();
                let w = what(format!("フィールド `{rd}.{cd}` の範囲"), format!("the range of the field `{rd}.{cd}`"));
                for (word, end, passes) in [
                    ("max", hi, hi.is_some_and(|h| reach.is_none_or(|(_, r)| r.cmp_to(h) == std::cmp::Ordering::Greater))),
                    ("min", lo, lo.is_some_and(|l| reach.is_none_or(|(r, _)| r.cmp_to(l) == std::cmp::Ordering::Less))),
                ] {
                    if end.is_none() || !passes {
                        continue;
                    }
                    let n = inputs.names.take(&extreme(s, c, word == "max"));
                    // over the rows where the computation has a value
                    let only = flags.iter().find(|(x, _)| *x == c.name.text).map(|(_, g)| format!(" where {g}")).unwrap_or_default();
                    body.put(format!("{word} {n} over {el} of {}{only}  empty {empty}", child(c)), c.line, &w);
                    let from = [Some(child(c)), flags.iter().find(|(x, _)| *x == c.name.text).map(|(_, g)| g.clone())];
                    reads.put(&n, from.iter().flatten().map(String::as_str));
                    extremes.push((n, format!("{}{}", super::route::field_type(c), output_round(c)), c.line, w.clone()));
                    any = true;
                }
            }
            if any {
                body.blank();
            }
        }
    }
    // the fields a computation reads that are not computed: inputs of the rule, declared before the
    // computations that name them
    for f in s.file.form.iter().filter(|f| computed_number(f)) {
        let mut all = Vec::new();
        f.computed.as_ref().unwrap().walk(&mut all);
        for e in all {
            if let ExprKind::Path(p) = &e.kind
                && p.len() == 1
                && let Some(x) = s.field(&p[0].text)
                && x.computed.is_none()
            {
                inputs.field(x, false);
            }
        }
    }
    // the totals and the computations of the form, in its order
    let mut outputs: Vec<(String, String, usize, Text)> = Vec::new();
    for f in s.file.form.iter().filter(|f| computed_number(f)) {
        let fd = &f.name.text;
        let e = f.computed.as_ref().unwrap();
        let name = inputs.names.take(f.name.ascii());
        let w = what(format!("フィールド `{fd}` の計算"), format!("the computation of the field `{fd}`"));
        match &e.kind {
            ExprKind::Sum(p) if p.len() == 2 => {
                let rows = s.field(&p[0].text)?;
                let child = rows.children.iter().find(|c| c.name.text == p[1].text)?;
                // the range written is the door's; with none written, what the total can reach. The
                // copy's is what it can reach over any number of rows up to the most, from none (a
                // screen works the rows filled in so far out), its rows' computations not held to
                // their ranges.
                let range = if open {
                    let (lo, hi) = crate::claims::reach_filling(cx.s, f).map_or((None, None), |(lo, hi)| (Some(lo), Some(hi)));
                    super::route::range_of(f, lo, hi)
                } else if f.range.is_some() {
                    words::range_text(f)
                } else {
                    let (lo, hi) = super::route::reach_or_range(cx.s, f);
                    super::route::range_of(f, lo, hi)
                };
                let (el, children) = elements.iter().zip(&lowered_rows).find(|(x, _)| x.name.text == rows.name.text).map(|(_, l)| l)?;
                let of = children.iter().find(|(n, _)| *n == child.name.text).map(|(_, l)| l.clone()).unwrap_or_else(|| child.name.ascii().to_string());
                // a row whose cell (or what its computation reads) is empty is not counted: it adds zero
                let of = counted.iter().find(|(r, _)| *r == rows.name.text).and_then(|(_, fl)| fl.iter().find(|(x, _)| *x == child.name.text)).map(|(_, n)| n.clone()).unwrap_or(of);
                body.put(format!("sum {name} over {el} of {of}  range {range}"), f.line, &w);
                body.blank();
                reads.put(&name, [of.as_str()]);
            }
            _ => {
                let name_of = |n: &str| -> Option<(String, Option<TypeExpr>)> {
                    let x = s.field(n)?;
                    let named = inputs.list.iter().find(|i| i.reads == vec![x.name.text.clone()]).map(|i| i.name.clone());
                    Some((named.unwrap_or_else(|| x.name.ascii().to_string()), Some(x.ty.clone())))
                };
                let Some(t) = expr_text(e, &name_of) else { continue };
                // one that may have no value is worked out over the values the fields it reads hold
                // where they are empty, and held to its range where it has one (DESIGN 4.9)
                let held = if open || super::may_be_none(s, f) { String::new() } else { accepted(f) };
                body.put(format!("define {name} : {} = {}{held}", super::route::field_type(f), rounded(f, t)), f.line, &w);
                body.blank();
                let from: Vec<String> = names_in(e).into_iter().filter_map(|n| name_of(n).map(|(x, _)| x)).collect();
                reads.put(&name, from.iter().map(String::as_str));
            }
        }
        inputs.derived(f, &name);
        outputs.push((name, format!("{}{}", super::route::field_type(f), output_round(f)), f.line, w));
    }
    outputs.extend(extremes);
    // the conditions: an output for each, and a table of both sides
    let mut tables: Vec<(String, Vec<usize>, Vec<(Vec<words::Cell>, bool)>, usize, Text)> = Vec::new();
    for f in &conds {
        let fd = &f.name.text;
        for (word, e, ja, en) in [("shown", &f.shown_when, "見せる条件", "the condition it is shown on"), ("required", &f.required_when, "必須にする条件", "the condition it is asked for on")] {
            let Some(e) = e else { continue };
            let Ok(cond) = inputs.cond(e, &[], f.line) else { continue };
            let name = inputs.names.take(&format!("{}_{word}", f.name.ascii()));
            let w = what(format!("フィールド `{fd}` の{ja}"), format!("{en} of the field `{fd}`"));
            let (used, rows) = rows_of(&inputs, &cond);
            let cols = inputs.cols();
            reads.put(&name, used.iter().map(|i| cols[*i].name.as_str()));
            outputs.push((name.clone(), "bool".into(), f.line, w.clone()));
            tables.push((name, used, rows, f.line, w));
        }
    }
    let mut out = Out::default();
    let name = cx.name();
    if open {
        out.put(format!("rule {name}_values v1"), 0, &none_w);
        out.put(format!("description \"What ringi writes from the form of {} for the code it generates: the totals and computations over all the values what they read can take, and when a field is shown and asked for; the code holds each computation to its range\"", cx.shown()), 0, &none_w);
    } else {
        out.put(format!("rule {name}_form v1"), 0, &none_w);
        out.put(format!("description \"What ringi writes from the form of {}: the totals and computations, and when a field is shown and asked for\"", cx.shown()), 0, &none_w);
    }
    out.blank();
    let imports = inputs.imports();
    for e in &imports {
        out.put(format!("import enum {e} from \"choices.rule\""), 0, &none_w);
    }
    if !imports.is_empty() {
        out.blank();
    }
    let before = out.lines.len();
    inputs.write(&mut out);
    if out.lines.len() > before {
        out.blank();
    }
    out.text.push_str(&body.text);
    out.lines.extend(body.lines);
    out.put("outputs", 0, &none_w);
    let w = outputs.iter().map(|o| o.0.chars().count()).max().unwrap_or(0);
    for (n, ty, from, wh) in &outputs {
        out.put(format!("  {n}{} : {ty}", " ".repeat(w - n.chars().count())), *from, wh);
    }
    let cols = inputs.cols();
    for (tname, used, rows, from, w) in &tables {
        out.blank();
        out.put(format!("table {tname}"), *from, w);
        let heads: Vec<String> = used.iter().map(|i| cols[*i].name.clone()).collect();
        let sub: Vec<&words::Col> = used.iter().map(|i| &cols[*i]).collect();
        let texts: Vec<(Vec<String>, usize, Text)> = rows
            .iter()
            .map(|(cells, holds)| {
                let mut t: Vec<String> = cells.iter().zip(&sub).map(|(c, col)| words::cell_text(col, c)).collect();
                t.push(holds.to_string());
                (t, *from, w.clone())
            })
            .collect();
        out.table(&heads, std::slice::from_ref(tname), (*from, w), &texts);
    }
    let (file, part) = if open { ("values.rule", Part::Values) } else { ("form.rule", Part::Form) };
    let mut given = inputs.given_columns();
    given.extend(row_given);
    let reads = outputs.iter().map(|(n, ..)| (n.clone(), reads.of(n))).collect();
    Some(LFile { name: file.into(), tool: LoweredTool::Rulec, part, text: out.text, lines: out.lines, renamed: inputs.names.renamed(), given, reads })
}

/// The cells of a row that may be empty a cell or a computation of the row reads, itself or through
/// the row's other computations, in the row's order: the cell itself, for a cell that may be empty.
pub(crate) fn row_empty_reads<'f>(rows: &'f Field, c: &'f Field) -> Vec<&'f Field> {
    fn walk<'f>(rows: &'f Field, c: &'f Field, depth: usize, out: &mut Vec<&'f Field>) {
        match &c.computed {
            None => {
                if super::may_be_empty(c) && !out.iter().any(|x| x.name.text == c.name.text) {
                    out.push(c);
                }
            }
            Some(e) if depth < 32 => {
                let mut all = Vec::new();
                e.walk(&mut all);
                for x in all {
                    if let ExprKind::Path(p) = &x.kind
                        && p.len() == 1
                        && let Some(y) = rows.children.iter().find(|y| y.name.text == p[0].text)
                    {
                        walk(rows, y, depth + 1, out);
                    }
                }
            }
            Some(_) => {}
        }
    }
    let mut out = Vec::new();
    walk(rows, c, 0, &mut out);
    out.sort_by_key(|x| rows.children.iter().position(|y| y.name.text == x.name.text));
    out
}

/// The rows of a condition over the columns it reads: the columns' indexes, and the rows over them.
pub fn rows_of(inputs: &Inputs, cond: &Cond) -> (Vec<usize>, Vec<(Vec<words::Cell>, bool)>) {
    let all = inputs.cols();
    let mut used = Vec::new();
    cols_of(cond, &mut used);
    used.sort();
    let sub: Vec<words::Col> = used.iter().map(|i| all[*i].clone()).collect();
    let remapped = remap(cond, &used);
    (used, words::rows(&sub, &remapped))
}

fn cols_of(c: &Cond, out: &mut Vec<usize>) {
    use words::Atom;
    match c {
        Cond::Atom(Atom::True(i)) | Cond::Atom(Atom::Absent(i)) | Cond::Atom(Atom::In(i, _)) | Cond::Atom(Atom::Cmp(i, ..)) => {
            if !out.contains(i) {
                out.push(*i);
            }
        }
        Cond::And(a, b) | Cond::Or(a, b) => {
            cols_of(a, out);
            cols_of(b, out);
        }
        Cond::Not(a) => cols_of(a, out),
        Cond::Const(_) => {}
    }
}

fn remap(c: &Cond, used: &[usize]) -> Cond {
    use words::Atom;
    let at = |i: &usize| used.iter().position(|u| u == i).unwrap_or(0);
    match c {
        Cond::Atom(Atom::True(i)) => Cond::Atom(Atom::True(at(i))),
        Cond::Atom(Atom::Absent(i)) => Cond::Atom(Atom::Absent(at(i))),
        Cond::Atom(Atom::In(i, v)) => Cond::Atom(Atom::In(at(i), v.clone())),
        Cond::Atom(Atom::Cmp(i, op, v)) => Cond::Atom(Atom::Cmp(at(i), *op, *v)),
        Cond::And(a, b) => Cond::And(Box::new(remap(a, used)), Box::new(remap(b, used))),
        Cond::Or(a, b) => Cond::Or(Box::new(remap(a, used)), Box::new(remap(b, used))),
        Cond::Not(a) => Cond::Not(Box::new(remap(a, used))),
        Cond::Const(b) => Cond::Const(*b),
    }
}

/// `form.cal`: each date the form computes by counting business days, `<date> + <n> business
/// days`, with the date and the number it reads as the inputs. koyomi's dates file takes one date
/// input, so a form whose computations read two dates is lowered for the first only.
fn form_dates(cx: &Ctx) -> Option<LFile> {
    let s = cx.s;
    let mut names = Names::koyomi();
    let mut dates: Vec<(String, &Field, String, Option<String>)> = Vec::new();
    let mut date_input: Option<&Field> = None;
    let mut ints: Vec<&Field> = Vec::new();
    for f in s.file.form.iter().filter(|f| f.computed.is_some() && matches!(f.ty, TypeExpr::Date)) {
        let e = f.computed.as_ref().unwrap();
        let ExprKind::Bin(op @ (BinOp::Add | BinOp::Sub), a, b) = &e.kind else { continue };
        let (ExprKind::Path(dp), ExprKind::BusinessDays(n, cal)) = (&a.kind, &b.kind) else { continue };
        let Some(d) = s.field(&dp[0].text) else { continue };
        if date_input.is_some_and(|x| x.name.text != d.name.text) {
            continue;
        }
        date_input = Some(d);
        let count = match &n.kind {
            ExprKind::Num(v) if v.value.is_int() => v.value.num.to_string(),
            ExprKind::Path(np) if np.len() == 1 => {
                let Some(x) = s.field(&np[0].text) else { continue };
                if !ints.iter().any(|i| i.name.text == x.name.text) {
                    ints.push(x);
                }
                np[0].text.clone()
            }
            _ => continue,
        };
        dates.push((count, f, op.sign().to_string(), cal.as_ref().map(|c| c.text.clone())));
    }
    let d = date_input?;
    let cal = cx.calendar(dates.iter().find_map(|x| x.3.as_deref()))?;
    let none_w = Text::default();
    let mut out = Out::default();
    out.put(format!("dates {} v1", words::koyomi_file(&format!("{}_form", cx.name()))), 0, &none_w);
    out.put(format!("description \"What ringi writes from the form of {}: the dates it counts in business days\"", cx.shown()), 0, &none_w);
    out.put(format!("use calendar \"{}\"", cal.path), 0, &none_w);
    out.blank();
    out.put("inputs", 0, &none_w);
    let dn = names.take(d.name.ascii());
    let dd = &d.name.text;
    // the days the date is counted from: the field's range, up to the last day the calendar's data
    // lets it be counted from ([`crate::usable`])
    let range = match cx.form_window {
        Some((lo, hi)) => format!(">={} <={}", crate::lex::date_text(lo), crate::lex::date_text(hi)),
        None => words::range_text(d),
    };
    let range = if range.is_empty() { String::new() } else { format!("  range {range}") };
    out.put(format!("  {dn} : date{range}"), d.line, &what(format!("フィールド `{dd}`"), format!("the field `{dd}`")));
    let mut int_names: Vec<(String, String)> = Vec::new();
    for i in &ints {
        let n = names.take(i.name.ascii());
        let id = &i.name.text;
        let range = words::range_text(i);
        let range = if range.is_empty() { String::new() } else { format!("  range {range}") };
        out.put(format!("  {n} : int{range}"), i.line, &what(format!("フィールド `{id}`"), format!("the field `{id}`")));
        int_names.push((i.name.text.clone(), n));
    }
    for (count, f, sign, _) in &dates {
        let fd = &f.name.text;
        let w = what(format!("フィールド `{fd}` の計算"), format!("the computation of the field `{fd}`"));
        let n = names.take(f.name.ascii());
        let count = int_names.iter().find(|(x, _)| x == count).map(|(_, n)| n.clone()).unwrap_or_else(|| count.clone());
        out.blank();
        out.put(format!("date {n} = {dn}"), f.line, &w);
        let unit = if count == "1" { "business day" } else { "business days" };
        out.put(format!("  {sign} {count} {unit}"), f.line, &w);
    }
    Some(LFile { name: "form.cal".into(), tool: LoweredTool::Koyomi, part: Part::FormDates, text: out.text, lines: out.lines, renamed: names.renamed(), given: Vec::new(), reads: Vec::new() })
}

/// The range a computed field writes, as the end of its `define` line: the range the lowered rule
/// accepts the value in (rulec's `define … range`). Empty when the field writes none.
fn accepted(f: &Field) -> String {
    let r = words::range_text(f);
    if f.range.is_none() || r.is_empty() { String::new() } else { format!("  range {r}") }
}

/// A computation of the question of E105, as a `define` line and an output.
pub struct Define {
    pub name: String,
    pub ty: String,
    pub text: String,
    pub round: String,
    pub line: usize,
    pub what: Text,
}

/// The question of E105 for one field: the columns its two conditions read, the computations
/// they read, and the condition of asking for the field and hiding it.
pub struct Question<'x, 'c, 'a> {
    pub inputs: Inputs<'x, 'c, 'a>,
    pub defines: Vec<Define>,
    pub cond: Cond,
}

/// The question of E105 for the field `f`. A request it names must be one that can be made. A
/// computed field the conditions read is asked about as a value of its own, between what its
/// computation reaches and the range it writes, unless what the computation reads in the end is
/// read by the conditions too, directly or through another computation (`fee = amount * 3%` beside
/// `amount`): then each computation other than a total is a `define` over what it reads, so that
/// its value follows from theirs, and the question asks for it in the range it writes. A total is
/// read as a value of its range either way: rulec holds a total's range at the door, and a total of
/// rows comes to the values between its ends.
pub fn question<'x, 'c, 'a>(cx: &'x Ctx<'c, 'a>, f: &Field) -> Option<Question<'x, 'c, 'a>> {
    use super::words::Atom;
    use crate::ast::CmpOp;
    let s = cx.s;
    let total = |g: &Field| matches!(g.computed.as_ref().map(|e| &e.kind), Some(ExprKind::Sum(_)));
    let conds: Vec<&Expr> = [f.required_when.as_ref(), f.shown_when.as_ref()].into_iter().flatten().collect();
    // the fields a computation reads in the end: the ones a person fills, and totals
    fn leaves(s: &crate::names::Scope, e: &Expr, total: &dyn Fn(&Field) -> bool, depth: usize, out: &mut Vec<String>) {
        let mut all = Vec::new();
        e.walk(&mut all);
        for x in all {
            if let ExprKind::Path(p) = &x.kind
                && p.len() == 1
                && let Some(g) = s.field(&p[0].text)
            {
                match &g.computed {
                    Some(c) if computed_number(g) && !total(g) && depth < 32 => leaves(s, c, total, depth + 1, out),
                    _ => {
                        if !out.contains(&g.name.text) {
                            out.push(g.name.text.clone());
                        }
                    }
                }
            }
        }
    }
    let mut read: Vec<&Field> = Vec::new();
    for e in &conds {
        let mut all = Vec::new();
        e.walk(&mut all);
        for x in all {
            if let ExprKind::Path(p) = &x.kind
                && p.len() == 1
                && let Some(g) = s.field(&p[0].text)
                && !read.iter().any(|r| r.name.text == g.name.text)
            {
                read.push(g);
            }
        }
    }
    let mut seen: Vec<String> = Vec::new();
    let mut tied = false;
    for g in &read {
        let mut ls = Vec::new();
        match &g.computed {
            Some(c) if computed_number(g) && !total(g) => leaves(s, c, &total, 0, &mut ls),
            _ => ls.push(g.name.text.clone()),
        }
        tied |= ls.iter().any(|l| seen.contains(l));
        seen.extend(ls);
    }
    // the computations other than totals the conditions read, and the ones those read, when the
    // conditions read what they follow from
    let mut needed: Vec<String> = Vec::new();
    let mut todo: Vec<&Expr> = if tied { conds.clone() } else { Vec::new() };
    while let Some(e) = todo.pop() {
        let mut all = Vec::new();
        e.walk(&mut all);
        for x in all {
            if let ExprKind::Path(p) = &x.kind
                && p.len() == 1
                && let Some(g) = s.field(&p[0].text)
                && computed_number(g)
                && !total(g)
                && !needed.contains(&g.name.text)
            {
                needed.push(g.name.text.clone());
                todo.push(g.computed.as_ref().unwrap());
            }
        }
    }
    let defs: Vec<&Field> = s.file.form.iter().filter(|g| needed.contains(&g.name.text)).collect();
    let mut inputs = Inputs::new(cx);
    // the computations it works out read a field that may be empty as its value and whether it is
    // filled in (DESIGN 4.9)
    inputs.computes = !defs.is_empty();
    // the question's output, which no input or computation is named
    inputs.names.keep("hidden");
    // what they read that is none of them: inputs, declared first, so that a `define` names each as
    // the rule declares it
    for g in &defs {
        let mut all = Vec::new();
        g.computed.as_ref().unwrap().walk(&mut all);
        for x in all {
            if let ExprKind::Path(p) = &x.kind
                && p.len() == 1
                && let Some(y) = s.field(&p[0].text)
                && !needed.contains(&y.name.text)
            {
                inputs.field(y, false);
            }
        }
    }
    let mut defines = Vec::new();
    let mut ranged: Vec<(usize, &Field)> = Vec::new();
    for g in &defs {
        let gd = &g.name.text;
        let name = inputs.names.take(g.name.ascii());
        let text = {
            let name_of = |n: &str| -> Option<(String, Option<TypeExpr>)> {
                let x = s.field(n)?;
                let named = inputs.list.iter().find(|i| i.reads == vec![x.name.text.clone()]).map(|i| i.name.clone());
                Some((named.unwrap_or_else(|| x.name.ascii().to_string()), Some(x.ty.clone())))
            };
            expr_text(g.computed.as_ref().unwrap(), &name_of)?
        };
        defines.push(Define { name: name.clone(), ty: super::route::field_type(g), text: rounded(g, text), round: output_round(g), line: g.line, what: what(format!("フィールド `{gd}` の計算"), format!("the computation of the field `{gd}`")) });
        inputs.derived(g, &name);
        // the column takes every value the computation comes to, as rulec reads the `define`: the
        // range written is a part of the question (below), not of the column
        if let (Some((rlo, rhi)), Some(last)) = (crate::claims::reach(s, g), inputs.list.last_mut())
            && let words::ColKind::Ordered { lo, hi, .. } = &mut last.col.kind
        {
            *lo = Some(rlo);
            *hi = Some(rhi);
        }
        if g.range.is_some() {
            ranged.push((inputs.list.len() - 1, g));
        }
    }
    let req = inputs.cond(f.required_when.as_ref()?, &[], f.line).ok()?;
    let shown = inputs.cond(f.shown_when.as_ref()?, &[], f.line).ok()?;
    let mut cond = Cond::And(Box::new(req), Box::new(Cond::Not(Box::new(shown))));
    // a request whose computation leaves the range it writes is refused at the door; one that has
    // no value is held to no range (DESIGN 4.9)
    for (i, g) in ranged {
        let (lo, hi) = words::ends(g);
        let mut inside = Cond::Const(true);
        for (op, end) in [(CmpOp::Ge, lo), (CmpOp::Le, hi)] {
            if let Some(v) = end {
                inside = Cond::And(Box::new(inside), Box::new(Cond::Atom(Atom::Cmp(i, op, v))));
            }
        }
        let held = match inputs.presence(Some(g)) {
            Some(p) => Cond::Or(Box::new(Cond::Not(Box::new(p))), Box::new(inside)),
            None => inside,
        };
        cond = Cond::And(Box::new(cond), Box::new(held));
    }
    Some(Question { inputs, defines, cond })
}

/// For each field asked for on a condition and shown on another, the question whether some
/// request asks for it and hides it (E105): a rule with one output, `hidden`, true where the
/// first holds and the second does not ([`question`]).
pub fn hidden(cx: &Ctx) -> Vec<LFile> {
    let s = cx.s;
    let mut out = Vec::new();
    for f in s.file.form.iter().filter(|f| f.shown_when.is_some() && f.required_when.is_some()) {
        let Some(Question { inputs, defines, cond }) = question(cx, f) else { continue };
        let (used, rows) = rows_of(&inputs, &cond);
        let fd = &f.name.text;
        let w = what(format!("フィールド `{fd}` の必須と表示の条件"), format!("the conditions the field `{fd}` is asked for and shown on"));
        let none_w = Text::default();
        let mut o = Out::default();
        o.put(format!("rule {}_{}_hidden v1", cx.name(), f.name.ascii()), 0, &none_w);
        o.blank();
        for e in inputs.imports() {
            o.put(format!("import enum {e} from \"choices.rule\""), 0, &none_w);
        }
        inputs.write(&mut o);
        o.blank();
        for d in &defines {
            o.put(format!("define {} : {} = {}", d.name, d.ty, d.text), d.line, &d.what);
            o.blank();
        }
        o.put("outputs", 0, &none_w);
        o.put("  hidden : bool", f.line, &w);
        // the computations, so that what they come to at the example can be said
        for d in &defines {
            o.put(format!("  {} : {}{}", d.name, d.ty, d.round), d.line, &d.what);
        }
        o.blank();
        o.put("table hidden", f.line, &w);
        let cols = inputs.cols();
        let heads: Vec<String> = used.iter().map(|i| cols[*i].name.clone()).collect();
        let sub: Vec<&words::Col> = used.iter().map(|i| &cols[*i]).collect();
        let texts: Vec<(Vec<String>, usize, Text)> = rows
            .iter()
            .map(|(cells, holds)| {
                let mut t: Vec<String> = cells.iter().zip(&sub).map(|(c, col)| words::cell_text(col, c)).collect();
                t.push(holds.to_string());
                (t, f.line, w.clone())
            })
            .collect();
        o.table(&heads, &["hidden".to_string()], (f.line, &w), &texts);
        out.push(LFile { name: format!("{}.hidden.rule", f.name.ascii()), tool: LoweredTool::Rulec, part: Part::Hidden(f.name.text.clone()), text: o.text, lines: o.lines, renamed: inputs.names.renamed(), given: inputs.given_columns(), reads: Vec::new() });
    }
    out
}
