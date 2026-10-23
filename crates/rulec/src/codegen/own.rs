//! The names the generated code of each target gives itself, beside the rule's own (§15.265).
//!
//! The code a target is written in declares names of its own: the type of the rows that matched,
//! of the answer and of one element, the errors, the brand of each unit, the helpers that round,
//! that compare the values of an ordered enum and that write a date, the constants of a machine;
//! and, in the `.proto`, its messages, its service, the field the rows that matched travel in and
//! the zero value of every enum. The rule's functions read some of them, and some of the
//! language's builtins as well, and they keep locals of their own beside the rule's values. A
//! name of the rule's that a target spells as one of these is declared twice there, or hides,
//! inside the rule's functions, the one they read: the code does not build, or answers otherwise.
//!
//! Only a name the target writes for this rule is here: the helper that rounds half to even is
//! read only by a rule that rounds so, and `Output` is declared only for a rule of two outputs or
//! more. Where the generators decide a name by what the rule has, it is asked of them: the locals
//! of a fold's walk (`Gen::fold_names`), the flag beside a `max` (`Gen::ident`), the columns the
//! SQL query binds (`Gen::sql_bound`). The test below holds the lists to the code the generators
//! write for every rule of the corpus, and `tests/own_names.rs` holds the warnings to the runs.

use super::spellings::Target;
use super::{camel, mode_fn, pascal, pub_name, Gen};
use crate::ast::{BinOp, Cell, Expr, Item, Lit, RuleFile};
use crate::num::RoundMode;
use crate::types::{Checked, Ty};

/// What a name of the target's is, for the note that says what a name of the rule's meets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum What {
    /// A function the generated code defines to help the rule's own: the rounding, a date, JSON.
    Helper,
    /// A type the generated code declares: the rows that matched, the answer, an element, an error,
    /// the brand of a unit, an enum.
    Type,
    /// A constant the generated code declares: the values of a group, a machine's states.
    Constant,
    /// A name the language has before any code is written.
    Builtin,
    /// The parameter the rule's function takes its inputs in (Go's `in`).
    Parameter,
    /// A local variable the generated code keeps for itself.
    Local,
    /// A column the SQL query keeps for itself.
    Column,
    /// A name the module imports.
    Import,
}

impl What {
    /// "a helper function", in the output language.
    pub(super) fn said(self) -> String {
        match self {
            What::Helper => tr!("補助関数", "a helper function"),
            What::Type => tr!("型", "a type"),
            What::Constant => tr!("定数", "a constant"),
            What::Builtin => tr!("言語の組み込みの名前", "a name the language has built in"),
            What::Parameter => tr!("関数の引数", "the function's parameter"),
            What::Local => tr!("生成コードの局所変数", "a local variable of the generated code"),
            What::Column => tr!("問い合わせの列", "a column of the query"),
            What::Import => tr!("読み込んだ名前", "a name it imports"),
        }
    }
}

/// Where in the rule's functions a name is read. It matters to Go and Rust, whose locals hide a
/// name only from where they are declared: a definition hides what the items after it and the
/// return read. Every other language holds a name for the whole of the function.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum At {
    /// Wherever the value is declared: the language holds a name for the whole function, or the
    /// declaration of a value of that name is itself what does not build (Rust's `let Ok = …`).
    Anywhere,
    /// At this place in the order the function is written in, inside the loop of the walk of the
    /// sequence `lp` where it is in one.
    Pos { pos: usize, lp: Option<usize> },
    /// Where the function returns the outputs, after every item.
    End,
}

/// One name a target's generated code reads or declares.
#[derive(Clone, Debug)]
pub(super) struct Own {
    /// As the target writes it.
    pub name: String,
    pub what: What,
    pub at: At,
    /// Read only in the functions that take the inputs as their parameters and nothing else of the
    /// rule's: the one that writes a call as a record, the one that counts the answers.
    pub inputs_only: bool,
    /// Which calls reach where it is read.
    pub reach: Reach,
    /// A local `Gen::temp` names apart from the rule's names as they are written: only a name the
    /// language writes otherwise (Swift's and Java's lowerCamelCase) meets it.
    pub apart: bool,
}

/// Which calls reach a name the generated code reads, for the targets that never make such a call.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) enum Reach {
    /// Every call, or every call the rule refuses: the refusals are calls as much as the answers.
    Every,
    /// Only a call that passes a whole number as a float (Python) or as a `number` rather than a
    /// `bigint` (TypeScript, JavaScript): the service of Connect is given whole numbers by the wire.
    Float,
    /// Only a type checker (mypy, tsc): where a row falls through, which rulec has checked no row
    /// does, and an annotation. JavaScript has none, and Connect's service is the Python module's.
    Typecheck,
}

impl Own {
    fn float(mut self) -> Own {
        self.reach = Reach::Float;
        self
    }

    fn typecheck(mut self) -> Own {
        self.reach = Reach::Typecheck;
        self
    }

    fn apart(mut self) -> Own {
        self.apart = true;
        self
    }
}

