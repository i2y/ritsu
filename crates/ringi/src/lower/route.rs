//! The route lowered to `route.rule` (DESIGN 6.4): one output for each stage, whose value is the way
//! to choose an approver the stage names there (an enum `<stage>_by`; `none` where the stage is not
//! there, and the output is optional when it can be not there). A route written as a table keeps its
//! rows, with `none` in each empty cell; a stage on a line of its own is a table of its own, whose
//! rows are the side of its `when` where it holds and the side where it does not; a shared route
//! is lowered with what the request gives it. A rule's output a stage or a condition reads is a
//! value the route works out itself: the rule is brought in with `apply`, its inputs bound to the
//! fields of the form it reads; a rule the route cannot apply so is read as an input, with where it
//! comes from (`fed by`). The route's `possible` claims and its examples are lowered with it.
//!
//! Each named table is lowered to a rule of its own (`<name>.rule`).

use super::lines::Out;
use super::words::{self, Atom, Cell, Col, ColKind, Cond, Names, Val};
use super::{Ctx, LFile, Part, what};
use crate::ast::{ClaimBody, CmpOp, Expr, ExprKind, Field, File, NamedTable, Route, RouteTable, Span, Spec, StageLine, Test, TypeExpr, UseKind, Word};
use ritsu_base::text::Text;
use ritsu_ports::{ColumnType, LoweredTool};

/// An input a lowered rule declares.
#[derive(Clone, Debug)]
pub struct Input {
    pub name: String,
    pub ty: String,
    pub range: String,
    /// `fed by rulec "…" output …`, for a rule's output.
    pub fed: Option<String>,
    /// The enum of `choices.rule` it is typed with.
    pub import: Option<String>,
    pub from: usize,
    pub what: Text,
    pub col: Col,
    /// What it reads in the request: a field, or a rule's output (`authority.board`).
    pub reads: Vec<String>,
    /// A value the rule computes itself (a total, a `define`), which it does not take as an input.
    pub derived: bool,
}

/// A rule the route brings in with `apply` (DESIGN 6.4): its alias as written, the name the file
/// gives the apply, the path and the pin of its meaning, each input of the rule with the column it
/// is bound to, and each output of the rule the route reads with the name the file gives it (an
/// output no table reads is not bound, as the code of the rule would hold a value nothing reads),
/// as the route first reads it; the rule's outputs, in the order it declares them.
#[derive(Clone, Debug)]
pub struct Applied {
    pub alias: String,
    pub name: String,
    pub path: String,
    pub meaning: String,
    pub binds: Vec<(String, usize)>,
    pub outs: Vec<(String, String)>,
    pub declared: Vec<String>,
    pub from: usize,
    pub what: Text,
}

/// A part of a condition the route's table cannot hold (E309, DESIGN 5.3), and where it is written.
#[derive(Clone, Debug)]
pub struct Unread {
    pub why: Unreadable,
    pub span: Span,
    /// Whether it is written in the request; else in the shared route the request uses (said at the
    /// request's `route` line).
    pub here: bool,
    /// Whether it is a condition of the form (`shown when`, `required when`), not of the route.
    pub form: bool,
}

/// What a condition reads that the route's table cannot hold.
#[derive(Clone, Debug, PartialEq)]
pub enum Unreadable {
    /// `requester`, `requester.department`, `requester.id`: who the requester is.
    Requester,
    /// `ticket.id`, `ticket.round`.
    Ticket,
    /// A field whose values the table cannot hold (text, a person, a choice from a master, an
    /// attachment), or a computed field the form does not compute (`computed`: one that is not a
    /// number or a date). Its name, and its type as written.
    Field { name: String, ty: String, computed: bool },
    /// Two values compared with each other: two fields, or a field and a rule's output. With the
    /// field to compute their difference in and the condition to write, for two numbers of a kind.
    Two { a: String, b: String, rewrite: Option<(String, String)> },
    /// A value computed in the condition (`limit - amount`, `sum(items.price)`), with the field to
    /// compute it in and the condition to write, for a number.
    Computed { rewrite: Option<(String, String)> },
    /// Anything else: a rule's output compared with `none`, a value of a rule's choice in quotes.
    Other,
}

/// An input of a shared route, as the request gives it.
#[derive(Clone, Debug)]
pub struct Given {
    pub name: String,
    /// What the request gives (or the input's own computation, written in the shared route); None
    /// for an optional input it does not give, which is `none` (a `bool`, false).
    pub value: Option<Expr>,
    /// Whether the value is written in the request.
    pub here: bool,
}

/// A name of a condition, with the shared route's inputs given.
enum Sub<'g> {
    /// No input: the name as written.
    Kept,
    /// What the input is, whether that is written in the request, and the inputs it reads (a
    /// computation of the shared route reads the others).
    Value(Expr, bool, &'g [Given]),
    /// An optional input not given: no value (DESIGN 4.9).
    Absent,
}

fn substitute<'g>(e: &Expr, args: &'g [Given]) -> Sub<'g> {
    let ExprKind::Path(p) = &e.kind else { return Sub::Kept };
    if p.len() != 1 {
        return Sub::Kept;
    }
    match args.iter().find(|g| g.name == p[0].text) {
        None => Sub::Kept,
        Some(g) => match &g.value {
            Some(v) => Sub::Value(v.clone(), g.here, args),
            None => Sub::Absent,
        },
    }
}

/// `requester…` and `ticket…`, which say who makes the request and which ticket it is.
fn about_who(p: &[Word]) -> Option<Unreadable> {
    match p[0].text.as_str() {
        "requester" => Some(Unreadable::Requester),
        "ticket" => Some(Unreadable::Ticket),
        _ => None,
    }
}

/// A value written as it is: a number, a date, a text, yes or no, `none`.
fn is_value(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Num(_) | ExprKind::Date(_) | ExprKind::Str(_) | ExprKind::Bool(_) | ExprKind::None => true,
        ExprKind::Neg(x) => is_value(x),
        _ => false,
    }
}

/// An optional input of the shared route that the request does not give, compared with `other`:
/// not there, it has no value (DESIGN 4.9). `is none` and `is not` a value hold; `is` a value and a
/// comparison do not.
fn absent(op: CmpOp, other: &Expr) -> Cond {
    if matches!(other.kind, ExprKind::None) {
        return Cond::Const(op == CmpOp::Is);
    }
    Cond::Const(op == CmpOp::IsNot)
}

/// The comparison read the other way round: `5 < amount` is `amount > 5`.
fn flipped(op: CmpOp) -> CmpOp {
    match op {
        CmpOp::Lt => CmpOp::Gt,
        CmpOp::Le => CmpOp::Ge,
        CmpOp::Gt => CmpOp::Lt,
        CmpOp::Ge => CmpOp::Le,
        o => o,
    }
}

fn dotted(p: &[Word]) -> String {
    p.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(".")
}

/// For two numbers of a kind compared, the field that holds their difference and the condition
/// that compares it with zero: `amount > limit` is `difference > 0JPY`.
fn difference(f: &Field, g: &Field, op: CmpOp) -> Option<(String, String)> {
    let (TypeExpr::Num(u), TypeExpr::Num(v)) = (&f.ty, &g.ty) else { return None };
    if u.dim != v.dim || u.compares_only() || v.compares_only() {
        return None;
    }
    let ty = crate::ast::unit_type(u);
    let zero = words::number(ritsu_units::Rat::int(0), u);
    Some((format!("difference : {ty} = {} - {}", f.name.text, g.name.text), format!("when difference {} {zero}", op.sign())))
}

/// Both sides of `and` and `or`, or every part of either that cannot be read.
fn both(a: Result<Cond, Vec<Unread>>, b: Result<Cond, Vec<Unread>>, join: fn(Box<Cond>, Box<Cond>) -> Cond) -> Result<Cond, Vec<Unread>> {
    match (a, b) {
        (Ok(x), Ok(y)) => Ok(join(Box::new(x), Box::new(y))),
        (Err(mut x), Err(y)) => {
            x.extend(y);
            Err(x)
        }
        (Err(x), _) | (_, Err(x)) => Err(x),
    }
}

/// The inputs of a lowered rule, as its conditions and cells come to read them.
pub struct Inputs<'x, 'c, 'a> {
    pub cx: &'x Ctx<'c, 'a>,
    pub list: Vec<Input>,
    pub names: Names,
    /// Whether what is lowered now is written in the request (else in the shared route it uses).
    pub here: bool,
    /// Whether the conditions lowered are the form's (`shown when`, `required when`).
    pub form: bool,
    /// Whether a computed field the rule computes itself is read over all its computation comes to,
    /// as the rule the generated code calls computes it (`values.rule`): else within its range.
    pub open: bool,
    /// Whether the rule works out the form's computations itself (`form.rule`, `values.rule`, the
    /// question of E105): a field that may be empty that a computation reads is then two inputs, its
    /// value and whether it is filled in (DESIGN 4.9).
    pub computes: bool,
    /// Whether a rule's output is worked out by the file, the rule brought in with `apply` (the
    /// route, DESIGN 6.4): else an input `fed by` the rule.
    pub apply: bool,
    /// The rules brought in with `apply`, in the order first read.
    pub applied: Vec<Applied>,
    /// The rules not brought in with `apply`, by their aliases, whose outputs are inputs `fed by`
    /// them: those the route reads some of the outputs of ([`route`]).
    pub apart: std::collections::BTreeSet<String>,
    depth: usize,
}

impl<'x, 'c, 'a> Inputs<'x, 'c, 'a> {
    pub fn new(cx: &'x Ctx<'c, 'a>) -> Self {
        Inputs {
            cx,
            list: Vec::new(),
            names: Names::rulec().avoiding(&words::declared(cx.s)),
            here: true,
            form: false,
            open: false,
            computes: false,
            apply: false,
            applied: Vec::new(),
            apart: std::collections::BTreeSet::new(),
            depth: 0,
        }
    }

