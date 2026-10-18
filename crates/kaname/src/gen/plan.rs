//! The plan of a module's code in a target (DESIGN 10.2–10.5): every function's body as the tree of
//! [`super::ex`], read from what the checks found of the module (each expression's type and what its
//! names name) and from the analysis of ranges (which places are shown).
//!
//! The body does what the reference interpreter does (DESIGN 8), in the same order, so that the
//! answer, the kind of a failure, its line and its message are the interpreter's
//! (`interp.rs`): the arguments and the fields from the left, the places they go to held after them
//! in the order declared, `and` and `or` from the left and the right not evaluated once the left
//! decides, `any`, `all` and `first` stopping at the element that decides them. A place the analysis
//! shows is not checked (DESIGN 10.4); one it does not show is, as the interpreter checks it.
//!
//! A value is held as the interpreter holds it, in the unit of its type: a whole number of its unit
//! as the target's integer (`bigint`, `int`, `int64`), anything else (a rate, an unrounded value)
//! as an exact fraction (`kaname.Q`, `Fraction`, `*big.Rat`). What a record holds, and what a
//! function takes and answers, is the value as it travels (DESIGN 9): an amount as the package's
//! type of its unit (`units`), a rate as the whole number of its step.

use super::ex::{Ex, Node, Op};
use super::{Options, Target};
use crate::ast::*;
use crate::check::Checked;
use crate::ranges::{Outcome, Place, What};
use crate::ty::*;
use crate::types::Ref;
use crate::value::Program;
use ritsu_base::text::{Lang, Text};
use ritsu_ports::{Column, ColumnType, RuleFacts};
use ritsu_units::{Dim, Rat, Unit};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

// ── Names ───────────────────────────────────────────────────────────────────

/// The type a record or an enum of a module is in the target: `Invoice` (from the alias).
pub fn type_name(t: Target, alias: &str) -> String {
    let p = ritsu_emit::ident::pascal(alias);
    match t {
        Target::Go => ritsu_emit::ident::go_exported(&p),
        _ => p,
    }
}

/// The member of a module's enum: `Tier.GOLD` (TypeScript, Python), `TierGold` (Go).
pub fn member_name(t: Target, enum_alias: &str, value_alias: &str) -> String {
    match t {
        Target::Go => format!("{}{}", type_name(t, enum_alias), ritsu_emit::ident::go_exported(&ritsu_emit::ident::pascal(value_alias))),
        _ => value_alias.to_uppercase(),
    }
}

/// The function a host calls: its alias (TypeScript, Python), in Pascal case in Go (`total_weight` is
/// `TotalWeight`). A Go function whose name a type of the module has (`fn claim` and `record Claim`
/// are both `Claim`) takes `Fn` after it.
pub fn fn_name(t: Target, c: &Checked, f: usize) -> String {
    let alias = c.decls.fns[f].name.ascii().to_string();
    match t {
        Target::Go => {
            let g = type_name(t, &alias);
            let types: BTreeSet<String> = c.decls.records.iter().map(|r| type_name(t, r.name.ascii())).chain(c.decls.enums.iter().map(|e| type_name(t, e.name.ascii()))).collect();
            if types.contains(&g) { format!("{g}Fn") } else { g }
        }
        _ => alias,
    }
}

/// The function of the module the others call, past the check of what a host gives (DESIGN 4.4):
/// `_claim`, `_weight_of`, and in Go `_weightOf`.
pub fn inner_name(t: Target, c: &Checked, f: usize) -> String {
    let a = c.decls.fns[f].name.ascii();
    match t {
        Target::Go => format!("_{}", go_camel(a)),
        _ => format!("_{a}"),
    }
}

/// `paid_on` → `paidOn`: a name inside Go's code (a parameter, a local, an unexported function), in
/// the mixed caps the package's other Go writes such names in (rulec's `roundDown`, koyomi's
/// `_daysFromCivil`). The parts between `_`, the first with its first letter in lower case and each
/// later one with its first letter in upper case; a letter with no case (Japanese) stays as it is.
pub fn go_camel(name: &str) -> String {
    let mut out = String::new();
    for (i, part) in name.split('_').filter(|p| !p.is_empty()).enumerate() {
        let mut cs = part.chars();
        let Some(c) = cs.next() else { continue };
        out.push(if i == 0 { c.to_ascii_lowercase() } else { c.to_ascii_uppercase() });
        out.push_str(cs.as_str());
    }
    out
}

/// The names of a function's parameters in the code of its body (DESIGN 10.3). TypeScript and
/// Python name a parameter by its alias, and one that a word the code reaches for (`BigInt`, Python's
/// `len`), a name of the module's code or an earlier parameter already has takes `_` after it
/// (`len_`; then `_2`, `_3`). Go names it in mixed caps (`paidOn`), and one taken so takes a number
/// after it, from 2 (`type2`, `paidOn2`). Python's function a host calls takes the aliases, the
/// keywords a host calls it with, and gives them on to the function of its body under these names.
pub fn param_names(g: &Gen, f: usize) -> Vec<String> {
    let t = g.t;
    let params = &g.c.decls.fns[f].params;
    let mut locals = Locals::default();
    match t {
        Target::Go => params.iter().map(|p| locals.fresh(t, p.name.ascii(), &g.outer)).collect(),
        _ => params
            .iter()
            .map(|p| {
                let a = p.name.ascii().to_string();
                let taken = |n: &str, l: &Locals| l.taken.contains(n) || g.outer.contains(n) || kept(t, n);
                let mut name = a.clone();
                let mut i = 1;
                while taken(&name, &locals) {
                    name = if i == 1 { format!("{a}_") } else { format!("{a}_{i}") };
                    i += 1;
                }
                locals.taken.insert(name.clone());
                name
            })
            .collect(),
    }
}

/// The alias of the type of what a function can fail with (DESIGN 4.5): `ClaimFails`.
pub fn fails_name(c: &Checked, f: usize) -> String {
    format!("{}Fails", ritsu_emit::ident::pascal(c.decls.fns[f].name.ascii()))
}

/// The Go package of a module, from its alias.
pub fn go_package(alias: &str) -> String {
    ritsu_emit::ident::go_package(alias)
}

/// A field of a record of a module in the target: its alias (Go's exported, in Pascal case).
pub fn field_name(t: Target, alias: &str) -> String {
    match t {
        Target::Go => ritsu_emit::ident::go_exported(&ritsu_emit::ident::pascal(alias)),
        _ => alias.to_string(),
    }
}

/// The words a name of the module's code may not be, besides its own names: the target's keywords,
/// and the globals the code reaches for.
fn kept(t: Target, w: &str) -> bool {
    match t {
        Target::TypeScript => {
            ritsu_emit::words::typescript::NAMES.contains(w) || ["BigInt", "Number", "String", "Math", "Array", "Object", "Error", "JSON", "Symbol", "Map", "Set", "Promise", "console", "kaname", "units"].contains(&w)
        }
        Target::Python => {
            ritsu_emit::words::python::NAMES.contains(w)
                || ["len", "int", "str", "list", "bool", "range", "min", "max", "abs", "isinstance", "print", "cast", "kaname", "enumerate", "zip", "object", "type", "None", "True", "False"].contains(&w)
        }
        Target::Go => ritsu_emit::words::go::NAMES.contains(w) || ["kaname", "units", "err", "ctx", "fmt", "strings", "big"].contains(&w),
    }
}

/// Whether a name of the source is a name the target takes as it is.
fn ident_ok(t: Target, w: &str) -> bool {
    let mut cs = w.chars();
    let first_ok = |c: char| c == '_' || c.is_alphabetic();
    matches!(cs.next(), Some(c) if first_ok(c)) && w.chars().all(|c| c == '_' || c.is_alphanumeric()) && !(t == Target::Go && w == "_")
}

/// The names of a function's code: each name of the source (a `let`, a parameter of a function given
/// to a builtin, the value `some` names) as a name of the target, and the target's own temporaries,
/// none the same as another or as a name the module's code uses.
#[derive(Default)]
struct Locals {
    taken: BTreeSet<String>,
    next: usize,
}

impl Locals {
    /// A name for a value of the source, from its name: as it is in TypeScript and Python, in mixed
    /// caps in Go (`days_late` is `daysLate`). One that is taken (an earlier name of the function, a
    /// name of the module's code, a word of the target) takes a number after it, from 2: `_2` in
    /// TypeScript and Python, and the number alone in Go (`daysLate2`), which writes no `_` in a
    /// name of several words.
    fn fresh(&mut self, t: Target, base: &str, outer: &BTreeSet<String>) -> String {
        let base = if t == Target::Go { go_camel(base) } else { base.to_string() };
        let base = if ident_ok(t, &base) { base } else { "v".to_string() };
        let mut name = base.clone();
        let mut i = 2;
        while self.taken.contains(&name) || outer.contains(&name) || kept(t, &name) {
            name = if t == Target::Go { format!("{base}{i}") } else { format!("{base}_{i}") };
            i += 1;
        }
        self.taken.insert(name.clone());
        name
    }

    fn temp(&mut self, t: Target, outer: &BTreeSet<String>) -> String {
        loop {
            self.next += 1;
            let n = format!("_t{}", self.next);
            if !self.taken.contains(&n) && !outer.contains(&n) {
                self.taken.insert(n.clone());
                return n;
            }
            let _ = t;
        }
    }
}

// ── Representations ─────────────────────────────────────────────────────────

/// How a number is held while it is worked on: a whole number of its unit (the target's integer), or
/// an exact fraction (DESIGN 10.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rep {
    Int,
    Q,
}

pub fn rep(n: &NumTy) -> Rep {
    if !n.is_rate() && n.step.is_some_and(|s| s.is_int()) { Rep::Int } else { Rep::Q }
}

fn num_of(t: &Ty) -> Option<&NumTy> {
    match t {
        Ty::Num(n) => Some(n),
        Ty::Opt(x) => num_of(x),
        _ => None,
    }
}

/// The step a declared rate travels in. Never 0: a rate on the step 0 is always 0 (the weight `0%` of
/// `running_totals`, which no declaration gives a step), and counts 0 on any step.
fn rate_step(n: &NumTy) -> Rat {
    n.unit.step.or(n.step).filter(|s| s.num != 0).unwrap_or(Rat::new(1, 100))
}

/// What a message writes after a number of the unit: `USDc`, `%` for a rate, nothing for a number.
fn unit_suffix(u: &Unit) -> String {
    match &u.dim {
        Dim::Rate => "%".into(),
        Dim::Number | Dim::Count(_) => String::new(),
        _ => u.unit.clone(),
    }
}

/// A value as the plan holds it: the expression of the target, and its type (the unit it counts in).
#[derive(Clone, Debug)]
pub struct Val {
    pub ex: Ex,
    pub ty: Ty,
    /// A value of an optional type that is there: one given where one may be absent, held as it is
    /// (TypeScript and Python), so that it travels as the value it is.
    pub present: bool,
}

impl Val {
    fn new(ex: Ex, ty: Ty) -> Val {
        Val { ex, ty, present: false }
    }
}

/// Where a value that leaves int64, or a divisor of 0, is said: the line and the message.
#[derive(Clone, Debug)]
pub struct At {
    pub line: usize,
    pub message: String,
}

/// A message of the interpreter with the parts the code fills in when it fails: each part a text, or
/// the name of a field of the `Outside` it found.
fn template(t: &Text, lang: Lang) -> Vec<Result<String, &'static str>> {
    let s = t.get(lang);
    let mut out: Vec<Result<String, &'static str>> = Vec::new();
    let mut rest = s;
    while let Some(at) = rest.find('\u{1}') {
        let (lit, more) = rest.split_at(at);
        if !lit.is_empty() {
            out.push(Ok(lit.to_string()));
        }
        let more = &more[1..];
        let end = more.find('\u{1}').unwrap_or(more.len());
        let key = match &more[..end] {
            "path" => "path",
            "shown" => "shown",
            "range" => "range",
            "count" => "count",
            _ => "path",
        };
        out.push(Err(key));
        rest = more.get(end + 1..).unwrap_or("");
    }
    if !rest.is_empty() {
        out.push(Ok(rest.to_string()));
    }
    out
}

const P: &str = "\u{1}path\u{1}";
const S: &str = "\u{1}shown\u{1}";
const R: &str = "\u{1}range\u{1}";
const N: &str = "\u{1}count\u{1}";

/// The two messages of a place a value does not keep (one for a value outside its range, one for a
/// list outside its count), with the parts the code fills in (DESIGN 8, `interp.rs`).
#[derive(Clone, Debug)]
pub enum Said {
    /// `check_entry`: the argument a host gives.
    Entry { fun: String },
    /// `place_args`: the argument one function gives another.
    Given { fun: String },
    /// `finish`: what a function answers.
    Answer { fun: String },
    /// `make_record`: a field of a record built.
    Field { rec: String },
}

impl Said {
    fn texts(&self) -> (Text, Text) {
        match self {
            Said::Entry { fun } => (
                tr!("関数 `{fun}` の引数 `{P}` の要素は {N} 個で、要素の数の範囲 {R} の外です", "The argument `{P}` of `{fun}` has {N} elements, outside its count {R}"),
                tr!("関数 `{fun}` の引数 `{P}` の {S} は、範囲 {R} の外です", "The argument `{P}` of `{fun}` is {S}, outside its range {R}"),
            ),
            Said::Given { fun } => (
                tr!("関数 `{fun}` に渡す引数 `{P}` の要素は {N} 個で、要素の数の範囲 {R} の外です", "The argument `{P}` given to `{fun}` has {N} elements, outside its count {R}"),
                tr!("関数 `{fun}` に渡す引数 `{P}` の {S} は、範囲 {R} の外です", "The argument `{P}` given to `{fun}` is {S}, outside its range {R}"),
            ),
            Said::Answer { fun } => (
                tr!("関数 `{fun}` の答えの要素は {N} 個で、要素の数の範囲 {R} の外です", "The answer of `{fun}` has {N} elements, outside its count {R}"),
                tr!("関数 `{fun}` の答え {S} は、範囲 {R} の外です", "The answer of `{fun}`, {S}, is outside its range {R}"),
            ),
            Said::Field { rec } => (
                tr!("{rec} のフィールド `{P}` に置く要素は {N} 個で、要素の数の範囲 {R} の外です", "The field `{P}` of {rec} is given {N} elements, outside its count {R}"),
                tr!("{rec} のフィールド `{P}` に置く {S} は、範囲 {R} の外です", "The field `{P}` of {rec} is given {S}, outside its range {R}"),
            ),
        }
    }
}

// ── The module ──────────────────────────────────────────────────────────────

/// A check of what a place declares, as a function of the module.
#[derive(Clone)]
pub struct Outside {
    /// The slot it checks (its type, range, count and whether a parameter reaches it), as a key.
    pub key: String,
    pub name: String,
    pub body: Vec<Node>,
    /// The type of the value it takes, as that travels.
    pub ty: String,
}

/// What the plan of a module reads and what it makes along the way: the imports it uses, the units
/// whose types it names, the checks of what a place declares, the orders of the enums it compares.
pub struct Gen<'a> {
    pub t: Target,
    pub lang: Lang,
    pub p: Program,
    pub c: Rc<Checked>,
    pub opts: &'a Options,
    /// Every place of every function, by the expression it is at.
    places: BTreeMap<usize, Vec<Place>>,
    /// The imports the code uses, by the name it reads them by: what each is.
    pub used: RefCell<BTreeMap<String, Import>>,
    /// The units whose types the code names (`units.USDc`).
    pub units: RefCell<Vec<Unit>>,
    /// The checks of what a place declares, each a function of the module: its key, its name, and
    /// its definition.
    pub outsides: RefCell<Vec<Outside>>,
    /// The enums the code compares, and the name of the function that gives a value's place.
    pub ranks: RefCell<BTreeMap<EnumId, String>>,
    /// The names the module's code uses outside its functions' bodies, which a local does not take.
    pub outer: BTreeSet<String>,
}

/// What a module of the package the code imports is.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Import {
    Kaname,
    Units,
    /// Python's `datetime`.
    Datetime,
    /// A rule's module: its file on the disk.
    Rule(PathBuf),
    Dates(PathBuf),
    /// Another module of kaname.
    Module(PathBuf),
    /// The types of the transfers of a book (`glue/books/<book>`).
    Book(PathBuf),
}