/// What a rule has, as far as the names its generated code gives itself go.
pub(super) struct Has {
    /// An input or a sequence: the function checks what it was given.
    pub inputs: bool,
    /// An input or a field of one element that is a number, an amount, a quantity or a rate, which
    /// the function checks is a whole number and turns from a float that is one.
    pub numbers: bool,
    /// An input or a field that is a date, which the function checks is a whole number.
    pub date_inputs: bool,
    /// An input, a field or an output that is a date, which a record writes as a date.
    pub dates: bool,
    /// An input or a field that is a truth value.
    pub bools: bool,
    /// An input or a field of an enum.
    pub enum_inputs: bool,
    /// A table or a clause: a row that matched is put on the trace, and one that falls through is
    /// a contradiction the function raises.
    pub tables: bool,
    pub outputs: usize,
    pub sequences: usize,
    pub folds: usize,
    /// The rounding modes the outputs round with.
    pub out_modes: Vec<RoundMode>,
    /// What the expressions call: the rounding modes, `min` and `max` of two numbers, a quotient's
    /// rounding, `mod`, `isqrt`, and the helpers of the ordered enums.
    pub calls: Calls,
    /// The brands of the inputs and the fields the function turns from a float.
    pub in_brands: Vec<String>,
    /// The brands of the outputs, which the function returns.
    pub out_brands: Vec<String>,
    /// Every brand the module declares.
    pub brands: Vec<String>,
    /// The types of the enums the rule reads, as every language that names a type writes them.
    pub enum_types: Vec<String>,
    /// The type one element is written as: `Element`, or `<Sequence>Element` for each.
    pub element_types: Vec<String>,
    pub random: bool,
    pub machine: bool,
    /// What each table reads, by the place of the table among the items.
    pub tables_read: Vec<(usize, TableReads)>,
    /// What each expression reads, by the place of its item among the items.
    pub items_call: Vec<(usize, Calls)>,
    /// The places of the walks (the summaries and the folds) among the items.
    pub walks: Vec<usize>,
    /// The columns the SQL query binds an operand it reads several times to (`_r1`, `_r2`, …), as
    /// the query rulec writes for the rule names them (`sql::Sql::bind`). A rule that walks a
    /// sequence has no query (§15.56).
    pub binds: Vec<String>,
    /// Where each item declares what it decides and where it reads what it reads, by its place
    /// among the items, in the order the function is written in (`Gen::walk_steps`): the items of
    /// one call, then each walk, whose summaries are declared before its loop, whose items of one
    /// element are inside it, and whose fold answers after it.
    pub declared: Vec<(usize, Option<usize>)>,
    pub read_at: Vec<(usize, Option<usize>)>,
    /// The types Go converts the outputs to where the function returns them: `bool`, `string` and
    /// the types of the enums.
    pub go_out_types: Vec<String>,
    /// Python's function reads the length of a sequence: to hold it to the most a count allows,
    /// to tell whether a fold's walk read any element, or to read one out of the caller's objects.
    pub py_len: bool,
    /// A fold picks by weight (§15.226), which Java's walk returns as a `Walked`.
    pub weighted: bool,
    /// The locals the folds' walks keep, as Swift and Java declare them (`Gen::fold_names`): the
    /// answer and the flag of a stop, the flag of a walk shared with other summaries, the taken,
    /// the kept and the best, the running total of a pick by weight, and what the walk holds.
    pub fold_locals_sw: Vec<String>,
    pub fold_locals_java: Vec<String>,
    /// The identifier of each `max` and `min`, beside which Swift and Java keep a flag
    /// (`<identifier>Set`).
    pub extremes: Vec<String>,
    /// Each walk, in the order the function is written in: the sequence, where its loop starts,
    /// which checks each element before the items of one element, and where the loop has ended,
    /// which checks the summaries that declare a range before the folds answer.
    pub loops: Vec<Loop>,
}

/// One walk's loop, for the places the function checks what it was given there.
#[derive(Clone, Copy, Debug)]
pub(super) struct Loop {
    pub seq: usize,
    pub start: usize,
    pub end: usize,
    /// A summary of the walk declares a range, which is checked once the loop has ended.
    pub ranged: bool,
}

/// What an expression calls.
#[derive(Clone, Debug, Default)]
pub(super) struct Calls {
    pub modes: Vec<RoundMode>,
    pub min: bool,
    pub max: bool,
    pub uses: super::arith::Uses,
    /// `_rank_<Type>`, `_max_<Type>`, `_min_<Type>`.
    pub enum_helpers: Vec<String>,
}

impl Calls {
    fn merge(&mut self, o: &Calls) {
        for m in &o.modes {
            if !self.modes.contains(m) {
                self.modes.push(*m);
            }
        }
        self.min |= o.min;
        self.max |= o.max;
        for m in &o.uses.div {
            if !self.uses.div.contains(m) {
                self.uses.div.push(*m);
            }
        }
        self.uses.modulo |= o.uses.modulo;
        self.uses.root |= o.uses.root;
        for h in &o.enum_helpers {
            if !self.enum_helpers.contains(h) {
                self.enum_helpers.push(h.clone());
            }
        }
    }
}

/// What one table reads beside the values: the groups its cells name, the ordered enums whose
/// order a cell compares, and whether a cell tests a prefix.
#[derive(Clone, Debug, Default)]
pub(super) struct TableReads {
    /// By their identifiers.
    pub groups: Vec<String>,
    /// By their types.
    pub ranked: Vec<String>,
    pub prefixes: bool,
    /// The types Go declares the table's outputs with before its branches (`var fee int64`), in
    /// the order of the outputs.
    pub go_types: Vec<String>,
}

