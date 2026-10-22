//! Every expression's type (DESIGN 2.4, 3, 6): what a name names, what a call calls, and whether
//! each value fits where it goes.
//!
//! The arithmetic is rulec's (rulec's DESIGN §2.3): amounts of one unit add and compare, a number or
//! a rate multiplies an amount, an amount over another of its unit is a ratio; a literal is read in
//! the unit of what it meets (`15lb` beside a `mass[oz]`). What kaname adds is checked here too: a
//! division by a value, `and`, `or`, `not`, `!=`, the difference of two dates (a `number`), and
//! `x in USDc`. A value goes into a place — an argument, a field, a function's answer, an input of
//! a rule or a dates file, an argument of a transfer — only when its type is the place's (E102) and
//! it is rounded to the place's step (E103): an amount times a rate is not, until a rounding says
//! how. The weights of `running_totals` fill the fields of the records it answers, and are held to
//! the same rule.
//!
//! The diagnostics: E101 (a name that names nothing), E102 (types that do not agree), E103 (an
//! unrounded value where a rounded one goes), E104 (a literal, a conversion or a step that does not
//! fit), E105 (an operation the type does not have), E106 (a `match` that leaves values out, or an
//! arm nothing reaches), E107 (a call that does not give what it takes), E109 (a builtin used as it
//! is not), E110 (a word that reads as a name and as a value), E006 (a local named twice), E204 (a
//! transfer that is not the book's), E602 (an example that is not a call with an answer of its
//! type).

use crate::ast::*;
use crate::diag::Diag;
use crate::names::{self, Dc, Decls, Target, column_slot, date_input_slot};
use crate::ty::*;
use ritsu_base::text::Text;
use ritsu_ports::{BookFacts, RuleFacts};
use ritsu_units::{Dim, Rat, Unit};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// What a name, a call or a field names: kept by the expression's id for the stages after the
/// check.
#[derive(Clone, Debug, PartialEq)]
pub enum Ref {
    None,
    /// A parameter, a `let`, a parameter of a function given to a builtin, a value `some` names.
    Local,
    /// A value of an enum, by the enum and the value's name.
    EnumValue(EnumId, String),
    /// A function of a module: the module's file and the function's name.
    Fn(PathBuf, String),
    /// A rule, by its file.
    Rule(PathBuf),
    /// A date of a dates file.
    Date(PathBuf, String),
    Transfer(TransferId),
    Builtin(&'static str),
    /// A builtin written after a value, on the `.` of the call (`lines.sum(l => l.amount)`, `d.get(k)`,
    /// DESIGN 21.4): the value before the `.` is its first argument.
    Method(&'static str),
    /// A field of a record, by its name.
    Field(String),
    /// A record built, by its type.
    Record(RecId),
    /// A call of a function value (DESIGN 27.1), on the call: the value of what it calls is called with
    /// the arguments in order.
    CallValue,
}

/// A function of a module written as a value (DESIGN 27.1), which the stages after the check read as
/// the lambda `(x) => f(x: x)` written where it is (`wrapped::desugar`): the function, the type arguments
/// of a generic one, the types of the lambda's parameters and of its answer, and whether it is given to
/// a builtin (where a lambda is the builtin's own, DESIGN 27.3).
#[derive(Clone, Debug, PartialEq)]
pub struct NamedFn {
    pub file: PathBuf,
    pub name: String,
    /// The names the function declares its parameters by, in their order.
    pub pnames: Vec<String>,
    pub inst: Vec<Ty>,
    pub params: Vec<Ty>,
    pub ret: Ty,
    pub inline: bool,
}

/// What the check gives every expression of a module.
pub struct Typed {
    pub types: Vec<Ty>,
    pub refs: Vec<Ref>,
    /// The type a `let` gives its name, by the id of its value: the value's type, read in the type
    /// written after the name when one is (`let w : mass[oz] = 15lb` names 240 ounces).
    pub bound: BTreeMap<usize, Ty>,
    /// The types a call of a generic function gives its type parameters, in their order (DESIGN 26.2),
    /// by the id of the call: what the stages after the check read the callee's types in.
    pub inst: BTreeMap<usize, Vec<Ty>>,
    /// The functions of modules written as values (DESIGN 27.1), by the id of the name.
    pub named: BTreeMap<usize, NamedFn>,
}

struct Local {
    name: String,
    ty: Ty,
}

/// What a numeric operation found wrong, before it is said.
enum Why {
    /// Two types that do not agree.
    Types,
    /// The same dimension in two units: the conversion the person can write.
    Units(Box<(Unit, Unit)>),
    /// Two taxes.
    Taxes,
    /// A constant that is no whole number of the unit it meets: its value there.
    NotWhole(Box<(Rat, Unit)>),
}

pub struct Ck<'a> {
    pub dc: Dc<'a>,
    decls: &'a Decls,
    types: Vec<Ty>,
    refs: Vec<Ref>,
    bound: BTreeMap<usize, Ty>,
    inst: BTreeMap<usize, Vec<Ty>>,
    scopes: Vec<Vec<Local>>,
    /// The type parameters of the function checked now (DESIGN 26.1).
    tparams: Vec<names::TParamInfo>,
    /// The type of the answer of the function checked now: what a `try` passes an `err` or a `none`
    /// to (DESIGN 26.3).
    ret: Ty,
    /// Whether a `let` of the block checked now may have `try`: the block is one the function answers
    /// with (its body, and the arms of an `if` or a `match` that answer for it).
    try_ok: bool,
    named: BTreeMap<usize, NamedFn>,
    /// Whether the expression checked next may be a function value (DESIGN 27.1): an argument whose
    /// parameter takes one, the value of a `let`, what a call calls. Every expression takes it and
    /// leaves it false for the ones inside it.
    allow: bool,
    /// The same, for the answer of the block checked next: a block whose answer is the value of a `let`.
    answer_ok: bool,
}

/// The check of every function's body and every example of a module whose declarations are read.
pub fn check(dc: Dc<'_>, decls: &Decls) -> (Typed, Vec<Diag>) {
    let n = dc.module.exprs;
    let mut ck = Ck {
        dc,
        decls,
        types: vec![Ty::Error; n],
        refs: vec![Ref::None; n],
        bound: BTreeMap::new(),
        inst: BTreeMap::new(),
        scopes: Vec::new(),
        tparams: Vec::new(),
        ret: Ty::Error,
        try_ok: false,
        named: BTreeMap::new(),
        allow: false,
        answer_ok: false,
    };
    for (i, f) in ck.dc.module.fns.iter().enumerate() {
        ck.function(i, f);
    }
    for x in &ck.dc.module.examples {
        ck.example(x);
    }
    ck.growing_calls();
    let Ck { dc, types, refs, bound, inst, named, .. } = ck;
    (Typed { types, refs, bound, inst, named }, dc.diags)
}

/// A unit's one step, written as a literal of it: `1USDc`, `1oz`, `0.01%`, `1`.
fn one_of(unit: &Unit, step: Option<Rat>) -> String {
    match unit.dim {
        Dim::Rate => format!("{}%", step.unwrap_or(Rat::new(1, 100)).mul(Rat::int(100))),
        Dim::Number | Dim::Count(_) => "1".into(),
        _ => format!("1{}", unit.unit),
    }
}

impl<'a> Ck<'a> {
    fn err(&mut self, code: &'static str, at: Span, msg: Text) -> &mut Diag {
        self.dc.diags.push(Diag::at(code, self.dc.path, at.line, at.col, msg).source(self.dc.src));
        self.dc.diags.last_mut().expect("just pushed")
    }

    fn set(&mut self, e: &Expr, t: &Ty) -> Ty {
        if let Some(x) = self.types.get_mut(e.id) {
            *x = t.clone();
        }
        t.clone()
    }

    fn refer(&mut self, e: &Expr, r: Ref) {
        if let Some(x) = self.refs.get_mut(e.id) {
            *x = r;
        }
    }

    /// The source of an expression on its line, as written.
    fn text(&self, sp: Span) -> String {
        let line = self.dc.src.lines().nth(sp.line.saturating_sub(1)).unwrap_or("");
        let cs: Vec<char> = line.chars().collect();
        let (a, b) = (sp.col.saturating_sub(1).min(cs.len()), sp.end.saturating_sub(1).min(cs.len()));
        cs[a..b.max(a)].iter().collect::<String>().trim().to_string()
    }

    // ── Names of types, for messages ────────────────────────────────────────────

    /// The `use` name the module reads a file by.
    fn use_of(&self, file: &Path) -> Option<&str> {
        self.dc.uses.iter().find(|(_, (_, t))| t.canon() == file).map(|(n, _)| n.as_str())
    }

    fn named_by(&self, owner: &Owner, name: &str) -> String {
        match owner {
            Owner::Module(f) if f == self.dc.canon => name.to_string(),
            Owner::Module(f) => match self.use_of(f) {
                Some(u) => format!("{u}.{name}"),
                None => self.dc.world.modules.get(f).map(|c| format!("{}.{name}", c.module.name.text)).unwrap_or_else(|| name.to_string()),
            },
            Owner::Rule(f) => match self.use_of(f) {
                Some(u) => format!("{u}.{name}"),
                None => self.dc.world.rules.get(f).map(|r| format!("{}.{name}", r.rule)).unwrap_or_else(|| name.to_string()),
            },
        }
    }

    /// A type as the module would write it.
    pub fn show(&self, t: &Ty) -> String {
        match t {
            Ty::Num(n) => unit_text(&n.unit),
            Ty::Bool => "bool".into(),
            Ty::Str => "string".into(),
            Ty::Date => "date".into(),
            Ty::Enum(id) => names::with_args(self.named_by(&id.owner, &id.name), &self.shown_args(&id.args)),
            Ty::Record(RecId::Module { file, name, args }) => names::with_args(self.named_by(&Owner::Module(file.clone()), name), &self.shown_args(args)),
            Ty::Record(RecId::Variant { file, name, value, args }) => names::with_args(format!("{}.{value}", self.named_by(&Owner::Module(file.clone()), name)), &self.shown_args(args)),
            Ty::Param(n) => n.clone(),
            Ty::Result(a, e) => format!("result[{}, {}]", self.show(a), self.show(e)),
            Ty::Record(RecId::Outputs(f)) => self.named_by(&Owner::Rule(f.clone()), "outputs"),
            Ty::Record(RecId::Element(f, s)) => self.named_by(&Owner::Rule(f.clone()), s),
            Ty::Record(RecId::Running(a, b)) => format!("running[{}, {}]", self.show(a), self.show(b)),
            Ty::Record(RecId::Entry(a, b)) => format!("entry[{}, {}]", self.show(a), self.show(b)),
            Ty::Record(RecId::Indexed(a)) => format!("indexed[{}]", self.show(a)),
            Ty::List(x) => format!("list[{}]", self.show(x)),
            Ty::Dict(k, v) => format!("dict[{}, {}]", self.show(k), self.show(v)),
            Ty::Set(x) => format!("set[{}]", self.show(x)),
            Ty::EmptyDict => "[:]".into(),
            Ty::Opt(x) => format!("{}?", self.show(x)),
            Ty::Transfer(id) => {
                let book = self.use_of(&id.book).map(str::to_string).unwrap_or_else(|| self.dc.world.books.get(&id.book).map(|b| b.name.clone()).unwrap_or_default());
                format!("{book}.{}.{}", id.transfer, id.op)
            }
            Ty::None => "none".into(),
            Ty::Empty => "[]".into(),
            Ty::Fn(ps, r, eff) => fn_text(&self.shown_args(ps), &self.show(r), eff.as_ref()),
            Ty::Error => "?".into(),
        }
    }

    fn shown_args(&self, args: &[Ty]) -> Vec<String> {
        args.iter().map(|a| self.show(a)).collect()
    }

    // ── What records and enums hold ─────────────────────────────────────────────

    /// A record of this module or of a module it reaches.
    fn record_info(&self, file: &Path, name: &str) -> Option<&names::RecordInfo> {
        let records = if file == self.dc.canon { &self.decls.records } else { &self.dc.world.modules.get(file)?.decls.records };
        records.iter().find(|r| r.name.text == *name)
    }

    /// The fields of a record: each its name and its place, the types its type arguments give its type
    /// parameters put in (DESIGN 26.1).
    fn fields(&self, id: &RecId) -> Option<Vec<(Name, Slot)>> {
        match id {
            RecId::Module { file, name, args } => {
                let r = self.record_info(file, name)?;
                let ps = names::tnames(&r.tparams);
                Some(r.fields.iter().map(|f| (f.name.clone(), subst_slot(&f.slot, &ps, args))).collect())
            }
            RecId::Outputs(file) => {
                let facts = self.dc.world.rules.get(file)?;
                Some(facts.outputs.iter().map(|c| (Name { text: c.name.clone(), alias: Some((c.alias.clone(), Span::default())), span: Span::default() }, column_slot(c, file))).collect())
            }
            RecId::Element(file, seq) => {
                let facts = self.dc.world.rules.get(file)?;
                let (_, _, cols) = facts.elements.iter().find(|(n, _, _)| n == seq)?;
                Some(cols.iter().map(|c| (Name { text: c.name.clone(), alias: Some((c.alias.clone(), Span::default())), span: Span::default() }, column_slot(c, file))).collect())
            }
            RecId::Variant { file, name, value, args } => {
                let e = self.enum_info(&EnumId::plain(Owner::Module(file.clone()), name.clone()))?;
                let ps = names::tnames(&e.tparams);
                Some(e.fields_of(value)?.iter().map(|f| (f.name.clone(), subst_slot(&f.slot, &ps, args))).collect())
            }
            RecId::Running(item, weight) => {
                let named = |w: &str| Name { text: w.to_string(), alias: None, span: Span::default() };
                Some(vec![
                    (named("item"), Slot::of((**item).clone())),
                    (named("before"), Slot::of((**weight).clone())),
                    (named("upto"), Slot::of((**weight).clone())),
                    (named("total"), Slot::of((**weight).clone())),
                ])
            }
            RecId::Entry(k, v) => {
                let named = |w: &str| Name { text: w.to_string(), alias: None, span: Span::default() };
                Some(vec![(named("key"), Slot::of((**k).clone())), (named("value"), Slot::of((**v).clone()))])
            }
            RecId::Indexed(x) => {
                let named = |w: &str| Name { text: w.to_string(), alias: None, span: Span::default() };
                Some(vec![(named("index"), Slot::of(Ty::Num(NumTy::number()))), (named("item"), Slot::of((**x).clone()))])
            }
        }
    }

    /// An enum of this module or of a module it reaches, with its values' fields.
    fn enum_info(&self, id: &EnumId) -> Option<&names::EnumInfo> {
        let Owner::Module(file) = &id.owner else { return None };
        let enums = if file == self.dc.canon { &self.decls.enums } else { &self.dc.world.modules.get(file)?.decls.enums };
        enums.iter().find(|e| e.name.text == id.name)
    }

    /// Whether an enum's values have fields: a sum, whose values each have a type (DESIGN 23.1).
    fn is_sum(&self, id: &EnumId) -> bool {
        self.enum_info(id).is_some_and(|e| e.is_sum())
    }

    /// The type of a value of a sum, by the value's declared name.
    fn variant_of(id: &EnumId, value: &str) -> Option<RecId> {
        RecId::variant(id, value)
    }

    /// E107 where a value of an enum that has fields is written as a value with none (`Order.placed`
    /// for `Order.placed { … }`): Some(the error type) where it is said.
    fn value_with_fields(&mut self, id: &EnumId, v: &str, at: Span) -> Option<Ty> {
        let fields = self.enum_info(id)?.fields_of(v)?.to_vec();
        if fields.is_empty() {
            return None;
        }
        let en = self.named_by(&id.owner, &id.name);
        let shown = fields.iter().map(|f| format!("{}: …", f.name.text)).collect::<Vec<_>>().join(", ");
        self.err("E107", at, tr!("`{en}.{v}` はフィールドを持つ値なので、名前だけでは書けません", "`{en}.{v}` is a value with fields, and is not written by its name alone"))
            .notes
            .push(tr!("`{en}.{v} {{ {shown} }}` のように、フィールドを全部書いてください。", "Write every field, as in `{en}.{v} {{ {shown} }}`."));
        Some(Ty::Error)
    }

    /// Whether a word names an enum: by its name or its alias.
    fn enum_word_is(&self, id: &EnumId, w: &str) -> bool {
        match &id.owner {
            Owner::Module(file) => {
                let enums = if file == self.dc.canon {
                    &self.decls.enums
                } else {
                    return self.dc.world.modules.get(file).is_some_and(|c| c.decls.enums.iter().any(|e| e.name.text == id.name && e.name.is(w)));
                };
                enums.iter().any(|e| e.name.text == id.name && e.name.is(w))
            }
            Owner::Rule(file) => self.dc.world.rules.get(file).is_some_and(|f| f.enums.iter().any(|e| e.name == id.name && names::rule_enum_is(e, w))),
        }
    }

    /// The values of an enum, and whether they are in order.
    fn values(&self, id: &EnumId) -> Option<(Vec<Name>, bool)> {
        match &id.owner {
            Owner::Module(file) => {
                let enums = if file == self.dc.canon { &self.decls.enums } else { &self.dc.world.modules.get(file)?.decls.enums };
                let e = enums.iter().find(|e| e.name.text == id.name)?;
                Some((e.values.clone(), e.ordered))
            }
            // a rule's enum is ordered when the rule writes `<` between its values, as the port says
            Owner::Rule(file) => {
                let facts = self.dc.world.rules.get(file)?;
                let e = facts.enums.iter().find(|e| e.name == id.name)?;
                let vs = e.values.iter().map(|v| Name { text: v.name.clone(), alias: (v.public != v.name).then(|| (v.public.clone(), Span::default())), span: Span::default() }).collect();
                Some((vs, e.ordered))
            }
        }
    }

    /// Whether two enums are one type: the same declaration, or one enum of an `enums` file that two
    /// rules read whole (DESIGN 3.5).
    fn same_enum(&self, x: &EnumId, y: &EnumId) -> bool {
        if x == y {
            return true;
        }
        let rule_enum = |id: &EnumId| match &id.owner {
            Owner::Rule(file) => self.dc.world.rules.get(file).and_then(|f| f.enums.iter().find(|e| e.name == id.name)),
            Owner::Module(_) => None,
        };
        match (rule_enum(x), rule_enum(y)) {
            (Some(a), Some(b)) => names::rule_enums_same(a, b),
            _ => false,
        }
    }

    // ── Locals ──────────────────────────────────────────────────────────────────

    fn lookup(&self, name: &str) -> Option<&Ty> {
        self.scopes.iter().rev().flat_map(|s| s.iter().rev()).find(|l| l.name == name).map(|l| &l.ty)
    }

    /// A name for a value in the innermost scope; E006 when a local or a `use` has it already.
    fn bind(&mut self, name: &str, ty: Ty, at: Span, check: bool) {
        if check {
            if self.lookup(name).is_some() {
                self.err("E006", at, tr!("`{name}` は、ここではもう名前になっています", "`{name}` is a name here already")).notes.push(tr!(
                    "kaname の変数は書き換えられず、一つの名前は関数の中で一つのものを指します。別の名前を付けてください。",
                    "A value of kaname is never written again, and a name means one thing in a function: choose another name."
                ));
            } else if self.dc.uses.contains_key(name) {
                self.err("E006", at, tr!("`{name}` は `use` で読んだものの名前です", "`{name}` is the name of what a `use` reads")).notes.push(tr!(
                    "`{name}(…)` や `{name}.…` がどちらを指すのか、読む人に分からなくなります。別の名前を付けてください。",
                    "A reader of `{name}(…)` or `{name}.…` could not tell which one it means: choose another name."
                ));
            }
        }
        if let Some(s) = self.scopes.last_mut() {
            s.push(Local { name: name.to_string(), ty });
        }
    }

    // ── Functions and examples ──────────────────────────────────────────────────

    /// E112 at a call in a ring of calls that gives a generic function of the ring a type argument made
    /// from a type parameter (DESIGN 26.2): `wrap_deep(x: [x], …)` in `wrap_deep[T]` gives `list[T]`,
    /// and each time the calls go round, the type argument would grow one deeper (`list[list[T]]`, …),
    /// which the generated Go cannot instantiate without end. A type parameter of the caller given as it
    /// is, and a type that names none, go round. A function written as a value is a call of it, as in
    /// the rings of the effects (DESIGN 27.5). The same rule as a type that holds itself (28.1).
    fn growing_calls(&mut self) {
        let module = self.dc.module;
        let canon = self.dc.canon.to_path_buf();
        let index = |name: &str| self.decls.fns.iter().position(|f| f.name.text == name);
        // each function's calls of the module's functions: the callee, where, and its type arguments
        let mut calls: Vec<Vec<(usize, Span, Vec<Ty>)>> = vec![Vec::new(); module.fns.len()];
        for (i, f) in module.fns.iter().enumerate() {
            let mut all = Vec::new();
            f.body.walk(&mut all);
            for e in all {
                let call = match &e.kind {
                    ExprKind::Call(callee, _) => match self.refs.get(callee.id) {
                        Some(Ref::Fn(file, name)) if *file == canon => index(name).map(|g| (g, self.inst.get(&e.id).cloned().unwrap_or_default())),
                        _ => None,
                    },
                    _ => None,
                };
                let named = || self.named.get(&e.id).filter(|n| n.file == canon).and_then(|n| index(&n.name).map(|g| (g, n.inst.clone())));
                if let Some((g, targs)) = call.or_else(named) {
                    calls[i].push((g, e.span, targs));
                }
            }
        }
        let edges: Vec<Vec<(usize, usize, Vec<Ty>)>> = calls.iter().map(|cs| cs.iter().enumerate().map(|(k, (g, _, t))| (*g, k, t.clone())).collect()).collect();
        let mut said = Vec::new();
        for part in names::strong_parts(module.fns.len(), &edges) {
            for &f in &part {
                let own = names::tnames(&self.decls.fns[f].tparams);
                for (g, at, targs) in &calls[f] {
                    if !part.contains(g) {
                        continue;
                    }
                    let grows = targs.iter().find(|a| !matches!(a, Ty::Param(p) if own.contains(p)) && a.has_param());
                    if let Some(a) = grows {
                        said.push((*at, self.decls.fns[f].name.text.clone(), self.decls.fns[*g].name.text.clone(), self.show(a)));
                    }
                }
            }
        }
        said.sort_by_key(|(at, ..)| (at.line, at.col));
        for (at, me, them, arg) in said {
            self.err(
                "E112",
                at,
                tr!(
                    "`{me}` は、呼び出しの輪の中で `{them}` を呼ぶところで、型の引数から作った型 `{arg}` を渡しています",
                    "Where `{me}` calls `{them}` in a ring of calls, it gives `{arg}`, a type argument made from a type parameter"
                ),
            )
            .notes
            .push(tr!(
                "呼び出しが輪を回るたびに、型の引数が一段ずつ深くなり（`list[list[T]]`、…）、呼ぶ型が限りなく増えます。生成した Go もこの形を書けません。呼び出しの輪の中では、型の引数をそのまま渡すか、型の引数を含まない型を渡してください。",
                "Each time the calls go round, the type argument grows one deeper (`list[list[T]]`, …), and the types it is called with never end; nor can the generated Go write them. In a ring of calls, give the type parameters as they are, or types that name none."
            ));
        }
    }

    fn function(&mut self, i: usize, f: &FnDecl) {
        let info = self.decls.fns[i].clone();
        self.scopes = vec![Vec::new()];
        // the type parameters are in scope for the types written in the body (DESIGN 26.1)
        self.tparams = info.tparams.clone();
        self.dc.scope = info.tparams.clone();
        self.ret = info.ret.ty.clone();
        self.try_ok = true;
        for p in &info.params {
            let n = p.name.text.clone();
            self.bind(&n, p.slot.ty.clone(), p.name.span, false);
            if self.dc.uses.contains_key(&n) {
                self.err("E006", p.name.span, tr!("`{n}` は `use` で読んだものの名前です", "`{n}` is the name of what a `use` reads")).notes.push(tr!(
                    "`{n}(…)` や `{n}.…` がどちらを指すのか、読む人に分からなくなります。別の名前を付けてください。",
                    "A reader of `{n}(…)` or `{n}.…` could not tell which one it means: choose another name."
                ));
            } else {
                self.fn_named(&n, &p.slot.ty, p.name.span);
            }
        }
        let want = info.ret.ty.clone();
        self.answer_ok = false;
        self.block(&f.body, Some(&want));
        let fun = &f.name.text;
        self.fits_at(&f.body.answer, &want, &tr!("関数 `{fun}` の答え", "the answer of `{fun}`"));
        self.scopes.clear();
        self.tparams.clear();
        self.dc.scope.clear();
        self.ret = Ty::Error;
        self.try_ok = false;
    }

    /// E006 for a function value given the name of a function of the module or of a builtin (DESIGN
    /// 27.1): which one `n(…)` calls would not be clear to a reader.
    fn fn_named(&mut self, n: &str, t: &Ty, at: Span) {
        if !matches!(t, Ty::Fn(..)) {
            return;
        }
        let what = if self.decls.fns.iter().any(|f| f.name.is(n)) {
            tr!("このモジュールの関数", "a function of the module")
        } else if crate::kw::is_builtin(n) {
            tr!("組み込みの関数", "a builtin function")
        } else {
            return;
        };
        self.err("E006", at, tr!("関数の値の名前 `{n}` は、{}の名前でもあります", "The function value `{n}` has the name of {}", what.ja; what.en)).notes.push(tr!(
            "`{n}(…)` がどちらを呼ぶのか、読む人に分からなくなります。別の名前を付けてください。",
            "A reader of `{n}(…)` could not tell which one it calls: choose another name."
        ));
    }

    /// Whether the value of `e` fits a place, said where it goes wrong: at the answer of each branch
    /// of an `if` or a `match`, rather than at the `if`.
    fn fits_at(&mut self, e: &Expr, want: &Ty, place: &Text) {
        // branches that did not join have been said already
        if self.types.get(e.id).is_none_or(Ty::is_error) {
            return;
        }
        match &e.kind {
            ExprKind::If(_, a, b) => {
                self.fits_at(&a.answer, want, place);
                self.fits_at(&b.answer, want, place);
            }
            ExprKind::Match(_, arms) => {
                for arm in arms {
                    self.fits_at(&arm.body.answer, want, place);
                }
            }
            ExprKind::Block(b) => self.fits_at(&b.answer, want, place),
            _ => {
                let got = self.types.get(e.id).cloned().unwrap_or(Ty::Error);
                self.fits(&got, want, e.span, place);
            }
        }
    }