impl<'a> Gen<'a> {
    pub fn new(c: &Rc<Checked>, t: Target, opts: &'a Options) -> Gen<'a> {
        let p = Program::of(c);
        let mut places: BTreeMap<usize, Vec<Place>> = BTreeMap::new();
        if let Some(a) = c.analysis.get() {
            for f in &a.fns {
                for pl in &f.places {
                    places.entry(pl.expr).or_default().push(pl.clone());
                }
            }
        }
        let mut outer: BTreeSet<String> = BTreeSet::new();
        for (f, _) in c.decls.fns.iter().enumerate() {
            outer.insert(fn_name(t, c, f));
            outer.insert(inner_name(t, c, f));
            outer.insert(fails_name(c, f));
        }
        for r in &c.decls.records {
            outer.insert(type_name(t, r.name.ascii()));
            outer.insert(format!("_outside_{}", type_name(t, r.name.ascii())));
        }
        for e in &c.decls.enums {
            outer.insert(type_name(t, e.name.ascii()));
            if t == Target::Go {
                for v in &e.values {
                    outer.insert(member_name(t, e.name.ascii(), v.ascii()));
                }
            }
        }
        for w in ["_module", "_dt", "_units", "kaname", "units"] {
            outer.insert(w.to_string());
        }
        let g = Gen { t, lang: opts.lang, p, c: c.clone(), opts, places, used: RefCell::default(), units: RefCell::default(), outsides: RefCell::default(), ranks: RefCell::default(), outer };
        // the names the imports are read by
        let mut outer = g.outer.clone();
        for t in g.c.uses.iter() {
            let imp = match t {
                crate::names::Target::Rule { canon, .. } => Import::Rule(canon.clone()),
                crate::names::Target::Dates { canon, .. } => Import::Dates(canon.clone()),
                crate::names::Target::Book { canon, .. } => Import::Book(canon.clone()),
                crate::names::Target::Module(m) => Import::Module(m.canon.clone()),
            };
            outer.insert(g.import_name(&imp));
        }
        Gen { outer, ..g }
    }

    pub fn say(&self, t: &Text) -> String {
        t.get(self.lang).to_string()
    }

    /// The name the code reads an import by, and the import, kept as used.
    pub fn imp(&self, i: Import) -> String {
        let n = self.import_name(&i);
        self.used.borrow_mut().insert(n.clone(), i);
        n
    }

    /// The name an import is read by.
    pub fn import_name(&self, i: &Import) -> String {
        let t = self.t;
        match i {
            Import::Kaname => "kaname".into(),
            Import::Units => if t == Target::Python { "_units" } else { "units" }.into(),
            Import::Datetime => "_dt".into(),
            Import::Rule(f) => {
                let alias = self.p.rules.get(f).map(|r| r.alias.clone()).unwrap_or_default();
                match t {
                    Target::TypeScript => format!("rule_{alias}"),
                    Target::Python => format!("_rule_{alias}"),
                    Target::Go => go_package(&alias),
                }
            }
            Import::Dates(f) => {
                let alias = self.p.dates.get(f).map(|d| d.alias.clone()).unwrap_or_default();
                match t {
                    Target::TypeScript => format!("dates_{alias}"),
                    Target::Python => format!("_dates_{alias}"),
                    Target::Go => go_package(&alias),
                }
            }
            Import::Module(f) => {
                let alias = self.p.module(f).module.name.ascii().to_string();
                match t {
                    Target::TypeScript => format!("mod_{alias}"),
                    Target::Python => format!("_mod_{alias}"),
                    Target::Go => go_package(&alias),
                }
            }
            Import::Book(f) => {
                let b = self.p.books.get(f).map(|b| b.name.clone()).unwrap_or_default();
                match t {
                    Target::TypeScript => format!("books_{b}"),
                    Target::Python => format!("_books_{b}"),
                    Target::Go => super::books::go_package_of(&self.p, f),
                }
            }
        }
    }

    // ── What the analysis says of a place ──────────────────────────────────

    /// The places at an expression that `pick` takes and the analysis does not show: what the code
    /// checks there (DESIGN 10.4).
    fn open(&self, e: usize, pick: impl Fn(&What) -> bool) -> bool {
        self.places.get(&e).is_some_and(|ps| ps.iter().any(|p| pick(&p.what) && !matches!(p.outcome, Outcome::Shown)))
    }

    /// Where an operation at `e` is held to int64 by the code: the place of `overflow` that the
    /// analysis does not show (None where it shows it, or finds no operation that can leave int64).
    pub fn overflow_at(&self, e: &Expr) -> Option<At> {
        self.open(e.id, |w| matches!(w, What::Overflow)).then(|| {
            let src = self.src(e.span);
            At { line: e.span.line, message: self.say(&tr!("`{src}` の値が int64 に収まりません", "The value of `{src}` does not fit in int64")) }
        })
    }

    pub fn zero_at(&self, e: &Expr) -> Option<At> {
        self.open(e.id, |w| matches!(w, What::Zero)).then(|| {
            let src = self.src(e.span);
            At { line: e.span.line, message: self.say(&tr!("`{src}` の割る数が 0 です", "The divisor in `{src}` is 0")) }
        })
    }

    /// The source of an expression on its line, as written.
    pub fn src(&self, sp: Span) -> String {
        let line = self.c.src.lines().nth(sp.line.saturating_sub(1)).unwrap_or("");
        let cs: Vec<char> = line.chars().collect();
        let (a, b) = (sp.col.saturating_sub(1).min(cs.len()), sp.end.saturating_sub(1).min(cs.len()));
        cs[a..b.max(a)].iter().collect::<String>().trim().to_string()
    }

    /// The line of the source, as written: the comment a statement of the code is read with.
    pub fn line_of(&self, line: usize) -> String {
        self.c.src.lines().nth(line.saturating_sub(1)).unwrap_or("").trim().to_string()
    }

    // ── Literals ───────────────────────────────────────────────────────────

    pub fn int(&self, v: i128) -> Ex {
        let s = match self.t {
            Target::TypeScript => format!("{v}n"),
            _ => v.to_string(),
        };
        if v < 0 { Ex::Atom(format!("({s})")) } else { Ex::Atom(s) }
    }

    pub fn frac(&self, v: Rat) -> Ex {
        let k = self.imp(Import::Kaname);
        match self.t {
            Target::TypeScript => Ex::call(format!("{k}.q"), vec![Ex::atom(format!("{}n", v.num)), Ex::atom(format!("{}n", v.den))]),
            Target::Python => Ex::call(format!("{k}.Q"), vec![Ex::atom(v.num.to_string()), Ex::atom(v.den.to_string())]),
            Target::Go => Ex::call(format!("{k}.Frac"), vec![Ex::atom(v.num.to_string()), Ex::atom(v.den.to_string())]),
        }
    }

    pub fn string(&self, s: &str) -> Ex {
        Ex::Atom(match self.t {
            Target::Go => ritsu_emit::lit::go(s),
            _ => ritsu_emit::lit::json(s),
        })
    }

    pub fn boolean(&self, b: bool) -> Ex {
        Ex::Atom(match (self.t, b) {
            (Target::Python, true) => "True".into(),
            (Target::Python, false) => "False".into(),
            (_, b) => b.to_string(),
        })
    }

    /// Whether `x` is the absent value, written out.
    pub fn absent(&self, x: &Ex) -> bool {
        matches!(x, Ex::Atom(a) if a == "null" || a == "None" || a == "nil")
    }

    pub fn null(&self) -> Ex {
        Ex::Atom(match self.t {
            Target::TypeScript => "null",
            Target::Python => "None",
            Target::Go => "nil",
        }
        .into())
    }

    pub fn date(&self, d: i64) -> Ex {
        match self.t {
            Target::TypeScript => self.string(&crate::date::text(d)),
            Target::Python => {
                let (y, m, dd) = crate::date::civil(d);
                Ex::call(format!("{}.date", self.imp(Import::Datetime)), vec![Ex::atom(y.to_string()), Ex::atom(m.to_string()), Ex::atom(dd.to_string())])
            }
            Target::Go => {
                let (y, m, dd) = crate::date::civil(d);
                let k = self.imp(Import::Kaname);
                Ex::Record(format!("{k}.Date"), vec![("Year".into(), Ex::atom(y.to_string())), ("Month".into(), Ex::atom(m.to_string())), ("Day".into(), Ex::atom(dd.to_string()))])
            }
        }
    }

    /// The module's name, as the code passes it to an error.
    pub fn module_const(&self) -> Ex {
        Ex::atom(if self.t == Target::Python { "_MODULE" } else { "_module" })
    }

    /// The three arguments of where an operation fails: the module, the line, the message.
    pub fn at_args(&self, at: &At) -> Vec<Ex> {
        vec![self.module_const(), Ex::atom(at.line.to_string()), self.string(&at.message)]
    }

    pub fn kcall(&self, f: &str, args: Vec<Ex>) -> Ex {
        let k = self.imp(Import::Kaname);
        Ex::call(format!("{k}.{f}"), args)
    }

    /// The name of a helper of `kaname` in this target: `heldInt`, `held_int`, `Add`.
    fn helper(&self, ts: &str, py: &str, go: &str) -> String {
        match self.t {
            Target::TypeScript => ts,
            Target::Python => py,
            Target::Go => go,
        }
        .to_string()
    }

    // ── Types ───────────────────────────────────────────────────────────────

    /// The type of the target a number of `unit` travels as: the package's type of its unit
    /// (`units.USDc`), a plain integer for a number, a rate (a count of its step) and a count.
    pub fn unit_type(&self, u: &Unit) -> String {
        match u.type_name() {
            Some(name) => {
                {
                    let mut us = self.units.borrow_mut();
                    let plain = Unit { step: None, ..u.clone() };
                    if !us.contains(&plain) {
                        us.push(plain);
                    }
                }
                format!("{}.{name}", self.imp(Import::Units))
            }
            None => self.int_type().into(),
        }
    }

    fn int_type(&self) -> &'static str {
        match self.t {
            Target::TypeScript => "bigint",
            Target::Python => "int",
            Target::Go => "int64",
        }
    }

    fn q_type(&self) -> String {
        format!("{}.Q", self.imp(Import::Kaname))
    }

    fn date_type(&self) -> String {
        match self.t {
            Target::TypeScript => "string".into(),
            Target::Python => format!("{}.date", self.imp(Import::Datetime)),
            Target::Go => format!("{}.Date", self.imp(Import::Kaname)),
        }
    }

    fn list_type(&self, inner: String) -> String {
        match self.t {
            Target::TypeScript => {
                if inner.contains(' ') { format!("readonly ({inner})[]") } else { format!("readonly {inner}[]") }
            }
            Target::Python => format!("list[{inner}]"),
            Target::Go => format!("[]{inner}"),
        }
    }

    fn opt_type(&self, inner: String) -> String {
        match self.t {
            Target::TypeScript => format!("{inner} | null"),
            Target::Python => format!("{inner} | None"),
            Target::Go => format!("*{inner}"),
        }
    }

    /// A rule's enum or a module's, as the target names it.
    pub fn enum_type(&self, id: &EnumId) -> String {
        match &id.owner {
            Owner::Module(f) => {
                let m = self.p.module(f);
                let alias = m.decls.enums.iter().find(|e| e.name.text == id.name).map(|e| e.name.ascii().to_string()).unwrap_or_default();
                let n = type_name(self.t, &alias);
                if *f == self.c.canon { n } else { format!("{}.{n}", self.imp(Import::Module(f.clone()))) }
            }
            Owner::Rule(f) => {
                let alias = self.rule_enum(f, &id.name).map(|e| e.alias.clone()).unwrap_or_default();
                format!("{}.{alias}", self.imp(Import::Rule(f.clone())))
            }
        }
    }

    /// The enum of a rule as the port says it, and as the rule's code names it in the target.
    fn rule_enum(&self, f: &Path, name: &str) -> Option<ritsu_ports::CallEnum> {
        let facts = self.p.rules.get(f)?;
        let call = self.call_of(facts);
        call.enums.iter().find(|e| e.name == name).cloned()
    }