    fn at(&self, reads: &[String]) -> Option<usize> {
        self.list.iter().position(|i| i.reads == reads)
    }

    /// The column that reads a field (`amount`), or a field's presence (`reviewer` given), or a
    /// rule's output; declared the first time.
    pub fn field(&mut self, f: &Field, given: bool) -> usize {
        let key = if given { vec![format!("{}?", f.name.text)] } else { vec![f.name.text.clone()] };
        if let Some(i) = self.at(&key) {
            return i;
        }
        let ascii = f.name.ascii();
        let fd = f.name.text.clone();
        let w = what(format!("フィールド `{fd}`"), format!("the field `{fd}`"));
        let opt = !given && self.opt(f);
        let (name, ty, range, import, kind) = if given {
            let n = self.names.take(&format!("{ascii}_given"));
            (n, "bool".to_string(), String::new(), None, ColKind::Bool)
        } else {
            let n = self.names.take(ascii);
            match &f.ty {
                TypeExpr::Bool => (n, "bool".into(), String::new(), None, ColKind::Bool),
                TypeExpr::Choice => {
                    let vs = words::lowered_values(self.cx.s, f).into_iter().map(|(_, v)| v).collect();
                    let e = super::enum_of(self.cx, f).unwrap_or_else(|| ascii.to_string());
                    (n, e.clone(), String::new(), Some(e), ColKind::Values(vs))
                }
                TypeExpr::ChoiceFrom(p) => {
                    let vs = self.cx.rule(&p[0].text).and_then(|r| r.facts.enums.iter().find(|e| e.name == p[1].text || e.public == p[1].text).map(|e| e.values.iter().map(|v| v.public.clone()).collect())).unwrap_or_default();
                    let e = super::enum_of(self.cx, f).unwrap_or_else(|| ascii.to_string());
                    (n, e.clone(), String::new(), Some(e), ColKind::Values(vs))
                }
                TypeExpr::Num(u) => {
                    let (lo, hi) = reach_or_range(self.cx.s, f);
                    let range = range_of(f, lo, hi);
                    (n, crate::ast::unit_type(u), range, None, ColKind::Ordered { lo, hi, step: words::step(u), ty: f.ty.clone() })
                }
                TypeExpr::Date => {
                    let (lo, hi) = words::ends(f);
                    (n, "date".into(), words::range_text(f), None, ColKind::Ordered { lo, hi, step: ritsu_units::Rat::int(1), ty: f.ty.clone() })
                }
                _ => (n, "string".into(), String::new(), None, ColKind::Values(Vec::new())),
            }
        };
        let ty = if opt { format!("{ty}?") } else { ty };
        self.list.push(Input { name: name.clone(), ty, range, fed: None, import, from: f.line, what: w, col: Col { name, kind, opt }, reads: key, derived: false });
        self.list.len() - 1
    }

    /// `f is none`: the column of its value holds `none` (a column that may have none); a field the
    /// rule reads as its value and whether it is filled in, or a computation it works out, is not
    /// filled in ([`Inputs::presence`]); else the column that reads whether it is filled in.
    fn absent_of(&mut self, f: &Field) -> Cond {
        if self.unreadable(f).is_none() {
            if let Some(p) = self.presence(Some(f)) {
                return Cond::Not(Box::new(p));
            }
            if f.computed.is_some() && self.at(std::slice::from_ref(&f.name.text)).is_some_and(|i| self.list[i].derived) {
                // a computation the rule works out that reads no field that may be empty
                return Cond::Const(false);
            }
            if self.opt(f) {
                let c = self.field(f, false);
                return Cond::Atom(Atom::Absent(c));
            }
        }
        let c = self.field(f, true);
        Cond::Not(Box::new(Cond::Atom(Atom::True(c))))
    }

    /// Whether the column of a field's value is one that may have no value (`T?`, DESIGN 4.9): a
    /// field that may be empty, unless the rule computes something of it (it is then its value and
    /// whether it is filled in: [`Inputs::paired`]); a computed field the rule takes as an input
    /// that reads one.
    fn opt(&self, f: &Field) -> bool {
        match &f.computed {
            None => super::may_be_empty(f) && !self.paired(f),
            Some(_) => super::may_be_none(self.cx.s, f),
        }
    }

    /// Whether the rule reads a field that may be empty as its value and whether it is filled in:
    /// one a computation the rule works out reads (DESIGN 4.9). Each comparison of its value holds
    /// only where it is filled in.
    pub fn paired(&self, f: &Field) -> bool {
        self.computes && super::computed_from(self.cx.s, f)
    }

    /// `c` read of the field `f`, where it has a value: as it is, or, for a field the rule reads as
    /// its value and whether it is filled in, with that too; for a computation the rule works out,
    /// with whether each field that may be empty it reads is filled in (DESIGN 4.9).
    fn filled(&mut self, f: Option<&Field>, c: Cond) -> Cond {
        match self.presence(f) {
            Some(p) => Cond::And(Box::new(p), Box::new(c)),
            None => c,
        }
    }

    /// Whether the field `f` has a value, where the rule reads that as whether fields are filled in
    /// (a field it reads as its value and whether it is filled in, a computation it works out that
    /// reads such fields); None where the column of its value says it, or it always has one.
    pub fn presence(&mut self, f: Option<&Field>) -> Option<Cond> {
        let f = f?;
        let cx = self.cx;
        let fields: Vec<&Field> = if f.computed.is_some() {
            let derived = self.at(std::slice::from_ref(&f.name.text)).is_some_and(|i| self.list[i].derived);
            if !derived {
                return None;
            }
            super::empty_reads(cx.s, f)
        } else if self.paired(f) {
            vec![f]
        } else {
            return None;
        };
        let mut out: Option<Cond> = None;
        for g in fields {
            let c = self.field(g, true);
            let atom = Cond::Atom(Atom::True(c));
            out = Some(match out {
                Some(x) => Cond::And(Box::new(x), Box::new(atom)),
                None => atom,
            });
        }
        out
    }

    /// A computed field the rule computes itself under the name `name`: a column its conditions
    /// read, read between the field's range (DESIGN 4.1), or when the rule is `open` over all its
    /// computation comes to, a total over any number of rows from none; and no input.
    pub fn derived(&mut self, f: &Field, name: &str) {
        let (lo, hi) = if self.open {
            crate::claims::reach_filling(self.cx.s, f).map_or((None, None), |(lo, hi)| (Some(lo), Some(hi)))
        } else {
            reach_or_range(self.cx.s, f)
        };
        let step = match &f.ty {
            TypeExpr::Num(u) => words::step(u),
            _ => ritsu_units::Rat::int(1),
        };
        let fd = f.name.text.clone();
        let w = what(format!("フィールド `{fd}`"), format!("the field `{fd}`"));
        // the value it comes to with what it reads, each filled in or not: a computation that reads
        // a field that may be empty has a value where they are all filled in ([`Inputs::filled`])
        let col = Col { name: name.to_string(), kind: ColKind::Ordered { lo, hi, step, ty: f.ty.clone() }, opt: false };
        self.list.push(Input { name: name.to_string(), ty: String::new(), range: String::new(), fed: None, import: None, from: f.line, what: w, col, reads: vec![f.name.text.clone()], derived: true });
    }

    /// The column that reads a rule's output (`authority.board`), declared the first time; None when
    /// the rule or its output is not known.
    pub fn output(&mut self, alias: &str, output: &str, line: usize) -> Option<usize> {
        let key = vec![alias.to_string(), output.to_string()];
        if let Some(i) = self.at(&key) {
            return Some(i);
        }
        let cx = self.cx;
        let r = cx.rule(alias)?;
        let o = r.facts.outputs.iter().find(|o| o.name == output || o.alias == output)?;
        // a rule the route applies: the output is a value the file works out from the fields the
        // rule reads, so that a table that reads it beside them is asked for no row the rule never
        // answers (DESIGN 6.4)
        let applied = if self.apply { self.applied_for(alias, line) } else { None };
        let alias_ascii = words::alias_ascii(cx.s, alias);
        let name = match applied {
            Some(a) => match self.applied[a].outs.iter().find(|(n, _)| *n == o.name) {
                Some((_, r)) => r.clone(),
                None => {
                    let n = self.names.take(&format!("{alias_ascii}_{}", o.alias));
                    self.applied[a].outs.push((o.name.clone(), n.clone()));
                    n
                }
            },
            None => self.names.take(&format!("{alias_ascii}_{}", o.alias)),
        };
        let fed = format!("fed by rulec \"{}\" output {}", r.path, o.name);
        let (ty, opt) = match &o.ty {
            ColumnType::Opt(inner) => (inner.as_ref(), "?"),
            t => (t, ""),
        };
        let (ty, range, import, kind) = match ty {
            ColumnType::Bool => ("bool".to_string(), String::new(), None, ColKind::Bool),
            ColumnType::Enum { name: en, .. } => {
                let vs: Vec<String> = r.facts.enums.iter().find(|e| &e.name == en).map(|e| e.values.iter().map(|v| v.public.clone()).collect()).unwrap_or_default();
                let e = super::form::output_enum(self.cx, alias, &o.alias).unwrap_or_else(|| format!("{alias_ascii}_{}", o.alias));
                (e.clone(), String::new(), Some(e), ColKind::Values(vs))
            }
            ColumnType::Num { written, unit, min, max, .. } => {
                let u = unit.clone().unwrap_or(ritsu_units::Unit { dim: ritsu_units::Dim::Number, unit: String::new(), tax: None, step: None });
                let st = words::step(&u);
                let lo = min.map(|m| ritsu_units::Rat::int(m).mul(st));
                let hi = max.map(|m| ritsu_units::Rat::int(m).mul(st));
                let mut parts = Vec::new();
                if let Some(l) = lo {
                    parts.push(format!(">={}", words::number(l, &u)));
                }
                if let Some(h) = hi {
                    parts.push(format!("<={}", words::number(h, &u)));
                }
                (written.clone(), parts.join(" "), None, ColKind::Ordered { lo, hi, step: st, ty: TypeExpr::Num(u) })
            }
            _ => ("string".to_string(), String::new(), None, ColKind::Values(Vec::new())),
        };
        let ty = format!("{ty}{opt}");
        let w = what(format!("規則の出力 `{alias}.{output}`"), format!("the rule's output `{alias}.{output}`"));
        let col = Col { name: name.clone(), kind, opt: !opt.is_empty() };
        match applied {
            // the value the apply brings in, of the rule's enum: no input
            Some(_) => self.list.push(Input { name, ty: String::new(), range: String::new(), fed: None, import: None, from: line, what: w, col, reads: key, derived: true }),
            None => self.list.push(Input { name, ty, range, fed: Some(fed), import, from: line, what: w, col, reads: key, derived: false }),
        }
        Some(self.list.len() - 1)
    }