impl Has {
    pub(super) fn of(f: &RuleFile, c: &Checked) -> Has {
        let g = Gen::for_names(f, c);
        let ty = |n: &str| c.ty_of(n).unwrap_or(Ty::Unknown).present().clone();
        let passed: Vec<&str> = f.inputs.iter().map(|i| i.name.text.as_str()).chain(f.elements.iter().flat_map(|el| el.fields.iter().map(|fd| fd.name.text.as_str()))).collect();
        let numbers = passed.iter().any(|n| matches!(ty(n), Ty::Number | Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate));
        let date_inputs = passed.iter().any(|n| matches!(ty(n), Ty::Date));
        let dates = date_inputs || f.outputs.iter().any(|o| matches!(ty(&o.name.text), Ty::Date));
        let branded = |t: &Ty| matches!(t, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate);
        let brands_of = |names: &mut dyn Iterator<Item = &str>| -> Vec<String> {
            let mut out: Vec<String> = Vec::new();
            for n in names {
                let t = ty(n);
                if branded(&t) {
                    let b = super::brand_of(&t);
                    if !out.contains(&b) {
                        out.push(b);
                    }
                }
            }
            out
        };
        let in_brands = brands_of(&mut passed.iter().copied());
        let out_brands = brands_of(&mut f.outputs.iter().map(|o| o.name.text.as_str()));
        let (own_brands, units) = g.brands();
        let brands: Vec<String> = own_brands.keys().chain(units.keys()).cloned().collect();
        let mut enum_types: Vec<String> = Vec::new();
        for en in &c.enum_order {
            let t = g.enum_type(en);
            if !enum_types.contains(&t) {
                enum_types.push(t);
            }
        }
        let mut tables_read: Vec<(usize, TableReads)> = Vec::new();
        let mut items_call: Vec<(usize, Calls)> = Vec::new();
        let mut walks: Vec<usize> = Vec::new();
        let mut calls = Calls::default();
        for (k, it) in f.items.iter().enumerate() {
            match it {
                Item::Table(t) => {
                    if let Some(t) = c.table_at(t) {
                        tables_read.push((k, table_reads(f, c, &g, t)));
                    }
                }
                Item::Derived(d) => items_call.push((k, expr_calls(&d.expr, &g))),
                Item::Define(d) => items_call.push((k, expr_calls(&d.expr, &g))),
                Item::Agg(_) | Item::Fold(_) => walks.push(k),
            }
        }
        for (_, cs) in &items_call {
            calls.merge(cs);
        }
        let mut walk_calls = Calls::default();
        for fd in f.folds() {
            let mut es: Vec<&Expr> = fd.empty.iter().chain(fd.exhausted.iter()).collect();
            for (_, arm, _) in &fd.arms {
                match arm {
                    crate::ast::Arm::Stop(Some(e)) => es.push(e),
                    crate::ast::Arm::Take { expr, .. } => es.push(expr),
                    crate::ast::Arm::KeepMax { expr, key } => {
                        es.push(expr);
                        es.push(key);
                    }
                    crate::ast::Arm::TakeWeighted { expr, weight } => {
                        es.push(expr);
                        es.push(weight);
                    }
                    _ => {}
                }
            }
            for e in es {
                walk_calls.merge(&expr_calls(e, &g));
            }
        }
        calls.merge(&walk_calls);
        if let Some(r) = &f.result {
            calls.merge(&expr_calls(&r.expr, &g));
        }
        // A `max` or a `min` over a column of an ordered enum compares two of its values (§15.219).
        for d in f.aggs() {
            if d.kind.extreme() {
                if let Some(en) = g.extreme_enum(d) {
                    let h = format!("_rank_{}", g.enum_type(&en));
                    if !calls.enum_helpers.contains(&h) {
                        calls.enum_helpers.push(h);
                    }
                }
            }
        }
        Has {
            inputs: !f.inputs.is_empty() || !f.elements.is_empty(),
            numbers,
            date_inputs,
            dates,
            bools: passed.iter().any(|n| matches!(ty(n), Ty::Bool)),
            enum_inputs: passed.iter().any(|n| matches!(ty(n), Ty::Enum(_))),
            tables: f.items.iter().any(|it| matches!(it, Item::Table(_))),
            outputs: f.outputs.len(),
            sequences: f.elements.len(),
            folds: f.folds().count(),
            out_modes: {
                let mut ms: Vec<RoundMode> = Vec::new();
                for m in f.outputs.iter().filter_map(|o| o.rounding.as_ref()).filter_map(|r| RoundMode::parse(&r.mode)) {
                    if !ms.contains(&m) {
                        ms.push(m);
                    }
                }
                ms
            },
            calls,
            in_brands,
            out_brands,
            brands,
            enum_types,
            element_types: (0..f.elements.len()).map(|s| g.element_type(s)).collect(),
            random: f.has_random(),
            machine: f.machine.is_some(),
            tables_read,
            items_call,
            walks,
            binds: if f.elements.is_empty() { (1..=g.sql_bound()).map(|k| format!("_r{k}")).collect() } else { Vec::new() },
            declared: Vec::new(),
            read_at: Vec::new(),
            go_out_types: {
                let mut ts: Vec<String> = Vec::new();
                for o in &f.outputs {
                    let t = go_local(&g, &ty(&o.name.text));
                    if t != "int64" && !ts.contains(&t) {
                        ts.push(t);
                    }
                }
                ts
            },
            py_len: (0..f.elements.len()).any(|s| g.count_cap_of(s).is_some())
                || f.folds().count() > 0
                || g.projections().iter().any(|p| matches!(p.kind, crate::ast::ProjKind::Count(None))),
            weighted: g.weighted_fold().is_some(),
            fold_locals_sw: fold_locals(f, &g, false),
            fold_locals_java: fold_locals(f, &g, true),
            extremes: f.aggs().filter(|d| d.kind.extreme()).map(|d| g.ident(&d.name.text)).collect(),
            loops: Vec::new(),
        }
        .placed(&g)
    }

    /// The places of the items in the order the function is written in.
    fn placed(mut self, g: &Gen) -> Has {
        let n = g.f.items.len();
        self.declared = vec![(0, None); n];
        self.read_at = vec![(0, None); n];
        let mut pos = 0usize;
        for step in g.walk_steps() {
            match step {
                super::WalkStep::Once(items) => {
                    for k in items {
                        self.declared[k] = (pos, None);
                        self.read_at[k] = (pos, None);
                        pos += 1;
                    }
                }
                super::WalkStep::Walk { seq, body, sums } => {
                    for &k in &sums {
                        self.declared[k] = (pos, None);
                        pos += 1;
                    }
                    let start = pos;
                    for &k in &body {
                        self.declared[k] = (pos, Some(seq));
                        self.read_at[k] = (pos, Some(seq));
                        pos += 1;
                    }
                    for &k in &sums {
                        self.read_at[k] = (pos, Some(seq));
                        pos += 1;
                    }
                    let ranged = sums.iter().any(|&k| matches!(&g.f.items[k], Item::Agg(d) if d.range.is_some()));
                    self.loops.push(Loop { seq, start, end: pos, ranged });
                    pos += 1;
                    for &k in &sums {
                        if matches!(g.f.items[k], Item::Fold(_)) {
                            self.declared[k] = (pos, None);
                            pos += 1;
                        }
                    }
                }
            }
        }
        self
    }

    /// Where an item reads what it reads.
    pub(super) fn at(&self, k: usize) -> At {
        let (pos, lp) = self.read_at.get(k).copied().unwrap_or((0, None));
        At::Pos { pos, lp }
    }

    /// Where a fold declares its answer, after its loop, which is where the function reads whether
    /// the walk read any element.
    fn after(&self, k: usize) -> At {
        let (pos, _) = self.declared.get(k).copied().unwrap_or((0, None));
        At::Pos { pos, lp: None }
    }

    /// Every rounding mode the code rounds with: the outputs' and the expressions'.
    fn modes(&self) -> Vec<RoundMode> {
        let mut ms = self.out_modes.clone();
        for m in &self.calls.modes {
            if !ms.contains(m) {
                ms.push(*m);
            }
        }
        ms
    }