    fn example(&mut self, x: &Example) {
        self.scopes = vec![Vec::new()];
        self.try_ok = false;
        let ExprKind::Call(callee, args) = &x.call.kind else { return };
        let found = callee.path().and_then(|p| match p.as_slice() {
            [(w, _)] => self.decls.fns.iter().position(|f| f.name.is(w)),
            _ => None,
        });
        let Some(fi) = found else {
            let shown = self.text(callee.span);
            self.err("E602", callee.span, tr!("例は、このモジュールの関数の呼び出しです。`{shown}` はそうではありません", "An example calls a function of this module, which `{shown}` is not"))
                .notes
                .push(tr!(
                    "例は `<関数>(<引数>: <値>, …) = <答え>` と書き、`check` が参照インタプリタで流して答えを比べます。",
                    "An example is `<function>(<parameter>: <value>, …) = <answer>`; `check` runs it in the reference interpreter and compares the answer."
                ));
            return;
        };
        let info = self.decls.fns[fi].clone();
        self.refer(callee, Ref::Fn(self.dc.canon.to_path_buf(), info.name.text.clone()));
        // a generic function: its type arguments from the arguments (DESIGN 26.2)
        let ret = if info.tparams.is_empty() {
            self.no_targs(&x.call, &info.name.text);
            self.call_fn(&info, args, callee.span, &x.call);
            info.ret.ty.clone()
        } else {
            self.call_generic(&info, args, &x.call, None)
        };
        self.set(&x.call, &ret);
        if let Expect::Value(want) = &x.expect {
            let got = self.expr(want, Some(&ret));
            if let Err(why) = self.agrees(&got, &ret) {
                let (g, w, fun) = (self.show(&got), self.show(&ret), info.name.text.clone());
                let d = self.err("E602", want.span, tr!("例の答えは {g} ですが、関数 `{fun}` が答えるのは {w} です", "The answer of the example is {g}, and `{fun}` answers {w}"));
                if let Some(n) = why {
                    d.notes.push(n);
                }
            }
        }
        self.scopes.clear();
    }

    /// Whether a value fits a type, saying nothing: Err with a note when it does not.
    fn agrees(&self, got: &Ty, want: &Ty) -> Result<(), Option<Text>> {
        match (got, want) {
            (Ty::Error, _) | (_, Ty::Error) => Ok(()),
            (Ty::None, Ty::Opt(_)) | (Ty::Empty, Ty::List(_)) | (Ty::EmptyDict, Ty::Dict(..)) => Ok(()),
            (Ty::Opt(g), Ty::Opt(w)) => self.agrees(g, w),
            (g, Ty::Opt(w)) => self.agrees(g, w),
            (Ty::List(g), Ty::List(w)) | (Ty::Set(g), Ty::Set(w)) => self.agrees(g, w),
            (Ty::Dict(gk, gv), Ty::Dict(wk, wv)) | (Ty::Result(gk, gv), Ty::Result(wk, wv)) => {
                self.agrees(gk, wk)?;
                self.agrees(gv, wv)
            }
            (Ty::Num(g), Ty::Num(w)) => match self.num_into(g, w) {
                Ok(_) => Ok(()),
                Err(_) => Err(None),
            },
            (Ty::Fn(gp, gr, _), Ty::Fn(wp, wr, _)) => {
                if gp.len() == wp.len() && gp.iter().zip(wp).all(|(g, w)| same_ty(g, w)) && same_ty(gr, wr) {
                    Ok(())
                } else {
                    Err(Some(fn_types_note()))
                }
            }
            (Ty::Enum(g), Ty::Enum(w)) if self.same_enum(g, w) => Ok(()),
            // a value of the type of a value of a sum goes where the sum goes (DESIGN 23.2)
            (Ty::Record(r), Ty::Enum(w)) if r.sum().as_ref() == Some(w) => Ok(()),
            (g, w) if g == w => Ok(()),
            _ => Err(None),
        }
    }

    // ── Blocks ──────────────────────────────────────────────────────────────────

    fn block(&mut self, b: &Block, want: Option<&Ty>) -> Ty {
        // the answer of a block whose answer is a `let`'s value may be a function value (DESIGN 27.1)
        let answer_ok = std::mem::take(&mut self.answer_ok);
        self.scopes.push(Vec::new());
        // a `let` may have `try` in a block the function answers with; the values of the `let` lines
        // are no such blocks (DESIGN 26.3)
        let answering = self.try_ok;
        for l in &b.lets {
            self.try_ok = false;
            let ty = match (&l.ty, l.tried) {
                (_, Some(at)) => self.tried(l, at, answering),
                (Some(t), None) => {
                    let declared = self.dc.resolve_in(t, names::Within::Value);
                    self.allow = true;
                    let got = self.expr(&l.value, Some(&declared));
                    self.as_declared(&got, &declared, l.value.span, &l.name)
                }
                (None, None) => {
                    self.allow = true;
                    self.expr(&l.value, None)
                }
            };
            self.fn_named(&l.name, &ty, l.span);
            if matches!(ty, Ty::None | Ty::Empty | Ty::EmptyDict) {
                let n = &l.name;
                let note = if matches!(ty, Ty::EmptyDict) {
                    tr!("`[:]` だけでは型が分かりません。`let {n} : <型> = …` と型を書いてください。", "`[:]` alone says no type: write it, as in `let {n} : <type> = …`.")
                } else {
                    tr!("`none` や `[]` だけでは型が分かりません。`let {n} : <型> = …` と型を書いてください。", "`none` or `[]` alone says no type: write it, as in `let {n} : <type> = …`.")
                };
                self.err("E102", l.value.span, tr!("`let {n}` の型が決まりません", "The type of `let {n}` cannot be told")).notes.push(note);
            }
            self.bound.insert(l.value.id, ty.clone());
            self.bind(&l.name, ty, l.span, true);
        }
        self.try_ok = answering;
        self.allow = answer_ok;
        let t = self.expr(&b.answer, want);
        self.try_ok = answering;
        self.scopes.pop();
        t
    }

    /// `let x = try e` (DESIGN 26.3): `e` a `result` or a value that may be none, the function's answer
    /// one that takes its `err` or its `none`; the type `x` is given. E115 where it cannot be written.
    fn tried(&mut self, l: &Let, at: Span, answering: bool) -> Ty {
        let declared = l.ty.as_ref().map(|t| self.dc.resolve_in(t, names::Within::Value));
        let got = self.expr(&l.value, None);
        if !answering {
            self.err("E115", at, tr!("ここには `try` を書けません", "`try` cannot be written here")).notes.push(crate::parse::try_note());
            return Ty::Error;
        }
        let ret = self.ret.clone();
        let fun_ret = self.show(&ret);
        let inner = match (&got, &ret) {
            (Ty::Error, _) | (_, Ty::Error) => return declared.unwrap_or(Ty::Error),
            (Ty::Result(a, e), Ty::Result(_, re)) => {
                if self.agrees(e, re).is_err() || self.agrees(re, e).is_err() {
                    let (es, rs) = (self.show(e), self.show(re));
                    self.err("E115", at, tr!("`try` の `err` の値は {es} ですが、関数が答える `err` の値は {rs} です", "The `err` of the `try` holds {es}, and the `err` the function answers holds {rs}")).notes.push(tr!(
                        "`try` は `err` をそのまま関数の答えにするので、二つの `err` の値の型が同じでなければなりません。`match` で分けて、関数の `err` の型に直してください。",
                        "`try` makes the `err` the function's answer as it is, so the two `err`s hold values of one type: split it with a `match`, and make the `err` of the function's type."
                    ));
                    return Ty::Error;
                }
                (**a).clone()
            }
            (Ty::Opt(a), Ty::Opt(_)) => (**a).clone(),
            (Ty::Result(..), _) => {
                self.err("E115", at, tr!("`try` が `err` を答えにするには、関数の答えが `result` の型でなければなりません。この関数は {fun_ret} を答えます", "For `try` to answer an `err`, the function answers a `result`; this one answers {fun_ret}")).notes.push(crate::parse::try_note());
                return Ty::Error;
            }
            (Ty::Opt(_), _) => {
                self.err("E115", at, tr!("`try` が `none` を答えにするには、関数の答えが値の無いことがある型（`T?`）でなければなりません。この関数は {fun_ret} を答えます", "For `try` to answer a `none`, the function answers a value that may be none (`T?`); this one answers {fun_ret}")).notes.push(crate::parse::try_note());
                return Ty::Error;
            }
            (other, _) => {
                let o = self.show(other);
                self.err("E115", at, tr!("`try` に渡せるのは `result` か値の無いことがある値です。これは {o} です", "`try` takes a `result` or a value that may be none; this is {o}")).notes.push(crate::parse::try_note());
                return Ty::Error;
            }
        };
        let inner = plain(inner);
        match declared {
            Some(d) => self.as_declared(&inner, &d, l.value.span, &l.name),
            None => inner,
        }
    }

    /// The type of `let x : T = …`: the value's, read in T's unit (DESIGN 3.2: the type of a `let`
    /// says the dimension and the unit; whether the value is rounded comes from the value).
    fn as_declared(&mut self, got: &Ty, declared: &Ty, at: Span, name: &str) -> Ty {
        match (got, declared) {
            (Ty::Error, _) | (_, Ty::Error) => declared.clone(),
            (Ty::Num(g), Ty::Num(d)) => match self.num_into(g, d) {
                Ok(n) => Ty::Num(n),
                Err(_) if Self::declares(g, d) => Ty::Num(NumTy { unit: Unit { step: None, ..d.unit.clone() }, ratio: false, ..g.clone() }),
                Err(why) => {
                    let (gs, ds) = (self.show(got), self.show(declared));
                    self.mismatch(at, tr!("`let {name}` の型 {ds} に、{gs} の値を入れています", "The value given to `let {name}`, of type {ds}, is {gs}"), why);
                    declared.clone()
                }
            },
            _ => {
                if let Err(n) = self.agrees(got, declared) {
                    let (gs, ds) = (self.show(got), self.show(declared));
                    let d = self.err("E102", at, tr!("`let {name}` の型 {ds} に、{gs} の値を入れています", "The value given to `let {name}`, of type {ds}, is {gs}"));
                    if let Some(n) = n {
                        d.notes.push(n);
                    }
                }
                declared.clone()
            }
        }
    }

    // ── Numbers ─────────────────────────────────────────────────────────────────

    /// Whether a `let`'s type says what a value is, where the value's own type leaves it open: a
    /// number and a rate are both dimensionless, and the type written says which one the value is
    /// (`let ratio : rate = total / 50`); an amount whose tax is not said takes the one written, and
    /// one written with no tax takes none (`let cost : money[JPY, incl_tax] = 10_000JPY * rate`).
    /// rulec gives a `define` and a `derive` the same latitude, and no other place (rulec's DESIGN
    /// §2.1, `check_same`): an amount with tax and one without still do not add up.
    fn declares(g: &NumTy, d: &NumTy) -> bool {
        let dimensionless = |n: &NumTy| matches!(n.unit.dim, Dim::Number | Dim::Rate);
        if dimensionless(g) && dimensionless(d) {
            return true;
        }
        let untaxed = |u: &Unit| Unit { tax: None, ..u.clone() };
        matches!(g.unit.dim, Dim::Money(_)) && (g.unit.tax.is_none() || d.unit.tax.is_none()) && same_unit(&untaxed(&g.unit), &untaxed(&d.unit))
    }

    /// A constant read in another unit of its dimension, with that unit's tax.
    fn konst_in(n: &NumTy, to: &Unit) -> Option<NumTy> {
        let v = n.konst?;
        let f = factor(&n.unit, to)?;
        // the value read with the unit's zero too: 30℃ beside a ℉ is 86℉ (a temperature is the one
        // dimension whose units do not share a zero, and it has no arithmetic)
        let x = convert(v, &n.unit, to)?;
        // and is on its own value, as a literal is, where the zeros differ (86℉, not 54℉)
        let step = step_in(n.step.map(|s| s.mul(f)), Some(x), &n.unit, to);
        Some(NumTy { unit: Unit { step: None, ..to.clone() }, step, konst: Some(x), ratio: false })
    }

    /// Whether a dimensionless value is of the kind of another: a ratio is a number or a rate.
    fn dimless_kind(a: &NumTy, b: &NumTy) -> Option<Unit> {
        match (a.ratio, b.ratio) {
            (true, true) => Some(Unit::number()),
            (true, false) if !b.has_unit() => Some(b.unit.clone()),
            (false, true) if !a.has_unit() => Some(a.unit.clone()),
            _ => None,
        }
    }

    /// Two numbers in one unit, for an operation that needs one (a sum, a comparison, an `if`'s two
    /// answers): a constant takes the unit of what it meets, a ratio the kind of a number or a rate.
    fn unify(&self, a: &NumTy, b: &NumTy) -> Result<(NumTy, NumTy), Why> {
        let zero = |n: &NumTy| n.konst.is_some_and(|v| v.num == 0) && n.unit.dim == Dim::Number;
        if zero(a) && !zero(b) {
            return Ok((NumTy { unit: b.unit.clone(), step: Some(Rat::zero()), konst: Some(Rat::zero()), ratio: b.ratio }, b.clone()));
        }
        if zero(b) && !zero(a) {
            return Ok((a.clone(), NumTy { unit: a.unit.clone(), step: Some(Rat::zero()), konst: Some(Rat::zero()), ratio: a.ratio }));
        }
        if let Some(u) = Self::dimless_kind(a, b) {
            return Ok((NumTy { unit: u.clone(), ..a.clone() }, NumTy { unit: u, ..b.clone() }));
        }
        if a.unit.dim != b.unit.dim {
            return Err(Why::Types);
        }
        if same_unit(&a.unit, &b.unit) {
            return Ok((a.clone(), b.clone()));
        }
        let whole = |n: &NumTy| n.unit.dim == Dim::Rate || n.konst.is_none_or(|v| v.is_int());
        if a.konst.is_some()
            && (b.konst.is_none() || !same_unit(&a.unit, &b.unit))
            && let Some(x) = Self::konst_in(a, &b.unit)
        {
            if !whole(&x) {
                return Err(Why::NotWhole(Box::new((x.konst.unwrap_or(Rat::zero()), b.unit.clone()))));
            }
            return Ok((x, b.clone()));
        }
        if b.konst.is_some()
            && let Some(x) = Self::konst_in(b, &a.unit)
        {
            if !whole(&x) {
                return Err(Why::NotWhole(Box::new((x.konst.unwrap_or(Rat::zero()), a.unit.clone()))));
            }
            return Ok((a.clone(), x));
        }
        if a.unit.tax != b.unit.tax && factor(&a.unit, &b.unit) == Some(Rat::int(1)) {
            return Err(Why::Taxes);
        }
        Err(Why::Units(Box::new((a.unit.clone(), b.unit.clone()))))
    }

    /// A number put into a place of type `want`: the value in the place's unit, or why it does not
    /// go (the step of the place is the caller's to hold).
    fn num_into(&self, got: &NumTy, want: &NumTy) -> Result<NumTy, Why> {
        if got.ratio && !want.has_unit() {
            return Ok(NumTy { unit: want.unit.clone(), ratio: false, ..got.clone() });
        }
        let (g, _) = self.unify(got, &NumTy { konst: None, ..want.clone() })?;
        Ok(g)
    }

    /// Say why two numbers do not go together (E102, E104), with what to write instead.
    fn mismatch(&mut self, at: Span, msg: Text, why: Why) {
        match why {
            Why::NotWhole(x) => {
                let (v, u) = (x.0.to_string(), unit_text(&x.1));
                let d = self.err("E104", at, tr!("定数を {u} で読むと、整数になりません（{v}）", "A constant read in {u} is no whole number ({v})"));
                d.notes.push(tr!(
                    "リテラルは、比べたり足したりする相手の単位で読みます。その単位の整数になる値で書いてください。",
                    "A literal is read in the unit of the value it is compared with or added to: write one that is a whole number of that unit."
                ));
                let _ = msg;
            }
            Why::Units(x) => {
                let (from, to) = *x;
                let (f, t) = (unit_text(&from), unit_text(&to));
                let d = self.err("E102", at, msg);
                d.notes.push(tr!(
                    "{f} と {t} は、同じ次元の違う単位です。黙って換算はしないので、`<値> in {}` のように換算を書いてください。",
                    "`<value> in {}` converts it: {f} and {t} are two units of one dimension, and nothing converts one into the other unasked.",
                    to.unit
                ));
            }
            Why::Taxes => {
                let d = self.err("E102", at, msg);
                d.notes.push(tr!(
                    "税込と税抜は別の型です。換算には税率が要るので、表に書いてください。",
                    "With tax and without are two types: turning one into the other takes a tax rate, which belongs in a table."
                ));
            }
            Why::Types => {
                self.err("E102", at, msg);
            }
        }
    }

    // ── Places ──────────────────────────────────────────────────────────────────

    /// Whether a value fits a place (DESIGN 3.2, 3.9): its type (E102, E104) and its step (E103).
    fn fits(&mut self, got: &Ty, want: &Ty, at: Span, place: &Text) {
        match (got, want) {
            (Ty::Error, _) | (_, Ty::Error) => {}
            (Ty::None, Ty::Opt(_)) | (Ty::Empty, Ty::List(_)) | (Ty::EmptyDict, Ty::Dict(..)) => {}
            (Ty::Opt(g), Ty::Opt(w)) => self.fits(g, w, at, place),
            (Ty::Opt(_), _) => {
                let (g, w) = (self.show(got), self.show(want));
                self.err("E102", at, tr!("{}は {w} で、値が無いことはありません。渡しているのは {g} です", "{} is {w}, never none; this is {g}", crate::diag::ja_before(&place.ja); place.en))
                    .notes
                    .push(tr!(
                        "値が無いことがある値は、`match` で `none` と `some x` に分けてから渡してください。",
                        "Split a value that may be none with a `match` on `none` and `some x` before it goes there."
                    ));
            }
            (g, Ty::Opt(w)) => self.fits(g, w, at, place),
            (Ty::List(g), Ty::List(w)) => {
                let p = tr!("{}の要素", "an element of {}", crate::diag::ja_before(&place.ja); place.en);
                self.fits(g, w, at, &p)
            }
            (Ty::Set(g), Ty::Set(w)) => {
                let p = tr!("{}の要素", "a member of {}", crate::diag::ja_before(&place.ja); place.en);
                self.fits(g, w, at, &p)
            }
            (Ty::Dict(gk, gv), Ty::Dict(wk, wv)) => {
                let pk = tr!("{}の鍵", "a key of {}", crate::diag::ja_before(&place.ja); place.en);
                self.fits(gk, wk, at, &pk);
                let pv = tr!("{}の値", "a value of {}", crate::diag::ja_before(&place.ja); place.en);
                self.fits(gv, wv, at, &pv)
            }
            (Ty::Result(ga, ge), Ty::Result(wa, we)) => {
                let pa = tr!("{}の `ok` の値", "the value of an `ok` of {}", crate::diag::ja_before(&place.ja); place.en);
                self.fits(ga, wa, at, &pa);
                let pe = tr!("{}の `err` の値", "the value of an `err` of {}", crate::diag::ja_before(&place.ja); place.en);
                self.fits(ge, we, at, &pe)
            }
            (Ty::Num(g), Ty::Num(w)) => self.num_fits(g, w, at, place),
            (Ty::Fn(gp, gr, _), Ty::Fn(wp, wr, _)) => self.fn_fits(got, want, (gp, gr), (wp, wr), at, place),
            // a function value where none goes is said where it is put (E113)
            (Ty::Fn(..), _) => {}
            (_, Ty::Fn(..)) => {
                let (g, w) = (self.show(got), self.show(want));
                self.err("E102", at, tr!("{}は関数（{w}）です。渡しているのは {g} です", "{} is a function ({w}); this is {g}", crate::diag::ja_before(&place.ja); place.en)).notes.push(tr!(
                    "関数の型の引数には、ラムダ（`x => …`）か、関数の名前か、関数の値を渡します。",
                    "A parameter of a function type takes a lambda (`x => …`), the name of a function, or a function value."
                ));
            }
            (Ty::None, w) => {
                let w = self.show(w);
                self.err("E102", at, tr!("{}は {w} で、`none` は入りません", "{} is {w}, which `none` is not", crate::diag::ja_before(&place.ja); place.en));
            }
            (Ty::Enum(g), Ty::Enum(w)) if self.same_enum(g, w) => {}
            (Ty::Record(r), Ty::Enum(w)) if r.sum().as_ref() == Some(w) => {}
            (g, w) if g == w => {}
            (g, w) => {
                let (gs, ws) = (self.show(g), self.show(w));
                let d = self.err("E102", at, tr!("{}は {ws} です。渡しているのは {gs} です", "{} is {ws}; this is {gs}", crate::diag::ja_before(&place.ja); place.en));
                // a value of a sum where the type of one of its values goes: split it first
                if let (Ty::Enum(e), Ty::Record(r)) = (g, w)
                    && r.sum().as_ref() == Some(e)
                {
                    d.notes.push(tr!(
                        "直和型の値を値の型に渡すには、`match` で分けて、名前を付けた値（`placed p` の `p`）を渡してください。値を作る式の型は直和型です。",
                        "To give a value of a sum where the type of one of its values goes, split it with a `match` and give the value the arm names (the `p` of `placed p`); an expression that builds a value is of the sum."
                    ));
                }
            }
        }
    }

    /// A function value where a function type is wanted (DESIGN 27.2): the same number of parameters of
    /// the same types, and the same answer. E103 where the answer is the same but for its rounding.
    #[allow(clippy::too_many_arguments)]
    fn fn_fits(&mut self, got: &Ty, want: &Ty, g: (&[Ty], &Ty), w: (&[Ty], &Ty), at: Span, place: &Text) {
        let params = g.0.len() == w.0.len() && g.0.iter().zip(w.0).all(|(a, b)| same_ty(a, b));
        if params && same_ty(g.1, w.1) {
            return;
        }
        let (gs, ws) = (self.show(got), self.show(want));
        if params
            && let (Ty::Num(ga), Ty::Num(wa)) = (g.1, w.1)
            && self.num_into(ga, wa).is_ok()
            && unrounded(g.1).is_some()
        {
            let one = one_of(&wa.unit, wa.step);
            let p = crate::diag::ja_before(&place.ja);
            self.err("E103", at, tr!("丸めていない値を答える関数を、{p}に渡しています（{gs}）", "A function that answers an unrounded value goes to {}: {gs}", ; place.en)).notes.push(tr!(
                "関数の値は、答えを丸めずに渡します。ラムダの答えを `half_up(…, {one})` のように丸めてください。",
                "A function value hands its answer on as it is: round the answer of the lambda, as in `half_up(…, {one})`."
            ));
            return;
        }
        self.err("E102", at, tr!("{}は {ws} です。渡しているのは {gs} です", "{} is {ws}; this is {gs}", crate::diag::ja_before(&place.ja); place.en)).notes.push(fn_types_note());
    }

    fn num_fits(&mut self, g: &NumTy, w: &NumTy, at: Span, place: &Text) {
        let x = match self.num_into(g, w) {
            Ok(x) => x,
            Err(why) => {
                let (gs, ws) = (unit_text(&g.unit), unit_text(&w.unit));
                let ws = if w.is_rate() { "rate".to_string() } else { ws };
                self.mismatch(at, tr!("{}は {ws} です。渡しているのは {gs} です", "{} is {ws}; this is {gs}", crate::diag::ja_before(&place.ja); place.en), why);
                if g.is_rate()
                    && w.is_number()
                    && let Some(d) = self.dc.diags.last_mut()
                {
                    d.notes.push(tr!(
                        "数と率の積は率です（rulec と同じです）。数を割るなら、`down(x / 2, 1)` のように定数で割って丸めてください。",
                        "A number times a rate is a rate (as in rulec); to take a part of a number, divide it by a constant and round, as in `down(x / 2, 1)`."
                    ));
                }
                return;
            }
        };
        let Some(step) = w.step else { return };
        if on_step(x.step, step) {
            return;
        }
        let one = one_of(&w.unit, Some(step));
        let unit = if w.is_rate() { "rate".to_string() } else { unit_text(&w.unit) };
        if let Some(v) = x.konst {
            let (v, s) = (names::step_text(v, &w.unit), names::step_text(step, &w.unit));
            self.err(
                "E104",
                at,
                tr!("{}は {unit} の刻み {s} の値です。{v} は、その刻みの上にありません", "{} takes {unit} on the step {s}, and {v} is not on it", crate::diag::ja_before(&place.ja); place.en),
            );
            return;
        }
        let shown = self.text(at);
        let what = match x.step {
            None => tr!("値で割った {unit} の値", "a value of {unit} divided by a value"),
            Some(st) if w.is_rate() => {
                let st = names::step_text(st, &w.unit);
                tr!("刻み {st} の率", "a rate on the step {st}")
            }
            Some(st) => {
                let st = st.to_string();
                tr!("{unit} で端数の出る値。刻みは {st}", "a value of {unit} that comes with a fraction, in steps of {st}")
            }
        };
        let p = crate::diag::ja_before(&place.ja);
        self.err("E103", at, tr!("丸めていない値を、{p}に渡しています（{}）", "An unrounded value goes to {}: {}", what.ja; place.en, what.en)).notes.push(tr!(
            "丸め方と刻みを、`half_up({shown}, {one})` のように書いてください。丸め方は `up`、`down`、`half_up`、`half_down`、`half_even` の五つで、端数をどちらへ寄せるかは、書く人が決めます。",
            "Write the way of rounding and the step, as in `half_up({shown}, {one})`. The five ways are `up`, `down`, `half_up`, `half_down` and `half_even`: which way a fraction goes is the writer's to decide."
        ));
    }

    /// The weights of `running_totals` (DESIGN 3.6): they go into the fields `before`, `upto` and
    /// `total` of the records it answers, rounded as a field takes a value (E103), said at the answer
    /// of each branch. A number, an amount or a quantity goes as a whole number of its unit. No
    /// declaration gives a rate its step here: it goes as a count of the step the weight's type says,
    /// and is unrounded only where a division by a value has left that step unknown.
    fn weight_fits(&mut self, e: &Expr, w: &NumTy) {
        if self.types.get(e.id).is_none_or(Ty::is_error) {
            return;
        }
        match &e.kind {
            ExprKind::If(_, a, b) => {
                self.weight_fits(&a.answer, w);
                self.weight_fits(&b.answer, w);
            }
            ExprKind::Match(_, arms) => {
                for arm in arms {
                    self.weight_fits(&arm.body.answer, w);
                }
            }
            ExprKind::Block(b) => self.weight_fits(&b.answer, w),
            _ => {
                let Some(Ty::Num(got)) = self.types.get(e.id).cloned() else { return };
                self.weight_num(&got, w, e.span);
            }
        }
    }

    /// A weight of `running_totals` of the type `got` (`weight_fits`): what a function value given to it
    /// answers, at `at` (DESIGN 27.1).
    fn weight_num(&mut self, got: &NumTy, w: &NumTy, at: Span) {
        let unit = Unit { step: None, ..w.unit.clone() };
        let want = if w.is_rate() {
            if got.step.is_some() {
                return;
            }
            // the step a rate whose type says none travels in (`gen`'s `rate_step`), for the note
            NumTy { unit, step: Some(Rat::new(1, 100)), konst: None, ratio: false }
        } else {
            NumTy::of(unit)
        };
        self.num_fits(got, &want, at, &tr!("`running_totals` の重み", "the weight of `running_totals`"));
    }

    // ── Expressions ─────────────────────────────────────────────────────────────

    pub fn expr(&mut self, e: &Expr, want: Option<&Ty>) -> Ty {
        // whether this one may be a function value; the ones inside it may not, unless it says so
        let ok = std::mem::take(&mut self.allow);
        let t = self.expr_inner(e, want, ok);
        // a function value where none goes (DESIGN 27.1): said once, here, and the place it goes to is
        // told nothing more
        if t.has_fn() && !(ok && matches!(t, Ty::Fn(..))) {
            // a function of a module written where it was meant to be called
            let called = self.named.get(&e.id).map(|n| n.name.clone());
            let d = self.err("E113", e.span, tr!("関数の値を、置けないところに置いています", "A function value is put where it cannot go"));
            if let Some(w) = called {
                d.notes.push(tr!("関数 `{w}` を呼ぶなら、引数を渡して `{w}(…)` と書きます。", "To call the function `{w}`, give it its arguments: `{w}(…)`."));
            }
            d.notes.push(names::fn_value_note());
            self.named.remove(&e.id);
            return self.set(e, &Ty::Error);
        }
        self.set(e, &t)
    }