    fn call_of<'r>(&self, facts: &'r RuleFacts) -> &'r ritsu_ports::Call {
        match self.t {
            Target::TypeScript => &facts.typescript,
            Target::Python => &facts.python,
            Target::Go => &facts.go,
        }
    }

    /// A value of an enum: `Tier.GOLD`, `rule_x.Urgency.OVERDUE`, `TierGold`, `x.UrgencyOverdue`.
    pub fn member(&self, id: &EnumId, value: &str) -> Ex {
        match &id.owner {
            Owner::Module(f) => {
                let m = self.p.module(f);
                let Some(e) = m.decls.enums.iter().find(|e| e.name.text == id.name) else { return self.string(value) };
                let v = e.values.iter().find(|v| v.text == value).map(|v| v.ascii().to_string()).unwrap_or_else(|| value.to_string());
                let ea = e.name.ascii().to_string();
                let local = f == &self.c.canon;
                match self.t {
                    Target::Go => {
                        let n = member_name(self.t, &ea, &v);
                        if local { Ex::atom(n) } else { Ex::atom(format!("{}.{n}", self.imp(Import::Module(f.clone())))) }
                    }
                    _ => {
                        let ty = type_name(self.t, &ea);
                        let ty = if local { ty } else { format!("{}.{ty}", self.imp(Import::Module(f.clone()))) };
                        Ex::atom(format!("{ty}.{}", member_name(self.t, &ea, &v)))
                    }
                }
            }
            Owner::Rule(f) => {
                let Some(e) = self.rule_enum(f, &id.name) else { return self.string(value) };
                let member = e.values.iter().find(|(n, _)| n == value).map(|(_, m)| m.clone()).unwrap_or_else(|| value.to_string());
                let r = self.imp(Import::Rule(f.clone()));
                match self.t {
                    Target::Go => Ex::atom(format!("{r}.{member}")),
                    _ => Ex::atom(format!("{r}.{}.{member}", e.alias)),
                }
            }
        }
    }

    /// The values of an enum, in order, by their names.
    pub fn enum_values(&self, id: &EnumId) -> Vec<String> {
        self.p.values(id).0.iter().map(|n| n.text.clone()).collect()
    }

    /// A record of a module, of a rule, or of `running_totals`, as the target names it.
    pub fn record_type(&self, id: &RecId) -> String {
        match id {
            RecId::Module { file, name } => {
                let m = self.p.module(file);
                let alias = m.decls.records.iter().find(|r| r.name.text == *name).map(|r| r.name.ascii().to_string()).unwrap_or_default();
                let n = type_name(self.t, &alias);
                if *file == self.c.canon { n } else { format!("{}.{n}", self.imp(Import::Module(file.clone()))) }
            }
            RecId::Outputs(f) => {
                let facts = self.p.rules.get(f).cloned();
                match facts {
                    Some(facts) if facts.outputs.len() == 1 => self.rule_col_type(f, &facts.outputs[0]),
                    _ => format!("{}.Output", self.imp(Import::Rule(f.clone()))),
                }
            }
            RecId::Element(f, seq) => format!("{}.{}", self.imp(Import::Rule(f.clone())), self.element_type(f, seq)),
            RecId::Running(item, w) => {
                let k = self.imp(Import::Kaname);
                let (i, w) = (self.boundary(item), self.boundary(w));
                match self.t {
                    Target::TypeScript => format!("{k}.Running<{i}, {w}>"),
                    Target::Python => format!("{k}.Running[{i}, {w}]"),
                    Target::Go => format!("{k}.Running[{i}, {w}]"),
                }
            }
        }
    }

    /// The type a rule's code gives one element of a list it walks: what the parameter of the list is a
    /// list of (`readonly Element[]`, `list[Element]`, `[]Element`).
    fn element_type(&self, f: &Path, seq: &str) -> String {
        let Some(facts) = self.p.rules.get(f) else { return "Element".into() };
        let call = self.call_of(facts);
        let ty = call.params.iter().find(|p| p.name == seq).map(|p| p.ty.clone()).unwrap_or_default();
        let s = ty.trim();
        let s = s.strip_prefix("readonly ").unwrap_or(s);
        s.strip_prefix("list[").and_then(|x| x.strip_suffix(']')).or_else(|| s.strip_prefix("[]")).or_else(|| s.strip_suffix("[]")).unwrap_or(s).to_string()
    }

    /// The type a rule's code gives one of its columns (an input, an output, a field of an element).
    pub fn rule_col_type(&self, f: &Path, c: &Column) -> String {
        self.rule_type(f, &c.ty)
    }

    fn rule_type(&self, f: &Path, t: &ColumnType) -> String {
        match t {
            ColumnType::Bool => self.bool_type().into(),
            ColumnType::Str => self.str_type().into(),
            ColumnType::Date => self.int_type().into(),
            ColumnType::Enum { name: n, .. } => match self.rule_enum(f, n) {
                Some(e) => format!("{}.{}", self.imp(Import::Rule(f.to_path_buf())), e.alias),
                None => self.str_type().into(),
            },
            ColumnType::Num { unit: Some(u), .. } if u.dim == Dim::Rate => format!("{}.Rate", self.imp(Import::Rule(f.to_path_buf()))),
            ColumnType::Num { unit: Some(u), .. } => self.unit_type(u),
            ColumnType::Num { unit: None, .. } => self.int_type().into(),
            ColumnType::Opt(x) => self.opt_type(self.rule_type(f, x)),
        }
    }

    fn bool_type(&self) -> &'static str {
        match self.t {
            Target::TypeScript => "boolean",
            _ => "bool",
        }
    }

    fn str_type(&self) -> &'static str {
        match self.t {
            Target::Python => "str",
            _ => "string",
        }
    }

    /// The type of a value of `t` while it is worked on (DESIGN 10.2).
    pub fn flight(&self, t: &Ty) -> String {
        match t {
            Ty::Num(n) => match rep(n) {
                Rep::Int => self.int_type().into(),
                Rep::Q => self.q_type(),
            },
            Ty::Bool => self.bool_type().into(),
            Ty::Str => self.str_type().into(),
            Ty::Date => self.date_type(),
            Ty::Enum(id) => self.enum_type(id),
            Ty::Record(id) => self.record_type(id),
            Ty::List(x) => self.list_type(self.flight(x)),
            Ty::Opt(x) => self.opt_type(self.flight(x)),
            Ty::Transfer(id) => self.transfer_type(id),
            Ty::None | Ty::Empty | Ty::Error => match self.t {
                Target::TypeScript => "null".into(),
                Target::Python => "None".into(),
                Target::Go => "any".into(),
            },
        }
    }

    /// The type of a value of `t` as it travels: what a function takes and answers, what a record
    /// holds (DESIGN 9): an amount as the package's type of its unit, a rate as a count of its step.
    pub fn boundary(&self, t: &Ty) -> String {
        match t {
            Ty::Num(n) if n.is_rate() => self.int_type().into(),
            Ty::Num(n) => self.unit_type(&n.unit),
            Ty::List(x) => self.list_type(self.boundary(x)),
            Ty::Opt(x) => self.opt_type(self.boundary(x)),
            t => self.flight(t),
        }
    }

    /// The type of a transfer's operation, from the types of its book (`glue/books/<book>`).
    pub fn transfer_type(&self, id: &TransferId) -> String {
        format!("{}.{}", self.imp(Import::Book(id.book.clone())), super::books::op_type(self.t, &id.transfer, &id.op))
    }

    /// Whether a value of `t` as it is worked on is taken where it travels (a parameter, a field, an
    /// answer) with no code between. An amount travels as the package's type of its unit: a subtype
    /// of the integer in TypeScript (taken by a cast), a `NewType` in Python (made by a call), a type
    /// of its own in Go (a conversion); a rate travels as a count of its step.
    fn same_out(&self, t: &Ty) -> bool {
        match t {
            Ty::Num(n) if n.is_rate() => false,
            Ty::Num(n) => n.unit.type_name().is_none(),
            Ty::Opt(x) | Ty::List(x) => self.same_out(x),
            _ => true,
        }
    }

    /// Whether a value of `t` as it travels is taken where it is worked on with no code between: an
    /// amount of the package's type is an integer in TypeScript and Python, but a list of them is no
    /// list of integers in Python (a list is invariant), and Go converts each.
    fn same_in(&self, t: &Ty) -> bool {
        match t {
            Ty::Num(n) if n.is_rate() => false,
            Ty::Num(n) => n.unit.type_name().is_none() || self.t != Target::Go,
            Ty::List(x) => match &**x {
                Ty::Num(n) if !n.is_rate() && n.unit.type_name().is_some() => self.t == Target::TypeScript,
                x => self.same_in(x),
            },
            Ty::Opt(x) => self.same_in(x),
            _ => true,
        }
    }

    // ── The checks of what a place declares ────────────────────────────────

    /// The function that finds where a value of `slot`, as it travels, is outside what the slot
    /// declares (`interp.rs`'s `outside`, with `reached` for a parameter): its name, made once.
    pub fn outside_fn(&self, slot: &Slot, reached: bool) -> Option<String> {
        if !self.checkable(&slot.ty, slot.range.is_some() || slot.count.is_some(), reached, 0) {
            return None;
        }
        let key = format!("{:?}|{:?}|{:?}|{reached}", slot.ty, slot.range, slot.count);
        if let Some(o) = self.outsides.borrow().iter().find(|o| o.key == key) {
            return Some(o.name.clone());
        }
        let n = self.outsides.borrow().len() + 1;
        let name = format!("_outside{n}");
        // made first, so that a record that holds itself is not made forever (it cannot: E108)
        self.outsides.borrow_mut().push(Outside { key: key.clone(), name: name.clone(), body: Vec::new(), ty: String::new() });
        let v = Ex::atom("v");
        let what = Ex::atom("what");
        let body = self.outside_body(&v, &slot.ty, slot.range.as_ref(), slot.count.as_ref(), what, reached, 0);
        let ty = self.boundary(&slot.ty);
        if let Some(o) = self.outsides.borrow_mut().iter_mut().find(|o| o.key == key) {
            o.body = body;
            o.ty = ty;
        }
        Some(name)
    }

    /// Whether a value of `t` can be outside what its place declares.
    fn checkable(&self, t: &Ty, declared: bool, reached: bool, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        match t {
            Ty::Opt(x) => self.checkable(x, declared, reached, depth),
            Ty::List(x) => declared || self.checkable(x, false, reached, depth),
            Ty::Num(n) => declared || (reached && n.is_rate() && n.step.is_some()),
            Ty::Date => declared,
            Ty::Record(RecId::Module { .. }) if reached => self.record_checkable(t, depth),
            _ => false,
        }
    }

    /// Whether a check of a value of `slot` can find a list outside its count.
    pub fn counts(&self, slot: &Slot, reached: bool) -> bool {
        self.counts_in(&slot.ty, slot.count.is_some(), reached, 0)
    }

    fn counts_in(&self, t: &Ty, counted: bool, reached: bool, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        match t {
            Ty::Opt(x) => self.counts_in(x, counted, reached, depth),
            Ty::List(x) => counted || self.counts_in(x, false, reached, depth + 1),
            Ty::Record(id @ RecId::Module { .. }) if reached => self.p.fields(id).iter().any(|(_, s)| self.counts_in(&s.ty, s.count.is_some(), true, depth + 1)),
            _ => false,
        }
    }

    pub fn record_checkable(&self, t: &Ty, depth: usize) -> bool {
        let Ty::Record(id @ RecId::Module { .. }) = t else { return false };
        self.p.fields(id).iter().any(|(_, s)| self.checkable(&s.ty, s.range.is_some() || s.count.is_some(), true, depth + 1))
    }

    /// The body of a check of a value `v` of `t`: a return of the first place it is outside, the
    /// path written from `what`.
    #[allow(clippy::too_many_arguments)]
    fn outside_body(&self, v: &Ex, t: &Ty, range: Option<&Range>, count: Option<&Range>, what: Ex, reached: bool, depth: usize) -> Vec<Node> {
        let mut out = Vec::new();
        match t {
            Ty::Opt(x) => {
                let (inner, deref) = match self.t {
                    Target::Go => (Ex::Deref(Box::new(v.clone())), true),
                    _ => (v.clone(), false),
                };
                let _ = deref;
                let body = self.outside_body(&inner, x, range, count, what, reached, depth);
                if !body.is_empty() {
                    out.push(Node::If(vec![(Ex::bin(Op::Ne, v.clone(), self.null()), body)], Vec::new()));
                }
            }
            Ty::List(x) => {
                let len = self.len_of(v);
                if let Some(c) = count {
                    let mut tests = Vec::new();
                    if let Some(lo) = c.min {
                        tests.push(Ex::bin(Op::Lt, len.clone(), Ex::atom(lo.num.to_string())));
                    }
                    if let Some(hi) = c.max {
                        tests.push(Ex::bin(Op::Gt, len.clone(), Ex::atom(hi.num.to_string())));
                    }
                    if let Some(test) = tests.into_iter().reduce(|a, b| Ex::bin(Op::Or, a, b)) {
                        let shown = crate::value::range_text(c, &Ty::Num(NumTy::number()));
                        out.push(Node::If(vec![(test, vec![Node::Line(self.ret(self.outside_value(what.clone(), self.string(""), self.string(&shown), Some(len.clone()))))])], Vec::new()));
                    }
                }
                let elem = match &**x {
                    Ty::Opt(y) => (**y).clone(),
                    y => y.clone(),
                };
                let opt = matches!(**x, Ty::Opt(_));
                let i = Ex::atom(format!("i{depth}"));
                let el = Ex::atom(format!("e{depth}"));
                let path = match self.t {
                    Target::TypeScript => Ex::bin(Op::Add, Ex::bin(Op::Add, Ex::bin(Op::Add, what.clone(), self.string("[")), i.clone()), self.string("]")),
                    Target::Python => Ex::bin(Op::Add, Ex::bin(Op::Add, Ex::bin(Op::Add, what.clone(), self.string("[")), Ex::call("str", vec![i.clone()])), self.string("]")),
                    Target::Go => self.kcall("Elem", vec![what.clone(), i.clone()]),
                };
                let inner = if opt {
                    let deref = if self.t == Target::Go { Ex::Deref(Box::new(el.clone())) } else { el.clone() };
                    let b = self.outside_body(&deref, &elem, range, None, path, reached, depth + 1);
                    if b.is_empty() { b } else { vec![Node::If(vec![(Ex::bin(Op::Ne, el.clone(), self.null()), b)], Vec::new())] }
                } else {
                    self.outside_body(&el, &elem, range, None, path, reached, depth + 1)
                };
                if !inner.is_empty() {
                    let head = match self.t {
                        Target::TypeScript => format!("for (let {} = 0; {} < {}.length; {}++)", i.print(self.t), i.print(self.t), v.print(self.t), i.print(self.t)),
                        Target::Python => format!("for {}, {} in enumerate({})", i.print(self.t), el.print(self.t), v.print(self.t)),
                        Target::Go => format!("for {}, {} := range {}", i.print(self.t), el.print(self.t), v.print(self.t)),
                    };
                    let mut body = Vec::new();
                    if self.t == Target::TypeScript {
                        body.push(Node::Line(format!("const {} = {}[{}];", el.print(self.t), v.print(self.t), i.print(self.t))));
                    }
                    body.extend(inner);
                    out.push(Node::Block(head, body, Vec::new()));
                }
            }
            _ => {
                let range = match (range, t) {
                    (Some(r), _) => Some(r.clone()),
                    (None, Ty::Num(n)) if reached && n.is_rate() && n.step.is_some() => Some(Range { min: Some(Rat::zero()), max: Some(Rat::int(1)) }),
                    _ => None,
                };
                if let Some(r) = range
                    && let Some(test) = self.out_of(v, t, &r)
                {
                    let shown = self.shown(v, t);
                    let rt = crate::value::range_text(&r, t);
                    out.push(Node::If(vec![(test, vec![Node::Line(self.ret(self.outside_value(what.clone(), shown, self.string(&rt), None)))])], Vec::new()));
                }
                if reached && self.record_checkable(t, depth) {
                    let Ty::Record(id) = t else { return out };
                    let o = Ex::atom(format!("o{depth}"));
                    let call = self.record_check_call(id, v.clone(), what);
                    out.extend(self.if_found(&o, call, vec![Node::Line(self.ret(o.clone()))]));
                }
            }
        }
        out
    }

    /// `if o := call; o != nil { … }`, and the same in TypeScript and Python.
    pub fn if_found(&self, o: &Ex, call: Ex, then: Vec<Node>) -> Vec<Node> {
        match self.t {
            Target::Go => vec![Node::If(vec![(Ex::atom(format!("{} := {}; {} != nil", o.print(self.t), call.print(self.t), o.print(self.t))), then)], Vec::new())],
            Target::TypeScript => vec![Node::Line(format!("const {} = {};", o.print(self.t), call.print(self.t))), Node::If(vec![(Ex::bin(Op::Ne, o.clone(), self.null()), then)], Vec::new())],
            Target::Python => vec![Node::Line(format!("{} = {}", o.print(self.t), call.print(self.t))), Node::If(vec![(Ex::atom(format!("{} is not None", o.print(self.t))), then)], Vec::new())],
        }
    }

    /// The check of a record of a module, reached from a parameter.
    pub fn record_check_call(&self, id: &RecId, v: Ex, what: Ex) -> Ex {
        let RecId::Module { file, name } = id else { return self.null() };
        let m = self.p.module(file);
        let alias = m.decls.records.iter().find(|r| r.name.text == *name).map(|r| r.name.ascii().to_string()).unwrap_or_default();
        match self.t {
            Target::Go => Ex::Call(Box::new(v.dot("KanameOutside")), vec![what]),
            _ => {
                let f = format!("_outside_{}", type_name(self.t, &alias));
                let f = if *file == self.c.canon { f } else { format!("{}.{f}", self.imp(Import::Module(file.clone()))) };
                Ex::call(f, vec![v, what])
            }
        }
    }

    /// An `Outside` of the target.
    fn outside_value(&self, path: Ex, shown: Ex, range: Ex, count: Option<Ex>) -> Ex {
        let k = self.imp(Import::Kaname);
        match self.t {
            Target::TypeScript => Ex::Record(String::new(), vec![("path".into(), path), ("shown".into(), shown), ("range".into(), range), ("count".into(), count.map(|c| Ex::call("Number", vec![c])).unwrap_or(self.null()))]),
            Target::Python => Ex::Record(format!("{k}.Outside"), vec![("path".into(), path), ("shown".into(), shown), ("range".into(), range), ("count".into(), count.unwrap_or(self.null()))]),
            Target::Go => Ex::atom(format!(
                "&{}",
                Ex::Record(format!("{k}.Outside"), vec![("Path".into(), path), ("Shown".into(), shown), ("Range".into(), range), ("Count".into(), count.map(|c| Ex::call("int", vec![c])).unwrap_or(Ex::atom("-1")))]).print(self.t)
            )),
        }
    }

    pub fn ret(&self, e: Ex) -> String {
        match self.t {
            Target::TypeScript => format!("return {};", e.print(self.t)),
            _ => format!("return {}", e.print(self.t)),
        }
    }

    /// The length of a list.
    pub fn len_of(&self, v: &Ex) -> Ex {
        match self.t {
            Target::TypeScript => v.clone().dot("length"),
            Target::Python => Ex::call("len", vec![v.clone()]),
            Target::Go => Ex::call("len", vec![v.clone()]),
        }
    }

    /// The test of a value as it travels against a range (None for a type no range is of).
    fn out_of(&self, v: &Ex, t: &Ty, r: &Range) -> Option<Ex> {
        let mut tests = Vec::new();
        match t {
            Ty::Num(n) => {
                // a rate travels as a count of its step: the range as counts, the lower end up and the
                // upper end down to a whole count
                let (lo, hi) = if n.is_rate() {
                    let s = rate_step(n);
                    (r.min.map(|v| ceil(v.div(s))), r.max.map(|v| floor(v.div(s))))
                } else {
                    (r.min.map(ceil), r.max.map(floor))
                };
                let x = match self.t {
                    Target::Go if n.unit.type_name().is_some() && !n.is_rate() => Ex::call("int64", vec![v.clone()]),
                    _ => v.clone(),
                };
                if let Some(lo) = lo {
                    tests.push(Ex::bin(Op::Lt, x.clone(), self.int(lo)));
                }
                if let Some(hi) = hi {
                    tests.push(Ex::bin(Op::Gt, x, self.int(hi)));
                }
            }
            Ty::Date => {
                let x = match self.t {
                    Target::Go => Ex::Call(Box::new(v.clone().dot("Days")), vec![]),
                    _ => v.clone(),
                };
                let end = |d: Rat| match self.t {
                    Target::Go => Ex::atom(d.num.to_string()),
                    _ => self.date(d.num as i64),
                };
                if let Some(lo) = r.min {
                    tests.push(Ex::bin(Op::Lt, x.clone(), end(lo)));
                }
                if let Some(hi) = r.max {
                    tests.push(Ex::bin(Op::Gt, x, end(hi)));
                }
            }
            _ => return None,
        }
        tests.into_iter().reduce(|a, b| Ex::bin(Op::Or, a, b))
    }

    /// A value as it travels, as a message writes it: `1150GBPc`, `0.5%`, `2026-04-01`.
    fn shown(&self, v: &Ex, t: &Ty) -> Ex {
        match t {
            Ty::Num(n) if n.is_rate() => {
                let q = self.kcall(&self.helper("rate", "rate", "Rate"), vec![v.clone(), self.frac(rate_step(n))]);
                self.kcall(&self.helper("numText", "num_text", "NumText"), vec![q, self.string("%")])
            }
            Ty::Num(n) => {
                let x = match self.t {
                    Target::Go => Ex::call("int64", vec![v.clone()]),
                    _ => v.clone(),
                };
                self.kcall(&self.helper("numText", "num_text", "IntText"), vec![if self.t == Target::Go { x } else { self.to_q_ex(x) }, self.string(&unit_suffix(&n.unit))])
            }
            Ty::Date => match self.t {
                Target::TypeScript => v.clone(),
                Target::Python => self.kcall("date_text", vec![v.clone()]),
                Target::Go => Ex::Call(Box::new(v.clone().dot("String")), vec![]),
            },
            _ => self.string(""),
        }
    }

    /// A whole number as a fraction.
    pub fn to_q_ex(&self, x: Ex) -> Ex {
        match self.t {
            Target::TypeScript => self.kcall("q", vec![x]),
            Target::Python => self.kcall("Q", vec![x]),
            Target::Go => self.kcall("Of", vec![x]),
        }
    }

    /// A fraction whose type says it is whole, as a whole number.
    pub fn to_int_ex(&self, x: Ex) -> Ex {
        self.kcall(&self.helper("whole", "whole", "Whole"), vec![x])
    }

    /// The code that fails with a message the code fills in from the `Outside` `o` it found.
    pub fn fail_outside(&self, kind: &str, line: usize, said: &Said, o: &Ex, counts: bool) -> Vec<Node> {
        let (count, value) = said.texts();
        let part = |key: &str| -> Ex {
            let field = match (self.t, key) {
                (Target::Go, k) => {
                    let mut cs = k.chars();
                    cs.next().map(|c| c.to_ascii_uppercase().to_string() + cs.as_str()).unwrap_or_default()
                }
                (_, k) => k.to_string(),
            };
            let x = o.clone().dot(field);
            match (self.t, key) {
                (Target::TypeScript, "count") => Ex::call("String", vec![x]),
                // the helper's, not `str`, which a parameter of the function a host calls may hide
                (Target::Python, "count") => self.kcall("itoa", vec![x]),
                (Target::Go, "count") => self.kcall("Itoa", vec![x]),
                _ => x,
            }
        };
        let build = |tmpl: &Text| -> Ex {
            template(tmpl, self.lang).into_iter().map(|p| match p {
                Ok(s) => self.string(&s),
                Err(k) => part(k),
            })
            .reduce(|a, b| Ex::bin(Op::Add, a, b))
            .unwrap_or(self.string(""))
        };
        let fail = |msg: Ex| -> Node { Node::Line(self.stmt(self.kcall(&self.helper("fail", "fail", "Fail"), vec![self.string(kind), self.module_const(), Ex::atom(line.to_string()), msg]))) };
        let is_count = match self.t {
            Target::TypeScript => Ex::bin(Op::Ne, o.clone().dot("count"), self.null()),
            Target::Python => Ex::atom(format!("{}.count is not None", o.print(self.t))),
            Target::Go => Ex::bin(Op::Ge, o.clone().dot("Count"), Ex::atom("0")),
        };
        if counts {
            vec![Node::If(vec![(is_count, vec![fail(build(&count))])], Vec::new()), fail(build(&value))]
        } else {
            vec![fail(build(&value))]
        }
    }

    /// An expression as a statement of the target.
    pub fn stmt(&self, e: Ex) -> String {
        match self.t {
            Target::TypeScript => format!("{};", e.print(self.t)),
            _ => e.print(self.t),
        }
    }

    /// The function that gives a value of an enum its place in its order, made once.
    pub fn rank_of(&self, id: &EnumId) -> String {
        if let Some(n) = self.ranks.borrow().get(id) {
            return n.clone();
        }
        let n = format!("_rank{}", self.ranks.borrow().len() + 1);
        self.ranks.borrow_mut().insert(id.clone(), n.clone());
        n
    }
}

/// The least whole number not below `v`.
fn ceil(v: Rat) -> i128 {
    let f = v.num.div_euclid(v.den);
    if v.num.rem_euclid(v.den) == 0 { f } else { f + 1 }
}

/// The greatest whole number not above `v`.
fn floor(v: Rat) -> i128 {
    v.num.div_euclid(v.den)
}

// ── A function ──────────────────────────────────────────────────────────────

/// Where the value of a block goes.
#[derive(Clone)]
enum Sink {
    /// It is read in the type `to` (the value of the `if` or the `match` it answers for, at `at`),
    /// then goes on to the next.
    Then(Ty, Box<Expr>, Box<Sink>),
    /// It is put in the variable, in the type `to`.
    Assign(String, Ty),
    /// It is what the function answers.
    Return,
}

/// The plan of one function's body.
pub struct Fx<'g, 'a> {
    pub g: &'g Gen<'a>,
    f: usize,
    /// The statements written so far, in the block being written.
    out: Vec<Node>,
    /// The names in scope: the source's name, its value, and whether the code reads it.
    scope: Vec<(String, Val, Rc<Cell<bool>>)>,
    locals: Locals,
}

impl<'g, 'a> Fx<'g, 'a> {
    pub fn new(g: &'g Gen<'a>, f: usize) -> Fx<'g, 'a> {
        Fx { g, f, out: Vec::new(), scope: Vec::new(), locals: Locals::default() }
    }

    fn t(&self) -> Target {
        self.g.t
    }

    fn ty(&self, e: &Expr) -> Ty {
        self.g.c.typed.types.get(e.id).cloned().unwrap_or(Ty::Error)
    }

    fn rf(&self, e: &Expr) -> Ref {
        self.g.c.typed.refs.get(e.id).cloned().unwrap_or(Ref::None)
    }

    /// A name for a value of the source.
    fn local(&mut self, base: &str) -> String {
        let t = self.t();
        self.locals.fresh(t, base, &self.g.outer)
    }

    fn temp(&mut self) -> String {
        let t = self.t();
        self.locals.temp(t, &self.g.outer)
    }

    fn emit(&mut self, n: Node) {
        self.out.push(n);
    }

    /// A line comment: the source line a statement does.
    fn comment(&mut self, line: usize) {
        let text = self.g.line_of(line);
        if text.is_empty() {
            return;
        }
        let mark = if self.t() == Target::Python { "#" } else { "//" };
        self.emit(Node::Line(format!("{mark} {line}: {}", ritsu_emit::header::one_line(&text))));
    }

    /// `name = value`, a name written once: `const`, `var` with its type in Go.
    fn bind(&mut self, name: &str, ty: &str, value: Ex) {
        let t = self.t();
        let line = match t {
            Target::TypeScript => format!("const {name}: {ty} = {};", value.print(t)),
            Target::Python => format!("{name}: {ty} = {}", value.print(t)),
            Target::Go => format!("var {name} {ty} = {}", value.print(t)),
        };
        self.emit(Node::Line(line));
    }

    /// A variable with no value yet, given one in the branches after it.
    fn declare(&mut self, name: &str, ty: &str) {
        let t = self.t();
        let line = match t {
            Target::TypeScript => format!("let {name}: {ty};"),
            Target::Python => format!("{name}: {ty}"),
            Target::Go => format!("var {name} {ty}"),
        };
        self.emit(Node::Line(line));
    }

    fn assign_line(&self, name: &str, value: Ex) -> Node {
        let t = self.t();
        Node::Line(match t {
            Target::TypeScript => format!("{name} = {};", value.print(t)),
            _ => format!("{name} = {}", value.print(t)),
        })
    }

    /// A value made a name, where it is read more than once or later code must not evaluate it again.
    fn spill(&mut self, v: Val) -> Val {
        if v.ex.is_simple() {
            return v;
        }
        let n = self.temp();
        let ty = self.g.flight(&v.ty);
        self.bind(&n, &ty, v.ex);
        Val { ex: Ex::atom(n), ty: v.ty, present: v.present }
    }

    /// The statements `f` writes, apart: what it returns, and them.
    fn apart<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> (T, Vec<Node>) {
        let saved = std::mem::take(&mut self.out);
        let r = f(self);
        let written = std::mem::replace(&mut self.out, saved);
        (r, written)
    }

    // ── Values from the left ───────────────────────────────────────────────

    /// The values of expressions, from the left (DESIGN 8): one whose code comes before a later
    /// one's statements is made a name first, so that it is evaluated before them.
    fn values(&mut self, es: &[&Expr]) -> Vec<Val> {
        let mut parts: Vec<(Val, Vec<Node>)> = Vec::new();
        for e in es {
            let (v, stmts) = self.apart(|s| s.expr(e));
            parts.push((v, stmts));
        }
        let later: Vec<bool> = (0..parts.len()).map(|i| parts[i + 1..].iter().any(|(_, s)| !s.is_empty())).collect();
        let mut out = Vec::new();
        for (i, (v, stmts)) in parts.into_iter().enumerate() {
            self.out.extend(stmts);
            out.push(if later[i] { self.spill(v) } else { v });
        }
        out
    }

    /// The values of the arguments of a call or of the fields of a record, from the left, as the
    /// interpreter works them out (`args` or `record`, then `coerce_at` from the left, DESIGN 8).
    /// `to` gives, for each, the place it goes to in the callee's order and the type it is read in;
    /// `checked` says checks of what the places declare come after them. Where the code places them
    /// otherwise than written, where checks come after them, or where reading one in its type could
    /// fail before a later one is worked out, each is made a name first, in the order written. The
    /// values, and whether each must be made a name again once it is read in its type (so that the
    /// readings too come in the order written, and before the checks).
    fn arg_values(&mut self, es: &[&Expr], to: &[Option<(usize, Ty)>], checked: bool) -> (Vec<Val>, bool) {
        let vals = self.values(es);
        let placed: Vec<usize> = to.iter().flatten().map(|(k, _)| *k).collect();
        let keep = checked || placed.windows(2).any(|w| w[0] > w[1]);
        let early = (0..vals.len()).any(|i| {
            let fails = to[i].as_ref().is_some_and(|(_, t)| !self.adapt_is_identity(&vals[i].ty, t) && self.g.open(es[i].id, |w| matches!(w, What::Overflow)));
            fails && vals[i + 1..].iter().any(|v| !v.ex.is_simple())
        });
        if keep || early {
            let vals = vals.into_iter().map(|v| self.spill(v)).collect();
            return (vals, keep);
        }
        (vals, keep)
    }