    /// What the tables read, merged.
    fn all_tables(&self) -> TableReads {
        let mut out = TableReads::default();
        for (_, t) in &self.tables_read {
            for g in &t.groups {
                if !out.groups.contains(g) {
                    out.groups.push(g.clone());
                }
            }
            for r in &t.ranked {
                if !out.ranked.contains(r) {
                    out.ranked.push(r.clone());
                }
            }
            out.prefixes |= t.prefixes;
        }
        out
    }

    /// The types Go converts the outputs to where the function returns them (`bool(ok)`), beside
    /// the brands and `int64`.
    pub(super) fn go_returned(&self) -> &[String] {
        &self.go_out_types
    }
}

/// The type Go holds a value of `ty` in inside the function: `int64` for every number, a brand
/// too, and else the type it is written as, an optional's own.
fn go_local(g: &Gen, ty: &Ty) -> String {
    match ty.present() {
        Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => "int64".into(),
        t => g.go_ty(t),
    }
}

/// What a table reads beside the values.
fn table_reads(f: &RuleFile, c: &Checked, g: &Gen, t: &crate::ast::Table) -> TableReads {
    let mut out = TableReads::default();
    out.go_types = t.outputs.iter().map(|oc| go_local(g, &c.ty_of(&oc.name.text).unwrap_or(Ty::Unknown))).collect();
    for row in &t.rows {
        for (ci, cell) in row.cells.iter().enumerate() {
            let words: Vec<&String> = match cell {
                Cell::Lit(Lit::Word(w)) => vec![w],
                Cell::Set(ls) | Cell::Not(ls) => ls.iter().filter_map(|l| if let Lit::Word(w) = l { Some(w) } else { None }).collect(),
                Cell::Ranked { .. } => {
                    if let Some((col, _)) = t.inputs.get(ci) {
                        if let Some(en) = c.ordered_enum(&c.ty_of(col).unwrap_or(Ty::Unknown)) {
                            let ty = g.enum_type(en);
                            if !out.ranked.contains(&ty) {
                                out.ranked.push(ty);
                            }
                        }
                    }
                    vec![]
                }
                Cell::Prefix(_) => {
                    out.prefixes = true;
                    vec![]
                }
                _ => vec![],
            };
            for w in words {
                if let Some(gr) = f.groups.iter().find(|gr| &gr.name.text == w) {
                    let id = pub_name(&gr.name);
                    if !out.groups.contains(&id) {
                        out.groups.push(id);
                    }
                }
            }
        }
    }
    out
}

/// What an expression calls, as `Gen::expr` writes it: a rounding by a grid, a rounding of a
/// quotient, `mod`, `isqrt`, `min` and `max` of two numbers, and the comparison and the later or
/// earlier of two values of an ordered enum.
fn expr_calls(e: &Expr, g: &Gen) -> Calls {
    fn walk(e: &Expr, g: &Gen, out: &mut Calls) {
        match e {
            Expr::Call(name, args, _) => {
                let ordered = args.first().and_then(|a| g.ordered_of(a));
                if crate::types::var_division(name, args).is_some() {
                    if let Some(m) = RoundMode::parse(name) {
                        if !out.uses.div.contains(&m) {
                            out.uses.div.push(m);
                        }
                    }
                } else if let Some(m) = RoundMode::parse(name) {
                    if !out.modes.contains(&m) {
                        out.modes.push(m);
                    }
                } else if name == crate::kw::MOD {
                    out.uses.modulo = true;
                } else if name == crate::kw::ISQRT {
                    out.uses.root = true;
                } else if name == crate::kw::MIN || name == crate::kw::MAX {
                    match ordered {
                        Some(en) => {
                            let h = format!("{}{}", if name == crate::kw::MIN { "_min_" } else { "_max_" }, g.enum_type(&en));
                            if !out.enum_helpers.contains(&h) {
                                out.enum_helpers.push(h);
                            }
                        }
                        None if name == crate::kw::MIN => out.min = true,
                        None => out.max = true,
                    }
                }
                for a in args {
                    walk(a, g, out);
                }
            }
            Expr::Bin(l, op, r, _) => {
                if matches!(op, BinOp::Le | BinOp::Lt | BinOp::Ge | BinOp::Gt) {
                    if let Some(en) = g.ordered_of(l).or_else(|| g.ordered_of(r)) {
                        let h = format!("_rank_{}", g.enum_type(&en));
                        if !out.enum_helpers.contains(&h) {
                            out.enum_helpers.push(h);
                        }
                    }
                }
                walk(l, g, out);
                walk(r, g, out);
            }
            _ => {}
        }
    }
    let mut out = Calls::default();
    walk(e, g, &mut out);
    // The modes of a quotient's rounding in the order §7.3 lists them, as `Uses` keeps them.
    let order = [RoundMode::Down, RoundMode::Up, RoundMode::Half, RoundMode::HalfDown, RoundMode::Bankers];
    out.uses.div.sort_by_key(|m| order.iter().position(|x| x == m));
    out
}

/// The rounding helper of `m` as the target writes it: `_round_half_down`, `_roundHalfDown`,
/// `roundHalfDown`, `round_half_down`.
fn round_name(t: Target, m: RoundMode) -> Option<String> {
    let p = pascal(mode_fn(m));
    Some(match t {
        Target::Python | Target::Connect | Target::Php | Target::Ruby => format!("_round_{}", mode_fn(m)),
        Target::TypeScript | Target::JavaScript | Target::Swift => format!("_round{p}"),
        Target::Go | Target::Java => format!("round{p}"),
        Target::Rust | Target::Wasm => format!("round_{}", mode_fn(m)),
        Target::Sqlite | Target::Postgres => return None,
    })
}