    fn expr_inner(&mut self, e: &Expr, want: Option<&Ty>, ok: bool) -> Ty {
        match &e.kind {
            ExprKind::Num(n) => self.literal(n, e.span),
            ExprKind::Date(_) => Ty::Date,
            ExprKind::Str(_) => Ty::Str,
            ExprKind::Bool(_) => Ty::Bool,
            ExprKind::None => match want {
                Some(Ty::Opt(t)) => Ty::Opt(t.clone()),
                _ => Ty::None,
            },
            ExprKind::Var(w) => self.var(e, w, want),
            ExprKind::Field(base, f, sp) => self.field(e, base, f, *sp, want),
            ExprKind::Call(callee, args) => self.call(e, callee, args, want),
            ExprKind::Record(path, base, fields) => self.record(e, path, base.as_deref(), fields, want),
            ExprKind::List(xs) => {
                let elem = match want {
                    Some(Ty::List(t)) => Some((**t).clone()),
                    _ => None,
                };
                let mut out: Option<Ty> = None;
                for x in xs {
                    let t = self.expr(x, elem.as_ref());
                    out = Some(match out {
                        None => t,
                        Some(o) => match self.join(&o, &t) {
                            Some(j) => j,
                            None => {
                                let (a, b) = (self.show(&o), self.show(&t));
                                self.err("E102", x.span, tr!("リストの要素の型がそろっていません（{a} と {b}）", "The elements of the list are not of one type ({a} and {b})"));
                                Ty::Error
                            }
                        },
                    });
                }
                match (out, elem) {
                    (None, Some(t)) => Ty::List(Box::new(t)),
                    (None, None) => Ty::Empty,
                    (Some(t), _) => Ty::List(Box::new(t)),
                }
            }
            ExprKind::Dict(pairs) => self.dict_literal(e, pairs, want),
            ExprKind::Neg(x) => {
                let t = self.expr(x, want);
                match t {
                    // a minus before a constant is a negative constant, of any unit: `-18℃` (DESIGN 3.10)
                    Ty::Num(n) if !n.unit.compares_only() || n.konst.is_some() => Ty::Num(NumTy { konst: n.konst.map(|v| Rat::zero().sub(v)), ..n }),
                    Ty::Error => Ty::Error,
                    other => {
                        let s = self.show(&other);
                        self.err("E105", e.span, tr!("{s} の値に `-` は付けられません", "A value of {s} takes no `-`"));
                        Ty::Error
                    }
                }
            }
            ExprKind::Not(x) => {
                let t = self.expr(x, Some(&Ty::Bool));
                self.condition(&t, x.span, &tr!("`not`", "`not`"));
                Ty::Bool
            }
            ExprKind::Bin(a, op, b) => self.binary(e, a, *op, b),
            ExprKind::In(x, unit, sp) => self.conversion(x, unit, *sp),
            ExprKind::If(c, t, f) => {
                let ct = self.expr(c, Some(&Ty::Bool));
                self.condition(&ct, c.span, &tr!("`if` の条件", "the condition of an `if`"));
                self.answer_ok = ok;
                let a = self.block(t, want);
                self.answer_ok = ok;
                let b = self.block(f, want);
                match self.join(&a, &b) {
                    Some(j) => j,
                    None => {
                        let (x, y) = (self.show(&a), self.show(&b));
                        self.err("E102", e.span, tr!("`if` の二つの答えの型が違います（{x} と {y}）", "The two answers of the `if` are of two types ({x} and {y})"));
                        Ty::Error
                    }
                }
            }
            ExprKind::Match(s, arms) => self.matching(e, s, arms, want, ok),
            ExprKind::Lambda(..) => match want {
                Some(Ty::Fn(ps, r, _)) => {
                    let (ps, r): (Vec<Option<Ty>>, Ty) = (ps.iter().cloned().map(Some).collect(), (**r).clone());
                    self.lambda_value(e, Some(&ps), (!r.is_error()).then_some(&r))
                }
                // where the place is said wrong already, the lambda is told nothing more
                Some(Ty::Error) => Ty::Error,
                _ => self.lambda_value(e, None, None),
            },
            ExprKind::Block(b) => {
                self.answer_ok = ok;
                self.block(b, want)
            }
        }
    }

    /// A lambda as a value (DESIGN 27.1): the types of its parameters are those written on them, else
    /// those of the function type of the place (`params`, None where a generic call has not worked one
    /// out), E112 where neither says one; its body is held to the answer the place wants (`ret`), and its
    /// type is the function type of them. Where no answer is wanted, the answer is the body's, as a
    /// declared type has it.
    fn lambda_value(&mut self, e: &Expr, params: Option<&[Option<Ty>]>, ret: Option<&Ty>) -> Ty {
        let ExprKind::Lambda(ps, body) = &e.kind else { return Ty::Error };
        let written = self.dc.module.ltypes.get(&e.id).cloned().unwrap_or_default();
        if let Some(wp) = params
            && wp.len() != ps.len()
        {
            let (n, k) = (wp.len(), ps.len());
            let en_n = ritsu_base::text::plural(n, "parameter", "parameters");
            self.err("E102", e.span, tr!("ここに置く関数は引数を {n} 個取ります。このラムダの引数は {k} 個です", "The function put here takes {en_n}, and this lambda has {k}"));
            return Ty::Error;
        }
        let mut tys = Vec::new();
        for (i, (p, sp)) in ps.iter().enumerate() {
            let w = written.get(i).cloned().flatten().map(|t| self.dc.resolve(&t));
            let t = match (w, params.and_then(|v| v.get(i)).and_then(Option::as_ref)) {
                (Some(w), Some(placed)) => {
                    if !same_ty(&w, placed) {
                        let (ws, pls) = (self.show(&w), self.show(placed));
                        self.err("E102", *sp, tr!("ラムダの引数 `{p}` の型 {ws} は、置かれた場所の関数の型の {pls} と違います", "The parameter `{p}` of the lambda is of {ws}, and the function type of the place says {pls}"))
                            .notes
                            .push(fn_types_note());
                    }
                    placed.clone()
                }
                (Some(w), None) => w,
                (None, Some(placed)) => placed.clone(),
                (None, None) => {
                    self.err("E112", *sp, tr!("ラムダの引数 `{p}` の型が決まりません", "The type of the parameter `{p}` of the lambda cannot be worked out")).notes.push(tr!(
                        "ラムダの引数の型は、置かれた場所の関数の型から決まります。型を書かない `let` のように決まらないところでは、`({p}: Line) => …` のように型を書いてください。",
                        "The types of a lambda's parameters come from the function type of the place it is put in; where none says them (a `let` with no type), write them: `({p}: Line) => …`."
                    ));
                    Ty::Error
                }
            };
            tys.push(t);
        }
        self.scopes.push(Vec::new());
        for ((p, sp), t) in ps.iter().zip(&tys) {
            self.bind(p, t.clone(), *sp, true);
        }
        let bt = self.expr(body, ret);
        let answer = match ret {
            Some(r) => {
                self.fits_at(body, r, &tr!("ラムダの答え", "the answer of the lambda"));
                r.clone()
            }
            None => match bt {
                Ty::None | Ty::Empty | Ty::EmptyDict => {
                    self.err("E102", body.span, tr!("ラムダの答えの型が決まりません", "The type of the lambda's answer cannot be told")).notes.push(tr!(
                        "`none`、`[]`、`[:]` だけでは型が分かりません。ラムダを型を書いた `let` に置くか（`let f : (Line) -> number? = …`）、関数の型の引数に渡してください。",
                        "`none`, `[]` or `[:]` alone says no type: put the lambda in a `let` with its type (`let f : (Line) -> number? = …`), or give it to a parameter of a function type."
                    ));
                    Ty::Error
                }
                t => type_arg(&t).unwrap_or_else(|| plain(t)),
            },
        };
        self.scopes.pop();
        if tys.iter().any(Ty::is_error) || answer.is_error() {
            return Ty::Error;
        }
        Ty::Fn(tys, Box::new(answer), None)
    }

    /// A condition must be a `bool`.
    fn condition(&mut self, t: &Ty, at: Span, what: &Text) {
        if !matches!(t, Ty::Bool | Ty::Error) {
            let s = self.show(t);
            self.err("E102", at, tr!("{}は bool です。これは {s} です", "{} is a bool; this is {s}", crate::diag::ja_before(&what.ja); what.en));
        }
    }

    fn literal(&mut self, n: &crate::lex::Num, at: Span) -> Ty {
        let raw = &n.raw;
        let (unit, v) = match n.unit.as_str() {
            "" => (Unit::number(), n.value),
            "%" => (Unit::rate(None), n.value.div(Rat::int(100))),
            u => match ritsu_units::table::unit(u) {
                Some((Dim::Money(_), _)) => (Unit::money(u, None).expect("a currency of the table"), n.value),
                Some((d, _)) => (Unit { dim: d, unit: u.to_string(), tax: None, step: None }, n.value),
                None => return Ty::Error,
            },
        };
        if unit.dim != Dim::Rate && !v.is_int() {
            let d = self.err("E104", at, tr!("`{raw}` は、その単位の整数ではありません", "`{raw}` is no whole number of its unit"));
            if unit.dim == Dim::Number {
                let pct = v.mul(Rat::int(100));
                d.notes.push(tr!("単位の無い数は整数です。割合なら率で `{pct}%` と書いてください。", "A number with no unit is whole; a fraction is written as the rate it is: `{pct}%`."));
            } else {
                d.notes.push(tr!(
                    "金額と量のリテラルは、書いた単位の整数で書きます。小さな単位で書いてください（`0.5USD` は `50USDc`、`1.5kg` は `1500g`）。",
                    "A literal amount or quantity is a whole number of the unit it is written in: write it in a smaller unit (`50USDc` for `0.5USD`, `1500g` for `1.5kg`)."
                ));
            }
            return Ty::Error;
        }
        let abs = if v.num < 0 { Rat::zero().sub(v) } else { v };
        Ty::Num(NumTy { unit, step: Some(abs), konst: Some(v), ratio: false })
    }

    /// A word: a local, or a value of the enum the place wants.
    fn var(&mut self, e: &Expr, w: &str, want: Option<&Ty>) -> Ty {
        let local = self.lookup(w).cloned();
        let value = match want.map(Ty::present) {
            Some(Ty::Enum(id)) => self.values(id).and_then(|(vs, _)| vs.iter().find(|v| v.is(w)).map(|v| (id.clone(), v.text.clone()))),
            _ => None,
        };
        match (local, value) {
            (Some(_), Some((id, _))) => {
                let en = self.named_by(&id.owner, &id.name);
                self.err("E110", e.span, tr!("`{w}` は、名前としても列挙 `{en}` の値としても読めます", "`{w}` reads both as a name and as a value of the enum `{en}`")).notes.push(tr!(
                    "どちらかの名前を替えてください。値なら `{en}.{w}` と書いても、名前と重ならなくなります。",
                    "Rename one of them; written `{en}.{w}`, the value no longer meets the name."
                ));
                Ty::Error
            }
            (Some(t), None) => {
                self.refer(e, Ref::Local);
                t
            }
            (None, Some((id, v))) => {
                if let Some(t) = self.value_with_fields(&id, &v, e.span) {
                    return t;
                }
                self.refer(e, Ref::EnumValue(id.clone(), v));
                Ty::Enum(id)
            }
            (None, None) => {
                if let Some((_, t)) = self.dc.uses.get(w) {
                    let what = match t {
                        Target::Rule { .. } => tr!("規則で、入力を渡して `{w}(…)` と呼びます", "a rule, called with its inputs: `{w}(…)`"),
                        Target::Dates { .. } => tr!("日付のファイルで、その日付を `{w}.<日付>(…)` と呼びます", "a dates file, whose dates are called: `{w}.<date>(…)`"),
                        Target::Book { .. } => tr!("帳簿で、振替を `{w}.<振替>.<操作>(…)` と組み立てます", "a book, whose transfers are built: `{w}.<transfer>.<operation>(…)`"),
                        Target::Module(_) => tr!("モジュールで、その関数を `{w}.<関数>(…)` と呼びます", "a module, whose functions are called: `{w}.<function>(…)`"),
                    };
                    self.err("E107", e.span, tr!("`{w}` は値ではありません。{}", "`{w}` is no value: it is {}", what.ja; what.en));
                    return Ty::Error;
                }
                // a function of the module, written as a value (DESIGN 27.1)
                if let Some(f) = self.decls.fns.iter().find(|f| f.name.is(w)).cloned() {
                    return self.named_value(e, self.dc.canon.to_path_buf(), &f, want);
                }
                if crate::kw::is_builtin(w) {
                    self.err("E109", e.span, tr!("組み込みの関数 `{w}` は、値にできません", "The builtin `{w}` is no value")).notes.push(tr!(
                        "組み込みの関数は、呼ぶか（`{w}(…)`）、ラムダで包んで渡してください（`x => {w}(x)`）。",
                        "Call a builtin (`{w}(…)`), or give it wrapped in a lambda (`x => {w}(x)`)."
                    ));
                    return Ty::Error;
                }
                // a word where the place's type is said to be wrong already (a call of a rule with no
                // outputs): which enum's value it is cannot be told, and the one mistake is said once
                if matches!(want, Some(Ty::Error)) {
                    return Ty::Error;
                }
                // a value of an enum where no type says which: `tier.gold`
                let owner = self.decls.enums.iter().find(|en| en.values.iter().any(|v| v.is(w))).map(|en| en.name.text.clone());
                let d = self.err("E101", e.span, tr!("`{w}` という名前はありません", "There is no `{w}`"));
                if let Some(o) = owner {
                    d.notes.push(tr!(
                        "列挙 `{o}` の値なら、型が決まらないところでは `{o}.{w}` と書いてください。",
                        "If it is the value of the enum `{o}`, write `{o}.{w}` where no type says which enum."
                    ));
                }
                Ty::Error
            }
        }
    }

    /// `a.b`: a value of an enum (`tier.gold`, `charges.band.low`), a function of a module a `use`
    /// reads written as a value (`pricing.price_of`, DESIGN 27.1), or a field of a record.
    fn field(&mut self, e: &Expr, base: &Expr, f: &str, at: Span, want: Option<&Ty>) -> Ty {
        if let Some(path) = e.path()
            && self.lookup(&path[0].0).is_none()
            && let Some(t) = self.enum_value(e, &path, want)
        {
            return t;
        }
        if let ExprKind::Var(u) = &base.kind
            && self.lookup(u).is_none()
            && let Some((_, Target::Module(c))) = self.dc.uses.get(u).cloned()
            && let Some(info) = c.decls.fns.iter().find(|x| x.name.is(f)).cloned()
        {
            if info.private {
                let note = names::private_note(&tr!("関数", "function"), &info.name.text, &c.module.name.text);
                self.err("E101", at, tr!("モジュール `{u}` に、関数 `{f}` はありません", "The module `{u}` has no function `{f}`")).notes.push(note);
                return Ty::Error;
            }
            return self.named_value(e, c.canon.clone(), &info, want);
        }
        let bt = self.expr(base, None);
        match &bt {
            Ty::Error => Ty::Error,
            Ty::Record(id) => match self.fields(id).and_then(|fs| fs.into_iter().find(|(n, _)| n.is(f))) {
                Some((n, slot)) => {
                    self.refer(e, Ref::Field(n.text.clone()));
                    slot.ty
                }
                None => {
                    let r = self.show(&bt);
                    let names: Vec<String> = self.fields(id).unwrap_or_default().iter().map(|(n, _)| format!("`{}`", n.text)).collect();
                    let note = fields_note(&r, &names);
                    self.err("E101", at, tr!("{r} にフィールド `{f}` はありません", "There is no field `{f}` in {r}")).notes.push(note);
                    Ty::Error
                }
            },
            Ty::Enum(id) if self.is_sum(id) => {
                let s = self.show(&bt);
                // a value that has the field, for the example of the note
                let v = self
                    .enum_info(id)
                    .and_then(|ei| ei.values.iter().zip(&ei.fields).find(|(_, fs)| fs.iter().any(|x| x.name.is(f))).or_else(|| ei.values.iter().zip(&ei.fields).find(|(_, fs)| !fs.is_empty())).map(|(v, _)| v.text.clone()))
                    .unwrap_or_default();
                self.err("E105", at, tr!("{s} の値のフィールドは、そのままでは読めません（`.{f}`）", "A field of a value of {s} is not read before a `match` names the value (`.{f}`)")).notes.push(tr!(
                    "値によってフィールドが違うので、`match` で値ごとに分け、`{v} p => p.{f}` のように、名前を付けた値のフィールドを読んでください。",
                    "Its fields depend on its value: split it with a `match`, and read the field of the value an arm names, as in `{v} p => p.{f}`."
                ));
                Ty::Error
            }
            Ty::Opt(inner) if matches!(**inner, Ty::Record(_)) => {
                let s = self.show(&bt);
                self.err("E105", at, tr!("{s} の値は無いことがあるので、そのままフィールドを読めません", "A value of {s} may be none, so its fields are not read as it is")).notes.push(tr!(
                    "`match` で `none` と `some x` に分け、`some x` の分岐で `x.{f}` を読んでください。",
                    "Split it with a `match` on `none` and `some x`, and read `x.{f}` in the arm of `some x`."
                ));
                Ty::Error
            }
            other => {
                let s = self.show(other);
                let d = self.err("E105", at, tr!("{s} の値にフィールドはありません（`.{f}`）", "A value of {s} has no fields (`.{f}`)"));
                if matches!(other, Ty::Param(_)) {
                    d.notes.push(param_note());
                }
                Ty::Error
            }
        }
    }

    /// A function of a module written as a value (DESIGN 27.1): its type, and the type arguments of a
    /// generic one from the function type of the place. Where the place wants a function type, the
    /// function is read as `(x) => f(x: x)` written here: each parameter of the place goes into the
    /// function's, and its answer into the place's, so the value is of the place's type. A function that
    /// takes a function is no value (E113).
    fn named_value(&mut self, e: &Expr, file: PathBuf, f: &names::FnInfo, want: Option<&Ty>) -> Ty {
        let fun = f.name.text.clone();
        if f.takes_fn() {
            self.err("E113", e.span, tr!("関数を取る関数 `{fun}` は、値にできません", "`{fun}` takes a function, and is no value")).notes.push(tr!(
                "関数の値は、関数を取ることも答えることもしません。`{fun}` を呼ぶラムダを書いてください（`x => {fun}(…)`）。",
                "A function value takes no function and answers none: write a lambda that calls `{fun}` (`x => {fun}(…)`)."
            ));
            return Ty::Error;
        }
        let names_ = names::tnames(&f.tparams);
        let wanted = match want {
            Some(Ty::Fn(wp, wr, _)) => Some((wp.clone(), (**wr).clone())),
            _ => None,
        };
        let inst = if f.tparams.is_empty() {
            Vec::new()
        } else {
            let mut got: Vec<Option<Ty>> = vec![None; f.tparams.len()];
            let (mut clash, mut unrounded) = (None, None);
            if let Some((wp, wr)) = &wanted {
                for (p, w) in f.params.iter().zip(wp) {
                    self.unify_ty(&p.slot.ty, w, &names_, &mut got, &mut clash, &mut unrounded, e.span);
                }
                if !wr.is_error() {
                    let mut none = None;
                    self.unify_ty(&f.ret.ty, wr, &names_, &mut got, &mut none, &mut unrounded, e.span);
                }
            }
            match self.settle(e, &tr!("関数 `{fun}`", "`{fun}`"), &f.tparams, got, clash, unrounded) {
                Some(t) => t,
                None => return Ty::Error,
            }
        };
        let params: Vec<Ty> = f.params.iter().map(|p| subst(&p.slot.ty, &names_, &inst)).collect();
        let ret = subst(&f.ret.ty, &names_, &inst);
        let (lparams, lret) = match &wanted {
            Some((wp, wr)) if wp.len() == params.len() => {
                for (i, (w, p)) in wp.iter().zip(&params).enumerate() {
                    let pn = &f.params[i].name.text;
                    self.fits(w, p, e.span, &tr!("関数 `{fun}` の引数 `{pn}`", "the parameter `{pn}` of `{fun}`"));
                }
                if !wr.is_error() {
                    self.fits(&ret, wr, e.span, &tr!("ここに置く関数の答え", "the answer of the function put here"));
                }
                (wp.clone(), if wr.is_error() { ret.clone() } else { wr.clone() })
            }
            _ => (params.clone(), ret.clone()),
        };
        let pnames = f.params.iter().map(|p| p.name.text.clone()).collect();
        self.named.insert(e.id, NamedFn { file, name: fun, pnames, inst, params: lparams.clone(), ret: ret.clone(), inline: false });
        Ty::Fn(lparams, Box::new(lret), None)
    }

    /// A value of an enum named with its enum: `tier.gold`, `charges.band.low`, `pricing.tier.gold`.
    fn enum_value(&mut self, e: &Expr, path: &[(String, Span)], want: Option<&Ty>) -> Option<Ty> {
        let words: Vec<&str> = path.iter().map(|(w, _)| w.as_str()).collect();
        let (id, v) = match words.as_slice() {
            [en, v] => {
                let x = self.decls.enums.iter().find(|x| x.name.is(en))?;
                (EnumId::plain(Owner::Module(self.dc.canon.to_path_buf()), x.name.text.clone()), *v)
            }
            [u, en, v] => match self.dc.uses.get(*u).map(|(_, t)| t) {
                Some(Target::Rule { canon, facts, .. }) => {
                    let x = facts.enums.iter().find(|x| names::rule_enum_is(x, en))?;
                    (EnumId::plain(Owner::Rule(canon.clone()), x.name.clone()), *v)
                }
                Some(Target::Module(c)) => {
                    let x = c.decls.enums.iter().find(|x| x.name.is(en))?;
                    // a `private` enum of another module (DESIGN 28.2)
                    if x.private {
                        let note = names::private_note(&tr!("列挙", "enum"), &x.name.text, &c.module.name.text);
                        self.err("E101", path[1].1, tr!("モジュール `{u}` に、列挙 `{en}` はありません", "The module `{u}` has no enum `{en}`")).notes.push(note);
                        return Some(Ty::Error);
                    }
                    (EnumId::plain(Owner::Module(c.canon.clone()), x.name.text.clone()), *v)
                }
                _ => return None,
            },
            _ => return None,
        };
        let (values, _) = self.values(&id)?;
        match values.iter().find(|x| x.is(v)) {
            Some(x) => {
                if let Some(t) = self.value_with_fields(&id, &x.text, e.span) {
                    return Some(t);
                }
                // a value of a generic enum takes the type arguments the place says (DESIGN 26.2)
                let id = match self.enum_info(&id).map(|i| i.tparams.len()).unwrap_or(0) {
                    0 => id,
                    _ => match want.map(Ty::present) {
                        Some(Ty::Enum(w)) if w.same_decl(&id) => w.clone(),
                        _ => {
                            let en = self.named_by(&id.owner, &id.name);
                            self.err("E112", e.span, tr!("列挙 `{en}` の型の引数が決まりません", "The type arguments of the enum `{en}` cannot be worked out")).notes.push(tr!(
                                "フィールドの無い値からは、型の引数が決まりません。型の決まるところ（型を書いた `let`、関数の答え、引数）に置いてください（`let x : {en}[number] = {en}.{v}`）。",
                                "A value with no fields says no type argument: put it where the type is said (a `let` with its type, a function's answer, an argument): `let x : {en}[number] = {en}.{v}`."
                            ));
                            return Some(Ty::Error);
                        }
                    },
                };
                self.refer(e, Ref::EnumValue(id.clone(), x.text.clone()));
                Some(Ty::Enum(id))
            }
            None => {
                let en = self.named_by(&id.owner, &id.name);
                let all = values.iter().map(|x| format!("`{}`", x.text)).collect::<Vec<_>>().join(", ");
                let at = path[path.len() - 1].1;
                self.err("E101", at, tr!("列挙 `{en}` に値 `{v}` はありません", "The enum `{en}` has no value `{v}`")).notes.push(tr!("値は {all} です。", "Its values are {all}."));
                Some(Ty::Error)
            }
        }
    }

    // ── Joining two answers ─────────────────────────────────────────────────────

    /// The type of two answers that go to one place (the two of an `if`, the arms of a `match`, the
    /// elements of a list); None when they do not go together.
    fn join(&self, a: &Ty, b: &Ty) -> Option<Ty> {
        match (a, b) {
            (Ty::Error, x) | (x, Ty::Error) => Some(x.clone()),
            (Ty::None, Ty::None) => Some(Ty::None),
            (Ty::None, Ty::Opt(x)) | (Ty::Opt(x), Ty::None) => Some(Ty::Opt(x.clone())),
            (Ty::None, x) | (x, Ty::None) => Some(Ty::Opt(Box::new(x.clone()))),
            (Ty::Empty, Ty::Empty) => Some(Ty::Empty),
            (Ty::Empty, Ty::List(x)) | (Ty::List(x), Ty::Empty) => Some(Ty::List(x.clone())),
            (Ty::EmptyDict, Ty::EmptyDict) => Some(Ty::EmptyDict),
            (Ty::EmptyDict, Ty::Dict(k, v)) | (Ty::Dict(k, v), Ty::EmptyDict) => Some(Ty::Dict(k.clone(), v.clone())),
            (Ty::Opt(x), Ty::Opt(y)) => Some(Ty::Opt(Box::new(self.join(x, y)?))),
            (Ty::Opt(x), y) | (y, Ty::Opt(x)) => Some(Ty::Opt(Box::new(self.join(x, y)?))),
            (Ty::List(x), Ty::List(y)) => Some(Ty::List(Box::new(self.join(x, y)?))),
            (Ty::Set(x), Ty::Set(y)) => Some(Ty::Set(Box::new(self.join(x, y)?))),
            (Ty::Dict(k, v), Ty::Dict(l, w)) => Some(Ty::Dict(Box::new(self.join(k, l)?), Box::new(self.join(v, w)?))),
            (Ty::Result(k, v), Ty::Result(l, w)) => Some(Ty::Result(Box::new(self.join(k, l)?), Box::new(self.join(v, w)?))),
            (Ty::Num(x), Ty::Num(y)) => {
                let (x, y) = self.unify(x, y).ok()?;
                Some(Ty::Num(NumTy { unit: x.unit.clone(), step: share(x.step, y.step), konst: None, ratio: x.ratio && y.ratio }))
            }
            (Ty::Enum(x), Ty::Enum(y)) if self.same_enum(x, y) => Some(a.clone()),
            // two function values of one type (DESIGN 27.2): the effect written on both, else none
            (Ty::Fn(ap, ar, ae), Ty::Fn(bp, br, be)) if ap.len() == bp.len() && ap.iter().zip(bp).all(|(x, y)| same_ty(x, y)) && same_ty(ar, br) => {
                Some(Ty::Fn(ap.clone(), ar.clone(), if ae == be { ae.clone() } else { None }))
            }
            (x, y) if x == y => Some(x.clone()),
            // two values of one sum, of the types of its values or of the sum: the sum (DESIGN 23.2)
            (Ty::Record(r), Ty::Enum(y)) | (Ty::Enum(y), Ty::Record(r)) if r.sum().as_ref() == Some(y) => Some(Ty::Enum(y.clone())),
            (Ty::Record(r), Ty::Record(q)) if r.sum().is_some() && r.sum() == q.sum() => r.sum().map(Ty::Enum),
            _ => None,
        }
    }