    /// The rule `alias` brought in with `apply`, the first time it is read (DESIGN 6.4): each of its
    /// inputs bound to the column of the field of the form of the same name, and each of its outputs
    /// under a name of the file's. None where the route cannot apply it, and reads it as an input
    /// `fed by` it: a rule whose pin is not known, one of a module (`@…`), one that walks a list, one
    /// with an optional number among its outputs (a value the file cannot hold, rulec's E044), and
    /// one with an input no field binds ([`Inputs::binds`]).
    fn applied_for(&mut self, alias: &str, line: usize) -> Option<usize> {
        if let Some(a) = self.applied.iter().position(|a| a.alias == alias) {
            return Some(a);
        }
        if self.apart.contains(alias) {
            return None;
        }
        let cx = self.cx;
        let r = cx.rule(alias)?;
        let meaning = r.meaning.clone()?;
        if r.path.starts_with('@') || !r.facts.elements.is_empty() {
            return None;
        }
        // the examples of the route and the conditions of `only when` read an output as a column of
        // the inputs (an example row, a range rulec is asked over): a rule they read stays an input
        let names_it = |p: &[Word]| p.len() == 2 && p[0].text == alias;
        let in_examples = cx.s.file.examples.as_ref().is_some_and(|ex| ex.inputs.iter().any(|p| names_it(p)));
        let in_claims = cx.s.file.claims.iter().any(|c| matches!(&c.body, ClaimBody::OnlyWhen { when, .. } if when.paths().into_iter().any(names_it)));
        if in_examples || in_claims {
            return None;
        }
        if r.facts.outputs.iter().any(|o| matches!(&o.ty, ColumnType::Opt(t) if matches!(t.as_ref(), ColumnType::Num { .. }))) {
            return None;
        }
        // each input by the field the request gives it, by its ASCII name, as the reference and the
        // glue give it
        let mut fields = Vec::new();
        for i in &r.facts.inputs {
            let f = cx.s.fields().iter().find(|f| f.name.ascii() == i.name || f.name.ascii() == i.alias)?;
            if !self.binds(alias, f, &i.ty) {
                return None;
            }
            fields.push((i.name.clone(), f));
        }
        let binds = fields.into_iter().map(|(n, f)| (n, self.field(f, false))).collect();
        let name = self.names.take(&words::alias_ascii(cx.s, alias));
        let declared = r.facts.outputs.iter().map(|o| o.name.clone()).collect();
        let w = what(format!("規則 `{alias}`"), format!("the rule `{alias}`"));
        self.applied.push(Applied { alias: alias.to_string(), name, path: r.path.clone(), meaning, binds, outs: Vec::new(), declared, from: line, what: w });
        Some(self.applied.len() - 1)
    }

    /// Whether the route binds an input of the rule `alias` of the type `ty` to the field `f` (DESIGN
    /// 6.4): a field the table can hold (5.3), of the input's type, which has a value wherever the
    /// input takes none, and whose values the input's range takes (rulec's E042 and E043 say
    /// otherwise); a choice, one whose every value, as the route writes it, the rule's enum has
    /// (`choice from authority.kind`), which rulec maps by its spelling.
    fn binds(&self, alias: &str, f: &Field, ty: &ColumnType) -> bool {
        if self.unreadable(f).is_some() {
            return false;
        }
        let (ty, takes_none) = match ty {
            ColumnType::Opt(t) => (t.as_ref(), true),
            t => (t, false),
        };
        if !takes_none && self.opt(f) {
            return false;
        }
        match (ty, &f.ty) {
            (ColumnType::Bool, TypeExpr::Bool) => true,
            (ColumnType::Enum { name, range: None }, TypeExpr::Choice | TypeExpr::ChoiceFrom(_)) => {
                let Some(theirs) = self.cx.rule(alias).and_then(|r| r.facts.enums.iter().find(|e| e.name == *name)) else { return false };
                let ours: Vec<String> = match &f.ty {
                    TypeExpr::ChoiceFrom(p) => self.cx.rule(&p[0].text).and_then(|r| r.facts.enums.iter().find(|e| e.name == p[1].text || e.public == p[1].text)).map(|e| e.values.iter().map(|v| v.public.clone()).collect()).unwrap_or_default(),
                    _ => words::lowered_values(self.cx.s, f).into_iter().map(|(_, v)| v).collect(),
                };
                !ours.is_empty() && ours.iter().all(|v| theirs.values.iter().any(|t| t.public == *v))
            }
            (ColumnType::Num { written, min, max, .. }, TypeExpr::Num(u)) => {
                let (lo, hi) = reach_or_range(self.cx.s, f);
                let st = words::step(u);
                let end = |m: &Option<i128>| m.map(|m| ritsu_units::Rat::int(m).mul(st));
                let above = match (end(min), lo) {
                    (None, _) => true,
                    (Some(m), Some(l)) => m.cmp_to(l) != std::cmp::Ordering::Greater,
                    (Some(_), None) => false,
                };
                let below = match (end(max), hi) {
                    (None, _) => true,
                    (Some(m), Some(h)) => m.cmp_to(h) != std::cmp::Ordering::Less,
                    (Some(_), None) => false,
                };
                *written == crate::ast::unit_type(u) && above && below
            }
            _ => false,
        }
    }

    /// The columns that read whether a field is filled in (`cover is none`), each by its name with
    /// the field's ASCII name, in the order they were declared.
    pub fn given_columns(&self) -> Vec<(String, String)> {
        self.list
            .iter()
            .filter_map(|i| {
                let [one] = i.reads.as_slice() else { return None };
                let f = self.cx.s.field(one.strip_suffix('?')?)?;
                Some((i.name.clone(), f.name.ascii().to_string()))
            })
            .collect()
    }

    /// The columns, in the order they were declared.
    pub fn cols(&self) -> Vec<Col> {
        self.list.iter().map(|i| i.col.clone()).collect()
    }

    /// Whether the table can hold a field's values (DESIGN 5.3): a `choice` of its own values or of
    /// a rule's enum, a number, a date, yes or no; and, for a computed field, one the form computes
    /// (a number or a date). Else what it is, for E309.
    fn unreadable(&self, f: &Field) -> Option<Unreadable> {
        let listed = match &f.ty {
            TypeExpr::Bool | TypeExpr::Choice | TypeExpr::Num(_) | TypeExpr::Date => true,
            TypeExpr::ChoiceFrom(p) => p.len() == 2 && self.cx.rule(&p[0].text).is_some(),
            _ => false,
        };
        let computed = f.computed.is_none() || matches!(f.ty, TypeExpr::Num(_) | TypeExpr::Date);
        (!listed || !computed).then(|| Unreadable::Field { name: f.name.text.clone(), ty: f.ty.word(), computed: f.computed.is_some() && listed })
    }

    fn unread(&self, why: Unreadable, span: Span, here: bool) -> Vec<Unread> {
        vec![Unread { why, span, here, form: self.form }]
    }

    /// With `here` saying whether what is lowered is written in the request (else in the shared
    /// route it uses).
    fn within<T>(&mut self, here: bool, f: impl FnOnce(&mut Self) -> T) -> T {
        let was = std::mem::replace(&mut self.here, here);
        let out = f(self);
        self.here = was;
        out
    }

    /// A condition of the request (with a shared route's inputs as the request gives them, `args`)
    /// as the rule reads it; else each part of it the table cannot hold (E309).
    pub fn cond(&mut self, e: &Expr, args: &[Given], line: usize) -> Result<Cond, Vec<Unread>> {
        self.depth += 1;
        let out = if self.depth > 16 { Err(self.unread(Unreadable::Other, e.span, self.here)) } else { self.cond_in(e, args, line) };
        self.depth -= 1;
        out
    }

