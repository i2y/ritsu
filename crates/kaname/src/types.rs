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
    /// A field of a record, by its name.
    Field(String),
    /// A record built, by its type.
    Record(RecId),
}

/// What the check gives every expression of a module.
pub struct Typed {
    pub types: Vec<Ty>,
    pub refs: Vec<Ref>,
    /// The type a `let` gives its name, by the id of its value: the value's type, read in the type
    /// written after the name when one is (`let w : mass[oz] = 15lb` names 240 ounces).
    pub bound: BTreeMap<usize, Ty>,
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
    scopes: Vec<Vec<Local>>,
}

/// The check of every function's body and every example of a module whose declarations are read.
pub fn check(dc: Dc<'_>, decls: &Decls) -> (Typed, Vec<Diag>) {
    let n = dc.module.exprs;
    let mut ck = Ck { dc, decls, types: vec![Ty::Error; n], refs: vec![Ref::None; n], bound: BTreeMap::new(), scopes: Vec::new() };
    for (i, f) in ck.dc.module.fns.iter().enumerate() {
        ck.function(i, f);
    }
    for x in &ck.dc.module.examples {
        ck.example(x);
    }
    let Ck { dc, types, refs, bound, .. } = ck;
    (Typed { types, refs, bound }, dc.diags)
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
            Ty::Enum(id) => self.named_by(&id.owner, &id.name),
            Ty::Record(RecId::Module { file, name }) => self.named_by(&Owner::Module(file.clone()), name),
            Ty::Record(RecId::Outputs(f)) => self.named_by(&Owner::Rule(f.clone()), "outputs"),
            Ty::Record(RecId::Element(f, s)) => self.named_by(&Owner::Rule(f.clone()), s),
            Ty::Record(RecId::Running(a, b)) => format!("running[{}, {}]", self.show(a), self.show(b)),
            Ty::List(x) => format!("list[{}]", self.show(x)),
            Ty::Opt(x) => format!("{}?", self.show(x)),
            Ty::Transfer(id) => {
                let book = self.use_of(&id.book).map(str::to_string).unwrap_or_else(|| self.dc.world.books.get(&id.book).map(|b| b.name.clone()).unwrap_or_default());
                format!("{book}.{}.{}", id.transfer, id.op)
            }
            Ty::None => "none".into(),
            Ty::Empty => "[]".into(),
            Ty::Error => "?".into(),
        }
    }

    // ── What records and enums hold ─────────────────────────────────────────────

    /// The fields of a record: each its name and its place.
    fn fields(&self, id: &RecId) -> Option<Vec<(Name, Slot)>> {
        match id {
            RecId::Module { file, name } => {
                let records = if file == self.dc.canon { &self.decls.records } else { &self.dc.world.modules.get(file)?.decls.records };
                let r = records.iter().find(|r| r.name.text == *name)?;
                Some(r.fields.iter().map(|f| (f.name.clone(), f.slot.clone())).collect())
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
            RecId::Running(item, weight) => {
                let named = |w: &str| Name { text: w.to_string(), alias: None, span: Span::default() };
                Some(vec![
                    (named("item"), Slot::of((**item).clone())),
                    (named("before"), Slot::of((**weight).clone())),
                    (named("upto"), Slot::of((**weight).clone())),
                    (named("total"), Slot::of((**weight).clone())),
                ])
            }
        }
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

    fn function(&mut self, i: usize, f: &FnDecl) {
        let info = self.decls.fns[i].clone();
        self.scopes = vec![Vec::new()];
        for p in &info.params {
            let n = p.name.text.clone();
            self.bind(&n, p.slot.ty.clone(), p.name.span, false);
            if self.dc.uses.contains_key(&n) {
                self.err("E006", p.name.span, tr!("`{n}` は `use` で読んだものの名前です", "`{n}` is the name of what a `use` reads")).notes.push(tr!(
                    "`{n}(…)` や `{n}.…` がどちらを指すのか、読む人に分からなくなります。別の名前を付けてください。",
                    "A reader of `{n}(…)` or `{n}.…` could not tell which one it means: choose another name."
                ));
            }
        }
        let want = info.ret.ty.clone();
        self.block(&f.body, Some(&want));
        let fun = &f.name.text;
        self.fits_at(&f.body.answer, &want, &tr!("関数 `{fun}` の答え", "the answer of `{fun}`"));
        self.scopes.clear();
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
        self.call_fn(&info, args, callee.span, &x.call);
        self.set(&x.call, &info.ret.ty);
        if let Expect::Value(want) = &x.expect {
            let got = self.expr(want, Some(&info.ret.ty));
            if let Err(why) = self.agrees(&got, &info.ret.ty) {
                let (g, w, fun) = (self.show(&got), self.show(&info.ret.ty), info.name.text.clone());
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
            (Ty::None, Ty::Opt(_)) | (Ty::Empty, Ty::List(_)) => Ok(()),
            (Ty::Opt(g), Ty::Opt(w)) => self.agrees(g, w),
            (g, Ty::Opt(w)) => self.agrees(g, w),
            (Ty::List(g), Ty::List(w)) => self.agrees(g, w),
            (Ty::Num(g), Ty::Num(w)) => match self.num_into(g, w) {
                Ok(_) => Ok(()),
                Err(_) => Err(None),
            },
            (Ty::Enum(g), Ty::Enum(w)) if self.same_enum(g, w) => Ok(()),
            (g, w) if g == w => Ok(()),
            _ => Err(None),
        }
    }

    // ── Blocks ──────────────────────────────────────────────────────────────────

    fn block(&mut self, b: &Block, want: Option<&Ty>) -> Ty {
        self.scopes.push(Vec::new());
        for l in &b.lets {
            let ty = match &l.ty {
                Some(t) => {
                    let declared = self.dc.resolve(t);
                    let got = self.expr(&l.value, Some(&declared));
                    self.as_declared(&got, &declared, l.value.span, &l.name)
                }
                None => self.expr(&l.value, None),
            };
            if matches!(ty, Ty::None | Ty::Empty) {
                let n = &l.name;
                self.err("E102", l.value.span, tr!("`let {n}` の型が決まりません", "The type of `let {n}` cannot be told"))
                    .notes
                    .push(tr!("`none` や `[]` だけでは型が分かりません。`let {n} : <型> = …` と型を書いてください。", "`none` or `[]` alone says no type: write it, as in `let {n} : <type> = …`."));
            }
            self.bound.insert(l.value.id, ty.clone());
            self.bind(&l.name, ty, l.span, true);
        }
        let t = self.expr(&b.answer, want);
        self.scopes.pop();
        t
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
        Some(NumTy { unit: Unit { step: None, ..to.clone() }, step: n.step.map(|s| s.mul(f)), konst: Some(x), ratio: false })
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
            (Ty::None, Ty::Opt(_)) | (Ty::Empty, Ty::List(_)) => {}
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
            (Ty::Num(g), Ty::Num(w)) => self.num_fits(g, w, at, place),
            (Ty::None, w) => {
                let w = self.show(w);
                self.err("E102", at, tr!("{}は {w} で、`none` は入りません", "{} is {w}, which `none` is not", crate::diag::ja_before(&place.ja); place.en));
            }
            (Ty::Enum(g), Ty::Enum(w)) if self.same_enum(g, w) => {}
            (g, w) if g == w => {}
            (g, w) => {
                let (g, w) = (self.show(g), self.show(w));
                self.err("E102", at, tr!("{}は {w} です。渡しているのは {g} です", "{} is {w}; this is {g}", crate::diag::ja_before(&place.ja); place.en));
            }
        }
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
                self.num_fits(&got, &want, e.span, &tr!("`running_totals` の重み", "the weight of `running_totals`"));
            }
        }
    }

    // ── Expressions ─────────────────────────────────────────────────────────────

    pub fn expr(&mut self, e: &Expr, want: Option<&Ty>) -> Ty {
        let t = self.expr_inner(e, want);
        self.set(e, &t)
    }

    fn expr_inner(&mut self, e: &Expr, want: Option<&Ty>) -> Ty {
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
            ExprKind::Record(path, fields) => self.record(e, path, fields),
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
            ExprKind::Neg(x) => {
                let t = self.expr(x, want);
                match t {
                    Ty::Num(n) if !n.unit.compares_only() => Ty::Num(NumTy { konst: n.konst.map(|v| Rat::zero().sub(v)), ..n }),
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
                let a = self.block(t, want);
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
            ExprKind::Match(s, arms) => self.matching(e, s, arms, want),
            ExprKind::Lambda(..) => {
                self.err("E109", e.span, tr!("関数を値にできるのは、組み込みの関数の引数だけです", "A function is a value only as an argument of a builtin")).notes.push(tr!(
                    "`sum(xs, x => …)` のように、組み込みの関数に渡すときだけ `x => …` と書けます。ほかで使う計算は、`fn` で名前を付けてください。",
                    "`x => …` is written only where a builtin takes it, as in `sum(xs, x => …)`; a computation used elsewhere is a `fn` with a name."
                ));
                Ty::Error
            }
            ExprKind::Block(b) => self.block(b, want),
        }
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
                if self.decls.fns.iter().any(|f| f.name.is(w)) {
                    self.err("E107", e.span, tr!("`{w}` は関数です。引数を渡して `{w}(…)` と呼びます", "`{w}` is a function, called with its arguments: `{w}(…)`"));
                    return Ty::Error;
                }
                if crate::kw::is_builtin(w) {
                    self.err("E109", e.span, tr!("`{w}` は組み込みの関数です。`{w}(…)` と呼びます", "`{w}` is a builtin function, called: `{w}(…)`"));
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

    /// `a.b`: a value of an enum (`tier.gold`, `charges.band.low`), or a field of a record.
    fn field(&mut self, e: &Expr, base: &Expr, f: &str, at: Span, want: Option<&Ty>) -> Ty {
        let _ = want;
        if let Some(path) = e.path()
            && self.lookup(&path[0].0).is_none()
            && let Some(t) = self.enum_value(e, &path)
        {
            return t;
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
                    let names = self.fields(id).unwrap_or_default().iter().map(|(n, _)| format!("`{}`", n.text)).collect::<Vec<_>>().join(", ");
                    self.err("E101", at, tr!("{r} にフィールド `{f}` はありません", "There is no field `{f}` in {r}"))
                        .notes
                        .push(tr!("{r} のフィールドは {names} です。", "The fields of {r} are {names}."));
                    Ty::Error
                }
            },
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
                self.err("E105", at, tr!("{s} の値にフィールドはありません（`.{f}`）", "A value of {s} has no fields (`.{f}`)"));
                Ty::Error
            }
        }
    }

    /// A value of an enum named with its enum: `tier.gold`, `charges.band.low`, `pricing.tier.gold`.
    fn enum_value(&mut self, e: &Expr, path: &[(String, Span)]) -> Option<Ty> {
        let words: Vec<&str> = path.iter().map(|(w, _)| w.as_str()).collect();
        let (id, v) = match words.as_slice() {
            [en, v] => {
                let x = self.decls.enums.iter().find(|x| x.name.is(en))?;
                (EnumId { owner: Owner::Module(self.dc.canon.to_path_buf()), name: x.name.text.clone() }, *v)
            }
            [u, en, v] => match self.dc.uses.get(*u).map(|(_, t)| t) {
                Some(Target::Rule { canon, facts, .. }) => {
                    let x = facts.enums.iter().find(|x| names::rule_enum_is(x, en))?;
                    (EnumId { owner: Owner::Rule(canon.clone()), name: x.name.clone() }, *v)
                }
                Some(Target::Module(c)) => {
                    let x = c.decls.enums.iter().find(|x| x.name.is(en))?;
                    (EnumId { owner: Owner::Module(c.canon.clone()), name: x.name.text.clone() }, *v)
                }
                _ => return None,
            },
            _ => return None,
        };
        let (values, _) = self.values(&id)?;
        match values.iter().find(|x| x.is(v)) {
            Some(x) => {
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
            (Ty::Opt(x), Ty::Opt(y)) => Some(Ty::Opt(Box::new(self.join(x, y)?))),
            (Ty::Opt(x), y) | (y, Ty::Opt(x)) => Some(Ty::Opt(Box::new(self.join(x, y)?))),
            (Ty::List(x), Ty::List(y)) => Some(Ty::List(Box::new(self.join(x, y)?))),
            (Ty::Num(x), Ty::Num(y)) => {
                let (x, y) = self.unify(x, y).ok()?;
                Some(Ty::Num(NumTy { unit: x.unit.clone(), step: share(x.step, y.step), konst: None, ratio: x.ratio && y.ratio }))
            }
            (Ty::Enum(x), Ty::Enum(y)) if self.same_enum(x, y) => Some(a.clone()),
            (x, y) if x == y => Some(x.clone()),
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
            let ta = self.expr(a, None);
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
                self.err("E105", e.span, tr!("{x} と {y} のあいだに `{o}` は使えません", "There is no `{o}` between {x} and {y}"));
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
            (Ty::Bool, Ty::Bool) | (Ty::Str, Ty::Str) => {
                let s = self.show(ta);
                self.err("E105", e.span, tr!("{s} の値は、`=` と `!=` でだけ比べられます", "A value of {s} compares with `=` and `!=` only"))
                    .notes
                    .push(tr!("文字列の大小は、言語ごとに照合の順序が違うので持ちません。", "kaname has no order of strings: the generated languages collate them in different orders."));
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
        Ty::Num(NumTy { unit: Unit { step: None, ..to }, step: n.step.and_then(|s| s.checked_mul(f)), konst: n.konst.map(|v| v.mul(f)), ratio: false })
    }

    // ── match ───────────────────────────────────────────────────────────────────

    fn matching(&mut self, e: &Expr, s: &Expr, arms: &[Arm], want: Option<&Ty>) -> Ty {
        let st = self.expr(s, None);
        // what the arms cover, and the values the type has
        let (all, kind): (Vec<String>, &str) = match &st {
            Ty::Error => (vec![], "error"),
            Ty::Enum(id) => match self.values(id) {
                Some((vs, _)) => (vs.iter().map(|v| v.text.clone()).collect(), "enum"),
                None => (vec![], "error"),
            },
            Ty::Bool => (vec!["true".into(), "false".into()], "bool"),
            Ty::Opt(_) | Ty::None => (vec!["none".into(), "some".into()], "opt"),
            other => {
                let t = self.show(other);
                self.err("E105", s.span, tr!("{t} の値は `match` で分けられません", "A value of {t} is not split with `match`")).notes.push(tr!(
                    "`match` が分けるのは、列挙、bool、値が無いことがある値（`none` と `some x`）です。数の場合分けは `if` で書いてください。",
                    "`match` splits an enum, a bool and a value that may be none (`none` and `some x`); split numbers with `if`."
                ));
                (vec![], "error")
            }
        };
        let mut covered: BTreeSet<String> = BTreeSet::new();
        let mut out: Option<Ty> = None;
        for arm in arms {
            let mut bound: Option<(String, Span, Ty)> = None;
            let mut new = false;
            for p in &arm.patterns {
                let key: Option<String> = match (p, kind) {
                    (_, "error") => None,
                    (Pattern::None(_), "opt") => Some("none".into()),
                    (Pattern::Some(x, sp), "opt") => {
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
                    (Pattern::Bool(b, _), "bool") => Some(b.to_string()),
                    (Pattern::Value(path, sp), "enum") => {
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
                            Some(x) if en_ok => Some(x.text),
                            _ => {
                                let en = self.named_by(&id.owner, &id.name);
                                let shown = path.iter().map(|(w, _)| w.as_str()).collect::<Vec<_>>().join(".");
                                self.err("E101", *sp, tr!("`{shown}` は列挙 `{en}` の値ではありません", "`{shown}` is not a value of the enum `{en}`"));
                                None
                            }
                        }
                    }
                    (p, _) => {
                        let t = self.show(&st);
                        self.err("E102", p.span(), tr!("この分岐は {t} の値ではありません", "This arm is not a value of {t}"));
                        None
                    }
                };
                if let Some(k) = key {
                    if covered.insert(k.clone()) {
                        new = true;
                    } else {
                        self.err("E106", p.span(), tr!("`{k}` は、上の分岐がもう受けています", "An arm above takes `{k}` already"))
                            .notes
                            .push(tr!("どの値も、一つの分岐だけが受けます。この値を消すか、上の分岐から外してください。", "Each value goes to one arm: take it out of this arm or of the one above."));
                    }
                }
            }
            let _ = new;
            self.scopes.push(Vec::new());
            if let Some((x, sp, t)) = bound {
                self.bind(&x, t, sp, true);
            }
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
        let missing: Vec<String> = all.iter().filter(|v| !covered.contains(*v)).map(|v| if v == "some" { "some x".into() } else { v.clone() }).collect();
        if !missing.is_empty() && kind != "error" {
            let m = missing.iter().map(|v| format!("`{v}`")).collect::<Vec<_>>().join(", ");
            self.err("E106", e.span, tr!("`match` が受けない値があります: {m}", "The `match` leaves values out: {m}"))
                .notes
                .push(tr!("どの値にも答えるよう、受けない値の分岐を足してください。", "Add an arm for each value left out, so that every value has an answer."));
        }
        out.unwrap_or(Ty::Error)
    }

    // ── Records ─────────────────────────────────────────────────────────────────

    fn record(&mut self, e: &Expr, path: &[(String, Span)], inits: &[FieldInit]) -> Ty {
        let t = self.dc.named(path);
        let id = match &t {
            Ty::Record(id @ (RecId::Module { .. } | RecId::Element(..))) => id.clone(),
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
        self.refer(e, Ref::Record(id.clone()));
        let fields = self.fields(&id).unwrap_or_default();
        let shown = self.show(&t);
        let mut given: BTreeMap<String, Span> = BTreeMap::new();
        for fi in inits {
            match fields.iter().find(|(n, _)| n.is(&fi.name)) {
                Some((n, slot)) => {
                    if given.insert(n.text.clone(), fi.span).is_some() {
                        let f = &fi.name;
                        self.err("E107", fi.span, tr!("フィールド `{f}` が二度書かれています", "The field `{f}` is given twice"));
                    }
                    let got = self.expr(&fi.value, Some(&slot.ty));
                    let f = &n.text;
                    self.fits(&got, &slot.ty, fi.value.span, &tr!("{shown} のフィールド `{f}`", "the field `{f}` of {shown}"));
                }
                None => {
                    self.expr(&fi.value, None);
                    let f = &fi.name;
                    let names = fields.iter().map(|(n, _)| format!("`{}`", n.text)).collect::<Vec<_>>().join(", ");
                    self.err("E101", fi.span, tr!("{shown} にフィールド `{f}` はありません", "There is no field `{f}` in {shown}"))
                        .notes
                        .push(tr!("{shown} のフィールドは {names} です。", "The fields of {shown} are {names}."));
                }
            }
        }
        let missing: Vec<String> = fields.iter().filter(|(n, s)| !given.contains_key(&n.text) && !matches!(s.ty, Ty::Opt(_))).map(|(n, _)| format!("`{}`", n.text)).collect();
        if !missing.is_empty() {
            let m = missing.join(", ");
            self.err("E107", e.span, tr!("{shown} を作るのに、フィールドが足りません: {m}", "Building {shown} leaves out fields: {m}")).notes.push(tr!(
                "レコードはフィールドを全部書いて作ります。値が無いことがある（`T?` の）フィールドだけは省けて、省けば `none` です。",
                "A record is built with every field; only a field that may be none (`T?`) may be left out, and is then `none`."
            ));
        }
        t
    }

    // ── Calls ───────────────────────────────────────────────────────────────────

    fn call(&mut self, e: &Expr, callee: &Expr, args: &[Arg], want: Option<&Ty>) -> Ty {
        let Some(path) = callee.path() else {
            self.expr(callee, None);
            self.err("E105", callee.span, tr!("呼び出せるのは、関数、規則、日付、振替、組み込みの関数です", "What is called is a function, a rule, a date, a transfer or a builtin"));
            return Ty::Error;
        };
        let words: Vec<&str> = path.iter().map(|(w, _)| w.as_str()).collect();
        // a local is never called (kaname has no function values), so a call names a function, a rule
        // or a builtin even where a local has the same name (`let count = count(lines)`)
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
                    self.call_fn(&info, args, callee.span, e);
                    return info.ret.ty;
                }
                match self.dc.uses.get(*w).map(|(_, t)| t.clone()) {
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
                    return self.builtin(e, b, args, want);
                }
                self.err("E101", callee.span, tr!("`{w}` という関数はありません", "There is no function `{w}`"));
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
                    self.refer(callee, Ref::Fn(c.canon.clone(), info.name.text.clone()));
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

    /// Arguments written by name, held to the places they go to: E107 (or `code`) for one that is
    /// not given, given twice, or given and not taken.
    /// Arguments written by name, held to the places they go to: E107 (or `code`) for one that is
    /// not given, given twice, given with no name, or given and not taken. `word` is what an
    /// argument is called (an argument, a rule's input), one and many.
    fn named_args(&mut self, params: &[(String, Slot, bool)], args: &[Arg], what: &Text, word: (&Text, &Text), e: &Expr, code: &'static str) {
        let mut given: BTreeSet<String> = BTreeSet::new();
        let w = crate::diag::ja_before(&what.ja);
        let (one, many) = word;
        let mut positional = false;
        for a in args {
            match a {
                Arg::Pos(x) => {
                    self.expr(x, None);
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
                        let got = self.expr(x, Some(&slot.ty));
                        let place = tr!("{w}の{} `{p}`", "the {} `{p}` of {}", one.ja; one.en, what.en);
                        self.fits(&got, &slot.ty, x.span, &place);
                    }
                    None => {
                        self.expr(x, None);
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

    /// The arguments of a builtin, in order: E107 for named ones, or too few or too many.
    fn positional<'x>(&mut self, name: &str, args: &'x [Arg], n: std::ops::RangeInclusive<usize>, e: &Expr) -> Option<Vec<&'x Expr>> {
        let mut out = Vec::new();
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
            let (lo, hi) = (*n.start(), *n.end());
            let want = if lo == hi {
                tr!("{lo} 個", "{lo}")
            } else if hi == usize::MAX {
                tr!("{lo} 個以上", "{lo} or more")
            } else {
                tr!("{lo} 個か {hi} 個", "{lo} or {hi}")
            };
            let got = out.len();
            self.err("E107", e.span, tr!("`{name}` が取る引数は{}です。渡しているのは {got} 個です", "`{name}` takes {} arguments, and is given {got}", want.ja; want.en));
            for x in &out {
                if !matches!(x.kind, ExprKind::Lambda(..)) {
                    self.expr(x, None);
                }
            }
            return None;
        }
        Some(out)
    }

    /// The list a builtin walks: its element type.
    fn list_arg(&mut self, name: &str, x: &Expr) -> Ty {
        match self.expr(x, None) {
            Ty::List(t) => *t,
            Ty::Empty | Ty::Error => Ty::Error,
            other => {
                let s = self.show(&other);
                self.err("E102", x.span, tr!("`{name}` の一つ目の引数はリストです。これは {s} です", "The first argument of `{name}` is a list; this is {s}"));
                Ty::Error
            }
        }
    }

    /// A function given to a builtin: its parameters typed `params`, its body's type.
    fn lambda(&mut self, name: &str, x: &Expr, params: &[Ty], want: Option<&Ty>) -> Ty {
        let ExprKind::Lambda(ps, body) = &x.kind else {
            self.expr(x, None);
            let n = params.len();
            let shape = if n == 2 { "(acc, x) => …" } else { "x => …" };
            self.err("E109", x.span, tr!("`{name}` のこの引数は、`{shape}` の形の関数です", "This argument of `{name}` is a function, written `{shape}`"));
            return Ty::Error;
        };
        if ps.len() != params.len() {
            let n = params.len();
            self.err("E109", x.span, tr!("`{name}` に渡す関数は、引数を {n} 個取ります", "The function given to `{name}` takes {n} parameters"));
            return Ty::Error;
        }
        self.scopes.push(Vec::new());
        for ((p, sp), t) in ps.iter().zip(params) {
            self.bind(p, t.clone(), *sp, true);
        }
        let t = self.expr(body, want);
        self.scopes.pop();
        self.set(x, &t)
    }

    fn builtin(&mut self, e: &Expr, name: &'static str, args: &[Arg], want: Option<&Ty>) -> Ty {
        let num = |t: &Ty| -> Option<NumTy> { t.num().cloned() };
        match name {
            "min" | "max" => {
                let Some(xs) = self.positional(name, args, 2..=2, e) else { return Ty::Error };
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
                                "`{name}` の二つの引数は、同じ型の数か日付か順序のある列挙です（{x} と {y}）",
                                "The two arguments of `{name}` are numbers, dates or values of an ordered enum, of one type ({x} and {y})"
                            ),
                        );
                        Ty::Error
                    }
                }
            }
            "mod" => {
                let Some(xs) = self.positional(name, args, 2..=2, e) else { return Ty::Error };
                let (a, b) = (self.expr(xs[0], None), self.expr(xs[1], None));
                let n = match &b {
                    Ty::Num(n) if n.is_number() && n.konst.is_some_and(|v| v.is_int() && v.num > 0) => n.konst,
                    Ty::Error => return Ty::Error,
                    _ => {
                        self.err("E109", xs[1].span, tr!("`mod` の割る数は、正の整数の定数で書いてください（`mod(x, 7)`）", "The divisor of `mod` is a positive whole constant (`mod(x, 7)`)"))
                            .notes
                            .push(tr!(
                                "割る数が変わる余りは、言語ごとに負の値の扱いが違うので持ちません（rulec と同じです）。",
                                "A remainder by a value that varies is where the languages disagree below zero, so kaname has none (as rulec has none)."
                            ));
                        return Ty::Error;
                    }
                };
                match &a {
                    Ty::Num(x) if x.is_number() && !x.ratio && on_step(x.step, Rat::int(1)) => {
                        Ty::Num(NumTy { unit: Unit::number(), step: x.step.zip(n).and_then(|(s, k)| share(Some(s), Some(k))), konst: None, ratio: false })
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
                let Some(xs) = self.positional(name, args, 1..=1, e) else { return Ty::Error };
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
                let Some(xs) = self.positional(name, args, 3..=3, e) else { return Ty::Error };
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
                let Some(xs) = self.positional(name, args, 2..=2, e) else { return Ty::Error };
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
                let Some(xs) = self.positional(name, args, 1..=2, e) else { return Ty::Error };
                let elem = self.list_arg(name, xs[0]);
                if xs.len() == 2 {
                    let t = self.lambda(name, xs[1], &[elem], Some(&Ty::Bool));
                    self.condition(&t, xs[1].span, &tr!("`count` に渡す関数の答え", "the answer of the function given to `count`"));
                }
                Ty::Num(NumTy::number())
            }
            "sum" | "max_of" | "min_of" | "running_totals" => {
                let Some(xs) = self.positional(name, args, 2..=2, e) else { return Ty::Error };
                let elem = self.list_arg(name, xs[0]);
                let t = self.lambda(name, xs[1], std::slice::from_ref(&elem), None);
                let ok = match &t {
                    Ty::Num(_) | Ty::Error => true,
                    Ty::Date | Ty::Enum(_) => matches!(name, "max_of" | "min_of"),
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
            "any" | "all" | "filter" | "first" => {
                let Some(xs) = self.positional(name, args, 2..=2, e) else { return Ty::Error };
                let elem = self.list_arg(name, xs[0]);
                let t = self.lambda(name, xs[1], std::slice::from_ref(&elem), Some(&Ty::Bool));
                self.condition(&t, xs[1].span, &tr!("`{name}` に渡す関数の答え", "the answer of the function given to `{name}`"));
                match name {
                    "any" | "all" => Ty::Bool,
                    "filter" => Ty::List(Box::new(elem)),
                    // an element that is none and no element at all are one none (DESIGN 3.6), and so
                    // is the type: `first` over a list of `T?` answers `T?`
                    _ => match elem {
                        Ty::Opt(_) => elem,
                        other => Ty::Opt(Box::new(other)),
                    },
                }
            }
            "map" => {
                let Some(xs) = self.positional(name, args, 2..=2, e) else { return Ty::Error };
                let elem = self.list_arg(name, xs[0]);
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
                let Some(xs) = self.positional(name, args, 3..=3, e) else { return Ty::Error };
                let elem = self.list_arg(name, xs[0]);
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
                let Some(xs) = self.positional(name, args, 2..=2, e) else { return Ty::Error };
                for x in xs {
                    let t = self.expr(x, Some(&Ty::Str));
                    self.string_arg(name, &t, x.span);
                }
                Ty::Bool
            }
            "concat" => {
                let Some(xs) = self.positional(name, args, 1..=usize::MAX, e) else { return Ty::Error };
                for x in xs {
                    let t = self.expr(x, Some(&Ty::Str));
                    self.string_arg(name, &t, x.span);
                }
                Ty::Str
            }
            "text" => {
                let Some(xs) = self.positional(name, args, 1..=1, e) else { return Ty::Error };
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
                    Ty::Num(_) | Ty::Date | Ty::Enum(_) | Ty::Bool | Ty::Str | Ty::Error => {}
                    other => {
                        let s = self.show(other);
                        self.err("E109", xs[0].span, tr!("`text` は {s} を文字列にしません", "`text` turns no {s} into a string"))
                            .notes
                            .push(tr!("`text` が取るのは、数、お金、量、日付、列挙、bool です。", "`text` takes a number, money, a quantity, a date, an enum or a bool."));
                    }
                }
                Ty::Str
            }
            _ => Ty::Error,
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

/// The file an enum is declared in.
fn owner_file(o: &Owner) -> &Path {
    match o {
        Owner::Module(f) | Owner::Rule(f) => f,
    }
}