    // ── Operators ───────────────────────────────────────────────────────────────

    fn binary(&mut self, e: &Expr, a: &Expr, op: BinOp, b: &Expr) -> Ty {
        let o = op.shown();
        if matches!(op, BinOp::And | BinOp::Or) {
            let (ta, tb) = (self.expr(a, Some(&Ty::Bool)), self.expr(b, Some(&Ty::Bool)));
            self.condition(&ta, a.span, &tr!("`{o}` の左", "the left of `{o}`"));
            self.condition(&tb, b.span, &tr!("`{o}` の右", "the right of `{o}`"));
            return Ty::Bool;
        }
        // a bare word on one side is read in the type of the other (an enum's value)
        let bare = |x: &Expr| matches!(&x.kind, ExprKind::Var(w) if self.lookup(w).is_none());
        let (ba, bb) = (bare(a), bare(b));
        let (ta, tb) = if ba && !bb {
            let tb = self.expr(b, None);
            (self.expr(a, Some(&tb)), tb)
        } else {
            let said = self.dc.diags.iter().filter(|d| d.code == "E113").count();
            let ta = self.expr(a, None);
            // a function value on the left is said there (E113), and one on the right is not said again
            if self.dc.diags.iter().filter(|d| d.code == "E113").count() > said {
                self.allow = true;
            }
            (ta.clone(), self.expr(b, Some(&ta)))
        };
        if ta.is_error() || tb.is_error() {
            return if op.is_comparison() { Ty::Bool } else { Ty::Error };
        }
        if op.is_comparison() {
            return self.comparison(e, &ta, op, &tb);
        }
        match (&ta, &tb) {
            (Ty::Date, Ty::Date) if op == BinOp::Sub => Ty::Num(NumTy::number()),
            (Ty::Date, _) | (_, Ty::Date) => {
                self.err("E105", e.span, tr!("日付に `{o}` は使えません", "A date takes no `{o}`")).notes.push(tr!(
                    "kaname が日付でできる計算は、二つの日付の差（`b - a` は日数の number）だけです。日付に日数や月を足すことは koyomi に書き、その日付の関数を呼んでください（koyomi は範囲のすべての日で計算を確かめます）。",
                    "The one computation kaname does with dates is the difference of two (`b - a`, the days as a number). Adding days or months to a date is written in koyomi, whose date is then called: koyomi checks the computation on every day of its range."
                ));
                Ty::Error
            }
            (Ty::Num(x), Ty::Num(y)) => self.arithmetic(e, x, op, y),
            (Ty::Str, Ty::Str) if op == BinOp::Add => {
                self.err("E105", e.span, tr!("文字列に `+` は使えません。つなぐには `concat(a, b)` と書いてください", "A string takes no `+`; join strings with `concat(a, b)`"));
                Ty::Error
            }
            _ => {
                let (x, y) = (self.show(&ta), self.show(&tb));
                let d = self.err("E105", e.span, tr!("{x} と {y} のあいだに `{o}` は使えません", "There is no `{o}` between {x} and {y}"));
                if matches!(ta, Ty::Param(_)) || matches!(tb, Ty::Param(_)) {
                    d.notes.push(param_note());
                }
                Ty::Error
            }
        }
    }

    fn comparison(&mut self, e: &Expr, ta: &Ty, op: BinOp, tb: &Ty) -> Ty {
        let o = op.shown();
        let order = !matches!(op, BinOp::Eq | BinOp::Ne);
        match (ta, tb) {
            (Ty::Num(x), Ty::Num(y)) => {
                if let Err(why) = self.unify(x, y) {
                    let (xs, ys) = (unit_text(&x.unit), unit_text(&y.unit));
                    self.mismatch(e.span, tr!("{xs} と {ys} は比べられません", "`{o}` does not compare {xs} and {ys}"), why);
                }
            }
            (Ty::Date, Ty::Date) => {}
            (Ty::Enum(x), Ty::Enum(y)) if self.same_enum(x, y) => {
                if order && !self.values(x).is_some_and(|(_, ordered)| ordered) {
                    let en = self.named_by(&x.owner, &x.name);
                    self.err("E105", e.span, tr!("列挙 `{en}` の値には順序が無いので、`{o}` で比べられません", "The values of the enum `{en}` have no order, so `{o}` does not compare them"))
                        .notes
                        .push(tr!(
                            "順序のある列挙は、値を `<` で区切って宣言します（`enum tier = basic < silver < gold`）。",
                            "An ordered enum divides its values with `<` (`enum tier = basic < silver < gold`)."
                        ));
                }
            }
            (Ty::Bool, Ty::Bool) | (Ty::Str, Ty::Str) if !order => {}
            // a value of a type parameter: `=` and `!=`, and the order of keys where it is held to keys
            // (DESIGN 26.2)
            (Ty::Param(x), Ty::Param(y)) if x == y => {
                if order && !self.tparams.iter().any(|p| p.name.text == *x && p.key) {
                    let x = x.clone();
                    self.err("E114", e.span, tr!("型の引数 `{x}` の値を `{o}` で比べるには、`key` が要ります", "A value of the type parameter `{x}` is compared with `{o}` only where it is held to keys")).notes.push(names::param_key_note(&x));
                }
            }
            // strings in the order of their code points (DESIGN 25.1)
            (Ty::Str, Ty::Str) => {}
            // `=` and `!=` compare two values of one type, whatever the type (DESIGN 23.5): a record and
            // a value of a sum field by field, a list element by element, a value that may be none
            _ if !order => {
                if self.join(ta, tb).is_none() {
                    let (x, y) = (self.show(ta), self.show(tb));
                    self.err("E102", e.span, tr!("{x} と {y} は比べられません", "{x} and {y} cannot be compared"));
                }
            }
            (Ty::Bool, Ty::Bool) => {
                let s = self.show(ta);
                self.err("E105", e.span, tr!("{s} の値は、`=` と `!=` でだけ比べられます", "A value of {s} compares with `=` and `!=` only"));
            }
            (Ty::Opt(_), _) | (_, Ty::Opt(_)) | (Ty::None, _) | (_, Ty::None) => {
                self.err("E105", e.span, tr!("値が無いことがある値は、`{o}` で比べられません", "A value that may be none is not compared with `{o}`"))
                    .notes
                    .push(tr!("`match` で `none` と `some x` に分けてから比べてください（rulec と同じです）。", "Split it with a `match` on `none` and `some x`, then compare (as in rulec)."));
            }
            _ => {
                let (x, y) = (self.show(ta), self.show(tb));
                if ta == tb {
                    self.err("E105", e.span, tr!("{x} の値は `{o}` で比べられません", "Values of {x} are not compared with `{o}`"));
                } else {
                    self.err("E102", e.span, tr!("{x} と {y} は比べられません", "{x} and {y} cannot be compared"));
                }
            }
        }
        Ty::Bool
    }

    fn arithmetic(&mut self, e: &Expr, x: &NumTy, op: BinOp, y: &NumTy) -> Ty {
        let o = op.shown();
        if x.unit.compares_only() || y.unit.compares_only() {
            let u = if x.unit.compares_only() { unit_text(&x.unit) } else { unit_text(&y.unit) };
            self.err("E105", e.span, tr!("{u} は比べと範囲だけの量で、`{o}` は使えません", "`{o}` is not used on {u}, a quantity of comparison and range only")).notes.push(tr!(
                "温度は零点のずれた尺度で、デシベルは対数なので、足し算や掛け算に意味がありません（rulec と同じです）。",
                "A temperature is a scale with a displaced zero and a decibel a logarithm, so sums and products of them mean nothing (as in rulec)."
            ));
            return Ty::Error;
        }
        match op {
            BinOp::Add | BinOp::Sub => match self.unify(x, y) {
                Ok((a, b)) => {
                    let konst = match (a.konst, b.konst) {
                        (Some(p), Some(q)) => Some(if op == BinOp::Add { p.add(q) } else { p.sub(q) }),
                        _ => None,
                    };
                    Ty::Num(NumTy { unit: if a.konst.is_some() { b.unit.clone() } else { a.unit.clone() }, step: share(a.step, b.step), konst, ratio: a.ratio && b.ratio })
                }
                Err(why) => {
                    let (xs, ys) = (unit_text(&x.unit), unit_text(&y.unit));
                    self.mismatch(e.span, tr!("{xs} と {ys} は `{o}` でつなげません", "`{o}` does not join {xs} and {ys}"), why);
                    Ty::Error
                }
            },
            BinOp::Mul => {
                let konst = match (x.konst, y.konst) {
                    (Some(p), Some(q)) => Some(p.mul(q)),
                    _ => None,
                };
                let step = times(x.step, y.step);
                let unit = match (x.has_unit(), y.has_unit()) {
                    (true, true) => {
                        let (xs, ys) = (unit_text(&x.unit), unit_text(&y.unit));
                        self.err("E105", e.span, tr!("{xs} と {ys} は掛けられません", "`*` does not multiply {xs} by {ys}")).notes.push(tr!(
                            "kaname は rulec と同じく、次元を掛け合わせません（金額 × 金額、重さ × 長さ）。積そのものが要るなら、入力として受け取るか表で引いてください。",
                            "Like rulec, kaname multiplies no two dimensions (an amount by an amount, a weight by a length): where the product itself is wanted, take it as an input or look it up in a table."
                        ));
                        return Ty::Error;
                    }
                    (true, false) => x.unit.clone(),
                    (false, true) => y.unit.clone(),
                    (false, false) if x.is_rate() || y.is_rate() => Unit::rate(None),
                    (false, false) => Unit::number(),
                };
                let ratio = !x.has_unit() && !y.has_unit() && !x.is_rate() && !y.is_rate() && (x.ratio || y.ratio);
                Ty::Num(NumTy { unit, step, konst, ratio })
            }
            _ => self.division(e, x, y),
        }
    }

    /// `a / b` (DESIGN 3.2): an amount over another of its unit is a ratio; anything over a number
    /// or a rate keeps its type. A divisor that is a value makes the quotient's step unknown.
    fn division(&mut self, e: &Expr, x: &NumTy, y: &NumTy) -> Ty {
        let by_const = |s: Option<Rat>, k: Option<Rat>| -> Option<Rat> {
            match (s, k) {
                (Some(s), Some(k)) if k.num != 0 => {
                    let k = if k.num < 0 { Rat::zero().sub(k) } else { k };
                    s.checked_div(k)
                }
                _ => None,
            }
        };
        let konst = match (x.konst, y.konst) {
            (Some(p), Some(q)) if q.num != 0 => Some(p.div(q)),
            _ => None,
        };
        match (x.has_unit(), y.has_unit()) {
            (true, true) => {
                if x.unit.dim != y.unit.dim {
                    let (xs, ys) = (unit_text(&x.unit), unit_text(&y.unit));
                    self.err("E105", e.span, tr!("{xs} を {ys} で割れません", "`/` does not divide {xs} by {ys}")).notes.push(tr!(
                        "割れるのは、同じ単位の量どうし（答えは比）と、量を数か率で割ること（答えは量）です。",
                        "What divides is two values of one unit (a ratio), and a value by a number or a rate (a value of its unit)."
                    ));
                    return Ty::Error;
                }
                match self.unify(x, y) {
                    Ok((a, b)) => Ty::Num(NumTy { unit: Unit::number(), step: by_const(a.step, b.konst), konst, ratio: true }),
                    Err(why) => {
                        let (xs, ys) = (unit_text(&x.unit), unit_text(&y.unit));
                        self.mismatch(e.span, tr!("{xs} を {ys} で割れません", "`/` does not divide {xs} by {ys}"), why);
                        Ty::Error
                    }
                }
            }
            (false, true) => {
                let (xs, ys) = (unit_text(&x.unit), unit_text(&y.unit));
                self.err("E105", e.span, tr!("{xs} を {ys} で割れません", "`/` does not divide {xs} by {ys}"))
                    .notes
                    .push(tr!("数や率を量で割った値は、どの単位の値でもありません。", "A number or a rate over a quantity is a value of no unit."));
                Ty::Error
            }
            _ => Ty::Num(NumTy { unit: x.unit.clone(), step: by_const(x.step, y.konst), konst, ratio: x.ratio }),
        }
    }

    /// `x in USDc` (DESIGN 3.2): the value counted in another unit of its dimension.
    fn conversion(&mut self, x: &Expr, unit: &str, at: Span) -> Ty {
        let t = self.expr(x, None);
        let n = match &t {
            Ty::Error => return Ty::Error,
            Ty::Num(n) if n.has_unit() => n.clone(),
            other => {
                let s = self.show(other);
                self.err("E104", at, tr!("{s} の値には単位が無いので、`in` で換算できません", "A value of {s} has no unit, so `in` converts nothing"));
                return Ty::Error;
            }
        };
        let to = match &n.unit.dim {
            Dim::Money(_) => Unit::money(unit, n.unit.tax),
            d => Unit::quantity(d.word(), unit),
        };
        let from = unit_text(&n.unit);
        let Some(to) = to.filter(|u| u.dim == n.unit.dim) else {
            let msg = match ritsu_units::table::unit(unit) {
                None => tr!("`{unit}` は単位の表にありません", "`{unit}` is not a unit of the table"),
                Some((Dim::Money(_), _)) if matches!(n.unit.dim, Dim::Money(_)) => {
                    tr!("{from} を `{unit}` に換算できません。違う通貨のあいだに為替の率はありません", "`in {unit}` does not convert {from}: there is no exchange rate between two currencies")
                }
                Some(_) => tr!("{from} を `{unit}` に換算できません。次元が違います", "`in {unit}` does not convert {from}: it is a unit of another dimension"),
            };
            self.err("E104", at, msg);
            return Ty::Error;
        };
        let f = factor(&n.unit, &to).unwrap_or(Rat::int(1));
        // a constant is read with the zero of its unit too: `-20℃ in ℉` is -4℉ (DESIGN 3.2)
        let konst = n.konst.and_then(|v| crate::ty::convert(v, &n.unit, &to));
        // and the values are on a step that holds the shift between the zeros: `t in ℉` of a whole
        // number of ℃ is on 1/5℉, not on 9/5℉
        let step = step_in(n.step.and_then(|s| s.checked_mul(f)), konst, &n.unit, &to);
        Ty::Num(NumTy { unit: Unit { step: None, ..to }, step, konst, ratio: false })
    }

    // ── match ───────────────────────────────────────────────────────────────────

    /// A `match` (DESIGN 23.3): what it splits (an enum, a sum, a bool, a value that may be none, or a
    /// number, a string or a date by literals), each arm's values and the name it gives a value with
    /// fields, `else`, and whether every value has an arm (E106).
    fn matching(&mut self, e: &Expr, s: &Expr, arms: &[Arm], want: Option<&Ty>, ok: bool) -> Ty {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Split {
            Error,
            Enum,
            Bool,
            Opt,
            Lit,
            /// `ok x` and `err e` (DESIGN 26.3)
            Result,
        }
        let st = self.expr(s, None);
        let mut sum = false;
        let (all, kind): (Vec<String>, Split) = match &st {
            Ty::Error => (vec![], Split::Error),
            Ty::Enum(id) => match self.values(id) {
                Some((vs, _)) => {
                    sum = self.is_sum(id);
                    (vs.iter().map(|v| v.text.clone()).collect(), Split::Enum)
                }
                None => (vec![], Split::Error),
            },
            Ty::Bool => (vec!["true".into(), "false".into()], Split::Bool),
            Ty::Opt(_) | Ty::None => (vec!["none".into(), "some".into()], Split::Opt),
            Ty::Result(..) => (vec!["ok".into(), "err".into()], Split::Result),
            Ty::Num(_) | Ty::Str | Ty::Date => (vec![], Split::Lit),
            other => {
                let t = self.show(other);
                let d = self.err("E105", s.span, tr!("{t} の値は `match` で分けられません", "A value of {t} is not split with `match`"));
                d.notes.push(tr!(
                    "`match` が分けるのは、列挙、bool、値が無いことがある値（`none` と `some x`）、`result`（`ok x` と `err e`）と、リテラルで分ける数、お金、量、日付、文字列です。",
                    "`match` splits an enum, a bool, a value that may be none (`none` and `some x`), a `result` (`ok x` and `err e`), and numbers, amounts, quantities, dates and strings by literals."
                ));
                if matches!(other, Ty::Param(_)) {
                    d.notes.push(param_note());
                }
                (vec![], Split::Error)
            }
        };
        let mut covered: BTreeSet<String> = BTreeSet::new();
        let mut lits: Vec<String> = Vec::new();
        // an arm said to be no value of the type split: whether `else` is missing is not said too
        let mut wrong = false;
        let mut rest: Option<Span> = None;
        let mut after_rest = false;
        let mut out: Option<Ty> = None;
        for arm in arms {
            // an arm after `else` is said once; what it takes is not said again
            let past_rest = rest.is_some();
            if past_rest && !after_rest {
                after_rest = true;
                self.err("E106", arm.span, tr!("`else` のあとの分岐には、値が来ません", "No value comes to an arm after `else`")).notes.push(tr!(
                    "`else` は、上の分岐が受けない値を全部受けます。`else` を最後の分岐にしてください。",
                    "`else` takes every value the arms above leave: make it the last arm."
                ));
            }
            let mut bound: Option<(String, Span, Ty)> = None;
            for p in &arm.patterns {
                let key: Option<String> = match (p, kind) {
                    (Pattern::Else(sp), _) => {
                        if arm.patterns.len() > 1 {
                            self.err("E106", *sp, tr!("`else` は、ほかの値と一つの分岐に並べられません", "`else` cannot share an arm with another value"))
                                .notes
                                .push(tr!("`else` は残りの値を全部受けるので、その分岐を一つにしてください。", "`else` takes every value left, so it takes an arm of its own."));
                        }
                        let left = match kind {
                            Split::Lit | Split::Error => true,
                            _ => all.iter().any(|v| !covered.contains(v)),
                        };
                        if !left {
                            self.err("E106", *sp, tr!("`else` が受ける値がありません。上の分岐が、どの値も受けています", "`else` takes no value: the arms above take every value"))
                                .notes
                                .push(tr!("`else` の分岐を消してください。", "Take the `else` arm out."));
                        }
                        covered.extend(all.iter().cloned());
                        if rest.is_none() {
                            rest = Some(*sp);
                        }
                        None
                    }
                    (Pattern::Lit(x), Split::Lit) => {
                        let lt = self.expr(x, Some(&st));
                        if let Some(k) = self.literal_key(&lt, &st, x) {
                            if lits.contains(&k) {
                                let shown = self.text(x.span);
                                self.err("E106", x.span, tr!("`{shown}` は、上の分岐がもう受けています", "An arm above takes `{shown}` already"))
                                    .notes
                                    .push(tr!("どの値も、一つの分岐だけが受けます。この値を消すか、上の分岐から外してください。", "Each value goes to one arm: take it out of this arm or of the one above."));
                            } else {
                                lits.push(k);
                            }
                        }
                        None
                    }
                    (_, Split::Error) => None,
                    (Pattern::None(_), Split::Opt) => Some("none".into()),
                    (Pattern::Some(x, sp), Split::Opt) => {
                        let inner = match &st {
                            Ty::Opt(t) => (**t).clone(),
                            _ => Ty::Error,
                        };
                        if arm.patterns.len() > 1 {
                            self.err("E106", *sp, tr!("`some {x}` は、ほかの値と一つの分岐に並べられません", "`some {x}` cannot share an arm with another value"))
                                .notes
                                .push(tr!("`some {x}` は値に名前を付けるので、その分岐を一つにしてください。", "`some {x}` names the value, so it takes an arm of its own."));
                        }
                        bound = Some((x.clone(), *sp, inner));
                        Some("some".into())
                    }
                    (Pattern::Bool(b, _), Split::Bool) => Some(b.to_string()),
                    // `ok x` and `err e`: the value an `ok` or an `err` holds, named (DESIGN 26.3)
                    (Pattern::Value(path, bind, sp), Split::Result) => {
                        let (a, e) = match &st {
                            Ty::Result(a, e) => ((**a).clone(), (**e).clone()),
                            _ => (Ty::Error, Ty::Error),
                        };
                        match path.as_slice() {
                            [(w, _)] if w == "ok" || w == "err" => {
                                if let Some((b, bsp)) = bind {
                                    if arm.patterns.len() > 1 {
                                        self.err("E106", *bsp, tr!("`{w} {b}` は、ほかの値と一つの分岐に並べられません", "`{w} {b}` cannot share an arm with another value"))
                                            .notes
                                            .push(tr!("`{w} {b}` は値に名前を付けるので、その分岐を一つにしてください。", "`{w} {b}` names the value, so it takes an arm of its own."));
                                    } else {
                                        bound = Some((b.clone(), *bsp, plain(if w == "ok" { a } else { e })));
                                    }
                                }
                                Some(w.clone())
                            }
                            _ => {
                                let shown = path.iter().map(|(w, _)| w.as_str()).collect::<Vec<_>>().join(".");
                                let t = self.show(&st);
                                self.err("E101", *sp, tr!("`{shown}` は {t} の値ではありません", "`{shown}` is not a value of {t}")).notes.push(tr!(
                                    "`result` の値は `ok x`（値があるとき）と `err e`（値が無い理由）で分けます。",
                                    "A `result` is split into `ok x` (a value) and `err e` (why there is none)."
                                ));
                                None
                            }
                        }
                    }
                    (Pattern::Value(path, bind, sp), Split::Enum) => {
                        let Ty::Enum(id) = &st else { unreachable!() };
                        let v = &path[path.len() - 1].0;
                        // `gold`, `tier.gold`, `charges.band.low`: the words before the value name the enum
                        let en_ok = match path.len() {
                            1 => true,
                            n => {
                                let word = &path[n - 2].0;
                                let file_ok = n == 2 || (n == 3 && self.dc.uses.get(&path[0].0).is_some_and(|(_, t)| *t.canon() == *owner_file(&id.owner)));
                                file_ok && self.enum_word_is(id, word)
                            }
                        };
                        match self.values(id).and_then(|(vs, _)| vs.into_iter().find(|x| x.is(v))) {
                            Some(x) if en_ok => {
                                // the name the arm gives a value with fields: `placed p`
                                if let Some((b, bsp)) = bind {
                                    let en = self.named_by(&id.owner, &id.name);
                                    if !sum {
                                        self.err("E106", *bsp, tr!("列挙 `{en}` の値にはフィールドが無いので、名前を付けられません", "A value of the enum `{en}` has no fields, and takes no name"))
                                            .notes
                                            .push(tr!("名前を付けられるのは、フィールドを持つ値のある列挙の値です（`placed p`）。", "A name is given to a value of an enum whose values have fields (`placed p`)."));
                                    } else if arm.patterns.len() > 1 {
                                        self.err("E106", *bsp, tr!("`{v} {b}` は、ほかの値と一つの分岐に並べられません", "`{v} {b}` cannot share an arm with another value"))
                                            .notes
                                            .push(tr!("`{v} {b}` は値に名前を付けるので、その分岐を一つにしてください。", "`{v} {b}` names the value, so it takes an arm of its own."));
                                    } else if let Some(r) = Self::variant_of(id, &x.text) {
                                        bound = Some((b.clone(), *bsp, Ty::Record(r)));
                                    }
                                }
                                Some(x.text)
                            }
                            _ => {
                                let en = self.named_by(&id.owner, &id.name);
                                let shown = path.iter().map(|(w, _)| w.as_str()).collect::<Vec<_>>().join(".");
                                self.err("E101", *sp, tr!("`{shown}` は列挙 `{en}` の値ではありません", "`{shown}` is not a value of the enum `{en}`"));
                                None
                            }
                        }
                    }
                    (p, _) => {
                        if let Pattern::Lit(x) = p {
                            self.expr(x, None);
                        }
                        let t = self.show(&st);
                        self.err("E102", p.span(), tr!("この分岐は {t} の値ではありません", "This arm is not a value of {t}"));
                        wrong = true;
                        None
                    }
                };
                if let Some(k) = key
                    && !covered.insert(k.clone())
                    && !past_rest
                {
                    self.err("E106", p.span(), tr!("`{k}` は、上の分岐がもう受けています", "An arm above takes `{k}` already"))
                        .notes
                        .push(tr!("どの値も、一つの分岐だけが受けます。この値を消すか、上の分岐から外してください。", "Each value goes to one arm: take it out of this arm or of the one above."));
                }
            }
            self.scopes.push(Vec::new());
            if let Some((x, sp, t)) = bound {
                self.bind(&x, t, sp, true);
            }
            self.answer_ok = ok;
            let t = self.block(&arm.body, want);
            self.scopes.pop();
            out = Some(match out {
                None => t,
                Some(o) => match self.join(&o, &t) {
                    Some(j) => j,
                    None => {
                        let (a, b) = (self.show(&o), self.show(&t));
                        self.err("E102", arm.span, tr!("`match` の分岐の答えの型が違います（{a} と {b}）", "The arms of the `match` answer two types ({a} and {b})"));
                        Ty::Error
                    }
                },
            });
        }
        match kind {
            Split::Lit if rest.is_none() && !wrong => {
                self.err("E106", e.span, tr!("リテラルで分ける `match` には、`else` が要ります", "A `match` that splits by literals needs `else`")).notes.push(tr!(
                    "数、お金、量、日付、文字列の値は数えきれないので、リテラルの分岐が受けない値を `else =>` で受けてください。範囲で分けるなら `if` で書けます。",
                    "Numbers, amounts, quantities, dates and strings have too many values to list: take the ones no literal takes with `else =>`. A split by ranges is written with `if`."
                ));
            }
            Split::Enum | Split::Bool | Split::Opt | Split::Result => {
                let missing: Vec<String> = all
                    .iter()
                    .filter(|v| !covered.contains(*v))
                    .map(|v| match v.as_str() {
                        "some" => "some x".into(),
                        "ok" if kind == Split::Result => "ok x".into(),
                        "err" if kind == Split::Result => "err e".into(),
                        _ => v.clone(),
                    })
                    .collect();
                if !missing.is_empty() {
                    let m = missing.iter().map(|v| format!("`{v}`")).collect::<Vec<_>>().join(", ");
                    self.err("E106", e.span, tr!("`match` が受けない値があります: {m}", "The `match` leaves values out: {m}"))
                        .notes
                        .push(tr!("どの値にも答えるよう、受けない値の分岐を足してください。", "Add an arm for each value left out, so that every value has an answer."));
                }
            }
            _ => {}
        }
        out.unwrap_or(Ty::Error)
    }

    /// The value a literal of an arm is, read in the type the `match` splits, written as a key that
    /// tells two arms apart; None where it is said to be no value of it (E102, E104).
    fn literal_key(&mut self, lt: &Ty, st: &Ty, x: &Expr) -> Option<String> {
        match (lt, st) {
            (Ty::Error, _) | (_, Ty::Error) => None,
            (Ty::Num(l), Ty::Num(sn)) => match self.unify(l, &NumTy { konst: None, ..sn.clone() }) {
                Ok((a, _)) => {
                    let v = a.konst?;
                    if !crate::num::fits(v) {
                        let shown = self.text(x.span);
                        self.err("E104", x.span, tr!("`{shown}` は int64 に収まりません", "`{shown}` does not fit in int64"));
                        return None;
                    }
                    Some(v.to_string())
                }
                Err(why) => {
                    let (xs, ys) = (unit_text(&l.unit), unit_text(&sn.unit));
                    self.mismatch(x.span, tr!("この分岐の {xs} は、分ける値の {ys} と比べられません", "This arm's {xs} is not compared with the {ys} the `match` splits"), why);
                    None
                }
            },
            (Ty::Date, Ty::Date) => match &x.kind {
                ExprKind::Date(d) => Some(d.to_string()),
                _ => None,
            },
            (Ty::Str, Ty::Str) => match &x.kind {
                ExprKind::Str(t) => Some(format!("{t:?}")),
                _ => None,
            },
            _ => {
                let t = self.show(st);
                self.err("E102", x.span, tr!("この分岐は {t} の値ではありません", "This arm is not a value of {t}"));
                None
            }
        }
    }