    fn cond_in(&mut self, e: &Expr, args: &[Given], line: usize) -> Result<Cond, Vec<Unread>> {
        let here = self.here;
        match &e.kind {
            ExprKind::Bool(b) => Ok(Cond::Const(*b)),
            ExprKind::And(a, b) => both(self.cond(a, args, line), self.cond(b, args, line), Cond::And),
            ExprKind::Or(a, b) => both(self.cond(a, args, line), self.cond(b, args, line), Cond::Or),
            ExprKind::Not(a) => Ok(Cond::Not(Box::new(self.cond(a, args, line)?))),
            ExprKind::Path(p) => {
                match substitute(e, args) {
                    Sub::Value(v, h, inner) => return self.within(h, |me| me.cond(&v, if h { &[] } else { inner }, line)),
                    // an optional input the request does not give has no value: as a condition, it
                    // does not hold (DESIGN 4.9)
                    Sub::Absent => return Ok(Cond::Const(false)),
                    Sub::Kept => {}
                }
                let cx = self.cx;
                if p.len() == 1
                    && let Some(f) = cx.s.field(&p[0].text)
                {
                    if let Some(why) = self.unreadable(f) {
                        return Err(self.unread(why, e.span, here));
                    }
                    let c = self.field(f, false);
                    return Ok(self.filled(Some(f), Cond::Atom(Atom::True(c))));
                }
                if let Some(why) = about_who(p) {
                    return Err(self.unread(why, e.span, here));
                }
                match (p.len() == 2).then(|| self.output(&p[0].text, &p[1].text, line)).flatten() {
                    Some(c) => Ok(Cond::Atom(Atom::True(c))),
                    None => Err(self.unread(Unreadable::Other, e.span, here)),
                }
            }
            ExprKind::Cmp(op, a, b) => self.compare(*op, (a, here), (b, here), e.span, args, line),
            _ => Err(self.unread(Unreadable::Other, e.span, here)),
        }
    }

    /// `a <op> b`, each side with whether it is written in the request; `at` is where the whole is
    /// written.
    fn compare(&mut self, op: CmpOp, a: (&Expr, bool), b: (&Expr, bool), at: Span, args: &[Given], line: usize) -> Result<Cond, Vec<Unread>> {
        let here = self.here;
        // the inputs of a shared route, as the request gives them
        let (a, ah) = match substitute(a.0, args) {
            Sub::Kept => (a.0.clone(), a.1),
            Sub::Value(v, h, _) => (v, h),
            Sub::Absent => return Ok(absent(op, b.0)),
        };
        let (b, bh) = match substitute(b.0, args) {
            Sub::Kept => (b.0.clone(), b.1),
            Sub::Value(v, h, _) => (v, h),
            Sub::Absent => return Ok(absent(op, &a)),
        };
        // a condition the request gives a `bool` input, compared with yes or no
        for ((x, xh), (y, _)) in [((&a, ah), (&b, bh)), ((&b, bh), (&a, ah))] {
            if let ExprKind::Bool(t) = y.kind
                && matches!(x.kind, ExprKind::Cmp(..) | ExprKind::And(..) | ExprKind::Or(..) | ExprKind::Not(_))
            {
                let c = self.within(xh, |me| me.cond(x, &[], line))?;
                return Ok(if (op == CmpOp::Is) == t { c } else { Cond::Not(Box::new(c)) });
            }
        }
        // a value on the left and a name on the right are read the other way round
        if !matches!(a.kind, ExprKind::Path(_)) && matches!(b.kind, ExprKind::Path(_)) && is_value(&a) {
            return self.compare(flipped(op), (&b, bh), (&a, ah), at, &[], line);
        }
        let ExprKind::Path(p) = &a.kind else {
            if matches!(a.kind, ExprKind::None) && matches!(b.kind, ExprKind::None) {
                return Ok(Cond::Const(op == CmpOp::Is));
            }
            return Err(self.operand(&a, ah, Some((op, &b, bh))));
        };
        // the column on the left: a field, or a rule's output
        let cx = self.cx;
        let (c, left) = match (p.len(), cx.s.field(&p[0].text)) {
            (1, Some(f)) => {
                // whether it is filled in: of a field of any kind; in the column of its value where
                // that may have none (DESIGN 4.9)
                if matches!(b.kind, ExprKind::None) {
                    let absent = self.absent_of(f);
                    return Ok(if op == CmpOp::Is { absent } else { Cond::Not(Box::new(absent)) });
                }
                if let Some(why) = self.unreadable(f) {
                    return Err(self.unread(why, a.span, ah));
                }
                (self.field(f, false), Some(f))
            }
            _ => {
                if let Some(why) = about_who(p) {
                    return Err(self.unread(why, a.span, ah));
                }
                let c = (p.len() == 2).then(|| self.output(&p[0].text, &p[1].text, line)).flatten();
                match c {
                    // whether a rule's output that may be none is there; an output that always has
                    // a value has no cell for it
                    Some(c) if matches!(b.kind, ExprKind::None) && self.list[c].col.opt => {
                        let absent = Cond::Atom(Atom::Absent(c));
                        return Ok(if op == CmpOp::Is { absent } else { Cond::Not(Box::new(absent)) });
                    }
                    Some(_) if matches!(b.kind, ExprKind::None) => return Err(self.unread(Unreadable::Other, at, here)),
                    Some(c) => (c, None),
                    None => return Err(self.unread(Unreadable::Other, a.span, ah)),
                }
            }
        };
        let kind = self.list[c].col.kind.clone();
        let opt = self.list[c].col.opt;
        // `is` holds where the value is there and is it; `is not` everywhere else, where there is
        // no value too; a comparison only where the value is there (DESIGN 4.9)
        let is = |me: &mut Self, atom: Cond| {
            let atom = me.filled(left, atom);
            if op == CmpOp::IsNot { Cond::Not(Box::new(atom)) } else { atom }
        };
        match &b.kind {
            ExprKind::Bool(t) if matches!(kind, ColKind::Bool) => {
                let atom = Cond::Atom(Atom::True(c));
                // `is false` holds of a value that is there and false
                let atom = match (*t, opt) {
                    (true, _) => atom,
                    (false, false) => Cond::Not(Box::new(atom)),
                    (false, true) => Cond::And(Box::new(Cond::Not(Box::new(atom))), Box::new(Cond::Not(Box::new(Cond::Atom(Atom::Absent(c)))))),
                };
                Ok(is(self, atom))
            }
            ExprKind::Path(v) => {
                // another field, or a rule's output: two values compared
                let other_field = (v.len() == 1).then(|| self.cx.s.field(&v[0].text)).flatten();
                if other_field.is_some() || (v.len() == 2 && self.cx.rule(&v[0].text).is_some()) {
                    let rewrite = match (left, other_field) {
                        (Some(f), Some(g)) if ah && bh => difference(f, g, op),
                        _ => None,
                    };
                    return Err(self.unread(Unreadable::Two { a: dotted(p), b: dotted(v), rewrite }, at, here));
                }
                if let Some(why) = about_who(v) {
                    return Err(self.unread(why, b.span, bh));
                }
                // a value of a choice
                match (&kind, v.len(), &op) {
                    (ColKind::Values(_), 1, CmpOp::Is | CmpOp::IsNot) => {
                        let value = match left {
                            Some(f) if matches!(f.ty, TypeExpr::Choice) => words::choice_value(self.cx.s, f, &v[0].text),
                            _ => v[0].text.clone(),
                        };
                        Ok(is(self, Cond::Atom(Atom::In(c, vec![value]))))
                    }
                    _ => Err(self.unread(Unreadable::Other, at, here)),
                }
            }
            _ => {
                let v = match &kind {
                    ColKind::Ordered { ty, .. } => words::constant_in(&b, ty),
                    _ => None,
                };
                let Some(v) = v else {
                    return Err(match &b.kind {
                        ExprKind::Bin(..) | ExprKind::Sum(_) | ExprKind::Lookup(..) | ExprKind::BusinessDays(..) | ExprKind::Neg(_) => self.operand(&b, bh, Some((flipped(op), &a, ah))),
                        _ => self.unread(Unreadable::Other, at, here),
                    });
                };
                let v = v.rat();
                Ok(match op {
                    CmpOp::Is | CmpOp::IsNot => is(self, Cond::And(Box::new(Cond::Atom(Atom::Cmp(c, CmpOp::Ge, v))), Box::new(Cond::Atom(Atom::Cmp(c, CmpOp::Le, v))))),
                    op => self.filled(left, Cond::Atom(Atom::Cmp(c, op, v))),
                })
            }
        }
    }

    /// A side of a comparison that is no name: a value computed in the condition, with how to
    /// compute it in a field instead when it is a number written in the request (`op` and `other`,
    /// the rest of the comparison, as the field would be compared).
    fn operand(&self, e: &Expr, here: bool, rest: Option<(CmpOp, &Expr, bool)>) -> Vec<Unread> {
        if !matches!(e.kind, ExprKind::Bin(..) | ExprKind::Sum(_) | ExprKind::Lookup(..) | ExprKind::BusinessDays(..) | ExprKind::Neg(_)) {
            return self.unread(Unreadable::Other, e.span, here);
        }
        let s = self.cx.s;
        let rewrite = (|| {
            let (op, other, oh) = rest?;
            if !here || !oh {
                return None;
            }
            let mut scratch = Vec::new();
            let ty = match crate::types::Typer::new(s, &mut scratch).expr(e) {
                crate::types::Ty::Num(u) if !u.compares_only() => crate::ast::unit_type(&u),
                _ => return None,
            };
            let (w, o) = (crate::types::written(s.src, e.span), crate::types::written(s.src, other.span));
            Some((format!("value : {ty} = {w}"), format!("when value {} {o}", op.sign())))
        })();
        self.unread(Unreadable::Computed { rewrite }, e.span, here)
    }

    /// The input lines, under `inputs`.
    pub fn write(&self, out: &mut Out) {
        let taken: Vec<&Input> = self.list.iter().filter(|i| !i.derived).collect();
        if taken.is_empty() && self.applied.is_empty() {
            return;
        }
        if taken.is_empty() {
            self.write_applied(out);
            return;
        }
        out.put("inputs", 0, &Text::default());
        let w = taken.iter().map(|i| i.name.chars().count()).max().unwrap_or(0);
        for i in taken {
            let mut line = format!("  {}{} : {}", i.name, " ".repeat(w - i.name.chars().count()), i.ty);
            if !i.range.is_empty() {
                line.push_str(&format!("  range {}", i.range));
            }
            if let Some(f) = &i.fed {
                line.push_str(&format!("  {f}"));
            }
            out.put(line, i.from, &i.what);
        }
        self.write_applied(out);
    }