/// The helpers `cs` names, as the target writes them: its rounding by a grid and of a quotient,
/// `mod`, `isqrt`, `min` and `max`, and the helpers of the ordered enums.
fn helpers_of(t: Target, cs: &Calls) -> Vec<String> {
    let lang = match t {
        Target::TypeScript | Target::JavaScript => "ts",
        Target::Go => "go",
        Target::Rust | Target::Wasm => "rs",
        Target::Swift => "sw",
        Target::Java => "java",
        _ => "py",
    };
    let mut out: Vec<String> = cs.modes.iter().filter_map(|m| round_name(t, *m)).collect();
    let (min, max) = match t {
        Target::Go => ("minInt", "maxInt"),
        Target::Rust | Target::Wasm => ("min_i64", "max_i64"),
        Target::Java => ("Math.min", "Math.max"),
        _ => ("_min", "_max"),
    };
    if cs.min {
        out.push(min.to_string());
    }
    if cs.max {
        out.push(max.to_string());
    }
    out.extend(super::arith::helper_names(&cs.uses, lang));
    out.extend(cs.enum_helpers.iter().cloned());
    out.retain(|n| !n.contains('.'));
    out
}

/// The names the rule's functions read in the code of `t`, which a value of the same identifier
/// hides there: a helper, a type, a constant, a builtin, a parameter, or a local the generated
/// code keeps for itself.
pub(super) fn reads(t: Target, f: &RuleFile, has: &Has) -> Vec<Own> {
    let mut out = Reads::default();
    let a = At::Anywhere;
    let tables = has.all_tables();
    match t {
        // The function holds the inputs as parameters and every other value as a local, and a name
        // the function assigns anywhere is a local throughout it (Python's scoping): every read of
        // a name the module defines reads the local instead.
        Target::Python | Target::Connect => {
            if has.inputs {
                out.add("_isinstance", What::Helper, a);
                // Where a call is refused.
                out.add("_repr", What::Helper, a);
                out.add("RuleInputError", What::Type, a);
                out.push(own("_json_str", What::Helper, a).inputs_only());
                // The plain function takes the answer of its traced twin into `out` and `_`, which
                // mypy holds to the type of a parameter of that name (`_` is said on its own).
                out.push(own("out", What::Local, a).inputs_only().typecheck());
            }
            if has.numbers || has.date_inputs {
                out.add("_int", What::Helper, a);
                out.add("_bool", What::Helper, a);
            }
            if has.numbers {
                out.add("_float", What::Helper, a);
            }
            if has.bools {
                out.add("_bool", What::Helper, a);
            }
            if has.py_len {
                out.add("_len", What::Helper, a);
            }
            if has.tables {
                out.add("Fired", What::Type, a);
                // A row that falls through raises it, which no call reaches: rulec has checked that no
                // row does.
                out.push(own("AssertionError", What::Builtin, a).typecheck());
            }
            if has.outputs >= 2 {
                out.add("Output", What::Type, a);
            }
            // An input or a field that comes as a float is turned into its brand (`Gram(_int(x))`).
            for b in &has.in_brands {
                out.push(own(b, What::Type, a).float());
            }
            for b in &has.out_brands {
                out.add(b, What::Type, a);
            }
            let mut all = has.calls.clone();
            all.modes = has.modes();
            for n in helpers_of(t, &all) {
                out.add(&n, What::Helper, a);
            }
            for e in &tables.ranked {
                out.add(&format!("{}_rank", super::snake(e)), What::Helper, a);
            }
            for e in &has.enum_types {
                out.add(e, What::Type, a);
            }
            if has.folds > 0 {
                // The accumulator of a walk is declared with its type, `answer: int = 0`, which only
                // a type checker reads.
                out.push(own("int", What::Builtin, a).typecheck());
            }
            if has.dates {
                out.push(own("_civil", What::Helper, a).inputs_only());
            }
            if has.random && t == Target::Python {
                out.push(own("Chances", What::Type, a).inputs_only());
            }
        }
        // The function holds the inputs as parameters and every other value as a constant of the
        // function, which no read before its declaration may reach: a value of a name the module
        // defines hides it in the whole function. A type (`Fired`, a brand) is read where a type
        // stands, which no value hides.
        Target::TypeScript | Target::JavaScript => {
            if has.inputs {
                // Where a call is refused, which reads the constant before its declaration.
                out.add("RuleInputError", What::Type, a);
                out.push(own("JSON", What::Builtin, a).inputs_only());
                out.push(own("String", What::Builtin, a).inputs_only());
            }
            if has.enum_inputs {
                out.add("Object", What::Builtin, a);
            }
            if has.numbers {
                // Read only where an input comes as a number rather than a bigint.
                out.push(own("Number", What::Builtin, a).float());
                out.push(own("BigInt", What::Builtin, a).float());
            }
            if has.tables && t == Target::TypeScript {
                // A row that falls through throws an `Error`, which `tsc` holds to where it stands.
                out.push(own("Error", What::Builtin, a).typecheck());
            }
            let mut all = has.calls.clone();
            all.modes = has.modes();
            for n in helpers_of(t, &all) {
                out.add(&n, What::Helper, a);
            }
            for e in &tables.ranked {
                out.add(&format!("{}Rank", super::lower_lead(e)), What::Helper, a);
            }
            for e in &has.enum_types {
                out.add(e, What::Constant, a);
            }
            for g in &tables.groups {
                out.add(&format!("_{g}"), What::Constant, a);
            }
            if has.dates {
                out.push(own("_civil", What::Helper, a).inputs_only());
            }
        }
        // A local hides an item of the module from where it is declared, where the function reads
        // it as a value: a function it calls. A value cannot be declared under the name of a
        // variant the prelude brings in, or of a brand, which is a struct of one field and so a
        // pattern as well: that declaration does not build wherever it stands.
        Target::Rust | Target::Wasm => {
            for n in ["Ok", "Err", "Some", "None"] {
                out.add(n, What::Builtin, a);
            }
            for b in &has.brands {
                out.add(b, What::Type, a);
            }
            for (k, cs) in &has.items_call {
                for n in helpers_of(t, cs) {
                    out.add(&n, What::Helper, has.at(*k));
                }
            }
            for (k, tr) in &has.tables_read {
                for g in &tr.groups {
                    out.add(&format!("is_{g}"), What::Helper, has.at(*k));
                }
            }
            let end = Calls { modes: has.out_modes.clone(), ..Calls::default() };
            for n in helpers_of(t, &end) {
                out.add(&n, What::Helper, At::End);
            }
            if has.inputs {
                out.push(own("json_str", What::Helper, a).inputs_only());
            }
            if has.dates {
                out.push(own("civil", What::Helper, a).inputs_only());
            }
        }
        // The inputs and the sequences are fields of `in`, the one parameter; every other value is
        // a local, which hides what the items after it and the return read.
        Target::Go => {
            // Every item that reads an input reads it from `in`.
            out.add("in", What::Parameter, a);
            for (k, tr) in &has.tables_read {
                out.add("Fired", What::Type, has.at(*k));
                for n in ["append", "panic"] {
                    out.add(n, What::Builtin, has.at(*k));
                }
                // The outputs are declared with their types before the branches.
                for t in &tr.go_types {
                    out.add(t, if matches!(t.as_str(), "int64" | "bool" | "string") { What::Builtin } else { What::Type }, has.at(*k));
                }
                if tr.prefixes {
                    out.add("strings", What::Builtin, has.at(*k));
                }
                for g in &tr.groups {
                    out.add(&format!("is{}", pascal(g)), What::Helper, has.at(*k));
                }
            }
            for (k, cs) in &has.items_call {
                for n in helpers_of(t, cs) {
                    out.add(&n, What::Helper, has.at(*k));
                }
            }
            // Each loop checks each element before the items of one element, and once it has ended,
            // the summaries that declare a range: a refusal returns the zero answer, no rows and
            // the error.
            for lp in &has.loops {
                let mut checks: Vec<(&str, What)> = vec![("RuleInputError", What::Type), ("nil", What::Builtin), ("int64", What::Builtin)];
                if has.outputs >= 2 {
                    checks.push(("Output", What::Type));
                }
                let fields = f.elements.get(lp.seq).is_some_and(|el| !el.fields.is_empty());
                for (n, w) in &checks {
                    if fields {
                        out.add(n, *w, At::Pos { pos: lp.start, lp: Some(lp.seq) });
                    }
                    if lp.ranged {
                        out.add(n, *w, At::Pos { pos: lp.end, lp: None });
                    }
                }
            }
            // After a fold's loop, whether the walk read any element (`len(in.<sequence>) > 0`).
            for k in &has.walks {
                if matches!(f.items[*k], Item::Fold(_)) {
                    out.add("len", What::Builtin, has.after(*k));
                }
            }
            for n in ["nil", "int64"] {
                out.add(n, What::Builtin, At::End);
            }
            for t in has.go_returned() {
                out.add(t, if matches!(t.as_str(), "bool" | "string") { What::Builtin } else { What::Type }, At::End);
            }
            if has.outputs >= 2 {
                out.add("Output", What::Type, At::End);
            }
            for b in &has.out_brands {
                out.add(b, What::Type, At::End);
            }
            let end = Calls { modes: has.out_modes.clone(), ..Calls::default() };
            for n in helpers_of(t, &end) {
                out.add(&n, What::Helper, At::End);
            }
        }
        // The values are lowerCamelCase. A helper whose name starts with `_` is hidden by a value
        // that does, and the locals the function keeps for itself, which `Gen::temp` keeps apart
        // from the rule's names only as they are written, meet a name the language writes in
        // lowerCamelCase.
        Target::Swift => {
            let mut all = has.calls.clone();
            all.modes = has.modes();
            for n in helpers_of(t, &all) {
                out.add(&n, What::Helper, a);
            }
            for g in &tables.groups {
                out.add(&format!("_{}", camel(g)), What::Constant, a);
            }
            if has.inputs {
                out.push(own("_jsonStr", What::Helper, a).inputs_only());
                for n in ["head", "ins", "obs", "rows", "out", "trace", "tag"] {
                    out.push(own(n, What::Local, a).inputs_only().apart());
                }
            }
            if has.dates {
                out.push(own("_civil", What::Helper, a).inputs_only());
                out.push(own("_pad", What::Helper, a).inputs_only());
            }
            for n in locals(has, false) {
                out.push(own(&n, What::Local, a).apart());
            }
        }
        // A method is not hidden by a variable, and a value is lowerCamelCase where a type is not:
        // what a value meets is a local the function keeps for itself.
        Target::Java => {
            if has.inputs {
                for n in ["head", "ins", "obs", "rows", "out", "trace", "tag"] {
                    out.push(own(n, What::Local, a).inputs_only().apart());
                }
            }
            for n in locals(has, true) {
                out.push(own(&n, What::Local, a).apart());
            }
        }
        // A variable is written with `$`, which no function, class or constant is; a method is
        // called with its arguments in parentheses, which a local variable does not take.
        Target::Php | Target::Ruby => {}
        // The query holds each value as a column of its own, beside the columns it keeps for itself.
        Target::Sqlite | Target::Postgres => {
            for n in ["_id", "_input_error"] {
                out.add(n, What::Column, a);
            }
            for o in &f.outputs {
                out.add(&format!("_raw_{}", pub_name(&o.name)), What::Column, a);
            }
            // A rounding, a quotient or `mod` binds what it reads to a column of its own first,
            // numbered from 1 (`sql::Sql::bind`).
            for n in &has.binds {
                out.add(n, What::Column, a);
            }
        }
    }
    out.0
}