    // ── Records ─────────────────────────────────────────────────────────────────

    /// A record built (`Claim { … }`), or a value of an enum that has fields (`Order.placed { … }`,
    /// `placed { … }` where an `Order` goes), its fields taken from another value first where `..x`
    /// is written (DESIGN 23.2, 23.4).
    fn record(&mut self, e: &Expr, path: &[(String, Span)], base: Option<&Expr>, inits: &[FieldInit], want: Option<&Ty>) -> Ty {
        // a value of the sum the place wants, by its name alone
        let wanted: Option<EnumId> = match want.map(Ty::present) {
            Some(Ty::Enum(id)) => Some(id.clone()),
            Some(Ty::Record(r)) => r.sum(),
            _ => None,
        };
        let by_place = match (path, &wanted) {
            ([(w, _)], Some(id)) if self.is_sum(id) => self.values(id).and_then(|(vs, _)| vs.into_iter().find(|v| v.is(w))).and_then(|v| Self::variant_of(id, &v.text)),
            _ => None,
        };
        let (t, tps) = match (by_place, &wanted) {
            (Some(r), Some(sum)) => {
                let w = &path[0].0;
                if self.decls.records.iter().any(|x| x.name.is(w)) {
                    let en = self.named_by(&sum.owner, &sum.name);
                    self.err("E110", path[0].1, tr!("`{w}` は、レコードとしても列挙 `{en}` の値としても読めます", "`{w}` reads both as a record and as a value of the enum `{en}`")).notes.push(tr!(
                        "どちらかの名前を替えてください。値なら `{en}.{w} {{ … }}` と書いても、レコードと重ならなくなります。",
                        "Rename one of them; written `{en}.{w} {{ … }}`, the value no longer meets the record."
                    ));
                    return Ty::Error;
                }
                // the type arguments are the sum's the place says
                (Ty::Record(r), Vec::new())
            }
            _ => self.dc.named_open(path),
        };
        let id = match &t {
            Ty::Record(id @ (RecId::Module { .. } | RecId::Element(..) | RecId::Variant { .. })) => id.clone(),
            Ty::Error => return Ty::Error,
            Ty::Record(RecId::Outputs(_)) => {
                let s = self.show(&t);
                self.err(
                    "E105",
                    path[0].1,
                    tr!("{s} は、規則を呼んで受け取るレコードなので、`{{ … }}` では作れません", "{s} is what a rule answers when it is called, which `{{ … }}` does not build"),
                );
                return Ty::Error;
            }
            other => {
                let s = self.show(other);
                let at = path[0].1;
                self.err("E105", at, tr!("{s} はレコードではないので、`{{ … }}` で作れません", "{s} is no record, which `{{ … }}` builds"));
                return Ty::Error;
            }
        };
        // a generic record or value of a sum (DESIGN 26.2): its type arguments from the place, from the value
        // the fields are taken from, then from the fields written
        let (id, t, pre) = if tps.is_empty() {
            (id, t, BTreeMap::new())
        } else {
            match self.build_args(e, &id, &tps, base, inits, want) {
                Some((id, pre)) => (id.clone(), Ty::Record(id), pre),
                None => return Ty::Error,
            }
        };
        let fields = self.fields(&id).unwrap_or_default();
        let shown = self.show(&t);
        // a value of a sum with no fields is written as it is
        if matches!(id, RecId::Variant { .. }) && fields.is_empty() {
            self.err("E105", path[0].1, tr!("{shown} はフィールドの無い値なので、`{{ … }}` では作れません", "{shown} is a value with no fields, which `{{ … }}` does not build"))
                .notes
                .push(tr!("フィールドの無い値は、`{shown}` とだけ書きます。", "A value with no fields is written `{shown}` alone."));
            return Ty::Error;
        }
        self.refer(e, Ref::Record(id.clone()));
        // the value the fields not written are taken from: one of the record's type, or of the value's
        if let Some(b) = base {
            let bt = match pre.get(&b.id) {
                Some(t) => t.clone(),
                None => self.expr(b, Some(&t)),
            };
            if !bt.is_error() && bt != t {
                let bs = self.show(&bt);
                // a value of the enum itself, for a value of one of its values: the value the note names
                let of_sum = match (&bt, id.sum(), &id) {
                    (Ty::Enum(x), Some(y), RecId::Variant { value, .. }) if *x == y => Some(self.values(x).and_then(|(vs, _)| vs.into_iter().find(|n| n.text == *value)).map(|n| n.text).unwrap_or_else(|| value.clone())),
                    _ => None,
                };
                let d = self.err("E102", b.span, tr!("`..` のあとに書けるのは {shown} の値です。これは {bs} の値です", "After `..` goes a value of {shown}; this one is of {bs}"));
                if let Some(v) = of_sum {
                    d.notes.push(tr!(
                        "列挙の値のフィールドを取るには、`match` で値ごとに分け、名前を付けた値（`{v} p` の `p`）を `..p` と書いてください。",
                        "To take the fields of a value of the enum, split it with a `match`, and write the value the arm names (the `p` of `{v} p`) as `..p`."
                    ));
                }
            }
        }
        let mut given: BTreeMap<String, Span> = BTreeMap::new();
        for fi in inits {
            match fields.iter().find(|(n, _)| n.is(&fi.name)) {
                Some((n, slot)) => {
                    if given.insert(n.text.clone(), fi.span).is_some() {
                        let f = &fi.name;
                        self.err("E107", fi.span, tr!("フィールド `{f}` が二度書かれています", "The field `{f}` is given twice"));
                    }
                    let got = match pre.get(&fi.value.id) {
                        Some(t) => t.clone(),
                        None => self.expr(&fi.value, Some(&slot.ty)),
                    };
                    let f = &n.text;
                    self.fits(&got, &slot.ty, fi.value.span, &tr!("{shown} のフィールド `{f}`", "the field `{f}` of {shown}"));
                }
                None => {
                    if !pre.contains_key(&fi.value.id) {
                        self.expr(&fi.value, None);
                    }
                    let f = &fi.name;
                    let names: Vec<String> = fields.iter().map(|(n, _)| format!("`{}`", n.text)).collect();
                    let note = fields_note(&shown, &names);
                    self.err("E101", fi.span, tr!("{shown} にフィールド `{f}` はありません", "There is no field `{f}` in {shown}")).notes.push(note);
                }
            }
        }
        let missing: Vec<String> = fields.iter().filter(|(n, s)| !given.contains_key(&n.text) && !matches!(s.ty, Ty::Opt(_))).map(|(n, _)| format!("`{}`", n.text)).collect();
        if base.is_none() && !missing.is_empty() {
            let m = missing.join(", ");
            self.err("E107", e.span, tr!("{shown} を作るのに、フィールドが足りません: {m}", "Building {shown} leaves out fields: {m}")).notes.push(tr!(
                "レコードはフィールドを全部書いて作ります。値が無いことがある（`T?` の）フィールドだけは省けて、省けば `none` です。",
                "A record is built with every field; only a field that may be none (`T?`) may be left out, and is then `none`."
            ));
        }
        // the expression that builds a value of a sum is of the sum (DESIGN 23.2)
        match id.sum() {
            Some(sum) => Ty::Enum(sum),
            None => t,
        }
    }

    /// The type arguments of a generic record or value of a sum built (DESIGN 26.2): those of the type
    /// the place says, of the value the fields are taken from (`..x`), then those the fields written
    /// say. The record's type with them, and the types of the expressions read for them (each read
    /// once). E112 where one cannot be worked out, or two fields say two types of one.
    #[allow(clippy::type_complexity)]
    fn build_args(&mut self, e: &Expr, id: &RecId, tps: &[names::TParamInfo], base: Option<&Expr>, inits: &[FieldInit], want: Option<&Ty>) -> Option<(RecId, BTreeMap<usize, Ty>)> {
        let names_ = names::tnames(tps);
        let mut got: Vec<Option<Ty>> = vec![None; tps.len()];
        let mut pre: BTreeMap<usize, Ty> = BTreeMap::new();
        let same = |a: &RecId, b: &RecId| match (a, b) {
            (RecId::Module { file: f, name: n, .. }, RecId::Module { file: g, name: m, .. }) => f == g && n == m,
            (RecId::Variant { file: f, name: n, .. }, RecId::Variant { file: g, name: m, .. }) => f == g && n == m,
            _ => false,
        };
        // the place's type
        match want.map(Ty::present) {
            Some(Ty::Record(w)) if same(w, id) => {
                if let RecId::Module { args, .. } | RecId::Variant { args, .. } = w {
                    for (g, a) in got.iter_mut().zip(args) {
                        *g = Some(a.clone());
                    }
                }
            }
            Some(Ty::Enum(w)) if matches!(id, RecId::Variant { file, name, .. } if w.same_decl(&EnumId::plain(Owner::Module(file.clone()), name.clone()))) => {
                for (g, a) in got.iter_mut().zip(&w.args) {
                    *g = Some(a.clone());
                }
            }
            _ => {}
        }
        // the value the fields are taken from
        let open = with_args_of(id, names_.iter().map(|n| Ty::Param(n.clone())).collect());
        let mut clash: Option<(usize, Ty, Ty)> = None;
        let mut unrounded: Option<(usize, Span)> = None;
        if let Some(b) = base {
            let bt = self.expr(b, None);
            pre.insert(b.id, bt.clone());
            self.unify_ty(&Ty::Record(open.clone()), &bt, &names_, &mut got, &mut clash, &mut unrounded, b.span);
        }
        // the fields, the ones that are no constant first
        let decl_fields = self.fields(&open).unwrap_or_default();
        let mut consts: Vec<(Ty, Ty, Span)> = Vec::new();
        for fi in inits {
            let Some((_, slot)) = decl_fields.iter().find(|(n, _)| n.is(&fi.name)) else { continue };
            let hint = known(&slot.ty, &names_, &got);
            let t = self.expr(&fi.value, hint.as_ref());
            pre.insert(fi.value.id, t.clone());
            if konst_of(&t) {
                consts.push((slot.ty.clone(), t, fi.value.span));
            } else {
                self.unify_ty(&slot.ty, &t, &names_, &mut got, &mut clash, &mut unrounded, fi.value.span);
            }
        }
        for (pt, t, sp) in consts {
            let mut none = None;
            self.unify_ty(&pt, &t, &names_, &mut got, &mut none, &mut unrounded, sp);
        }
        let shown = Text::same(format!("`{}`", self.show(&Ty::Record(with_args_of(id, Vec::new())))));
        self.settle(e, &shown, tps, got, clash, unrounded).map(|args| (with_args_of(id, args), pre))
    }

    /// The type arguments worked out, held to their type parameters: E112 for one that is not worked
    /// out or that two places say two types of, E103 for an unrounded value, E114 for one of a
    /// parameter of keys that cannot be one (DESIGN 26.2).
    fn settle(&mut self, e: &Expr, shown: &Text, tps: &[names::TParamInfo], got: Vec<Option<Ty>>, clash: Option<(usize, Ty, Ty)>, unrounded: Option<(usize, Span)>) -> Option<Vec<Ty>> {
        if let Some((i, a, b)) = clash {
            let (p, x, y) = (&tps[i].name.text, self.show(&a), self.show(&b));
            self.err("E112", e.span, tr!("{} の型の引数 `{p}` に、二つの型 {x} と {y} が渡されています", "The type parameter `{p}` of {} is given two types, {x} and {y}", shown.ja; shown.en)).notes.push(tr!(
                "一つの型の引数には、一つの型だけが当たります。単位の違うお金や量も違う型なので、`in` で換算してそろえてください。",
                "A type parameter takes one type: amounts or quantities of two units are of two types too, and `in` converts one to the other."
            ));
            return None;
        }
        if let Some((i, at)) = unrounded {
            let p = &tps[i].name.text;
            self.err("E103", at, tr!("丸めていない値を、型の引数 `{p}` に渡しています", "An unrounded value goes to the type parameter `{p}`")).notes.push(tr!(
                "型の引数には、丸めた値の型だけが当たります。`half_up(x, 1USDc)` のように、丸め方と刻みを書いてから渡してください。",
                "A type parameter takes the type of rounded values only: write the way of rounding and the step first, as in `half_up(x, 1USDc)`."
            ));
            return None;
        }
        let mut out = Vec::new();
        for (p, g) in tps.iter().zip(got) {
            match g {
                Some(t) => {
                    if p.key && !self.key(&t, e.span, &tr!("型の引数 `{}` に当たる型", "the type the type parameter `{}` takes", p.name.text; p.name.text), false, false) {
                        return None;
                    }
                    out.push(t)
                }
                None => {
                    let n = &p.name.text;
                    self.err("E112", e.span, tr!("{} の型の引数 `{n}` が決まりません", "The type parameter `{n}` of {} cannot be worked out", shown.ja; shown.en)).notes.push(tr!(
                        "型の引数は、渡す値の型から決まり、決まらなければ置かれた場所の型から決まります。`none` や `[]` だけでは決まりません。型を書いた `let` に置くか（`let x : …[number] = …`）、関数なら名前のあとに型を書いてください（`f[number](…)`）。",
                        "A type parameter takes the type of the values given, else the type of the place the value is put in; `none` or `[]` alone says none. Put it where a type is written (`let x : …[number] = …`), or write the type after a function's name (`f[number](…)`)."
                    ));
                    return None;
                }
            }
        }
        if !self.none_free(tps, &out, e.span, shown) {
            return None;
        }
        Some(out)
    }

    /// E112 for a type argument whose values may be none (`number?`): in a generic body `T?` would hold
    /// a none of its own and the one of the type it is given, which the generated Go tells apart and
    /// kaname does not (DESIGN 26.2). Whether none is.
    fn none_free(&mut self, tps: &[names::TParamInfo], tys: &[Ty], at: Span, shown: &Text) -> bool {
        for (p, t) in tps.iter().zip(tys) {
            // a function type: a value of a type parameter is held and compared, a function value is not
            // (DESIGN 27.1)
            if t.has_fn() {
                let (n, ty) = (&p.name.text, self.show(t));
                self.err("E112", at, tr!("{} の型の引数 `{n}` に、関数の型 {ty} が当たっています", "The type parameter `{n}` of {} is given the function type {ty}", shown.ja; shown.en)).notes.push(tr!(
                    "型の引数の値は、入れて持ち、比べることのできる値です。関数の値は渡すことと呼ぶことにだけ使うので、型の引数には当たりません。",
                    "A value of a type parameter is held and compared, and a function value is only passed and called, so no type parameter takes a function type."
                ));
                return false;
            }
            if let Ty::Opt(_) = t {
                let (n, ty) = (&p.name.text, self.show(t));
                self.err("E112", at, tr!("{} の型の引数 `{n}` に、値の無いことがある型 {ty} が当たっています", "The type parameter `{n}` of {} is given {ty}, a type whose values may be none", shown.ja; shown.en)).notes.push(tr!(
                    "型の引数には、値の無いことがある型は当たりません（総称の中の `{n}?` が、二つの none を持つことになるためです）。`?` の無い型を当て、値の無いことがあるところは `{n}?` と書いてください。",
                    "A type parameter takes no type whose values may be none (its `{n}?` would hold two kinds of none): give it the type with no `?`, and write `{n}?` where the value may be none."
                ));
                return false;
            }
        }
        true
    }

    /// The types the type parameters `names` take, from a type that has them (`pat`) and the type of the
    /// value put there (`act`): what is found is kept in `got`; two types for one in `clash`; an
    /// unrounded value in `unrounded`. `none`, `[]` and `[:]` say nothing (DESIGN 26.2).
    #[allow(clippy::too_many_arguments)]
    fn unify_ty(&self, pat: &Ty, act: &Ty, names: &[String], got: &mut [Option<Ty>], clash: &mut Option<(usize, Ty, Ty)>, unrounded: &mut Option<(usize, Span)>, at: Span) {
        match (pat, act) {
            (_, Ty::Error | Ty::None | Ty::Empty | Ty::EmptyDict) => {}
            (Ty::Param(n), a) => {
                let Some(i) = names.iter().position(|x| x == n) else { return };
                let Some(t) = type_arg(a) else {
                    if unrounded.is_none() {
                        *unrounded = Some((i, at));
                    }
                    return;
                };
                match &got[i] {
                    None => got[i] = Some(t),
                    Some(g) => {
                        // a constant in another unit of the one found goes there (DESIGN 3.2)
                        let fits = *g == t || matches!(a, Ty::Num(x) if x.konst.is_some() && self.agrees(a, g).is_ok());
                        if !fits && clash.is_none() {
                            *clash = Some((i, g.clone(), t));
                        }
                    }
                }
            }
            (Ty::Opt(p), Ty::Opt(a)) | (Ty::List(p), Ty::List(a)) | (Ty::Set(p), Ty::Set(a)) => self.unify_ty(p, a, names, got, clash, unrounded, at),
            (Ty::Opt(p), a) => self.unify_ty(p, a, names, got, clash, unrounded, at),
            (Ty::Fn(pp, pr, _), Ty::Fn(ap, ar, _)) if pp.len() == ap.len() => {
                for (x, y) in pp.iter().zip(ap) {
                    self.unify_ty(x, y, names, got, clash, unrounded, at);
                }
                self.unify_ty(pr, ar, names, got, clash, unrounded, at);
            }
            (Ty::Dict(pk, pv), Ty::Dict(ak, av)) | (Ty::Result(pk, pv), Ty::Result(ak, av)) => {
                self.unify_ty(pk, ak, names, got, clash, unrounded, at);
                self.unify_ty(pv, av, names, got, clash, unrounded, at);
            }
            (Ty::Enum(p), Ty::Enum(a)) if p.same_decl(a) => {
                for (x, y) in p.args.iter().zip(&a.args) {
                    self.unify_ty(x, y, names, got, clash, unrounded, at);
                }
            }
            (Ty::Enum(p), Ty::Record(r)) if r.sum().is_some_and(|s| s.same_decl(p)) => {
                if let RecId::Variant { args, .. } = r {
                    for (x, y) in p.args.iter().zip(args) {
                        self.unify_ty(x, y, names, got, clash, unrounded, at);
                    }
                }
            }
            (Ty::Record(p), Ty::Record(a)) => match (p, a) {
                (RecId::Module { file: f, name: n, args: pa }, RecId::Module { file: g, name: m, args: aa }) | (RecId::Variant { file: f, name: n, args: pa, .. }, RecId::Variant { file: g, name: m, args: aa, .. })
                    if f == g && n == m =>
                {
                    for (x, y) in pa.iter().zip(aa) {
                        self.unify_ty(x, y, names, got, clash, unrounded, at);
                    }
                }
                (RecId::Entry(pk, pv), RecId::Entry(ak, av)) | (RecId::Running(pk, pv), RecId::Running(ak, av)) => {
                    self.unify_ty(pk, ak, names, got, clash, unrounded, at);
                    self.unify_ty(pv, av, names, got, clash, unrounded, at);
                }
                (RecId::Indexed(p), RecId::Indexed(a)) => self.unify_ty(p, a, names, got, clash, unrounded, at),
                _ => {}
            },
            _ => {}
        }
    }

    /// A call of a generic function (DESIGN 26.2): its type arguments, written after its name or worked
    /// out from the arguments (from the left, those that are no constant first) and then from the place
    /// the call is put in; each argument held to its parameter with them put in. The call's type.
    fn call_generic(&mut self, f: &names::FnInfo, args: &[Arg], e: &Expr, want: Option<&Ty>) -> Ty {
        let names_ = names::tnames(&f.tparams);
        let fun = f.name.text.clone();
        let shown = tr!("関数 `{fun}`", "the function `{fun}`");
        // the arguments by the parameters' names (an alias too)
        let named: Vec<(Option<usize>, &Expr)> = args
            .iter()
            .map(|a| match a {
                Arg::Named(n, _, x) => (f.params.iter().position(|p| p.name.is(n)), x),
                Arg::Pos(x) => (None, x),
            })
            .collect();
        let mut pre: BTreeMap<usize, Ty> = BTreeMap::new();
        let written = self.dc.module.targs.get(&e.id).cloned();
        let targs: Option<Vec<Ty>> = match written {
            Some(ts) => {
                let tys: Vec<Ty> = ts.iter().map(|t| self.dc.resolve_in(t, names::Within::Arg)).collect();
                if tys.len() != f.tparams.len() {
                    let (n, k) = (f.tparams.len(), tys.len());
                    let en_n = ritsu_base::text::plural(n, "type argument", "type arguments");
                    self.err("E112", e.span, tr!("関数 `{fun}` は型の引数を {n} 個取ります。書いてあるのは {k} 個です", "`{fun}` takes {en_n}, and is given {k}"));
                    None
                } else if tys.iter().any(Ty::is_error) {
                    None
                } else {
                    let mut ok = true;
                    for (p, t) in f.tparams.iter().zip(&tys) {
                        if p.key && !self.key(t, e.span, &tr!("型の引数 `{}` に当たる型", "the type the type parameter `{}` takes", p.name.text; p.name.text), false, false) {
                            ok = false;
                        }
                    }
                    let ok = ok && self.none_free(&f.tparams, &tys, e.span, &shown);
                    ok.then_some(tys)
                }
            }
            None => {
                let mut got: Vec<Option<Ty>> = vec![None; f.tparams.len()];
                let (mut clash, mut unrounded) = (None, None);
                let mut consts: Vec<(Ty, Ty, Span)> = Vec::new();
                // a lambda given to a parameter of a function type is read last, with the types the other
                // arguments give (DESIGN 27.1)
                let lambda_at = |k: &Option<usize>, x: &Expr| matches!(x.kind, ExprKind::Lambda(..)) && k.is_some_and(|k| matches!(f.params[k].slot.ty, Ty::Fn(..)));
                for (k, x) in &named {
                    if lambda_at(k, x) {
                        continue;
                    }
                    let hint = k.and_then(|k| known(&f.params[k].slot.ty, &names_, &got));
                    // a function value given to a parameter of a type parameter is said where its type
                    // is held to the type parameters (E112), not where it is put
                    self.allow = k.is_some_and(|k| matches!(f.params[k].slot.ty, Ty::Fn(..) | Ty::Param(_)));
                    let t = self.expr(x, hint.as_ref());
                    pre.insert(x.id, t.clone());
                    let Some(k) = k else { continue };
                    if konst_of(&t) {
                        consts.push((f.params[*k].slot.ty.clone(), t, x.span));
                    } else {
                        self.unify_ty(&f.params[*k].slot.ty, &t, &names_, &mut got, &mut clash, &mut unrounded, x.span);
                    }
                }
                for (pt, t, sp) in consts {
                    let mut none = None;
                    self.unify_ty(&pt, &t, &names_, &mut got, &mut none, &mut unrounded, sp);
                }
                if let Some(w) = want
                    && got.iter().any(Option::is_none)
                {
                    let mut none = None;
                    let mut nr = None;
                    self.unify_ty(&f.ret.ty, w, &names_, &mut got, &mut none, &mut nr, e.span);
                }
                let mut lambda_said = false;
                for (k, x) in &named {
                    if !lambda_at(k, x) {
                        continue;
                    }
                    let pt = f.params[k.expect("a parameter")].slot.ty.clone();
                    let Ty::Fn(pps, pr, _) = &pt else { continue };
                    let placed: Vec<Option<Ty>> = pps.iter().map(|p| known(p, &names_, &got)).collect();
                    let ret = known(pr, &names_, &got);
                    let t = self.lambda_value(x, Some(&placed), ret.as_ref());
                    self.set(x, &t);
                    pre.insert(x.id, t.clone());
                    lambda_said |= t.is_error();
                    self.unify_ty(&pt, &t, &names_, &mut got, &mut clash, &mut unrounded, x.span);
                }
                if lambda_said {
                    None
                } else {
                    let named_fn = tr!("関数 `{fun}`", "`{fun}`");
                    self.settle(e, &named_fn, &f.tparams, got, clash, unrounded)
                }
            }
        };
        let Some(targs) = targs else {
            // what is given is still read, once (a lambda, which takes its types from the call, is not)
            for (_, x) in &named {
                if !pre.contains_key(&x.id) && !matches!(x.kind, ExprKind::Lambda(..)) {
                    self.expr(x, None);
                }
            }
            return Ty::Error;
        };
        let params: Vec<(String, Slot, bool)> = f.params.iter().map(|p| (p.name.text.clone(), subst_slot(&p.slot, &names_, &targs), true)).collect();
        let args: Vec<Arg> = args
            .iter()
            .map(|a| match a {
                Arg::Named(n, sp, x) => match f.params.iter().find(|p| p.name.is(n)) {
                    Some(p) => Arg::Named(p.name.text.clone(), *sp, x.clone()),
                    None => a.clone(),
                },
                _ => a.clone(),
            })
            .collect();
        self.named_args_pre(&params, &args, &shown, (&tr!("引数", "argument"), &tr!("引数", "arguments")), e, "E107", &pre);
        let ret = subst(&f.ret.ty, &names_, &targs);
        self.inst.insert(e.id, targs);
        ret
    }

    // ── Calls ───────────────────────────────────────────────────────────────────