    /// The `apply` of each rule the file brings in: the rule and the pin of its meaning, each of its
    /// inputs bound to its column, each of its outputs the route reads under the file's name, in the
    /// order the rule declares them (DESIGN 6.4).
    fn write_applied(&self, out: &mut Out) {
        for a in &self.applied {
            out.blank();
            out.put(format!("apply {} = \"{}\" {}", a.name, a.path, a.meaning), a.from, &a.what);
            for (input, c) in &a.binds {
                out.put(format!("  {input} = {}", self.list[*c].name), a.from, &a.what);
            }
            for output in &a.declared {
                if let Some((_, name)) = a.outs.iter().find(|(o, _)| o == output) {
                    out.put(format!("  {output} -> {name}"), a.from, &a.what);
                }
            }
        }
    }

    /// The `import enum` lines for the enums of `choices.rule` the inputs are typed with.
    pub fn imports(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for e in self.list.iter().filter_map(|i| i.import.as_ref()) {
            if !out.contains(e) {
                out.push(e.clone());
            }
        }
        out
    }
}

/// The ends a numeric field is read with (DESIGN 4.1): a field a person fills, its range as
/// written; a computed field, what its computation can reach, within the range it writes (a part of
/// the range the computation does not reach never comes, and a request outside the range is
/// refused).
pub fn reach_or_range(s: &crate::names::Scope, f: &Field) -> (Option<ritsu_units::Rat>, Option<ritsu_units::Rat>) {
    let reached = if f.computed.is_some() { crate::claims::reach(s, f) } else { None };
    match (f.range.is_some(), reached) {
        (true, Some(v)) => match crate::claims::within(f, v) {
            Some((lo, hi)) => (Some(lo), Some(hi)),
            None => words::ends(f),
        },
        (true, None) => words::ends(f),
        (false, Some((lo, hi))) => (Some(lo), Some(hi)),
        (false, None) => (None, None),
    }
}

/// A range from its ends, in the field's unit. A field a person fills, and a computed field read
/// over the whole of its written range, keep the range as written.
pub fn range_of(f: &Field, lo: Option<ritsu_units::Rat>, hi: Option<ritsu_units::Rat>) -> String {
    let same = |a: Option<ritsu_units::Rat>, b: Option<ritsu_units::Rat>| match (a, b) {
        (Some(a), Some(b)) => a.cmp_to(b) == std::cmp::Ordering::Equal,
        (a, b) => a.is_none() && b.is_none(),
    };
    let (wlo, whi) = words::ends(f);
    if f.range.is_some() && (f.computed.is_none() || (same(lo, wlo) && same(hi, whi))) {
        return words::range_text(f);
    }
    let mut parts = Vec::new();
    if let Some(l) = lo {
        parts.push(format!(">={}", words::written(Val::Num(l), &f.ty)));
    }
    if let Some(h) = hi {
        parts.push(format!("<={}", words::written(Val::Num(h), &f.ty)));
    }
    parts.join(" ")
}

/// A stage's output: its name in the rule, the stage's ASCII name, the enum of its values and how
/// each reads, whether it can be not there, and where it comes from. The two names differ for a
/// stage named as a word rulec reserves (`final` is `final_value`).
struct Output {
    name: String,
    ascii: String,
    enum_name: String,
    values: Vec<(String, String)>,
    /// The values only a `possible` or an example names, which no row gives: the claim or the
    /// example that does not hold says so (E306, E601), and rulec's word that no row gives them is
    /// not said again (W303).
    named_only: Vec<String>,
    optional: bool,
    from: usize,
    what: Text,
}

impl Output {
    fn add(&mut self, v: &(String, String)) {
        if !self.values.iter().any(|x| x.0 == v.0) {
            self.values.push(v.clone());
        }
    }
}

/// The inputs of the shared route the request uses, as it gives them: what it writes for each; for
/// an input it does not give, the input's own computation, false for a `bool`, else not there.
fn given(shared: &File, args: &[crate::ast::Arg]) -> Vec<Given> {
    let mut out: Vec<Given> = args.iter().map(|a| Given { name: a.name.text.clone(), value: Some(a.value.clone()), here: true }).collect();
    for i in &shared.inputs {
        if out.iter().any(|g| g.name == i.name.text) {
            continue;
        }
        let value = match (&i.computed, &i.ty) {
            (Some(e), _) => Some(e.clone()),
            (None, TypeExpr::Bool) => Some(Expr { kind: ExprKind::Bool(false), span: i.name.span }),
            _ => None,
        };
        out.push(Given { name: i.name.text.clone(), value, here: false });
    }
    out
}

/// The conditions lowered besides the route's own: the form's (`shown when`, `required when`) and
/// the route's claims (`only when`); each part of them their tables cannot hold (E309).
pub fn other_conditions(cx: &Ctx) -> Vec<Unread> {
    let mut out = Vec::new();
    for f in &cx.s.file.form {
        for e in [&f.shown_when, &f.required_when].into_iter().flatten() {
            let mut i = Inputs::new(cx);
            i.form = true;
            out.extend(i.cond(e, &[], f.line).err().unwrap_or_default());
        }
    }
    for c in &cx.s.file.claims {
        if let ClaimBody::OnlyWhen { when, .. } = &c.body {
            out.extend(Inputs::new(cx).cond(when, &[], c.line).err().unwrap_or_default());
        }
    }
    out
}

/// The span from the first word of a path to its last.
fn span_of(p: &[Word]) -> Span {
    let (a, b) = (p[0].span, p[p.len() - 1].span);
    Span { line: a.line, col: a.col, end: b.end }
}

/// The condition a row of a shared route's table holds on, over the route's inputs, written as an
/// expression: each cell's test of its column, all of them.
fn row_condition(t: &RouteTable, r: &crate::ast::RouteRow) -> Expr {
    let at = |span: Span| move |kind: ExprKind| Expr { kind, span };
    let mut all: Option<Expr> = None;
    for (k, (test, span)) in r.tests.iter().enumerate() {
        let Some(col) = t.inputs.get(k) else { continue };
        let mk = at(*span);
        let column = || Box::new(mk(ExprKind::Path(col.clone())));
        let one = match test {
            Test::Any => continue,
            Test::Values { values, not } => {
                let mut any: Option<Expr> = None;
                for v in values {
                    let is = mk(ExprKind::Cmp(CmpOp::Is, column(), Box::new(v.clone())));
                    any = Some(match any {
                        None => is,
                        Some(x) => mk(ExprKind::Or(Box::new(x), Box::new(is))),
                    });
                }
                let any = any.unwrap_or_else(|| mk(ExprKind::Bool(false)));
                if *not { mk(ExprKind::Not(Box::new(any))) } else { any }
            }
            Test::Compare(cmps) => {
                let mut every: Option<Expr> = None;
                for (op, v) in cmps {
                    let c = mk(ExprKind::Cmp(*op, column(), Box::new(v.clone())));
                    every = Some(match every {
                        None => c,
                        Some(x) => mk(ExprKind::And(Box::new(x), Box::new(c))),
                    });
                }
                every.unwrap_or_else(|| mk(ExprKind::Bool(true)))
            }
        };
        all = Some(match all {
            None => one,
            Some(x) => Expr { kind: ExprKind::And(Box::new(x), Box::new(one)), span: Span { line: r.line, col: 1, end: 1 } },
        });
    }
    all.unwrap_or(Expr { kind: ExprKind::Bool(true), span: Span { line: r.line, col: 1, end: 1 } })
}

/// `route.rule`, when the request has a route to lower, and what it names things by; else each part
/// of a condition the route's table cannot hold (E309). Ok(None) for a file with no route. A rule the
/// route reads only some of the outputs of is not brought in with `apply`: the code rulec writes of a
/// rule that applies another works out every output of it, and a language warns of a value nothing
/// reads (Rust's `unused_variables`, which a package is built without); its outputs the route reads
/// are inputs `fed by` it, as the outputs of a rule the route cannot apply.
pub fn route(cx: &Ctx) -> Result<Option<(LFile, super::RouteNames)>, Vec<Unread>> {
    let mut apart = std::collections::BTreeSet::new();
    loop {
        let mut partly = Vec::new();
        let lowered = route_apart(cx, &apart, &mut partly)?;
        if partly.is_empty() {
            return Ok(lowered);
        }
        apart.extend(partly);
    }
}