    /// A value as the code writes it at a boundary, made a name of the type `ty`, unless it is one.
    fn spill_as(&mut self, ex: Ex, ty: &str) -> Ex {
        if ex.is_simple() {
            return ex;
        }
        let n = self.temp();
        self.bind(&n, ty, ex);
        Ex::atom(n)
    }

    // ── Conversions (interp.rs's `coerce`) ─────────────────────────────────

    /// The value read in the type `to` (`interp.rs`'s `coerce`, at `at`): a number in another unit
    /// of its dimension, the same through optionals and lists, and the representation `to` is held
    /// in.
    fn adapt(&mut self, v: Val, to: &Ty, at: &Expr) -> Val {
        let from = v.ty.clone();
        if matches!(from, Ty::None) || matches!(v.ex, Ex::Atom(ref s) if s == "null" || s == "None" || s == "nil") && matches!(to, Ty::Opt(_)) {
            return Val::new(self.g.null(), to.clone());
        }
        if matches!(from, Ty::Empty) {
            return Val::new(self.empty_list(to), to.clone());
        }
        match (&from, to) {
            (Ty::Opt(f), Ty::Opt(t)) => {
                if self.adapt_is_identity(f, t) {
                    return Val::new(v.ex, to.clone());
                }
                let v = self.spill(v);
                let inner = match self.t() {
                    Target::Go => Ex::Deref(Box::new(v.ex.clone())),
                    _ => v.ex.clone(),
                };
                let x = self.adapt(Val::new(inner, (**f).clone()), t, at);
                self.opt_map(v.ex, x.ex, to)
            }
            (Ty::Opt(f), t) => {
                // a value that may be absent, read where one is there: only a branch that has split it
                let inner = match self.t() {
                    Target::Go => Ex::Deref(Box::new(v.ex)),
                    _ => v.ex,
                };
                self.adapt(Val::new(inner, (**f).clone()), t, at)
            }
            (f, Ty::Opt(t)) => {
                let x = self.adapt(Val::new(v.ex, f.clone()), t, at);
                match self.t() {
                    Target::Go => Val::new(self.g.kcall("Some", vec![x.ex]), to.clone()),
                    _ => Val { ex: x.ex, ty: to.clone(), present: true },
                }
            }
            (Ty::Num(a), Ty::Num(b)) => {
                let konst = a.konst;
                Val::new(self.convert(v.ex, a, b, at, konst), to.clone())
            }
            (Ty::List(a), Ty::List(b)) => {
                if self.adapt_is_identity(a, b) {
                    return Val::new(v.ex, to.clone());
                }
                let v = self.spill(v);
                let el = self.temp();
                let x = self.adapt(Val::new(Ex::atom(el.clone()), (**a).clone()), b, at);
                Val::new(self.list_map(v.ex, &el, &self.g.flight(a), x.ex, &self.g.flight(b)), to.clone())
            }
            _ => Val::new(v.ex, to.clone()),
        }
    }

    /// Whether reading a value of `a` in `b` changes nothing of it.
    fn adapt_is_identity(&self, a: &Ty, b: &Ty) -> bool {
        match (a, b) {
            (Ty::Num(x), Ty::Num(y)) => rep(x) == rep(y) && factor_between(x, y) == Rat::int(1) && x.konst.is_none(),
            (Ty::Opt(x), Ty::Opt(y)) | (Ty::List(x), Ty::List(y)) => self.adapt_is_identity(x, y),
            (Ty::Opt(_), _) | (_, Ty::Opt(_)) => false,
            _ => true,
        }
    }

    /// An empty list of the type `to`.
    fn empty_list(&self, to: &Ty) -> Ex {
        let inner = match to {
            Ty::List(x) => self.g.flight(x),
            _ => "any".into(),
        };
        match self.t() {
            Target::Go => Ex::List(format!("[]{inner}"), vec![]),
            _ => Ex::List(String::new(), vec![]),
        }
    }

    /// `x` of a value that may be absent, mapped where it is there.
    fn opt_map(&mut self, v: Ex, mapped: Ex, to: &Ty) -> Val {
        match self.t() {
            Target::TypeScript => Val::new(Ex::Cond(Box::new(Ex::bin(Op::Eq, v, self.g.null())), Box::new(self.g.null()), Box::new(mapped)), to.clone()),
            Target::Python => Val::new(Ex::Cond(Box::new(Ex::atom(format!("{} is None", v.print(Target::Python)))), Box::new(self.g.null()), Box::new(mapped)), to.clone()),
            Target::Go => {
                let n = self.temp();
                let ty = self.g.flight(to);
                self.declare(&n, &ty);
                let then = vec![self.assign_line(&n, self.g.kcall("Some", vec![mapped]))];
                self.emit(Node::If(vec![(Ex::bin(Op::Ne, v, self.g.null()), then)], Vec::new()));
                Val::new(Ex::atom(n), to.clone())
            }
        }
    }

    /// A list with each element `el` mapped.
    fn list_map(&mut self, xs: Ex, el: &str, from: &str, mapped: Ex, to: &str) -> Ex {
        let t = self.t();
        match t {
            Target::TypeScript => Ex::call(format!("{}.map", xs.print(t)), vec![Ex::atom(format!("({el}: {from}): {to} => {}", mapped.print(t)))]),
            Target::Python => Ex::atom(format!("[{} for {el} in {}]", mapped.print(t), xs.print(t))),
            Target::Go => {
                let n = self.temp();
                self.bind(&n, &format!("[]{to}"), Ex::call("make", vec![Ex::atom(format!("[]{to}")), Ex::atom("0"), Ex::call("len", vec![xs.clone()])]));
                let push = Node::Line(format!("{n} = {}", Ex::call("append", vec![Ex::atom(n.clone()), mapped]).print(t)));
                self.emit(Node::Block(format!("for _, {el} := range {}", xs.print(t)), vec![push], Vec::new()));
                Ex::atom(n)
            }
        }
    }

    /// A number of `a` read in the unit and the representation of `b` (`interp.rs`'s `convert`),
    /// held to int64 at `at` where the analysis does not show it fits. A literal is read when the code
    /// is written.
    fn convert(&mut self, x: Ex, a: &NumTy, b: &NumTy, at: &Expr, konst: Option<Rat>) -> Ex {
        let g = self.g;
        if let (Some(k), ExprKind::Num(_)) = (konst, &at.kind)
            && let Ok(v) = crate::interp::convert(k, &a.unit, &b.unit)
            && crate::num::fits(v)
        {
            return match rep(b) {
                Rep::Int if v.is_int() => g.int(v.num),
                _ => g.frac(v),
            };
        }
        let f = factor_between(a, b);
        let check = g.overflow_at(at);
        let (ra, rb) = (rep(a), rep(b));
        if f == Rat::int(1) {
            return self.rep_to(x, ra, rb);
        }
        if ra == Rep::Int && f.is_int() {
            let p = self.int_op(Op::Mul, x, g.int(f.num), check.as_ref());
            return self.rep_to(p, Rep::Int, rb);
        }
        let q = self.rep_to(x, ra, Rep::Q);
        let p = self.q_op(Op::Mul, q, g.frac(f), check.as_ref());
        self.rep_to(p, Rep::Q, rb)
    }

    fn rep_to(&self, x: Ex, from: Rep, to: Rep) -> Ex {
        match (from, to) {
            (Rep::Int, Rep::Q) => self.g.to_q_ex(x),
            (Rep::Q, Rep::Int) => self.g.to_int_ex(x),
            _ => x,
        }
    }

    /// A whole-number operation, held to int64 where `check` says.
    fn int_op(&self, op: Op, a: Ex, b: Ex, check: Option<&At>) -> Ex {
        let g = self.g;
        match (check, self.t()) {
            (None, _) => Ex::bin(op, a, b),
            (Some(at), Target::Go) => {
                let f = match op {
                    Op::Add => "Add",
                    Op::Sub => "Sub",
                    _ => "Mul",
                };
                let mut args = vec![a, b];
                args.extend(g.at_args(at));
                g.kcall(f, args)
            }
            (Some(at), _) => {
                let mut args = vec![Ex::bin(op, a, b)];
                args.extend(g.at_args(at));
                g.kcall(&g.helper("heldInt", "held_int", ""), args)
            }
        }
    }

    /// An operation on fractions, held to int64 where `check` says.
    fn q_op(&self, op: Op, a: Ex, b: Ex, check: Option<&At>) -> Ex {
        let g = self.g;
        let raw = match self.t() {
            Target::Python => Ex::bin(op, a, b),
            _ => {
                let f = match (self.t(), op) {
                    (Target::TypeScript, Op::Add) => "add",
                    (Target::TypeScript, Op::Sub) => "sub",
                    (Target::TypeScript, Op::Mul) => "mul",
                    (Target::TypeScript, _) => "div",
                    (_, Op::Add) => "QAdd",
                    (_, Op::Sub) => "QSub",
                    (_, Op::Mul) => "QMul",
                    _ => "QDiv",
                };
                g.kcall(f, vec![a, b])
            }
        };
        match check {
            None => raw,
            Some(at) => {
                let mut args = vec![raw];
                args.extend(g.at_args(at));
                g.kcall(&g.helper("held", "held", "Held"), args)
            }
        }
    }

    // ── Expressions ────────────────────────────────────────────────────────

    /// The value of an expression, in the unit and the representation of its type.
    pub fn expr(&mut self, e: &Expr) -> Val {
        let ty = self.ty(e);
        match &e.kind {
            ExprKind::Num(n) => {
                let v = if n.unit == "%" { n.value.div(Rat::int(100)) } else { n.value };
                let ex = match ty.num().map(rep) {
                    Some(Rep::Int) if v.is_int() => self.g.int(v.num),
                    _ => self.g.frac(v),
                };
                if !crate::num::fits(v)
                    && let Some(at) = self.g.overflow_at(e)
                {
                    // a literal past int64: the interpreter fails when it reads it
                    let mut args = vec![self.g.string("overflow"), self.g.module_const(), Ex::atom(at.line.to_string()), self.g.string(&at.message)];
                    let fail = self.g.kcall(&self.g.helper("fail", "fail", "Fail"), std::mem::take(&mut args));
                    self.emit(Node::Line(self.g.stmt(fail)));
                }
                Val::new(ex, ty)
            }
            ExprKind::Date(d) => Val::new(self.g.date(*d), ty),
            ExprKind::Str(s) => Val::new(self.g.string(s), ty),
            ExprKind::Bool(b) => Val::new(self.g.boolean(*b), ty),
            ExprKind::None => Val::new(self.g.null(), ty),
            ExprKind::Var(w) => match self.rf(e) {
                Ref::EnumValue(id, v) => Val::new(self.g.member(&id, &v), ty),
                _ => self.lookup(w).unwrap_or_else(|| Val::new(self.g.null(), ty)),
            },
            ExprKind::Field(base, _, _) => match self.rf(e) {
                Ref::EnumValue(id, v) => Val::new(self.g.member(&id, &v), ty),
                Ref::Field(name) => {
                    let b = self.expr(base);
                    self.field(b, &name, &ty)
                }
                _ => Val::new(self.g.null(), ty),
            },
            ExprKind::Call(callee, args) => self.call(e, callee, args),
            ExprKind::Record(_, inits) => self.record(e, inits),
            ExprKind::List(xs) => {
                let elem = match &ty {
                    Ty::List(t) => (**t).clone(),
                    _ => Ty::Error,
                };
                let mut parts: Vec<(Val, Vec<Node>)> = Vec::new();
                for x in xs {
                    let (v, stmts) = self.apart(|s| {
                        let v = s.expr(x);
                        s.adapt(v, &elem, x)
                    });
                    parts.push((v, stmts));
                }
                let later: Vec<bool> = (0..parts.len()).map(|i| parts[i + 1..].iter().any(|(_, s)| !s.is_empty())).collect();
                let mut items = Vec::new();
                for (i, (v, stmts)) in parts.into_iter().enumerate() {
                    self.out.extend(stmts);
                    items.push(if later[i] { self.spill(v).ex } else { v.ex });
                }
                let lt = match self.t() {
                    Target::Go => format!("[]{}", self.g.flight(&elem)),
                    _ => String::new(),
                };
                Val::new(Ex::List(lt, items), ty)
            }
            ExprKind::Neg(x) => {
                let v = self.expr(x);
                let check = self.g.overflow_at(e);
                let ex = match v.ty.num().map(rep) {
                    Some(Rep::Int) => match (check, self.t()) {
                        (None, _) => Ex::Neg(Box::new(v.ex)),
                        (Some(at), Target::Go) => {
                            let mut args = vec![v.ex];
                            args.extend(self.g.at_args(&at));
                            self.g.kcall("Neg", args)
                        }
                        (Some(at), _) => {
                            let mut args = vec![Ex::Neg(Box::new(v.ex))];
                            args.extend(self.g.at_args(&at));
                            self.g.kcall(&self.g.helper("heldInt", "held_int", ""), args)
                        }
                    },
                    _ => {
                        let raw = match self.t() {
                            Target::Python => Ex::Neg(Box::new(v.ex)),
                            _ => self.g.kcall(&self.g.helper("neg", "", "QNeg"), vec![v.ex]),
                        };
                        match check {
                            None => raw,
                            Some(at) => {
                                let mut args = vec![raw];
                                args.extend(self.g.at_args(&at));
                                self.g.kcall(&self.g.helper("held", "held", "Held"), args)
                            }
                        }
                    }
                };
                // the representation of the type of `-x` (its step may differ from `x`'s)
                let from = v.ty.num().map(rep).unwrap_or(Rep::Int);
                let to = ty.num().map(rep).unwrap_or(from);
                Val::new(self.rep_to(ex, from, to), ty)
            }
            ExprKind::Not(x) => {
                let v = self.expr(x);
                Val::new(Ex::Not(Box::new(v.ex)), ty)
            }
            ExprKind::In(x, _, _) => {
                let v = self.expr(x);
                self.adapt(v, &ty, e)
            }
            ExprKind::Bin(a, op, b) => self.binary(e, a, *op, b),
            ExprKind::If(..) | ExprKind::Match(..) | ExprKind::Block(_) => self.answer_value(e),
            ExprKind::Lambda(..) => Val::new(self.g.null(), ty),
        }
    }

    fn lookup(&self, w: &str) -> Option<Val> {
        let (_, v, used) = self.scope.iter().rev().find(|(n, ..)| n == w)?;
        used.set(true);
        Some(v.clone())
    }

    /// A field of a record, as it is worked on.
    fn field(&mut self, b: Val, name: &str, ty: &Ty) -> Val {
        let Ty::Record(id) = b.ty.present().clone() else { return Val::new(self.g.null(), ty.clone()) };
        match &id {
            RecId::Module { file, name: rec } => {
                let m = self.g.p.module(file);
                let Some(r) = m.decls.records.iter().find(|r| r.name.text == *rec) else { return Val::new(self.g.null(), ty.clone()) };
                let Some(f) = r.fields.iter().find(|f| f.name.text == name) else { return Val::new(self.g.null(), ty.clone()) };
                let raw = b.ex.dot(field_name(self.t(), f.name.ascii()));
                self.from_boundary(raw, &f.slot.ty, ty)
            }
            RecId::Outputs(file) => {
                let Some(facts) = self.g.p.rules.get(file).cloned() else { return Val::new(self.g.null(), ty.clone()) };
                let Some(col) = facts.outputs.iter().find(|c| c.name == name) else { return Val::new(self.g.null(), ty.clone()) };
                let raw = if facts.outputs.len() == 1 {
                    b.ex
                } else {
                    let call = self.g.call_of(&facts);
                    let alias = call.outputs.iter().find(|p| p.name == name).map(|p| p.alias.clone()).unwrap_or_else(|| col.alias.clone());
                    b.ex.dot(alias)
                };
                self.rule_out(raw, &col.ty, ty)
            }
            RecId::Element(file, seq) => {
                let Some(facts) = self.g.p.rules.get(file).cloned() else { return Val::new(self.g.null(), ty.clone()) };
                let Some((_, _, cols)) = facts.elements.iter().find(|(n, _, _)| n == seq) else { return Val::new(self.g.null(), ty.clone()) };
                let Some(col) = cols.iter().find(|c| c.name == name) else { return Val::new(self.g.null(), ty.clone()) };
                let alias = match self.t() {
                    Target::Go => ritsu_emit::ident::pascal(&col.alias),
                    _ => col.alias.clone(),
                };
                self.rule_out(b.ex.dot(alias), &col.ty, ty)
            }
            RecId::Running(item, w) => {
                let (fname, slot_ty) = match name {
                    "item" => ("item", (**item).clone()),
                    n => (n, (**w).clone()),
                };
                let f = match self.t() {
                    Target::Go => {
                        let mut cs = fname.chars();
                        cs.next().map(|c| c.to_ascii_uppercase().to_string() + cs.as_str()).unwrap_or_default()
                    }
                    _ => fname.to_string(),
                };
                self.from_boundary(b.ex.dot(f), &slot_ty, ty)
            }
        }
    }

    /// A value as it travels (in a record, given to a function, answered by one), as it is worked
    /// on: an amount as a plain number, a rate as the fraction it is.
    pub fn from_boundary(&mut self, x: Ex, slot: &Ty, ty: &Ty) -> Val {
        let t = self.t();
        match slot {
            Ty::Num(n) if n.is_rate() => {
                let step = self.g.frac(rate_step(n));
                let q = self.g.kcall(&self.g.helper("rate", "rate", "Rate"), vec![x, step]);
                let r = rep(ty.num().unwrap_or(n));
                Val::new(self.rep_to(q, Rep::Q, r), ty.clone())
            }
            Ty::Num(n) => {
                let ex = if t == Target::Go && n.unit.type_name().is_some() { Ex::call("int64", vec![x]) } else { x };
                let r = rep(ty.num().unwrap_or(n));
                Val::new(self.rep_to(ex, Rep::Int, r), ty.clone())
            }
            Ty::Opt(s) | Ty::List(s) if !self.g.same_in(slot) => {
                let inner_ty = match ty {
                    Ty::Opt(x) | Ty::List(x) => (**x).clone(),
                    x => x.clone(),
                };
                let is_list = matches!(slot, Ty::List(_));
                let v = self.spill(Val::new(x, slot.clone()));
                if is_list {
                    if t == Target::Go && matches!(**s, Ty::Num(ref n) if !n.is_rate()) {
                        return Val::new(Ex::Call(Box::new(Ex::atom(format!("{}.Ints[int64]", self.g.imp(Import::Kaname)))), vec![v.ex]), ty.clone());
                    }
                    let el = self.temp();
                    let m = self.from_boundary(Ex::atom(el.clone()), s, &inner_ty);
                    let m = match (t, &**s) {
                        // the package's type of a unit, as a plain int (a list of it is no list of int)
                        (Target::Python, Ty::Num(n)) if !n.is_rate() && n.unit.type_name().is_some() => Ex::call("int", vec![m.ex]),
                        _ => m.ex,
                    };
                    let from = self.g.boundary(s);
                    let to = self.g.flight(&inner_ty);
                    Val::new(self.list_map(v.ex, &el, &from, m, &to), ty.clone())
                } else {
                    if t == Target::Go && matches!(**s, Ty::Num(ref n) if !n.is_rate()) {
                        return Val::new(Ex::Call(Box::new(Ex::atom(format!("{}.Int[int64]", self.g.imp(Import::Kaname)))), vec![v.ex]), ty.clone());
                    }
                    let inner = if t == Target::Go { Ex::Deref(Box::new(v.ex.clone())) } else { v.ex.clone() };
                    let m = self.from_boundary(inner, s, &inner_ty);
                    let mut r = self.opt_map(v.ex, m.ex, ty);
                    r.ty = ty.clone();
                    r
                }
            }
            _ => Val::new(x, ty.clone()),
        }
    }