/// The names a target's functions read, each once where it is read.
#[derive(Default)]
struct Reads(Vec<Own>);

impl Reads {
    /// One more place a name is read; read at one place twice, it is read wherever either is.
    fn push(&mut self, o: Own) {
        match self.0.iter_mut().find(|x| x.name == o.name && x.at == o.at) {
            Some(x) => {
                x.inputs_only &= o.inputs_only;
                x.reach = x.reach.min(o.reach);
                x.apart &= o.apart;
            }
            None => self.0.push(o),
        }
    }

    fn add(&mut self, name: &str, what: What, at: At) {
        self.push(own(name, what, at));
    }
}

fn own(name: &str, what: What, at: At) -> Own {
    Own { name: name.to_string(), what, at, inputs_only: false, reach: Reach::Every, apart: false }
}

impl Own {
    fn inputs_only(mut self) -> Own {
        self.inputs_only = true;
        self
    }
}

/// The locals the rule's function keeps for itself in Swift and Java, which `Gen::temp` keeps apart
/// from the rule's names as they are written and which these two write in lowerCamelCase, so that
/// `Taken` and `hxtopprice_set` meet `taken` and `hxtoppriceSet`: the trace, the loop's variable
/// where it is a local of the function (`elem` in Java), the locals of the folds' walks, and the
/// flag beside the largest or the smallest of a column.
fn locals(has: &Has, java: bool) -> Vec<String> {
    let mut out: Vec<String> = vec!["trace".into()];
    if java && has.sequences > 0 {
        out.push("elem".into());
    }
    out.extend(if java { has.fold_locals_java.iter().cloned() } else { has.fold_locals_sw.iter().cloned() });
    out.extend(has.extremes.iter().map(|n| format!("{}Set", camel(n))));
    out
}