    fn call(&mut self, e: &Expr, callee: &Expr, args: &[Arg], want: Option<&Ty>) -> Ty {
        // a builtin written after a value (DESIGN 21.4): `x.f(…)` where `x` is no `use` name, which a
        // name in a function never is (E006)
        if let ExprKind::Field(base, f, fsp) = &callee.kind {
            let first = base.path().map(|p| p[0].0.clone());
            let used = first.as_deref().is_some_and(|w| self.lookup(w).is_none() && self.dc.uses.contains_key(w));
            if !used {
                return self.method(e, callee, base, f, *fsp, args, want);
            }
        }
        // a function value called (DESIGN 27.1): a local of a function type, or a value that is no name
        let Some(path) = callee.path() else {
            return self.call_value(e, callee, args);
        };
        if let [(w, _)] = path.as_slice()
            && matches!(self.lookup(w), Some(Ty::Fn(..) | Ty::Error))
        {
            return self.call_value(e, callee, args);
        }
        let words: Vec<&str> = path.iter().map(|(w, _)| w.as_str()).collect();
        // a local that is no function value is never called, so a call names a function, a rule or a
        // builtin even where a local has the same name (`let count = count(lines)`)
        let callable = words.len() > 1 || self.decls.fns.iter().any(|f| f.name.is(words[0])) || self.dc.uses.contains_key(words[0]) || crate::kw::is_builtin(words[0]);
        if !callable && self.lookup(words[0]).is_some() {
            let w = words[0];
            self.err("E105", callee.span, tr!("`{w}` は値なので、呼び出せません", "`{w}` is a value, which is not called"));
            return Ty::Error;
        }
        if words.len() > 1 && self.lookup(words[0]).is_some() {
            let w = words[0];
            self.err("E105", callee.span, tr!("`{w}` は値なので、その中のものは呼び出せません", "`{w}` is a value, and nothing in it is called"));
            return Ty::Error;
        }
        match words.as_slice() {
            [w] => {
                if let Some(fi) = self.decls.fns.iter().position(|f| f.name.is(w)) {
                    let info = self.decls.fns[fi].clone();
                    self.refer(callee, Ref::Fn(self.dc.canon.to_path_buf(), info.name.text.clone()));
                    if !info.tparams.is_empty() {
                        return self.call_generic(&info, args, e, want);
                    }
                    self.no_targs(e, &info.name.text);
                    self.call_fn(&info, args, callee.span, e);
                    return info.ret.ty;
                }
                match self.dc.uses.get(*w).map(|(_, t)| t.clone()) {
                    // a rule with no outputs (an `enums` file) is read for its enums: nothing to call
                    // is generated from it (rulec's §15.258), so a call of it stops (E201)
                    Some(Target::Rule { facts, .. }) if facts.outputs.is_empty() => {
                        for a in args {
                            self.expr(a.expr(), None);
                        }
                        self.err("E201", callee.span, tr!("規則 `{w}` には出力が無いので、呼べません", "The rule `{w}` has no outputs, and is not called")).notes.push(no_outputs_note(w));
                        return Ty::Error;
                    }
                    Some(Target::Rule { canon, facts, .. }) => {
                        self.refer(callee, Ref::Rule(canon.clone()));
                        self.call_rule(w, &canon, &facts, args, e);
                        return Ty::Record(RecId::Outputs(canon));
                    }
                    Some(_) => {
                        self.err("E107", callee.span, tr!("`{w}` は、そのままでは呼べません", "`{w}` is not called as it is")).notes.push(tr!(
                            "日付は `{w}.<日付>(…)`、振替は `{w}.<振替>.<操作>(…)`、ほかのモジュールの関数は `{w}.<関数>(…)` と呼びます。",
                            "A date is called `{w}.<date>(…)`, a transfer `{w}.<transfer>.<operation>(…)`, another module's function `{w}.<function>(…)`."
                        ));
                        return Ty::Error;
                    }
                    None => {}
                }
                if let Some(b) = crate::kw::BUILTINS.iter().find(|b| **b == *w) {
                    self.refer(callee, Ref::Builtin(b));
                    return self.builtin(e, b, None, args, want);
                }
                let d = self.err("E101", callee.span, tr!("`{w}` という関数はありません", "There is no function `{w}`"));
                // a builtin written only after a value, written as a function (DESIGN 21.4)
                if crate::kw::is_method(w) {
                    d.notes.push(tr!(
                        "`{w}` は、値のあとに `.` を付けて呼ぶ組み込みの関数です（`<値>.{w}(…)`）。",
                        "`{w}` is a builtin written after a value, with a `.`: `<value>.{w}(…)`."
                    ));
                }
                Ty::Error
            }
            [u, d] => match self.dc.uses.get(*u).map(|(_, t)| t.clone()) {
                Some(Target::Dates { canon, facts, .. }) => {
                    let Some(f) = facts.functions.iter().find(|f| f.name == *d || f.alias == *d).cloned() else {
                        let all = facts.functions.iter().map(|f| format!("`{}`", f.name)).collect::<Vec<_>>().join(", ");
                        self.err("E101", path[1].1, tr!("日付のファイル `{u}` に、日付 `{d}` はありません", "The dates file `{u}` has no date `{d}`"))
                            .notes
                            .push(tr!("日付は {all} です。", "Its dates are {all}."));
                        return Ty::Error;
                    };
                    self.refer(callee, Ref::Date(canon.clone(), f.name.clone()));
                    let params: Vec<(String, Slot, bool)> = f.params.iter().filter_map(|p| date_input_slot(&facts, p).map(|s| (p.clone(), s, true))).collect();
                    let what = tr!("日付 `{u}.{d}`", "the date `{u}.{d}`");
                    self.named_args(&params, args, &what, (&tr!("入力", "input"), &tr!("入力", "inputs")), e, "E107");
                    Ty::Date
                }
                Some(Target::Module(c)) => {
                    let Some(info) = c.decls.fns.iter().find(|f| f.name.is(d)).cloned() else {
                        self.err("E101", path[1].1, tr!("モジュール `{u}` に、関数 `{d}` はありません", "The module `{u}` has no function `{d}`"));
                        return Ty::Error;
                    };
                    // a `private` function of another module (DESIGN 28.2)
                    if info.private {
                        for a in args {
                            self.expr(a.expr(), None);
                        }
                        let note = names::private_note(&tr!("関数", "function"), &info.name.text, &c.module.name.text);
                        self.err("E101", path[1].1, tr!("モジュール `{u}` に、関数 `{d}` はありません", "The module `{u}` has no function `{d}`")).notes.push(note);
                        return Ty::Error;
                    }
                    self.refer(callee, Ref::Fn(c.canon.clone(), info.name.text.clone()));
                    if !info.tparams.is_empty() {
                        return self.call_generic(&info, args, e, want);
                    }
                    self.no_targs(e, &info.name.text);
                    self.call_fn(&info, args, callee.span, e);
                    info.ret.ty
                }
                _ => {
                    let shown = words.join(".");
                    self.err("E101", callee.span, tr!("`{shown}` という関数はありません", "There is no function `{shown}`"));
                    Ty::Error
                }
            },
            [b, t, op] => match self.dc.uses.get(*b).map(|(_, x)| x.clone()) {
                Some(Target::Book { canon, facts, .. }) => self.transfer(e, b, &canon, &facts, t, op, &path, args),
                _ => {
                    let shown = words.join(".");
                    self.err("E101", callee.span, tr!("`{shown}` という関数はありません", "There is no function `{shown}`"));
                    Ty::Error
                }
            },
            _ => {
                let shown = words.join(".");
                self.err("E101", callee.span, tr!("`{shown}` という関数はありません", "There is no function `{shown}`"));
                Ty::Error
            }
        }
    }

    /// A call of a function value (DESIGN 27.1): the callee's value, of a function type, called with the
    /// arguments in order and with no names (E107), each fitting its parameter.
    fn call_value(&mut self, e: &Expr, callee: &Expr, args: &[Arg]) -> Ty {
        self.allow = true;
        let ft = self.expr(callee, None);
        let (ps, r) = match &ft {
            Ty::Fn(ps, r, _) => (ps.clone(), (**r).clone()),
            // what is called is said to be wrong already: its arguments are read with nothing more said
            // of a lambda among them
            Ty::Error => {
                for a in args {
                    self.allow = true;
                    self.expr(a.expr(), Some(&Ty::Error));
                }
                return Ty::Error;
            }
            other => {
                for a in args {
                    self.allow = true;
                    self.expr(a.expr(), Some(&Ty::Error));
                }
                let shown = self.show(other);
                let d = self.err("E105", callee.span, tr!("{shown} の値は、呼び出せません", "A value of {shown} is not called"));
                if let ExprKind::Var(w) = &callee.kind {
                    d.notes.push(tr!("`{w}` は値です。呼び出せるのは、関数、規則、日付、振替、組み込みの関数と、関数の値です。", "`{w}` is a value: what is called is a function, a rule, a date, a transfer, a builtin or a function value."));
                }
                return Ty::Error;
            }
        };
        self.refer(e, Ref::CallValue);
        let mut named = false;
        for (i, a) in args.iter().enumerate() {
            if let Arg::Named(n, sp, _) = a
                && !named
            {
                named = true;
                self.err("E107", *sp, tr!("関数の値には、引数を名前を付けずに順に渡します（`{n}:` は要りません）", "A function value is given its arguments in order, with no names (`{n}:` is not written)")).notes.push(tr!(
                    "関数の型は引数の名前を持たないので、関数の値は `f(x)` のように引数を順に並べて呼びます。名前を付けて渡すのは、宣言した関数と規則の呼び出しです。",
                    "A function type has no names for its parameters, so a function value is called with its arguments in order, as in `f(x)`; arguments are named in a call of a declared function or a rule."
                ));
            }
            let x = a.expr();
            match ps.get(i) {
                Some(p) => {
                    let got = self.expr(x, Some(p));
                    let k = i + 1;
                    self.fits(&got, p, x.span, &tr!("関数の値の {k} 番目の引数", "argument {k} of the function value"));
                }
                None => {
                    self.expr(x, None);
                }
            }
        }
        if args.len() != ps.len() {
            let (n, k) = (ps.len(), args.len());
            let shown = self.show(&ft);
            let en_n = ritsu_base::text::plural(n, "argument", "arguments");
            self.err("E107", e.span, tr!("関数の値（{shown}）が取る引数は {n} 個です。渡しているのは {k} 個です", "The function value ({shown}) takes {en_n}, and is given {k}"));
        }
        r
    }

    /// `x.f(…)`: the builtin `f` with the value `x` as its first argument (DESIGN 21.4). E101 where `f`
    /// is no builtin written after a value.
    #[allow(clippy::too_many_arguments)]
    fn method(&mut self, e: &Expr, callee: &Expr, base: &Expr, f: &str, at: Span, args: &[Arg], want: Option<&Ty>) -> Ty {
        let found = crate::kw::BUILTINS.iter().chain(crate::kw::METHODS.iter()).find(|b| **b == f && !crate::kw::FUNCTIONS_ONLY.contains(b));
        let Some(b) = found else {
            let t = self.expr(base, None);
            let d = self.err("E101", at, tr!("値のあとに書ける組み込みの関数 `{f}` はありません", "There is no builtin `{f}` to write after a value"));
            if crate::kw::FUNCTIONS_ONLY.contains(&f) {
                d.notes.push(tr!("`{f}` は、値のあとに書かず、関数の形で呼ぶ組み込みの関数です（`{f}(…)`）。", "`{f}` is a builtin called as a function, not written after a value: `{f}(…)`."));
            } else if let Ty::Record(_) = t {
                d.notes.push(tr!(
                    "レコードのフィールドは `.` で読み、呼びません。関数を値にして入れることはできません。",
                    "A field of a record is read with `.`, and not called: no field holds a function."
                ));
            }
            return Ty::Error;
        };
        self.refer(callee, Ref::Method(b));
        self.builtin(e, b, Some(base), args, want)
    }

    /// Arguments written by name, held to the places they go to: E107 (or `code`) for one that is
    /// not given, given twice, given with no name, or given and not taken. `word` is what an
    /// argument is called (an argument, a rule's input), one and many.
    fn named_args(&mut self, params: &[(String, Slot, bool)], args: &[Arg], what: &Text, word: (&Text, &Text), e: &Expr, code: &'static str) {
        self.named_args_pre(params, args, what, word, e, code, &BTreeMap::new())
    }

    /// The same, the types of the arguments read already (`pre`, by the id of the expression) taken as
    /// they are.
    #[allow(clippy::too_many_arguments)]
    fn named_args_pre(&mut self, params: &[(String, Slot, bool)], args: &[Arg], what: &Text, word: (&Text, &Text), e: &Expr, code: &'static str, pre: &BTreeMap<usize, Ty>) {
        let mut given: BTreeSet<String> = BTreeSet::new();
        let w = crate::diag::ja_before(&what.ja);
        let (one, many) = word;
        let mut positional = false;
        for a in args {
            match a {
                Arg::Pos(x) => {
                    if !pre.contains_key(&x.id) {
                        self.expr(x, None);
                    }
                    if !positional {
                        positional = true;
                        self.err(code, x.span, tr!("{w}の{}は、名前を付けて渡してください（`<名前>: <値>`）", "Give the {} of {} by name (`<name>: <value>`)", many.ja; many.en, what.en)).notes.push(
                            tr!(
                                "同じ型の値を取り違えないよう、関数、規則、日付、振替に渡す値は名前で書きます。",
                                "What a function, a rule, a date or a transfer is given is written by name, so that two values of one type are not taken for each other."
                            ),
                        );
                    }
                }
                Arg::Named(n, sp, x) => match params.iter().find(|(p, _, _)| p == n) {
                    Some((p, slot, _)) => {
                        if !given.insert(p.clone()) {
                            self.err(code, *sp, tr!("{} `{n}` が二度書かれています", "The {} `{n}` is given twice", one.ja; one.en));
                        }
                        let got = match pre.get(&x.id) {
                            Some(t) => t.clone(),
                            None => {
                                self.allow = matches!(slot.ty, Ty::Fn(..));
                                self.expr(x, Some(&slot.ty))
                            }
                        };
                        let place = tr!("{w}の{} `{p}`", "the {} `{p}` of {}", one.ja; one.en, what.en);
                        self.fits(&got, &slot.ty, x.span, &place);
                    }
                    None => {
                        if !pre.contains_key(&x.id) {
                            self.expr(x, None);
                        }
                        let all = Text::list(&params.iter().map(|(p, _, _)| Text::same(format!("`{p}`"))).collect::<Vec<_>>());
                        self.err(code, *sp, tr!("{w}は `{n}` を取りません", "{} takes no `{n}`", ; what.en)).notes.push(tr!("取るのは {} です。", "It takes {}.", all.ja; all.en));
                    }
                },
            }
        }
        if positional {
            return;
        }
        let missing: Vec<String> = params.iter().filter(|(p, s, required)| *required && !given.contains(p) && !matches!(s.ty, Ty::Opt(_))).map(|(p, _, _)| format!("`{p}`")).collect();
        if !missing.is_empty() {
            let m = missing.join(", ");
            self.err(code, e.span, tr!("{w}の{}が足りません: {m}", "{} is missing {}: {m}", one.ja; what.en, many.en));
        }
    }

    /// E112 for type arguments written after a function that takes none.
    fn no_targs(&mut self, e: &Expr, fun: &str) {
        if self.dc.module.targs.contains_key(&e.id) {
            self.err("E112", e.span, tr!("関数 `{fun}` は型の引数を取りません", "`{fun}` takes no type arguments"));
        }
    }

    fn call_fn(&mut self, f: &names::FnInfo, args: &[Arg], at: Span, e: &Expr) {
        let _ = at;
        let params: Vec<(String, Slot, bool)> = f.params.iter().map(|p| (p.name.text.clone(), p.slot.clone(), true)).collect();
        // an argument may be written by the parameter's alias, as a name may
        let args: Vec<Arg> = args
            .iter()
            .map(|a| match a {
                Arg::Named(n, sp, x) => match f.params.iter().find(|p| p.name.is(n)) {
                    Some(p) => Arg::Named(p.name.text.clone(), *sp, x.clone()),
                    None => a.clone(),
                },
                _ => a.clone(),
            })
            .collect();
        let fun = &f.name.text;
        self.named_args(&params, &args, &tr!("関数 `{fun}`", "the function `{fun}`"), (&tr!("引数", "argument"), &tr!("引数", "arguments")), e, "E107");
    }

    fn call_rule(&mut self, name: &str, canon: &Path, facts: &RuleFacts, args: &[Arg], e: &Expr) {
        let mut params: Vec<(String, Slot, bool)> = facts.inputs.iter().map(|c| (c.name.clone(), column_slot(c, canon), true)).collect();
        for (seq, _, _) in &facts.elements {
            params.push((seq.clone(), Slot::of(Ty::List(Box::new(Ty::Record(RecId::Element(canon.to_path_buf(), seq.clone()))))), true));
        }
        // an input or a list the rule walks may be written by its alias, as a name may
        let named = |n: &str| -> Option<String> {
            if facts.inputs.iter().any(|c| c.name == n) || facts.elements.iter().any(|(s, _, _)| s == n) {
                return None;
            }
            facts.inputs.iter().find(|c| c.alias == n).map(|c| c.name.clone()).or_else(|| facts.elements.iter().find(|(_, a, _)| a == n).map(|(s, _, _)| s.clone()))
        };
        let args: Vec<Arg> = args
            .iter()
            .map(|a| match a {
                Arg::Named(n, sp, x) => match named(n) {
                    Some(name) => Arg::Named(name, *sp, x.clone()),
                    None => a.clone(),
                },
                _ => a.clone(),
            })
            .collect();
        self.named_args(&params, &args, &tr!("規則 `{name}`", "the rule `{name}`"), (&tr!("入力", "input"), &tr!("入力", "inputs")), e, "E107");
    }

    #[allow(clippy::too_many_arguments)]
    fn transfer(&mut self, e: &Expr, b: &str, canon: &Path, facts: &BookFacts, t: &str, op: &str, path: &[(String, Span)], args: &[Arg]) -> Ty {
        let Some(x) = facts.transfers.iter().find(|x| x.name == t).cloned() else {
            let all = facts.transfers.iter().map(|x| format!("`{}`", x.name)).collect::<Vec<_>>().join(", ");
            self.err("E204", path[1].1, tr!("帳簿 `{b}` に、振替 `{t}` はありません", "The book `{b}` has no transfer `{t}`")).notes.push(tr!("振替は {all} です。", "Its transfers are {all}."));
            return Ty::Error;
        };
        let ops: &[&str] = if x.pending.is_some() { &["hold", "post", "void"] } else { &["do"] };
        if !ops.contains(&op) {
            let all = ops.iter().map(|o| format!("`{o}`")).collect::<Vec<_>>().join(", ");
            let kind = if x.pending.is_some() { tr!("仮押さえする振替", "a transfer that holds first") } else { tr!("すぐに確定する振替", "a transfer done at once") };
            self.err("E204", path[2].1, tr!("振替 `{b}.{t}` に、操作 `{op}` はありません", "The transfer `{b}.{t}` has no operation `{op}`")).notes.push(tr!(
                "`{t}` は{}なので、操作は {all} です（chobo の振替と同じです）。",
                "`{t}` is {}, whose operations are {all} (as chobo's are).",
                kind.ja;
                kind.en
            ));
            return Ty::Error;
        }
        let id = TransferId { book: canon.to_path_buf(), transfer: x.name.clone(), op: op.to_string() };
        if let ExprKind::Call(callee, _) = &e.kind {
            self.refer(callee, Ref::Transfer(id.clone()));
        }
        let unit_of = |p: &ritsu_ports::TransferParam| -> Ty {
            match &p.unit {
                None => Ty::Str,
                Some(u) => names::book_unit(facts, u).map(|u| Ty::Num(NumTy::of(u))).unwrap_or(Ty::Error),
            }
        };
        let keyed = matches!(op, "post" | "void");
        let mut params: Vec<(String, Slot, bool)> = Vec::new();
        for p in &x.params {
            let in_key = x.key.contains(&p.name);
            if !keyed || in_key {
                params.push((p.name.clone(), Slot::of(unit_of(p)), true));
            } else if op == "post" && p.unit.is_some() {
                params.push((p.name.clone(), Slot::of(unit_of(p)), false));
            }
        }
        let what = tr!("振替 `{b}.{t}.{op}`", "the transfer `{b}.{t}.{op}`");
        self.named_args(&params, args, &what, (&tr!("引数", "argument"), &tr!("引数", "arguments")), e, "E204");
        // a post that confirms a part gives every amount, or none (DESIGN 3.8)
        if op == "post" {
            let amounts: Vec<&String> = params.iter().filter(|(_, _, r)| !*r).map(|(p, _, _)| p).collect();
            let given = amounts.iter().filter(|p| args.iter().any(|a| matches!(a, Arg::Named(n, _, _) if n == **p))).count();
            if given > 0 && given < amounts.len() {
                let all = amounts.iter().map(|p| format!("`{p}`")).collect::<Vec<_>>().join(", ");
                self.err("E204", e.span, tr!("一部を確定する `post` は、額の引数を全部書いてください（{all}）", "A `post` of a part gives every amount ({all})"))
                    .notes
                    .push(tr!("額を一つも書かなければ、仮押さえした全部を確定します。", "With no amount given, it posts the whole of the hold."));
            }
        }
        // an amount in another unit of the book's dimension: what the book's unit counts, said with
        // the diagnostic of that argument (DESIGN 12, E102)
        for a in args {
            let Arg::Named(n, _, v) = a else { continue };
            let Some(u) = x.params.iter().find(|p| p.name == *n).and_then(|p| p.unit.clone()) else { continue };
            let (Some(Ty::Num(got)), Some(bu), Some(want)) = (self.types.get(v.id).cloned(), facts.units.iter().find(|bu| bu.name == u), names::book_unit(facts, &u)) else { continue };
            if !got.has_unit() || got.konst.is_some() || got.unit.dim != want.dim || same_unit(&got.unit, &want) {
                continue;
            }
            let (wt, scale) = (unit_text(&want), bu.scale);
            let at = (Some(v.span.line), Some(v.span.col));
            if let Some(d) = self.dc.diags.iter_mut().find(|d| d.code == "E102" && (d.line, d.col) == at) {
                d.notes.insert(0, tr!("帳簿 `{b}` の単位 `{u} scale {scale}` は、{wt} で数えます。", "The unit `{u} scale {scale}` of the book `{b}` counts {wt}."));
            }
        }
        Ty::Transfer(id)
    }

    // ── Builtins ────────────────────────────────────────────────────────────────

    /// The arguments of a builtin, in order, the value it is written after first (DESIGN 21.4): E107
    /// for named ones, or too few or too many (counted after the value, for a builtin written after
    /// one).
    fn positional<'x>(&mut self, name: &str, recv: Option<&'x Expr>, args: &'x [Arg], n: std::ops::RangeInclusive<usize>, e: &Expr) -> Option<Vec<&'x Expr>> {
        let mut out: Vec<&'x Expr> = recv.into_iter().collect();
        for a in args {
            match a {
                Arg::Pos(x) => out.push(x),
                Arg::Named(w, sp, x) => {
                    self.err(
                        "E107",
                        *sp,
                        tr!(
                            "組み込みの関数 `{name}` の引数は、名前を付けずに順に渡します（`{w}:` は要りません）",
                            "The arguments of the builtin `{name}` are given in order, with no name (`{w}:` is not written)"
                        ),
                    );
                    out.push(x);
                }
            }
        }
        if !n.contains(&out.len()) {
            let skip = usize::from(recv.is_some());
            let (lo, hi) = (*n.start() - skip, if *n.end() == usize::MAX { usize::MAX } else { *n.end() - skip });
            let want = if lo == hi && lo == 1 {
                tr!("1 個", "1 argument")
            } else if lo == hi {
                tr!("{lo} 個", "{lo} arguments")
            } else if hi == usize::MAX {
                tr!("{lo} 個以上", "{lo} or more arguments")
            } else {
                tr!("{lo} 個か {hi} 個", "{lo} or {hi} arguments")
            };
            let got = out.len() - skip;
            if recv.is_some() {
                self.err("E107", e.span, tr!("`{name}` が値のあとに取る引数は {}です。渡しているのは {got} 個です", "`{name}` takes {} after the value, and is given {got}", want.ja; want.en));
            } else {
                self.err("E107", e.span, tr!("`{name}` が取る引数は {}です。渡しているのは {got} 個です", "`{name}` takes {}, and is given {got}", want.ja; want.en));
            }
            for x in &out {
                if !matches!(x.kind, ExprKind::Lambda(..)) {
                    self.expr(x, None);
                }
            }
            return None;
        }
        Some(out)
    }

    /// E102 for a builtin given what it is not called on: its first argument, or the value it is
    /// written after (`on`).
    fn not_on(&mut self, name: &str, at: Span, t: &Ty, on: bool, what: &Text) {
        let s = self.show(t);
        if on {
            self.err("E102", at, tr!("`{name}` を呼べるのは{}です。これは {s} です", "`{name}` is called on {}; this is {s}", what.ja; what.en));
        } else {
            self.err("E102", at, tr!("`{name}` の一つ目の引数は{}です。これは {s} です", "The first argument of `{name}` is {}; this is {s}", what.ja; what.en));
        }
    }

    /// The list a builtin walks: its element type.
    fn list_arg(&mut self, name: &str, x: &Expr, on: bool) -> Ty {
        match self.expr(x, None) {
            Ty::List(t) => *t,
            Ty::Empty | Ty::Error => Ty::Error,
            other => {
                self.not_on(name, x.span, &other, on, &tr!("リスト", "a list"));
                Ty::Error
            }
        }
    }

    /// A function given to a builtin: its parameters typed `params`, its body's type. A lambda written
    /// there is the builtin's own; a function value, written by a name (of a function, a local, a
    /// parameter, DESIGN 27.1), is read as the lambda that calls it (`wrapped.rs`), and its answer is the
    /// body's type.
    fn lambda(&mut self, name: &str, x: &Expr, params: &[Ty], want: Option<&Ty>) -> Ty {
        let n = params.len();
        let ExprKind::Lambda(ps, body) = &x.kind else {
            if x.path().is_none() {
                self.expr(x, None);
                let shape = if n == 2 { "(acc, x) => …" } else { "x => …" };
                self.err("E109", x.span, tr!("`{name}` に渡す関数は、ラムダ（`{shape}`）か名前で書きます", "A function given to `{name}` is a lambda (`{shape}`) or a name")).notes.push(tr!(
                    "関数の値を選ぶ式は、`let` で名前を付けてから渡してください。",
                    "An expression that picks a function value is named by a `let` first, and given by the name."
                ));
                return Ty::Error;
            }
            self.allow = true;
            let t = self.expr(x, Some(&Ty::Fn(params.to_vec(), Box::new(Ty::Error), None)));
            return match &t {
                Ty::Fn(fp, fr, _) => {
                    if fp.len() != n {
                        self.err("E109", x.span, tr!("`{name}` に渡す関数は、引数を {n} 個取ります", "The function given to `{name}` takes {n} parameters"));
                        return Ty::Error;
                    }
                    // a function of a module is read as the lambda that calls it, the builtin's own
                    if let Some(named) = self.named.get_mut(&x.id) {
                        named.inline = true;
                        let r = named.ret.clone();
                        self.set(x, &r);
                        return r;
                    }
                    for (i, (p, f)) in params.iter().zip(fp).enumerate() {
                        if !same_ty(p, f) {
                            let (ps_, fs) = (self.show(p), self.show(f));
                            let k = i + 1;
                            self.err("E102", x.span, tr!("`{name}` が関数の {k} 番目の引数に渡すのは {ps_} です。この関数の値が取るのは {fs} です", "`{name}` gives argument {k} of the function {ps_}, and the function value takes {fs}")).notes.push(fn_types_note());
                            return Ty::Error;
                        }
                    }
                    (**fr).clone()
                }
                Ty::Error => Ty::Error,
                _ => {
                    let shape = if n == 2 { "(acc, x) => …" } else { "x => …" };
                    self.err("E109", x.span, tr!("`{name}` のこの引数は、`{shape}` の形の関数か、関数の値です", "This argument of `{name}` is a function, written `{shape}`, or a function value"));
                    Ty::Error
                }
            };
        };
        if ps.len() != n {
            self.err("E109", x.span, tr!("`{name}` に渡す関数は、引数を {n} 個取ります", "The function given to `{name}` takes {n} parameters"));
            return Ty::Error;
        }
        // the types written on its parameters are the ones the builtin gives them (DESIGN 27.1)
        if let Some(written) = self.dc.module.ltypes.get(&x.id).cloned() {
            for ((w, (p, sp)), t) in written.iter().zip(ps).zip(params) {
                let Some(w) = w else { continue };
                let wt = self.dc.resolve(w);
                if !same_ty(&wt, t) {
                    let (ws, ts) = (self.show(&wt), self.show(t));
                    self.err("E102", *sp, tr!("ラムダの引数 `{p}` の型 {ws} は、`{name}` が渡す {ts} と違います", "The parameter `{p}` of the lambda is of {ws}, and `{name}` gives it {ts}"));
                }
            }
        }
        self.scopes.push(Vec::new());
        for ((p, sp), t) in ps.iter().zip(params) {
            self.bind(p, t.clone(), *sp, true);
        }
        let t = self.expr(body, want);
        self.scopes.pop();
        self.set(x, &t)
    }