/// [`route`], the rules of `apart` not brought in with `apply`; Ok(None) with the aliases of the rules
/// it would apply and reads only some of the outputs of put in `partly`.
fn route_apart(cx: &Ctx, apart: &std::collections::BTreeSet<String>, partly: &mut Vec<String>) -> Result<Option<(LFile, super::RouteNames)>, Vec<Unread>> {
    let s = cx.s;
    let mut inputs = Inputs::new(cx);
    inputs.apply = true;
    inputs.apart = apart.clone();
    let mut outputs: Vec<Output> = Vec::new();
    // each table of the rule: its name, its columns' indexes, the output, the head's line, and rows
    let mut tables: Vec<(String, Vec<usize>, Vec<usize>, (usize, Text), Vec<(Vec<String>, usize, Text)>)> = Vec::new();
    let mut rows_text: Vec<(Vec<String>, usize, Text)> = Vec::new();
    let mut unread: Vec<Unread> = Vec::new();
    // a shared route the request uses, and what it gives it
    let shared = match &s.file.route {
        Some(Route::Shared { alias, args, line }) => match s.ringi(&alias.text, UseKind::Route) {
            Some(f) => Some((f, given(f, args), *line)),
            // what is wrong with its `use` line is said there
            None => return Ok(None),
        },
        _ => None,
    };
    match (&s.file.route, &shared) {
        // a table of the stages, or one that chooses the shared route (`-> route`): its one output's
        // values are the routes' names
        (Some(Route::Table(t)), _) => {
            // the columns of the inputs
            let mut cols = Vec::new();
            for p in &t.inputs {
                let at = span_of(p);
                let field = (p.len() == 1).then(|| s.field(&p[0].text)).flatten();
                let c = match field {
                    Some(f) => match inputs.unreadable(f) {
                        Some(why) => Err(inputs.unread(why, at, true)),
                        None => Ok(inputs.field(f, false)),
                    },
                    None => match about_who(p) {
                        Some(why) => Err(inputs.unread(why, at, true)),
                        None => (p.len() == 2).then(|| inputs.output(&p[0].text, &p[1].text, t.line)).flatten().ok_or_else(|| inputs.unread(Unreadable::Other, at, true)),
                    },
                };
                match c {
                    Ok(c) => cols.push(c),
                    Err(u) => unread.extend(u),
                }
            }
            if !unread.is_empty() {
                return Err(unread);
            }
            for st in &t.stages {
                let n = inputs.names.take(st.ascii());
                outputs.push(Output { name: n.clone(), ascii: st.ascii().to_string(), enum_name: format!("{n}_by"), values: Vec::new(), named_only: Vec::new(), optional: false, from: t.line, what: stage_what(&st.text) });
            }
            for (i, r) in t.rows.iter().enumerate() {
                let mut cells: Vec<String> = Vec::new();
                for (k, (test, _)) in r.tests.iter().enumerate() {
                    let field = t.inputs.get(k).filter(|p| p.len() == 1).and_then(|p| s.field(&p[0].text));
                    cells.push(words::test_text(s, field, test));
                }
                for (k, (spec, _)) in r.stages.iter().enumerate() {
                    let value = match spec {
                        Spec::Route(w) => Some((words::alias_ascii(s, &w.text), w.text.clone())),
                        _ => words::spec_value(s, spec),
                    };
                    match value {
                        Some(v) => {
                            outputs[k].add(&v);
                            cells.push(v.0);
                        }
                        None => {
                            outputs[k].optional = true;
                            cells.push("none".into());
                        }
                    }
                }
                let n = i + 1;
                rows_text.push((cells, r.line, what(format!("経路の表の {n} 行目"), format!("row {n} of the route"))));
            }
            let outs: Vec<usize> = (0..outputs.len()).collect();
            tables.push(("route".into(), cols, outs, (t.line, what("経路の表", "the route's table")), rows_text.clone()));
        }
        // a shared route written as a table: each row holds on its tests of the inputs, as the
        // request gives them, and the rows of the request's table are the cells where it holds
        (Some(Route::Shared { alias, .. }), Some((f, args, line))) if matches!(f.route, Some(Route::Table(_))) => {
            let Some(Route::Table(t)) = &f.route else { unreachable!() };
            let w = what(format!("共有の経路 `{}` の表", alias.text), format!("the table of the shared route `{}`", alias.text));
            for st in &t.stages {
                let n = inputs.names.take(st.ascii());
                outputs.push(Output { name: n.clone(), ascii: st.ascii().to_string(), enum_name: format!("{n}_by"), values: Vec::new(), named_only: Vec::new(), optional: false, from: *line, what: stage_what(&st.text) });
            }
            let mut conds = Vec::new();
            inputs.here = false;
            for r in &t.rows {
                match inputs.cond(&row_condition(t, r), args, *line) {
                    Ok(c) => conds.push(c),
                    Err(u) => unread.extend(u),
                }
            }
            inputs.here = true;
            if !unread.is_empty() {
                return Err(unread);
            }
            let mut used: Vec<usize> = Vec::new();
            for c in &conds {
                collect_cols(c, &mut used);
            }
            used.sort();
            let all = inputs.cols();
            let sub: Vec<Col> = used.iter().map(|i| all[*i].clone()).collect();
            let mut rows = Vec::new();
            for (r, c) in t.rows.iter().zip(&conds) {
                for (cells, holds) in words::rows(&sub, &remap_cond(c, &used)) {
                    if !holds {
                        continue;
                    }
                    let mut texts: Vec<String> = cells.iter().zip(&sub).map(|(c, col)| words::cell_text(col, c)).collect();
                    for (k, (spec, _)) in r.stages.iter().enumerate() {
                        match words::spec_value(s, spec) {
                            Some(v) => {
                                outputs[k].add(&v);
                                texts.push(v.0);
                            }
                            None => {
                                outputs[k].optional = true;
                                texts.push("none".into());
                            }
                        }
                    }
                    rows.push((texts, *line, w.clone()));
                }
            }
            let outs: Vec<usize> = (0..outputs.len()).collect();
            tables.push(("route".into(), used, outs, (*line, w), rows));
        }
        (Some(Route::Stages(_)), _) | (Some(Route::Shared { .. }), Some(_)) => {
            // the lines, what the shared route is given, and the request's `route` line, which a
            // line of a shared route is said at
            let (lines, args, shared_line): (&[StageLine], &[Given], Option<usize>) = match (&s.file.route, &shared) {
                (Some(Route::Stages(lines)), _) => (lines, &[], None),
                (_, Some((f, args, line))) => match &f.route {
                    Some(Route::Stages(lines)) => (lines, args, Some(*line)),
                    _ => return Ok(None),
                },
                _ => return Ok(None),
            };
            for l in lines {
                let at = shared_line.unwrap_or(l.line);
                let n = inputs.names.take(l.name.ascii());
                let w = stage_what(&l.name.text);
                let mut out = Output { name: n.clone(), ascii: l.name.ascii().to_string(), enum_name: format!("{n}_by"), values: Vec::new(), named_only: Vec::new(), optional: false, from: at, what: w.clone() };
                inputs.here = shared_line.is_none();
                let cond = match &l.when {
                    Some(e) => inputs.cond(e, args, at),
                    None => Ok(Cond::Const(true)),
                };
                // a rule's output names the approver: a column of its values
                let by = match &l.spec {
                    Spec::RuleOutput(p) if p.len() == 2 => match inputs.output(&p[0].text, &p[1].text, at) {
                        Some(c) => Ok(Some(c)),
                        None => Err(inputs.unread(Unreadable::Other, l.spec_span, inputs.here)),
                    },
                    _ => Ok(None),
                };
                inputs.here = true;
                // every part no line can be read by is said, and nothing is lowered
                let (cond, by) = match (cond, by) {
                    (Ok(c), Ok(b)) if unread.is_empty() => (c, b),
                    (c, b) => {
                        unread.extend(c.err().unwrap_or_default());
                        unread.extend(b.err().unwrap_or_default());
                        continue;
                    }
                };
                let all = inputs.cols();
                let mut used: Vec<usize> = Vec::new();
                collect_cols(&cond, &mut used);
                if let Some(b) = by
                    && !used.contains(&b)
                {
                    used.push(b);
                }
                used.sort();
                let sub: Vec<Col> = used.iter().map(|i| all[*i].clone()).collect();
                let remap = |c: &Cond| remap_cond(c, &used);
                let cond_sub = remap(&cond);
                let k_by = by.and_then(|b| used.iter().position(|x| *x == b));
                let mut rows = Vec::new();
                for (cells, holds) in words::rows(&sub, &cond_sub) {
                    // a rule's output that names the approver: where the stage is there, one row for
                    // each of its values, which names the approver of that value
                    let each = match k_by {
                        Some(k) if holds => one_value_each(&sub[k], &cells, k),
                        _ => vec![cells],
                    };
                    for cells in each {
                        let mut texts: Vec<String> = cells.iter().zip(&sub).map(|(c, col)| words::cell_text(col, c)).collect();
                        let value = if !holds {
                            None
                        } else if let Some(k) = k_by {
                            match &cells[k] {
                                Cell::Values(vs) if vs.len() == 1 => Some((words::ascii_of_who(s, &vs[0]), vs[0].clone())),
                                _ => None,
                            }
                        } else {
                            words::spec_value(s, &l.spec)
                        };
                        match value {
                            Some(v) => {
                                out.add(&v);
                                texts.push(v.0);
                            }
                            None => {
                                out.optional = true;
                                texts.push("none".into());
                            }
                        }
                        rows.push((texts, at, w.clone()));
                    }
                }
                // a stage no request reaches still names its approver, so that rulec says the value
                // is in no row (W303)
                if out.values.is_empty()
                    && by.is_none()
                    && let Some(v) = words::spec_value(s, &l.spec)
                {
                    out.add(&v);
                }
                let k = outputs.len();
                outputs.push(out);
                tables.push((n, used, vec![k], (at, w), rows));
            }
            if !unread.is_empty() {
                return Err(unread);
            }
        }
        _ => return Ok(None),
    }
    // a value a claim or an example names is a value of the stage too, though no row gives it: rulec
    // then says it is in no row (W303 for `only when`, which holds of a value no request comes to),
    // and the `possible` or the example does not hold (E306, E601)
    for c in &s.file.claims {
        if let ClaimBody::Possible { stage, spec } | ClaimBody::OnlyWhen { stage, spec, .. } = &c.body
            && let Some(o) = outputs.iter_mut().find(|o| stage_named(s, &o.ascii, stage))
            && let Some(v) = words::spec_value(s, spec)
        {
            let given = o.values.iter().any(|x| x.0 == v.0);
            match &c.body {
                ClaimBody::Possible { .. } if !given => o.named_only.push(v.0.clone()),
                ClaimBody::OnlyWhen { .. } => o.named_only.retain(|x| *x != v.0),
                _ => {}
            }
            o.add(&v);
        }
    }
    if let Some(ex) = &s.file.examples {
        for r in &ex.rows {
            for (k, (spec, _)) in r.stages.iter().enumerate() {
                if let Some(st) = ex.stages.get(k)
                    && let Some(o) = outputs.iter_mut().find(|o| stage_named(s, &o.ascii, st))
                    && let Some(v) = words::spec_value(s, spec)
                {
                    if !o.values.iter().any(|x| x.0 == v.0) {
                        o.named_only.push(v.0.clone());
                    }
                    o.add(&v);
                }
            }
        }
    }
    // the stages' outputs and their values
    let mut decl = Names::rulec();
    for i in &inputs.list {
        decl.take(&i.name);
    }
    for o in &outputs {
        decl.take(&o.name);
    }
    // the enum of each output, none of the enums the rule imports (a choice `first_by`)
    let mut enums = Names::enums();
    for (.., e) in super::form::output_enums(cx) {
        enums.keep(&e);
    }
    for (_, e) in super::choice_enums(cx) {
        enums.keep(&e);
    }
    for o in outputs.iter_mut() {
        o.enum_name = enums.take(&o.enum_name);
    }
    // one value for each way and each value of a stage in the whole rule, none of them another's
    let wants: Vec<String> = outputs.iter().flat_map(|o| o.values.iter().map(|(v, _)| v.clone())).collect();
    let valued = words::approver_values(&wants, &decl, &words::declared(s));
    let value_of = |v: &str| valued.iter().find(|(w, _)| w == v).map(|(_, n)| n.clone()).unwrap_or_else(|| v.to_string());
    let mut values_renamed: Vec<(String, String)> = Vec::new();
    for o in outputs.iter_mut() {
        let renamed: Vec<(String, String)> = o.values.iter().map(|(v, shown)| (value_of(v), shown.clone())).collect();
        for (old, new) in o.values.iter().zip(&renamed) {
            if old.0 != new.0 && !values_renamed.iter().any(|(g, _)| *g == new.0) {
                values_renamed.push((new.0.clone(), old.0.clone()));
            }
        }
        // the rows write the values renamed too
        for t in tables.iter_mut() {
            for (cells, ..) in t.4.iter_mut() {
                for (old, new) in o.values.iter().zip(&renamed) {
                    let at = t.1.len();
                    for c in cells.iter_mut().skip(at) {
                        if *c == old.0 {
                            *c = new.0.clone();
                        }
                    }
                }
            }
        }
        o.named_only = o.named_only.iter().map(|v| o.values.iter().zip(&renamed).find(|(old, _)| old.0 == *v).map(|(_, new)| new.0.clone()).unwrap_or_else(|| v.clone())).collect();
        o.values = renamed;
    }
    partly.extend(inputs.applied.iter().filter(|a| a.outs.len() < a.declared.len()).map(|a| a.alias.clone()));
    if !partly.is_empty() {
        return Ok(None);
    }
    let mut out = Out::default();
    let name = cx.name();
    let file = cx.shown();
    let none_w = Text::default();
    out.put(format!("rule {name}_route v1"), 0, &none_w);
    let described = if matches!(&s.file.route, Some(Route::Table(t)) if t.chooses_route) {
        format!("description \"What ringi writes from the route of {file}: the shared route each request takes\"")
    } else {
        format!("description \"What ringi writes from the route of {file}: one optional output for each stage, whose value is the approver the stage names; a blank cell is none, the stage is not there\"")
    };
    out.put(described, 0, &none_w);
    out.blank();
    let imports = inputs.imports();
    for e in &imports {
        out.put(format!("import enum {e} from \"choices.rule\""), 0, &none_w);
    }
    for o in &outputs {
        if o.values.is_empty() {
            continue;
        }
        let vs: Vec<String> = o.values.iter().map(|(v, shown)| format!("{v} \"{}\"", shown.replace('"', "'"))).collect();
        out.put(format!("enum {} = {}", o.enum_name, vs.join(" | ")), o.from, &o.what);
    }
    out.blank();
    inputs.write(&mut out);
    out.blank();
    out.put("outputs", 0, &none_w);
    let w = outputs.iter().map(|o| o.name.chars().count()).max().unwrap_or(0);
    for o in &outputs {
        let ty = if o.values.is_empty() { "bool".to_string() } else { format!("{}{}", o.enum_name, if o.optional { "?" } else { "" }) };
        out.put(format!("  {}{} : {ty}", o.name, " ".repeat(w - o.name.chars().count())), o.from, &o.what);
    }
    for (tname, cols, outs, head, rows) in &tables {
        out.blank();
        out.put(format!("table {tname}"), head.0, &head.1);
        let ins: Vec<String> = cols.iter().map(|c| inputs.list[*c].name.clone()).collect();
        let os: Vec<String> = outs.iter().map(|o| outputs[*o].name.clone()).collect();
        out.table(&ins, &os, (head.0, &head.1), rows);
    }
    // the claims of the route that rulec decides: `possible`
    let mut possible = Vec::new();
    for c in &s.file.claims {
        if let ClaimBody::Possible { stage, spec } = &c.body {
            let Some(o) = outputs.iter().find(|o| stage_named(s, &o.ascii, stage)) else { continue };
            let Some((v, _)) = words::spec_value(s, spec) else { continue };
            let v = value_of(&v);
            let n = &c.name.text;
            possible.push((format!("possible {} = {v}", o.name), c.line, what(format!("主張 `{n}`"), format!("the claim `{n}`"))));
        }
    }
    if !possible.is_empty() {
        out.blank();
        for (l, from, w) in &possible {
            out.put(l, *from, w);
        }
    }
    examples(cx, &inputs, &outputs, &valued, &mut out);
    let stage_names: Vec<String> = s.stages().unwrap_or_default().iter().map(|n| n.text.clone()).collect();
    let names = super::RouteNames {
        inputs: inputs.list.iter().map(|i| (i.reads.clone(), i.name.clone(), i.col.kind.clone())).collect(),
        worked: inputs.list.iter().filter(|i| i.derived && i.reads.len() == 2).map(|i| i.name.clone()).collect(),
        stages: outputs
            .iter()
            .map(|o| {
                let written = stage_names.iter().find(|n| s.stages().unwrap_or_default().iter().any(|x| x.text == **n && x.ascii() == o.ascii)).cloned().unwrap_or_else(|| o.ascii.clone());
                (written, o.name.clone(), o.values.clone())
            })
            .collect(),
        named_only: outputs.iter().flat_map(|o| o.named_only.iter().map(|v| (o.name.clone(), v.clone()))).collect(),
        enums: outputs.iter().filter(|o| !o.values.is_empty()).map(|o| (o.name.clone(), o.enum_name.clone())).collect(),
        applied: inputs.applied.iter().map(|a| a.alias.clone()).collect(),
    };
    let renamed = inputs.names.renamed().into_iter().chain(values_renamed).collect();
    Ok(Some((LFile { name: "route.rule".into(), tool: LoweredTool::Rulec, part: Part::Route, text: out.text, lines: out.lines, renamed, given: Vec::new(), reads: Vec::new() }, names)))
}