/// The locals each fold's walk declares in Swift (`Gen::sw_fold`) or Java (`java::java_fold`), as
/// `Gen::fold_names` names them, for the arms the fold has.
fn fold_locals(f: &RuleFile, g: &Gen, java: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (i, it) in f.items.iter().enumerate() {
        let Item::Fold(fold) = it else { continue };
        let n = g.fold_names(i);
        out.extend([n.answer, n.stopped]);
        // What the walk holds is declared inside the block that answers once the walk has ended,
        // which Java does not let hide a local of the function, and Swift does.
        if java {
            out.push(n.held);
        }
        if !g.fold_alone(i) {
            out.push(n.done);
        }
        let takes = fold.arms.iter().any(|(_, a, _)| matches!(a, crate::ast::Arm::Take { .. } | crate::ast::Arm::TakeWeighted { .. }));
        let keeps = fold.arms.iter().any(|(_, a, _)| matches!(a, crate::ast::Arm::KeepMax { .. }));
        if takes || java {
            out.push(n.taken);
        }
        if keeps || java {
            out.extend([n.kept, n.best]);
        }
        if keeps {
            out.push(n.key);
        }
        if fold.weighted() {
            out.extend([n.total, n.cuts, n.weight]);
        }
    }
    out
}

/// The names the generated code of `t` declares at the top of its module (its package, the class
/// the rule is): its own types, the brands and the errors. An enum's type is declared beside them.
pub(super) fn tops(t: Target, has: &Has) -> Vec<Own> {
    let mut out: Vec<Own> = Vec::new();
    let mut add = |name: &str, what: What| {
        if !out.iter().any(|o: &Own| o.name == name) {
            out.push(own(name, what, At::Anywhere));
        }
    };
    let every_rule: &[&str] = match t {
        Target::Python | Target::Connect => &["Fired", "RuleInputError", "RuleContradictionError", "NamedTuple", "ValueError", "AssertionError"],
        Target::TypeScript => &["Fired", "RuleInputError", "RuleContradictionError"],
        Target::JavaScript => &["RuleInputError", "RuleContradictionError"],
        Target::Rust | Target::Wasm => &["Fired", "RuleError"],
        Target::Go => &["Fired", "Input", "RuleInputError", "RuleContradictionError"],
        Target::Swift => &["Fired", "RuleError"],
        Target::Java => &["Fired", "Traced", "RuleInputError", "RuleContradictionError"],
        Target::Php | Target::Ruby => &["Fired", "RuleInputError", "RuleContradictionError"],
        Target::Sqlite | Target::Postgres => &[],
    };
    for n in every_rule {
        let what = match *n {
            "NamedTuple" => What::Import,
            "ValueError" | "AssertionError" => What::Builtin,
            _ => What::Type,
        };
        add(n, what);
    }
    if matches!(t, Target::Sqlite | Target::Postgres) {
        return out;
    }
    if t != Target::JavaScript {
        if has.outputs >= 2 {
            add("Output", What::Type);
        }
        for e in &has.element_types {
            add(e, What::Type);
        }
    }
    if has.random {
        if t != Target::JavaScript {
            add("Chances", What::Type);
        }
        if matches!(t, Target::Go | Target::Swift | Target::Java) {
            add("Way", What::Type);
        }
        if t == Target::Java && has.weighted {
            add("Walked", What::Type);
        }
    }
    if matches!(t, Target::Python | Target::Connect | Target::TypeScript | Target::Rust | Target::Wasm | Target::Go | Target::Swift) {
        for b in &has.brands {
            add(b, What::Type);
        }
        if matches!(t, Target::Python | Target::Connect) && !has.brands.is_empty() {
            add("NewType", What::Import);
        }
    }
    if has.machine && t == Target::Go {
        for n in ["Initial", "Final", "IsFinal"] {
            add(n, What::Constant);
        }
    }
    out
}

/// The names of the `.proto` the rule's service is written in, at the top of its package: the
/// messages and the service. A wire enum of the same name is a second declaration there.
pub(super) fn proto_messages(f: &RuleFile, has: &Has) -> Vec<String> {
    let mut out: Vec<String> = vec!["Fired".into(), "DecideRequest".into(), "DecideResponse".into()];
    out.extend(has.element_types.iter().cloned());
    out.push(format!("{}Service", pascal(&pub_name(&f.name))));
    out
}

/// The functions the Rust module declares beside the rule's: the five rounding modes, the two
/// comparisons and the JSON helper of every module, the date's, and the helpers the expressions call
/// and the groups and the ordered enums need. A rule's function of one of these names is declared
/// twice.
pub(super) fn rust_fns(has: &Has) -> Vec<String> {
    let mut out: Vec<String> = ["round_down", "round_up", "round_half", "round_half_down", "round_bankers", "min_i64", "max_i64", "json_str"].map(str::to_string).to_vec();
    if has.dates {
        out.push("civil".into());
    }
    out.extend(super::arith::helper_names(&has.calls.uses, "rs"));
    out.extend(has.calls.enum_helpers.iter().cloned());
    for (_, t) in &has.tables_read {
        out.extend(t.groups.iter().map(|g| format!("is_{g}")));
    }
    out
}

/// The helpers the TypeScript and the JavaScript modules declare under a name that starts with `_`,
/// where a group's constant (`_<group>`) stands beside them: the two comparisons and the five
/// rounding modes of every module, the date's, and the helpers the expressions call and the ordered
/// enums need.
pub(super) fn ts_helpers(has: &Has) -> Vec<String> {
    let mut out: Vec<String> = ["_min", "_max", "_roundDown", "_roundUp", "_roundHalf", "_roundHalfDown", "_roundBankers"].map(str::to_string).to_vec();
    if has.dates {
        out.push("_civil".into());
    }
    out.extend(super::arith::helper_names(&has.calls.uses, "ts"));
    out.extend(has.calls.enum_helpers.iter().cloned());
    out
}