    /// A value as it is worked on, as it travels in a place of `slot`.
    pub fn to_boundary(&mut self, v: Val, slot: &Ty) -> Ex {
        let t = self.t();
        match slot {
            Ty::Num(n) if n.is_rate() => {
                let q = self.rep_to(v.ex, v.ty.num().map(rep).unwrap_or(Rep::Q), Rep::Q);
                self.g.kcall(&self.g.helper("steps", "steps", "Steps"), vec![q, self.g.frac(rate_step(n))])
            }
            Ty::Num(n) => {
                let x = self.rep_to(v.ex, v.ty.num().map(rep).unwrap_or(Rep::Int), Rep::Int);
                match n.unit.type_name() {
                    Some(_) => {
                        let ut = self.g.unit_type(&n.unit);
                        match t {
                            Target::TypeScript => Ex::Cast(Box::new(x), ut),
                            _ => Ex::Cast(Box::new(x), ut),
                        }
                    }
                    None => x,
                }
            }
            Ty::Opt(_) if self.g.absent(&v.ex) => v.ex,
            Ty::Opt(s) | Ty::List(s) if !self.g.same_out(slot) => {
                let inner_ty = match &v.ty {
                    Ty::Opt(x) | Ty::List(x) => (**x).clone(),
                    x => x.clone(),
                };
                // a value that is there travels as the value it is
                if v.present && matches!(slot, Ty::Opt(_)) {
                    return self.to_boundary(Val::new(v.ex, inner_ty), s);
                }
                // TypeScript casts an amount that may be absent to its unit's type; a list of amounts
                // is no list of the type (neither array type is the other's), so its elements are cast
                if t == Target::TypeScript && matches!(slot, Ty::Opt(_)) && matches!(**s, Ty::Num(ref n) if !n.is_rate()) {
                    return Ex::Cast(Box::new(v.ex), self.g.boundary(slot));
                }
                let is_list = matches!(slot, Ty::List(_));
                let v = self.spill(v);
                if t == Target::Go && matches!(**s, Ty::Num(ref n) if !n.is_rate() && n.unit.type_name().is_some()) {
                    let n = num_of(s).cloned().unwrap_or_else(NumTy::number);
                    let ut = self.g.unit_type(&n.unit);
                    let f = if is_list { "Ints" } else { "Int" };
                    return Ex::Call(Box::new(Ex::atom(format!("{}.{f}[{ut}]", self.g.imp(Import::Kaname)))), vec![v.ex]);
                }
                if is_list {
                    let el = self.temp();
                    let inner = self.to_boundary(Val::new(Ex::atom(el.clone()), inner_ty.clone()), s);
                    let from = self.g.flight(&inner_ty);
                    let to = self.g.boundary(s);
                    self.list_map(v.ex, &el, &from, inner, &to)
                } else {
                    let x = if t == Target::Go { Ex::Deref(Box::new(v.ex.clone())) } else { v.ex.clone() };
                    let inner = self.to_boundary(Val::new(x, inner_ty), s);
                    self.opt_map(v.ex, inner, slot).ex
                }
            }
            _ => v.ex,
        }
    }

    /// A value a rule's code answers (an output, a field of an element), as it is worked on.
    fn rule_out(&mut self, x: Ex, col: &ColumnType, ty: &Ty) -> Val {
        let t = self.t();
        match col {
            ColumnType::Opt(inner) => {
                let inner_ty = ty.present().clone();
                if matches!(**inner, ColumnType::Num { unit: Some(ref u), .. } if u.dim != Dim::Rate) && t != Target::Go {
                    return Val::new(x, ty.clone());
                }
                let v = self.spill(Val::new(x, ty.clone()));
                let deref = if t == Target::Go { Ex::Deref(Box::new(v.ex.clone())) } else { v.ex.clone() };
                if matches!(**inner, ColumnType::Bool | ColumnType::Str | ColumnType::Enum { .. }) {
                    return Val::new(v.ex, ty.clone());
                }
                let m = self.rule_out(deref, inner, &inner_ty);
                let target = if matches!(ty, Ty::Opt(_)) { ty.clone() } else { Ty::Opt(Box::new(ty.clone())) };
                self.opt_map(v.ex, m.ex, &target)
            }
            ColumnType::Date => Val::new(
                match t {
                    Target::TypeScript => self.g.kcall("dateOf", vec![Ex::call("Number", vec![x])]),
                    Target::Python => self.g.kcall("date_of", vec![x]),
                    Target::Go => self.g.kcall("DateOf", vec![x]),
                },
                ty.clone(),
            ),
            ColumnType::Num { unit: Some(u), .. } if u.dim == Dim::Rate => {
                let step = u.step.unwrap_or(Rat::new(1, 100));
                let k = if t == Target::Go { Ex::call("int64", vec![x]) } else { x };
                let q = self.g.kcall(&self.g.helper("rate", "rate", "Rate"), vec![k, self.g.frac(step)]);
                let r = ty.num().map(rep).unwrap_or(Rep::Q);
                Val::new(self.rep_to(q, Rep::Q, r), ty.clone())
            }
            ColumnType::Num { .. } => {
                let k = if t == Target::Go { Ex::call("int64", vec![x]) } else { x };
                let r = ty.num().map(rep).unwrap_or(Rep::Int);
                Val::new(self.rep_to(k, Rep::Int, r), ty.clone())
            }
            _ => Val::new(x, ty.clone()),
        }
    }

    /// A value as it is worked on, as a rule's code takes it in a column of `col`.
    fn rule_in(&mut self, v: Val, file: &Path, col: &ColumnType) -> Ex {
        let t = self.t();
        match col {
            ColumnType::Opt(inner) => {
                if matches!(v.ex, Ex::Atom(ref s) if s == "null" || s == "None" || s == "nil") {
                    return self.g.null();
                }
                let present = v.ty.present().clone();
                if !matches!(v.ty, Ty::Opt(_)) {
                    // a value that is there, where one may be absent
                    let x = self.rule_in(Val::new(v.ex, present), file, inner);
                    return if t == Target::Go { self.g.kcall("Some", vec![x]) } else { x };
                }
                let v = self.spill(v);
                let deref = if t == Target::Go { Ex::Deref(Box::new(v.ex.clone())) } else { v.ex.clone() };
                let x = self.rule_in(Val::new(deref, present), file, inner);
                let to_ty = self.g.rule_type(file, col);
                match t {
                    Target::TypeScript => Ex::Cond(Box::new(Ex::bin(Op::Eq, v.ex.clone(), self.g.null())), Box::new(self.g.null()), Box::new(x)),
                    Target::Python => Ex::Cond(Box::new(Ex::atom(format!("{} is None", v.ex.print(t)))), Box::new(self.g.null()), Box::new(x)),
                    Target::Go => {
                        let n = self.temp();
                        self.declare(&n, &to_ty);
                        let then = vec![self.assign_line(&n, self.g.kcall("Some", vec![x]))];
                        self.emit(Node::If(vec![(Ex::bin(Op::Ne, v.ex, self.g.null()), then)], Vec::new()));
                        Ex::atom(n)
                    }
                }
            }
            ColumnType::Date => match t {
                Target::TypeScript => Ex::call("BigInt", vec![self.g.kcall("dayOf", vec![v.ex])]),
                Target::Python => self.g.kcall("day_of", vec![v.ex]),
                Target::Go => Ex::Call(Box::new(v.ex.dot("Days")), vec![]),
            },
            ColumnType::Num { unit: Some(u), .. } if u.dim == Dim::Rate => {
                let step = u.step.unwrap_or(Rat::new(1, 100));
                let q = self.rep_to(v.ex, v.ty.num().map(rep).unwrap_or(Rep::Q), Rep::Q);
                let k = self.g.kcall(&self.g.helper("steps", "steps", "Steps"), vec![q, self.g.frac(step)]);
                let rate = format!("{}.Rate", self.g.imp(Import::Rule(file.to_path_buf())));
                Ex::Cast(Box::new(k), rate)
            }
            ColumnType::Num { unit: Some(u), .. } => {
                let x = self.rep_to(v.ex, v.ty.num().map(rep).unwrap_or(Rep::Int), Rep::Int);
                if u.type_name().is_some() { Ex::Cast(Box::new(x), self.g.unit_type(u)) } else { x }
            }
            ColumnType::Num { unit: None, .. } => self.rep_to(v.ex, v.ty.num().map(rep).unwrap_or(Rep::Int), Rep::Int),
            _ => v.ex,
        }
    }

    // ── Operators ──────────────────────────────────────────────────────────

    fn binary(&mut self, e: &Expr, a: &Expr, op: BinOp, b: &Expr) -> Val {
        let ty = self.ty(e);
        if matches!(op, BinOp::And | BinOp::Or) {
            return self.lazy(e, a, op, b);
        }
        let (ta, tb) = (self.ty(a), self.ty(b));
        let vs = self.values(&[a, b]);
        let (x, y) = (vs[0].clone(), vs[1].clone());
        if op.is_comparison() {
            return Val::new(self.compare(a, b, op, x, y, &ta, &tb), ty);
        }
        if matches!(ta, Ty::Date) && matches!(tb, Ty::Date) {
            let ex = match self.t() {
                Target::TypeScript => self.g.kcall("between", vec![x.ex, y.ex]),
                Target::Python => self.g.kcall("between", vec![x.ex, y.ex]),
                Target::Go => self.g.kcall("Between", vec![x.ex, y.ex]),
            };
            let r = ty.num().map(rep).unwrap_or(Rep::Int);
            return Val::new(self.rep_to(ex, Rep::Int, r), ty);
        }
        let (Some(na), Some(nb), Some(ne)) = (ta.num().cloned(), tb.num().cloned(), ty.num().cloned()) else { return Val::new(self.g.null(), ty) };
        let check = self.g.overflow_at(e);
        let ex = match op {
            BinOp::Add | BinOp::Sub => {
                let p = self.convert(x.ex, &na, &NumTy { konst: None, ..ne.clone() }, a, na.konst);
                let q = self.convert(y.ex, &nb, &NumTy { konst: None, ..ne.clone() }, b, nb.konst);
                let o = if op == BinOp::Add { Op::Add } else { Op::Sub };
                match rep(&ne) {
                    Rep::Int => self.int_op(o, p, q, check.as_ref()),
                    Rep::Q => self.q_op(o, p, q, check.as_ref()),
                }
            }
            BinOp::Mul => {
                if rep(&na) == Rep::Int && rep(&nb) == Rep::Int && rep(&ne) == Rep::Int {
                    self.int_op(Op::Mul, x.ex, y.ex, check.as_ref())
                } else {
                    let p = self.rep_to(x.ex, rep(&na), Rep::Q);
                    let q = self.rep_to(y.ex, rep(&nb), Rep::Q);
                    let r = self.q_op(Op::Mul, p, q, check.as_ref());
                    self.rep_to(r, Rep::Q, rep(&ne))
                }
            }
            _ => {
                // two amounts are read in one unit before one is divided by the other
                let (p, q) = if na.has_unit() && nb.has_unit() {
                    let t = crate::interp::common(&ta, &tb).clone();
                    let nt = t.num().cloned().unwrap_or_else(NumTy::number);
                    let nt = NumTy { konst: None, ..nt };
                    let p = self.convert(x.ex, &na, &NumTy { step: na.step, ..nt.clone() }, a, na.konst);
                    let q = self.convert(y.ex, &nb, &NumTy { step: nb.step, ..nt }, b, nb.konst);
                    (self.rep_to(p, rep(&na), Rep::Q), self.rep_to(q, rep(&nb), Rep::Q))
                } else {
                    (self.rep_to(x.ex, rep(&na), Rep::Q), self.rep_to(y.ex, rep(&nb), Rep::Q))
                };
                let quotient = match self.g.zero_at(e) {
                    Some(at) => {
                        let mut args = vec![p, q];
                        args.extend(self.g.at_args(&at));
                        self.g.kcall(&self.g.helper("over", "over", "Over"), args)
                    }
                    None => match self.t() {
                        Target::Python => Ex::bin(Op::Div, p, q),
                        _ => self.g.kcall(&self.g.helper("div", "", "QDiv"), vec![p, q]),
                    },
                };
                let held = match check {
                    Some(at) => {
                        let mut args = vec![quotient];
                        args.extend(self.g.at_args(&at));
                        self.g.kcall(&self.g.helper("held", "held", "Held"), args)
                    }
                    None => quotient,
                };
                self.rep_to(held, Rep::Q, rep(&ne))
            }
        };
        Val::new(ex, ty)
    }

    /// `and`, `or`: the right is evaluated only when the left does not decide (DESIGN 8).
    fn lazy(&mut self, e: &Expr, a: &Expr, op: BinOp, b: &Expr) -> Val {
        let ty = self.ty(e);
        let left = self.expr(a);
        let (right, stmts) = self.apart(|s| s.expr(b));
        let o = if op == BinOp::And { Op::And } else { Op::Or };
        if stmts.is_empty() {
            return Val::new(Ex::bin(o, left.ex, right.ex), ty);
        }
        let n = self.temp();
        let bt = self.g.flight(&Ty::Bool);
        if self.t() == Target::Python {
            self.emit(Node::Line(format!("{n}: {bt} = {}", left.ex.print(self.t()))));
        } else if self.t() == Target::TypeScript {
            self.emit(Node::Line(format!("let {n}: {bt} = {};", left.ex.print(self.t()))));
        } else {
            self.emit(Node::Line(format!("var {n} {bt} = {}", left.ex.print(self.t()))));
        }
        let cond = if op == BinOp::And { Ex::atom(n.clone()) } else { Ex::Not(Box::new(Ex::atom(n.clone()))) };
        let mut body = stmts;
        body.push(self.assign_line(&n, right.ex));
        self.emit(Node::If(vec![(cond, body)], Vec::new()));
        Val::new(Ex::atom(n), ty)
    }

    #[allow(clippy::too_many_arguments)]
    fn compare(&mut self, a: &Expr, b: &Expr, op: BinOp, x: Val, y: Val, ta: &Ty, tb: &Ty) -> Ex {
        let o = match op {
            BinOp::Eq => Op::Eq,
            BinOp::Ne => Op::Ne,
            BinOp::Lt => Op::Lt,
            BinOp::Le => Op::Le,
            BinOp::Gt => Op::Gt,
            _ => Op::Ge,
        };
        let t = self.t();
        match (ta.present(), tb.present()) {
            (Ty::Num(na), Ty::Num(nb)) => {
                let common = crate::interp::common(ta, tb).clone();
                let nt = common.num().cloned().unwrap_or_else(NumTy::number);
                let target = NumTy { konst: None, ..nt };
                // both in the unit they are compared in: whole numbers when both are, else fractions
                let both_int = rep(na) == Rep::Int && rep(nb) == Rep::Int && factor_between(na, &target).is_int() && factor_between(nb, &target).is_int();
                let r = if both_int { Rep::Int } else { Rep::Q };
                let p = self.convert(x.ex, na, &NumTy { step: if both_int { Some(Rat::int(1)) } else { None }, ..target.clone() }, a, na.konst);
                let q = self.convert(y.ex, nb, &NumTy { step: if both_int { Some(Rat::int(1)) } else { None }, ..target.clone() }, b, nb.konst);
                match (r, t) {
                    (Rep::Int, _) | (Rep::Q, Target::Python) => Ex::bin(o, p, q),
                    (Rep::Q, Target::TypeScript) => Ex::bin(o, self.g.kcall("cmp", vec![p, q]), Ex::atom("0")),
                    (Rep::Q, Target::Go) => Ex::bin(o, self.g.kcall("Cmp", vec![p, q]), Ex::atom("0")),
                }
            }
            (Ty::Date, Ty::Date) => match (t, o) {
                (Target::Go, Op::Eq | Op::Ne) => Ex::bin(o, x.ex, y.ex),
                (Target::Go, _) => Ex::bin(o, self.g.kcall("Compare", vec![x.ex, y.ex]), Ex::atom("0")),
                _ => Ex::bin(o, x.ex, y.ex),
            },
            (Ty::Enum(id), Ty::Enum(_)) if !matches!(o, Op::Eq | Op::Ne) => {
                let rank = self.g.rank_of(id);
                Ex::bin(o, Ex::call(rank.clone(), vec![x.ex]), Ex::call(rank, vec![y.ex]))
            }
            _ => Ex::bin(o, x.ex, y.ex),
        }
    }

    // ── Blocks, `if`, `match` ──────────────────────────────────────────────

    /// The `let` lines of a block, then its answer, where `sink` says.
    fn block(&mut self, b: &Block, sink: &Sink) -> Option<Val> {
        let mark = self.scope.len();
        let mut declared: Vec<(usize, String, Rc<Cell<bool>>)> = Vec::new();
        for l in &b.lets {
            self.comment(l.span.line);
            let v = self.expr(&l.value);
            let bound = self.g.c.typed.bound.get(&l.value.id).cloned().unwrap_or_else(|| self.ty(&l.value));
            let v = self.adapt(v, &bound, &l.value);
            let name = self.local(&l.name);
            let ty = self.g.flight(&bound);
            self.bind(&name, &ty, v.ex);
            let used = Rc::new(Cell::new(false));
            declared.push((self.out.len(), name.clone(), used.clone()));
            self.scope.push((l.name.clone(), Val::new(Ex::atom(name), bound), used));
        }
        let out = self.answer(&b.answer, sink);
        // Go asks a name that is never read to be read once
        if self.t() == Target::Go {
            for (at, name, used) in declared.into_iter().rev() {
                if !used.get() && at <= self.out.len() {
                    self.out.insert(at, Node::Line(format!("_ = {name}")));
                }
            }
        }
        self.scope.truncate(mark);
        out
    }

    /// The value of an `if`, a `match` or a block of `let`, as an expression.
    fn answer_value(&mut self, e: &Expr) -> Val {
        let ty = self.ty(e);
        // a one-line `if` whose branches write nothing is the target's conditional expression
        if let ExprKind::If(c, a, b) = &e.kind
            && self.t() != Target::Go
            && a.lets.is_empty()
            && b.lets.is_empty()
            && !matches!(a.answer.kind, ExprKind::If(..) | ExprKind::Match(..) | ExprKind::Block(_))
            && !matches!(b.answer.kind, ExprKind::If(..) | ExprKind::Match(..) | ExprKind::Block(_))
        {
            let cond = self.expr(c);
            let (x, sx) = self.apart(|s| {
                let v = s.expr(&a.answer);
                s.adapt(v, &ty, &a.answer)
            });
            let (y, sy) = self.apart(|s| {
                let v = s.expr(&b.answer);
                s.adapt(v, &ty, &b.answer)
            });
            if sx.is_empty() && sy.is_empty() {
                return Val::new(Ex::Cond(Box::new(cond.ex), Box::new(x.ex), Box::new(y.ex)), ty);
            }
            // the branches write statements: as a statement, below
            let n = self.temp();
            let fty = self.g.flight(&ty);
            self.declare(&n, &fty);
            let mut then = sx;
            then.push(self.assign_line(&n, x.ex));
            let mut els = sy;
            els.push(self.assign_line(&n, y.ex));
            self.emit(Node::If(vec![(cond.ex, then)], els));
            return Val::new(Ex::atom(n), ty);
        }
        let n = self.temp();
        let fty = self.g.flight(&ty);
        self.declare(&n, &fty);
        self.answer(e, &Sink::Assign(n.clone(), ty.clone()));
        Val::new(Ex::atom(n), ty)
    }