/// Whether the output `out` is the stage `stage` names.
fn stage_named(s: &crate::names::Scope, out: &str, stage: &Word) -> bool {
    s.stages().unwrap_or_default().iter().any(|n| n.text == stage.text && n.ascii() == out)
}

fn stage_what(stage: &str) -> Text {
    what(format!("ステップ `{stage}`"), format!("the stage `{stage}`"))
}

/// The columns a condition reads.
fn collect_cols(c: &Cond, out: &mut Vec<usize>) {
    match c {
        Cond::Atom(Atom::True(i)) | Cond::Atom(Atom::Absent(i)) | Cond::Atom(Atom::In(i, _)) | Cond::Atom(Atom::Cmp(i, ..)) => {
            if !out.contains(i) {
                out.push(*i);
            }
        }
        Cond::And(a, b) | Cond::Or(a, b) => {
            collect_cols(a, out);
            collect_cols(b, out);
        }
        Cond::Not(a) => collect_cols(a, out),
        Cond::Const(_) => {}
    }
}

/// The condition with its columns numbered among `used`.
fn remap_cond(c: &Cond, used: &[usize]) -> Cond {
    let at = |i: &usize| used.iter().position(|u| u == i).unwrap_or(0);
    match c {
        Cond::Atom(Atom::True(i)) => Cond::Atom(Atom::True(at(i))),
        Cond::Atom(Atom::Absent(i)) => Cond::Atom(Atom::Absent(at(i))),
        Cond::Atom(Atom::In(i, v)) => Cond::Atom(Atom::In(at(i), v.clone())),
        Cond::Atom(Atom::Cmp(i, op, v)) => Cond::Atom(Atom::Cmp(at(i), *op, *v)),
        Cond::And(a, b) => Cond::And(Box::new(remap_cond(a, used)), Box::new(remap_cond(b, used))),
        Cond::Or(a, b) => Cond::Or(Box::new(remap_cond(a, used)), Box::new(remap_cond(b, used))),
        Cond::Not(a) => Cond::Not(Box::new(remap_cond(a, used))),
        Cond::Const(b) => Cond::Const(*b),
    }
}

/// A row whose cell `k` (a rule's output that names the approver) holds several values, as one row
/// for each value, in the column's order.
fn one_value_each(col: &Col, cells: &[Cell], k: usize) -> Vec<Vec<Cell>> {
    let ColKind::Values(all) = &col.kind else { return vec![cells.to_vec()] };
    // `-` and `not:` take `none` too, where the output may be none: a row of its own, which names
    // no approver
    let (vals, none): (Vec<String>, bool) = match &cells[k] {
        Cell::Any => (all.clone(), col.opt),
        Cell::Values(vs) => (all.iter().filter(|v| vs.contains(v)).cloned().collect(), false),
        Cell::NotValues(rest) => (all.iter().filter(|v| !rest.contains(v)).cloned().collect(), true),
        _ => return vec![cells.to_vec()],
    };
    let mut out: Vec<Vec<Cell>> = vals
        .into_iter()
        .map(|v| {
            let mut cs = cells.to_vec();
            cs[k] = Cell::Values(vec![v]);
            cs
        })
        .collect();
    if none {
        let mut cs = cells.to_vec();
        cs[k] = Cell::None;
        out.push(cs);
    }
    out
}