    /// Whether values of `t` can be the keys a builtin orders or holds (DESIGN 24.1): E114 where they
    /// cannot (a record of keys is a key of an order where `record` says so), and, for a key a dict or
    /// a set holds, E103 where it is not rounded.
    fn key(&mut self, t: &Ty, at: Span, what: &Text, held: bool, record: bool) -> bool {
        if t.is_error() {
            return false;
        }
        // a type parameter held to keys (`K: key`, DESIGN 26.2)
        if let Ty::Param(n) = t {
            if self.tparams.iter().any(|p| p.name.text == *n && p.key) {
                return true;
            }
            let n = n.clone();
            let d = self.err("E114", at, tr!("{}は型の引数 `{n}` の値で、鍵にできません", "{} is a value of the type parameter `{n}`, which cannot be a key", crate::diag::ja_before(&what.ja); what.en));
            d.notes.push(names::param_key_note(&n));
            return false;
        }
        let ok = is_key(t, |id| self.is_sum(id)) || (record && self.key_record(t));
        if !ok {
            let s = self.show(t);
            let d = self.err("E114", at, tr!("{}は {s} で、鍵にできません", "{} is {s}, which cannot be a key", crate::diag::ja_before(&what.ja); what.en));
            d.notes.push(names::key_note());
            if record {
                d.notes.push(tr!(
                    "並べ替えの鍵には、鍵にできる型のフィールドだけを持つレコードも書けます。フィールドを宣言の順に比べます。",
                    "A key of an order may be a record whose fields can all be keys too: it is compared field by field, in the order they are declared."
                ));
            }
            return false;
        }
        if held {
            return self.rounded(t, at, what);
        }
        true
    }

    /// Whether a record's fields can all be keys: a key of an order (DESIGN 24.1). A field of a type
    /// parameter held to keys is one (`Pair[K]`, DESIGN 26.2).
    fn key_record(&self, t: &Ty) -> bool {
        let key_param = |t: &Ty| matches!(t, Ty::Param(n) if self.tparams.iter().any(|p| p.name.text == *n && p.key));
        match t {
            Ty::Record(id) => self.fields(id).is_some_and(|fs| !fs.is_empty() && fs.iter().all(|(_, s)| is_key(&s.ty, |i| self.is_sum(i)) || key_param(&s.ty))),
            _ => false,
        }
    }

    /// What a dict or a set holds, as a field holds a value (DESIGN 24.6): every number in it rounded
    /// to a whole number of its unit (a rate to its step); E103 where one is not.
    fn rounded(&mut self, t: &Ty, at: Span, what: &Text) -> bool {
        let Some(n) = unrounded(t) else { return true };
        let unit = if n.is_rate() { "rate".to_string() } else { unit_text(&n.unit) };
        let one = one_of(&n.unit, None);
        let p = crate::diag::ja_before(&what.ja);
        self.err("E103", at, tr!("丸めていない {unit} の値を、{p}に入れています", "An unrounded value of {unit} goes into {}", ; what.en)).notes.push(tr!(
            "辞書と集合は、レコードのフィールドと同じく、受け渡す形で値を持ちます。`half_up(x, {one})` のように、丸め方と刻みを書いてから入れてください。",
            "A dict and a set hold their values as they travel, as a field of a record does: write the way of rounding and the step first, as in `half_up(x, {one})`."
        ));
        false
    }

    /// E109 where a builtin's argument is not a whole number (`at`, `take`, `numbers`, …).
    fn whole_arg(&mut self, name: &str, t: &Ty, at: Span) {
        match t {
            Ty::Num(n) if n.is_number() && !n.ratio && on_step(n.step, Rat::int(1)) => {}
            Ty::Error => {}
            other => {
                let s = self.show(other);
                self.err("E109", at, tr!("`{name}` が取るのは、整数の number です。これは {s} です", "`{name}` takes a whole number; this is {s}"));
            }
        }
    }

    fn builtin(&mut self, e: &Expr, name: &'static str, recv: Option<&Expr>, args: &[Arg], want: Option<&Ty>) -> Ty {
        let on = recv.is_some();
        let num = |t: &Ty| -> Option<NumTy> { t.num().cloned() };
        match name {
            "min" | "max" => {
                let Some(xs) = self.positional(name, recv, args, 2..=2, e) else { return Ty::Error };
                let a = self.expr(xs[0], want);
                let b = self.expr(xs[1], Some(&a));
                match (&a, &b) {
                    (Ty::Error, _) | (_, Ty::Error) => Ty::Error,
                    (Ty::Num(x), Ty::Num(y)) => match self.unify(x, y) {
                        Ok((p, q)) => Ty::Num(NumTy { unit: if p.konst.is_some() { q.unit.clone() } else { p.unit.clone() }, step: share(p.step, q.step), konst: None, ratio: p.ratio && q.ratio }),
                        Err(why) => {
                            let (xs, ys) = (unit_text(&x.unit), unit_text(&y.unit));
                            self.mismatch(e.span, tr!("`{name}` の二つの引数の型が違います（{xs} と {ys}）", "The two arguments of `{name}` are of two types ({xs} and {ys})"), why);
                            Ty::Error
                        }
                    },
                    (Ty::Date, Ty::Date) => Ty::Date,
                    // strings in the order of their code points (DESIGN 25.1)
                    (Ty::Str, Ty::Str) => Ty::Str,
                    // the values of a type parameter held to keys, in the order of keys (DESIGN 26.2)
                    (Ty::Param(x), Ty::Param(y)) if x == y => {
                        self.key(&a, e.span, &tr!("`{name}` が比べる値", "a value `{name}` compares"), false, false);
                        a.clone()
                    }
                    (Ty::Enum(x), Ty::Enum(y)) if self.same_enum(x, y) => {
                        if !self.values(x).is_some_and(|(_, o)| o) {
                            let en = self.named_by(&x.owner, &x.name);
                            self.err("E105", e.span, tr!("列挙 `{en}` の値には順序が無いので、`{name}` を取れません", "The values of the enum `{en}` have no order, so `{name}` takes none of them"));
                        }
                        a.clone()
                    }
                    _ => {
                        let (x, y) = (self.show(&a), self.show(&b));
                        self.err(
                            "E102",
                            e.span,
                            tr!(
                                "`{name}` の二つの引数は、同じ型の数か日付か文字列か順序のある列挙です（{x} と {y}）",
                                "The two arguments of `{name}` are numbers, dates, strings or values of an ordered enum, of one type ({x} and {y})"
                            ),
                        );
                        Ty::Error
                    }
                }
            }
            "mod" => {
                let Some(xs) = self.positional(name, recv, args, 2..=2, e) else { return Ty::Error };
                let (a, b) = (self.expr(xs[0], None), self.expr(xs[1], None));
                // the divisor: a whole number, a constant or a value (DESIGN 24.3)
                let divisor = match &b {
                    Ty::Num(n) if n.is_number() && !n.ratio && on_step(n.step, Rat::int(1)) => n.clone(),
                    Ty::Error => return Ty::Error,
                    other => {
                        let s = self.show(other);
                        self.err("E109", xs[1].span, tr!("`mod` の割る数は、整数の number です。これは {s} です", "The divisor of `mod` is a whole number; this is {s}"));
                        return Ty::Error;
                    }
                };
                match &a {
                    Ty::Num(x) if x.is_number() && !x.ratio && on_step(x.step, Rat::int(1)) => {
                        let step = match divisor.konst {
                            Some(k) if k.is_int() && k.num > 0 => x.step.and_then(|s| share(Some(s), Some(k))),
                            _ => share(x.step, divisor.step),
                        };
                        Ty::Num(NumTy { unit: Unit::number(), step, konst: None, ratio: false })
                    }
                    Ty::Error => Ty::Error,
                    other => {
                        let s = self.show(other);
                        self.err("E109", xs[0].span, tr!("`mod` が取るのは、整数の number です。これは {s} です", "`mod` takes a whole number; this is {s}"));
                        Ty::Error
                    }
                }
            }
            "isqrt" => {
                let Some(xs) = self.positional(name, recv, args, 1..=1, e) else { return Ty::Error };
                match self.expr(xs[0], None) {
                    Ty::Num(x) if x.is_number() => Ty::Num(NumTy::number()),
                    Ty::Error => Ty::Error,
                    other => {
                        let s = self.show(&other);
                        self.err("E109", xs[0].span, tr!("`isqrt` が取るのは number です。これは {s} です", "`isqrt` takes a number; this is {s}"));
                        Ty::Error
                    }
                }
            }
            "allocate" => {
                let Some(xs) = self.positional(name, recv, args, 3..=3, e) else { return Ty::Error };
                let (t, c, s) = (self.expr(xs[0], None), self.expr(xs[1], None), self.expr(xs[2], None));
                let (Some(t), Some(c), Some(s)) = (num(&t), num(&c), num(&s)) else {
                    if ![&t, &c, &s].iter().any(|x| x.is_error()) {
                        self.err(
                            "E109",
                            e.span,
                            tr!("`allocate` の三つの引数は数です（配る額、途中までの合計、全部の合計）", "The three arguments of `allocate` are numbers: the amount, the running total, the whole"),
                        );
                    }
                    return Ty::Error;
                };
                if let Err(why) = self.unify(&c, &s) {
                    self.mismatch(e.span, tr!("`allocate` の途中までの合計と全部の合計は、同じ型です", "The running total and the whole of `allocate` are of one type"), why);
                }
                Ty::Num(NumTy { unit: t.unit.clone(), step: Some(Rat::int(1)), konst: None, ratio: false })
            }
            "up" | "down" | "half_up" | "half_down" | "half_even" => {
                let Some(xs) = self.positional(name, recv, args, 2..=2, e) else { return Ty::Error };
                let (x, g) = (self.expr(xs[0], None), self.expr(xs[1], None));
                let x = match &x {
                    Ty::Num(n) => n.clone(),
                    Ty::Error => return Ty::Error,
                    other => {
                        let s = self.show(other);
                        self.err("E102", xs[0].span, tr!("`{name}` が丸めるのは数です。これは {s} です", "`{name}` rounds a number; this is {s}"));
                        return Ty::Error;
                    }
                };
                let grid = match &g {
                    Ty::Num(n) if n.konst.is_some_and(|v| v.num > 0) => n.clone(),
                    Ty::Error => return Ty::Error,
                    _ => {
                        self.err("E104", xs[1].span, tr!("丸めの刻みは、正の定数で書いてください（`{name}(x, 1USDc)`）", "Write the step of the rounding as a positive constant (`{name}(x, 1USDc)`)"));
                        return Ty::Error;
                    }
                };
                // a ratio is rounded to a number or a rate, as its step says
                if x.ratio {
                    if grid.has_unit() {
                        let u = unit_text(&grid.unit);
                        self.err("E104", xs[1].span, tr!("比を丸める刻みは、number か率です（{u} ではありません）", "The step a ratio is rounded to is a number or a rate, not {u}"));
                        return Ty::Error;
                    }
                    return Ty::Num(NumTy { unit: grid.unit.clone(), step: grid.konst, konst: None, ratio: false });
                }
                match self.unify(&x, &grid) {
                    Ok((_, g)) => Ty::Num(NumTy { unit: x.unit.clone(), step: g.konst, konst: None, ratio: false }),
                    Err(why) => {
                        let u = unit_text(&x.unit);
                        let raw = self.text(xs[1].span);
                        self.mismatch(xs[1].span, tr!("丸めの刻み `{raw}` は {u} の値ではありません", "The step `{raw}` of the rounding is not a value of {u}"), why);
                        Ty::Error
                    }
                }
            }
            "count" => {
                let Some(xs) = self.positional(name, recv, args, 1..=2, e) else { return Ty::Error };
                let t0 = self.expr(xs[0], None);
                let elem = match &t0 {
                    Ty::List(t) => (**t).clone(),
                    // the pairs of a dict, the members of a set (DESIGN 24.3), the code points of a
                    // string (DESIGN 25.2)
                    Ty::Dict(..) | Ty::Set(_) | Ty::Str if xs.len() == 1 => Ty::Error,
                    Ty::Empty | Ty::EmptyDict | Ty::Error => Ty::Error,
                    other => {
                        let what = if xs.len() == 1 { tr!("リスト、辞書、集合、文字列", "a list, a dict, a set or a string") } else { tr!("リスト", "a list") };
                        self.not_on(name, xs[0].span, other, on, &what);
                        Ty::Error
                    }
                };
                if xs.len() == 2 {
                    let t = self.lambda(name, xs[1], &[elem], Some(&Ty::Bool));
                    self.condition(&t, xs[1].span, &tr!("`count` に渡す関数の答え", "the answer of the function given to `count`"));
                }
                Ty::Num(NumTy::number())
            }
            "sum" | "max_of" | "min_of" | "running_totals" => {
                let Some(xs) = self.positional(name, recv, args, 2..=2, e) else { return Ty::Error };
                let elem = self.list_arg(name, xs[0], on);
                let t = self.lambda(name, xs[1], std::slice::from_ref(&elem), None);
                let ok = match &t {
                    Ty::Num(_) | Ty::Error => true,
                    Ty::Date | Ty::Enum(_) => matches!(name, "max_of" | "min_of"),
                    Ty::Param(n) => matches!(name, "max_of" | "min_of") && self.tparams.iter().any(|p| p.name.text == *n && p.key),
                    _ => false,
                };
                if !ok {
                    let s = self.show(&t);
                    self.err("E109", xs[1].span, tr!("`{name}` に渡す関数は数を答えます。これは {s} を答えます", "The function given to `{name}` answers a number; this one answers {s}"));
                    return Ty::Error;
                }
                match name {
                    "sum" => match t {
                        Ty::Num(n) => Ty::Num(NumTy { konst: None, ..n }),
                        other => other,
                    },
                    "running_totals" => {
                        // the weights fill the fields `before`, `upto` and `total` of the records it
                        // answers, which take a rounded value as every field does (DESIGN 3.2, 3.6);
                        // records of a weight said to be unrounded are of no type, so that the
                        // mistake is said once
                        if let (Ty::Num(w), ExprKind::Lambda(_, body)) = (&t, &xs[1].kind) {
                            let said = self.dc.diags.len();
                            self.weight_fits(body, w);
                            if self.dc.diags.len() > said {
                                return Ty::Error;
                            }
                        } else if let Ty::Num(w) = &t {
                            // a function value: what it answers (DESIGN 27.1)
                            let said = self.dc.diags.len();
                            self.weight_num(w, w, xs[1].span);
                            if self.dc.diags.len() > said {
                                return Ty::Error;
                            }
                        }
                        let w = match t {
                            Ty::Num(n) => Ty::Num(NumTy { konst: None, ..n }),
                            other => other,
                        };
                        Ty::List(Box::new(Ty::Record(RecId::Running(Box::new(elem), Box::new(w)))))
                    }
                    _ => {
                        if let Ty::Enum(id) = &t
                            && !self.values(id).is_some_and(|(_, o)| o)
                        {
                            let en = self.named_by(&id.owner, &id.name);
                            self.err(
                                "E105",
                                xs[1].span,
                                tr!("列挙 `{en}` の値には順序が無いので、`{name}` を取れません", "The values of the enum `{en}` have no order, so `{name}` takes none of them"),
                            );
                        }
                        match t {
                            Ty::Num(n) => Ty::Opt(Box::new(Ty::Num(NumTy { konst: None, ..n }))),
                            Ty::Error => Ty::Error,
                            other => Ty::Opt(Box::new(other)),
                        }
                    }
                }
            }
            "any" | "all" => {
                let Some(xs) = self.positional(name, recv, args, 2..=2, e) else { return Ty::Error };
                let elem = self.list_arg(name, xs[0], on);
                let t = self.lambda(name, xs[1], std::slice::from_ref(&elem), Some(&Ty::Bool));
                self.condition(&t, xs[1].span, &tr!("`{name}` に渡す関数の答え", "the answer of the function given to `{name}`"));
                Ty::Bool
            }
            "filter" => {
                let Some(xs) = self.positional(name, recv, args, 2..=2, e) else { return Ty::Error };
                let t0 = self.expr(xs[0], None);
                // a list's elements, or a dict's pairs as entries (DESIGN 24.3)
                let (param, out) = match &t0 {
                    Ty::List(x) => ((**x).clone(), t0.clone()),
                    Ty::Dict(k, v) => (Ty::Record(RecId::Entry(k.clone(), v.clone())), t0.clone()),
                    Ty::Empty | Ty::Error => (Ty::Error, Ty::List(Box::new(Ty::Error))),
                    Ty::EmptyDict => (Ty::Error, Ty::Error),
                    other => {
                        self.not_on(name, xs[0].span, other, on, &tr!("リストか辞書", "a list or a dict"));
                        (Ty::Error, Ty::Error)
                    }
                };
                let t = self.lambda(name, xs[1], &[param], Some(&Ty::Bool));
                self.condition(&t, xs[1].span, &tr!("`{name}` に渡す関数の答え", "the answer of the function given to `{name}`"));
                out
            }
            "first" | "last" => {
                let Some(xs) = self.positional(name, recv, args, 1..=2, e) else { return Ty::Error };
                let elem = self.list_arg(name, xs[0], on);
                if xs.len() == 2 {
                    let t = self.lambda(name, xs[1], std::slice::from_ref(&elem), Some(&Ty::Bool));
                    self.condition(&t, xs[1].span, &tr!("`{name}` に渡す関数の答え", "the answer of the function given to `{name}`"));
                }
                // an element that is none and no element at all are one none (DESIGN 3.6), and so
                // is the type: `first` over a list of `T?` answers `T?`
                opt(elem)
            }
            "map" => {
                let Some(xs) = self.positional(name, recv, args, 2..=2, e) else { return Ty::Error };
                let elem = self.list_arg(name, xs[0], on);
                let inner = match want {
                    Some(Ty::List(t)) => Some((**t).clone()),
                    _ => None,
                };
                let t = self.lambda(name, xs[1], &[elem], inner.as_ref());
                let t = match t {
                    Ty::Num(n) => Ty::Num(NumTy { konst: None, ..n }),
                    other => other,
                };
                Ty::List(Box::new(t))
            }
            "reduce" => {
                let Some(xs) = self.positional(name, recv, args, 3..=3, e) else { return Ty::Error };
                let elem = self.list_arg(name, xs[0], on);
                let init = self.expr(xs[1], want);
                let init = match init {
                    Ty::Num(n) => Ty::Num(NumTy { konst: None, ..n }),
                    other => other,
                };
                let t = self.lambda(name, xs[2], &[init.clone(), elem], Some(&init));
                match self.join(&init, &t) {
                    Some(Ty::Num(n)) => Ty::Num(NumTy { konst: None, ..n }),
                    Some(j) => j,
                    None => {
                        let (a, b) = (self.show(&init), self.show(&t));
                        self.err(
                            "E102",
                            xs[2].span,
                            tr!(
                                "`reduce` の初めの値と、渡す関数の答えの型が違います（{a} と {b}）",
                                "The first value of `reduce` and the answer of the function it is given are of two types ({a} and {b})"
                            ),
                        );
                        Ty::Error
                    }
                }
            }
            "starts_with" => {
                let Some(xs) = self.positional(name, recv, args, 2..=2, e) else { return Ty::Error };
                for x in xs {
                    let t = self.expr(x, Some(&Ty::Str));
                    self.string_arg(name, &t, x.span);
                }
                Ty::Bool
            }
            "concat" => {
                let Some(xs) = self.positional(name, recv, args, 1..=usize::MAX, e) else { return Ty::Error };
                let first_want = match want {
                    Some(t @ Ty::List(_)) => Some(t.clone()),
                    _ => Some(Ty::Str),
                };
                let t0 = self.expr(xs[0], first_want.as_ref());
                if matches!(t0, Ty::List(_) | Ty::Empty) {
                    // lists, one after another (DESIGN 24.3)
                    let mut acc = t0;
                    for x in &xs[1..] {
                        let w = if matches!(acc, Ty::List(_)) { Some(acc.clone()) } else { None };
                        let t = self.expr(x, w.as_ref());
                        acc = match self.join(&acc, &t) {
                            Some(j) if matches!(j, Ty::List(_) | Ty::Empty | Ty::Error) => j,
                            _ => {
                                let (a, b) = (self.show(&acc), self.show(&t));
                                self.err("E102", x.span, tr!("`concat` がつなぐリストの型が違います（{a} と {b}）", "The lists `concat` joins are of two types ({a} and {b})"));
                                return Ty::Error;
                            }
                        };
                    }
                    // lists that are all `[]` are of the list the place says
                    return match acc {
                        Ty::List(t) => Ty::List(Box::new(plain(*t))),
                        Ty::Empty => match want {
                            Some(t @ Ty::List(_)) => t.clone(),
                            _ => Ty::Empty,
                        },
                        other => other,
                    };
                }
                self.string_arg(name, &t0, xs[0].span);
                for x in &xs[1..] {
                    let t = self.expr(x, Some(&Ty::Str));
                    self.string_arg(name, &t, x.span);
                }
                Ty::Str
            }
            "text" => {
                let Some(xs) = self.positional(name, recv, args, 1..=1, e) else { return Ty::Error };
                let t = self.expr(xs[0], None);
                match &t {
                    Ty::Num(n) if n.is_rate() || n.ratio => {
                        self.err("E109", xs[0].span, tr!("`text` は率を文字列にしません", "`text` turns no rate into a string")).notes.push(tr!(
                            "率は刻みの数でやりとりされ、書き方（`3.49%`、`0.0349`）が一通りに決まらないので、`text` は取りません。",
                            "A rate travels as a count of steps, and has no one way to be written (`3.49%`, `0.0349`), so `text` does not take one."
                        ));
                    }
                    Ty::Num(n) if !on_step(n.step, Rat::int(1)) => {
                        self.err("E109", xs[0].span, tr!("`text` は丸めていない値を文字列にしません", "`text` turns no unrounded value into a string"))
                            .notes
                            .push(tr!("丸めてから渡してください（`text(half_up(x, 1USDc))`）。", "Round it first (`text(half_up(x, 1USDc))`)."));
                    }
                    Ty::Enum(id) if !self.is_sum(id) => {}
                    Ty::Num(_) | Ty::Date | Ty::Bool | Ty::Str | Ty::Error => {}
                    other => {
                        let s = self.show(other);
                        self.err("E109", xs[0].span, tr!("`text` は {s} を文字列にしません", "`text` turns no {s} into a string"))
                            .notes
                            .push(tr!("`text` が取るのは、数、お金、量、日付、列挙、bool です。", "`text` takes a number, money, a quantity, a date, an enum or a bool."));
                    }
                }
                Ty::Str
            }
            "numbers" => {
                let Some(xs) = self.positional(name, recv, args, 2..=2, e) else { return Ty::Error };
                for x in xs {
                    let t = self.expr(x, None);
                    self.whole_arg(name, &t, x.span);
                }
                Ty::List(Box::new(Ty::Num(NumTy::number())))
            }
            // `char(10)`: the string of one code point, written as a constant (DESIGN 25.2)
            "char" => {
                let Some(xs) = self.positional(name, recv, args, 1..=1, e) else { return Ty::Error };
                match self.expr(xs[0], None) {
                    Ty::Num(n) if n.is_number() && n.konst.is_some_and(|k| k.is_int() && crate::text::is_scalar(k.num)) => {}
                    Ty::Num(n) if n.is_number() && n.konst.is_some_and(|k| k.is_int()) => {
                        let raw = self.text(xs[0].span);
                        self.err("E109", xs[0].span, tr!("`char` に渡す {raw} は、Unicode のスカラー値ではありません", "`char` is given {raw}, which is no Unicode scalar value")).notes.push(tr!(
                            "スカラー値は 0 から 1114111（U+10FFFF）までの数で、サロゲートの 55296〜57343（U+D800〜U+DFFF）を除きます。",
                            "A scalar value is a number from 0 to 1114111 (U+10FFFF), but for the surrogates, 55296 to 57343 (U+D800 to U+DFFF)."
                        ));
                    }
                    Ty::Error => {}
                    other => {
                        let s = self.show(&other);
                        self.err("E109", xs[0].span, tr!("`char` が取るのは、コードポイントを書いた整数の定数です。これは {s} です", "`char` takes a whole number written as a constant, a code point; this is {s}")).notes.push(tr!(
                            "改行は `char(10)`、タブは `char(9)` のように、コードポイントを数で書いてください。値の数から文字列を作るには `[n].to_text()` を使います。",
                            "Write the code point as a number: a new line is `char(10)`, a tab `char(9)`. A string is made from numbers that are values with `[n].to_text()`."
                        ));
                    }
                }
                Ty::Str
            }
            // `ok(x)` and `err(e)`: a `result` of the type the place says (DESIGN 26.3)
            "ok" | "err" => {
                let Some(xs) = self.positional(name, recv, args, 1..=1, e) else { return Ty::Error };
                match want.map(Ty::present) {
                    Some(Ty::Result(a, er)) => {
                        let place = if name == "ok" { (**a).clone() } else { (**er).clone() };
                        let t = self.expr(xs[0], Some(&place));
                        let w = tr!("`{name}` の値", "the value of `{name}`");
                        self.fits(&t, &place, xs[0].span, &w);
                        Ty::Result(a.clone(), er.clone())
                    }
                    _ => {
                        self.expr(xs[0], None);
                        self.err("E112", e.span, tr!("`{name}(…)` の `result` の型が決まりません", "The type of the `result` `{name}(…)` makes cannot be worked out")).notes.push(tr!(
                            "`ok(x)` と `err(e)` は、置かれた場所の型が `result[T, E]` と言うところに書いてください（関数の答え、型を書いた `let`、引数）。",
                            "Write `ok(x)` and `err(e)` where the place says the type, `result[T, E]`: a function's answer, a `let` with its type, an argument."
                        ));
                        Ty::Error
                    }
                }
            }
            "chars" | "code_points" | "to_text" | "index_of" | "ends_with" | "split" | "words" | "replace" | "trim" | "trim_start" | "trim_end" | "lower_ascii" | "upper_ascii" | "pad_start" | "pad_end" | "parse_number"
            | "parse_date" => self.string_method(e, name, recv, args),
            "get" | "put" | "remove" | "has" | "keys" | "values" | "entries" | "map_values" | "merge" | "merge_with" | "add" | "union" | "intersect" | "difference" | "to_list" => {
                self.collection(e, name, recv, args, want)
            }
            _ => self.list_method(e, name, recv, args, want),
        }
    }