    /// The answer of a block, where the sink says: an `if`, a `match` or a block takes the sink into
    /// its branches.
    fn answer(&mut self, e: &Expr, sink: &Sink) -> Option<Val> {
        let ty = self.ty(e);
        match &e.kind {
            ExprKind::If(c, a, b) => {
                let cond = self.expr(c);
                let then_sink = Sink::Then(ty.clone(), Box::new(a.answer.clone()), Box::new(sink.clone()));
                let else_sink = Sink::Then(ty.clone(), Box::new(b.answer.clone()), Box::new(sink.clone()));
                let (_, then) = self.apart(|s| s.block(a, &then_sink));
                let (_, els) = self.apart(|s| s.block(b, &else_sink));
                // `else if`: the `if` of the else branch joins this one
                match els.as_slice() {
                    [Node::If(more, last)] if b.lets.is_empty() && matches!(b.answer.kind, ExprKind::If(..)) => {
                        let mut arms = vec![(cond.ex, then)];
                        arms.extend(more.clone());
                        self.emit(Node::If(arms, last.clone()));
                    }
                    _ => self.emit(Node::If(vec![(cond.ex, then)], els)),
                }
                None
            }
            ExprKind::Match(s, arms) => {
                self.matching(e, s, arms, sink);
                None
            }
            ExprKind::Block(b) => {
                let inner = Sink::Then(ty.clone(), Box::new(b.answer.clone()), Box::new(sink.clone()));
                self.block(b, &inner)
            }
            _ => {
                let v = self.expr(e);
                self.sink(v, sink)
            }
        }
    }

    /// A value, where the sink takes it.
    fn sink(&mut self, v: Val, sink: &Sink) -> Option<Val> {
        match sink {
            Sink::Then(to, at, next) => {
                let v = self.adapt(v, to, at);
                self.sink(v, next)
            }
            Sink::Assign(n, to) => {
                let v = self.adapt(v, to, &Expr { id: usize::MAX, span: Span::default(), kind: ExprKind::None });
                let line = self.assign_line(n, v.ex);
                self.emit(line);
                None
            }
            Sink::Return => {
                self.finish(v);
                None
            }
        }
    }

    /// What the function answers: in its result's type, held to what the result declares, as it
    /// travels (`interp.rs`'s `finish`).
    fn finish(&mut self, v: Val) {
        let g = self.g;
        let decl = &g.c.module.fns[self.f];
        let info = &g.c.decls.fns[self.f];
        let v = self.adapt(v, &info.ret.ty, &decl.body.answer);
        let ret_ty = info.ret.ty.clone();
        let mut b = self.to_boundary(v, &ret_ty);
        let place = info.ret.range.is_some() || info.ret.count.is_some();
        if place && g.open(decl.body.answer.id, |w| matches!(w, What::Answer { .. })) {
            b = self.spill_as(b, &g.boundary(&ret_ty));
            if let Some(f) = g.outside_fn(&info.ret, false) {
                let o = Ex::atom(self.temp());
                let said = Said::Answer { fun: info.name.text.clone() };
                let fail = g.fail_outside("range", decl.body.answer.span.line, &said, &o, g.counts(&info.ret, false));
                let stmts = g.if_found(&o, Ex::call(f, vec![b.clone(), g.string(&info.name.text)]), fail);
                for s in stmts {
                    self.emit(s);
                }
            }
        }
        let line = g.ret(b);
        self.emit(Node::Line(line));
    }

    fn matching(&mut self, e: &Expr, s: &Expr, arms: &[Arm], sink: &Sink) {
        let ty = self.ty(e);
        let v = self.expr(s);
        let v = self.spill(v);
        let st = self.ty(s);
        let mut branches: Vec<(Ex, Vec<Node>)> = Vec::new();
        let mut last: Vec<Node> = Vec::new();
        let n = arms.len();
        for (k, arm) in arms.iter().enumerate() {
            let next = Sink::Then(ty.clone(), Box::new(arm.body.answer.clone()), Box::new(sink.clone()));
            let mut bind: Option<String> = None;
            let mut tests: Vec<Ex> = Vec::new();
            for p in &arm.patterns {
                match p {
                    Pattern::None(_) => tests.push(self.is_null(&v.ex)),
                    Pattern::Some(x, _) => {
                        bind = Some(x.clone());
                        tests.push(Ex::Not(Box::new(self.is_null(&v.ex))));
                    }
                    Pattern::Bool(b, _) => tests.push(if *b { v.ex.clone() } else { Ex::Not(Box::new(v.ex.clone())) }),
                    Pattern::Value(path, _) => {
                        let word = &path[path.len() - 1].0;
                        if let Ty::Enum(id) = st.present() {
                            let name = self.g.p.values(id).0.iter().find(|n| n.is(word)).map(|n| n.text.clone()).unwrap_or_else(|| word.clone());
                            let present = match (self.t(), &st) {
                                (Target::Go, Ty::Opt(_)) => Ex::Deref(Box::new(v.ex.clone())),
                                _ => v.ex.clone(),
                            };
                            tests.push(Ex::bin(Op::Eq, present, self.g.member(id, &name)));
                        }
                    }
                }
            }
            let (_, body) = self.apart(|f| {
                let mark = f.scope.len();
                if let Some(x) = &bind {
                    let name = f.local(x);
                    let inner_ty = st.present().clone();
                    let val = match f.t() {
                        Target::Go => Ex::Deref(Box::new(v.ex.clone())),
                        _ => v.ex.clone(),
                    };
                    let fty = f.g.flight(&inner_ty);
                    f.bind(&name, &fty, val);
                    let used = Rc::new(Cell::new(false));
                    let at = f.out.len();
                    f.scope.push((x.clone(), Val::new(Ex::atom(name.clone()), inner_ty), used.clone()));
                    f.block(&arm.body, &next);
                    if f.t() == Target::Go && !used.get() {
                        f.out.insert(at, Node::Line(format!("_ = {name}")));
                    }
                } else {
                    f.block(&arm.body, &next);
                }
                f.scope.truncate(mark);
            });
            if k + 1 == n {
                last = body;
            } else {
                let test = tests.into_iter().reduce(|a, b| Ex::bin(Op::Or, a, b)).unwrap_or(self.g.boolean(true));
                branches.push((test, body));
            }
        }
        if branches.is_empty() {
            self.out.extend(last);
        } else {
            self.emit(Node::If(branches, last));
        }
    }

    fn is_null(&self, v: &Ex) -> Ex {
        match self.t() {
            Target::Python => Ex::atom(format!("{} is None", v.print(Target::Python))),
            _ => Ex::bin(Op::Eq, v.clone(), self.g.null()),
        }
    }

    // ── Records ────────────────────────────────────────────────────────────

    fn record(&mut self, e: &Expr, inits: &[FieldInit]) -> Val {
        let ty = self.ty(e);
        let Ref::Record(id) = self.rf(e) else { return Val::new(self.g.null(), ty) };
        let fields = self.g.p.fields(&id);
        let es: Vec<&Expr> = inits.iter().map(|f| &f.value).collect();
        let module = matches!(id, RecId::Module { .. });
        let to: Vec<Option<(usize, Ty)>> = inits.iter().map(|fi| fields.iter().position(|(n, _)| n.is(&fi.name)).map(|k| (k, fields[k].1.ty.clone()))).collect();
        let checked = module
            && inits.iter().zip(&to).any(|(fi, t)| {
                t.as_ref().is_some_and(|(k, _)| {
                    let slot = &fields[*k].1;
                    (slot.range.is_some() || slot.count.is_some()) && self.g.open(fi.value.id, |w| matches!(w, What::Field { .. })) && self.g.outside_fn(slot, false).is_some()
                })
            });
        let (vals, keep) = self.arg_values(&es, &to, checked);
        // read in the fields' types, as written; held to what they declare, in their order
        let mut given: Vec<Option<(Val, usize)>> = vec![None; fields.len()];
        for ((fi, v), t) in inits.iter().zip(vals).zip(&to) {
            let Some((k, ft)) = t else { continue };
            let v = self.adapt(v, ft, &fi.value);
            let v = if keep { self.spill(v) } else { v };
            given[*k] = Some((v, fi.span.line));
        }
        let mut parts: Vec<(String, Ex)> = Vec::new();
        for (k, (f, slot)) in fields.iter().enumerate() {
            let (b, line) = match given[k].take() {
                Some((v, line)) => {
                    let b = match &id {
                        RecId::Element(file, seq) => {
                            let col = self.g.p.rules.get(file).and_then(|r| r.elements.iter().find(|(n, _, _)| n == seq)).and_then(|(_, _, cols)| cols.iter().find(|c| c.name == f.text).cloned());
                            match col {
                                Some(col) => self.rule_in(v, file, &col.ty),
                                None => v.ex,
                            }
                        }
                        _ => self.to_boundary(v, &slot.ty),
                    };
                    (b, line)
                }
                None => (self.g.null(), e.span.line),
            };
            let b = if module && (slot.range.is_some() || slot.count.is_some()) {
                let at = inits.iter().find(|fi| f.is(&fi.name)).map(|fi| fi.value.id);
                if at.is_some_and(|x| self.g.open(x, |w| matches!(w, What::Field { .. }))) {
                    let b = self.spill_as(b, &self.g.boundary(&slot.ty));
                    if let Some(fun) = self.g.outside_fn(slot, false) {
                        let o = Ex::atom(self.temp());
                        let rec = self.g.p.show_ty(&self.g.c, &Ty::Record(id.clone()));
                        let fail = self.g.fail_outside("range", line, &Said::Field { rec }, &o, self.g.counts(slot, false));
                        let stmts = self.g.if_found(&o, Ex::call(fun, vec![b.clone(), self.g.string(&f.text)]), fail);
                        for s in stmts {
                            self.emit(s);
                        }
                    }
                    b
                } else {
                    b
                }
            } else {
                b
            };
            let key = match &id {
                RecId::Element(file, seq) => {
                    let alias = self.g.p.rules.get(file).and_then(|r| r.elements.iter().find(|(n, _, _)| n == seq)).and_then(|(_, _, cols)| cols.iter().find(|c| c.name == f.text)).map(|c| c.alias.clone()).unwrap_or_else(|| f.text.clone());
                    match self.t() {
                        Target::Go => ritsu_emit::ident::pascal(&alias),
                        _ => alias,
                    }
                }
                RecId::Running(..) => match self.t() {
                    Target::Go => {
                        let mut cs = f.text.chars();
                        cs.next().map(|c| c.to_ascii_uppercase().to_string() + cs.as_str()).unwrap_or_default()
                    }
                    _ => f.text.clone(),
                },
                _ => field_name(self.t(), f.ascii()),
            };
            parts.push((key, b));
        }
        let tyname = match self.t() {
            Target::TypeScript => String::new(),
            _ => self.g.record_type(&id),
        };
        Val::new(Ex::Record(tyname, parts), ty)
    }

    // ── Calls ──────────────────────────────────────────────────────────────

    fn call(&mut self, e: &Expr, callee: &Expr, args: &[Arg]) -> Val {
        match self.rf(callee) {
            Ref::Fn(canon, name) => self.call_fn(e, &canon, &name, args),
            Ref::Rule(canon) => self.call_rule(e, callee, &canon, args),
            Ref::Date(canon, date) => self.call_date(e, callee, &canon, &date, args),
            Ref::Transfer(id) => self.transfer(e, &id, args),
            Ref::Builtin(name) => self.builtin(e, name, args),
            _ => Val::new(self.g.null(), self.ty(e)),
        }
    }

    /// A function of kaname: the arguments from the left, read in its parameters' types as written,
    /// held to what the parameters declare where the analysis does not show them, then the call
    /// (`interp.rs`'s `place_args`): of the module's function past the check of what a host gives,
    /// or of another module's as a host calls it.
    fn call_fn(&mut self, e: &Expr, canon: &Path, name: &str, args: &[Arg]) -> Val {
        let ty = self.ty(e);
        let es: Vec<&Expr> = args.iter().map(Arg::expr).collect();
        let g = self.g;
        let callee = g.p.module(canon).clone();
        let Some(fi) = callee.decls.fns.iter().position(|f| f.name.text == name) else { return Val::new(g.null(), ty) };
        let info = callee.decls.fns[fi].clone();
        let to: Vec<Option<(usize, Ty)>> = args
            .iter()
            .map(|a| match a {
                Arg::Named(n, _, _) => info.params.iter().position(|p| p.name.is(n)).map(|pi| (pi, info.params[pi].slot.ty.clone())),
                _ => None,
            })
            .collect();
        let checked = args.iter().zip(&to).any(|(a, t)| t.as_ref().is_some_and(|(pi, _)| g.open(a.expr().id, |w| matches!(w, What::Param { .. })) && g.outside_fn(&info.params[*pi].slot, true).is_some()));
        let (vals, keep) = self.arg_values(&es, &to, checked);
        let mut given: Vec<Option<Ex>> = vec![None; info.params.len()];
        let mut at_of: Vec<Option<usize>> = vec![None; info.params.len()];
        for ((a, v), t) in args.iter().zip(vals).zip(&to) {
            let (Arg::Named(_, _, x), Some((pi, pt))) = (a, t) else { continue };
            let v = self.adapt(v, pt, x);
            let v = if keep { self.spill(v) } else { v };
            given[*pi] = Some(self.to_boundary(v, pt));
            at_of[*pi] = Some(x.id);
        }
        for (pi, p) in info.params.iter().enumerate() {
            let Some(x) = at_of[pi] else { continue };
            if !g.open(x, |w| matches!(w, What::Param { .. })) {
                continue;
            }
            let Some(fun) = g.outside_fn(&p.slot, true) else { continue };
            let b = self.spill_as(given[pi].clone().unwrap_or(g.null()), &g.boundary(&p.slot.ty));
            given[pi] = Some(b.clone());
            let o = Ex::atom(self.temp());
            let fail = g.fail_outside("range", e.span.line, &Said::Given { fun: info.name.text.clone() }, &o, g.counts(&p.slot, true));
            let stmts = g.if_found(&o, Ex::call(fun, vec![b, g.string(&p.name.text)]), fail);
            for s in stmts {
                self.emit(s);
            }
        }
        let args: Vec<Ex> = given.into_iter().map(|x| x.unwrap_or(g.null())).collect();
        let raw = if *canon == g.c.canon {
            let f = inner_name(self.t(), &callee, fi);
            match self.t() {
                // the function of the body takes its parameters by the names of its code
                Target::Python => Ex::CallKw(Box::new(Ex::atom(f)), param_names(g, fi).into_iter().zip(args).collect()),
                _ => Ex::call(f, args),
            }
        } else {
            let m = g.imp(Import::Module(canon.to_path_buf()));
            let f = format!("{m}.{}", fn_name(self.t(), &callee, fi));
            match self.t() {
                Target::Python => Ex::CallKw(Box::new(Ex::atom(f)), info.params.iter().map(|p| p.name.ascii().to_string()).zip(args).collect()),
                Target::Go => {
                    let fails = callee.analysis.get().and_then(|a| a.fns.get(fi)).is_some_and(|x| !x.effect.fails.is_empty());
                    if fails { g.kcall("Must", vec![Ex::call(f, args)]) } else { Ex::call(f, args) }
                }
                _ => Ex::call(f, args),
            }
        };
        let raw = self.spill_as(raw, &g.boundary(&info.ret.ty));
        let v = self.from_boundary(raw, &info.ret.ty, &info.ret.ty);
        Val::new(v.ex, info.ret.ty.clone())
    }

    /// A rule, through its code in the package (DESIGN 10.5): the arguments from the left, read in
    /// the types of the inputs, then the call. Where the analysis does not show the call keeps what
    /// the rule takes (an input's range, a precondition, a day), the rule's refusal is the failure
    /// `rule` at the call's line; where it does, nothing is wrapped (DESIGN 6.4).
    fn call_rule(&mut self, e: &Expr, callee: &Expr, canon: &Path, args: &[Arg]) -> Val {
        let ty = self.ty(e);
        let g = self.g;
        let es: Vec<&Expr> = args.iter().map(Arg::expr).collect();
        let Some(facts) = g.p.rules.get(canon).cloned() else { return Val::new(g.null(), ty) };
        let name = g.src(callee.span);
        let call = g.call_of(&facts).clone();
        // what the call gives each input and each list, by the rule's names, each read in its type
        let mut places: Vec<(String, Ty, Option<ColumnType>)> = facts.inputs.iter().map(|c| (c.name.clone(), crate::names::column_slot(c, canon).ty, Some(c.ty.clone()))).collect();
        for (seq, _, _) in &facts.elements {
            places.push((seq.clone(), Ty::List(Box::new(Ty::Record(RecId::Element(canon.to_path_buf(), seq.clone())))), None));
        }
        // the place of each argument, as the code of the rule takes them
        let order = |k: usize| call.params.iter().position(|p| p.name == places[k].0).unwrap_or(k);
        let to: Vec<Option<(usize, Ty)>> = args
            .iter()
            .map(|a| {
                let Arg::Named(n, _, _) = a else { return None };
                let k = places.iter().position(|(q, _, _)| q == n).or_else(|| facts.inputs.iter().position(|c| c.alias == *n)).or_else(|| facts.elements.iter().position(|(_, al, _)| al == n).map(|i| facts.inputs.len() + i))?;
                Some((k, places[k].1.clone()))
            })
            .collect();
        let placed: Vec<Option<(usize, Ty)>> = to.iter().map(|t| t.as_ref().map(|(k, ty)| (order(*k), ty.clone()))).collect();
        let (vals, keep) = self.arg_values(&es, &placed, false);
        let mut given: Vec<Option<Ex>> = vec![None; places.len()];
        let mut open = g.open(e.id, |w| matches!(w, What::Precondition { .. }));
        for ((a, v), t) in args.iter().zip(vals).zip(&to) {
            let (Arg::Named(_, _, x), Some((k, pt))) = (a, t) else { continue };
            open |= g.open(x.id, |w| matches!(w, What::RuleInput { .. } | What::RuleEnum { .. } | What::RuleField { .. } | What::RuleDate { .. }));
            let v = self.adapt(v, pt, x);
            let v = if keep { self.spill(v) } else { v };
            let ex = match &places[*k].2 {
                Some(col) => self.rule_in(v, canon, &col.clone()),
                None => v.ex,
            };
            given[*k] = Some(ex);
        }
        let r = g.imp(Import::Rule(canon.to_path_buf()));
        // the code's parameters, in its order (the inputs, then the lists)
        let mut ordered: Vec<Ex> = Vec::new();
        let mut fields: Vec<(String, Ex)> = Vec::new();
        for p in &call.params {
            let k = places.iter().position(|(q, _, _)| *q == p.name);
            let x = k.and_then(|k| given[k].clone()).unwrap_or(g.null());
            ordered.push(x.clone());
            fields.push((p.alias.clone(), x));
        }
        let rt = g.record_type(&RecId::Outputs(canon.to_path_buf()));
        let raw = match self.t() {
            Target::Go => Ex::call(format!("{r}.{}", call.function), vec![Ex::Record(format!("{r}.{}", if call.input_type.is_empty() { "Input" } else { &call.input_type }), fields)]),
            _ => Ex::call(format!("{r}.{}", call.function), ordered),
        };
        let out = self.temp();
        let said = g.say(&tr!("規則 `{name}` が入力を受け付けません", "The rule `{name}` refuses its inputs"));
        let line = e.span.line;
        match self.t() {
            Target::TypeScript => {
                if open {
                    self.declare(&out, &rt);
                    let body = vec![Node::Line(format!("{out} = {};", raw.print(self.t())))];
                    let catch = vec![
                        Node::If(
                            vec![(
                                Ex::atom(format!("e instanceof {r}.RuleInputError || e instanceof {r}.RuleContradictionError")),
                                vec![Node::Line(g.stmt(g.kcall("fail", vec![g.string("rule"), g.module_const(), Ex::atom(line.to_string()), g.string(&said), Ex::atom("e")])))],
                            )],
                            Vec::new(),
                        ),
                        Node::Line("throw e;".into()),
                    ];
                    self.emit(Node::Block("try".into(), body, vec![("catch (e)".into(), catch)]));
                } else {
                    self.bind(&out, &rt, raw);
                }
            }
            Target::Python => {
                if open {
                    let body = vec![Node::Line(format!("{out}: {rt} = {}", raw.print(self.t())))];
                    let k = g.imp(Import::Kaname);
                    let catch = vec![Node::Line(format!("raise {k}.KanameError(\"rule\", _MODULE, {line}, {}) from e", g.string(&said).print(self.t())))];
                    self.emit(Node::Block("try".into(), body, vec![(format!("except ({r}.RuleInputError, {r}.RuleContradictionError) as e"), catch)]));
                } else {
                    self.bind(&out, &rt, raw);
                }
            }
            Target::Go => {
                let err = self.temp();
                self.emit(Node::Line(format!("{out}, {err} := {}", raw.print(self.t()))));
                let fail = if open {
                    g.stmt(g.kcall("Refused", vec![g.string("rule"), g.module_const(), Ex::atom(line.to_string()), g.string(&said), Ex::atom(err.clone())]))
                } else {
                    format!("panic({err})")
                };
                self.emit(Node::If(vec![(Ex::bin(Op::Ne, Ex::atom(err.clone()), g.null()), vec![Node::Line(fail)])], Vec::new()));
            }
        }
        Val::new(Ex::atom(out), ty)
    }