/// The route's examples, under `examples`, when the request writes them: the columns the route
/// reads, an empty stage `none`.
fn examples(cx: &Ctx, inputs: &Inputs, outputs: &[Output], valued: &[(String, String)], out: &mut Out) {
    let s = cx.s;
    let Some(ex) = &s.file.examples else { return };
    // a column the route does not read is left out: no stage comes by it, and the rest of the row
    // still says what the route gives
    let mut heads_in = Vec::new();
    let mut kept = Vec::new();
    for p in &ex.inputs {
        let i = inputs.list.iter().find(|i| i.reads == p.iter().map(|w| w.text.clone()).collect::<Vec<_>>());
        kept.push(i.is_some());
        if let Some(i) = i {
            heads_in.push(i.name.clone());
        }
    }
    let mut heads_out = Vec::new();
    for st in &ex.stages {
        let Some(o) = outputs.iter().find(|o| stage_named(s, &o.ascii, st)) else { return };
        heads_out.push(o.name.clone());
    }
    let mut rows = Vec::new();
    for (i, r) in ex.rows.iter().enumerate() {
        let mut cells = Vec::new();
        for (k, (v, _)) in r.values.iter().enumerate() {
            if !kept.get(k).copied().unwrap_or(false) {
                continue;
            }
            let field = ex.inputs.get(k).filter(|p| p.len() == 1).and_then(|p| s.field(&p[0].text));
            cells.push(words::value_text(s, field, v));
        }
        for (spec, _) in &r.stages {
            let cell = match words::spec_value(s, spec) {
                Some((v, _)) => valued.iter().find(|(w, _)| *w == v).map(|(_, n)| n.clone()).unwrap_or(v),
                None => "none".into(),
            };
            cells.push(cell);
        }
        let n = i + 1;
        rows.push((cells, r.line, what(format!("例の {n} 行目"), format!("row {n} of the examples"))));
    }
    out.blank();
    out.put("examples", ex.line, &what("例", "the examples"));
    out.table(&heads_in, &heads_out, (ex.line, &what("例", "the examples")), &rows);
}

/// A named table lowered to `<name>.rule` (DESIGN 6.4): a column that names a field holds that
/// field's values, any other the ways to choose an approver (an enum `approver` of every way the
/// file declares).
pub fn named_table(cx: &Ctx, t: &NamedTable) -> LFile {
    let s = cx.s;
    let mut names = Names::rulec().avoiding(&words::declared(s));
    let tw = what(format!("表 `{}`", t.name.text), format!("the table `{}`", t.name.text));
    let mut out = Out::default();
    let name = cx.name();
    // the table's name, the name of its file and of its rule's: none of the other lowered rules'
    let tname = table_names(s).into_iter().find(|(n, _)| *n == t.name.text).map(|(_, l)| l).unwrap_or_else(|| t.name.ascii().to_string());
    let none_w = Text::default();
    out.put(format!("rule {name}_{tname} v1"), 0, &none_w);
    out.put(format!("description \"What ringi writes from the table {} of {}\"", t.name.text, cx.shown()), 0, &none_w);
    out.blank();
    let mut imports = Vec::new();
    let col_of = |n: &crate::ast::Name| s.field(&n.text);
    for c in t.inputs.iter().chain(&t.outputs) {
        if let Some(f) = col_of(c)
            && let Some(e) = super::enum_of(cx, f)
            && !imports.contains(&e)
        {
            imports.push(e);
        }
    }
    // the enum is a type, whose name the columns may take too (`approver : approver`), and none of
    // the enums the rule imports (a choice `approver` a column reads)
    let mut enums = Names::enums();
    for e in &imports {
        enums.keep(e);
    }
    let enum_name = enums.take("approver");
    for e in &imports {
        out.put(format!("import enum {e} from \"choices.rule\""), 0, &none_w);
    }
    // the columns, then the ways to choose an approver as the values of the enum: a word rulec reads
    // otherwise in a value's place, or a column's name, is written with `_approver` after it, and a
    // number after that where another way has that name ([`words::approver_values`])
    let ins: Vec<String> = t.inputs.iter().map(|c| names.take(c.ascii())).collect();
    let outs: Vec<String> = t.outputs.iter().map(|c| names.take(c.ascii())).collect();
    let ways: Vec<String> = s.who_all().iter().map(|w| w.who.name.ascii().to_string()).collect();
    let valued = words::approver_values(&ways, &names, &words::declared(s));
    let who: Vec<(String, String, String)> = s
        .who_all()
        .iter()
        .map(|w| {
            let a = w.who.name.ascii().to_string();
            let v = valued.iter().find(|(x, _)| *x == a).map(|(_, v)| v.clone()).unwrap_or_else(|| a.clone());
            (a, v, w.who.name.text.clone())
        })
        .collect();
    let approver = |w: &str| {
        let a = words::ascii_of_who(s, w);
        who.iter().find(|(x, ..)| *x == a).map(|(_, v, _)| v.clone()).unwrap_or(a)
    };
    let vs: Vec<String> = who.iter().map(|(_, v, n)| format!("{v} \"{n}\"")).collect();
    out.put(format!("enum {enum_name} = {}", vs.join(" | ")), t.line, &tw);
    out.blank();
    out.put("inputs", 0, &none_w);
    for (c, n) in t.inputs.iter().zip(&ins) {
        let ty = match col_of(c) {
            Some(f) => super::enum_of(cx, f).unwrap_or_else(|| field_type(f)),
            None => enum_name.clone(),
        };
        let range = col_of(c).map(words::range_text).filter(|r| !r.is_empty()).map(|r| format!("  range {r}")).unwrap_or_default();
        // a field that may have no value is read as one that may be none, which a cell `none` takes
        // (DESIGN 4.9)
        let opt = col_of(c).is_some_and(|f| super::may_be_empty(f) || super::may_be_none(s, f));
        out.put(format!("  {n} : {ty}{}{range}", if opt { "?" } else { "" }), t.line, &tw);
    }
    out.blank();
    out.put("outputs", 0, &none_w);
    for (k, (c, n)) in t.outputs.iter().zip(&outs).enumerate() {
        let optional = t.rows.iter().any(|r| matches!(r.stages.get(k), Some((Spec::Empty, _))));
        let ty = match col_of(c) {
            Some(f) => super::enum_of(cx, f).unwrap_or_else(|| field_type(f)),
            None => enum_name.clone(),
        };
        out.put(format!("  {n} : {ty}{}", if optional { "?" } else { "" }), t.line, &tw);
    }
    out.blank();
    out.put(format!("table {tname}"), t.line, &tw);
    let rows: Vec<(Vec<String>, usize, Text)> = t
        .rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut cells: Vec<String> = r.tests.iter().enumerate().map(|(k, (test, _))| {
                let field = t.inputs.get(k).and_then(col_of);
                match (field, test) {
                    (None, crate::ast::Test::Values { values, not }) => {
                        let vs: Vec<String> = values.iter().map(|v| match &v.kind {
                            ExprKind::Path(p) if p.len() == 1 => approver(&p[0].text),
                            _ => words::value_text(s, None, v),
                        }).collect();
                        format!("{}{}", if *not { "not: " } else { "" }, vs.join(", "))
                    }
                    _ => words::test_text(s, field, test),
                }
            }).collect();
            for (spec, _) in &r.stages {
                cells.push(match spec {
                    Spec::Empty => "none".into(),
                    Spec::Who(ws) if ws.len() == 1 => approver(&ws[0].text),
                    other => words::spec_value(s, other).map(|v| v.0).unwrap_or_else(|| "none".into()),
                });
            }
            let n = i + 1;
            (cells, r.line, what(format!("表 `{}` の {n} 行目", t.name.text), format!("row {n} of the table `{}`", t.name.text)))
        })
        .collect();
    out.table(&ins, &outs, (t.line, &tw), &rows);
    let mut renamed = names.renamed();
    if tname != t.name.ascii() {
        renamed.push((tname.clone(), t.name.ascii().to_string()));
    }
    renamed.extend(who.iter().filter(|(a, v, _)| a != v).map(|(a, v, _)| (v.clone(), a.clone())));
    LFile { name: format!("{tname}.rule"), tool: LoweredTool::Rulec, part: Part::Table(t.name.text.clone()), text: out.text, lines: out.lines, renamed, given: Vec::new(), reads: Vec::new() }
}

/// The name each named table is lowered under, by its name as written: the name of its file
/// (`<name>.rule`), of its rule and of its table. Its ASCII name, unless rulec reserves the word or
/// another lowered rule of the request is named so (`route`, `form`, `values`, `choices`), when
/// `_value` follows it.
pub fn table_names(s: &crate::names::Scope) -> Vec<(String, String)> {
    let mut names = Names::rulec().avoiding(&words::declared(s));
    for other in ["route", "form", "values", "choices"] {
        names.keep(other);
    }
    s.file.tables.iter().map(|t| (t.name.text.clone(), names.take(t.name.ascii()))).collect()
}

/// A field's type as a rule declares it.
pub fn field_type(f: &Field) -> String {
    match &f.ty {
        TypeExpr::Num(u) => crate::ast::unit_type(u),
        TypeExpr::Date => "date".into(),
        TypeExpr::Bool => "bool".into(),
        _ => "string".into(),
    }
}