    /// The builtins of a dict and of a set (DESIGN 24.3), written after the dict or the set.
    fn collection(&mut self, e: &Expr, name: &'static str, recv: Option<&Expr>, args: &[Arg], want: Option<&Ty>) -> Ty {
        let on = recv.is_some();
        let n = match name {
            "keys" | "values" | "entries" | "to_list" => 1..=1,
            "put" | "merge_with" => 3..=3,
            _ => 2..=2,
        };
        let Some(xs) = self.positional(name, recv, args, n, e) else { return Ty::Error };
        let dict_only = matches!(name, "get" | "put" | "keys" | "values" | "entries" | "map_values" | "merge" | "merge_with");
        let set_only = matches!(name, "add" | "union" | "intersect" | "difference" | "to_list");
        let place = |w: Option<&Ty>| match w {
            Some(t @ (Ty::Dict(..) | Ty::Set(_))) => Some(t.clone()),
            _ => None,
        };
        let t0 = self.expr(xs[0], place(want).as_ref());
        let t0 = match (&t0, place(want)) {
            (Ty::EmptyDict, Some(w @ Ty::Dict(..))) => w,
            _ => t0,
        };
        let arg = |name: &str, what: (&str, &str)| tr!("`{name}` の{}", "the {} of `{name}`", what.0; what.1);
        match t0.clone() {
            Ty::Dict(k, v) if !set_only => match name {
                "get" | "has" | "remove" => {
                    let kt = self.expr(xs[1], Some(&k));
                    self.fits(&kt, &k, xs[1].span, &arg(name, ("鍵", "key")));
                    match name {
                        "get" => opt(*v),
                        "has" => Ty::Bool,
                        _ => t0,
                    }
                }
                "put" => {
                    let kt = self.expr(xs[1], Some(&k));
                    self.fits(&kt, &k, xs[1].span, &arg(name, ("鍵", "key")));
                    let vt = self.expr(xs[2], Some(&v));
                    self.fits(&vt, &v, xs[2].span, &arg(name, ("値", "value")));
                    t0
                }
                "keys" => Ty::List(k),
                "values" => Ty::List(v),
                "entries" => Ty::List(Box::new(Ty::Record(RecId::Entry(k, v)))),
                "map_values" => {
                    let u = plain(self.lambda(name, xs[1], &[*v], None));
                    if !self.rounded(&u, xs[1].span, &tr!("`map_values` が答える辞書の値", "a value of the dict `map_values` answers")) {
                        return Ty::Error;
                    }
                    Ty::Dict(k, Box::new(u))
                }
                "merge" => {
                    let o = self.expr(xs[1], Some(&t0));
                    self.fits(&o, &t0, xs[1].span, &tr!("`merge` に渡す辞書", "the dict given to `merge`"));
                    t0
                }
                _ => {
                    // merge_with
                    let o = self.expr(xs[1], Some(&t0));
                    self.fits(&o, &t0, xs[1].span, &tr!("`merge_with` に渡す辞書", "the dict given to `merge_with`"));
                    let r = self.lambda(name, xs[2], &[(*v).clone(), (*v).clone()], Some(&v));
                    self.fits(&r, &v, xs[2].span, &tr!("`merge_with` に渡す関数の答え", "the answer of the function given to `merge_with`"));
                    t0
                }
            },
            // `[:].put(k, v)` where no place says the dict's type: the key's and the value's
            Ty::EmptyDict if name == "put" => {
                let kt = plain(self.expr(xs[1], None));
                let vt = plain(self.expr(xs[2], None));
                if !self.key(&kt, xs[1].span, &tr!("辞書の鍵", "a key of a dict"), true, false) || !self.rounded(&vt, xs[2].span, &tr!("辞書の値", "a value of a dict")) {
                    return Ty::Error;
                }
                Ty::Dict(Box::new(kt), Box::new(vt))
            }
            Ty::Set(x) if !dict_only => match name {
                "has" | "add" | "remove" => {
                    let t = self.expr(xs[1], Some(&x));
                    self.fits(&t, &x, xs[1].span, &arg(name, ("要素", "member")));
                    if name == "has" { Ty::Bool } else { t0 }
                }
                "to_list" => Ty::List(x),
                _ => {
                    // union, intersect, difference
                    let o = self.expr(xs[1], Some(&t0));
                    self.fits(&o, &t0, xs[1].span, &tr!("`{name}` に渡す集合", "the set given to `{name}`"));
                    t0
                }
            },
            Ty::Error => {
                // what is given is still checked, a function given with what it takes unknown
                for x in &xs[1..] {
                    match &x.kind {
                        ExprKind::Lambda(ps, _) => {
                            let params = vec![Ty::Error; ps.len()];
                            self.lambda(name, x, &params, None);
                        }
                        _ => {
                            self.expr(x, None);
                        }
                    }
                }
                Ty::Error
            }
            other => {
                let what = if dict_only {
                    tr!("辞書", "a dict")
                } else if set_only {
                    tr!("集合", "a set")
                } else {
                    tr!("辞書か集合", "a dict or a set")
                };
                self.not_on(name, xs[0].span, &other, on, &what);
                Ty::Error
            }
        }
    }

    /// The builtins of a list added by DESIGN 24.3, written after the list.
    fn list_method(&mut self, e: &Expr, name: &'static str, recv: Option<&Expr>, args: &[Arg], want: Option<&Ty>) -> Ty {
        let on = recv.is_some();
        let n = match name {
            "slice" | "to_dict" => 3..=3,
            "append" | "at" | "take" | "drop" | "sort_by" | "sort_by_desc" | "contains" | "flat_map" | "filter_map" | "max_by" | "min_by" | "group_by" | "join" => 2..=2,
            _ => 1..=1,
        };
        let Some(xs) = self.positional(name, recv, args, n, e) else { return Ty::Error };
        // the list the place wants, where the value is `[]`: `[].to_set()` where a set goes
        let lw = match want {
            Some(Ty::Set(t)) if name == "to_set" => Some(Ty::List(t.clone())),
            Some(t @ Ty::List(_)) if matches!(name, "append" | "reverse" | "sort" | "take" | "drop" | "slice" | "distinct") => Some(t.clone()),
            _ => None,
        };
        let t0 = self.expr(xs[0], lw.as_ref());
        // a string's `slice`, `take`, `drop` and `contains` (DESIGN 25.2)
        if matches!(t0, Ty::Str) && matches!(name, "slice" | "take" | "drop" | "contains") {
            return self.string_rest(name, &xs);
        }
        // the element type; None for `[]` where no place says it
        let elem: Option<Ty> = match &t0 {
            Ty::List(x) => Some((**x).clone()),
            Ty::Empty => match &lw {
                Some(Ty::List(x)) => Some((**x).clone()),
                _ => None,
            },
            Ty::Error => Some(Ty::Error),
            other => {
                let what = if matches!(name, "slice" | "take" | "drop" | "contains") { tr!("リストか文字列", "a list or a string") } else { tr!("リスト", "a list") };
                self.not_on(name, xs[0].span, other, on, &what);
                Some(Ty::Error)
            }
        };
        let el = elem.clone().unwrap_or(Ty::Error);
        let list_of = |t: &Ty| if t.is_error() { Ty::Error } else { Ty::List(Box::new(t.clone())) };
        match name {
            "append" => {
                let x = self.expr(xs[1], elem.as_ref());
                match &elem {
                    Some(t) => {
                        self.fits(&x, t, xs[1].span, &tr!("`append` が足す要素", "the element `append` adds"));
                        list_of(t)
                    }
                    None => list_of(&plain(x)),
                }
            }
            "at" | "take" | "drop" | "slice" => {
                for x in &xs[1..] {
                    let t = self.expr(x, None);
                    self.whole_arg(name, &t, x.span);
                }
                if name == "at" { opt(el) } else { list_of(&el) }
            }
            "reverse" => list_of(&el),
            "sort" | "distinct" => {
                let held = name == "distinct";
                self.key(&el, xs[0].span, &tr!("`{name}` が並べる要素", "an element `{name}` orders"), held, false);
                list_of(&el)
            }
            "sort_by" | "sort_by_desc" | "max_by" | "min_by" => {
                let k = plain(self.lambda(name, xs[1], std::slice::from_ref(&el), None));
                self.key(&k, xs[1].span, &tr!("`{name}` の鍵", "the key of `{name}`"), false, true);
                if matches!(name, "max_by" | "min_by") { opt(el) } else { list_of(&el) }
            }
            "contains" => {
                let x = self.expr(xs[1], elem.as_ref());
                if let Some(t) = &elem
                    && self.join(t, &x).is_none()
                {
                    let (a, b) = (self.show(t), self.show(&x));
                    self.err("E102", xs[1].span, tr!("`contains` が探すのは {a} の値です。これは {b} です", "`contains` looks for a value of {a}; this is {b}"));
                }
                Ty::Bool
            }
            "flatten" => match &el {
                Ty::List(u) => list_of(u),
                Ty::Error => Ty::Error,
                other => {
                    let s = self.show(other);
                    self.err("E102", xs[0].span, tr!("`flatten` を呼べるのはリストのリストです。これは {s} のリストです", "`flatten` is called on a list of lists; this is a list of {s}"));
                    Ty::Error
                }
            },
            "flat_map" => {
                let u = self.lambda(name, xs[1], std::slice::from_ref(&el), None);
                match u {
                    Ty::List(w) => list_of(&plain(*w)),
                    Ty::Error => Ty::Error,
                    other => {
                        let s = self.show(&other);
                        self.err("E109", xs[1].span, tr!("`flat_map` に渡す関数はリストを答えます。これは {s} を答えます", "The function given to `flat_map` answers a list; this one answers {s}"));
                        Ty::Error
                    }
                }
            }
            "filter_map" => {
                let u = self.lambda(name, xs[1], std::slice::from_ref(&el), None);
                match u {
                    Ty::Opt(w) => list_of(&plain(*w)),
                    Ty::Error => Ty::Error,
                    Ty::None => {
                        self.err("E102", xs[1].span, tr!("`filter_map` に渡す関数がいつも `none` を答えるので、答えの型が決まりません", "The function given to `filter_map` always answers `none`, which says no type of the answer"));
                        Ty::Error
                    }
                    other => list_of(&plain(other)),
                }
            }
            "indexed" => {
                if !self.rounded(&el, xs[0].span, &tr!("`indexed` が答えるレコードの `item`", "the `item` of the records `indexed` answers")) {
                    return Ty::Error;
                }
                list_of(&Ty::Record(RecId::Indexed(Box::new(el))))
            }
            "to_set" => match &elem {
                Some(t) => {
                    if !self.key(t, xs[0].span, &tr!("集合の要素", "a member of a set"), true, false) {
                        return Ty::Error;
                    }
                    Ty::Set(Box::new(t.clone()))
                }
                None => {
                    self.err("E102", xs[0].span, tr!("`[].to_set()` の集合の型が決まりません", "The type of the set `[].to_set()` makes cannot be told")).notes.push(tr!(
                        "空の集合は、型の決まるところ（関数の答え、型を書いた `let`）に置いてください（`let s : set[string] = [].to_set()`）。",
                        "Put an empty set where its type is said (a function's answer, a `let` with its type): `let s : set[string] = [].to_set()`."
                    ));
                    Ty::Error
                }
            },
            "to_dict" => {
                let k = plain(self.lambda(name, xs[1], std::slice::from_ref(&el), None));
                let v = plain(self.lambda(name, xs[2], std::slice::from_ref(&el), None));
                let ok_k = self.key(&k, xs[1].span, &tr!("`to_dict` の鍵", "the key of `to_dict`"), true, false);
                let ok_v = self.rounded(&v, xs[2].span, &tr!("`to_dict` が答える辞書の値", "a value of the dict `to_dict` answers"));
                if !ok_k || !ok_v || k.is_error() || v.is_error() {
                    return Ty::Error;
                }
                Ty::Dict(Box::new(k), Box::new(v))
            }
            "group_by" => {
                let k = plain(self.lambda(name, xs[1], std::slice::from_ref(&el), None));
                let ok_k = self.key(&k, xs[1].span, &tr!("`group_by` の鍵", "the key of `group_by`"), true, false);
                let ok_v = self.rounded(&el, xs[0].span, &tr!("`group_by` が答える辞書の値", "a value of the dict `group_by` answers"));
                if !ok_k || !ok_v || k.is_error() || el.is_error() {
                    return Ty::Error;
                }
                Ty::Dict(Box::new(k), Box::new(Ty::List(Box::new(el))))
            }
            "join" => {
                if !matches!(el, Ty::Str | Ty::Error) {
                    let s = self.show(&el);
                    self.err("E102", xs[0].span, tr!("`join` がつなぐのは文字列のリストです。これは {s} のリストです", "`join` joins a list of strings; this is a list of {s}"));
                }
                let t = self.expr(xs[1], Some(&Ty::Str));
                self.string_arg(name, &t, xs[1].span);
                Ty::Str
            }
            _ => Ty::Error,
        }
    }

    /// The builtins of a string (DESIGN 25.2), written after the string; `to_text` after a list of
    /// code points.
    fn string_method(&mut self, e: &Expr, name: &'static str, recv: Option<&Expr>, args: &[Arg]) -> Ty {
        let on = recv.is_some();
        let n = match name {
            "replace" | "pad_start" | "pad_end" => 3..=3,
            "index_of" | "ends_with" | "split" => 2..=2,
            _ => 1..=1,
        };
        let Some(xs) = self.positional(name, recv, args, n, e) else { return Ty::Error };
        if name == "to_text" {
            // code points: a list of whole numbers
            match self.expr(xs[0], None) {
                Ty::List(t) => match &*t {
                    Ty::Num(n) if n.is_number() && !n.ratio && on_step(n.step, Rat::int(1)) => {}
                    Ty::Error => {}
                    other => {
                        let s = self.show(other);
                        self.err("E102", xs[0].span, tr!("`to_text` を呼べるのは、コードポイントの整数のリストです。これは {s} のリストです", "`to_text` is called on a list of whole numbers, code points; this is a list of {s}"));
                    }
                },
                Ty::Empty | Ty::Error => {}
                other => self.not_on(name, xs[0].span, &other, on, &tr!("コードポイントの整数のリスト", "a list of whole numbers, code points")),
            }
            return Ty::Opt(Box::new(Ty::Str));
        }
        match self.expr(xs[0], Some(&Ty::Str)) {
            Ty::Str => self.string_rest(name, &xs),
            Ty::Error => {
                for x in &xs[1..] {
                    self.expr(x, None);
                }
                Ty::Error
            }
            other => {
                self.not_on(name, xs[0].span, &other, on, &tr!("文字列", "a string"));
                Ty::Error
            }
        }
    }

    /// The arguments of a builtin of a string after the string, and its answer (DESIGN 25.2).
    fn string_rest(&mut self, name: &str, xs: &[&Expr]) -> Ty {
        // the arguments: strings, and the whole numbers `slice`, `take`, `drop` and the widths of
        // `pad_start` and `pad_end` take
        let whole: &[usize] = match name {
            "slice" => &[1, 2],
            "take" | "drop" | "pad_start" | "pad_end" => &[1],
            _ => &[],
        };
        for (i, x) in xs.iter().enumerate().skip(1) {
            if whole.contains(&i) {
                let t = self.expr(x, None);
                self.whole_arg(name, &t, x.span);
            } else {
                let t = self.expr(x, Some(&Ty::Str));
                self.string_arg(name, &t, x.span);
            }
        }
        match name {
            "chars" | "split" | "words" => Ty::List(Box::new(Ty::Str)),
            "code_points" => Ty::List(Box::new(Ty::Num(NumTy::number()))),
            "index_of" => Ty::Opt(Box::new(Ty::Num(NumTy::number()))),
            "contains" | "ends_with" => Ty::Bool,
            "parse_number" => Ty::Opt(Box::new(Ty::Num(NumTy::number()))),
            "parse_date" => Ty::Opt(Box::new(Ty::Date)),
            _ => Ty::Str,
        }
    }

    /// A dict written out (DESIGN 24.2): its keys of one type that can be a key, each constant key
    /// once (E107), its values of one type; `[:]` the dict the place wants.
    fn dict_literal(&mut self, _e: &Expr, pairs: &[(Expr, Expr)], want: Option<&Ty>) -> Ty {
        let (wk, wv) = match want {
            Some(Ty::Dict(k, v)) => (Some((**k).clone()), Some((**v).clone())),
            _ => (None, None),
        };
        if pairs.is_empty() {
            return match want {
                Some(t @ Ty::Dict(..)) => t.clone(),
                _ => Ty::EmptyDict,
            };
        }
        let mut kt: Option<Ty> = None;
        let mut vt: Option<Ty> = None;
        // where the place says the dict's type, each key and value is held to it; else they are of one
        // type among them
        for (k, v) in pairs {
            let a = self.expr(k, wk.as_ref());
            let b = self.expr(v, wv.as_ref());
            match &wk {
                Some(w) => self.fits(&a, w, k.span, &tr!("辞書の鍵", "a key of the dict")),
                None => {
                    kt = match kt {
                        None => Some(a),
                        Some(o) => match self.join(&o, &a) {
                            Some(j) => Some(j),
                            None => {
                                let (x, y) = (self.show(&o), self.show(&a));
                                self.err("E102", k.span, tr!("辞書の鍵の型がそろっていません（{x} と {y}）", "The keys of the dict are not of one type ({x} and {y})"));
                                return Ty::Error;
                            }
                        },
                    }
                }
            }
            match &wv {
                Some(w) => self.fits(&b, w, v.span, &tr!("辞書の値", "a value of the dict")),
                None => {
                    vt = match vt {
                        None => Some(b),
                        Some(o) => match self.join(&o, &b) {
                            Some(j) => Some(j),
                            None => {
                                let (x, y) = (self.show(&o), self.show(&b));
                                self.err("E102", v.span, tr!("辞書の値の型がそろっていません（{x} と {y}）", "The values of the dict are not of one type ({x} and {y})"));
                                return Ty::Error;
                            }
                        },
                    }
                }
            }
        }
        let k = wk.unwrap_or_else(|| plain(kt.unwrap_or(Ty::Error)));
        let v = wv.unwrap_or_else(|| plain(vt.unwrap_or(Ty::Error)));
        if want.is_none() {
            let at = pairs[0].0.span;
            if !self.key(&k, at, &tr!("辞書の鍵", "a key of a dict"), true, false) || !self.rounded(&v, pairs[0].1.span, &tr!("辞書の値", "a value of a dict")) {
                return Ty::Error;
            }
        }
        // a constant key written twice: the later pair would be the one kept
        let mut seen: Vec<(String, Span)> = Vec::new();
        for (x, _) in pairs {
            let Some(c) = self.constant_key(x, &k) else { continue };
            if seen.iter().any(|(s, _)| *s == c) {
                let raw = self.text(x.span);
                self.err("E107", x.span, tr!("辞書に、同じ鍵 `{raw}` が二度書かれています", "The key `{raw}` is written twice in the dict"))
                    .notes
                    .push(tr!("辞書は、一つの鍵に一つの値を持ちます。どちらかの組を消してください。", "A dict holds one value for a key: take one of the two pairs out."));
            } else {
                seen.push((c, x.span));
            }
        }
        if k.is_error() || v.is_error() {
            return Ty::Error;
        }
        Ty::Dict(Box::new(k), Box::new(v))
    }

    /// A key written as a constant, read in the key's type: the same text for two that are one key.
    fn constant_key(&self, x: &Expr, k: &Ty) -> Option<String> {
        match (&x.kind, self.types.get(x.id)) {
            (ExprKind::Str(s), _) => Some(format!("s:{s}")),
            (ExprKind::Date(d), _) => Some(format!("d:{d}")),
            (ExprKind::Bool(b), _) => Some(format!("b:{b}")),
            (ExprKind::Num(_) | ExprKind::Neg(_), Some(Ty::Num(n))) => {
                let v = n.konst?;
                let to = k.num().map(|t| t.unit.clone()).unwrap_or_else(|| n.unit.clone());
                let r = convert(v, &n.unit, &to).unwrap_or(v);
                Some(format!("n:{r}"))
            }
            _ => match self.refs.get(x.id) {
                Some(Ref::EnumValue(_, v)) => Some(format!("e:{v}")),
                _ => None,
            },
        }
    }

    fn string_arg(&mut self, name: &str, t: &Ty, at: Span) {
        if !matches!(t, Ty::Str | Ty::Error) {
            let s = self.show(t);
            self.err("E102", at, tr!("`{name}` の引数は文字列です。これは {s} です", "The arguments of `{name}` are strings; this is {s}"))
                .notes
                .push(tr!("数や日付は `text(x)` で文字列にしてから渡してください。", "Turn a number or a date into a string with `text(x)` first."));
        }
    }
}

/// What a value of a type parameter can be put to (DESIGN 26.2), said where it is put to something
/// else.
fn param_note() -> Text {
    tr!(
        "型の引数の値は、どの型の値かが呼ぶところで決まるので、総称の関数の中では、渡すこと、レコード・リスト・辞書の値・オプショナルな値・`result` に入れること、`=` と `!=` で比べることだけができます。計算や分け方は、呼ぶ側の具体的な型で書いてください。",
        "What type a value of a type parameter is of is said where the function is called, so a generic function only passes it on, puts it in a record, a list, a dict's values, an optional or a `result`, and compares it with `=` and `!=`; compute with it and split it where its type is one, at the caller."
    )
}

/// The note of E101 for a field a record does not have: the fields it has, or that it has none (a
/// value of a sum with no fields, `Order.cancelled`).
fn fields_note(shown: &str, names: &[String]) -> Text {
    if names.is_empty() {
        return tr!("{shown} にはフィールドがありません。", "{shown} has no fields.");
    }
    let names = names.join(", ");
    tr!("{shown} のフィールドは {names} です。", "The fields of {shown} are {names}.")
}

/// The file an enum is declared in.
fn owner_file(o: &Owner) -> &Path {
    match o {
        Owner::Module(f) | Owner::Rule(f) => f,
    }
}

/// A record's or a value's type with these type arguments.
fn with_args_of(id: &RecId, args: Vec<Ty>) -> RecId {
    match id {
        RecId::Module { file, name, .. } => RecId::Module { file: file.clone(), name: name.clone(), args },
        RecId::Variant { file, name, value, .. } => RecId::Variant { file: file.clone(), name: name.clone(), value: value.clone(), args },
        other => other.clone(),
    }
}

/// The type `t` of a parameter or a field with the type arguments found so far put in, where every one
/// of `names` in it is found: what a value given there is read with (`[]` as a list of it).
fn known(t: &Ty, names: &[String], got: &[Option<Ty>]) -> Option<Ty> {
    let args: Option<Vec<Ty>> = got.iter().cloned().collect();
    match args {
        Some(a) => Some(subst(t, names, &a)),
        None => {
            let partial: Vec<Ty> = got.iter().zip(names).map(|(g, n)| g.clone().unwrap_or(Ty::Param(n.clone()))).collect();
            let s = subst(t, names, &partial);
            (!names.iter().any(|n| mentions(&s, n))).then_some(s)
        }
    }
}

/// Whether the type parameter `n` is in a type.
fn mentions(t: &Ty, n: &str) -> bool {
    match t {
        Ty::Param(x) => x == n,
        Ty::List(x) | Ty::Set(x) | Ty::Opt(x) => mentions(x, n),
        Ty::Dict(a, b) | Ty::Result(a, b) => mentions(a, n) || mentions(b, n),
        Ty::Fn(ps, r, _) => ps.iter().any(|p| mentions(p, n)) || mentions(r, n),
        Ty::Enum(id) => id.args.iter().any(|a| mentions(a, n)),
        Ty::Record(r) => match r {
            RecId::Module { args, .. } | RecId::Variant { args, .. } => args.iter().any(|a| mentions(a, n)),
            RecId::Running(a, b) | RecId::Entry(a, b) => mentions(a, n) || mentions(b, n),
            RecId::Indexed(x) => mentions(x, n),
            _ => false,
        },
        _ => false,
    }
}

/// Whether a type is a constant's (a literal, or literals put together), as far as it holds a number.
fn konst_of(t: &Ty) -> bool {
    match t {
        Ty::Num(n) => n.konst.is_some(),
        Ty::Opt(x) | Ty::List(x) => konst_of(x),
        _ => false,
    }
}

/// The type a type parameter takes from a value of `t` (DESIGN 26.2): the type as a place declares it
/// (a number whole in its unit, a rate on its step, no constant). None where the value is unrounded,
/// which no type parameter takes.
fn type_arg(t: &Ty) -> Option<Ty> {
    Some(match t {
        Ty::Num(n) => {
            if n.ratio || n.step.is_none() {
                return None;
            }
            if n.is_rate() {
                Ty::Num(NumTy { konst: None, ratio: false, ..n.clone() })
            } else if on_step(n.step, Rat::int(1)) || n.step.is_some_and(|s| s.num == 0) {
                Ty::Num(NumTy::of(Unit { step: None, ..n.unit.clone() }))
            } else {
                return None;
            }
        }
        Ty::List(x) => Ty::List(Box::new(type_arg(x)?)),
        Ty::Set(x) => Ty::Set(Box::new(type_arg(x)?)),
        Ty::Opt(x) => Ty::Opt(Box::new(type_arg(x)?)),
        Ty::Dict(k, v) => Ty::Dict(Box::new(type_arg(k)?), Box::new(type_arg(v)?)),
        Ty::Result(a, e) => Ty::Result(Box::new(type_arg(a)?), Box::new(type_arg(e)?)),
        other => other.clone(),
    })
}

/// What E201 says of a rule with no outputs (an `enums` file) where it is called or its outputs are
/// read (DESIGN 2.7): its enums are what a module reads it for.
pub fn no_outputs_note(w: &str) -> Text {
    tr!(
        "出力の無い規則（`enums` のファイル）からは、呼ぶためのコードが生成されません。読めるのはその列挙で、`{w}.<列挙>` と書いて型として使います。",
        "Nothing to call is generated from a rule with no outputs (an `enums` file). What a module reads it for is its enums, written `{w}.<enum>` where a type goes."
    )
}

/// Whether two types are one, a constant's value aside: what a function type's parameters and answer
/// are held to (DESIGN 27.2). A type said wrong already is any type.
fn same_ty(a: &Ty, b: &Ty) -> bool {
    a.is_error() || b.is_error() || plain(a.clone()) == plain(b.clone())
}

/// When two function types are one (E102, DESIGN 27.2).
fn fn_types_note() -> Text {
    tr!(
        "関数の型が合うのは、引数の数と型、答えの型が同じときです（お金と量の単位も同じです）。関数の名前を渡すときは、置かれた場所の引数がその関数の引数に入り、その答えが置かれた場所の答えに入れば渡せます。",
        "Two function types are one where their parameters are as many and of the same types, and their answers of one type (amounts and quantities in one unit). A function given by its name goes where each parameter of the place goes into its own, and its answer into the place's."
    )
}

/// A value that may be none: the type itself where it is one already (DESIGN 3.6).
fn opt(t: Ty) -> Ty {
    match t {
        Ty::Opt(_) | Ty::Error => t,
        other => Ty::Opt(Box::new(other)),
    }
}

/// A type with no constant in it: what a list, a dict or a set built from a value holds.
fn plain(t: Ty) -> Ty {
    match t {
        Ty::Num(n) => Ty::Num(NumTy { konst: None, ..n }),
        Ty::List(x) => Ty::List(Box::new(plain(*x))),
        Ty::Set(x) => Ty::Set(Box::new(plain(*x))),
        Ty::Opt(x) => Ty::Opt(Box::new(plain(*x))),
        Ty::Dict(k, v) => Ty::Dict(Box::new(plain(*k)), Box::new(plain(*v))),
        other => other,
    }
}

/// The first number of `t`, through optionals, lists, dicts and sets, that is not rounded to a whole
/// number of its unit (a rate to a step): what a dict or a set does not hold, as a field does not.
fn unrounded(t: &Ty) -> Option<&NumTy> {
    match t {
        Ty::Num(n) if n.ratio || n.step.is_none() || (!n.is_rate() && !on_step(n.step, Rat::int(1))) => Some(n),
        Ty::Opt(x) | Ty::List(x) | Ty::Set(x) => unrounded(x),
        Ty::Dict(k, v) => unrounded(k).or_else(|| unrounded(v)),
        _ => None,
    }
}