/// The builtins Python's module reads by their own names at run time, below the functions that
/// hold the rule's names, which reach them through names of the module's own (§15.207): the
/// rounding (`abs`), the JSON of a record (`ord`), the errors (`super`), and the readers of the
/// caller's objects (§15.125). A function of the rule's of one of these names takes its place
/// there, in the module and in the service of Connect that runs it.
pub(super) fn py_reads(projects: bool) -> Vec<&'static str> {
    let mut out = vec!["abs", "ord", "super"];
    if projects {
        out.extend(["isinstance", "int", "float", "bool", "str", "dict", "list", "all", "enumerate", "len", "max", "ValueError"]);
    }
    out
}

/// The classes of the language Ruby's module reads by their names from inside the rule's module,
/// where it nests the module of each enum (`module Tier`): the errors' parent (`ArgumentError`),
/// what it checks an input is (`Hash`, `Array`, `Integer`, `Float`, `String`, `Object`) and what the
/// answer is made with (`Struct`). An enum's module of one of these names is what that code reads.
pub(super) fn rb_reads() -> &'static [&'static str] {
    &["ArgumentError", "Object", "Hash", "Struct", "Array", "Float", "Integer", "String"]
}

/// The builtins PHP's module calls without a namespace, which PHP looks for in the rule's own
/// namespace first: the rounding, the dates and the JSON of a record (`abs`, `intdiv`, `ord`,
/// `sprintf`, `strlen`, `implode`), a set (`in_array`), a prefix (`str_starts_with`), a walk, and
/// the readers of the caller's objects. A function of the rule's of one of these names, which
/// PHP reads with case set aside, is what they call.
pub(super) fn php_reads(has: &Has, projects: bool) -> Vec<&'static str> {
    let mut out = vec!["abs", "implode", "intdiv", "ord", "sprintf", "strlen"];
    let tables = has.all_tables();
    if !tables.groups.is_empty() || has.enum_inputs {
        out.push("in_array");
    }
    if tables.prefixes {
        out.push("str_starts_with");
    }
    if has.sequences > 0 {
        out.extend(["array_filter", "array_map", "array_merge", "array_reduce", "array_sum", "count", "max", "min"]);
    }
    if projects {
        out.extend(["floor", "is_finite", "is_float", "is_int", "is_array", "array_is_list", "is_bool", "is_string", "preg_match", "count", "max"]);
    }
    out
}

#[cfg(test)]
mod tests {
    //! The names this module lists are names the generated code has. For rules of the corpus that
    //! reach the tables, the groups, the ordered enums, the walks and the folds, the dates, the
    //! brands, the random inputs and a machine, every helper, type, constant, local, column,
    //! parameter and import a target's functions are said to read or its module to declare is an
    //! identifier of what that target writes. A builtin of the language is read whether or not the
    //! code names it, and is not held here.

    use super::*;

    /// The rules of the corpus that pass the check, by their paths under the crate.
    fn rules() -> Vec<String> {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut out: Vec<String> = std::fs::read_dir(root.join("tests/corpus"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".rule"))
            .map(|n| format!("tests/corpus/{n}"))
            .collect();
        out.sort();
        out
    }

    /// The backend a target's code is written by.
    fn backend(t: Target) -> &'static str {
        match t {
            Target::Python | Target::Connect => "python",
            Target::TypeScript => "typescript",
            Target::JavaScript => "javascript",
            Target::Rust => "rust",
            Target::Wasm => "wasm",
            Target::Ruby => "ruby",
            Target::Php => "php",
            Target::Go => "go",
            Target::Swift => "swift",
            Target::Java => "java",
            Target::Sqlite | Target::Postgres => "sql",
        }
    }

    /// Whether `name` stands in `text` as an identifier: with no letter, digit or `_` on either
    /// side.
    fn has_ident(text: &str, name: &str) -> bool {
        let word = |c: char| c.is_alphanumeric() || c == '_';
        let mut from = 0;
        while let Some(i) = text[from..].find(name) {
            let at = from + i;
            let before = text[..at].chars().next_back();
            let after = text[at + name.len()..].chars().next();
            if !before.is_some_and(word) && !after.is_some_and(word) {
                return true;
            }
            from = at + name.len();
        }
        false
    }

    #[test]
    fn every_name_listed_is_in_the_code_written() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut missing: Vec<String> = Vec::new();
        let mut read = 0;
        for rel in &rules() {
            let path = root.join(rel).to_string_lossy().into_owned();
            let src = std::fs::read_to_string(&path).unwrap();
            let Ok((f, c)) = crate::prepare(&src, &path) else { continue };
            if f.kind != crate::ast::FileKind::Rule {
                continue;
            }
            read += 1;
            let has = Has::of(&f, &c);
            let g = Gen::new(&f, &c, &src, &path);
            let alias = f.name.ascii.clone().unwrap_or_else(|| f.name.text.clone());
            let pkg = crate::backend::go_package(&alias);
            for t in [
                Target::Python,
                Target::Connect,
                Target::TypeScript,
                Target::JavaScript,
                Target::Rust,
                Target::Wasm,
                Target::Ruby,
                Target::Php,
                Target::Go,
                Target::Swift,
                Target::Java,
                Target::Sqlite,
            ] {
                let Some(b) = crate::backend::ALL.iter().find(|b| b.id == backend(t)) else { continue };
                // SQL and NumPy write no rule that walks a sequence (§15.56).
                if !b.folds && !f.elements.is_empty() {
                    continue;
                }
                let text: String = (b.files)(&g, &alias, &pkg).into_iter().map(|(_, body)| body).collect::<Vec<_>>().join("\n");
                let mut listed: Vec<Own> = reads(t, &f, &has);
                listed.extend(tops(t, &has));
                for o in listed.iter().filter(|o| o.what != What::Builtin) {
                    // A local written only by the function that writes a record is there where
                    // the rule has inputs.
                    if !has_ident(&text, &o.name) {
                        missing.push(format!("{rel}: {} has no `{}` ({:?})", t.name(), o.name, o.what));
                    }
                }
            }
        }
        missing.sort();
        missing.dedup();
        assert!(missing.is_empty(), "named here but not written:\n{}", missing.join("\n"));
        assert!(read >= 100, "fewer rules of the corpus were read than there are: {read}");
    }
}