    /// A date of a dates file, through its code in the package: the date's inputs in its order, the
    /// day as koyomi's code takes it; koyomi's refusal is the failure `date` where the analysis does
    /// not show the call keeps what the date takes.
    fn call_date(&mut self, e: &Expr, callee: &Expr, canon: &Path, date: &str, args: &[Arg]) -> Val {
        let ty = self.ty(e);
        let g = self.g;
        let es: Vec<&Expr> = args.iter().map(Arg::expr).collect();
        let Some(facts) = g.p.dates.get(canon).cloned() else { return Val::new(g.null(), ty) };
        let name = g.src(callee.span);
        let Some(func) = facts.functions.iter().find(|f| f.name == date).cloned() else { return Val::new(g.null(), ty) };
        let input_of = |a: &Arg| match a {
            Arg::Named(n, _, _) => facts.inputs.iter().find(|i| i.name == *n || i.alias == *n),
            _ => None,
        };
        let place_of = |kind: ritsu_ports::DateKind| match kind {
            ritsu_ports::DateKind::Date => Ty::Date,
            ritsu_ports::DateKind::Int => Ty::Num(NumTy::number()),
        };
        let to: Vec<Option<(usize, Ty)>> = args.iter().map(|a| input_of(a).map(|i| (func.params.iter().position(|p| *p == i.name).unwrap_or(usize::MAX), place_of(i.kind)))).collect();
        let (vals, keep) = self.arg_values(&es, &to, false);
        let mut given: BTreeMap<String, Ex> = BTreeMap::new();
        let mut open = false;
        for (a, v) in args.iter().zip(vals) {
            let Arg::Named(_, _, x) = a else { continue };
            let Some(input) = input_of(a) else { continue };
            open |= g.open(x.id, |w| matches!(w, What::DateDay { .. } | What::DateInt { .. }));
            let v = self.adapt(v, &place_of(input.kind), x);
            let v = if keep { self.spill(v) } else { v };
            let d = g.import_name(&Import::Dates(canon.to_path_buf()));
            let ex = match (input.kind, self.t()) {
                (ritsu_ports::DateKind::Int, Target::TypeScript) => Ex::call("Number", vec![v.ex]),
                (ritsu_ports::DateKind::Int, Target::Go) => Ex::call("int", vec![v.ex]),
                (ritsu_ports::DateKind::Date, Target::Go) => Ex::Cast(Box::new(v.ex), format!("{d}.Date")),
                _ => v.ex,
            };
            given.insert(input.name.clone(), ex);
        }
        let d = g.imp(Import::Dates(canon.to_path_buf()));
        let fname = match self.t() {
            Target::Go => ritsu_emit::ident::pascal(&func.alias),
            _ => func.alias.clone(),
        };
        let argv: Vec<Ex> = func.params.iter().map(|p| given.get(p).cloned().unwrap_or(g.null())).collect();
        let raw = Ex::call(format!("{d}.{fname}"), argv);
        let out = self.temp();
        let said = g.say(&tr!("日付 `{name}` が求まりません", "The date `{name}` cannot be worked out"));
        let line = e.span.line;
        let dt = g.flight(&Ty::Date);
        match self.t() {
            Target::TypeScript => {
                if open {
                    self.declare(&out, &dt);
                    let body = vec![Node::Line(format!("{out} = {};", raw.print(self.t())))];
                    let catch = vec![
                        Node::If(
                            vec![(Ex::atom(format!("e instanceof {d}.KoyomiError")), vec![Node::Line(g.stmt(g.kcall("fail", vec![g.string("date"), g.module_const(), Ex::atom(line.to_string()), g.string(&said), Ex::atom("e")])))])],
                            Vec::new(),
                        ),
                        Node::Line("throw e;".into()),
                    ];
                    self.emit(Node::Block("try".into(), body, vec![("catch (e)".into(), catch)]));
                } else {
                    self.bind(&out, &dt, raw);
                }
                Val::new(Ex::atom(out), ty)
            }
            Target::Python => {
                if open {
                    let body = vec![Node::Line(format!("{out}: {dt} = {}", raw.print(self.t())))];
                    let k = g.imp(Import::Kaname);
                    let catch = vec![Node::Line(format!("raise {k}.KanameError(\"date\", _MODULE, {line}, {}) from e", g.string(&said).print(self.t())))];
                    self.emit(Node::Block("try".into(), body, vec![(format!("except {d}.KoyomiError as e"), catch)]));
                } else {
                    self.bind(&out, &dt, raw);
                }
                Val::new(Ex::atom(out), ty)
            }
            Target::Go => {
                let err = self.temp();
                self.emit(Node::Line(format!("{out}, {err} := {}", raw.print(self.t()))));
                let fail = if open {
                    g.stmt(g.kcall("Refused", vec![g.string("date"), g.module_const(), Ex::atom(line.to_string()), g.string(&said), Ex::atom(err.clone())]))
                } else {
                    format!("panic({err})")
                };
                self.emit(Node::If(vec![(Ex::bin(Op::Ne, Ex::atom(err.clone()), g.null()), vec![Node::Line(fail)])], Vec::new()));
                let k = g.imp(Import::Kaname);
                Val::new(Ex::Cast(Box::new(Ex::atom(out)), format!("{k}.Date")), ty)
            }
        }
    }

    /// A transfer built (DESIGN 6.3): its arguments from the left, read in the book's types; an
    /// amount the analysis does not show is 0 or more is checked as it is read, as the interpreter
    /// checks it.
    fn transfer(&mut self, e: &Expr, id: &TransferId, args: &[Arg]) -> Val {
        let ty = self.ty(e);
        let g = self.g;
        let es: Vec<&Expr> = args.iter().map(Arg::expr).collect();
        let Some((book, params)) = g.p.transfer(id).map(|(b, p)| (b.clone(), p)) else { return Val::new(g.null(), ty) };
        let to: Vec<Option<(usize, Ty)>> = args
            .iter()
            .map(|a| match a {
                Arg::Named(n, _, _) => params.iter().position(|(q, _)| q == n).map(|k| (k, params[k].1.clone())),
                _ => None,
            })
            .collect();
        let checked = args.iter().zip(&to).any(|(a, t)| matches!(t, Some((_, Ty::Num(_)))) && g.open(a.expr().id, |w| matches!(w, What::Amount { .. })));
        let (vals, keep) = self.arg_values(&es, &to, checked);
        let mut given: Vec<Option<Ex>> = vec![None; params.len()];
        for ((a, v), t) in args.iter().zip(vals).zip(&to) {
            let (Arg::Named(n, _, x), Some((k, pt))) = (a, t) else { continue };
            let (k, pt) = (*k, pt.clone());
            let v = self.adapt(v, &pt, x);
            let v = if let (Ty::Num(nt), true) = (&pt, g.open(x.id, |w| matches!(w, What::Amount { .. }))) {
                let v = self.spill(v);
                let t = format!("{}.{}", id.transfer, id.op);
                let shown = g.kcall(&g.helper("numText", "num_text", "IntText"), vec![if self.t() == Target::Go { v.ex.clone() } else { g.to_q_ex(v.ex.clone()) }, g.string(&unit_suffix(&nt.unit))]);
                let msg = template(&tr!("振替 `{t}` の額 `{n}` に、負の値 \u{1}shown\u{1} を渡しています", "The amount `{n}` of the transfer `{t}` is given \u{1}shown\u{1}, which is below 0"), g.lang)
                    .into_iter()
                    .map(|p| match p {
                        Ok(s) => g.string(&s),
                        Err(_) => shown.clone(),
                    })
                    .reduce(|a, b| Ex::bin(Op::Add, a, b))
                    .unwrap_or(g.string(""));
                let fail = Node::Line(g.stmt(g.kcall(&g.helper("fail", "fail", "Fail"), vec![g.string("range"), g.module_const(), Ex::atom(x.span.line.to_string()), msg])));
                self.emit(Node::If(vec![(Ex::bin(Op::Lt, v.ex.clone(), g.int(0)), vec![fail])], Vec::new()));
                v
            } else if keep {
                self.spill(v)
            } else {
                v
            };
            given[k] = Some(self.to_boundary(v, &pt));
        }
        let names = super::books::param_names(self.t(), &book, &id.transfer);
        let tyname = g.transfer_type(id);
        let ex = match self.t() {
            Target::TypeScript => {
                let args: Vec<(String, Ex)> = params.iter().zip(given).zip(&names).filter_map(|(((_, _), v), n)| v.map(|v| (ritsu_emit::lit::json(n), v))).collect();
                let quote = |s: &str| ritsu_emit::lit::json(s);
                Ex::Record(
                    String::new(),
                    vec![
                        ("book".into(), Ex::atom(quote(&book.name))),
                        ("transfer".into(), Ex::atom(quote(&id.transfer))),
                        ("op".into(), Ex::atom(quote(&id.op))),
                        ("args".into(), Ex::Record(String::new(), args)),
                    ],
                )
            }
            _ => {
                let args: Vec<(String, Ex)> = params.iter().zip(given).zip(&names).filter_map(|(((_, _), v), n)| v.map(|v| (n.clone(), v))).collect();
                let args = if self.t() == Target::Go {
                    // a post's amounts that may be left out are pointers
                    let amounts = super::books::optional_amounts(&book, &id.transfer, &id.op);
                    args.into_iter().map(|(n, v)| if amounts.contains(&n) { (n, g.kcall("Some", vec![v])) } else { (n, v) }).collect()
                } else {
                    args
                };
                Ex::Record(tyname.clone(), args)
            }
        };
        let ex = if self.t() == Target::TypeScript { Ex::Cast(Box::new(ex), tyname) } else { ex };
        Val::new(ex, ty)
    }

    // ── Builtins ───────────────────────────────────────────────────────────

    fn builtin(&mut self, e: &Expr, name: &str, args: &[Arg]) -> Val {
        match (name, args) {
            ("reduce", [list, first, f]) => self.reduce(e, list.expr(), first.expr(), f.expr()),
            ("count" | "sum" | "any" | "all" | "map" | "filter" | "first" | "max_of" | "min_of" | "running_totals", [list, f]) => self.walk(e, name, list.expr(), f.expr()),
            _ => {
                let es: Vec<&Expr> = args.iter().map(Arg::expr).collect();
                let vals = self.values(&es);
                self.scalar(e, name, &es, vals)
            }
        }
    }

    /// The loop over a list with the function given to a builtin: `each` gets the element's name,
    /// and writes the body with the function's answer for it.
    fn each(&mut self, xs: Val, lam: &Expr, params: Vec<Val>, mut body: impl FnMut(&mut Self, Val) -> Vec<Node>) -> Node {
        let ExprKind::Lambda(ps, lbody) = &lam.kind else { return Node::Line(String::new()) };
        let elem_ty = match &xs.ty {
            Ty::List(t) => (**t).clone(),
            _ => Ty::Error,
        };
        let el_name = self.local(&ps.last().map(|(p, _)| p.clone()).unwrap_or_default());
        let mark = self.scope.len();
        let used = Rc::new(Cell::new(false));
        // the parameters before the element (`reduce`'s accumulator) are bound by the caller
        for (p, v) in ps.iter().zip(params) {
            self.scope.push((p.0.clone(), v, Rc::new(Cell::new(true))));
        }
        self.scope.push((ps.last().map(|(p, _)| p.clone()).unwrap_or_default(), Val::new(Ex::atom(el_name.clone()), elem_ty.clone()), used.clone()));
        let (answer, mut stmts) = self.apart(|s| s.expr(lbody));
        let tail = {
            let saved = std::mem::take(&mut self.out);
            let mut more = body(self, answer);
            let pre = std::mem::replace(&mut self.out, saved);
            let mut all = pre;
            all.append(&mut more);
            all
        };
        stmts.extend(tail);
        self.scope.truncate(mark);
        let t = self.t();
        let head = match t {
            Target::TypeScript => format!("for (const {el_name} of {})", xs.ex.print(t)),
            Target::Python => format!("for {el_name} in {}", xs.ex.print(t)),
            Target::Go => {
                if used.get() {
                    format!("for _, {el_name} := range {}", xs.ex.print(t))
                } else {
                    format!("for range {}", xs.ex.print(t))
                }
            }
        };
        Node::Block(head, stmts, Vec::new())
    }

    /// The type the answers of the function given to a builtin are read in (`interp.rs`'s
    /// `lambda_place`).
    fn lambda_place(&self, e: &Expr, name: &str) -> Option<Ty> {
        let me = self.ty(e);
        match name {
            "sum" => Some(me),
            "max_of" | "min_of" => Some(me.present().clone()),
            "map" => match me {
                Ty::List(t) => Some(*t),
                _ => None,
            },
            "running_totals" => match me {
                Ty::List(t) => match *t {
                    Ty::Record(RecId::Running(_, w)) => Some(*w),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }
    }

    fn walk(&mut self, e: &Expr, name: &str, list: &Expr, lam: &Expr) -> Val {
        let ty = self.ty(e);
        let xs = self.expr(list);
        let xs = self.spill(xs);
        let to = self.lambda_place(e, name);
        let lam_ty = self.ty(lam);
        let t = self.t();
        let g = self.g;
        let acc = self.temp();
        let brk = Node::Line(if t == Target::TypeScript { "break;".into() } else { "break".into() });
        let truth = |v: &Val| v.ex.clone();
        match name {
            "any" | "all" => {
                let start = name == "all";
                let bt = g.flight(&Ty::Bool);
                self.mutable(&acc, &bt, g.boolean(start));
                let a2 = acc.clone();
                let node = self.each(xs, lam, vec![], |s, v| {
                    let test = if start { Ex::Not(Box::new(truth(&v))) } else { truth(&v) };
                    vec![Node::If(vec![(test, vec![s.assign_line(&a2, s.g.boolean(!start)), brk.clone()])], Vec::new())]
                });
                self.emit(node);
                Val::new(Ex::atom(acc), ty)
            }
            "count" => {
                let it = g.int_type();
                self.mutable(&acc, it, g.int(0));
                let a2 = acc.clone();
                let node = self.each(xs, lam, vec![], |s, v| {
                    let one = Ex::bin(Op::Add, Ex::atom(a2.clone()), s.g.int(1));
                    vec![Node::If(vec![(truth(&v), vec![s.assign_line(&a2, one)])], Vec::new())]
                });
                self.emit(node);
                Val::new(Ex::atom(acc), ty)
            }
            "first" => {
                let fty = g.flight(&ty);
                self.mutable(&acc, &fty, g.null());
                let a2 = acc.clone();
                let ExprKind::Lambda(ps, _) = &lam.kind else { return Val::new(g.null(), ty) };
                let el = ps.last().map(|(p, _)| p.clone()).unwrap_or_default();
                let node = self.each(xs, lam, vec![], |s, v| {
                    let x = s.lookup(&el).map(|x| x.ex).unwrap_or(s.g.null());
                    let x = match (s.t(), &ty) {
                        // an element that is there, where one may be absent
                        (Target::Go, Ty::Opt(_)) if !matches!(s.lookup(&el).map(|x| x.ty), Some(Ty::Opt(_))) => s.g.kcall("Some", vec![x]),
                        _ => x,
                    };
                    vec![Node::If(vec![(truth(&v), vec![s.assign_line(&a2, x), brk.clone()])], Vec::new())]
                });
                self.emit(node);
                Val::new(Ex::atom(acc), ty)
            }
            "filter" => {
                let fty = g.flight(&ty);
                let a2 = acc.clone();
                self.new_list(&acc, &fty);
                let ExprKind::Lambda(ps, _) = &lam.kind else { return Val::new(g.null(), ty) };
                let el = ps.last().map(|(p, _)| p.clone()).unwrap_or_default();
                let node = self.each(xs, lam, vec![], |s, v| {
                    let x = s.lookup(&el).map(|x| x.ex).unwrap_or(s.g.null());
                    vec![Node::If(vec![(truth(&v), vec![s.push(&a2, x)])], Vec::new())]
                });
                self.emit(node);
                Val::new(Ex::atom(acc), ty)
            }
            "map" => {
                let fty = g.flight(&ty);
                self.new_list(&acc, &fty);
                let a2 = acc.clone();
                let to = to.unwrap_or(lam_ty.clone());
                let node = self.each(xs, lam, vec![], |s, v| {
                    let v = s.adapt(v, &to, lam);
                    vec![s.push(&a2, v.ex)]
                });
                self.emit(node);
                Val::new(Ex::atom(acc), ty)
            }
            "sum" => {
                let check = g.overflow_at(e);
                let fty = g.flight(&ty);
                let r = ty.num().map(rep).unwrap_or(Rep::Int);
                let zero = match r {
                    Rep::Int => g.int(0),
                    Rep::Q => g.frac(Rat::zero()),
                };
                self.mutable(&acc, &fty, zero);
                let a2 = acc.clone();
                let to = to.unwrap_or(ty.clone());
                let node = self.each(xs, lam, vec![], |s, v| {
                    let v = s.adapt(v, &to, lam);
                    let next = match r {
                        Rep::Int => s.int_op(Op::Add, Ex::atom(a2.clone()), v.ex, check.as_ref()),
                        Rep::Q => s.q_op(Op::Add, Ex::atom(a2.clone()), v.ex, check.as_ref()),
                    };
                    vec![s.assign_line(&a2, next)]
                });
                self.emit(node);
                Val::new(Ex::atom(acc), ty)
            }
            "max_of" | "min_of" => {
                let fty = g.flight(&ty);
                self.mutable(&acc, &fty, g.null());
                let a2 = acc.clone();
                let present = ty.present().clone();
                let to = to.unwrap_or(present.clone());
                let max = name == "max_of";
                let node = self.each(xs, lam, vec![], |s, v| {
                    let v = s.adapt(v, &to, lam);
                    let v = s.spill(v);
                    let best = match s.t() {
                        Target::Go => Ex::Deref(Box::new(Ex::atom(a2.clone()))),
                        _ => Ex::atom(a2.clone()),
                    };
                    let better = s.order(&present, v.ex.clone(), best, if max { Op::Gt } else { Op::Lt });
                    let none = s.is_null(&Ex::atom(a2.clone()));
                    let store = match s.t() {
                        Target::Go => s.g.kcall("Some", vec![v.ex.clone()]),
                        _ => v.ex.clone(),
                    };
                    vec![Node::If(vec![(Ex::bin(Op::Or, none, better), vec![s.assign_line(&a2, store)])], Vec::new())]
                });
                self.emit(node);
                Val::new(Ex::atom(acc), ty)
            }
            "running_totals" => self.running(e, xs, lam, &ty, to),
            _ => Val::new(g.null(), ty),
        }
    }

    /// `a op b` of two values of `t` in the order of their type: numbers, dates, an ordered enum.
    fn order(&self, t: &Ty, a: Ex, b: Ex, op: Op) -> Ex {
        let g = self.g;
        match (t, self.t()) {
            (Ty::Num(n), _) if rep(n) == Rep::Int => Ex::bin(op, a, b),
            (Ty::Num(_), Target::Python) => Ex::bin(op, a, b),
            (Ty::Num(_), Target::TypeScript) => Ex::bin(op, g.kcall("cmp", vec![a, b]), Ex::atom("0")),
            (Ty::Num(_), Target::Go) => Ex::bin(op, g.kcall("Cmp", vec![a, b]), Ex::atom("0")),
            (Ty::Date, Target::Go) => Ex::bin(op, g.kcall("Compare", vec![a, b]), Ex::atom("0")),
            (Ty::Enum(id), _) => {
                let r = g.rank_of(id);
                Ex::bin(op, Ex::call(r.clone(), vec![a]), Ex::call(r, vec![b]))
            }
            _ => Ex::bin(op, a, b),
        }
    }

    /// `running_totals(xs, x => w)` (`interp.rs`'s `fold`): the weights first, then their total, then
    /// each element with what comes before it and up to it, each sum held to int64 at the call.
    fn running(&mut self, e: &Expr, xs: Val, lam: &Expr, ty: &Ty, to: Option<Ty>) -> Val {
        let g = self.g;
        let check = g.overflow_at(e);
        let Ty::List(rt) = ty else { return Val::new(g.null(), ty.clone()) };
        let Ty::Record(RecId::Running(item_ty, w_ty)) = &**rt else { return Val::new(g.null(), ty.clone()) };
        let w_ty = to.unwrap_or((**w_ty).clone());
        let r = w_ty.num().map(rep).unwrap_or(Rep::Int);
        let wf = g.flight(&w_ty);
        let weights = self.temp();
        self.new_list(&weights, &g.list_type(wf.clone()));
        let w2 = weights.clone();
        let node = self.each(xs.clone(), lam, vec![], |s, v| {
            let v = s.adapt(v, &w_ty, lam);
            vec![s.push(&w2, v.ex)]
        });
        self.emit(node);
        let zero = match r {
            Rep::Int => g.int(0),
            Rep::Q => g.frac(Rat::zero()),
        };
        let total = self.temp();
        self.mutable(&total, &wf, zero.clone());
        let w = self.temp();
        let add = |s: &Self, a: Ex, b: Ex| match r {
            Rep::Int => s.int_op(Op::Add, a, b, check.as_ref()),
            Rep::Q => s.q_op(Op::Add, a, b, check.as_ref()),
        };
        let next = add(self, Ex::atom(total.clone()), Ex::atom(w.clone()));
        let head = match self.t() {
            Target::TypeScript => format!("for (const {w} of {weights})"),
            Target::Python => format!("for {w} in {weights}"),
            Target::Go => format!("for _, {w} := range {weights}"),
        };
        self.emit(Node::Block(head, vec![self.assign_line(&total, next)], Vec::new()));
        let out = self.temp();
        let fty = g.flight(ty);
        self.new_list(&out, &fty);
        let before = self.temp();
        self.mutable(&before, &wf, zero);
        let i = self.temp();
        let upto = self.temp();
        let item = match self.t() {
            Target::TypeScript | Target::Python => Ex::Index(Box::new(xs.ex.clone()), Box::new(Ex::atom(i.clone()))),
            Target::Go => Ex::Index(Box::new(xs.ex.clone()), Box::new(Ex::atom(i.clone()))),
        };
        let wi = Ex::Index(Box::new(Ex::atom(weights.clone())), Box::new(Ex::atom(i.clone())));
        let up = add(self, Ex::atom(before.clone()), wi);
        let mut body = Vec::new();
        let (bv, mut pre) = self.apart(|s| {
            let item_b = s.to_boundary(Val::new(item.clone(), (**item_ty).clone()), item_ty);
            let bb = s.to_boundary(Val::new(Ex::atom(before.clone()), w_ty.clone()), &w_ty);
            let ub = s.to_boundary(Val::new(Ex::atom(upto.clone()), w_ty.clone()), &w_ty);
            let tb = s.to_boundary(Val::new(Ex::atom(total.clone()), w_ty.clone()), &w_ty);
            let (ki, kb, ku, kt) = match s.t() {
                Target::Go => ("Item", "Before", "Upto", "Total"),
                _ => ("item", "before", "upto", "total"),
            };
            let tyname = match s.t() {
                Target::TypeScript => String::new(),
                _ => s.g.record_type(&RecId::Running(item_ty.clone(), Box::new(w_ty.clone()))),
            };
            Ex::Record(tyname, vec![(ki.into(), item_b), (kb.into(), bb), (ku.into(), ub), (kt.into(), tb)])
        });
        match self.t() {
            Target::TypeScript => body.push(Node::Line(format!("const {upto}: {wf} = {};", up.print(self.t())))),
            Target::Python => body.push(Node::Line(format!("{upto}: {wf} = {}", up.print(self.t())))),
            Target::Go => body.push(Node::Line(format!("var {upto} {wf} = {}", up.print(self.t())))),
        }
        body.append(&mut pre);
        body.push(self.push(&out, bv));
        body.push(self.assign_line(&before, Ex::atom(upto.clone())));
        let head = match self.t() {
            Target::TypeScript => format!("for (let {i} = 0; {i} < {}.length; {i}++)", xs.ex.print(self.t())),
            Target::Python => format!("for {i} in range(len({}))", xs.ex.print(self.t())),
            Target::Go => format!("for {i} := range {}", xs.ex.print(self.t())),
        };
        self.emit(Node::Block(head, body, Vec::new()));
        Val::new(Ex::atom(out), ty.clone())
    }

    /// `reduce(xs, first, (acc, x) => …)`: the running value in the type of the first, the answer in
    /// the type of `reduce` (`interp.rs`'s `reduce`).
    fn reduce(&mut self, e: &Expr, list: &Expr, first: &Expr, lam: &Expr) -> Val {
        let ty = self.ty(e);
        let vs = self.values(&[list, first]);
        let xs = self.spill(vs[0].clone());
        // the running value is no constant, whatever the first is
        let first_ty = match self.ty(first) {
            Ty::Num(n) => Ty::Num(NumTy { konst: None, ..n }),
            t => t,
        };
        let acc = self.temp();
        let at = self.g.flight(&first_ty);
        self.mutable(&acc, &at, vs[1].ex.clone());
        let a2 = acc.clone();
        let accv = Val::new(Ex::atom(acc.clone()), first_ty.clone());
        let node = self.each(xs, lam, vec![accv], |s, v| {
            let v = s.adapt(v, &first_ty, lam);
            vec![s.assign_line(&a2, v.ex)]
        });
        self.emit(node);
        let v = Val::new(Ex::atom(acc), first_ty);
        self.adapt(v, &ty, first)
    }

    /// A variable given a first value, written again later.
    fn mutable(&mut self, name: &str, ty: &str, value: Ex) {
        let t = self.t();
        let line = match t {
            Target::TypeScript => format!("let {name}: {ty} = {};", value.print(t)),
            Target::Python => format!("{name}: {ty} = {}", value.print(t)),
            Target::Go => format!("var {name} {ty} = {}", value.print(t)),
        };
        self.emit(Node::Line(line));
    }

    /// An empty list to push to.
    fn new_list(&mut self, name: &str, ty: &str) {
        let t = self.t();
        let ty = match t {
            Target::TypeScript => ty.trim_start_matches("readonly ").to_string(),
            _ => ty.to_string(),
        };
        let line = match t {
            Target::TypeScript => format!("const {name}: {ty} = [];"),
            Target::Python => format!("{name}: {ty} = []"),
            Target::Go => format!("var {name} {ty} = {ty}{{}}"),
        };
        self.emit(Node::Line(line));
    }

    fn push(&self, list: &str, v: Ex) -> Node {
        let t = self.t();
        Node::Line(match t {
            Target::TypeScript => format!("{list}.push({});", v.print(t)),
            Target::Python => format!("{list}.append({})", v.print(t)),
            Target::Go => format!("{list} = {}", Ex::call("append", vec![Ex::atom(list), v]).print(t)),
        })
    }

    /// A builtin that takes no function (`interp.rs`'s `scalar`).
    fn scalar(&mut self, e: &Expr, name: &str, xs: &[&Expr], vals: Vec<Val>) -> Val {
        let me = self.ty(e);
        let g = self.g;
        let t = self.t();
        match name {
            "min" | "max" => {
                let a = self.adapt(vals[0].clone(), &me, xs[0]);
                let b = self.adapt(vals[1].clone(), &me, xs[1]);
                let (a, b) = (self.spill(a), self.spill(b));
                let op = if name == "min" { Op::Le } else { Op::Ge };
                let test = self.order(me.present(), a.ex.clone(), b.ex.clone(), op);
                match t {
                    Target::Go => {
                        let n = self.temp();
                        let ty = g.flight(&me);
                        self.declare(&n, &ty);
                        let then = vec![self.assign_line(&n, a.ex)];
                        let els = vec![self.assign_line(&n, b.ex)];
                        self.emit(Node::If(vec![(test, then)], els));
                        Val::new(Ex::atom(n), me)
                    }
                    _ => Val::new(Ex::Cond(Box::new(test), Box::new(a.ex), Box::new(b.ex)), me),
                }
            }
            "mod" => {
                let d = vals.get(1).and_then(|v| match &v.ex {
                    Ex::Atom(s) => s.trim_end_matches('n').parse::<i128>().ok(),
                    _ => None,
                });
                let x = self.rep_to(vals[0].ex.clone(), vals[0].ty.num().map(rep).unwrap_or(Rep::Int), Rep::Q);
                let ex = match d {
                    Some(d) if d > 0 => g.kcall(&g.helper("mod", "mod", "Mod"), vec![x, g.int(d)]),
                    _ => g.int(0),
                };
                Val::new(self.rep_to(ex, Rep::Int, me.num().map(rep).unwrap_or(Rep::Int)), me)
            }
            "isqrt" => {
                let x = self.rep_to(vals[0].ex.clone(), vals[0].ty.num().map(rep).unwrap_or(Rep::Int), Rep::Q);
                let checked = g.open(xs[0].id, |w| matches!(w, What::Isqrt));
                let r = me.num().map(rep).unwrap_or(Rep::Int);
                if !checked {
                    let ex = match t {
                        Target::TypeScript => Ex::atom(format!("{}!", g.kcall("isqrt", vec![x]).print(t))),
                        Target::Python => g.kcall("root", vec![x]),
                        Target::Go => g.kcall("Root", vec![x]),
                    };
                    return Val::new(self.rep_to(ex, Rep::Int, r), me);
                }
                let xv = self.spill(Val::new(x, Ty::Num(NumTy { unit: Unit::number(), step: None, konst: None, ratio: false })));
                let n = self.temp();
                let shown = g.kcall(&g.helper("ratText", "rat_text", "RatText"), vec![xv.ex.clone()]);
                let msg = template(&tr!("`isqrt` に負の値 \u{1}shown\u{1} を渡しています", "`isqrt` is given \u{1}shown\u{1}, which is below 0"), g.lang)
                    .into_iter()
                    .map(|p| match p {
                        Ok(s) => g.string(&s),
                        Err(_) => shown.clone(),
                    })
                    .reduce(|a, b| Ex::bin(Op::Add, a, b))
                    .unwrap_or(g.string(""));
                let fail = Node::Line(g.stmt(g.kcall(&g.helper("fail", "fail", "Fail"), vec![g.string("range"), g.module_const(), Ex::atom(e.span.line.to_string()), msg])));
                match t {
                    Target::TypeScript => {
                        self.emit(Node::Line(format!("const {n} = {};", g.kcall("isqrt", vec![xv.ex.clone()]).print(t))));
                        self.emit(Node::If(vec![(Ex::bin(Op::Eq, Ex::atom(n.clone()), g.null()), vec![fail])], Vec::new()));
                    }
                    Target::Python => {
                        self.emit(Node::Line(format!("{n} = {}", g.kcall("isqrt", vec![xv.ex.clone()]).print(t))));
                        self.emit(Node::If(vec![(Ex::atom(format!("{n} is None")), vec![fail])], Vec::new()));
                    }
                    Target::Go => {
                        let ok = self.temp();
                        self.emit(Node::Line(format!("{n}, {ok} := {}", g.kcall("Isqrt", vec![xv.ex.clone()]).print(t))));
                        self.emit(Node::If(vec![(Ex::Not(Box::new(Ex::atom(ok))), vec![fail])], Vec::new()));
                    }
                }
                Val::new(self.rep_to(Ex::atom(n), Rep::Int, r), me)
            }
            "allocate" => {
                let (tc, ts) = (self.ty(xs[1]), self.ty(xs[2]));
                let both = crate::interp::common(&tc, &ts).clone();
                let nb = both.num().cloned().unwrap_or_else(NumTy::number);
                let nb = NumTy { konst: None, step: None, ..nb };
                let n0 = vals[0].ty.num().cloned().unwrap_or_else(NumTy::number);
                let n1 = tc.num().cloned().unwrap_or_else(NumTy::number);
                let n2 = ts.num().cloned().unwrap_or_else(NumTy::number);
                let tq = self.rep_to(vals[0].ex.clone(), rep(&n0), Rep::Q);
                let c = self.convert(vals[1].ex.clone(), &n1, &nb, xs[1], n1.konst);
                let s = self.convert(vals[2].ex.clone(), &n2, &nb, xs[2], n2.konst);
                let over = |at: Option<At>| -> Ex {
                    match (at, t) {
                        (None, Target::Go) => Ex::atom("nil"),
                        (None, _) => g.null(),
                        (Some(at), Target::Go) => Ex::atom(format!("&{}", Ex::Record(format!("{}.At", g.imp(Import::Kaname)), vec![("Module".into(), g.module_const()), ("Line".into(), Ex::atom(at.line.to_string())), ("Message".into(), g.string(&at.message))]).print(t))),
                        (Some(at), _) => Ex::List(String::new(), g.at_args(&at)),
                    }
                };
                let v = g.kcall(&g.helper("allocate", "allocate", "Allocate"), vec![tq, c, s, over(g.overflow_at(e)), over(g.zero_at(e))]);
                // read in the unit of the answer
                let nm = me.num().cloned().unwrap_or_else(NumTy::number);
                let from = NumTy { step: None, konst: None, ..n0 };
                let v = self.convert(v, &from, &NumTy { step: None, ..nm.clone() }, e, None);
                Val::new(self.rep_to(v, Rep::Q, rep(&nm)), me)
            }
            "up" | "down" | "half_up" | "half_down" | "half_even" => {
                let tx = self.ty(xs[0]);
                let nx = tx.num().cloned().unwrap_or_else(NumTy::number);
                let ng = self.ty(xs[1]).num().cloned().unwrap_or_else(NumTy::number);
                // the step of the rounding, read in the unit of what is rounded
                let grid = ng.konst.and_then(|k| crate::interp::convert(k, &ng.unit, &nx.unit).ok()).unwrap_or(Rat::int(1));
                let x = self.rep_to(vals[0].ex.clone(), rep(&nx), Rep::Q);
                let f = g.helper("round", "round_to", "Round");
                let mut v = g.kcall(&f, vec![x, g.string(name), g.frac(grid)]);
                let check = g.overflow_at(e);
                if let Some(at) = &check {
                    let mut args = vec![v];
                    args.extend(g.at_args(at));
                    v = g.kcall(&g.helper("held", "held", "Held"), args);
                }
                let nm = me.num().cloned().unwrap_or_else(NumTy::number);
                let v = self.convert(v, &NumTy { step: None, konst: None, ..nx }, &NumTy { step: None, ..nm.clone() }, e, None);
                Val::new(self.rep_to(v, Rep::Q, rep(&nm)), me)
            }
            "count" => {
                let n = match t {
                    Target::TypeScript => Ex::call("BigInt", vec![vals[0].ex.clone().dot("length")]),
                    Target::Python => Ex::call("len", vec![vals[0].ex.clone()]),
                    Target::Go => Ex::call("int64", vec![Ex::call("len", vec![vals[0].ex.clone()])]),
                };
                Val::new(n, me)
            }
            "starts_with" => {
                let ex = match t {
                    Target::TypeScript => Ex::Call(Box::new(vals[0].ex.clone().dot("startsWith")), vec![vals[1].ex.clone()]),
                    Target::Python => Ex::Call(Box::new(vals[0].ex.clone().dot("startswith")), vec![vals[1].ex.clone()]),
                    Target::Go => g.kcall("StartsWith", vec![vals[0].ex.clone(), vals[1].ex.clone()]),
                };
                Val::new(ex, me)
            }
            "concat" => {
                let ex = vals.iter().map(|v| v.ex.clone()).reduce(|a, b| Ex::bin(Op::Add, a, b)).unwrap_or(g.string(""));
                Val::new(ex, me)
            }
            "text" => {
                let v = &vals[0];
                let ex = match &v.ty {
                    Ty::Num(n) => {
                        let x = self.rep_to(v.ex.clone(), rep(n), Rep::Q);
                        g.kcall(&g.helper("numText", "num_text", "NumText"), vec![x, g.string(&unit_suffix(&n.unit))])
                    }
                    Ty::Date => match t {
                        Target::TypeScript => v.ex.clone(),
                        Target::Python => g.kcall("date_text", vec![v.ex.clone()]),
                        Target::Go => Ex::Call(Box::new(v.ex.clone().dot("String")), vec![]),
                    },
                    Ty::Bool => match t {
                        Target::TypeScript => Ex::call("String", vec![v.ex.clone()]),
                        Target::Python => Ex::Cond(Box::new(v.ex.clone()), Box::new(g.string("true")), Box::new(g.string("false"))),
                        Target::Go => g.kcall("BoolText", vec![v.ex.clone()]),
                    },
                    Ty::Enum(id) => match (t, &id.owner) {
                        (Target::TypeScript, _) => v.ex.clone(),
                        (Target::Python, _) => v.ex.clone().dot("value"),
                        (Target::Go, Owner::Module(_)) => Ex::call("string", vec![v.ex.clone()]),
                        (Target::Go, Owner::Rule(_)) => Ex::Call(Box::new(v.ex.clone().dot("String")), vec![]),
                    },
                    _ => v.ex.clone(),
                };
                Val::new(ex, me)
            }
            _ => Val::new(g.null(), me),
        }
    }

    // ── The function itself ────────────────────────────────────────────────

    /// The body of the function, its parameters (as they travel) named by `params`.
    pub fn body(mut self, params: &[String]) -> Vec<Node> {
        let g = self.g;
        let info = g.c.decls.fns[self.f].clone();
        for (p, name) in info.params.iter().zip(params) {
            self.locals.taken.insert(name.clone());
            let v = self.from_boundary(Ex::atom(name.clone()), &p.slot.ty, &p.slot.ty);
            let v = if v.ex.is_simple() { v } else { self.spill(v) };
            self.scope.push((p.name.text.clone(), v, Rc::new(Cell::new(true))));
        }
        let decl = &g.c.module.fns[self.f];
        self.block(&decl.body, &Sink::Return);
        self.out
    }
}

/// The factor a number of `a` is read in `b` by (`interp.rs`'s `convert`): 1 between a number and a
/// rate, and where the units are one.
fn factor_between(a: &NumTy, b: &NumTy) -> Rat {
    let dimensionless = |u: &Unit| matches!(u.dim, Dim::Number | Dim::Rate);
    if dimensionless(&a.unit) {
        return Rat::int(1);
    }
    match crate::ty::factor(&a.unit, &b.unit) {
        Some(f) => f,
        None => Rat::int(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A rate travels as a count of its step: the declared one, else the step its type says, and
    /// never 0, which would make the count 0/0 (the weight `0%` of `running_totals`).
    #[test]
    fn a_rate_travels_on_a_step_that_is_not_zero() {
        let rate = |step: Option<Rat>| NumTy { unit: Unit::rate(None), step, konst: None, ratio: false };
        assert_eq!(rate_step(&rate(Some(Rat::new(1, 10_000)))), Rat::new(1, 10_000));
        assert_eq!(rate_step(&rate(Some(Rat::new(1, 30_000)))), Rat::new(1, 30_000));
        assert_eq!(rate_step(&rate(Some(Rat::zero()))), Rat::new(1, 100));
        assert_eq!(rate_step(&rate(None)), Rat::new(1, 100));
    }
}
