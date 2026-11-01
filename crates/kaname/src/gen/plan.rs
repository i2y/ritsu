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
//! as the target's integer (`bigint`, `int`, `int64`, `i64`), anything else (a rate, an unrounded
//! value) as an exact fraction (`kaname.Q`, `Fraction`, `*big.Rat`, `kaname::Q`). What a record holds, and what a
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
        Target::Rust => rust_type(&p),
        _ => p,
    }
}

/// The types of Rust's prelude and standard library the code of a module names by their names alone
/// (`Option`, `Vec`, `BTreeMap`, the traits of its bounds): a type of the module whose name is one of
/// them takes `_` after it in Rust (`record option` is `Option_`, DESIGN 36.3), as a word no raw
/// identifier writes does (`Self_`).
pub const RUST_STD_TYPES: [&str; 15] = ["Option", "Result", "Vec", "String", "Box", "BTreeMap", "BTreeSet", "Fn", "FnOnce", "Clone", "Ord", "PartialOrd", "PartialEq", "Eq", "From"];

/// The values of Rust's prelude the code of a module names by their names alone: a function of the
/// module whose name is one of them takes `_` after it in Rust (`fn Ok` is `Ok_`).
pub const RUST_STD_VALUES: [&str; 4] = ["Some", "None", "Ok", "Err"];

/// A type of a module in Rust, from its name in Pascal case: a raw identifier for a keyword, and `_`
/// after a word no raw identifier writes or a type of [`RUST_STD_TYPES`].
fn rust_type(p: &str) -> String {
    if RUST_STD_TYPES.contains(&p) { format!("{p}_") } else { ritsu_emit::ident::rust_name(p) }
}

/// A name of the module's code for what is written `private` (DESIGN 28.2): with `_` before it, which
/// Go does not export and Python takes as the module's own (TypeScript exports none of them either).
pub fn hidden(name: String, private: bool) -> String {
    if private { format!("_{name}") } else { name }
}

/// The type a value of a sum of a module is in the target (`Order.placed` is `OrderPlaced`, DESIGN
/// 23.8), from the aliases of the enum and the value.
pub fn variant_name(t: Target, enum_alias: &str, value_alias: &str) -> String {
    let n = crate::names::variant_type_name(enum_alias, value_alias);
    match t {
        Target::Go => ritsu_emit::ident::go_exported(&n),
        Target::Rust => rust_type(&n),
        _ => n,
    }
}

/// The member of a module's enum: `Tier.GOLD` (TypeScript, Python), `TierGold` (Go), `Tier::Gold`
/// (Rust).
pub fn member_name(t: Target, enum_alias: &str, value_alias: &str) -> String {
    match t {
        Target::Go => format!("{}{}", type_name(t, enum_alias), ritsu_emit::ident::go_exported(&ritsu_emit::ident::pascal(value_alias))),
        Target::Rust => ritsu_emit::ident::rust_name(&ritsu_emit::ident::pascal(value_alias)),
        _ => value_alias.to_uppercase(),
    }
}

/// The function a host calls: its alias (TypeScript, Python, Rust), in Pascal case in Go
/// (`total_weight` is `TotalWeight`). A Go function whose name a type of the module has (`fn claim`
/// and `record Claim` are both `Claim`) takes `Fn` after it; Rust's functions and types are apart,
/// and a Rust function takes `_` after it where its name is a value of the prelude ([`RUST_STD_VALUES`])
/// or a word no raw identifier writes.
pub fn fn_name(t: Target, c: &Checked, f: usize) -> String {
    let alias = c.decls.fns[f].name.ascii().to_string();
    match t {
        Target::Go => {
            let g = type_name(t, &alias);
            let mut types: BTreeSet<String> =
                c.decls.records.iter().map(|r| hidden(type_name(t, r.name.ascii()), r.private)).chain(c.decls.enums.iter().map(|e| hidden(type_name(t, e.name.ascii()), e.private))).collect();
            for e in c.decls.enums.iter().filter(|e| e.is_sum()) {
                types.extend(e.values.iter().map(|v| hidden(variant_name(t, e.name.ascii(), v.ascii()), e.private)));
            }
            if types.contains(&g) { format!("{g}Fn") } else { g }
        }
        Target::Rust if RUST_STD_VALUES.contains(&alias.as_str()) => format!("{alias}_"),
        Target::Rust => ritsu_emit::ident::rust_name(&alias),
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

/// Rust's check of what a host gives a function (DESIGN 36.3): `check_claim`, from the alias; with a
/// number after it where a function of the module has that name (`check_claim2`).
pub fn check_name(t: Target, c: &Checked, f: usize) -> String {
    let base = format!("check_{}", c.decls.fns[f].name.ascii());
    let taken: BTreeSet<String> = (0..c.decls.fns.len()).flat_map(|g| [fn_name(t, c, g), inner_name(t, c, g)]).collect();
    let mut name = base.clone();
    let mut i = 2;
    while taken.contains(&name) {
        name = format!("{base}{i}");
        i += 1;
    }
    name
}

/// The one function of a generic function (DESIGN 21.3, 26.7), which the generated code of every module
/// calls and no host: `_first_or` in TypeScript (exported) and Python, `FirstOr` in Go.
/// Whether Rust's function `f` of `c` answers a `Result` (DESIGN 36.4): it can fail, or it takes a
/// function whose type writes no effect, which may be given one that fails and goes on to the callers
/// (`through`, DESIGN 27.5).
/// Rust: the places of the type parameters of the enum `e` that the fields of its value `value` name
/// (DESIGN 36.2). The value's struct takes those alone: a struct of Rust uses every type parameter it
/// takes (`FoundMiss { why }` of `Found[T]`).
pub fn variant_tparams(e: &crate::names::EnumInfo, value: &str) -> Vec<usize> {
    let Some(fs) = e.values.iter().position(|v| v.text == value).and_then(|i| e.fields.get(i)) else { return Vec::new() };
    e.tparams.iter().enumerate().filter(|(_, p)| fs.iter().any(|f| names_param(&f.slot.ty, &p.name.text))).map(|(i, _)| i).collect()
}

/// Whether the type parameter `n` is in the type `t`.
fn names_param(t: &Ty, n: &str) -> bool {
    match t {
        Ty::Param(x) => x == n,
        Ty::List(x) | Ty::Set(x) | Ty::Opt(x) => names_param(x, n),
        Ty::Dict(a, b) | Ty::Result(a, b) => names_param(a, n) || names_param(b, n),
        Ty::Fn(ps, r, _) => ps.iter().any(|p| names_param(p, n)) || names_param(r, n),
        Ty::Enum(id) => id.args.iter().any(|a| names_param(a, n)),
        Ty::Record(r) => match r {
            RecId::Module { args, .. } | RecId::Variant { args, .. } => args.iter().any(|a| names_param(a, n)),
            RecId::Running(a, b) | RecId::Entry(a, b) => names_param(a, n) || names_param(b, n),
            RecId::Indexed(x) => names_param(x, n),
            _ => false,
        },
        _ => false,
    }
}

pub fn rs_fails(c: &Checked, f: usize) -> bool {
    let own = c.analysis.get().and_then(|a| a.fns.get(f)).is_some_and(|x| !x.effect.fails.is_empty());
    own || c.decls.fns.get(f).is_some_and(|info| !crate::effects::through(info).is_empty())
}

pub fn generic_name(t: Target, c: &Checked, f: usize) -> String {
    match t {
        // a `private` function is not exported (DESIGN 28.2)
        Target::Go if !c.decls.fns[f].private => fn_name(t, c, f),
        _ => inner_name(t, c, f),
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

/// `QAdd` → `q_add`: a helper's Go name as Rust's helper names it (DESIGN 36.7); a name in snake case
/// already is as it is.
pub fn snake(name: &str) -> String {
    let cs: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (i, &c) in cs.iter().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            let prev = cs[i - 1];
            let next_lower = cs.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
            if prev.is_ascii_lowercase() || prev.is_ascii_digit() || (prev.is_ascii_uppercase() && next_lower) {
                out.push('_');
            }
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// Whether an atom of Rust's code is a name a `format!` string can hold in its braces: a name that is
/// no raw identifier.
fn plain_ident(a: &str) -> bool {
    let mut cs = a.chars();
    matches!(cs.next(), Some(c) if c == '_' || c.is_alphabetic()) && cs.all(|c| c == '_' || c.is_alphanumeric()) && a != "_"
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
                // a keyword of Rust's, as a raw identifier (`r#ref`, DESIGN 36.3)
                if t == Target::Rust && ritsu_emit::words::rust::NAMES.contains(&name) {
                    name = format!("r#{name}");
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
        Target::Rust => ritsu_emit::ident::rust_name(alias),
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
        // a keyword of Rust is a raw identifier (`r#ref`) but for these; and the values of the
        // prelude the code names bare (DESIGN 36.3): a path is not hidden by a local
        Target::Rust => ritsu_emit::ident::RUST_UNRAW.contains(&w) || ["Some", "None", "Ok", "Err", "std", "core", "alloc"].contains(&w),
    }
}

/// The words a type parameter may not be named in the target besides those of [`kept`]: the types the
/// code names with no import (TypeScript's `ReadonlyMap`, Go's `any`), and Python's names of `typing`
/// and the module's own, which a type variable of the module would hide.
fn type_kept(t: Target, w: &str) -> bool {
    kept(t, w)
        || match t {
            Target::TypeScript => ["ReadonlyMap", "ReadonlySet", "Readonly", "Record", "Partial", "Boolean", "Date", "Promise", "Iterable"].contains(&w),
            Target::Python => [
                "Any", "Callable", "Generic", "TypeVar", "Union", "Literal", "NoReturn", "AbstractSet", "Mapping", "Sequence", "Iterable", "dict", "set", "frozenset", "tuple", "dataclass", "dataclasses", "enum", "_MODULE", "_dt",
            ]
            .contains(&w),
            Target::Go => ["any", "comparable", "int64", "int", "string", "bool", "error", "map"].contains(&w),
            Target::Rust => [
                "Option", "Result", "Vec", "String", "Box", "BTreeMap", "BTreeSet", "Fn", "Clone", "Ord", "PartialEq", "PartialOrd", "Eq", "From", "Debug", "Copy", "i64", "i32", "u32", "usize",
                "bool", "str", "char", "Self_",
            ]
            .contains(&w),
        }
}

/// Whether a name of the source is a name the target takes as it is: its characters are ones the
/// target's names take (DESIGN 36.3; the tables of ritsu-emit, from the Unicode Character
/// Database), and `_` alone is no name of Go's or Rust's. A name of the source is a letter or `_`,
/// then letters, numbers and `_`; `m²` is no name of any target's, nor `कीमत` of Go's.
pub fn ident_ok(t: Target, w: &str) -> bool {
    let takes = match t {
        Target::TypeScript => ritsu_emit::ident::typescript_takes(w),
        Target::Python => ritsu_emit::ident::python_takes(w),
        Target::Go => ritsu_emit::ident::go_takes(w),
        Target::Rust => ritsu_emit::ident::rust_takes(w),
    };
    takes && !(matches!(t, Target::Go | Target::Rust) && w == "_")
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
        // a keyword of Rust's, as a raw identifier (DESIGN 36.3)
        let base = if t == Target::Rust && ritsu_emit::words::rust::NAMES.contains(&base) && !ritsu_emit::ident::RUST_UNRAW.contains(&base.as_str()) { format!("r#{base}") } else { base };
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

/// How Rust's code holds a value (DESIGN 36.4): made where it is written (the answer of a call, a
/// literal), in a place the function owns (a `let`, a field of one), lent as the type a function
/// takes it as (a parameter: `&str`, `&[T]`, `Option<&T>`, `&T`), or reached through a reference to
/// where it is owned (`&T`: an element of a list it walks, a value of a dict, what a `match` binds).
/// The other targets hold every value alike; a value of a type that is `Copy` in Rust is never
/// lent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    Made,
    Kept,
    Lent,
    Ref,
}

/// How Rust's field of a record or of a value's type holds a value of a type that holds the record
/// (DESIGN 36.2): not boxed, in a box (`Box<Expr>`), in a box where it is there (`Option<Box<Node>>`),
/// in a box on the side of a result that holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boxed {
    No,
    Whole,
    InOpt,
    InResult(bool, bool),
}

/// A value as the plan holds it: the expression of the target, and its type (the unit it counts in).
#[derive(Clone, Debug)]
pub struct Val {
    pub ex: Ex,
    pub ty: Ty,
    /// A value of an optional type that is there: one given where one may be absent, held as it is
    /// (TypeScript and Python), so that it travels as the value it is.
    pub present: bool,
    pub held: Held,
}

impl Val {
    fn new(ex: Ex, ty: Ty) -> Val {
        Val { ex, ty, present: false, held: Held::Made }
    }

    fn held(ex: Ex, ty: Ty, held: Held) -> Val {
        Val { ex, ty, present: false, held }
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

/// The messages of a place a value does not keep (a value outside its range, a list outside its
/// count, a string outside its count of code points, a string that is not well formed), with the parts
/// the code fills in (DESIGN 8, `interp.rs`).
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

/// The messages of [`Said`], by what is outside.
struct Texts {
    count: Text,
    value: Text,
    chars: Text,
    bad: Text,
}

impl Said {
    fn texts(&self) -> Texts {
        let (count, value) = self.list_texts();
        let (chars, bad) = match self {
            Said::Entry { fun } => (
                tr!("関数 `{fun}` の引数 `{P}` のコードポイントは {N} 個で、コードポイントの数の範囲 {R} の外です", "The argument `{P}` of `{fun}` has {N} code points, outside its count {R}"),
                tr!("関数 `{fun}` の引数 `{P}` は、Unicode の文字列として正しくありません", "The argument `{P}` of `{fun}` is no well-formed string of Unicode"),
            ),
            Said::Given { fun } => (
                tr!("関数 `{fun}` に渡す引数 `{P}` のコードポイントは {N} 個で、コードポイントの数の範囲 {R} の外です", "The argument `{P}` given to `{fun}` has {N} code points, outside its count {R}"),
                tr!("関数 `{fun}` に渡す引数 `{P}` は、Unicode の文字列として正しくありません", "The argument `{P}` given to `{fun}` is no well-formed string of Unicode"),
            ),
            Said::Answer { fun } => (
                tr!("関数 `{fun}` の答えのコードポイントは {N} 個で、コードポイントの数の範囲 {R} の外です", "The answer of `{fun}` has {N} code points, outside its count {R}"),
                Text::same(String::new()),
            ),
            Said::Field { rec } => (
                tr!("{rec} のフィールド `{P}` に置くコードポイントは {N} 個で、コードポイントの数の範囲 {R} の外です", "The field `{P}` of {rec} is given {N} code points, outside its count {R}"),
                Text::same(String::new()),
            ),
        };
        Texts { count, value, chars, bad }
    }

    fn list_texts(&self) -> (Text, Text) {
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

/// How a check of a value holds it to what its place declares (DESIGN 4.4, 28.1), as `interp.rs`'s
/// `Hold` does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reach {
    /// Where a value is put: a field of a record built, a function's answer.
    Place,
    /// Where a function of kaname calls another: what the parameter reaches, short of the values of
    /// types that hold themselves.
    Call,
    /// Where a host calls a function: what the parameter reaches, at every depth.
    Entry,
}

impl Reach {
    /// Whether what a parameter reaches is held: the fields of its records, a rate with no range 0%
    /// to 100%, a number or a date whose range is not written to the whole of its type.
    pub fn reached(self) -> bool {
        self != Reach::Place
    }
}

/// A check of what a place declares, as a function of the module.
#[derive(Clone)]
pub struct Outside {
    /// The slot it checks (its type, range, count and whether a parameter reaches it), as a key.
    pub key: String,
    pub name: String,
    pub body: Vec<Node>,
    /// The type of the value it takes, as that travels.
    pub ty: String,
    /// The type parameters it declares, where that type holds one (`<T>`, `[T any]`).
    pub tparams: String,
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
    /// The functions that find a host's argument nested too deep (DESIGN 28.1), each by the type it
    /// takes, as the checks are.
    pub nests: RefCell<Vec<Outside>>,
    /// The enums the code compares, and the name of the function that gives a value's place.
    pub ranks: RefCell<Vec<(EnumId, String)>>,
    /// Go's functions that tell whether two values of one type are the same (DESIGN 23.8): each its
    /// key (the type, and whether the values travel), its name, and its definition.
    pub eqs: RefCell<Vec<(String, String, String)>>,
    /// The orders of the records the bodies order by (a key of `sort_by`, DESIGN 24.6): each its key,
    /// its name, and its definition (TypeScript's and Go's compare two, Python's gives the key).
    pub cmps: RefCell<Vec<(String, String, String)>>,
    /// The names the module's code uses outside its functions' bodies, which a local does not take.
    pub outer: BTreeSet<String>,
    /// The type parameters of the declaration written now (DESIGN 26.7).
    pub tscope: RefCell<Vec<TScope>>,
    /// Rust: the parameters of the function written last that its body never reads, which its
    /// signature names `_` (DESIGN 36.4).
    pub unread: RefCell<Vec<String>>,
}

/// A type parameter of the declaration written now, as the code names it (DESIGN 26.7).
#[derive(Clone, Debug)]
pub struct TScope {
    /// Its name in the source (`Ty::Param`).
    pub name: String,
    /// Its name in the target's code.
    pub ty: String,
    /// Whether it is held to keys (`K: key`).
    pub key: bool,
    /// Of a function's type parameter held to keys, the order it is given, a hidden argument before
    /// the others: TypeScript's and Go's comparison, Python's function of the key.
    pub order: Option<String>,
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
            if t == Target::Rust {
                outer.insert(check_name(t, c, f));
            }
        }
        for r in &c.decls.records {
            outer.insert(hidden(type_name(t, r.name.ascii()), r.private));
            outer.insert(format!("_outside_{}", hidden(type_name(t, r.name.ascii()), r.private)));
            outer.insert(format!("_enter_{}", hidden(type_name(t, r.name.ascii()), r.private)));
        }
        for e in &c.decls.enums {
            outer.insert(hidden(type_name(t, e.name.ascii()), e.private));
            if e.is_sum() {
                // the types of its values, and the check of their fields
                for v in &e.values {
                    outer.insert(hidden(variant_name(t, e.name.ascii(), v.ascii()), e.private));
                }
                outer.insert(format!("_outside_{}", hidden(type_name(t, e.name.ascii()), e.private)));
                outer.insert(format!("_enter_{}", hidden(type_name(t, e.name.ascii()), e.private)));
            } else if t == Target::Go {
                for v in &e.values {
                    outer.insert(hidden(member_name(t, e.name.ascii(), v.ascii()), e.private));
                }
            }
        }
        // Rust reads `kaname::…` and `units::…` as paths, which no local hides (DESIGN 36.3)
        let words: &[&str] = if t == Target::Rust { &["_MODULE"] } else { &["_module", "_dt", "_units", "kaname", "units"] };
        for w in words {
            outer.insert(w.to_string());
        }
        let g = Gen { t, lang: opts.lang, p, c: c.clone(), opts, places, used: RefCell::default(), units: RefCell::default(), outsides: RefCell::default(), nests: RefCell::default(), ranks: RefCell::default(), eqs: RefCell::default(), cmps: RefCell::default(), outer, tscope: RefCell::default(), unread: RefCell::default() };
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

    /// The name an import is read by: for a file of a dependency's, with the names of the
    /// dependencies on the way to it before its own (`rule_pricing_fee`, ritsu's DESIGN 17.8), so
    /// that the files of one name of two modules are two names.
    pub fn import_name(&self, i: &Import) -> String {
        let t = self.t;
        let of = |f: &std::path::Path, name: String| ritsu_emit::package::local_name(&self.opts.dir_of(f), &name);
        match i {
            Import::Kaname => "kaname".into(),
            Import::Units => if t == Target::Python { "_units" } else { "units" }.into(),
            Import::Datetime => "_dt".into(),
            Import::Rule(f) => {
                let alias = of(f, self.p.rules.get(f).map(|r| r.alias.clone()).unwrap_or_default());
                match t {
                    Target::TypeScript | Target::Rust => format!("rule_{alias}"),
                    Target::Python => format!("_rule_{alias}"),
                    Target::Go => go_package(&alias),
                }
            }
            Import::Dates(f) => {
                let alias = of(f, self.p.dates.get(f).map(|d| d.alias.clone()).unwrap_or_default());
                match t {
                    Target::TypeScript | Target::Rust => format!("dates_{alias}"),
                    Target::Python => format!("_dates_{alias}"),
                    Target::Go => go_package(&alias),
                }
            }
            Import::Module(f) => {
                let alias = of(f, self.p.module(f).module.name.ascii().to_string());
                match t {
                    Target::TypeScript | Target::Rust => format!("mod_{alias}"),
                    Target::Python => format!("_mod_{alias}"),
                    Target::Go => go_package(&alias),
                }
            }
            Import::Book(f) => {
                let b = of(f, self.p.books.get(f).map(|b| b.name.clone()).unwrap_or_default());
                match t {
                    Target::TypeScript | Target::Rust => format!("books_{b}"),
                    Target::Python => format!("_books_{b}"),
                    Target::Go => {
                        let p = super::books::go_package_of(&self.p, f);
                        let n = of(f, p.clone());
                        if n == p { p } else { go_package(&n) }
                    }
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
            // Rust calls the parentheses of `f((-1))` unused: a negative number is a negation, which
            // the printer puts in parentheses where it must
            Target::Rust if v < 0 => return Ex::Neg(Box::new(Ex::Atom(v.unsigned_abs().to_string()))),
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
            Target::Rust => Ex::call(format!("{k}::frac"), vec![self.int(v.num), Ex::atom(v.den.to_string())]),
        }
    }

    pub fn string(&self, s: &str) -> Ex {
        Ex::Atom(match self.t {
            Target::Go => ritsu_emit::lit::go(s),
            Target::Rust => ritsu_emit::lit::rust(s),
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
            Target::Python | Target::Rust => "None",
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
            Target::Rust => {
                let (y, m, dd) = crate::date::civil(d);
                let k = self.imp(Import::Kaname);
                Ex::Record(format!("{k}::Date"), vec![("year".into(), Ex::atom(y.to_string())), ("month".into(), Ex::atom(m.to_string())), ("day".into(), Ex::atom(dd.to_string()))])
            }
        }
    }

    /// The module's name, as the code passes it to an error.
    pub fn module_const(&self) -> Ex {
        Ex::atom(if matches!(self.t, Target::Python | Target::Rust) { "_MODULE" } else { "_module" })
    }

    /// The three arguments of where an operation fails: the module, the line, the message.
    pub fn at_args(&self, at: &At) -> Vec<Ex> {
        vec![self.module_const(), Ex::atom(at.line.to_string()), self.string(&at.message)]
    }

    /// Rust: the answer of a rule's or a date's code (`raw`, a `Result`): where the analysis does not
    /// show the call keeps what it takes (`open`), its refusal is the failure `kind` at `line` with the
    /// message `said` and the refusal as its cause; else it is not reached (DESIGN 36.4).
    pub fn answered_rs(&self, raw: Ex, open: bool, kind: &str, line: usize, said: &str) -> Ex {
        if open {
            let refused = self.kcall("Refused", vec![self.string(kind), self.module_const(), Ex::atom(line.to_string()), self.string(said), Ex::atom("e")]);
            raw.method("map_err", vec![Ex::atom(format!("|e| {}", refused.print(Target::Rust)))]).tried()
        } else {
            raw.method("unwrap_or_else", vec![Ex::atom("|e| panic!(\"{e}\")")])
        }
    }

    /// The call of the helper that holds a fraction to int64 (`held`), Rust's with `?`.
    pub fn held_call(&self, args: Vec<Ex>) -> Ex {
        let call = self.kcall(&self.helper("held", "held", "Held"), args);
        if self.t == Target::Rust { call.tried() } else { call }
    }

    pub fn kcall(&self, f: &str, args: Vec<Ex>) -> Ex {
        let k = self.imp(Import::Kaname);
        match self.t {
            Target::Rust => Ex::call(format!("{k}::{}", ritsu_emit::ident::rust_name(&snake(f))), args),
            _ => Ex::call(format!("{k}.{f}"), args),
        }
    }

    /// The name of a helper of `kaname` in this target: `heldInt`, `held_int`, `Add`, and Rust's the
    /// Go's in snake case (`add`, DESIGN 36.7).
    fn helper(&self, ts: &str, py: &str, go: &str) -> String {
        match self.t {
            Target::TypeScript => ts.to_string(),
            Target::Python => py.to_string(),
            Target::Go => go.to_string(),
            Target::Rust => snake(go),
        }
    }

    /// A member of a module of the target, or of a type: `kaname.Q`, Rust's `kaname::Q`.
    pub fn path(&self, base: &str, member: &str) -> String {
        match self.t {
            Target::Rust => format!("{base}::{member}"),
            _ => format!("{base}.{member}"),
        }
    }

    /// The type of Rust's standard library a module's code names, by the path that reaches it where
    /// a name of the module's code is its name: a type of the module is not ([`RUST_STD_TYPES`]), but
    /// a function may be (`fn Vec`), which is a value and hides no type, and a path costs nothing.
    pub fn std_type(&self, name: &str) -> String {
        if !self.outer.contains(name) {
            return name.to_string();
        }
        match name {
            "Option" => "::std::option::Option".into(),
            "Result" => "::std::result::Result".into(),
            "Vec" => "::std::vec::Vec".into(),
            "String" => "::std::string::String".into(),
            "Box" => "::std::boxed::Box".into(),
            "BTreeMap" | "BTreeSet" => format!("::std::collections::{name}"),
            "Fn" | "FnOnce" => format!("::std::ops::{name}"),
            "Clone" => "::std::clone::Clone".into(),
            "Ord" | "PartialEq" | "PartialOrd" | "Eq" => format!("::std::cmp::{name}"),
            "From" => "::std::convert::From".into(),
            n => n.to_string(),
        }
    }

    // ── How Rust holds a value (DESIGN 36.4) ────────────────────────────────

    /// Whether a value of `t` is `Copy` in Rust: given by value, never lent, never cloned.
    pub fn is_copy(&self, t: &Ty) -> bool {
        match t {
            Ty::Num(_) | Ty::Bool | Ty::Date | Ty::Fn(..) | Ty::None | Ty::Error => true,
            Ty::Enum(id) => !self.p.is_sum(id),
            Ty::Opt(x) => self.is_copy(x),
            Ty::Result(a, e) => self.is_copy(a) && self.is_copy(e),
            _ => false,
        }
    }

    /// Rust: whether a value of `t` has the order its type's `Ord` gives (DESIGN 36.2), the order of
    /// keys (`interp.rs`'s `key_cmp`): a module's record and enum derive it where the types of all
    /// their fields have it; an operation of a transfer, a function, and a rule's element and outputs
    /// (rulec's structs) have none.
    pub fn orderable(&self, t: &Ty) -> bool {
        self.orderable_in(t, &mut Vec::new())
    }

    fn orderable_in(&self, t: &Ty, seen: &mut Vec<Ty>) -> bool {
        match t {
            Ty::Transfer(_) | Ty::Fn(..) => false,
            Ty::Record(RecId::Element(..)) => false,
            Ty::Record(RecId::Outputs(f)) => matches!(self.p.rules.get(f), Some(facts) if facts.outputs.len() == 1),
            Ty::Opt(x) | Ty::List(x) | Ty::Set(x) => self.orderable_in(x, seen),
            Ty::Dict(a, b) | Ty::Result(a, b) => self.orderable_in(a, seen) && self.orderable_in(b, seen),
            Ty::Record(RecId::Running(a, b) | RecId::Entry(a, b)) => self.orderable_in(a, seen) && self.orderable_in(b, seen),
            Ty::Record(RecId::Indexed(x)) => self.orderable_in(x, seen),
            Ty::Record(id @ (RecId::Module { .. } | RecId::Variant { .. })) => {
                if seen.contains(t) {
                    return true;
                }
                seen.push(t.clone());
                self.p.fields(id).iter().all(|(_, s)| self.orderable_in(&s.ty, seen))
            }
            Ty::Enum(id) if self.p.is_sum(id) => {
                if seen.contains(t) {
                    return true;
                }
                seen.push(t.clone());
                let Owner::Module(file) = &id.owner else { return true };
                let values: Vec<String> = self.p.values(id).0.iter().map(|v| v.text.clone()).collect();
                values.into_iter().all(|v| {
                    let r = RecId::Variant { file: file.clone(), name: id.name.clone(), value: v, args: id.args.clone() };
                    self.p.fields(&r).iter().all(|(_, s)| self.orderable_in(&s.ty, seen))
                })
            }
            _ => true,
        }
    }

    /// The derives of a record or an enum of the module whose values are of `t` (DESIGN 36.2):
    /// `PartialOrd` and `Ord` where [`Gen::orderable`] says so.
    pub fn derives(&self, t: &Ty) -> &'static str {
        if self.orderable(t) { "#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]" } else { "#[derive(Clone, Debug, PartialEq, Eq)]" }
    }

    /// The type a value of `t` is lent as (DESIGN 36.4): `&str`, `&[T]`, `Option<&T>`, `&T`; `t` itself
    /// where it is `Copy`. `boundary` says whether it is the value as it travels or as it is worked on.
    pub fn lent_type(&self, t: &Ty, boundary: bool) -> String {
        if self.is_copy(t) {
            return if boundary { self.boundary(t) } else { self.flight(t) };
        }
        let of = |x: &Ty| if boundary { self.boundary(x) } else { self.flight(x) };
        match t {
            Ty::Str => "&str".into(),
            Ty::List(x) => format!("&[{}]", of(x)),
            Ty::Opt(x) => format!("{}<{}>", self.std_type("Option"), self.lent_type(x, boundary)),
            t => format!("&{}", of(t)),
        }
    }

    /// The type a function takes a parameter of `t` as: by value where it is `Copy`, lent otherwise.
    pub fn param_rust(&self, t: &Ty) -> String {
        self.lent_type(t, true)
    }

    /// A value as Rust's code holds it where it is put (a field, an element, an answer, a `let`): as
    /// it is where it is made there or is `Copy`, a copy of a place or of what is lent (DESIGN 36.4).
    pub fn owned(&self, v: &Val) -> Ex {
        if self.t != Target::Rust || self.is_copy(&v.ty) || v.held == Held::Made || self.absent(&v.ex) {
            return v.ex.clone();
        }
        // `Some` of what is lent, made here: `Some` of its copy (`Some("x".to_string())`)
        if let (Held::Lent, Ty::Opt(x), Ex::Call(f, args)) = (v.held, &v.ty, &v.ex)
            && matches!(&**f, Ex::Atom(n) if n == "Some")
            && args.len() == 1
        {
            return Ex::call("Some", vec![self.owned(&Val::held(args[0].clone(), (**x).clone(), Held::Lent))]);
        }
        match (v.held, &v.ty) {
            (Held::Lent, Ty::Str) => v.ex.clone().method("to_string", vec![]),
            (Held::Lent, Ty::List(_)) => v.ex.clone().method("to_vec", vec![]),
            (Held::Lent, Ty::Opt(x)) => match &**x {
                Ty::Str => v.ex.clone().method("map", vec![Ex::atom("str::to_string")]),
                Ty::List(_) => v.ex.clone().method("map", vec![Ex::atom("<[_]>::to_vec")]),
                _ => v.ex.clone().method("cloned", vec![]),
            },
            _ => v.ex.clone().method("clone", vec![]),
        }
    }

    /// A value as Rust's code lends it (a parameter, a helper's argument): as it is where it is lent
    /// or `Copy`, borrowed otherwise (`x.as_str()`, `&x`, `x.as_deref()`, `x.as_ref()`).
    pub fn lent(&self, v: &Val) -> Ex {
        if self.t != Target::Rust || self.is_copy(&v.ty) || v.held == Held::Lent || self.absent(&v.ex) {
            return v.ex.clone();
        }
        match &v.ty {
            Ty::Str => v.ex.clone().method("as_str", vec![]),
            Ty::Opt(x) => match &**x {
                Ty::Str | Ty::List(_) => v.ex.clone().method("as_deref", vec![]),
                _ => v.ex.clone().method("as_ref", vec![]),
            },
            Ty::List(_) if v.held == Held::Ref => v.ex.clone().method("as_slice", vec![]),
            // a reference already
            _ if v.held == Held::Ref => v.ex.clone(),
            _ => v.ex.clone().borrow(),
        }
    }

    /// A value lent where a function takes it as a parameter declared `decl` (DESIGN 36.4): as
    /// [`Gen::lent`] does, but where `decl` is a type parameter (`&T`) or a type parameter that may be
    /// absent (`Option<&T>`), as a reference to the whole value the type argument names: `&x` of a
    /// place or of what is `Copy`, a reference as it is, a copy of what is lent (`&x.to_string()`
    /// for a `&str` where `T` is `String`).
    pub fn lent_as(&self, v: &Val, decl: &Ty) -> Ex {
        if self.t != Target::Rust || self.absent(&v.ex) {
            return self.lent(v);
        }
        match decl {
            Ty::Param(_) => match v.held {
                Held::Ref if !self.is_copy(&v.ty) => v.ex.clone(),
                Held::Lent if !self.is_copy(&v.ty) => self.owned(v).borrow(),
                _ => v.ex.clone().borrow(),
            },
            Ty::Opt(x) if matches!(**x, Ty::Param(_)) => match (&v.ty, v.held) {
                (Ty::Opt(inner), Held::Lent) if !self.is_copy(inner) => self.owned(v).method("as_ref", vec![]),
                _ => v.ex.clone().method("as_ref", vec![]),
            },
            _ => self.lent(v),
        }
    }

    /// A value as Rust's code compares it (`==`, `<`): what is lent as a borrowed string or as what
    /// may be absent, lent (`a.as_str() < b`, `x.as_deref() == y`); anything else as the place it is
    /// in (`*r == x`), so that neither side is borrowed only to be compared (DESIGN 36.4).
    pub fn cmp_side(&self, v: &Val) -> Ex {
        if self.t != Target::Rust || self.is_copy(&v.ty) || self.absent(&v.ex) {
            return v.ex.clone();
        }
        match (&v.ty, v.held) {
            (Ty::Str | Ty::Opt(_), _) => self.lent(v),
            (_, Held::Lent | Held::Ref) => Ex::Deref(Box::new(v.ex.clone())),
            _ => v.ex.clone(),
        }
    }

    /// A value Rust's code reaches through a reference `r` to it (an element of a list walked, a value
    /// of a dict, what a `match` binds), as the plan holds a value: the value where it is `Copy`
    /// (`*r`), else the place it is in.
    pub fn through(&self, r: Ex, t: &Ty) -> Val {
        if self.is_copy(t) {
            return Val::held(Ex::Deref(Box::new(r)), t.clone(), Held::Made);
        }
        // a reference to the place (`r.clone()`, `r.as_str()`, `r`)
        Val::held(r, t.clone(), Held::Ref)
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
                self.path(&self.imp(Import::Units), &name)
            }
            None => self.int_type().into(),
        }
    }

    fn int_type(&self) -> &'static str {
        match self.t {
            Target::TypeScript => "bigint",
            Target::Python => "int",
            Target::Go => "int64",
            Target::Rust => "i64",
        }
    }

    fn q_type(&self) -> String {
        self.path(&self.imp(Import::Kaname), "Q")
    }

    fn date_type(&self) -> String {
        match self.t {
            Target::TypeScript => "string".into(),
            Target::Python => format!("{}.date", self.imp(Import::Datetime)),
            Target::Go => format!("{}.Date", self.imp(Import::Kaname)),
            Target::Rust => format!("{}::Date", self.imp(Import::Kaname)),
        }
    }

    fn list_type(&self, inner: String) -> String {
        match self.t {
            Target::TypeScript => {
                if inner.contains(' ') { format!("readonly ({inner})[]") } else { format!("readonly {inner}[]") }
            }
            Target::Python => format!("list[{inner}]"),
            Target::Go => format!("[]{inner}"),
            Target::Rust => format!("{}<{inner}>", self.std_type("Vec")),
        }
    }

    /// A dict of the target: TypeScript's `ReadonlyMap`, Python's `Mapping` (a `dict` is made), Go's
    /// map.
    pub fn dict_type(&self, k: String, v: String) -> String {
        match self.t {
            Target::TypeScript => format!("ReadonlyMap<{k}, {v}>"),
            Target::Python => format!("Mapping[{k}, {v}]"),
            Target::Go => format!("map[{k}]{v}"),
            Target::Rust => format!("{}<{k}, {v}>", self.std_type("BTreeMap")),
        }
    }

    /// A set of the target: TypeScript's `ReadonlySet`, Python's `AbstractSet` (a `frozenset` is
    /// made), Go's map to `struct{}`.
    pub fn set_type(&self, x: String) -> String {
        match self.t {
            Target::TypeScript => format!("ReadonlySet<{x}>"),
            Target::Python => format!("AbstractSet[{x}]"),
            Target::Go => format!("map[{x}]struct{{}}"),
            Target::Rust => format!("{}<{x}>", self.std_type("BTreeSet")),
        }
    }

    fn opt_type(&self, inner: String) -> String {
        match self.t {
            Target::TypeScript => format!("{inner} | null"),
            Target::Python => format!("{inner} | None"),
            Target::Go => format!("*{inner}"),
            Target::Rust => format!("{}<{inner}>", self.std_type("Option")),
        }
    }

    /// A rule's enum or a module's, as the target names it.
    pub fn enum_type(&self, id: &EnumId) -> String {
        let args = if self.p.is_sum(id) { self.targs_text(&id.args) } else { String::new() };
        match &id.owner {
            Owner::Module(f) => {
                let m = self.p.module(f);
                let (alias, private) = m.decls.enums.iter().find(|e| e.name.text == id.name).map(|e| (e.name.ascii().to_string(), e.private)).unwrap_or_default();
                let n = hidden(type_name(self.t, &alias), private);
                let n = if *f == self.c.canon { n } else { self.path(&self.imp(Import::Module(f.clone())), &n) };
                format!("{n}{args}")
            }
            Owner::Rule(f) => {
                let alias = self.rule_enum(f, &id.name).map(|e| e.alias.clone()).unwrap_or_default();
                self.path(&self.imp(Import::Rule(f.clone())), &alias)
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
            Target::Rust => &facts.rust,
        }
    }

    /// A value of an enum: `Tier.GOLD`, `rule_x.Urgency.OVERDUE`, `TierGold`, `x.UrgencyOverdue`; a value
    /// of a sum with no fields as its type builds it: `{ kind: "cancelled" }`, `OrderCancelled()`,
    /// `OrderCancelled{}` (DESIGN 23.8).
    pub fn member(&self, id: &EnumId, value: &str) -> Ex {
        if self.p.is_sum(id)
            && let Owner::Module(file) = &id.owner
        {
            let ty = self.record_type(&RecId::Variant { file: file.clone(), name: id.name.clone(), value: value.to_string(), args: id.args.clone() });
            return match self.t {
                Target::TypeScript => Ex::Record(String::new(), vec![("kind".into(), self.string(value))]),
                // the value of the enum that holds it: `Order::Cancelled(OrderCancelled {})`
                Target::Rust => Ex::call(self.variant_of(id, value), vec![Ex::Record(ty, Vec::new())]),
                _ => Ex::Record(ty, Vec::new()),
            };
        }
        match &id.owner {
            Owner::Module(f) => {
                let m = self.p.module(f);
                let Some(e) = m.decls.enums.iter().find(|e| e.name.text == id.name) else { return self.string(value) };
                let v = e.values.iter().find(|v| v.text == value).map(|v| v.ascii().to_string()).unwrap_or_else(|| value.to_string());
                let ea = e.name.ascii().to_string();
                let local = f == &self.c.canon;
                match self.t {
                    Target::Go => {
                        let n = hidden(member_name(self.t, &ea, &v), e.private);
                        if local { Ex::atom(n) } else { Ex::atom(format!("{}.{n}", self.imp(Import::Module(f.clone())))) }
                    }
                    _ => {
                        let ty = hidden(type_name(self.t, &ea), e.private);
                        let ty = if local { ty } else { self.path(&self.imp(Import::Module(f.clone())), &ty) };
                        Ex::atom(self.path(&ty, &member_name(self.t, &ea, &v)))
                    }
                }
            }
            Owner::Rule(f) => {
                let Some(e) = self.rule_enum(f, &id.name) else { return self.string(value) };
                let member = e.values.iter().find(|(n, _)| n == value).map(|(_, m)| m.clone()).unwrap_or_else(|| value.to_string());
                let r = self.imp(Import::Rule(f.clone()));
                match self.t {
                    Target::Go => Ex::atom(format!("{r}.{member}")),
                    Target::Rust => Ex::atom(format!("{r}::{}::{member}", e.alias)),
                    _ => Ex::atom(format!("{r}.{}.{member}", e.alias)),
                }
            }
        }
    }

    /// Rust's variant of a sum that holds the value `value` (DESIGN 36.2): `Order::Placed`, the enum's
    /// type with the value's alias in Pascal case.
    pub fn variant_of(&self, id: &EnumId, value: &str) -> String {
        let Owner::Module(f) = &id.owner else { return String::new() };
        let m = self.p.module(f);
        let Some(e) = m.decls.enums.iter().find(|e| e.name.text == id.name) else { return String::new() };
        let v = e.values.iter().find(|v| v.text == value).map(|v| v.ascii().to_string()).unwrap_or_else(|| value.to_string());
        let ty = hidden(type_name(self.t, e.name.ascii()), e.private);
        let ty = if *f == self.c.canon { ty } else { self.path(&self.imp(Import::Module(f.clone())), &ty) };
        format!("{ty}::{}", member_name(self.t, e.name.ascii(), &v))
    }

    /// The values of an enum, in order, by their names.
    pub fn enum_values(&self, id: &EnumId) -> Vec<String> {
        self.p.values(id).0.iter().map(|n| n.text.clone()).collect()
    }

    /// A record of a module, of a rule, or of `running_totals`, as the target names it: a generic one
    /// with its type arguments (`Page<Invoice>`, `Page[Invoice]`).
    pub fn record_type(&self, id: &RecId) -> String {
        match id {
            // Rust: a value's struct takes the type parameters its fields name, and no other
            RecId::Variant { file, name, value, args } if self.t == Target::Rust => {
                let m = self.p.module(file);
                let used = m.decls.enums.iter().find(|e| e.name.text == *name).map(|e| variant_tparams(e, value)).unwrap_or_default();
                let args: Vec<Ty> = used.iter().filter_map(|i| args.get(*i).cloned()).collect();
                format!("{}{}", self.record_type_bare(id), self.targs_text(&args))
            }
            RecId::Module { args, .. } | RecId::Variant { args, .. } => format!("{}{}", self.record_type_bare(id), self.targs_text(args)),
            _ => self.record_type_bare(id),
        }
    }

    /// The same with no type arguments: the class Python's `isinstance` takes.
    pub fn record_type_bare(&self, id: &RecId) -> String {
        match id {
            RecId::Module { file, name, .. } => {
                let m = self.p.module(file);
                let (alias, private) = m.decls.records.iter().find(|r| r.name.text == *name).map(|r| (r.name.ascii().to_string(), r.private)).unwrap_or_default();
                let n = hidden(type_name(self.t, &alias), private);
                if *file == self.c.canon { n } else { self.path(&self.imp(Import::Module(file.clone())), &n) }
            }
            RecId::Variant { file, name, value, .. } => {
                let m = self.p.module(file);
                let Some(e) = m.decls.enums.iter().find(|e| e.name.text == *name) else { return String::new() };
                let va = e.values.iter().find(|v| v.text == *value).map(|v| v.ascii().to_string()).unwrap_or_default();
                let n = hidden(variant_name(self.t, e.name.ascii(), &va), e.private);
                if *file == self.c.canon { n } else { self.path(&self.imp(Import::Module(file.clone())), &n) }
            }
            RecId::Outputs(f) => {
                let facts = self.p.rules.get(f).cloned();
                match facts {
                    Some(facts) if facts.outputs.len() == 1 => self.rule_col_type(f, &facts.outputs[0]),
                    _ => self.path(&self.imp(Import::Rule(f.clone())), "Output"),
                }
            }
            RecId::Element(f, seq) => self.path(&self.imp(Import::Rule(f.clone())), &self.element_type(f, seq)),
            RecId::Running(item, w) => {
                let k = self.imp(Import::Kaname);
                let (i, w) = (self.boundary(item), self.boundary(w));
                match self.t {
                    Target::TypeScript => format!("{k}.Running<{i}, {w}>"),
                    Target::Python => format!("{k}.Running[{i}, {w}]"),
                    Target::Go => format!("{k}.Running[{i}, {w}]"),
                    Target::Rust => format!("{k}::Running<{i}, {w}>"),
                }
            }
            // the elements of what `d.entries()` and `xs.indexed()` answer (DESIGN 24.6)
            RecId::Entry(kt, vt) => {
                let k = self.imp(Import::Kaname);
                let (a, b) = (self.boundary(kt), self.boundary(vt));
                match self.t {
                    Target::TypeScript => format!("{k}.Entry<{a}, {b}>"),
                    Target::Rust => format!("{k}::Entry<{a}, {b}>"),
                    _ => format!("{k}.Entry[{a}, {b}]"),
                }
            }
            RecId::Indexed(x) => {
                let k = self.imp(Import::Kaname);
                let a = self.boundary(x);
                match self.t {
                    Target::TypeScript => format!("{k}.Indexed<{a}>"),
                    Target::Rust => format!("{k}::Indexed<{a}>"),
                    _ => format!("{k}.Indexed[{a}]"),
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
        s.strip_prefix("list[")
            .and_then(|x| x.strip_suffix(']'))
            .or_else(|| s.strip_prefix("&[").and_then(|x| x.strip_suffix(']')))
            .or_else(|| s.strip_prefix("[]"))
            .or_else(|| s.strip_suffix("[]"))
            .unwrap_or(s)
            .to_string()
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
                Some(e) => self.path(&self.imp(Import::Rule(f.to_path_buf())), &e.alias),
                None => self.str_type().into(),
            },
            ColumnType::Num { unit: Some(u), .. } if u.dim == Dim::Rate => self.path(&self.imp(Import::Rule(f.to_path_buf())), "Rate"),
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
            Target::Rust => "String",
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
            // a dict and a set hold their keys and values as they travel, as a record does (DESIGN 24.6)
            Ty::Dict(k, v) => self.dict_type(self.boundary(k), self.boundary(v)),
            Ty::Set(x) => self.set_type(self.boundary(x)),
            Ty::Opt(x) => self.opt_type(self.flight(x)),
            Ty::Transfer(id) => self.transfer_type(id),
            Ty::Param(n) => self.param_type(n),
            Ty::Result(a, e) => self.result_type(a, e),
            Ty::Fn(ps, r, eff) => self.fn_type(ps, r, eff),
            Ty::None | Ty::Empty | Ty::EmptyDict | Ty::Error => match self.t {
                Target::TypeScript => "null".into(),
                Target::Python => "None".into(),
                Target::Go => "any".into(),
                Target::Rust => "_".into(),
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

    /// A function type in the target (DESIGN 27.6): its parameters and its answer as they travel, as a
    /// function's do: TypeScript's `(x: T) => U`, Python's `Callable[[T], U]`, Go's `func(T) U` (a
    /// failure is a panic, as in every function the generated code calls inside).
    pub fn fn_type(&self, ps: &[Ty], r: &Ty, eff: &Option<crate::ty::FnEffect>) -> String {
        let pts: Vec<String> = ps.iter().map(|p| self.boundary(p)).collect();
        let rt = self.boundary(r);
        match self.t {
            Target::TypeScript => {
                let named: Vec<String> = match pts.as_slice() {
                    [one] => vec![format!("x: {one}")],
                    _ => pts.iter().enumerate().map(|(i, p)| format!("x{}: {p}", i + 1)).collect(),
                };
                format!("({}) => {rt}", named.join(", "))
            }
            Target::Python => format!("Callable[[{}], {rt}]", pts.join(", ")),
            Target::Go => format!("func({}) {rt}", pts.join(", ")),
            // lent, as a parameter is (DESIGN 36.2): what has no effect written may be given one
            // that fails, so it answers a `Result`; `total` answers the value
            Target::Rust => {
                let pts: Vec<String> = ps.iter().map(|p| self.param_rust(p)).collect();
                let total = matches!(eff, Some(e) if e.fails.is_empty());
                let k = self.imp(Import::Kaname);
                let rt = if total { rt } else { format!("{}<{rt}, {k}::KanameError>", self.std_type("Result")) };
                format!("&dyn {}({}) -> {rt}", self.std_type("Fn"), pts.join(", "))
            }
        }
    }

    /// A type parameter in the target (DESIGN 26.7): its alias, as the declaration written now names it.
    pub fn param_type(&self, n: &str) -> String {
        self.tscope.borrow().iter().find(|p| p.name == n).map(|p| p.ty.clone()).unwrap_or_else(|| n.to_string())
    }

    /// The order the function written now is given for its type parameter `n` (held to keys): the
    /// name of the hidden argument.
    pub fn param_order(&self, n: &str) -> String {
        self.tscope.borrow().iter().find(|p| p.name == n).and_then(|p| p.order.clone()).unwrap_or_else(|| "undefined".to_string())
    }

    /// The names of the parameters of the function `f` of the module `callee` in the code of its body:
    /// the keywords Python's generated code calls its generic function by.
    pub fn callee_params(&self, callee: &Rc<Checked>, f: usize) -> Vec<String> {
        if callee.canon == self.c.canon {
            return param_names(self, f);
        }
        let other = Gen::new(callee, self.t, self.opts);
        param_names(&other, f)
    }

    /// The names a declaration's type parameters take in the target's code (DESIGN 26.7): each its
    /// alias, unless a name of the module's code, a word of the target or a type the code names has
    /// it: then with `_` after it in TypeScript and Python (`T_`), and a number in Go (`T2`). Python
    /// declares each once in the module (`T = TypeVar("T")`), which the declarations share.
    pub fn tparam_names(&self, ps: &[crate::names::TParamInfo]) -> Vec<String> {
        let mut taken: BTreeSet<String> = BTreeSet::new();
        ps.iter()
            .map(|p| {
                let a = p.name.ascii().to_string();
                let mut name = a.clone();
                let mut i = 1;
                while taken.contains(&name) || self.outer.contains(&name) || type_kept(self.t, &name) {
                    i += 1;
                    name = match self.t {
                        Target::Go => format!("{a}{i}"),
                        _ if i == 2 => format!("{a}_"),
                        _ => format!("{a}_{i}"),
                    };
                }
                taken.insert(name.clone());
                name
            })
            .collect()
    }

    /// The type parameters of a declaration in scope, for the code of its fields' types or of its
    /// body; a function's held to keys given the orders `orders` names.
    pub fn enter(&self, ps: &[crate::names::TParamInfo], orders: &[Option<String>]) {
        let names = self.tparam_names(ps);
        *self.tscope.borrow_mut() = ps.iter().zip(names).enumerate().map(|(i, (p, ty))| TScope { name: p.name.text.clone(), ty, key: p.key, order: orders.get(i).cloned().flatten() }).collect();
    }

    /// No type parameters in scope.
    pub fn leave(&self) {
        self.tscope.borrow_mut().clear();
    }

    /// A declaration's type parameters as the target declares them after its name: TypeScript's
    /// `<T, K>`, Python's `T, K` (in `Generic[…]`), Go's `[T any, K comparable]` (a key of a map is
    /// `comparable`; the order of keys is given apart, DESIGN 26.7). Nothing for none.
    pub fn tparams_decl(&self, ps: &[crate::names::TParamInfo]) -> String {
        if ps.is_empty() {
            return String::new();
        }
        let names = self.tparam_names(ps);
        match self.t {
            Target::TypeScript => format!("<{}>", names.join(", ")),
            Target::Python => names.join(", "),
            Target::Go => format!("[{}]", ps.iter().zip(&names).map(|(p, n)| format!("{n} {}", if p.key { "comparable" } else { "any" })).collect::<Vec<_>>().join(", ")),
            Target::Rust => format!("<{}>", ps.iter().zip(&names).map(|(p, n)| format!("{n}: {}", self.bounds(p.key))).collect::<Vec<_>>().join(", ")),
        }
    }

    /// The bounds of a type parameter in Rust (DESIGN 36.2): a key's `Ord` is the order of keys.
    pub fn bounds(&self, key: bool) -> String {
        if key { format!("{} + {}", self.std_type("Ord"), self.std_type("Clone")) } else { format!("{} + {}", self.std_type("Clone"), self.std_type("PartialEq")) }
    }

    /// A declaration's type parameters as the target writes them where the type is used: TypeScript's
    /// `<T, K>`, Python's and Go's `[T, K]`. Nothing for none.
    pub fn tparams_use(&self, ps: &[crate::names::TParamInfo]) -> String {
        if ps.is_empty() {
            return String::new();
        }
        let names = self.tparam_names(ps);
        match self.t {
            Target::TypeScript | Target::Rust => format!("<{}>", names.join(", ")),
            _ => format!("[{}]", names.join(", ")),
        }
    }

    /// The type parameters of the declaration written now that a type holds, as a function of the
    /// module that takes a value of it declares them (a check of a place in a generic function's body):
    /// TypeScript's `<T>`, Go's `[T any]`; nothing in Python, whose type variables are the module's.
    pub fn tparams_of(&self, t: &Ty) -> String {
        let mut names: Vec<String> = Vec::new();
        params_in(t, &mut names);
        if names.is_empty() || self.t == Target::Python {
            return String::new();
        }
        let scope = self.tscope.borrow();
        let parts: Vec<String> = names
            .iter()
            .map(|n| {
                let p = scope.iter().find(|p| p.name == *n);
                let ty = p.map(|p| p.ty.clone()).unwrap_or_else(|| n.clone());
                match self.t {
                    Target::Go => format!("{ty} {}", if p.is_some_and(|p| p.key) { "comparable" } else { "any" }),
                    Target::Rust => format!("{ty}: {}", self.bounds(p.is_some_and(|p| p.key))),
                    _ => ty,
                }
            })
            .collect();
        match self.t {
            Target::TypeScript | Target::Rust => format!("<{}>", parts.join(", ")),
            _ => format!("[{}]", parts.join(", ")),
        }
    }

    /// The type arguments of a generic record's or enum's type as the target writes them after its
    /// name (`<Invoice>`, `[Invoice]`), each as it travels; nothing for one that takes none.
    pub fn targs_text(&self, args: &[Ty]) -> String {
        if args.is_empty() {
            return String::new();
        }
        let xs: Vec<String> = args.iter().map(|a| self.boundary(a)).collect();
        match self.t {
            Target::TypeScript | Target::Rust => format!("<{}>", xs.join(", ")),
            _ => format!("[{}]", xs.join(", ")),
        }
    }

    /// `result[T, E]` in the target (DESIGN 26.7): the helper's `Result`.
    pub fn result_type(&self, a: &Ty, e: &Ty) -> String {
        let k = self.imp(Import::Kaname);
        let (x, y) = (self.boundary(a), self.boundary(e));
        match self.t {
            Target::TypeScript => format!("{k}.Result<{x}, {y}>"),
            // the standard library's (DESIGN 36.2)
            Target::Rust => format!("{}<{x}, {y}>", self.std_type("Result")),
            _ => format!("{k}.Result[{x}, {y}]"),
        }
    }

    /// A value of `result[a, e]`, `ok` holding `x` or `err` holding it, as it travels (DESIGN 26.7):
    /// TypeScript's object with its `kind`, Python's `kaname.Ok` and `kaname.Err` with their type
    /// written (mypy reads `Ok(value=[])` as of no type), Go's `kaname.Ok[A, E]` and `kaname.Err[A, E]`.
    pub fn result_value(&self, ok: bool, x: Ex, a: &Ty, e: &Ty) -> Ex {
        let k = self.imp(Import::Kaname);
        let (field, ty) = if ok { ("value", "Ok") } else { ("error", "Err") };
        match self.t {
            Target::TypeScript => Ex::Record(String::new(), vec![("kind".into(), self.string(if ok { "ok" } else { "err" })), (field.into(), x)]),
            Target::Python => Ex::Record(format!("{k}.{ty}[{}]", self.boundary(if ok { a } else { e })), vec![(field.into(), x)]),
            Target::Go => Ex::Record(format!("{k}.{ty}[{}, {}]", self.boundary(a), self.boundary(e)), vec![(if ok { "Value" } else { "Error" }.into(), x)]),
            Target::Rust => Ex::call(ty, vec![x]),
        }
    }

    /// The type of a transfer's operation, from the types of its book (`glue/books/<book>`).
    pub fn transfer_type(&self, id: &TransferId) -> String {
        self.path(&self.imp(Import::Book(id.book.clone())), &super::books::op_type(self.t, &id.transfer, &id.op))
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
            Ty::Num(n) => n.unit.type_name().is_none() || !matches!(self.t, Target::Go | Target::Rust),
            Ty::List(x) => match &**x {
                Ty::Num(n) if !n.is_rate() && n.unit.type_name().is_some() => self.t == Target::TypeScript,
                x => self.same_in(x),
            },
            Ty::Opt(x) => self.same_in(x),
            _ => true,
        }
    }

    // ── What Rust holds in a box (DESIGN 36.2) ──────────────────────────────

    /// A record, a sum or the type of one of its values as it is declared, with no type arguments:
    /// what the boxes are worked out on.
    fn bare(t: &Ty) -> Ty {
        match t {
            Ty::Record(RecId::Module { file, name, .. }) => Ty::Record(RecId::Module { file: file.clone(), name: name.clone(), args: Vec::new() }),
            Ty::Record(RecId::Variant { file, name, value, .. }) => Ty::Record(RecId::Variant { file: file.clone(), name: name.clone(), value: value.clone(), args: Vec::new() }),
            Ty::Enum(id) => Ty::Enum(EnumId { owner: id.owner.clone(), name: id.name.clone(), args: Vec::new() }),
            t => t.clone(),
        }
    }

    /// The records, sums and values' types a value of `t` holds in itself, not behind a list, a dict
    /// or a set: what would make a type of Rust's that holds itself of no size.
    fn held_in(&self, t: &Ty) -> Vec<Ty> {
        match t {
            Ty::Opt(x) => self.held_in(x),
            Ty::Result(a, e) => {
                let mut v = self.held_in(a);
                v.extend(self.held_in(e));
                v
            }
            Ty::Record(RecId::Module { .. } | RecId::Variant { .. }) => vec![Gen::bare(t)],
            Ty::Enum(id) if self.p.is_sum(id) => vec![Gen::bare(t)],
            _ => Vec::new(),
        }
    }

    /// What a value of a declared type holds in itself, one step down: a record's and a value's fields,
    /// a sum's values.
    fn holds(&self, t: &Ty) -> Vec<Ty> {
        match t {
            Ty::Record(id @ (RecId::Module { .. } | RecId::Variant { .. })) => self.p.fields(id).iter().flat_map(|(_, s)| self.held_in(&s.ty)).collect(),
            Ty::Enum(id) if self.p.is_sum(id) => {
                let Owner::Module(file) = &id.owner else { return Vec::new() };
                self.p.values(id).0.iter().map(|v| Ty::Record(RecId::Variant { file: file.clone(), name: id.name.clone(), value: v.text.clone(), args: Vec::new() })).collect()
            }
            _ => Vec::new(),
        }
    }

    /// Whether a value of `from` holds in itself, at some depth, a value of `to`.
    fn holds_deep(&self, from: &Ty, to: &Ty) -> bool {
        let mut seen: Vec<Ty> = Vec::new();
        let mut todo = vec![from.clone()];
        while let Some(x) = todo.pop() {
            for y in self.holds(&x) {
                if y == *to {
                    return true;
                }
                if !seen.contains(&y) {
                    seen.push(y.clone());
                    todo.push(y);
                }
            }
        }
        false
    }

    /// How a field of type `t` of the record or value's type `holder` is boxed in Rust (DESIGN 36.2):
    /// a field that holds, not behind a list, a dict or a set, a type that holds `holder`.
    pub fn boxed(&self, holder: &Ty, t: &Ty) -> Boxed {
        let cyc = |x: &Ty| matches!(x, Ty::Record(RecId::Module { .. } | RecId::Variant { .. })) || matches!(x, Ty::Enum(id) if self.p.is_sum(id));
        let boxes = |x: &Ty| cyc(x) && self.t == Target::Rust && { let (b, h) = (Gen::bare(x), Gen::bare(holder)); b == h || self.holds_deep(&b, &h) };
        match t {
            x if cyc(x) => if boxes(x) { Boxed::Whole } else { Boxed::No },
            Ty::Opt(x) if boxes(x) => Boxed::InOpt,
            Ty::Result(a, e) if boxes(a) || boxes(e) => Boxed::InResult(boxes(a), boxes(e)),
            _ => Boxed::No,
        }
    }

    /// The type of a field of `holder` as it travels: Rust's boxed where [`Gen::boxed`] says so
    /// (`Box<Expr>`, `Option<Box<Node>>`).
    pub fn field_type(&self, holder: &Ty, t: &Ty) -> String {
        let b = |x: &Ty| format!("{}<{}>", self.std_type("Box"), self.boundary(x));
        match (self.boxed(holder, t), t) {
            (Boxed::Whole, _) => b(t),
            (Boxed::InOpt, Ty::Opt(x)) => self.opt_type(b(x)),
            (Boxed::InResult(o, e), Ty::Result(a, er)) => {
                format!("{}<{}, {}>", self.std_type("Result"), if o { b(a) } else { self.boundary(a) }, if e { b(er) } else { self.boundary(er) })
            }
            _ => self.boundary(t),
        }
    }

    /// A value `x` of a field of type `t` of `holder` as the field holds it: Rust's in its box
    /// (`Box::new(x)`, `Some(Box::new(x))`, `x.map(Box::new)`).
    pub fn boxed_value(&self, holder: &Ty, t: &Ty, x: Ex) -> Ex {
        let new = format!("{}::new", self.std_type("Box"));
        let wrap = |x: Ex| Ex::call(new.clone(), vec![x]);
        let inside = |x: Ex, ctor: &str, m: &str| -> Ex {
            match x {
                Ex::Call(f, mut a) if matches!(&*f, Ex::Atom(n) if n == ctor) && a.len() == 1 => Ex::call(ctor, vec![wrap(a.remove(0))]),
                x => x.method(m, vec![Ex::atom(new.clone())]),
            }
        };
        match self.boxed(holder, t) {
            Boxed::No => x,
            Boxed::Whole => wrap(x),
            Boxed::InOpt if self.absent(&x) => x,
            Boxed::InOpt => inside(x, "Some", "map"),
            Boxed::InResult(o, e) => {
                let x = if o { inside(x, "Ok", "map") } else { x };
                if e { inside(x, "Err", "map_err") } else { x }
            }
        }
    }

    /// The field `raw` of `holder`, of type `t`, out of its box: the place of the value (`*x.left`),
    /// a value that may be absent lent (`x.next.as_deref()`), a result copied; and how it is held.
    pub fn unboxed(&self, holder: &Ty, t: &Ty, raw: Ex, held: Held) -> (Ex, Held) {
        match self.boxed(holder, t) {
            Boxed::No => (raw, held),
            Boxed::Whole => (Ex::Deref(Box::new(raw)), held),
            Boxed::InOpt => (raw.method("as_deref", vec![]), Held::Lent),
            Boxed::InResult(o, e) => {
                let mut x = raw.method("clone", vec![]);
                if o {
                    x = x.method("map", vec![Ex::atom("|b| *b")]);
                }
                if e {
                    x = x.method("map_err", vec![Ex::atom("|b| *b")]);
                }
                (x, Held::Made)
            }
        }
    }

    // ── The checks of what a place declares ────────────────────────────────

    /// The function that finds where a value of `slot`, as it travels, is outside what the slot
    /// declares (`interp.rs`'s `outside`, as `reach` holds it): its name, made once. Where a host's
    /// argument is held at every depth and a kaname call's is held short of the values of types that
    /// hold themselves (DESIGN 28.1), the two are two functions; elsewhere one.
    pub fn outside_fn(&self, slot: &Slot, reach: Reach) -> Option<String> {
        if !self.checkable(&slot.ty, slot.range.is_some() || slot.count.is_some(), reach, 0) {
            return None;
        }
        let tparams = self.tparams_of(&slot.ty);
        let mode = if reach == Reach::Entry && self.p.reaches_recursive(&slot.ty) { "entry".to_string() } else { reach.reached().to_string() };
        let key = format!("{:?}|{:?}|{:?}|{mode}|{tparams}", slot.ty, slot.range, slot.count);
        if let Some(o) = self.outsides.borrow().iter().find(|o| o.key == key) {
            return Some(o.name.clone());
        }
        let n = self.outsides.borrow().len() + 1;
        let name = format!("_outside{n}");
        // made first, so that a record that holds itself is not made forever (it cannot: E108)
        self.outsides.borrow_mut().push(Outside { key: key.clone(), name: name.clone(), body: Vec::new(), ty: String::new(), tparams });
        let v = Ex::atom("v");
        let what = Ex::atom("what");
        let body = self.outside_body(&v, &slot.ty, slot.range.as_ref(), slot.count.as_ref(), what, reach, 0);
        // Rust's check takes the value lent, as a parameter is (DESIGN 36.4)
        let ty = if self.t == Target::Rust { self.lent_type(&slot.ty, true) } else { self.boundary(&slot.ty) };
        if let Some(o) = self.outsides.borrow_mut().iter_mut().find(|o| o.key == key) {
            o.body = body;
            o.ty = ty;
        }
        Some(name)
    }

    /// Whether a value of `t` can be outside what its place declares.
    fn checkable(&self, t: &Ty, declared: bool, reach: Reach, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        let reached = reach.reached();
        match t {
            Ty::Opt(x) => self.checkable(x, declared, reach, depth),
            Ty::List(x) => declared || self.checkable(x, false, reach, depth),
            // a dict's count and range of its values, the type of its keys; a set's (DESIGN 24.6)
            Ty::Dict(k, x) => declared || self.checkable(k, false, reach, depth) || self.checkable(x, false, reach, depth),
            Ty::Set(x) => declared || self.checkable(x, false, reach, depth),
            // where a parameter reaches it, a number whose range is not written is held to int64 in
            // TypeScript and Python (Go's int64 holds nothing else), and a date to the days of
            // 0001-01-01 to 9999-12-31 in TypeScript and Go (Python's date is one) (DESIGN 22.5)
            Ty::Num(n) => declared || (reached && n.is_rate() && n.step.is_some()) || (reached && !n.is_rate() && !matches!(self.t, Target::Go | Target::Rust)),
            Ty::Date => declared || (reached && self.t != Target::Python),
            // a string, to its count, and where a parameter reaches it to be well formed (DESIGN 25.1):
            // a string of Rust's is always (DESIGN 36.5)
            Ty::Str => declared || (reached && self.t != Target::Rust),
            Ty::Record(RecId::Module { .. } | RecId::Variant { .. }) if reached => self.record_checkable(t, depth, reach),
            Ty::Enum(id) if reached && self.p.is_sum(id) => self.record_checkable(t, depth, reach),
            // a result, by the value it holds (DESIGN 26.3)
            Ty::Result(a, e) if reached => self.checkable(a, false, reach, depth + 1) || self.checkable(e, false, reach, depth + 1),
            _ => false,
        }
    }

    /// Whether a check of a value of `slot` can find a list outside its count.
    pub fn counts(&self, slot: &Slot, reach: Reach) -> bool {
        self.counts_in(&slot.ty, slot.count.is_some(), reach, 0)
    }

    /// Whether a check of a value of `slot` can find a string outside its count of code points, and a
    /// string that is not well formed (where a parameter reaches it).
    fn strings(&self, slot: &Slot, reach: Reach) -> (bool, bool) {
        self.strings_in(&slot.ty, slot.count.is_some(), reach, 0)
    }

    fn strings_in(&self, t: &Ty, counted: bool, reach: Reach, depth: usize) -> (bool, bool) {
        if depth > 8 {
            return (false, false);
        }
        let reached = reach.reached();
        // a kaname call holds no value of a type that holds itself (DESIGN 28.1)
        let inside = reached && !(reach == Reach::Call && self.p.recursive(t));
        let or = |a: (bool, bool), b: (bool, bool)| (a.0 || b.0, a.1 || b.1);
        match t {
            Ty::Opt(x) => self.strings_in(x, counted, reach, depth),
            Ty::Str => (counted, reached && self.t != Target::Rust),
            Ty::List(x) | Ty::Set(x) => self.strings_in(x, false, reach, depth + 1),
            Ty::Dict(k, x) => or(self.strings_in(k, false, reach, depth + 1), self.strings_in(x, false, reach, depth + 1)),
            Ty::Record(id @ (RecId::Module { .. } | RecId::Variant { .. })) if inside => {
                self.p.fields(id).iter().fold((false, false), |acc, (_, s)| or(acc, self.strings_in(&s.ty, s.count.is_some(), reach, depth + 1)))
            }
            Ty::Enum(id) if inside && self.p.is_sum(id) => self.p.values(id).0.iter().fold((false, false), |acc, v| {
                self.p.value_fields(id, &v.text).iter().fold(acc, |acc, (_, s)| or(acc, self.strings_in(&s.ty, s.count.is_some(), reach, depth + 1)))
            }),
            Ty::Result(a, e) if reached => or(self.strings_in(a, false, reach, depth + 1), self.strings_in(e, false, reach, depth + 1)),
            _ => (false, false),
        }
    }

    fn counts_in(&self, t: &Ty, counted: bool, reach: Reach, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        let reached = reach.reached();
        let inside = reached && !(reach == Reach::Call && self.p.recursive(t));
        match t {
            Ty::Opt(x) => self.counts_in(x, counted, reach, depth),
            Ty::List(x) | Ty::Set(x) => counted || self.counts_in(x, false, reach, depth + 1),
            Ty::Dict(k, x) => counted || self.counts_in(k, false, reach, depth + 1) || self.counts_in(x, false, reach, depth + 1),
            Ty::Record(id @ (RecId::Module { .. } | RecId::Variant { .. })) if inside => self.p.fields(id).iter().any(|(_, s)| self.counts_in(&s.ty, s.count.is_some(), reach, depth + 1)),
            Ty::Enum(id) if inside && self.p.is_sum(id) => self.p.values(id).0.iter().any(|v| self.p.value_fields(id, &v.text).iter().any(|(_, s)| self.counts_in(&s.ty, s.count.is_some(), reach, depth + 1))),
            Ty::Result(a, e) if reached => self.counts_in(a, false, reach, depth + 1) || self.counts_in(e, false, reach, depth + 1),
            _ => false,
        }
    }

    /// Whether a record, a value of a sum or the type of one of its values has a field a parameter
    /// reaching it can find outside what it declares, as `reach` holds it: a kaname call does not hold
    /// a value of a type that holds itself (DESIGN 28.1).
    pub fn record_checkable(&self, t: &Ty, depth: usize, reach: Reach) -> bool {
        if reach == Reach::Call && self.p.recursive(t) {
            return false;
        }
        let fields: Vec<(crate::ast::Name, Slot)> = match t {
            Ty::Record(id @ (RecId::Module { .. } | RecId::Variant { .. })) => self.p.fields(id),
            Ty::Enum(id) if self.p.is_sum(id) => self.p.values(id).0.iter().flat_map(|v| self.p.value_fields(id, &v.text)).collect(),
            _ => return false,
        };
        fields.iter().any(|(_, s)| self.checkable(&s.ty, s.range.is_some() || s.count.is_some(), reach, depth + 1))
    }

    /// The sum a type is of, for a value of it or the type of one of its values: the check of its fields
    /// is the sum's (`_outside_Order`, Go's `KanameOutside` of each value's type).
    fn sum_of(t: &Ty) -> Option<EnumId> {
        match t {
            Ty::Enum(id) => Some(id.clone()),
            Ty::Record(r) => r.sum(),
            _ => None,
        }
    }

    /// The body of a check of a value `v` of `t`: a return of the first place it is outside, the
    /// path written from `what`.
    #[allow(clippy::too_many_arguments)]
    fn outside_body(&self, v: &Ex, t: &Ty, range: Option<&Range>, count: Option<&Range>, what: Ex, reach: Reach, depth: usize) -> Vec<Node> {
        let mut out = Vec::new();
        let reached = reach.reached();
        match t {
            // Rust's value that is there, by `if let` (an `Option` of a `Copy` type, or of a reference)
            Ty::Opt(x) if self.t == Target::Rust => {
                let inner = Ex::atom(format!("x{depth}"));
                let body = self.outside_body(&inner, x, range, count, what, reach, depth);
                if !body.is_empty() {
                    out.push(Node::If(vec![(Ex::atom(format!("let Some({}) = {}", inner.print(self.t), v.print(self.t))), body)], Vec::new()));
                }
            }
            Ty::Opt(x) => {
                let (inner, deref) = match self.t {
                    Target::Go => (Ex::Deref(Box::new(v.clone())), true),
                    _ => (v.clone(), false),
                };
                let _ = deref;
                let body = self.outside_body(&inner, x, range, count, what, reach, depth);
                if !body.is_empty() {
                    out.push(Node::If(vec![(Ex::bin(Op::Ne, v.clone(), self.null()), body)], Vec::new()));
                }
            }
            Ty::List(x) => {
                let len = self.len_of(v);
                if let Some(c) = count {
                    let mut tests = Vec::new();
                    // Rust's length is a `usize`, never below 0; below 1 is `is_empty`
                    if let Some(lo) = c.min
                        && (self.t != Target::Rust || lo.num > 0)
                    {
                        tests.push(if self.t == Target::Rust && lo.num == 1 { v.clone().method("is_empty", vec![]) } else { Ex::bin(Op::Lt, len.clone(), Ex::atom(lo.num.to_string())) });
                    }
                    if let Some(hi) = c.max {
                        tests.push(Ex::bin(Op::Gt, len.clone(), Ex::atom(hi.num.to_string())));
                    }
                    if let Some(test) = tests.into_iter().reduce(|a, b| Ex::bin(Op::Or, a, b)) {
                        let shown = crate::value::range_text(c, &Ty::Num(NumTy::number()));
                        out.push(Node::If(vec![(test, vec![Node::Line(self.ret_found(self.outside_value(what.clone(), self.string(""), self.string(&shown), Some(len.clone()))))])], Vec::new()));
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
                    Target::Rust => self.kcall("elem", vec![self.what_arg(&what), i.clone()]),
                };
                let inner = if opt {
                    if self.t == Target::Rust {
                        let x = Ex::atom(format!("x{}", depth + 1));
                        let b = self.outside_body(&x, &elem, range, None, path, reach, depth + 1);
                        if b.is_empty() { b } else { vec![Node::If(vec![(Ex::atom(format!("let Some({}) = {}", x.print(self.t), el.print(self.t))), b)], Vec::new())] }
                    } else {
                        let deref = if self.t == Target::Go { Ex::Deref(Box::new(el.clone())) } else { el.clone() };
                        let b = self.outside_body(&deref, &elem, range, None, path, reach, depth + 1);
                        if b.is_empty() { b } else { vec![Node::If(vec![(Ex::bin(Op::Ne, el.clone(), self.null()), b)], Vec::new())] }
                    }
                } else {
                    self.outside_body(&el, &elem, range, None, path, reach, depth + 1)
                };
                if !inner.is_empty() {
                    let head = match self.t {
                        Target::TypeScript => format!("for (let {} = 0; {} < {}.length; {}++)", i.print(self.t), i.print(self.t), v.print(self.t), i.print(self.t)),
                        Target::Python => format!("for {}, {} in enumerate({})", i.print(self.t), el.print(self.t), v.print(self.t)),
                        Target::Go => format!("for {}, {} := range {}", i.print(self.t), el.print(self.t), v.print(self.t)),
                        // an element of a `Copy` type by value, any other lent
                        Target::Rust => format!("for ({}, {}{}) in {}.iter().enumerate()", i.print(self.t), if self.is_copy(x) { "&" } else { "" }, el.print(self.t), v.print(self.t)),
                    };
                    let mut body = Vec::new();
                    if self.t == Target::TypeScript {
                        body.push(Node::Line(format!("const {} = {}[{}];", el.print(self.t), v.print(self.t), i.print(self.t))));
                    }
                    body.extend(inner);
                    out.push(Node::Block(head, body, Vec::new()));
                }
            }
            // a dict: its count, then each pair in the order of its keys, its key and its value, where
            // they travel (`d[0][0]`, `d[0][1]`); a set: its count, then each member (`s[0]`) (DESIGN
            // 24.4, `interp.rs`'s `outside`)
            Ty::Dict(_, _) | Ty::Set(_) => {
                let (kt, vt) = match t {
                    Ty::Dict(k, x) => ((**k).clone(), Some((**x).clone())),
                    Ty::Set(x) => ((**x).clone(), None),
                    _ => (Ty::Error, None),
                };
                let len = match self.t {
                    Target::TypeScript => Ex::call("BigInt", vec![v.clone().dot("size")]),
                    Target::Python => Ex::call("len", vec![v.clone()]),
                    Target::Go => Ex::call("len", vec![v.clone()]),
                    Target::Rust => v.clone().method("len", vec![]),
                };
                if let Some(c) = count {
                    let mut tests = Vec::new();
                    // Rust's length is a `usize`, never below 0; below 1 is `is_empty`
                    if let Some(lo) = c.min
                        && (self.t != Target::Rust || lo.num > 0)
                    {
                        tests.push(if self.t == Target::Rust && lo.num == 1 { v.clone().method("is_empty", vec![]) } else { Ex::bin(Op::Lt, len.clone(), self.count_lit(lo.num)) });
                    }
                    if let Some(hi) = c.max {
                        tests.push(Ex::bin(Op::Gt, len.clone(), self.count_lit(hi.num)));
                    }
                    if let Some(test) = tests.into_iter().reduce(|a, b| Ex::bin(Op::Or, a, b)) {
                        let shown = crate::value::range_text(c, &Ty::Num(NumTy::number()));
                        out.push(Node::If(vec![(test, vec![Node::Line(self.ret_found(self.outside_value(what.clone(), self.string(""), self.string(&shown), Some(len.clone()))))])], Vec::new()));
                    }
                }
                let i = Ex::atom(format!("i{depth}"));
                let key = Ex::atom(format!("k{depth}"));
                let val = Ex::atom(format!("e{depth}"));
                let at = |idx: Option<i32>| -> Ex {
                    let base = match self.t {
                        Target::TypeScript => Ex::bin(Op::Add, Ex::bin(Op::Add, Ex::bin(Op::Add, what.clone(), self.string("[")), i.clone()), self.string("]")),
                        Target::Python => Ex::bin(Op::Add, Ex::bin(Op::Add, Ex::bin(Op::Add, what.clone(), self.string("[")), Ex::call("str", vec![i.clone()])), self.string("]")),
                        Target::Go => self.kcall("Elem", vec![what.clone(), i.clone()]),
                        Target::Rust => self.kcall("elem", vec![self.what_arg(&what), i.clone()]),
                    };
                    match (idx, self.t) {
                        (None, _) => base,
                        (Some(n), Target::Go) => self.kcall("Elem", vec![base, Ex::atom(n.to_string())]),
                        (Some(n), Target::Rust) => self.kcall("elem", vec![base.borrow(), Ex::atom(n.to_string())]),
                        (Some(n), _) => Ex::bin(Op::Add, base, self.string(&format!("[{n}]"))),
                    }
                };
                let mut inner = Vec::new();
                // the value of a pair is read where it is checked (a string key may be checked alone)
                let mut valued = false;
                let mut keyed = true;
                match &vt {
                    Some(x) => {
                        let of_key = self.outside_body(&key, &kt, None, None, at(Some(0)), reach, depth + 1);
                        keyed = !of_key.is_empty();
                        inner.extend(of_key);
                        let of_value = self.elem_body(&val, x, range, at(Some(1)), reach, depth);
                        valued = !of_value.is_empty();
                        inner.extend(of_value);
                    }
                    None => inner.extend(self.elem_body(&key, &kt, range, at(None), reach, depth)),
                }
                // Rust walks a `BTreeMap` and a `BTreeSet` in the order of their keys, each key and value
                // of a `Copy` type by value, any other lent
                if !inner.is_empty() && self.t == Target::Rust {
                    let pat = |used: bool, x: &Ex, ty: &Ty| if !used { "_".to_string() } else if self.is_copy(ty) { format!("&{}", x.print(self.t)) } else { x.print(self.t) };
                    let head = match &vt {
                        Some(x) => format!("for ({}, ({}, {})) in {}.iter().enumerate()", i.print(self.t), pat(keyed, &key, &kt), pat(valued, &val, x), v.print(self.t)),
                        None => format!("for ({}, {}) in {}.iter().enumerate()", i.print(self.t), pat(true, &key, &kt), v.print(self.t)),
                    };
                    out.push(Node::Block(head, inner, Vec::new()));
                } else if !inner.is_empty() {
                    let keys = match self.t {
                        Target::TypeScript => self.kcall("sortedKeys", vec![v.clone(), self.order_fn(&kt, false)]),
                        Target::Python => self.kcall("keys_of", vec![v.clone(), self.py_key_fn(&kt)]),
                        Target::Go => self.kcall("SortedKeys", vec![v.clone(), self.order_fn(&kt, false)]),
                        Target::Rust => unreachable!("Rust walks its maps and sets above"),
                    };
                    let ks = Ex::atom(format!("ks{depth}"));
                    let mut body = Vec::new();
                    let head = match self.t {
                        Target::TypeScript => {
                            body.push(Node::Line(format!("const {} = {}[{}];", key.print(self.t), ks.print(self.t), i.print(self.t))));
                            if valued {
                                body.push(Node::Line(format!("const {} = {}.get({}) as {};", val.print(self.t), v.print(self.t), key.print(self.t), self.boundary(vt.as_ref().unwrap_or(&Ty::Error)))));
                            }
                            out.push(Node::Line(format!("const {} = {};", ks.print(self.t), keys.print(self.t))));
                            format!("for (let {} = 0; {} < {}.length; {}++)", i.print(self.t), i.print(self.t), ks.print(self.t), i.print(self.t))
                        }
                        Target::Python => {
                            if valued {
                                body.push(Node::Line(format!("{} = {}[{}]", val.print(self.t), v.print(self.t), key.print(self.t))));
                            }
                            format!("for {}, {} in enumerate({})", i.print(self.t), key.print(self.t), keys.print(self.t))
                        }
                        Target::Go => {
                            if valued {
                                body.push(Node::Line(format!("{} := {}[{}]", val.print(self.t), v.print(self.t), key.print(self.t))));
                            }
                            format!("for {}, {} := range {}", i.print(self.t), key.print(self.t), keys.print(self.t))
                        }
                        Target::Rust => unreachable!("Rust walks its maps and sets above"),
                    };
                    body.extend(inner);
                    out.push(Node::Block(head, body, Vec::new()));
                }
            }
            // a string: well formed where a parameter reaches it, then its code points to its count
            // (DESIGN 25.1, `interp.rs`'s `outside`)
            Ty::Str => {
                if reached && self.t != Target::Rust {
                    let test = Ex::Not(Box::new(self.kcall(&self.helper("isText", "is_text", "IsText"), vec![v.clone()])));
                    out.push(Node::If(vec![(test, vec![Node::Line(self.ret_found(self.outside_flagged(what.clone(), "bad", None, "")))])], Vec::new()));
                }
                if let Some(c) = count {
                    let len = match self.t {
                        Target::Python => Ex::call("len", vec![v.clone()]),
                        _ => self.kcall(&self.helper("cpCount", "", "CpCount"), vec![v.clone()]),
                    };
                    let mut tests = Vec::new();
                    if let Some(lo) = c.min {
                        tests.push(Ex::bin(Op::Lt, len.clone(), self.count_lit(lo.num)));
                    }
                    if let Some(hi) = c.max {
                        tests.push(Ex::bin(Op::Gt, len.clone(), self.count_lit(hi.num)));
                    }
                    if let Some(test) = tests.into_iter().reduce(|a, b| Ex::bin(Op::Or, a, b)) {
                        let shown = crate::value::range_text(c, &Ty::Num(NumTy::number()));
                        out.push(Node::If(vec![(test, vec![Node::Line(self.ret_found(self.outside_flagged(what.clone(), "chars", Some(len.clone()), &shown)))])], Vec::new()));
                    }
                }
            }
            // a result: the value it holds, where a parameter reaches it (`r.ok.qty`, DESIGN 26.3)
            Ty::Result(a, er) => {
                if !reached {
                    return out;
                }
                let at = |tag: &str| match self.t {
                    Target::Rust => self.rs_format(vec![Err(what.clone()), Ok(format!(".{tag}"))]),
                    _ => Ex::bin(Op::Add, what.clone(), self.string(&format!(".{tag}"))),
                };
                match self.t {
                    Target::Go => {
                        let k = self.imp(Import::Kaname);
                        let (ab, eb) = (self.boundary(a), self.boundary(er));
                        let x = Ex::atom(format!("r{depth}"));
                        let mut cases = Vec::new();
                        for (tag, ty, field, inner) in [("ok", "Ok", "Value", &**a), ("err", "Err", "Error", &**er)] {
                            let body = self.outside_body(&x.clone().dot(field), inner, None, None, at(tag), reach, depth + 1);
                            if !body.is_empty() {
                                cases.push((format!("{k}.{ty}[{ab}, {eb}]"), body));
                            }
                        }
                        if !cases.is_empty() {
                            out.push(Node::Switch(format!("switch {} := {}.(type)", x.print(self.t), v.print(self.t)), cases, Vec::new()));
                        }
                    }
                    // Rust's lent `Result`: what it holds, by `if let`
                    Target::Rust => {
                        let x = Ex::atom(format!("r{depth}"));
                        let mut arms = Vec::new();
                        for (tag, ctor, inner) in [("ok", "Ok", &**a), ("err", "Err", &**er)] {
                            let body = self.outside_body(&x, inner, None, None, at(tag), reach, depth + 1);
                            if !body.is_empty() {
                                let pat = if self.is_copy(inner) { format!("&{}", x.print(self.t)) } else { x.print(self.t) };
                                arms.push((Ex::atom(format!("let {ctor}({pat}) = {}", v.print(self.t))), body));
                            }
                        }
                        if !arms.is_empty() {
                            out.push(Node::If(arms, Vec::new()));
                        }
                    }
                    _ => {
                        let ok_body = self.outside_body(&v.clone().dot("value"), a, None, None, at("ok"), reach, depth + 1);
                        let err_body = self.outside_body(&v.clone().dot("error"), er, None, None, at("err"), reach, depth + 1);
                        let is = |ok: bool| match self.t {
                            Target::TypeScript => Ex::bin(Op::Eq, v.clone().dot("kind"), self.string(if ok { "ok" } else { "err" })),
                            _ => Ex::call("isinstance", vec![v.clone(), Ex::atom(format!("{}.{}", self.imp(Import::Kaname), if ok { "Ok" } else { "Err" }))]),
                        };
                        let mut arms = Vec::new();
                        if !ok_body.is_empty() {
                            arms.push((is(true), ok_body));
                        }
                        if !err_body.is_empty() {
                            arms.push((is(false), err_body));
                        }
                        if !arms.is_empty() {
                            out.push(Node::If(arms, Vec::new()));
                        }
                    }
                }
            }
            _ => {
                let written = match (range, t) {
                    (Some(r), _) => Some(r.clone()),
                    (None, Ty::Num(n)) if reached && n.is_rate() && n.step.is_some() => Some(Range { min: Some(Rat::zero()), max: Some(Rat::int(1)) }),
                    _ => None,
                };
                // where a parameter reaches it, an end not written is the type's (DESIGN 22.1, 22.5):
                // the message says it, and the code holds the value to it where the target's value can
                // be outside it
                let whole = if reached { crate::ty::filled(written.as_ref(), crate::ty::whole_range(t)) } else { written.clone() };
                if let Some(r) = &whole
                    && let Some(test) = self.range_test(v, t, written.as_ref(), r)
                {
                    let shown = self.shown(v, t);
                    let rt = crate::value::range_text(r, t);
                    out.push(Node::If(vec![(test, vec![Node::Line(self.ret_found(self.outside_value(what.clone(), shown, self.string(&rt), None)))])], Vec::new()));
                }
                if reached && self.record_checkable(t, depth, reach) {
                    let o = Ex::atom(format!("o{depth}"));
                    // a host's argument, held at every depth where it holds a value of a type that
                    // holds itself (DESIGN 28.1): the check of the entry, which goes into it
                    let deep = reach == Reach::Entry && self.p.reaches_recursive(t);
                    let call = match (t, Self::sum_of(t)) {
                        // a generic record or sum with its type arguments (DESIGN 26.7): the check of
                        // that type, made for it
                        (_, Some(sum)) if !sum.args.is_empty() => self.inst_check_call(&Ty::Enum(sum), v.clone(), what, reach),
                        (Ty::Record(id @ RecId::Module { args, .. }), None) if !args.is_empty() => self.inst_check_call(&Ty::Record(id.clone()), v.clone(), what, reach),
                        (_, Some(sum)) => self.sum_check_call(&sum, v.clone(), what, deep),
                        (Ty::Record(id), None) => self.record_check_call(id, v.clone(), what, deep),
                        _ => return out,
                    };
                    out.extend(self.if_found(&o, call, vec![Node::Line(self.ret_found(o.clone()))]));
                }
            }
        }
        out
    }

    /// The check of an element of a list, a value of a dict or a member of a set: one that may be none
    /// is checked where it is there.
    fn elem_body(&self, el: &Ex, x: &Ty, range: Option<&Range>, path: Ex, reach: Reach, depth: usize) -> Vec<Node> {
        match x {
            Ty::Opt(y) if self.t == Target::Rust => {
                let inner = Ex::atom(format!("x{}", depth + 1));
                let b = self.outside_body(&inner, y, range, None, path, reach, depth + 1);
                if b.is_empty() { b } else { vec![Node::If(vec![(Ex::atom(format!("let Some({}) = {}", inner.print(self.t), el.print(self.t))), b)], Vec::new())] }
            }
            Ty::Opt(y) => {
                let deref = if self.t == Target::Go { Ex::Deref(Box::new(el.clone())) } else { el.clone() };
                let b = self.outside_body(&deref, y, range, None, path, reach, depth + 1);
                if b.is_empty() { b } else { vec![Node::If(vec![(Ex::bin(Op::Ne, el.clone(), self.null()), b)], Vec::new())] }
            }
            y => self.outside_body(el, y, range, None, path, reach, depth + 1),
        }
    }

    /// A count written in a check, as the target compares it with a count: TypeScript's `bigint`.
    fn count_lit(&self, n: i128) -> Ex {
        match self.t {
            Target::TypeScript => Ex::atom(format!("{n}n")),
            _ => Ex::atom(n.to_string()),
        }
    }

    /// `if o := call; o != nil { … }`, and the same in TypeScript and Python.
    pub fn if_found(&self, o: &Ex, call: Ex, then: Vec<Node>) -> Vec<Node> {
        match self.t {
            Target::Go => vec![Node::If(vec![(Ex::atom(format!("{} := {}; {} != nil", o.print(self.t), call.print(self.t), o.print(self.t))), then)], Vec::new())],
            Target::TypeScript => vec![Node::Line(format!("const {} = {};", o.print(self.t), call.print(self.t))), Node::If(vec![(Ex::bin(Op::Ne, o.clone(), self.null()), then)], Vec::new())],
            Target::Python => vec![Node::Line(format!("{} = {}", o.print(self.t), call.print(self.t))), Node::If(vec![(Ex::atom(format!("{} is not None", o.print(self.t))), then)], Vec::new())],
            Target::Rust => vec![Node::If(vec![(Ex::atom(format!("let Some({}) = {}", o.print(self.t), call.print(self.t))), then)], Vec::new())],
        }
    }

    /// The path of a place as Rust's check passes it on (`what`, lent; a path made here, borrowed).
    pub fn what_arg(&self, what: &Ex) -> Ex {
        match what {
            Ex::Atom(a) if a == "what" => what.clone(),
            _ if self.t == Target::Rust => what.clone().borrow(),
            _ => what.clone(),
        }
    }

    /// `return o`, of what a check found: Rust's in `Some`.
    pub fn ret_found(&self, e: Ex) -> String {
        match self.t {
            Target::Rust => self.ret(Ex::call("Some", vec![e])),
            _ => self.ret(e),
        }
    }

    /// Rust's `format!` of the parts (DESIGN 36.8): a text as it is, a name inside the braces (as clippy
    /// asks), any other value by `{}`. A text alone is its literal.
    pub fn rs_format(&self, parts: Vec<Result<String, Ex>>) -> Ex {
        let mut fmt = String::new();
        let mut plain = String::new();
        let mut args = Vec::new();
        let mut filled = false;
        for p in parts {
            match p {
                Ok(s) => {
                    fmt.push_str(&s.replace('{', "{{").replace('}', "}}"));
                    plain.push_str(&s);
                }
                Err(Ex::Atom(a)) if plain_ident(&a) => {
                    fmt.push_str(&format!("{{{a}}}"));
                    filled = true;
                }
                Err(x) => {
                    fmt.push_str("{}");
                    args.push(x);
                    filled = true;
                }
            }
        }
        if !filled {
            return self.string(&plain);
        }
        let mut all = vec![Ex::Atom(ritsu_emit::lit::rust(&fmt))];
        all.extend(args);
        Ex::call("format!", all)
    }

    /// The check of a value of a generic record or sum with its type arguments (`Page[Invoice]`),
    /// reached from a parameter (DESIGN 26.7): a function of the module, made once for the type, that
    /// checks each field as the declaration's own check does (which a generic declaration has not).
    fn inst_check_call(&self, t: &Ty, v: Ex, what: Ex, reach: Reach) -> Ex {
        let deep = reach == Reach::Entry && self.p.reaches_recursive(t);
        let key = if deep { format!("inst-entry|{t:?}") } else { format!("inst|{t:?}") };
        let found = self.outsides.borrow().iter().find(|o| o.key == key).map(|o| o.name.clone());
        let name = match found {
            Some(n) => n,
            None => {
                let n = format!("_outside{}", self.outsides.borrow().len() + 1);
                self.outsides.borrow_mut().push(Outside { key: key.clone(), name: n.clone(), body: Vec::new(), ty: String::new(), tparams: String::new() });
                let body = self.inst_body(t, if deep { Reach::Entry } else { Reach::Call });
                let ty = if self.t == Target::Rust { self.lent_type(t, true) } else { self.boundary(t) };
                if let Some(o) = self.outsides.borrow_mut().iter_mut().find(|o| o.key == key) {
                    o.body = body;
                    o.ty = ty;
                }
                n
            }
        };
        Ex::call(name, vec![v, self.what_arg(&what)])
    }

    /// The body of the check of a value `v` of a generic record or sum with its type arguments: each
    /// field, by the check of its place with the type arguments put in (`what.items`, `what.done.value`).
    fn inst_body(&self, t: &Ty, reach: Reach) -> Vec<Node> {
        let v = Ex::atom("v");
        let field_checks = |x: &Ex, holder: &Ty, fields: &[(crate::ast::Name, Slot)], prefix: &str, n0: usize| -> Vec<Node> {
            let mut out = Vec::new();
            for (k, (f, slot)) in fields.iter().enumerate() {
                let Some(fun) = self.outside_fn(slot, reach) else { continue };
                let what = self.field_path(&format!("{prefix}.{}", f.text));
                let o = Ex::atom(format!("o{}", n0 + k + 1));
                out.extend(self.if_found(&o, Ex::call(fun, vec![self.lent_field(x, holder, f.ascii(), &slot.ty), what]), vec![Node::Line(self.ret_found(o.clone()))]));
            }
            out
        };
        match t {
            Ty::Record(id) => {
                let holder = t;
                field_checks(&v, holder, &self.p.fields(id), "", 0)
            }
            Ty::Enum(id) => {
                let Owner::Module(file) = &id.owner else { return Vec::new() };
                let mut arms: Vec<(String, Ex, Vec<Node>)> = Vec::new();
                let mut n0 = 0;
                for value in self.p.values(id).0.iter() {
                    let fields = self.p.value_fields(id, &value.text);
                    let r = RecId::Variant { file: file.clone(), name: id.name.clone(), value: value.text.clone(), args: id.args.clone() };
                    let x = if matches!(self.t, Target::Go | Target::Rust) { Ex::atom("x") } else { v.clone() };
                    let holder = Ty::Record(r.clone());
                    let body = field_checks(&x, &holder, &fields, &format!(".{}", value.text), n0);
                    n0 += fields.len();
                    if !body.is_empty() {
                        let test = match self.t {
                            Target::TypeScript => Ex::bin(Op::Eq, v.clone().dot("kind"), self.string(&value.text)),
                            Target::Rust => Ex::atom(format!("let {}(x) = v", self.variant_of(id, &value.text))),
                            _ => Ex::call("isinstance", vec![v.clone(), Ex::atom(self.record_type_bare(&r))]),
                        };
                        arms.push((self.record_type(&r), test, body));
                    }
                }
                if arms.is_empty() {
                    return Vec::new();
                }
                match self.t {
                    Target::Go => vec![Node::Switch("switch x := v.(type)".into(), arms.into_iter().map(|(ty, _, b)| (ty, b)).collect(), Vec::new())],
                    _ => vec![Node::If(arms.into_iter().map(|(_, test, b)| (test, b)).collect(), Vec::new())],
                }
            }
            _ => Vec::new(),
        }
    }

    // ── How deep a host's argument nests (DESIGN 28.1) ──────────────────────

    /// The function that finds, in a value of `t` a host gives, the first value of a record or an enum
    /// that holds itself nested more than [`crate::interp::NESTED`] deep, counted along each path
    /// (`interp.rs`'s `nested`): its name, made once; None where no value of `t` holds one. It takes
    /// the value and how many such values are over it, and answers the name of that value's type as it
    /// is declared, or none (Go's empty string).
    pub fn nest_fn(&self, t: &Ty) -> Option<String> {
        if !self.p.reaches_recursive(t) {
            return None;
        }
        let tparams = self.tparams_of(t);
        let key = format!("{t:?}|{tparams}");
        if let Some(o) = self.nests.borrow().iter().find(|o| o.key == key) {
            return Some(o.name.clone());
        }
        let name = format!("_nest{}", self.nests.borrow().len() + 1);
        // made first, so that the function of a type that holds itself can call itself
        self.nests.borrow_mut().push(Outside { key: key.clone(), name: name.clone(), body: Vec::new(), ty: String::new(), tparams });
        let body = self.nest_body(t);
        let ty = if self.t == Target::Rust { self.lent_type(t, true) } else { self.boundary(t) };
        if let Some(o) = self.nests.borrow_mut().iter_mut().find(|o| o.key == key) {
            o.body = body;
            o.ty = ty;
        }
        Some(name)
    }

    /// The path of a field of the value a check is given: `what + ".lines"`, Rust's borrowed
    /// `&format!("{what}.lines")`.
    pub fn field_path(&self, tail: &str) -> Ex {
        match self.t {
            Target::Rust => self.rs_format(vec![Err(Ex::atom("what")), Ok(tail.to_string())]).borrow(),
            _ => Ex::bin(Op::Add, Ex::atom("what"), self.string(tail)),
        }
    }

    /// A field of a value of `holder` as a check or a call takes it: Rust's lent (`&x.lines`,
    /// `x.memo.as_deref()`), out of its box (`&*x.left`, `x.next.as_deref()`).
    pub fn lent_field(&self, x: &Ex, holder: &Ty, alias: &str, ty: &Ty) -> Ex {
        let f = x.clone().dot(field_name(self.t, alias));
        if self.t != Target::Rust {
            return f;
        }
        let (f, held) = self.unboxed(holder, ty, f, Held::Kept);
        self.lent(&Val::held(f, ty.clone(), held))
    }

    /// `o = call`, and `then` where it found a value nested too deep.
    pub fn if_nested(&self, o: &Ex, call: Ex, then: Vec<Node>) -> Vec<Node> {
        match self.t {
            Target::Go => vec![Node::If(vec![(Ex::atom(format!("{} := {}; {} != \"\"", o.print(self.t), call.print(self.t), o.print(self.t))), then)], Vec::new())],
            _ => self.if_found(o, call, then),
        }
    }

    /// The body of the function of [`Gen::nest_fn`] for a value `v` of `t`, with `above` the values of
    /// types that hold themselves over it.
    fn nest_body(&self, t: &Ty) -> Vec<Node> {
        let v = Ex::atom("v");
        let above = Ex::atom("above");
        let o = Ex::atom("o");
        let back = |call: Ex| self.if_nested(&o, call, vec![Node::Line(self.ret_found(o.clone()))]);
        let mut out = Vec::new();
        match t {
            Ty::Opt(x) if self.t == Target::Rust => {
                let Some(f) = self.nest_fn(x) else { return out };
                out.push(Node::If(vec![(Ex::atom("let Some(x) = v"), back(Ex::call(f, vec![Ex::atom("x"), above])))], Vec::new()));
            }
            Ty::Opt(x) => {
                let Some(f) = self.nest_fn(x) else { return out };
                let inner = if self.t == Target::Go { Ex::Deref(Box::new(v.clone())) } else { v.clone() };
                out.push(Node::If(vec![(Ex::bin(Op::Ne, v.clone(), self.null()), back(Ex::call(f, vec![inner, above])))], Vec::new()));
            }
            Ty::List(x) | Ty::Set(x) => {
                let Some(f) = self.nest_fn(x) else { return out };
                let head = match self.t {
                    Target::TypeScript => "for (const e of v)".to_string(),
                    Target::Python => "for e in v".to_string(),
                    Target::Go => "for _, e := range v".to_string(),
                    Target::Rust => "for e in v".to_string(),
                };
                out.push(Node::Block(head, back(Ex::call(f, vec![Ex::atom("e"), above])), Vec::new()));
            }
            // Rust's `BTreeMap` walks its values in the order of its keys
            Ty::Dict(_, x) if self.t == Target::Rust => {
                let Some(f) = self.nest_fn(x) else { return out };
                out.push(Node::Block("for e in v.values()".to_string(), back(Ex::call(f, vec![Ex::atom("e"), above])), Vec::new()));
            }
            // a dict's values, in the order of its keys, as the interpreter holds its pairs
            Ty::Dict(k, x) => {
                let Some(f) = self.nest_fn(x) else { return out };
                let keys = match self.t {
                    Target::TypeScript => self.kcall("sortedKeys", vec![v.clone(), self.order_fn(k, false)]),
                    Target::Python => self.kcall("keys_of", vec![v.clone(), self.py_key_fn(k)]),
                    Target::Go => self.kcall("SortedKeys", vec![v.clone(), self.order_fn(k, false)]),
                    Target::Rust => unreachable!("Rust walks a map's values above"),
                };
                let (head, value) = match self.t {
                    Target::TypeScript => (format!("for (const k of {})", keys.print(self.t)), Ex::atom(format!("v.get(k) as {}", self.boundary(x)))),
                    Target::Python => (format!("for k in {}", keys.print(self.t)), Ex::atom("v[k]")),
                    Target::Go => (format!("for _, k := range {}", keys.print(self.t)), Ex::atom("v[k]")),
                    Target::Rust => unreachable!("Rust walks a map's values above"),
                };
                out.push(Node::Block(head, back(Ex::call(f, vec![value, above])), Vec::new()));
            }
            // a result: the value its `ok` or its `err` holds
            Ty::Result(a, e) => {
                let (fa, fe) = (self.nest_fn(a), self.nest_fn(e));
                match self.t {
                    Target::Go => {
                        let k = self.imp(Import::Kaname);
                        let (ab, eb) = (self.boundary(a), self.boundary(e));
                        let mut cases = Vec::new();
                        for (ty, field, f) in [("Ok", "Value", fa), ("Err", "Error", fe)] {
                            if let Some(f) = f {
                                cases.push((format!("{k}.{ty}[{ab}, {eb}]"), back(Ex::call(f, vec![Ex::atom("r").dot(field), above.clone()]))));
                            }
                        }
                        out.push(Node::Switch("switch r := v.(type)".into(), cases, Vec::new()));
                    }
                    Target::Rust => {
                        let mut arms = Vec::new();
                        if let Some(f) = fa {
                            arms.push((Ex::atom("let Ok(r) = v"), back(Ex::call(f, vec![Ex::atom("r"), above.clone()]))));
                        }
                        if let Some(f) = fe {
                            arms.push((Ex::atom("let Err(r) = v"), back(Ex::call(f, vec![Ex::atom("r"), above.clone()]))));
                        }
                        out.push(Node::If(arms, Vec::new()));
                    }
                    _ => {
                        let is = |ok: bool| match self.t {
                            Target::TypeScript => Ex::bin(Op::Eq, v.clone().dot("kind"), self.string(if ok { "ok" } else { "err" })),
                            _ => Ex::call("isinstance", vec![v.clone(), Ex::atom(format!("{}.{}", self.imp(Import::Kaname), if ok { "Ok" } else { "Err" }))]),
                        };
                        let mut arms = Vec::new();
                        if let Some(f) = fa {
                            arms.push((is(true), back(Ex::call(f, vec![v.clone().dot("value"), above.clone()]))));
                        }
                        if let Some(f) = fe {
                            arms.push((is(false), back(Ex::call(f, vec![v.clone().dot("error"), above.clone()]))));
                        }
                        out.push(Node::If(arms, Vec::new()));
                    }
                }
            }
            Ty::Record(_) | Ty::Enum(_) => {
                // a value of a type that holds itself is one deeper than those over it
                let here = if self.p.recursive(t) {
                    let name = match t {
                        Ty::Enum(id) => id.name.clone(),
                        Ty::Record(RecId::Module { name, .. } | RecId::Variant { name, .. }) => name.clone(),
                        _ => String::new(),
                    };
                    let decl = match self.t {
                        Target::TypeScript => "const here = above + 1;".to_string(),
                        Target::Python => "here = above + 1".to_string(),
                        Target::Go => "here := above + 1".to_string(),
                        Target::Rust => "let here = above + 1;".to_string(),
                    };
                    out.push(Node::Line(decl));
                    out.push(Node::If(vec![(Ex::bin(Op::Gt, Ex::atom("here"), Ex::atom(crate::interp::NESTED.to_string())), vec![Node::Line(self.ret_found(self.string(&name)))])], Vec::new()));
                    Ex::atom("here")
                } else {
                    above.clone()
                };
                // the fields that can hold such a value: each by its own function
                let fields_of = |x: &Ex, holder: &Ty, fields: &[(crate::ast::Name, Slot)]| -> Vec<Node> {
                    let mut body = Vec::new();
                    for (k, (f, s)) in fields.iter().enumerate() {
                        if let Some(nf) = self.nest_fn(&s.ty) {
                            let o = Ex::atom(format!("o{}", k + 1));
                            body.extend(self.if_nested(&o, Ex::call(nf, vec![self.lent_field(x, holder, f.ascii(), &s.ty), here.clone()]), vec![Node::Line(self.ret_found(o.clone()))]));
                        }
                    }
                    body
                };
                match (t, Self::sum_of(t)) {
                    (Ty::Record(id @ RecId::Module { .. }), _) => out.extend(fields_of(&v, t, &self.p.fields(id))),
                    (Ty::Record(id @ RecId::Variant { .. }), _) => out.extend(fields_of(&v, t, &self.p.fields(id))),
                    (Ty::Enum(_), Some(id)) => {
                        let Owner::Module(file) = &id.owner else { return out };
                        let mut arms: Vec<(String, Ex, Vec<Node>)> = Vec::new();
                        for value in self.p.values(&id).0.iter() {
                            let r = RecId::Variant { file: file.clone(), name: id.name.clone(), value: value.text.clone(), args: id.args.clone() };
                            let x = if matches!(self.t, Target::Go | Target::Rust) { Ex::atom("x") } else { v.clone() };
                            let body = fields_of(&x, &Ty::Record(r.clone()), &self.p.value_fields(&id, &value.text));
                            if body.is_empty() {
                                continue;
                            }
                            let test = match self.t {
                                Target::TypeScript => Ex::bin(Op::Eq, v.clone().dot("kind"), self.string(&value.text)),
                                Target::Rust => Ex::atom(format!("let {}(x) = v", self.variant_of(&id, &value.text))),
                                _ => Ex::call("isinstance", vec![v.clone(), Ex::atom(self.record_type_bare(&r))]),
                            };
                            arms.push((self.record_type(&r), test, body));
                        }
                        if !arms.is_empty() {
                            out.push(match self.t {
                                Target::Go => Node::Switch("switch x := v.(type)".into(), arms.into_iter().map(|(ty, _, b)| (ty, b)).collect(), Vec::new()),
                                _ => Node::If(arms.into_iter().map(|(_, test, b)| (test, b)).collect(), Vec::new()),
                            });
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        out
    }

    /// The code that fails a host's argument `param` of `fun`, at `line`, nested too deep: `n` the name
    /// of the type of the value it found (DESIGN 28.1, `interp.rs`'s `check_entry_with`).
    pub fn fail_nested(&self, line: usize, fun: &str, param: &str, n: &Ex) -> Vec<Node> {
        let most = crate::interp::NESTED;
        let text = tr!("関数 `{fun}` の引数 `{param}` は、`{P}` の入れ子が {most} 段を超えています", "The argument `{param}` of `{fun}` nests `{P}` more than {most} deep");
        if self.t == Target::Rust {
            let msg = self.rs_format(template(&text, self.lang).into_iter().map(|p| p.map_err(|_| n.clone())).collect());
            return vec![self.fail_line("input", line, msg)];
        }
        let msg = template(&text, self.lang)
            .into_iter()
            .map(|p| match p {
                Ok(s) => self.string(&s),
                Err(_) => n.clone(),
            })
            .reduce(|a, b| Ex::bin(Op::Add, a, b))
            .unwrap_or(self.string(""));
        vec![self.fail_line("input", line, msg)]
    }

    /// The check of a record of a module, reached from a parameter: `deep` the check of a host's
    /// argument that goes into the values of types that hold themselves (`_enter_Category`, Go's
    /// `KanameEnter`, DESIGN 28.1), else `_outside_Line` (Go's `KanameOutside`).
    pub fn record_check_call(&self, id: &RecId, v: Ex, what: Ex, deep: bool) -> Ex {
        let RecId::Module { file, name, .. } = id else { return self.null() };
        let m = self.p.module(file);
        let (alias, private) = m.decls.records.iter().find(|r| r.name.text == *name).map(|r| (r.name.ascii().to_string(), r.private)).unwrap_or_default();
        match self.t {
            Target::Go => Ex::Call(Box::new(v.dot(if deep { "KanameEnter" } else { "KanameOutside" })), vec![what]),
            // the method of the type, as Go's (DESIGN 36.5)
            Target::Rust => v.method(if deep { "kaname_enter" } else { "kaname_outside" }, vec![self.what_arg(&what)]),
            _ => {
                let f = format!("{}{}", if deep { "_enter_" } else { "_outside_" }, hidden(type_name(self.t, &alias), private));
                let f = if *file == self.c.canon { f } else { format!("{}.{f}", self.imp(Import::Module(file.clone()))) };
                Ex::call(f, vec![v, what])
            }
        }
    }

    /// The check of a value of a sum (or of the type of one of its values), reached from a parameter:
    /// `_outside_Order(v, what)` in TypeScript and Python, the method of the value in Go.
    pub fn sum_check_call(&self, id: &EnumId, v: Ex, what: Ex, deep: bool) -> Ex {
        let Owner::Module(file) = &id.owner else { return self.null() };
        match self.t {
            Target::Go => Ex::Call(Box::new(v.dot(if deep { "KanameEnter" } else { "KanameOutside" })), vec![what]),
            Target::Rust => v.method(if deep { "kaname_enter" } else { "kaname_outside" }, vec![self.what_arg(&what)]),
            _ => {
                let m = self.p.module(file);
                let (alias, private) = m.decls.enums.iter().find(|e| e.name.text == id.name).map(|e| (e.name.ascii().to_string(), e.private)).unwrap_or_default();
                let f = format!("{}{}", if deep { "_enter_" } else { "_outside_" }, hidden(type_name(self.t, &alias), private));
                let f = if *file == self.c.canon { f } else { format!("{}.{f}", self.imp(Import::Module(file.clone()))) };
                Ex::call(f, vec![v, what])
            }
        }
    }

    /// An `Outside` of a string, flagged `chars` (outside its count, `count` the code points) or `bad`
    /// (not well formed).
    fn outside_flagged(&self, path: Ex, flag: &str, count: Option<Ex>, range: &str) -> Ex {
        let k = self.imp(Import::Kaname);
        let (shown, range) = (self.string(""), self.string(range));
        match self.t {
            Target::TypeScript => Ex::Record(String::new(), vec![("path".into(), path), ("shown".into(), shown), ("range".into(), range), ("count".into(), count.map(|c| Ex::call("Number", vec![c])).unwrap_or(self.null())), (flag.into(), Ex::atom("true"))]),
            Target::Python => Ex::Record(format!("{k}.Outside"), vec![("path".into(), path), ("shown".into(), shown), ("range".into(), range), ("count".into(), count.unwrap_or(self.null())), (flag.into(), Ex::atom("True"))]),
            Target::Go => {
                let f = if flag == "bad" { "Bad" } else { "Chars" };
                Ex::atom(format!(
                    "&{}",
                    Ex::Record(format!("{k}.Outside"), vec![("Path".into(), path), ("Shown".into(), shown), ("Range".into(), range), ("Count".into(), count.map(|c| Ex::call("int", vec![c])).unwrap_or(Ex::atom("-1"))), (f.into(), Ex::atom("true"))]).print(self.t)
                ))
            }
            // a string of Rust's is well formed: only its count is outside
            Target::Rust => Ex::call(format!("{k}::outside_chars"), vec![path, count.unwrap_or(Ex::atom("0")), range]),
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
            Target::Rust => match count {
                Some(c) => Ex::call(format!("{k}::outside_count"), vec![path, c, range]),
                None => Ex::call(format!("{k}::outside"), vec![path, shown, range]),
            },
        }
    }

    pub fn ret(&self, e: Ex) -> String {
        match self.t {
            Target::TypeScript | Target::Rust => format!("return {};", e.print(self.t)),
            _ => format!("return {}", e.print(self.t)),
        }
    }

    /// Rust: the return of the answer `e` of a function or a closure, in `Ok` where it can fail; an
    /// answer that is itself the value of a call that can fail (`f(x)?`) is that call's `Result`
    /// (clippy's `needless_question_mark`).
    pub fn ret_rs(&self, e: Ex, fallible: bool) -> String {
        match e {
            Ex::Try(inner) if fallible => self.ret(*inner),
            e if fallible => self.ret(Ex::call("Ok", vec![e])),
            e => self.ret(e),
        }
    }

    /// Rust: the walk of a list's elements by reference: an array where the list is written out
    /// (clippy's `useless_vec`), else `x.iter()`.
    pub fn iter_rs(&self, x: &Ex) -> String {
        match x {
            Ex::List(_, xs) if !xs.is_empty() => format!("[{}].iter()", xs.iter().map(|a| a.print(Target::Rust)).collect::<Vec<_>>().join(", ")),
            _ => x.clone().method("iter", vec![]).print(Target::Rust),
        }
    }

    /// The length of a list.
    pub fn len_of(&self, v: &Ex) -> Ex {
        match self.t {
            Target::TypeScript => v.clone().dot("length"),
            Target::Python => Ex::call("len", vec![v.clone()]),
            Target::Go => Ex::call("len", vec![v.clone()]),
            Target::Rust => v.clone().method("len", vec![]),
        }
    }

    /// The test of a value as it travels against the range `whole` a place holds it to, `written` the
    /// ends the place writes (the others are the type's, DESIGN 22.5): a number to every end in
    /// TypeScript and Python, to the ends written in Go (its int64 holds no other); a date to the ends
    /// written, and to be a day of the calendar, whether the ends are written or not. TypeScript's
    /// date is a string and Go's and Rust's a `Date` of any year, month and day, held to be a day of
    /// 0001-01-01 to 9999-12-31; where both ends are written, Rust holds it to the calendar alone,
    /// the ends holding the year. The day number of 2026-02-30 is that of 2026-03-02, and falls
    /// between the ends. Python's date is a day of the calendar.
    fn range_test(&self, v: &Ex, t: &Ty, written: Option<&Range>, whole: &Range) -> Option<Ex> {
        match t {
            Ty::Num(n) if !n.is_rate() && matches!(self.t, Target::Go | Target::Rust) => written.and_then(|r| self.out_of(v, t, r)),
            Ty::Num(_) => self.out_of(v, t, whole),
            Ty::Date => {
                let compared = written.and_then(|r| self.out_of(v, t, r));
                let open = written.is_none_or(|r| r.min.is_none() || r.max.is_none());
                let day = match self.t {
                    Target::TypeScript => Some(Ex::Not(Box::new(self.kcall("isDate", vec![v.clone()])))),
                    Target::Go => Some(Ex::Not(Box::new(Ex::Call(Box::new(v.clone().dot("Valid")), vec![])))),
                    Target::Rust if open => Some(Ex::Not(Box::new(v.clone().method("valid", vec![])))),
                    Target::Rust => Some(Ex::Not(Box::new(v.clone().method("on_calendar", vec![])))),
                    _ => None,
                };
                match (day, compared) {
                    (Some(d), Some(c)) => Some(Ex::bin(Op::Or, d, c)),
                    (d, c) => d.or(c),
                }
            }
            _ => None,
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
                    Target::Rust if n.unit.type_name().is_some() && !n.is_rate() => v.clone().dot("0"),
                    _ => v.clone(),
                };
                // Rust: a range of one value, as one comparison (clippy's `double_comparisons`)
                if let (Target::Rust, Some(lo), Some(hi)) = (self.t, lo, hi)
                    && lo == hi
                {
                    return Some(Ex::bin(Op::Ne, x, self.int(lo)));
                }
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
                    Target::Rust => v.clone().method("days", vec![]),
                    _ => v.clone(),
                };
                let end = |d: Rat| match self.t {
                    Target::Go | Target::Rust => self.int(d.num),
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
                    Target::Rust if n.unit.type_name().is_some() => v.clone().dot("0"),
                    _ => v.clone(),
                };
                self.kcall(&self.helper("numText", "num_text", "IntText"), vec![if matches!(self.t, Target::Go | Target::Rust) { x } else { self.to_q_ex(x) }, self.string(&unit_suffix(&n.unit))])
            }
            Ty::Date => match self.t {
                Target::TypeScript => v.clone(),
                Target::Python => self.kcall("date_text", vec![v.clone()]),
                Target::Go => Ex::Call(Box::new(v.clone().dot("String")), vec![]),
                Target::Rust => v.clone().method("to_string", vec![]),
            },
            _ => self.string(""),
        }
    }

    /// A whole number as a fraction.
    pub fn to_q_ex(&self, x: Ex) -> Ex {
        match self.t {
            Target::TypeScript => self.kcall("q", vec![x]),
            Target::Python => self.kcall("Q", vec![x]),
            Target::Go | Target::Rust => self.kcall("Of", vec![x]),
        }
    }

    /// A fraction whose type says it is whole, as a whole number.
    pub fn to_int_ex(&self, x: Ex) -> Ex {
        self.kcall(&self.helper("whole", "whole", "Whole"), vec![x])
    }

    /// The code that fails with a message the code fills in from the `Outside` `o` it found, a check
    /// of a value of `slot` as `reach` holds it: the messages its check can need.
    pub fn fail_outside(&self, kind: &str, line: usize, said: &Said, o: &Ex, slot: &Slot, reach: Reach) -> Vec<Node> {
        let counts = self.counts(slot, reach);
        let (chars, bad) = self.strings(slot, reach);
        let Texts { count, value, chars: chars_text, bad: bad_text } = said.texts();
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
                (Target::Rust, "count") => self.kcall("itoa", vec![x]),
                _ => x,
            }
        };
        let build = |tmpl: &Text| -> Ex {
            if self.t == Target::Rust {
                return self.rs_format(template(tmpl, self.lang).into_iter().map(|p| p.map_err(&part)).collect());
            }
            template(tmpl, self.lang).into_iter().map(|p| match p {
                Ok(s) => self.string(&s),
                Err(k) => part(k),
            })
            .reduce(|a, b| Ex::bin(Op::Add, a, b))
            .unwrap_or(self.string(""))
        };
        let fail = |msg: Ex| -> Node { self.fail_line(kind, line, msg) };
        let is_count = match self.t {
            Target::TypeScript => Ex::bin(Op::Ne, o.clone().dot("count"), self.null()),
            Target::Python => Ex::atom(format!("{}.count is not None", o.print(self.t))),
            Target::Go => Ex::bin(Op::Ge, o.clone().dot("Count"), Ex::atom("0")),
            Target::Rust => o.clone().dot("count").method("is_some", vec![]),
        };
        // a flag of a string's `Outside`: `chars`, `bad`
        let flagged = |f: &str| -> Ex {
            match self.t {
                Target::TypeScript => Ex::bin(Op::Eq, o.clone().dot(f), Ex::atom("true")),
                Target::Python | Target::Rust => o.clone().dot(f),
                Target::Go => o.clone().dot(if f == "bad" { "Bad" } else { "Chars" }),
            }
        };
        // a string alone: not well formed, or past its count of code points; nothing else is outside
        let mut ty = &slot.ty;
        while let Ty::Opt(x) = ty {
            ty = x;
        }
        if matches!(ty, Ty::Str) {
            return match (bad, chars) {
                (true, true) => vec![Node::If(vec![(flagged("bad"), vec![fail(build(&bad_text))])], Vec::new()), fail(build(&chars_text))],
                (true, false) => vec![fail(build(&bad_text))],
                _ => vec![fail(build(&chars_text))],
            };
        }
        let mut out = Vec::new();
        if bad {
            out.push(Node::If(vec![(flagged("bad"), vec![fail(build(&bad_text))])], Vec::new()));
        }
        // a count outside: a string's code points where the check flags it so, else a list's elements
        match (counts, chars) {
            (true, true) => out.push(Node::If(vec![(is_count, vec![Node::If(vec![(flagged("chars"), vec![fail(build(&chars_text))])], Vec::new()), fail(build(&count))])], Vec::new())),
            (true, false) => out.push(Node::If(vec![(is_count, vec![fail(build(&count))])], Vec::new())),
            (false, true) => out.push(Node::If(vec![(is_count, vec![fail(build(&chars_text))])], Vec::new())),
            (false, false) => {}
        }
        out.push(fail(build(&value)));
        out
    }

    /// An expression as a statement of the target.
    pub fn stmt(&self, e: Ex) -> String {
        match self.t {
            Target::TypeScript | Target::Rust => format!("{};", e.print(self.t)),
            _ => e.print(self.t),
        }
    }

    /// The statement that fails with `kind` at `line` of the module and the message `msg`: a call of
    /// the helper's `fail` (a throw, a raise, a panic), and Rust's return of the error (DESIGN 36.5).
    pub fn fail_line(&self, kind: &str, line: usize, msg: Ex) -> Node {
        let call = self.kcall(&self.helper("fail", "fail", "Fail"), vec![self.string(kind), self.module_const(), Ex::atom(line.to_string()), msg]);
        match self.t {
            Target::Rust => Node::Line(format!("return Err({});", call.print(self.t))),
            _ => Node::Line(self.stmt(call)),
        }
    }

    /// Whether values of `t` are compared whole (DESIGN 23.5): a value that may be none, a list, a
    /// record, a value of a sum, a transfer.
    pub fn whole(&self, t: &Ty) -> bool {
        match t {
            Ty::Opt(_) | Ty::None | Ty::List(_) | Ty::Empty | Ty::Record(_) | Ty::Transfer(_) | Ty::Dict(..) | Ty::Set(_) | Ty::EmptyDict | Ty::Result(..) | Ty::Param(_) => true,
            Ty::Enum(id) => self.p.is_sum(id),
            _ => false,
        }
    }

    /// Whether TypeScript's `===` compares two values of `t` as they are worked on: a whole number, a
    /// bool, a string, a date, a value of an enum with no fields, and a value that may be none of
    /// those.
    fn ts_primitive(&self, t: &Ty) -> bool {
        match t {
            Ty::Num(n) => rep(n) == Rep::Int,
            Ty::Bool | Ty::Str | Ty::Date | Ty::None => true,
            Ty::Enum(id) => !self.p.is_sum(id),
            Ty::Opt(x) => self.ts_primitive(x),
            _ => false,
        }
    }

    /// Whether Go's `==` compares two values of `t`, as they are worked on, or as they travel (in a
    /// record, `boundary`): no pointer, slice, interface or fraction in it.
    fn go_comparable(&self, t: &Ty, boundary: bool) -> bool {
        match t {
            Ty::Num(n) => boundary || rep(n) == Rep::Int,
            Ty::Bool | Ty::Str | Ty::Date => true,
            Ty::Enum(id) => !self.p.is_sum(id),
            _ => false,
        }
    }

    /// Whether two values `a` and `b` of `t` (as they are worked on) are the same.
    pub fn eq_ex(&self, a: Ex, b: Ex, t: &Ty) -> Ex {
        let sum = match t {
            Ty::Enum(id) => self.p.is_sum(id),
            Ty::Record(r) => r.sum().is_some(),
            Ty::Result(..) => true,
            _ => false,
        };
        match self.t {
            // a value of a sum an `isinstance` has narrowed: `kaname.eq`, which mypy lets compare
            Target::Python if sum => self.kcall("eq", vec![a, b]),
            Target::Python => Ex::bin(Op::Eq, a, b),
            Target::TypeScript if self.ts_primitive(t) => Ex::bin(Op::Eq, a, b),
            Target::TypeScript => self.kcall("eq", vec![a, b]),
            Target::Go => self.go_eq(a, b, t, false),
            // derived `PartialEq`, by value; the two sides held alike (`compare`, DESIGN 36.4)
            Target::Rust => Ex::bin(Op::Eq, a, b),
        }
    }

    /// Go: whether two values of `t` are the same, as they are worked on or (`boundary`) as they
    /// travel.
    fn go_eq(&self, a: Ex, b: Ex, t: &Ty, boundary: bool) -> Ex {
        // a value of a type parameter, and what holds one: by its contents, whatever its type is
        // (DESIGN 26.7)
        if t.has_param() {
            return self.kcall("Equal", vec![a, b]);
        }
        if self.go_comparable(t, boundary) {
            return Ex::bin(Op::Eq, a, b);
        }
        if let Ty::Num(_) = t {
            return Ex::bin(Op::Eq, self.kcall("Cmp", vec![a, b]), Ex::atom("0"));
        }
        Ex::call(self.eq_fn(t, boundary), vec![a, b])
    }

    /// Go's function that tells whether two values of `t` are the same, made once (DESIGN 23.8).
    fn eq_fn(&self, t: &Ty, boundary: bool) -> String {
        let key = format!("{t:?}|{boundary}");
        if let Some((_, n, _)) = self.eqs.borrow().iter().find(|(k, _, _)| *k == key) {
            return n.clone();
        }
        let n = format!("_eq{}", self.eqs.borrow().len() + 1);
        self.eqs.borrow_mut().push((key.clone(), n.clone(), String::new()));
        let def = self.eq_def(&n, t, boundary);
        if let Some(x) = self.eqs.borrow_mut().iter_mut().find(|(k, _, _)| *k == key) {
            x.2 = def;
        }
        n
    }

    fn eq_def(&self, n: &str, t: &Ty, boundary: bool) -> String {
        let ty = if boundary { self.boundary(t) } else { self.flight(t) };
        let ret = |body: String| format!("func {n}(a, b {ty}) bool {{\n{body}}}\n");
        let both = |parts: Vec<Ex>| -> String {
            let e = parts.into_iter().reduce(|x, y| Ex::bin(Op::And, x, y)).unwrap_or(Ex::atom("true"));
            format!("\treturn {}\n", e.print(Target::Go))
        };
        match t {
            Ty::Opt(x) => {
                let inner = self.go_eq(Ex::Deref(Box::new(Ex::atom("a"))), Ex::Deref(Box::new(Ex::atom("b"))), x, boundary);
                ret(format!("\tif a == nil || b == nil {{\n\t\treturn a == b\n\t}}\n\treturn {}\n", inner.print(Target::Go)))
            }
            Ty::List(x) => {
                let el = |v: &str| Ex::Index(Box::new(Ex::atom(v)), Box::new(Ex::atom("i")));
                let inner = self.go_eq(el("a"), el("b"), x, boundary);
                ret(format!(
                    "\tif len(a) != len(b) {{\n\t\treturn false\n\t}}\n\tfor i := range a {{\n\t\tif !({}) {{\n\t\t\treturn false\n\t\t}}\n\t}}\n\treturn true\n",
                    inner.print(Target::Go)
                ))
            }
            Ty::Record(id @ (RecId::Module { .. } | RecId::Variant { .. })) => {
                let parts = self.p.fields(id).iter().map(|(f, s)| self.go_eq(Ex::atom("a").dot(field_name(Target::Go, f.ascii())), Ex::atom("b").dot(field_name(Target::Go, f.ascii())), &s.ty, true)).collect();
                ret(both(parts))
            }
            // a dict: the same keys, each with the same value as it travels; a set: the same members
            // (DESIGN 24.2)
            Ty::Dict(_, x) => {
                let inner = self.go_eq(Ex::atom("x"), Ex::atom("y"), x, true);
                ret(format!(
                    "\tif len(a) != len(b) {{\n\t\treturn false\n\t}}\n\tfor k, x := range a {{\n\t\ty, ok := b[k]\n\t\tif !ok || !({}) {{\n\t\t\treturn false\n\t\t}}\n\t}}\n\treturn true\n",
                    inner.print(Target::Go)
                ))
            }
            Ty::Set(_) => ret("\tif len(a) != len(b) {\n\t\treturn false\n\t}\n\tfor k := range a {\n\t\tif _, ok := b[k]; !ok {\n\t\t\treturn false\n\t\t}\n\t}\n\treturn true\n".into()),
            Ty::Record(RecId::Entry(k, v)) => {
                let parts = vec![self.go_eq(Ex::atom("a").dot("Key"), Ex::atom("b").dot("Key"), k, true), self.go_eq(Ex::atom("a").dot("Value"), Ex::atom("b").dot("Value"), v, true)];
                ret(both(parts))
            }
            Ty::Record(RecId::Indexed(x)) => {
                let parts = vec![Ex::bin(Op::Eq, Ex::atom("a").dot("Index"), Ex::atom("b").dot("Index")), self.go_eq(Ex::atom("a").dot("Item"), Ex::atom("b").dot("Item"), x, true)];
                ret(both(parts))
            }
            Ty::Record(RecId::Running(item, w)) => {
                let mut parts = vec![self.go_eq(Ex::atom("a").dot("Item"), Ex::atom("b").dot("Item"), item, true)];
                for f in ["Before", "Upto", "Total"] {
                    parts.push(self.go_eq(Ex::atom("a").dot(f), Ex::atom("b").dot(f), w, true));
                }
                ret(both(parts))
            }
            Ty::Record(RecId::Element(file, seq)) => {
                let cols = self.p.rules.get(file).and_then(|r| r.elements.iter().find(|(n, _, _)| n == seq).map(|(_, _, c)| c.clone())).unwrap_or_default();
                let parts = cols.iter().map(|c| rule_col_eq(&ritsu_emit::ident::pascal(&c.alias), &c.ty)).collect();
                ret(both(parts))
            }
            Ty::Record(RecId::Outputs(file)) => {
                let Some(facts) = self.p.rules.get(file).cloned() else { return ret("\treturn a == b\n".into()) };
                if facts.outputs.len() == 1 {
                    let e = match &facts.outputs[0].ty {
                        ColumnType::Opt(_) => Ex::atom("(a == nil) == (b == nil) && (a == nil || *a == *b)"),
                        _ => Ex::bin(Op::Eq, Ex::atom("a"), Ex::atom("b")),
                    };
                    return ret(format!("\treturn {}\n", e.print(Target::Go)));
                }
                let call = self.call_of(&facts);
                let parts = facts.outputs.iter().zip(&call.outputs).map(|(c, o)| rule_col_eq(&o.alias, &c.ty)).collect();
                ret(both(parts))
            }
            Ty::Enum(id) => {
                // a value of a sum: the same value, and its fields the same
                let mut cases = String::new();
                let Owner::Module(file) = &id.owner else { return ret("\treturn a == b\n".into()) };
                for v in self.p.values(id).0 {
                    let vt = self.record_type(&RecId::Variant { file: file.clone(), name: id.name.clone(), value: v.text.clone(), args: id.args.clone() });
                    let fields = self.p.value_fields(id, &v.text);
                    if fields.is_empty() {
                        cases.push_str(&format!("\tcase {vt}:\n\t\t_, ok := b.({vt})\n\t\treturn ok\n"));
                    } else {
                        let parts: Vec<Ex> = fields.iter().map(|(f, s)| self.go_eq(Ex::atom("x").dot(field_name(Target::Go, f.ascii())), Ex::atom("y").dot(field_name(Target::Go, f.ascii())), &s.ty, true)).collect();
                        let e = parts.into_iter().reduce(|p, q| Ex::bin(Op::And, p, q)).unwrap_or(Ex::atom("true"));
                        cases.push_str(&format!("\tcase {vt}:\n\t\ty, ok := b.({vt})\n\t\treturn ok && {}\n", e.print(Target::Go)));
                    }
                }
                ret(format!("\tswitch x := a.(type) {{\n{cases}\t}}\n\treturn false\n"))
            }
            // a result: the same value, `ok` or `err`, and the same in it (DESIGN 26.3)
            Ty::Result(x, y) => {
                let k = self.imp(Import::Kaname);
                let (xb, yb) = (self.boundary(x), self.boundary(y));
                let ok_eq = self.go_eq(Ex::atom("p").dot("Value"), Ex::atom("q").dot("Value"), x, true);
                let err_eq = self.go_eq(Ex::atom("p").dot("Error"), Ex::atom("q").dot("Error"), y, true);
                ret(format!(
                    "\tswitch p := a.(type) {{\n\tcase {k}.Ok[{xb}, {yb}]:\n\t\tq, ok := b.({k}.Ok[{xb}, {yb}])\n\t\treturn ok && {}\n\tcase {k}.Err[{xb}, {yb}]:\n\t\tq, ok := b.({k}.Err[{xb}, {yb}])\n\t\treturn ok && {}\n\t}}\n\treturn false\n",
                    ok_eq.print(Target::Go),
                    err_eq.print(Target::Go)
                ))
            }
            Ty::Transfer(tid) => {
                let Some(book) = self.p.books.get(&tid.book).cloned() else { return ret("\treturn a == b\n".into()) };
                let names = super::books::param_names(Target::Go, &book, &tid.transfer);
                let parts = super::books::op_params(&book, &tid.transfer, &tid.op)
                    .iter()
                    .map(|(i, optional)| {
                        let f = &names[*i];
                        if *optional { Ex::atom(format!("(a.{f} == nil) == (b.{f} == nil) && (a.{f} == nil || *a.{f} == *b.{f})")) } else { Ex::bin(Op::Eq, Ex::atom(format!("a.{f}")), Ex::atom(format!("b.{f}"))) }
                    })
                    .collect();
                ret(both(parts))
            }
            _ => ret("\treturn a == b\n".into()),
        }
    }

    /// The function that gives a value of an enum its place in its order, made once.
    pub fn rank_of(&self, id: &EnumId) -> String {
        if let Some((_, n)) = self.ranks.borrow().iter().find(|(k, _)| k.same_decl(id)) {
            return n.clone();
        }
        let n = format!("_rank{}", self.ranks.borrow().len() + 1);
        self.ranks.borrow_mut().push((id.clone(), n.clone()));
        n
    }

    // ── Orders of keys (DESIGN 24.1, 24.6) ──────────────────────────────────

    /// TypeScript's and Go's function that compares two values of `t` (a type that can be a key, or a
    /// record of them), as they are worked on (`flight`) or as they travel (in a dict, a set, a
    /// record): below 0, 0 or above 0.
    pub fn order_fn(&self, t: &Ty, flight: bool) -> Ex {
        let k = self.imp(Import::Kaname);
        // Rust: every type that can be a key orders by its `Ord`, the order of keys (DESIGN 36.3)
        if self.t == Target::Rust {
            return Ex::atom(format!("{}::cmp", self.std_type("Ord")));
        }
        match (self.t, t) {
            (Target::TypeScript, Ty::Num(n)) if flight && rep(n) == Rep::Q => Ex::atom(format!("{k}.cmp")),
            (Target::TypeScript, Ty::Num(_)) => Ex::atom(format!("{k}.cmpInt")),
            (Target::TypeScript, Ty::Str | Ty::Date) => Ex::atom(format!("{k}.cmpStr")),
            (Target::TypeScript, Ty::Bool) => Ex::atom(format!("{k}.cmpBool")),
            (Target::Go, Ty::Num(n)) if flight && rep(n) == Rep::Q => Ex::atom(format!("{k}.Cmp")),
            (Target::Go, Ty::Num(n)) => {
                let ty = if flight { self.flight(t) } else if n.is_rate() { self.int_type().to_string() } else { self.unit_type(&n.unit) };
                Ex::atom(format!("{k}.CmpInt[{ty}]"))
            }
            (Target::Go, Ty::Str) => Ex::atom(format!("{k}.CmpStr")),
            (Target::Go, Ty::Date) => Ex::atom(format!("{k}.Compare")),
            (Target::Go, Ty::Bool) => Ex::atom(format!("{k}.CmpBool")),
            // a type parameter held to keys: the order the function is given (DESIGN 26.7)
            (_, Ty::Param(n)) => Ex::atom(self.param_order(n)),
            (_, Ty::Record(id)) if t.has_param() => self.record_order_inline(id),
            (_, Ty::Enum(id)) => {
                let r = self.rank_of(id);
                match self.t {
                    Target::Go => Ex::call(format!("{k}.ByRank"), vec![Ex::atom(r)]),
                    _ => Ex::call(format!("{k}.byRank"), vec![Ex::atom(r)]),
                }
            }
            (_, Ty::Record(id)) => Ex::atom(self.record_order(id)),
            (Target::Python, _) => self.py_key_fn(t),
            _ => Ex::atom(format!("{k}.cmpInt")),
        }
    }

    /// TypeScript's and Go's comparison of two values of `t`, as an expression: below 0, 0 or above 0.
    pub fn order_ex(&self, t: &Ty, a: Ex, b: Ex, flight: bool) -> Ex {
        let k = self.imp(Import::Kaname);
        if self.t == Target::Rust {
            return a.method("cmp", vec![b.borrow()]);
        }
        match (self.t, t) {
            // a whole number: the comparison of the language's; the order of an enum, its places
            (Target::Go, Ty::Num(n)) if !(flight && rep(n) == Rep::Q) => Ex::call(format!("{k}.CmpInt"), vec![a, b]),
            (_, Ty::Enum(id)) => {
                let r = self.rank_of(id);
                Ex::bin(Op::Sub, Ex::call(r.clone(), vec![a]), Ex::call(r, vec![b]))
            }
            _ => Ex::Call(Box::new(self.order_fn(t, flight)), vec![a, b]),
        }
    }

    /// Python's function that gives a value of `t` its key in its order: `natural` for a number, a
    /// date, a string (by its code points) and a bool, an enum's place, a record's tuple.
    pub fn py_key_fn(&self, t: &Ty) -> Ex {
        let k = self.imp(Import::Kaname);
        match t {
            Ty::Param(n) => Ex::atom(self.param_order(n)),
            Ty::Record(id) if t.has_param() => self.record_order_inline(id),
            Ty::Enum(id) => Ex::atom(self.rank_of(id)),
            Ty::Record(id) => Ex::atom(self.record_order(id)),
            _ => Ex::atom(format!("{k}.natural")),
        }
    }

    /// Python's key of a value `x` of `t`: the value itself where its order is its own.
    pub fn py_key(&self, t: &Ty, x: Ex) -> Ex {
        match t {
            Ty::Param(n) => Ex::call(self.param_order(n), vec![x]),
            Ty::Record(id) if t.has_param() => Ex::Call(Box::new(self.record_order_inline(id)), vec![x]),
            Ty::Enum(id) => Ex::call(self.rank_of(id), vec![x]),
            Ty::Record(id) => Ex::call(self.record_order(id), vec![x]),
            _ => x,
        }
    }

    /// The order of a record a body orders by, field by field in the order declared, made once:
    /// TypeScript's and Go's function that compares two, Python's that gives the tuple of the keys.
    pub fn record_order(&self, id: &RecId) -> String {
        let key = format!("{id:?}");
        if let Some((_, n, _)) = self.cmps.borrow().iter().find(|(k, _, _)| *k == key) {
            return n.clone();
        }
        let n = match self.t {
            Target::Python => format!("_key{}", self.cmps.borrow().len() + 1),
            _ => format!("_cmp{}", self.cmps.borrow().len() + 1),
        };
        self.cmps.borrow_mut().push((key.clone(), n.clone(), String::new()));
        let ty = self.record_type(id);
        let fields: Vec<(String, Ty)> = self.p.fields(id).iter().map(|(f, s)| (field_name(self.t, f.ascii()), s.ty.clone())).collect();
        let def = match self.t {
            Target::TypeScript => {
                let parts: Vec<String> = fields.iter().map(|(f, t)| self.order_ex(t, Ex::atom(format!("a.{f}")), Ex::atom(format!("b.{f}")), false).print(self.t)).collect();
                format!("function {n}(a: {ty}, b: {ty}): number {{
  return {} || 0;
}}
", parts.join(" || "))
            }
            Target::Python => {
                let parts: Vec<String> = fields.iter().map(|(f, t)| self.py_key(t, Ex::atom(format!("r.{f}"))).print(self.t)).collect();
                format!("
def {n}(r: {ty}) -> tuple[Any, ...]:
    return ({},)
", parts.join(", "))
            }
            Target::Go => {
                let mut body = String::new();
                for (i, (f, t)) in fields.iter().enumerate() {
                    let c = self.order_ex(t, Ex::atom(format!("a.{f}")), Ex::atom(format!("b.{f}")), false).print(self.t);
                    if i + 1 == fields.len() {
                        body.push_str(&format!("	return {c}
"));
                    } else {
                        body.push_str(&format!("	if c := {c}; c != 0 {{
		return c
	}}
"));
                    }
                }
                if fields.is_empty() {
                    body.push_str("	return 0
");
                }
                format!("func {n}(a, b {ty}) int {{
{body}}}
")
            }
            Target::Rust => unreachable!("Rust orders a record by its derived Ord"),
        };
        if let Some(x) = self.cmps.borrow_mut().iter_mut().find(|(k, _, _)| *k == key) {
            x.2 = def;
        }
        n
    }
}

impl Gen<'_> {
    /// The order of a record whose fields hold a type parameter held to keys (`Pair[K]`, `indexed[K]`:
    /// a key of a sort in a generic function, DESIGN 26.2), which reads the order the function is
    /// given: TypeScript's function that compares two and Python's that gives the key, written where
    /// it is used; Go's, a function of the module made once (`record_order_go`).
    fn record_order_inline(&self, id: &RecId) -> Ex {
        let ty = self.record_type(id);
        let fields: Vec<(String, Ty)> = self.p.fields(id).iter().map(|(f, s)| (field_name(self.t, f.ascii()), s.ty.clone())).collect();
        match self.t {
            Target::TypeScript => {
                let parts: Vec<String> = fields.iter().map(|(f, t)| self.order_ex(t, Ex::atom(format!("a.{f}")), Ex::atom(format!("b.{f}")), false).print(self.t)).collect();
                Ex::atom(format!("((a: {ty}, b: {ty}): number => {} || 0)", if parts.is_empty() { "0".to_string() } else { parts.join(" || ") }))
            }
            Target::Python => {
                let parts: Vec<String> = fields.iter().map(|(f, t)| self.py_key(t, Ex::atom(format!("r.{f}"))).print(self.t)).collect();
                Ex::atom(format!("(lambda r: ({},))", parts.join(", ")))
            }
            Target::Go => self.record_order_go(id, &ty, &fields),
            Target::Rust => unreachable!("Rust orders a record by its derived Ord"),
        }
    }

    /// Go: the order of a record whose fields hold a type parameter held to keys, as a function of the
    /// module made once for the record's type and the names its type parameters take where it is used
    /// (gofmt writes the body of a function literal over lines, so it is not written in an
    /// expression): it takes those type parameters and their orders, and answers the function that
    /// compares two values field by field (`_cmp3[K](orderK)`).
    fn record_order_go(&self, id: &RecId, ty: &str, fields: &[(String, Ty)]) -> Ex {
        let mut names: Vec<String> = Vec::new();
        params_in(&Ty::Record(id.clone()), &mut names);
        let scope: Vec<TScope> = self.tscope.borrow().clone();
        let ps: Vec<TScope> = names.iter().filter_map(|n| scope.iter().find(|p| p.name == *n).cloned()).collect();
        let key = format!("{id:?} {}", ps.iter().map(|p| format!("{} {}", p.ty, p.order.clone().unwrap_or_default())).collect::<Vec<_>>().join(", "));
        let known = self.cmps.borrow().iter().find(|(k, _, _)| *k == key).map(|(_, n, _)| n.clone());
        let n = match known {
            Some(n) => n,
            None => {
                let n = format!("_cmp{}", self.cmps.borrow().len() + 1);
                self.cmps.borrow_mut().push((key.clone(), n.clone(), String::new()));
                let decl = ps.iter().map(|p| format!("{} {}", p.ty, if p.key { "comparable" } else { "any" })).collect::<Vec<_>>().join(", ");
                let args = ps.iter().filter_map(|p| p.order.as_ref().map(|o| format!("{o} func(a, b {}) int", p.ty))).collect::<Vec<_>>().join(", ");
                let mut body = String::new();
                for (i, (f, t)) in fields.iter().enumerate() {
                    let c = self.order_ex(t, Ex::atom(format!("a.{f}")), Ex::atom(format!("b.{f}")), false).print(self.t);
                    if i + 1 == fields.len() {
                        body.push_str(&format!("\t\treturn {c}\n"));
                    } else {
                        body.push_str(&format!("\t\tif c := {c}; c != 0 {{\n\t\t\treturn c\n\t\t}}\n"));
                    }
                }
                if fields.is_empty() {
                    body.push_str("\t\treturn 0\n");
                }
                let def = format!("func {n}[{decl}]({args}) func(a, b {ty}) int {{\n\treturn func(a, b {ty}) int {{\n{body}\t}}\n}}\n");
                if let Some(x) = self.cmps.borrow_mut().iter_mut().find(|(k, _, _)| *k == key) {
                    x.2 = def;
                }
                n
            }
        };
        let targs = ps.iter().map(|p| p.ty.clone()).collect::<Vec<_>>().join(", ");
        Ex::call(format!("{n}[{targs}]"), ps.iter().filter_map(|p| p.order.clone()).map(Ex::atom).collect())
    }
}

/// Go: whether the field `f` of two values of a rule's record (an element of a list it walks, its
/// outputs) are the same: a value that may be none by what it points to.
fn rule_col_eq(f: &str, t: &ColumnType) -> Ex {
    match t {
        ColumnType::Opt(_) => Ex::atom(format!("(a.{f} == nil) == (b.{f} == nil) && (a.{f} == nil || *a.{f} == *b.{f})")),
        _ => Ex::bin(Op::Eq, Ex::atom(format!("a.{f}")), Ex::atom(format!("b.{f}"))),
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
    /// Rust: whether the function or the closure being written answers a `Result`, so that a call
    /// that can fail carries the failure with `?` (DESIGN 36.4).
    tries: bool,
    /// Rust: the function type, as the callee declares it (its type parameters not put in), of the
    /// parameter each lambda given as an argument goes to, by the lambda's expression: the closure
    /// takes its parameters as `&dyn Fn` of that type does (`&T` for `T`, DESIGN 36.4).
    fn_place: std::collections::HashMap<usize, Ty>,
}

impl<'g, 'a> Fx<'g, 'a> {
    pub fn new(g: &'g Gen<'a>, f: usize) -> Fx<'g, 'a> {
        Fx { g, f, out: Vec::new(), scope: Vec::new(), locals: Locals::default(), tries: rs_fails(&g.c, f), fn_place: std::collections::HashMap::new() }
    }

    /// Rust: the call `call` of what answers a `Result` (DESIGN 36.4): with `?` where the code being
    /// written answers one; else the analysis showed the call does not fail (a function given only
    /// functions that are `total`), and an error it answered anyway is a panic.
    fn carried(&self, call: Ex) -> Ex {
        if self.tries { call.tried() } else { call.method("unwrap_or_else", vec![Ex::atom("|e| panic!(\"{e}\")")]) }
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
            Target::Rust => format!("let {name}: {ty} = {};", value.print(t)),
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
            Target::Rust => format!("let {name}: {ty};"),
        };
        self.emit(Node::Line(line));
    }

    fn assign_line(&self, name: &str, value: Ex) -> Node {
        let t = self.t();
        if t == Target::Rust
            && let Ex::Bin(op @ (Op::Add | Op::Sub | Op::Mul), a, b) = &value
            && matches!(&**a, Ex::Atom(x) if x == name)
        {
            let o = match op {
                Op::Add => "+=",
                Op::Sub => "-=",
                _ => "*=",
            };
            return Node::Line(format!("{name} {o} {};", b.print(t)));
        }
        Node::Line(match t {
            Target::TypeScript | Target::Rust => format!("{name} = {};", value.print(t)),
            _ => format!("{name} = {}", value.print(t)),
        })
    }

    /// A value made a name, where it is read more than once or later code must not evaluate it again.
    fn spill(&mut self, v: Val) -> Val {
        if v.ex.is_simple() {
            // Rust: a name made is read again as the place it is
            if self.t() == Target::Rust && v.held == Held::Made && !self.g.is_copy(&v.ty) && !self.g.absent(&v.ex) {
                return Val { held: Held::Kept, ..v };
            }
            return v;
        }
        let n = self.temp();
        // Rust: a value made is kept in the name; a place or what is lent, lent to it (DESIGN 36.4)
        if self.t() == Target::Rust && v.held != Held::Made && !self.g.is_copy(&v.ty) {
            let ty = self.g.lent_type(&v.ty, false);
            let ex = self.g.lent(&v);
            self.bind(&n, &ty, ex);
            return Val { ex: Ex::atom(n), ty: v.ty, present: v.present, held: Held::Lent };
        }
        let ty = self.g.flight(&v.ty);
        self.bind(&n, &ty, v.ex);
        Val { ex: Ex::atom(n), ty: v.ty, present: v.present, held: Held::Kept }
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
                    return Val { held: v.held, ..Val::new(v.ex, to.clone()) };
                }
                if self.t() == Target::Rust {
                    let v = self.spill(v);
                    return self.opt_map_rs(&v, f, to, |s, x| s.adapt(x, t, at));
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
                    Target::Rust => v.ex.method("unwrap", vec![]),
                    _ => v.ex,
                };
                self.adapt(Val::new(inner, (**f).clone()), t, at)
            }
            (f, Ty::Opt(t)) => {
                let x = self.adapt(Val { held: v.held, ..Val::new(v.ex, f.clone()) }, t, at);
                match self.t() {
                    // a whole number written out is Go's untyped constant: `kaname.Some(int64(2))`
                    Target::Go => {
                        let whole = matches!(&x.ex, Ex::Atom(s) if s.trim_start_matches('(').trim_end_matches(')').parse::<i128>().is_ok());
                        let inner = if whole && t.num().map(rep) == Some(Rep::Int) { Ex::call("int64", vec![x.ex]) } else { x.ex };
                        // a value of a sum is held as its interface: `kaname.Some[Change](ChangeAdded{…})`
                        let some = match &**t {
                            Ty::Enum(id) if self.g.p.is_sum(id) => format!("Some[{}]", self.g.flight(t)),
                            Ty::Result(..) => format!("Some[{}]", self.g.flight(t)),
                            _ => "Some".to_string(),
                        };
                        Val::new(self.g.kcall(&some, vec![inner]), to.clone())
                    }
                    // Rust: `Some`, lending what is not made here (`Some(x.as_str())`, DESIGN 36.4)
                    Target::Rust => {
                        if self.g.is_copy(t) || x.held == Held::Made {
                            Val::new(Ex::call("Some", vec![x.ex]), to.clone())
                        } else {
                            Val::held(Ex::call("Some", vec![self.g.lent(&x)]), to.clone(), Held::Lent)
                        }
                    }
                    _ => Val { ex: x.ex, ty: to.clone(), present: true, held: x.held },
                }
            }
            (Ty::Num(a), Ty::Num(b)) => {
                let konst = a.konst;
                Val::new(self.convert(v.ex, a, b, at, konst), to.clone())
            }
            // Rust: a value of one of a sum's values, as the sum (`Order::Placed(x)`)
            (Ty::Record(RecId::Variant { value, .. }), Ty::Enum(id)) if self.t() == Target::Rust && self.g.p.is_sum(id) => {
                let w = self.g.variant_of(id, value);
                Val::new(Ex::call(w, vec![self.g.owned(&v)]), to.clone())
            }
            (Ty::List(a), Ty::List(b)) => {
                if self.adapt_is_identity(a, b) {
                    return Val { held: v.held, ..Val::new(v.ex, to.clone()) };
                }
                let v = self.spill(v);
                if self.t() == Target::Rust {
                    let to_el = self.g.flight(b);
                    return self.list_map_rs(&v, a, &to_el, to, |s, x| s.adapt(x, b, at));
                }
                let el = self.temp();
                let x = self.adapt(Val::new(Ex::atom(el.clone()), (**a).clone()), b, at);
                Val::new(self.list_map(v.ex, &el, &self.g.flight(a), x.ex, &self.g.flight(b)), to.clone())
            }
            // a dict and a set hold their keys and values as they travel: where those of the two types
            // travel otherwise (a rate on another step), each is read anew
            (Ty::Dict(ak, av), Ty::Dict(bk, bv)) => {
                if self.same_rep(ak, bk) && self.same_rep(av, bv) {
                    return Val { held: v.held, ..Val::new(v.ex, to.clone()) };
                }
                let v = self.spill(v);
                let (k, x) = (self.temp(), self.temp());
                let (kx, mut body) = self.apart(|s| s.reread(Ex::atom(k.clone()), ak, bk, at));
                let (vx, more) = self.apart(|s| s.reread(Ex::atom(x.clone()), av, bv, at));
                body.extend(more);
                self.rebuilt(v.ex, &from, Some((k, kx)), (x, vx), body, to)
            }
            (Ty::Set(a), Ty::Set(b)) => {
                if self.same_rep(a, b) {
                    return Val { held: v.held, ..Val::new(v.ex, to.clone()) };
                }
                let v = self.spill(v);
                let x = self.temp();
                let (xx, body) = self.apart(|s| s.reread(Ex::atom(x.clone()), a, b, at));
                self.rebuilt(v.ex, &from, None, (x, xx), body, to)
            }
            _ => Val { held: v.held, ..Val::new(v.ex, to.clone()) },
        }
    }

    /// Whether values of `a` and of `b` travel alike: a rate on one step, an amount in one unit.
    fn same_rep(&self, a: &Ty, b: &Ty) -> bool {
        match (a, b) {
            (Ty::Num(x), Ty::Num(y)) if x.is_rate() || y.is_rate() => x.is_rate() && y.is_rate() && rate_step(x) == rate_step(y),
            (Ty::Num(x), Ty::Num(y)) => factor_between(x, y) == Rat::int(1) && x.unit.type_name() == y.unit.type_name(),
            (Ty::Opt(x), Ty::Opt(y)) | (Ty::List(x), Ty::List(y)) | (Ty::Set(x), Ty::Set(y)) => self.same_rep(x, y),
            (Ty::Dict(k, x), Ty::Dict(l, y)) => self.same_rep(k, l) && self.same_rep(x, y),
            _ => true,
        }
    }

    /// A key or a value as it travels in `a`, as it travels in `b`.
    fn reread(&mut self, x: Ex, a: &Ty, b: &Ty, at: &Expr) -> Ex {
        let v = self.from_boundary(x, a, a);
        let v = self.adapt(v, b, at);
        self.to_boundary(v, b)
    }

    /// A dict (with `key`) or a set, each pair or member read anew: the loop that makes it.
    fn rebuilt(&mut self, src: Ex, from: &Ty, key: Option<(String, Ex)>, val: (String, Ex), mut body: Vec<Node>, to: &Ty) -> Val {
        let t = self.t();
        let out = self.temp();
        let (x, vx) = val;
        let head = match (&key, to) {
            (Some((k, kx)), Ty::Dict(bk, bv)) => {
                let (kb, vb) = (self.g.boundary(bk), self.g.boundary(bv));
                match t {
                    Target::TypeScript => {
                        self.emit(Node::Line(format!("const {out} = new Map<{kb}, {vb}>();")));
                        body.push(Node::Line(format!("{out}.set({}, {});", kx.print(t), vx.print(t))));
                        format!("for (const [{k}, {x}] of {})", src.print(t))
                    }
                    Target::Python => {
                        self.emit(Node::Line(format!("{out}: dict[{kb}, {vb}] = {{}}")));
                        body.push(Node::Line(format!("{out}[{}] = {}", kx.print(t), vx.print(t))));
                        format!("for {k}, {x} in {}.items()", src.print(t))
                    }
                    Target::Go => {
                        self.emit(Node::Line(format!("var {out} = map[{kb}]{vb}{{}}")));
                        body.push(Node::Line(format!("{out}[{}] = {}", kx.print(t), vx.print(t))));
                        format!("for {k}, {x} := range {}", src.print(t))
                    }
                    Target::Rust => {
                        let (map, (ak, av)) = (self.g.dict_type(kb, vb), dict_tys(from));
                        self.emit(Node::Line(format!("let mut {out}: {map} = {}::new();", self.g.std_type("BTreeMap"))));
                        body.push(Node::Line(format!("{out}.insert({}, {});", kx.print(t), vx.print(t))));
                        let pk = if self.g.is_copy(&ak) { format!("&{k}") } else { k.clone() };
                        let px = if self.g.is_copy(&av) { format!("&{x}") } else { x.clone() };
                        format!("for ({pk}, {px}) in {}.iter()", src.print(t))
                    }
                }
            }
            (_, to) => {
                let xb = match to {
                    Ty::Set(b) => self.g.boundary(b),
                    _ => "any".into(),
                };
                match t {
                    Target::TypeScript => {
                        self.emit(Node::Line(format!("const {out} = new Set<{xb}>();")));
                        body.push(Node::Line(format!("{out}.add({});", vx.print(t))));
                        format!("for (const {x} of {})", src.print(t))
                    }
                    Target::Python => {
                        self.emit(Node::Line(format!("{out}: set[{xb}] = set()")));
                        body.push(Node::Line(format!("{out}.add({})", vx.print(t))));
                        format!("for {x} in {}", src.print(t))
                    }
                    Target::Go => {
                        self.emit(Node::Line(format!("var {out} = map[{xb}]struct{{}}{{}}")));
                        body.push(Node::Line(format!("{out}[{}] = struct{{}}{{}}", vx.print(t))));
                        format!("for {x} := range {}", src.print(t))
                    }
                    Target::Rust => {
                        let set = self.g.set_type(xb.clone());
                        self.emit(Node::Line(format!("let mut {out}: {set} = {}::new();", self.g.std_type("BTreeSet"))));
                        body.push(Node::Line(format!("{out}.insert({});", vx.print(t))));
                        let from_el = match from {
                            Ty::Set(a) => (**a).clone(),
                            _ => Ty::Error,
                        };
                        let px = if self.g.is_copy(&from_el) { format!("&{x}") } else { x.clone() };
                        format!("for {px} in {}.iter()", src.print(t))
                    }
                }
            }
        };
        self.emit(Node::Block(head, body, Vec::new()));
        let ex = match (t, to) {
            (Target::Python, Ty::Set(_)) => Ex::call("frozenset", vec![Ex::atom(out)]),
            _ => Ex::atom(out),
        };
        Val::new(ex, to.clone())
    }

    /// Whether reading a value of `a` in `b` changes nothing of it.
    fn adapt_is_identity(&self, a: &Ty, b: &Ty) -> bool {
        match (a, b) {
            (Ty::Num(x), Ty::Num(y)) => rep(x) == rep(y) && factor_between(x, y) == Rat::int(1) && x.konst.is_none(),
            (Ty::Opt(x), Ty::Opt(y)) | (Ty::List(x), Ty::List(y)) => self.adapt_is_identity(x, y),
            (Ty::Set(_), Ty::Set(_)) | (Ty::Dict(..), Ty::Dict(..)) => self.same_rep(a, b),
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

    /// Rust: a value that may be absent (`v`, of `T?` with `T` the type `inner`), mapped where it is
    /// there by `f` (given the value inside, as Rust holds it): `x.map(|y| …)` where the mapping
    /// writes no statement and cannot fail, else a `match` that gives a name the answer.
    fn opt_map_rs(&mut self, v: &Val, inner: &Ty, to: &Ty, f: impl FnOnce(&mut Self, Val) -> Val) -> Val {
        let y = self.temp();
        let copy = self.g.is_copy(inner);
        let src = if copy { v.ex.clone() } else { self.g.lent(v) };
        let held = if copy { Held::Made } else { Held::Lent };
        let (m, body) = self.apart(|s| f(s, Val::held(Ex::atom(y.clone()), inner.clone(), held)));
        let mapped = self.g.owned(&m);
        if body.is_empty() && !mapped.tries() {
            // a function of the value alone is given as it is (`.map(units::USDc)`, clippy's
            // `redundant_closure`)
            let f = match &mapped {
                Ex::Call(f, xs) if matches!(&**f, Ex::Atom(_)) && matches!(xs.as_slice(), [Ex::Atom(a)] if *a == y) => f.print(Target::Rust),
                _ => format!("|{y}| {}", mapped.print(Target::Rust)),
            };
            let ex = src.method("map", vec![Ex::atom(f)]);
            return Val::new(ex, to.clone());
        }
        let n = self.temp();
        let ty = self.g.flight(to);
        self.declare(&n, &ty);
        let mut some = body;
        some.push(self.assign_line(&n, Ex::call("Some", vec![mapped])));
        let none = vec![self.assign_line(&n, Ex::atom("None"))];
        self.emit(Node::Switch(format!("match {}", src.print(Target::Rust)), vec![(format!("Some({y})"), some), ("None".to_string(), none)], Vec::new()));
        Val::held(Ex::atom(n), to.clone(), Held::Kept)
    }

    /// Rust: a list (`v`, of elements of `el`) with each element mapped by `f` into a new list of
    /// `to_el`: a loop that pushes each, the element given by value where it is `Copy`.
    fn list_map_rs(&mut self, v: &Val, el: &Ty, to_el: &str, to: &Ty, f: impl FnOnce(&mut Self, Val) -> Val) -> Val {
        let y = self.temp();
        let copy = self.g.is_copy(el);
        let held = if copy { Held::Made } else { Held::Ref };
        let (m, mut body) = self.apart(|s| f(s, Val::held(Ex::atom(y.clone()), el.clone(), held)));
        let n = self.temp();
        self.emit(Node::Line(format!("let mut {n}: Vec<{to_el}> = Vec::with_capacity({});", v.ex.clone().method("len", vec![]).print(Target::Rust))));
        body.push(self.push(&n, self.g.owned(&m)));
        let pat = if copy { format!("&{y}") } else { y };
        self.emit(Node::Block(format!("for {pat} in {}", v.ex.clone().method("iter", vec![]).print(Target::Rust)), body, Vec::new()));
        Val::held(Ex::atom(n), to.clone(), Held::Kept)
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
            Target::Rust => unreachable!("Rust maps an optional with opt_map_rs"),
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
            Target::Rust => unreachable!("Rust maps a list with list_map_rs"),
        }
    }

    /// A number of `a` read in the unit and the representation of `b` (`interp.rs`'s `convert`),
    /// held to int64 at `at` where the analysis does not show it fits. A literal, and a literal below
    /// 0 (`-18℃`, DESIGN 3.10), is read when the code is written. A temperature is read with its
    /// unit's zero: times the factor, plus the shift between the two zeros, held once, at the end, as
    /// the interpreter holds it (DESIGN 3.2).
    fn convert(&mut self, x: Ex, a: &NumTy, b: &NumTy, at: &Expr, konst: Option<Rat>) -> Ex {
        let g = self.g;
        if let Some(k) = konst
            && literal(at)
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
        let shift = zero_shift(a, b);
        if shift.num != 0 {
            let q = self.rep_to(x, ra, Rep::Q);
            let p = self.q_op(Op::Mul, q, g.frac(f), None);
            let s = self.q_op(Op::Add, p, g.frac(shift), check.as_ref());
            return self.rep_to(s, Rep::Q, rb);
        }
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
            (Some(at), Target::Go | Target::Rust) => {
                let f = match op {
                    Op::Add => "Add",
                    Op::Sub => "Sub",
                    _ => "Mul",
                };
                let mut args = vec![a, b];
                args.extend(g.at_args(at));
                let call = g.kcall(f, args);
                if self.t() == Target::Rust { call.tried() } else { call }
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
                g.held_call(args)
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
                    if self.t() == Target::Rust {
                        // Rust: the error returned with `?`, so that the code after it is not unreachable
                        let fail = self.g.kcall("fails", std::mem::take(&mut args));
                        self.emit(Node::Line(format!("{};", fail.tried().print(Target::Rust))));
                    } else {
                        let fail = self.g.kcall(&self.g.helper("fail", "fail", "Fail"), std::mem::take(&mut args));
                        self.emit(Node::Line(self.g.stmt(fail)));
                    }
                }
                Val::new(ex, ty)
            }
            ExprKind::Date(d) => Val::new(self.g.date(*d), ty),
            // Rust: a literal is a `&'static str`, lent
            ExprKind::Str(s) if self.t() == Target::Rust => Val::held(self.g.string(s), ty, Held::Lent),
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
            ExprKind::Record(_, base, inits) => self.record(e, base.as_deref(), inits),
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
                    let v = if later[i] { self.spill(v) } else { v };
                    items.push(self.g.owned(&v));
                }
                let lt = match self.t() {
                    Target::Go => format!("[]{}", self.g.flight(&elem)),
                    _ => String::new(),
                };
                Val::new(Ex::List(lt, items), ty)
            }
            ExprKind::Dict(pairs) => self.dict_literal(e, pairs),
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
                        (Some(at), Target::Rust) => {
                            let mut args = vec![v.ex];
                            args.extend(self.g.at_args(&at));
                            self.g.kcall("Neg", args).tried()
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
                                self.g.held_call(args)
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
            ExprKind::Lambda(ps, body) => self.lambda_value(e, ps, body),
        }
    }

    /// A lambda made a value (DESIGN 27.6): a function of the target, written before the statement that
    /// uses it, whose body reads the names in scope (a closure): a TypeScript arrow function, a nested
    /// `def` in Python, a function literal in Go. It takes its parameters and gives its answer as they
    /// travel, as a function of the module does.
    fn lambda_value(&mut self, e: &Expr, ps: &[(String, Span)], body: &Expr) -> Val {
        let ty = self.ty(e);
        let Ty::Fn(pts, rt, _) = &ty else { return Val::new(self.g.null(), ty) };
        let (pts, rt) = (pts.clone(), (**rt).clone());
        let t = self.t();
        let name = self.temp();
        let names: Vec<String> = ps.iter().map(|(p, _)| self.local(p)).collect();
        let mark = self.scope.len();
        if t == Target::Rust {
            let decl = self.fn_place.get(&e.id).cloned();
            return self.closure_rs(&ty, decl.as_ref(), ps, &names, &pts, &rt, body, name, mark);
        }
        let (answer, mut stmts) = self.apart(|s| {
            for (((p, _), n), pt) in ps.iter().zip(&names).zip(&pts) {
                let v = s.from_boundary(Ex::atom(n.clone()), pt, pt);
                let v = if v.ex.is_simple() { v } else { s.spill(v) };
                s.scope.push((p.clone(), v, Rc::new(Cell::new(true))));
            }
            let v = s.expr(body);
            let v = s.adapt(v, &rt, body);
            s.to_boundary(v, &rt)
        });
        self.scope.truncate(mark);
        stmts.push(Node::Line(self.g.ret(answer)));
        let ret = self.g.boundary(&rt);
        let sig: Vec<(String, String)> = names.iter().cloned().zip(pts.iter().map(|p| self.g.boundary(p))).collect();
        let head = match t {
            Target::TypeScript => format!("const {name} = ({}): {ret} =>", sig.iter().map(|(n, ty)| format!("{n}: {ty}")).collect::<Vec<_>>().join(", ")),
            Target::Python => format!("def {name}({}) -> {ret}", sig.iter().map(|(n, ty)| format!("{n}: {ty}")).collect::<Vec<_>>().join(", ")),
            Target::Go => format!("{name} := func({}) {ret}", sig.iter().map(|(n, ty)| format!("{n} {ty}")).collect::<Vec<_>>().join(", ")),
            Target::Rust => unreachable!("Rust's closure is written by closure_rs"),
        };
        self.emit(Node::Block(head, stmts, Vec::new()));
        Val::new(Ex::atom(name), ty)
    }

    /// Rust: a lambda made a value (DESIGN 36.4): a closure that takes its parameters as a function
    /// takes them (lent where they are not `Copy`) and answers as they travel, in a `Result` unless
    /// its type is `total`; the name is a reference to it, which a parameter `&dyn Fn` takes.
    #[allow(clippy::too_many_arguments)]
    fn closure_rs(&mut self, ty: &Ty, decl: Option<&Ty>, ps: &[(String, Span)], names: &[String], pts: &[Ty], rt: &Ty, body: &Expr, name: String, mark: usize) -> Val {
        // the type of the parameter it is given to says what it answers and how it takes its
        // parameters: `total` answers the value; a type parameter is taken as `&T`, a `Copy` one
        // read by its pattern (`&x: &i64`)
        let total = match decl {
            Some(Ty::Fn(_, _, eff)) => matches!(eff, Some(e) if e.fails.is_empty()),
            _ => matches!(ty, Ty::Fn(_, _, Some(e)) if e.fails.is_empty()),
        };
        let by_ref: Vec<bool> = match decl {
            Some(Ty::Fn(dps, _, _)) => (0..pts.len()).map(|i| matches!(dps.get(i), Some(Ty::Param(_)))).collect(),
            _ => vec![false; pts.len()],
        };
        let used: Vec<Rc<Cell<bool>>> = names.iter().map(|_| Rc::new(Cell::new(false))).collect();
        let outer = std::mem::replace(&mut self.tries, !total);
        let (answer, mut stmts) = self.apart(|s| {
            for (((((p, _), n), pt), u), r) in ps.iter().zip(names).zip(pts).zip(&used).zip(&by_ref) {
                let held = match (s.g.is_copy(pt), r) {
                    (true, _) => Held::Made,
                    (false, true) => Held::Ref,
                    (false, false) => Held::Lent,
                };
                let v = s.from_boundary_as(Ex::atom(n.clone()), pt, pt, held);
                let v = if v.ex.is_simple() { v } else { s.spill(v) };
                s.scope.push((p.clone(), v, u.clone()));
            }
            let v = s.expr(body);
            let v = s.adapt(v, rt, body);
            s.to_boundary(v, rt)
        });
        self.tries = outer;
        self.scope.truncate(mark);
        stmts.push(Node::Line(self.g.ret_rs(answer, !total)));
        let k = self.g.imp(Import::Kaname);
        let rb = self.g.boundary(rt);
        let ret = if total { rb } else { format!("{}<{rb}, {k}::KanameError>", self.g.std_type("Result")) };
        let sig: Vec<String> = names
            .iter()
            .zip(pts)
            .zip(&used)
            .zip(&by_ref)
            .map(|(((n, p), u), r)| {
                let pat = match (u.get(), *r && self.g.is_copy(p)) {
                    (false, _) => "_".to_string(),
                    (true, true) => format!("&{n}"),
                    (true, false) => n.clone(),
                };
                let ty = if *r { format!("&{}", self.g.boundary(p)) } else { self.g.param_rust(p) };
                format!("{pat}: {ty}")
            })
            .collect();
        let stmts = super::ex::tail(stmts);
        self.emit(Node::Block(format!("let {name} = &|{}| -> {ret}", sig.join(", ")), stmts, Vec::new()));
        Val::new(Ex::atom(name), ty.clone())
    }

    /// A function value called (DESIGN 27.6): what it is, its arguments from the left, each read in its
    /// parameter's type and as it travels, then the call; its answer as it is worked on.
    fn call_value(&mut self, e: &Expr, callee: &Expr, args: &[Arg]) -> Val {
        let ty = self.ty(e);
        let Ty::Fn(pts, rt, _) = self.ty(callee) else { return Val::new(self.g.null(), ty) };
        let mut es: Vec<&Expr> = vec![callee];
        es.extend(args.iter().map(Arg::expr));
        let mut vals = self.values(&es).into_iter();
        let Some(f) = vals.next() else { return Val::new(self.g.null(), ty) };
        let mut xs = Vec::new();
        for ((v, a), pt) in vals.zip(args).zip(pts.iter()) {
            let v = self.adapt(v, pt, a.expr());
            xs.push(if self.t() == Target::Rust { self.arg_rs(v, pt) } else { self.to_boundary(v, pt) });
        }
        let raw = Ex::Call(Box::new(f.ex), xs);
        let total = matches!(self.ty(callee), Ty::Fn(_, _, Some(ref e)) if e.fails.is_empty());
        let raw = if self.t() == Target::Rust && !total { self.carried(raw) } else { raw };
        let raw = self.spill_as(raw, &self.g.boundary(&rt));
        let v = self.from_boundary(raw, &rt, &ty);
        Val::new(v.ex, ty)
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
            // a generic record's fields with its type arguments put in (DESIGN 26.1)
            RecId::Module { .. } | RecId::Variant { .. } => {
                let fields = self.g.p.fields(&id);
                let Some((f, slot)) = fields.iter().find(|(f, _)| f.text == name) else { return Val::new(self.g.null(), ty.clone()) };
                let raw = b.ex.dot(field_name(self.t(), f.ascii()));
                // Rust: a field of a value made here is made; of a place, a place (DESIGN 36.4); out of
                // its box (DESIGN 36.2)
                let held = if b.held == Held::Made { Held::Made } else { Held::Kept };
                let (raw, held) = self.g.unboxed(&Ty::Record(id.clone()), &slot.ty, raw, held);
                self.from_boundary_as(raw, &slot.ty, ty, held)
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
                let mut v = self.rule_out(raw, &col.ty, ty);
                if self.t() == Target::Rust && b.held != Held::Made && v.held == Held::Made && !self.g.is_copy(ty) {
                    v.held = Held::Kept;
                }
                v
            }
            RecId::Element(file, seq) => {
                let Some(facts) = self.g.p.rules.get(file).cloned() else { return Val::new(self.g.null(), ty.clone()) };
                let Some((_, _, cols)) = facts.elements.iter().find(|(n, _, _)| n == seq) else { return Val::new(self.g.null(), ty.clone()) };
                let Some(col) = cols.iter().find(|c| c.name == name) else { return Val::new(self.g.null(), ty.clone()) };
                let alias = match self.t() {
                    Target::Go => ritsu_emit::ident::pascal(&col.alias),
                    Target::Rust => ritsu_emit::ident::rust_name(&col.alias),
                    _ => col.alias.clone(),
                };
                self.rule_out(b.ex.dot(alias), &col.ty, ty)
            }
            RecId::Entry(..) | RecId::Indexed(_) => {
                let slot_ty = self.g.p.fields(&id).into_iter().find(|(f, _)| f.text == name).map(|(_, s)| s.ty).unwrap_or(Ty::Error);
                let f = match self.t() {
                    Target::Go => {
                        let mut cs = name.chars();
                        cs.next().map(|c| c.to_ascii_uppercase().to_string() + cs.as_str()).unwrap_or_default()
                    }
                    _ => name.to_string(),
                };
                let held = if b.held == Held::Made { Held::Made } else { Held::Kept };
                self.from_boundary_as(b.ex.dot(f), &slot_ty, ty, held)
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
                let held = if b.held == Held::Made { Held::Made } else { Held::Kept };
                self.from_boundary_as(b.ex.dot(f), &slot_ty, ty, held)
            }
        }
    }

    /// A value as it travels (in a record, given to a function, answered by one), as it is worked
    /// on: an amount as a plain number, a rate as the fraction it is.
    pub fn from_boundary(&mut self, x: Ex, slot: &Ty, ty: &Ty) -> Val {
        self.from_boundary_as(x, slot, ty, Held::Kept)
    }

    /// [`Fx::from_boundary`] of a value Rust's code holds as `held` (a place, by default): what is
    /// read as it is keeps it; what is read anew is made.
    pub fn from_boundary_as(&mut self, x: Ex, slot: &Ty, ty: &Ty, held: Held) -> Val {
        let t = self.t();
        match slot {
            Ty::Num(n) if n.is_rate() => {
                let step = self.g.frac(rate_step(n));
                let q = self.g.kcall(&self.g.helper("rate", "rate", "Rate"), vec![x, step]);
                let r = rep(ty.num().unwrap_or(n));
                Val::new(self.rep_to(q, Rep::Q, r), ty.clone())
            }
            Ty::Num(n) => {
                let ex = match t {
                    Target::Go if n.unit.type_name().is_some() => Ex::call("int64", vec![x]),
                    // the package's type of the unit, `units::USDc(pub i64)`
                    Target::Rust if n.unit.type_name().is_some() => x.dot("0"),
                    _ => x,
                };
                let r = rep(ty.num().unwrap_or(n));
                Val::new(self.rep_to(ex, Rep::Int, r), ty.clone())
            }
            Ty::Opt(s) | Ty::List(s) if !self.g.same_in(slot) && t == Target::Rust => {
                let inner_ty = match ty {
                    Ty::Opt(x) | Ty::List(x) => (**x).clone(),
                    x => x.clone(),
                };
                let v = self.spill(Val::held(x, slot.clone(), held));
                let s = (**s).clone();
                if matches!(slot, Ty::List(_)) {
                    let to_el = self.g.flight(&inner_ty);
                    self.list_map_rs(&v, &s, &to_el, ty, |f, el| f.from_boundary_as(el.ex.clone(), &s, &inner_ty, el.held))
                } else {
                    self.opt_map_rs(&v, &s, ty, |f, el| f.from_boundary_as(el.ex.clone(), &s, &inner_ty, el.held))
                }
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
            _ => Val::held(x, ty.clone(), held),
        }
    }

    /// Rust: a value as a function, a closure or a helper takes it in a parameter of `slot`: as it
    /// travels, lent where it is not `Copy` (DESIGN 36.4).
    pub fn arg_rs(&mut self, v: Val, slot: &Ty) -> Ex {
        if self.g.same_out(slot) && !matches!(slot, Ty::Num(_)) {
            return self.g.lent(&v);
        }
        let b = self.to_boundary(v, slot);
        if self.g.is_copy(slot) {
            return b;
        }
        self.g.lent(&Val::new(b, slot.clone()))
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
                            Target::Rust => Ex::call(ut, vec![x]),
                            _ => Ex::Cast(Box::new(x), ut),
                        }
                    }
                    None => x,
                }
            }
            Ty::Opt(_) if self.g.absent(&v.ex) => v.ex,
            Ty::Opt(s) | Ty::List(s) if !self.g.same_out(slot) && t == Target::Rust => {
                let inner_ty = match &v.ty {
                    Ty::Opt(x) | Ty::List(x) => (**x).clone(),
                    x => x.clone(),
                };
                let s = (**s).clone();
                let v = self.spill(v);
                if matches!(slot, Ty::List(_)) {
                    let to_el = self.g.boundary(&s);
                    self.list_map_rs(&v, &inner_ty, &to_el, slot, |f, el| {
                        let ex = f.to_boundary(el, &s);
                        Val::new(ex, s.clone())
                    })
                    .ex
                } else {
                    self.opt_map_rs(&v, &inner_ty, slot, |f, el| {
                        let ex = f.to_boundary(el, &s);
                        Val::new(ex, s.clone())
                    })
                    .ex
                }
            }
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
            // Rust: what is put where a value travels is owned there (DESIGN 36.4)
            _ if t == Target::Rust => self.g.owned(&v),
            _ => v.ex,
        }
    }

    /// A value a rule's code answers (an output, a field of an element), as it is worked on.
    fn rule_out(&mut self, x: Ex, col: &ColumnType, ty: &Ty) -> Val {
        let t = self.t();
        match col {
            // Rust: the rule's `Option`, its value read where it is there
            ColumnType::Opt(inner) if t == Target::Rust => {
                let v = self.spill(Val::held(x, ty.clone(), Held::Kept));
                if matches!(**inner, ColumnType::Bool | ColumnType::Str | ColumnType::Enum { .. }) {
                    return Val::held(v.ex, ty.clone(), v.held);
                }
                let inner_ty = ty.present().clone();
                let target = if matches!(ty, Ty::Opt(_)) { ty.clone() } else { Ty::Opt(Box::new(ty.clone())) };
                let inner = (**inner).clone();
                self.opt_map_rs(&v, &inner_ty, &target, |f, el| f.rule_out(el.ex, &inner, &inner_ty))
            }
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
                    Target::Go | Target::Rust => self.g.kcall("DateOf", vec![x]),
                },
                ty.clone(),
            ),
            ColumnType::Num { unit: Some(u), .. } if u.dim == Dim::Rate => {
                let step = u.step.unwrap_or(Rat::new(1, 100));
                let k = match t {
                    Target::Go => Ex::call("int64", vec![x]),
                    // the rule's `Rate(pub i64)`
                    Target::Rust => x.dot("0"),
                    _ => x,
                };
                let q = self.g.kcall(&self.g.helper("rate", "rate", "Rate"), vec![k, self.g.frac(step)]);
                let r = ty.num().map(rep).unwrap_or(Rep::Q);
                Val::new(self.rep_to(q, Rep::Q, r), ty.clone())
            }
            ColumnType::Num { unit, .. } => {
                let k = match t {
                    Target::Go => Ex::call("int64", vec![x]),
                    // the package's type of the unit (rulec's Rust in a package, DESIGN 36.6)
                    Target::Rust if unit.as_ref().is_some_and(|u| u.type_name().is_some()) => x.dot("0"),
                    _ => x,
                };
                let r = ty.num().map(rep).unwrap_or(Rep::Int);
                Val::new(self.rep_to(k, Rep::Int, r), ty.clone())
            }
            _ => Val::held(x, ty.clone(), Held::Kept),
        }
    }

    /// A value as it is worked on, as a rule's code takes it in a column of `col`.
    fn rule_in(&mut self, v: Val, file: &Path, col: &ColumnType) -> Ex {
        let t = self.t();
        match col {
            ColumnType::Opt(inner) if t == Target::Rust => {
                if self.g.absent(&v.ex) {
                    return self.g.null();
                }
                let present = v.ty.present().clone();
                if !matches!(v.ty, Ty::Opt(_)) {
                    let x = self.rule_in(Val { ty: present, ..v }, file, inner);
                    return Ex::call("Some", vec![x]);
                }
                if matches!(**inner, ColumnType::Bool | ColumnType::Str | ColumnType::Enum { .. }) {
                    return self.g.owned(&v);
                }
                let v = self.spill(v);
                let (inner, file) = ((**inner).clone(), file.to_path_buf());
                let to = v.ty.clone();
                self.opt_map_rs(&v, &present, &to, |f, el| {
                    let x = f.rule_in(el, &file, &inner);
                    Val::new(x, Ty::Error)
                })
                .ex
            }
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
                    Target::Rust => unreachable!("Rust reads an optional column above"),
                }
            }
            ColumnType::Date => match t {
                Target::TypeScript => Ex::call("BigInt", vec![self.g.kcall("dayOf", vec![v.ex])]),
                Target::Python => self.g.kcall("day_of", vec![v.ex]),
                Target::Go => Ex::Call(Box::new(v.ex.dot("Days")), vec![]),
                Target::Rust => v.ex.method("days", vec![]),
            },
            ColumnType::Num { unit: Some(u), .. } if u.dim == Dim::Rate => {
                let step = u.step.unwrap_or(Rat::new(1, 100));
                let q = self.rep_to(v.ex, v.ty.num().map(rep).unwrap_or(Rep::Q), Rep::Q);
                let k = self.g.kcall(&self.g.helper("steps", "steps", "Steps"), vec![q, self.g.frac(step)]);
                let rate = self.g.path(&self.g.imp(Import::Rule(file.to_path_buf())), "Rate");
                if t == Target::Rust { Ex::call(rate, vec![k]) } else { Ex::Cast(Box::new(k), rate) }
            }
            ColumnType::Num { unit: Some(u), .. } => {
                let x = self.rep_to(v.ex, v.ty.num().map(rep).unwrap_or(Rep::Int), Rep::Int);
                match (u.type_name().is_some(), t) {
                    (true, Target::Rust) => Ex::call(self.g.unit_type(u), vec![x]),
                    (true, _) => Ex::Cast(Box::new(x), self.g.unit_type(u)),
                    _ => x,
                }
            }
            ColumnType::Num { unit: None, .. } => self.rep_to(v.ex, v.ty.num().map(rep).unwrap_or(Rep::Int), Rep::Int),
            // Rust: rulec's code takes its strings owned
            _ if t == Target::Rust => self.g.owned(&v),
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
                Target::Go | Target::Rust => self.g.kcall("Between", vec![x.ex, y.ex]),
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
                        let call = self.g.kcall(&self.g.helper("over", "over", "Over"), args);
                        if self.t() == Target::Rust { call.tried() } else { call }
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
                        self.g.held_call(args)
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
        } else if self.t() == Target::Rust {
            self.emit(Node::Line(format!("let mut {n}: {bt} = {};", left.ex.print(self.t()))));
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
        // two values of a type parameter held to keys, in their order (DESIGN 26.2)
        if let (Ty::Param(_), Ty::Param(_)) = (ta, tb)
            && !matches!(o, Op::Eq | Op::Ne)
        {
            if t == Target::Rust {
                return Ex::bin(o, self.g.cmp_side(&x), self.g.cmp_side(&y));
            }
            return self.order(ta, x.ex, y.ex, o);
        }
        // two values compared whole: a value that may be none, a list, a record, a value of a sum, a
        // transfer, a result, a value of a type parameter (DESIGN 23.5)
        if self.g.whole(ta) || self.g.whole(tb) {
            return self.compare_whole(a, b, o, x, y, ta, tb);
        }
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
                    // Rust's `kaname::Q` orders by its value
                    (Rep::Int, _) | (Rep::Q, Target::Python | Target::Rust) => Ex::bin(o, p, q),
                    (Rep::Q, Target::TypeScript) => Ex::bin(o, self.g.kcall("cmp", vec![p, q]), Ex::atom("0")),
                    (Rep::Q, Target::Go) => Ex::bin(o, self.g.kcall("Cmp", vec![p, q]), Ex::atom("0")),
                }
            }
            (Ty::Date, Ty::Date) => match (t, o) {
                (Target::Go, Op::Eq | Op::Ne) => Ex::bin(o, go_bare(x.ex, t), go_bare(y.ex, t)),
                (Target::Go, _) => Ex::bin(o, self.g.kcall("Compare", vec![x.ex, y.ex]), Ex::atom("0")),
                _ => Ex::bin(o, x.ex, y.ex),
            },
            // Rust: an enum's derived `Ord` is the order of its values
            (Ty::Enum(_), Ty::Enum(_)) if t == Target::Rust => Ex::bin(o, x.ex, y.ex),
            (Ty::Enum(id), Ty::Enum(_)) if !matches!(o, Op::Eq | Op::Ne) => {
                let rank = self.g.rank_of(id);
                Ex::bin(o, Ex::call(rank.clone(), vec![x.ex]), Ex::call(rank, vec![y.ex]))
            }
            // two strings by their code points: JavaScript's `<` compares UTF-16 units (DESIGN 25.7)
            (Ty::Str, Ty::Str) if t == Target::TypeScript && !matches!(o, Op::Eq | Op::Ne) => Ex::bin(o, self.g.kcall("cmpStr", vec![x.ex, y.ex]), Ex::atom("0")),
            // Rust: a string that is empty by `is_empty`; else the two as Rust compares them
            // (`a.as_str() < b`, DESIGN 36.4)
            (Ty::Str, Ty::Str) if t == Target::Rust && matches!(o, Op::Eq | Op::Ne) && (empty_str(&x.ex) || empty_str(&y.ex)) => {
                let other = if empty_str(&x.ex) { &y } else { &x };
                let e = other.ex.clone().method("is_empty", vec![]);
                if o == Op::Ne { Ex::Not(Box::new(e)) } else { e }
            }
            _ if t == Target::Rust => Ex::bin(o, self.g.cmp_side(&x), self.g.cmp_side(&y)),
            _ => Ex::bin(o, x.ex, y.ex),
        }
    }

    /// `a = b` and `a != b` of two values compared whole (`interp.rs`'s `combine`): each read in the
    /// type they are compared in (the numbers a list or a value that may be none holds in one unit),
    /// then the same in every part: TypeScript's `kaname.eq`, Python's `==`, Go's function of the type.
    #[allow(clippy::too_many_arguments)]
    fn compare_whole(&mut self, a: &Expr, b: &Expr, o: Op, x: Val, y: Val, ta: &Ty, tb: &Ty) -> Ex {
        let blank = |t: &Ty| matches!(t, Ty::None | Ty::Empty);
        // `none = none`, `[] = []`: the one value of each
        if blank(ta) && blank(tb) {
            return self.g.boolean(o == Op::Eq);
        }
        let t = match (ta, tb) {
            (Ty::None | Ty::Empty, t) | (t, Ty::None | Ty::Empty) => t.clone(),
            // a value of a sum and a value of the type of one of its values: as values of the sum
            (Ty::Record(r), t @ Ty::Enum(_)) | (t @ Ty::Enum(_), Ty::Record(r)) if r.sum().is_some() => t.clone(),
            (Ty::Record(r), Ty::Record(q)) if r.sum().is_some() && r != q => r.sum().map(Ty::Enum).unwrap_or(Ty::Error),
            _ => crate::interp::common(ta, tb).clone(),
        };
        let p = self.adapt(x, &t, a);
        let q = self.adapt(y, &t, b);
        if self.t() == Target::Rust {
            // Rust: a value that may be none against none by `is_none`, a list against `[]` by
            // `is_empty`; else the two as Rust compares them (DESIGN 36.4)
            let (pa, qa) = (self.g.absent(&p.ex) || matches!(ta, Ty::None), self.g.absent(&q.ex) || matches!(tb, Ty::None));
            let (pe, qe) = (matches!(ta, Ty::Empty) || empty_list(&p.ex), matches!(tb, Ty::Empty) || empty_list(&q.ex));
            if pa != qa {
                let other = if pa { &q } else { &p };
                let f = if o == Op::Ne { "is_some" } else { "is_none" };
                return other.ex.clone().method(f, vec![]);
            }
            if pe != qe {
                let other = if pe { &q } else { &p };
                let e = other.ex.clone().method("is_empty", vec![]);
                return if o == Op::Ne { Ex::Not(Box::new(e)) } else { e };
            }
            let same = self.g.eq_ex(self.g.cmp_side(&p), self.g.cmp_side(&q), &t);
            return if o == Op::Ne { Ex::Not(Box::new(same)) } else { same };
        }
        let same = self.g.eq_ex(p.ex, q.ex, &t);
        if o == Op::Ne { Ex::Not(Box::new(same)) } else { same }
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
            let held = if self.t() == Target::Rust && v.held != Held::Made && !self.g.is_copy(&bound) {
                let lt = self.g.lent_type(&bound, false);
                let lx = self.g.lent(&v);
                self.bind(&name, &lt, lx);
                Held::Lent
            } else {
                self.bind(&name, &ty, v.ex);
                Held::Kept
            };
            let used = Rc::new(Cell::new(false));
            declared.push((self.out.len(), name.clone(), used.clone()));
            self.scope.push((l.name.clone(), Val::held(Ex::atom(name), bound, held), used));
        }
        let out = self.answer(&b.answer, sink);
        // Rust: a name never read is not given one (`let _: T = …;`)
        if self.t() == Target::Rust {
            for (at, name, used) in &declared {
                if !used.get()
                    && let Some(Node::Line(s)) = self.out.get_mut(at - 1)
                    && let Some(rest) = s.strip_prefix(&format!("let {name}: "))
                {
                    *s = format!("let _: {rest}");
                }
            }
        }
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
            let (x, y) = if self.t() == Target::Rust { (Val::new(self.g.owned(&x), x.ty), Val::new(self.g.owned(&y), y.ty)) } else { (x, y) };
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
            return Val::held(Ex::atom(n), ty, Held::Kept);
        }
        let n = self.temp();
        let fty = self.g.flight(&ty);
        self.declare(&n, &fty);
        self.answer(e, &Sink::Assign(n.clone(), ty.clone()));
        Val::held(Ex::atom(n), ty, Held::Kept)
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
                let line = self.assign_line(n, self.g.owned(&v));
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
            if let Some(f) = g.outside_fn(&info.ret, Reach::Place) {
                let o = Ex::atom(self.temp());
                let said = Said::Answer { fun: info.name.text.clone() };
                let fail = g.fail_outside("range", decl.body.answer.span.line, &said, &o, &info.ret, Reach::Place);
                let stmts = g.if_found(&o, Ex::call(f, vec![b.clone(), g.string(&info.name.text)]), fail);
                for s in stmts {
                    self.emit(s);
                }
            }
        }
        let line = if self.t() == Target::Rust { g.ret_rs(b, self.fallible()) } else { g.ret(b) };
        self.emit(Node::Line(line));
    }

    /// Whether the function can fail (its effect, DESIGN 21.4): Rust's answers a `Result`.
    fn fallible(&self) -> bool {
        rs_fails(&self.g.c, self.f)
    }

    fn matching(&mut self, e: &Expr, s: &Expr, arms: &[Arm], sink: &Sink) {
        let ty = self.ty(e);
        let st = self.ty(s);
        let v = self.expr(s);
        // a value of a sum is named first, so that the target narrows the name by the tests
        let sum = match st.present() {
            Ty::Enum(id) if self.g.p.is_sum(id) => Some(id.clone()),
            _ => None,
        };
        let result = matches!(st, Ty::Result(..));
        if self.t() == Target::Rust {
            let v = self.spill(v);
            return self.match_rs(&ty, &st, &v, arms, sink);
        }
        let v = if (sum.is_some() || result) && !matches!(v.ex, Ex::Atom(_)) {
            let n = self.temp();
            let fty = self.g.flight(&st);
            self.bind(&n, &fty, v.ex);
            Val::new(Ex::atom(n), v.ty)
        } else {
            self.spill(v)
        };
        if let (Some(id), Target::Go) = (&sum, self.t()) {
            return self.go_switch(id, &ty, &v, arms, sink);
        }
        if result && self.t() == Target::Go {
            return self.go_result_switch(&st, &ty, &v, arms, sink);
        }
        let mut branches: Vec<(Ex, Vec<Node>)> = Vec::new();
        let mut last: Vec<Node> = Vec::new();
        let n = arms.len();
        for (k, arm) in arms.iter().enumerate() {
            let next = Sink::Then(ty.clone(), Box::new(arm.body.answer.clone()), Box::new(sink.clone()));
            let mut bind: Option<(String, Ty)> = None;
            // the value `ok x` and `err e` name, as it travels in the result (DESIGN 26.3)
            let mut payload: Option<Ex> = None;
            let mut tests: Vec<Ex> = Vec::new();
            for p in &arm.patterns {
                match p {
                    Pattern::Value(path, named, _) if result => {
                        let Ty::Result(a, er) = &st else { continue };
                        let ok = path[path.len() - 1].0 == "ok";
                        tests.push(match self.t() {
                            Target::TypeScript => Ex::bin(Op::Eq, v.ex.clone().dot("kind"), self.g.string(if ok { "ok" } else { "err" })),
                            _ => Ex::call("isinstance", vec![v.ex.clone(), Ex::atom(format!("{}.{}", self.g.imp(Import::Kaname), if ok { "Ok" } else { "Err" }))]),
                        });
                        if let Some((x, _)) = named {
                            bind = Some((x.clone(), if ok { (**a).clone() } else { (**er).clone() }));
                            payload = Some(v.ex.clone().dot(if ok { "value" } else { "error" }));
                        }
                    }
                    Pattern::None(_) => tests.push(self.is_null(&v.ex)),
                    Pattern::Some(x, _) => {
                        bind = Some((x.clone(), st.present().clone()));
                        tests.push(Ex::Not(Box::new(self.is_null(&v.ex))));
                    }
                    Pattern::Bool(b, _) => tests.push(if *b { v.ex.clone() } else { Ex::Not(Box::new(v.ex.clone())) }),
                    Pattern::Value(path, named, _) => {
                        let word = &path[path.len() - 1].0;
                        if let Ty::Enum(id) = st.present() {
                            let name = self.g.p.values(id).0.iter().find(|n| n.is(word)).map(|n| n.text.clone()).unwrap_or_else(|| word.clone());
                            if let Some(sid) = &sum {
                                // the value of a sum: its kind, or its class
                                let Owner::Module(file) = &sid.owner else { continue };
                                let r = RecId::Variant { file: file.clone(), name: sid.name.clone(), value: name.clone(), args: sid.args.clone() };
                                tests.push(match self.t() {
                                    Target::TypeScript => Ex::bin(Op::Eq, v.ex.clone().dot("kind"), self.g.string(&name)),
                                    _ => Ex::call("isinstance", vec![v.ex.clone(), Ex::atom(self.g.record_type_bare(&r))]),
                                });
                                if let Some((x, _)) = named {
                                    bind = Some((x.clone(), Ty::Record(r)));
                                }
                            } else {
                                let present = match (self.t(), &st) {
                                    (Target::Go, Ty::Opt(_)) => Ex::Deref(Box::new(v.ex.clone())),
                                    _ => v.ex.clone(),
                                };
                                tests.push(Ex::bin(Op::Eq, present, self.g.member(id, &name)));
                            }
                        }
                    }
                    Pattern::Lit(x) => tests.push(self.literal_test(&v, &st, x)),
                    Pattern::Else(_) => tests.push(self.g.boolean(true)),
                }
            }
            let (_, body) = self.apart(|f| {
                let mark = f.scope.len();
                if let Some((x, inner_ty)) = &bind {
                    let name = f.local(x);
                    let val = match (f.t(), inner_ty, &payload) {
                        // the value of a result, worked on as its type is
                        (_, _, Some(raw)) => f.from_boundary(raw.clone(), inner_ty, inner_ty).ex,
                        // the value `some x` names, where it is there
                        (Target::Go, t, None) if !matches!(t, Ty::Record(RecId::Variant { .. })) => Ex::Deref(Box::new(v.ex.clone())),
                        _ => v.ex.clone(),
                    };
                    let fty = f.g.flight(inner_ty);
                    f.bind(&name, &fty, val);
                    let used = Rc::new(Cell::new(false));
                    let at = f.out.len();
                    f.scope.push((x.clone(), Val::new(Ex::atom(name.clone()), inner_ty.clone()), used.clone()));
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

    /// Rust: a `match` as Rust's `match` (DESIGN 36.4) of the value `v` of `st`, lent where it is not
    /// `Copy`: each arm its patterns (`None`, `Some(x)`, `Order::Placed(p)`, `Ok(x)`, `Size::Small`,
    /// a literal), the value an arm names bound by the pattern; a literal Rust cannot write as a
    /// pattern (a fraction) a guard; the last arm `_` unless it names a value.
    fn match_rs(&mut self, ty: &Ty, st: &Ty, v: &Val, arms: &[Arm], sink: &Sink) {
        let copy = self.g.is_copy(st);
        let src = if copy { v.ex.clone() } else { self.g.lent(v) };
        let opt = matches!(st, Ty::Opt(_));
        let inner = st.present().clone();
        let wrap = |p: String| if opt { format!("Some({p})") } else { p };
        let mut cases: Vec<(String, Vec<Node>)> = Vec::new();
        let n = arms.len();
        for (k, arm) in arms.iter().enumerate() {
            let next = Sink::Then(ty.clone(), Box::new(arm.body.answer.clone()), Box::new(sink.clone()));
            // each pattern: Ok(the pattern, with `{}` where the name goes), or Err(the test a guard makes)
            let mut pats: Vec<Result<String, Ex>> = Vec::new();
            // the name an arm binds: its name in the source, its type, and how the value is reached
            let mut bind: Option<(String, Ty, bool)> = None;
            let mut all = false;
            for p in &arm.patterns {
                match p {
                    Pattern::None(_) => pats.push(Ok("None".into())),
                    Pattern::Some(x, _) => {
                        bind = Some((x.clone(), inner.clone(), false));
                        pats.push(Ok("Some({})".into()));
                    }
                    Pattern::Bool(b, _) => pats.push(Ok(wrap(b.to_string()))),
                    Pattern::Else(_) => all = true,
                    Pattern::Value(path, named, _) if matches!(inner, Ty::Result(..)) => {
                        let Ty::Result(a, er) = &inner else { continue };
                        let ok = path[path.len() - 1].0 == "ok";
                        let word = if ok { "Ok" } else { "Err" };
                        match named {
                            Some((x, _)) => {
                                bind = Some((x.clone(), if ok { (**a).clone() } else { (**er).clone() }, true));
                                pats.push(Ok(wrap(format!("{word}({{}})"))));
                            }
                            None => pats.push(Ok(wrap(format!("{word}(_)")))),
                        }
                    }
                    Pattern::Value(path, named, _) => {
                        let word = &path[path.len() - 1].0;
                        let Ty::Enum(id) = &inner else { continue };
                        let name = self.g.p.values(id).0.iter().find(|n| n.is(word)).map(|n| n.text.clone()).unwrap_or_else(|| word.clone());
                        if self.g.p.is_sum(id) {
                            let Owner::Module(file) = &id.owner else { continue };
                            let w = self.g.variant_of(id, &name);
                            match named {
                                Some((x, _)) => {
                                    let r = RecId::Variant { file: file.clone(), name: id.name.clone(), value: name.clone(), args: id.args.clone() };
                                    bind = Some((x.clone(), Ty::Record(r), false));
                                    pats.push(Ok(wrap(format!("{w}({{}})"))));
                                }
                                None => pats.push(Ok(wrap(format!("{w}(_)")))),
                            }
                        } else {
                            pats.push(Ok(wrap(self.g.member(id, &name).print(Target::Rust))));
                        }
                    }
                    Pattern::Lit(x) => pats.push(self.literal_pattern(v, st, x).map(wrap)),
                }
            }
            let bound = bind.as_ref().map(|(x, _, _)| self.local(x));
            let used = Rc::new(Cell::new(false));
            let (_, body) = self.apart(|f| {
                let mark = f.scope.len();
                if let (Some((x, t, payload)), Some(name)) = (&bind, &bound) {
                    // what the pattern binds: the value where the matched value is `Copy`, else what
                    // it lends (`Some(x)` of `as_deref`, a value of a sum) or a reference into it (a
                    // result's)
                    let val = if copy {
                        Val::held(Ex::atom(name.clone()), t.clone(), Held::Made)
                    } else if *payload {
                        f.g.through(Ex::atom(name.clone()), t)
                    } else if f.g.is_copy(t) {
                        Val::held(Ex::atom(name.clone()), t.clone(), Held::Made)
                    } else {
                        Val::held(Ex::atom(name.clone()), t.clone(), Held::Lent)
                    };
                    let val = if *payload { f.from_boundary_as(val.ex.clone(), t, t, val.held) } else { val };
                    f.scope.push((x.clone(), val, used.clone()));
                }
                f.block(&arm.body, &next);
                f.scope.truncate(mark);
            });
            let name = match &bound {
                Some(n) if used.get() => n.clone(),
                _ => "_".to_string(),
            };
            let guards: Vec<Ex> = pats.iter().filter_map(|p| p.clone().err()).collect();
            let pattern = if all || (k + 1 == n && bind.is_none()) {
                "_".to_string()
            } else if !guards.is_empty() {
                // a test Rust writes as no pattern: the arm's every test, in a guard
                let tests: Vec<Ex> = pats
                    .into_iter()
                    .map(|p| match p {
                        Ok(p) => Ex::atom(format!("matches!({}, {})", super::ex::rust_cond(&src), p.replace("{}", "_"))),
                        Err(g) => g,
                    })
                    .collect();
                let e = tests.into_iter().reduce(|a, b| Ex::bin(Op::Or, a, b)).unwrap_or(self.g.boolean(true));
                format!("_ if {}", e.print(Target::Rust))
            } else {
                pats.into_iter().filter_map(Result::ok).map(|p| p.replace("{}", &name)).collect::<Vec<_>>().join(" | ")
            };
            cases.push((pattern, body));
        }
        self.emit(Node::Switch(format!("match {}", super::ex::rust_cond(&src)), cases, Vec::new()));
    }

    /// Rust: a literal of an arm as a pattern (`5`, `-3`, `"a"`, `kaname::Date { … }`), read in the
    /// matched value's type; Err with the test of the guard where Rust writes it as no pattern.
    fn literal_pattern(&self, v: &Val, st: &Ty, x: &Expr) -> Result<String, Ex> {
        let g = self.g;
        let present = st.present().clone();
        match crate::interp::literal(&g.c, x, &present) {
            Some(crate::value::Value::Num(r)) if r.is_int() && present.num().map(rep) == Some(Rep::Int) => Ok(g.int(r.num).print(Target::Rust)),
            Some(crate::value::Value::Date(d)) => Ok(g.date(d).print(Target::Rust)),
            Some(crate::value::Value::Str(t)) => Ok(g.string(&t).print(Target::Rust)),
            _ => Err(self.literal_test(v, st, x)),
        }
    }

    /// Whether the value a `match` splits is a literal of an arm, read in the value's type (`interp.rs`'s
    /// `literal`): a number in the value's unit and representation, a date, a string.
    fn literal_test(&self, v: &Val, st: &Ty, x: &Expr) -> Ex {
        let g = self.g;
        let lit = crate::interp::literal(&g.c, x, st);
        match (lit, st) {
            (Some(crate::value::Value::Num(r)), Ty::Num(n)) => match (rep(n), self.t()) {
                (Rep::Int, _) if r.is_int() => Ex::bin(Op::Eq, v.ex.clone(), g.int(r.num)),
                (_, Target::Python) => Ex::bin(Op::Eq, v.ex.clone(), g.frac(r)),
                (Rep::Int, Target::TypeScript) => Ex::bin(Op::Eq, g.kcall("cmp", vec![g.to_q_ex(v.ex.clone()), g.frac(r)]), Ex::atom("0")),
                (_, Target::TypeScript) => Ex::bin(Op::Eq, g.kcall("cmp", vec![v.ex.clone(), g.frac(r)]), Ex::atom("0")),
                (Rep::Int, Target::Go) => Ex::bin(Op::Eq, g.kcall("Cmp", vec![g.to_q_ex(v.ex.clone()), g.frac(r)]), Ex::atom("0")),
                (_, Target::Go) => Ex::bin(Op::Eq, g.kcall("Cmp", vec![v.ex.clone(), g.frac(r)]), Ex::atom("0")),
                (Rep::Int, Target::Rust) => Ex::bin(Op::Eq, g.to_q_ex(v.ex.clone()), g.frac(r)),
                (_, Target::Rust) => Ex::bin(Op::Eq, v.ex.clone(), g.frac(r)),
            },
            (Some(crate::value::Value::Date(d)), _) => Ex::bin(Op::Eq, v.ex.clone(), go_bare(g.date(d), self.t())),
            (Some(crate::value::Value::Str(t)), _) => Ex::bin(Op::Eq, v.ex.clone(), g.string(&t)),
            _ => g.boolean(false),
        }
    }

    /// Go: a `match` of a value of a sum as a type switch (DESIGN 23.8): each arm a `case` of the types
    /// of its values, the value an arm names given its value's type; `else`, and else no arm, the
    /// `default`.
    fn go_switch(&mut self, id: &EnumId, ty: &Ty, v: &Val, arms: &[Arm], sink: &Sink) {
        let Owner::Module(file) = &id.owner else { return };
        let names = arms.iter().any(|a| a.patterns.iter().any(|p| matches!(p, Pattern::Value(_, Some(_), _))));
        let x = if names { Some(self.temp()) } else { None };
        let mut cases: Vec<(String, Vec<Node>)> = Vec::new();
        let mut default: Vec<Node> = Vec::new();
        for arm in arms {
            let next = Sink::Then(ty.clone(), Box::new(arm.body.answer.clone()), Box::new(sink.clone()));
            let mut types: Vec<String> = Vec::new();
            let mut bind: Option<(String, Ty)> = None;
            let mut rest = false;
            for p in &arm.patterns {
                match p {
                    Pattern::Value(path, named, _) => {
                        let word = &path[path.len() - 1].0;
                        let name = self.g.p.values(id).0.iter().find(|n| n.is(word)).map(|n| n.text.clone()).unwrap_or_else(|| word.clone());
                        let r = RecId::Variant { file: file.clone(), name: id.name.clone(), value: name, args: id.args.clone() };
                        types.push(self.g.record_type(&r));
                        if let Some((n, _)) = named {
                            bind = Some((n.clone(), Ty::Record(r)));
                        }
                    }
                    Pattern::Else(_) => rest = true,
                    _ => {}
                }
            }
            let (_, body) = self.apart(|f| {
                let mark = f.scope.len();
                if let (Some((n, t)), Some(x)) = (&bind, &x) {
                    let name = f.local(n);
                    let fty = f.g.flight(t);
                    f.bind(&name, &fty, Ex::atom(x.clone()));
                    let used = Rc::new(Cell::new(false));
                    let at = f.out.len();
                    f.scope.push((n.clone(), Val::new(Ex::atom(name.clone()), t.clone()), used.clone()));
                    f.block(&arm.body, &next);
                    if !used.get() {
                        f.out.insert(at, Node::Line(format!("_ = {name}")));
                    }
                } else {
                    f.block(&arm.body, &next);
                }
                f.scope.truncate(mark);
            });
            if rest {
                default = body;
            } else {
                cases.push((types.join(", "), body));
            }
        }
        if default.is_empty() {
            default.push(Node::Line("panic(\"kaname: no arm takes the value\")".into()));
        }
        let head = match &x {
            Some(x) => format!("switch {x} := {}.(type)", v.ex.print(Target::Go)),
            None => format!("switch {}.(type)", v.ex.print(Target::Go)),
        };
        self.emit(Node::Switch(head, cases, default));
    }

    /// Go: a `match` of a result as a type switch (DESIGN 26.3): `ok x` the case of `kaname.Ok`, `err e`
    /// the case of `kaname.Err`, the value each names read from what it holds.
    fn go_result_switch(&mut self, st: &Ty, ty: &Ty, v: &Val, arms: &[Arm], sink: &Sink) {
        let Ty::Result(a, er) = st else { return };
        let k = self.g.imp(Import::Kaname);
        let (ab, eb) = (self.g.boundary(a), self.g.boundary(er));
        let names = arms.iter().any(|a| a.patterns.iter().any(|p| matches!(p, Pattern::Value(_, Some(_), _))));
        let x = if names { Some(self.temp()) } else { None };
        let mut cases: Vec<(String, Vec<Node>)> = Vec::new();
        let mut default: Vec<Node> = Vec::new();
        for arm in arms {
            let next = Sink::Then(ty.clone(), Box::new(arm.body.answer.clone()), Box::new(sink.clone()));
            let mut types: Vec<String> = Vec::new();
            let mut bind: Option<(String, Ty, &str)> = None;
            let mut rest = false;
            for p in &arm.patterns {
                match p {
                    Pattern::Value(path, named, _) => {
                        let ok = path[path.len() - 1].0 == "ok";
                        types.push(format!("{k}.{}[{ab}, {eb}]", if ok { "Ok" } else { "Err" }));
                        if let Some((n, _)) = named {
                            bind = Some((n.clone(), if ok { (**a).clone() } else { (**er).clone() }, if ok { "Value" } else { "Error" }));
                        }
                    }
                    Pattern::Else(_) => rest = true,
                    _ => {}
                }
            }
            let (_, body) = self.apart(|f| {
                let mark = f.scope.len();
                if let (Some((n, t, field)), Some(x)) = (&bind, &x) {
                    let name = f.local(n);
                    let val = f.from_boundary(Ex::atom(x.clone()).dot(*field), t, t);
                    let fty = f.g.flight(t);
                    f.bind(&name, &fty, val.ex);
                    let used = Rc::new(Cell::new(false));
                    let at = f.out.len();
                    f.scope.push((n.clone(), Val::new(Ex::atom(name.clone()), t.clone()), used.clone()));
                    f.block(&arm.body, &next);
                    if !used.get() {
                        f.out.insert(at, Node::Line(format!("_ = {name}")));
                    }
                } else {
                    f.block(&arm.body, &next);
                }
                f.scope.truncate(mark);
            });
            if rest {
                default = body;
            } else {
                cases.push((types.join(", "), body));
            }
        }
        if default.is_empty() {
            default.push(Node::Line("panic(\"kaname: no arm takes the value\")".into()));
        }
        let head = match &x {
            Some(x) => format!("switch {x} := {}.(type)", v.ex.print(Target::Go)),
            None => format!("switch {}.(type)", v.ex.print(Target::Go)),
        };
        self.emit(Node::Switch(head, cases, default));
    }

    fn is_null(&self, v: &Ex) -> Ex {
        match self.t() {
            Target::Python => Ex::atom(format!("{} is None", v.print(Target::Python))),
            Target::Rust => v.clone().method("is_none", vec![]),
            _ => Ex::bin(Op::Eq, v.clone(), self.g.null()),
        }
    }

    // ── Records ────────────────────────────────────────────────────────────

    /// A record built, or a value of a sum (DESIGN 23.8): the value its fields are taken from first
    /// (`..x`), then the fields from the left, read in the fields' types and held to what they declare,
    /// in their order (`interp.rs`'s `record`).
    fn record(&mut self, e: &Expr, base: Option<&Expr>, inits: &[FieldInit]) -> Val {
        let ty = self.ty(e);
        let Ref::Record(id) = self.rf(e) else { return Val::new(self.g.null(), ty) };
        let fields = self.g.p.fields(&id);
        let mut es: Vec<&Expr> = Vec::new();
        let mut to: Vec<Option<(usize, Ty)>> = Vec::new();
        if let Some(b) = base {
            es.push(b);
            to.push(None);
        }
        es.extend(inits.iter().map(|f| &f.value));
        to.extend(inits.iter().map(|fi| fields.iter().position(|(n, _)| n.is(&fi.name)).map(|k| (k, fields[k].1.ty.clone()))));
        let module = matches!(id, RecId::Module { .. } | RecId::Variant { .. });
        let checked = module
            && inits.iter().zip(&to[base.is_some() as usize..]).any(|(fi, t)| {
                t.as_ref().is_some_and(|(k, _)| {
                    let slot = &fields[*k].1;
                    (slot.range.is_some() || slot.count.is_some()) && self.g.open(fi.value.id, |w| matches!(w, What::Field { .. })) && self.g.outside_fn(slot, Reach::Place).is_some()
                })
            });
        let (mut vals, keep) = self.arg_values(&es, &to, checked || base.is_some());
        // the value the fields not written are taken from, made a name
        let from: Option<Val> = if base.is_some() {
            let b = vals.remove(0);
            Some(self.spill(b))
        } else {
            None
        };
        let to = &to[base.is_some() as usize..];
        // read in the fields' types, as written; held to what they declare, in their order
        let mut given: Vec<Option<(Val, usize)>> = vec![None; fields.len()];
        for ((fi, v), t) in inits.iter().zip(vals).zip(to) {
            let Some((k, ft)) = t else { continue };
            let v = self.adapt(v, ft, &fi.value);
            let v = if keep { self.spill(v) } else { v };
            given[*k] = Some((v, fi.span.line));
        }
        let mut parts: Vec<(String, Ex)> = Vec::new();
        for (k, (f, slot)) in fields.iter().enumerate() {
            let key = match &id {
                RecId::Element(file, seq) => {
                    let alias = self.g.p.rules.get(file).and_then(|r| r.elements.iter().find(|(n, _, _)| n == seq)).and_then(|(_, _, cols)| cols.iter().find(|c| c.name == f.text)).map(|c| c.alias.clone()).unwrap_or_else(|| f.text.clone());
                    match self.t() {
                        Target::Go => ritsu_emit::ident::pascal(&alias),
                        Target::Rust => ritsu_emit::ident::rust_name(&alias),
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
            let held = module && (slot.range.is_some() || slot.count.is_some());
            let Some((v, line)) = given[k].take() else {
                // a field taken from the value it is made from: held to what it declares where the
                // analysis does not show it keeps it (the place is at `..x`)
                if let (Some(fv), Some(b)) = (&from, base)
                    && held
                    && self.g.open(b.id, |w| matches!(w, What::Field { field, .. } if *field == f.text))
                    && let Some(fun) = self.g.outside_fn(slot, Reach::Place)
                {
                    let o = Ex::atom(self.temp());
                    let rec = self.g.p.show_ty(&self.g.c, &Ty::Record(id.clone()));
                    let fail = self.g.fail_outside("range", b.span.line, &Said::Field { rec }, &o, slot, Reach::Place);
                    let x = self.g.lent_field(&fv.ex, &Ty::Record(id.clone()), f.ascii(), &slot.ty);
                    let stmts = self.g.if_found(&o, Ex::call(fun, vec![x, self.g.string(&f.text)]), fail);
                    for s in stmts {
                        self.emit(s);
                    }
                }
                if from.is_none() {
                    parts.push((key, self.g.null()));
                }
                continue;
            };
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
            let b = if held {
                let at = inits.iter().find(|fi| f.is(&fi.name)).map(|fi| fi.value.id);
                if at.is_some_and(|x| self.g.open(x, |w| matches!(w, What::Field { .. }))) {
                    let b = self.spill_as(b, &self.g.boundary(&slot.ty));
                    if let Some(fun) = self.g.outside_fn(slot, Reach::Place) {
                        let o = Ex::atom(self.temp());
                        let rec = self.g.p.show_ty(&self.g.c, &Ty::Record(id.clone()));
                        let fail = self.g.fail_outside("range", line, &Said::Field { rec }, &o, slot, Reach::Place);
                        let x = self.g.lent(&Val::held(b.clone(), slot.ty.clone(), Held::Kept));
                        let stmts = self.g.if_found(&o, Ex::call(fun, vec![x, self.g.string(&f.text)]), fail);
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
            let b = if module { self.g.boxed_value(&Ty::Record(id.clone()), &slot.ty, b) } else { b };
            parts.push((key, b));
        }
        let tyname = match self.t() {
            Target::TypeScript => String::new(),
            _ => self.g.record_type(&id),
        };
        // a value of a sum is told apart by its kind in TypeScript (DESIGN 23.8)
        if let (RecId::Variant { value, .. }, Target::TypeScript, None) = (&id, self.t(), &from) {
            parts.insert(0, ("kind".into(), self.g.string(value)));
        }
        let ex = match (from, self.t()) {
            (None, _) => Ex::Record(tyname, parts),
            (Some(fv), Target::TypeScript) => {
                parts.insert(0, ("...".into(), fv.ex));
                Ex::Record(tyname, parts)
            }
            (Some(fv), Target::Python) => {
                let kw: Vec<String> = parts.iter().map(|(k, v)| format!("{k}={}", v.print(Target::Python))).collect();
                Ex::atom(format!("dataclasses.replace({}{}{})", fv.ex.print(Target::Python), if kw.is_empty() { "" } else { ", " }, kw.join(", ")))
            }
            (Some(fv), Target::Go) => {
                let n = self.temp();
                self.bind(&n, &tyname, fv.ex);
                for (k, v) in parts {
                    self.emit(Node::Line(format!("{n}.{k} = {}", v.print(Target::Go))));
                }
                Ex::atom(n)
            }
            // Rust: `T { a: x, ..base }`, with no base where every field is written (clippy's
            // `needless_update`)
            (Some(fv), Target::Rust) => {
                if parts.len() < fields.len() {
                    parts.push(("...".into(), self.g.owned(&fv)));
                }
                Ex::Record(tyname, parts)
            }
        };
        // Rust: a value of one of a sum's values, typed as the sum where the place says so, is the
        // sum's variant that holds the record (`Order::Cart(OrderCart { … })`, DESIGN 36.2)
        if let (Target::Rust, RecId::Variant { value, .. }, Ty::Enum(sum)) = (self.t(), &id, ty.present())
            && self.g.p.is_sum(sum)
        {
            let ctor = self.g.variant_of(sum, value);
            return Val::new(Ex::call(ctor, vec![ex]), ty);
        }
        Val::new(ex, ty)
    }

    // ── Calls ──────────────────────────────────────────────────────────────

    fn call(&mut self, e: &Expr, callee: &Expr, args: &[Arg]) -> Val {
        if let Ref::CallValue = self.rf(e) {
            return self.call_value(e, callee, args);
        }
        match self.rf(callee) {
            Ref::Fn(canon, name) => self.call_fn(e, &canon, &name, args),
            Ref::Rule(canon) => self.call_rule(e, callee, &canon, args),
            Ref::Date(canon, date) => self.call_date(e, callee, &canon, &date, args),
            Ref::Transfer(id) => self.transfer(e, &id, args),
            Ref::Builtin(name) => {
                let xs: Vec<&Expr> = args.iter().map(Arg::expr).collect();
                self.builtin(e, name, &xs)
            }
            // a builtin written after a value: the value is its first argument (DESIGN 21.4)
            Ref::Method(name) => {
                let ExprKind::Field(base, _, _) = &callee.kind else { return Val::new(self.g.null(), self.ty(e)) };
                let mut xs: Vec<&Expr> = vec![&**base];
                xs.extend(args.iter().map(Arg::expr));
                self.builtin(e, name, &xs)
            }
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
        // a generic function: its parameters' types and its answer's with the types the call gives its
        // type parameters put in (DESIGN 26.2)
        let tn = crate::names::tnames(&info.tparams);
        let targs: Vec<Ty> = g.c.typed.inst.get(&e.id).cloned().unwrap_or_default();
        let slots: Vec<Slot> = info.params.iter().map(|p| subst_slot(&p.slot, &tn, &targs)).collect();
        let ret_ty = subst(&info.ret.ty, &tn, &targs);
        let to: Vec<Option<(usize, Ty)>> = args
            .iter()
            .map(|a| match a {
                Arg::Named(n, _, _) => info.params.iter().position(|p| p.name.is(n)).map(|pi| (pi, slots[pi].ty.clone())),
                _ => None,
            })
            .collect();
        let checked = args.iter().zip(&to).any(|(a, t)| t.as_ref().is_some_and(|(pi, _)| g.open(a.expr().id, |w| matches!(w, What::Param { .. })) && g.outside_fn(&slots[*pi], Reach::Call).is_some()));
        if self.t() == Target::Rust {
            for (a, t) in args.iter().zip(&to) {
                if let Some((pi, _)) = t
                    && matches!(info.params[*pi].slot.ty, Ty::Fn(..))
                {
                    self.fn_place.insert(a.expr().id, info.params[*pi].slot.ty.clone());
                }
            }
        }
        let (vals, keep) = self.arg_values(&es, &to, checked);
        if self.t() == Target::Rust {
            return self.call_fn_rs(e, canon, &callee, fi, args, vals, keep, &to, &slots, &targs, &ret_ty);
        }
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
            let Some(fun) = g.outside_fn(&slots[pi], Reach::Call) else { continue };
            let b = self.spill_as(given[pi].clone().unwrap_or(g.null()), &g.boundary(&slots[pi].ty));
            given[pi] = Some(b.clone());
            let o = Ex::atom(self.temp());
            let fail = g.fail_outside("range", e.span.line, &Said::Given { fun: info.name.text.clone() }, &o, &slots[pi], Reach::Call);
            let stmts = g.if_found(&o, Ex::call(fun, vec![b, g.string(&p.name.text)]), fail);
            for s in stmts {
                self.emit(s);
            }
        }
        let args: Vec<Ex> = given.into_iter().map(|x| x.unwrap_or(g.null())).collect();
        let raw = if info.inner() {
            // the one function of a generic function, with the types the call gives its type parameters
            // written (Python reads them from the arguments and the place), and before the arguments the
            // order of each type parameter held to keys (DESIGN 26.7)
            let lead: Vec<Ex> = info.tparams.iter().zip(&targs).filter(|(p, _)| p.key).map(|(_, a)| if self.t() == Target::Python { g.py_key_fn(a) } else { g.order_fn(a, false) }).collect();
            let f = generic_name(self.t(), &callee, fi);
            let f = if *canon == g.c.canon { f } else { format!("{}.{f}", g.imp(Import::Module(canon.to_path_buf()))) };
            let written: Vec<String> = targs.iter().map(|a| g.boundary(a)).collect();
            match self.t() {
                // one that takes no type parameters (a function that takes functions, a `private` one)
                Target::TypeScript | Target::Go if written.is_empty() => Ex::call(f, lead.into_iter().chain(args).collect()),
                Target::TypeScript => Ex::call(format!("{f}<{}>", written.join(", ")), lead.into_iter().chain(args).collect()),
                Target::Go => Ex::call(format!("{f}[{}]", written.join(", ")), lead.into_iter().chain(args).collect()),
                Target::Python => {
                    let names = g.callee_params(&callee, fi);
                    Ex::CallKw(Box::new(Ex::atom(f)), lead.into_iter().map(|x| (String::new(), x)).chain(names.into_iter().zip(args)).collect())
                }
                Target::Rust => unreachable!("Rust calls with call_fn_rs"),
            }
        } else if *canon == g.c.canon {
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
        let raw = self.spill_as(raw, &g.boundary(&ret_ty));
        let v = self.from_boundary(raw, &ret_ty, &ret_ty);
        Val::new(v.ex, ret_ty)
    }

    /// Rust: [`Fx::call_fn`]'s call (DESIGN 36.4): each argument as it travels, lent where it is not
    /// `Copy`; the module's function past its check (`_claim`), a generic one with its type arguments
    /// (`_first_or::<T>`), another module's as a host calls it; `?` where it can fail.
    #[allow(clippy::too_many_arguments)]
    fn call_fn_rs(&mut self, e: &Expr, canon: &Path, callee: &Rc<Checked>, fi: usize, args: &[Arg], vals: Vec<Val>, keep: bool, to: &[Option<(usize, Ty)>], slots: &[Slot], targs: &[Ty], ret_ty: &Ty) -> Val {
        let g = self.g;
        let info = callee.decls.fns[fi].clone();
        let mut given: Vec<Option<Val>> = vec![None; info.params.len()];
        let mut at_of: Vec<Option<usize>> = vec![None; info.params.len()];
        for ((a, v), t) in args.iter().zip(vals).zip(to) {
            let (Arg::Named(_, _, x), Some((pi, pt))) = (a, t) else { continue };
            let v = self.adapt(v, pt, x);
            let v = if keep { self.spill(v) } else { v };
            // as it travels: read anew where it travels otherwise, else as it is
            let b = if g.same_out(pt) && !matches!(pt, Ty::Num(_)) { v } else { Val::new(self.to_boundary(v, pt), pt.clone()) };
            given[*pi] = Some(b);
            at_of[*pi] = Some(x.id);
        }
        for (pi, p) in info.params.iter().enumerate() {
            let Some(x) = at_of[pi] else { continue };
            if !g.open(x, |w| matches!(w, What::Param { .. })) {
                continue;
            }
            let Some(fun) = g.outside_fn(&slots[pi], Reach::Call) else { continue };
            let Some(b) = given[pi].take() else { continue };
            let b = self.spill(b);
            let o = Ex::atom(self.temp());
            let fail = g.fail_outside("range", e.span.line, &Said::Given { fun: info.name.text.clone() }, &o, &slots[pi], Reach::Call);
            let stmts = g.if_found(&o, Ex::call(fun, vec![g.lent(&b), g.string(&p.name.text)]), fail);
            for s in stmts {
                self.emit(s);
            }
            given[pi] = Some(b);
        }
        let args: Vec<Ex> = given.into_iter().zip(&info.params).map(|(x, p)| x.map(|v| g.lent_as(&v, &p.slot.ty)).unwrap_or(g.null())).collect();
        let fails = rs_fails(callee, fi);
        let here = *canon == g.c.canon;
        let m = || g.imp(Import::Module(canon.to_path_buf()));
        let raw = if info.inner() {
            let f = generic_name(Target::Rust, callee, fi);
            let f = if here { f } else { g.path(&m(), &f) };
            let written: Vec<String> = targs.iter().map(|a| g.boundary(a)).collect();
            if written.is_empty() { Ex::call(f, args) } else { Ex::call(format!("{f}::<{}>", written.join(", ")), args) }
        } else if here {
            Ex::call(inner_name(Target::Rust, callee, fi), args)
        } else {
            Ex::call(g.path(&m(), &fn_name(Target::Rust, callee, fi)), args)
        };
        let raw = if fails { self.carried(raw) } else { raw };
        let raw = self.spill_as(raw, &g.boundary(ret_ty));
        let v = self.from_boundary(raw, ret_ty, ret_ty);
        Val::held(v.ex, ret_ty.clone(), v.held)
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
                // Rust: a list of the rule's elements, lent (`&[Element]`)
                None if self.t() == Target::Rust => g.lent(&v),
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
            _ => Ex::call(g.path(&r, &call.function), ordered),
        };
        let out = self.temp();
        let said = g.say(&tr!("規則 `{name}` が入力を受け付けません", "The rule `{name}` refuses its inputs"));
        let line = e.span.line;
        if self.t() == Target::Rust {
            let ex = g.answered_rs(raw, open, "rule", line, &said);
            self.bind(&out, &rt, ex);
            return Val::held(Ex::atom(out), ty, Held::Kept);
        }
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
            Target::Rust => unreachable!("Rust's call of a rule is written above"),
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
                // Rust: koyomi's `Date`, field by field
                (ritsu_ports::DateKind::Date, Target::Rust) => {
                    let x = self.spill(v);
                    Ex::Record(format!("{d}::Date"), ["year", "month", "day"].iter().map(|f| (f.to_string(), x.ex.clone().dot(*f))).collect())
                }
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
        let raw = Ex::call(g.path(&d, &fname), argv);
        let out = self.temp();
        let said = g.say(&tr!("日付 `{name}` が求まりません", "The date `{name}` cannot be worked out"));
        let line = e.span.line;
        let dt = g.flight(&Ty::Date);
        if self.t() == Target::Rust {
            let ex = g.answered_rs(raw, open, "date", line, &said);
            let day = self.temp();
            self.bind(&day, &format!("{d}::Date"), ex);
            let k = g.imp(Import::Kaname);
            let fields = ["year", "month", "day"].iter().map(|f| (f.to_string(), Ex::atom(day.clone()).dot(*f))).collect();
            self.bind(&out, &dt, Ex::Record(format!("{k}::Date"), fields));
            return Val::new(Ex::atom(out), ty);
        }
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
            Target::Rust => unreachable!("Rust's call of a date is written above"),
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
                let shown = g.kcall(&g.helper("numText", "num_text", "IntText"), vec![if matches!(self.t(), Target::Go | Target::Rust) { v.ex.clone() } else { g.to_q_ex(v.ex.clone()) }, g.string(&unit_suffix(&nt.unit))]);
                let parts = template(&tr!("振替 `{t}` の額 `{n}` に、負の値 \u{1}shown\u{1} を渡しています", "The amount `{n}` of the transfer `{t}` is given \u{1}shown\u{1}, which is below 0"), g.lang);
                let msg = if self.t() == Target::Rust {
                    g.rs_format(parts.into_iter().map(|p| p.map_err(|_| shown.clone())).collect())
                } else {
                    parts
                        .into_iter()
                        .map(|p| match p {
                            Ok(s) => g.string(&s),
                            Err(_) => shown.clone(),
                        })
                        .reduce(|a, b| Ex::bin(Op::Add, a, b))
                        .unwrap_or(g.string(""))
                };
                let fail = if self.t() == Target::Rust {
                    g.fail_line("range", x.span.line, msg)
                } else {
                    Node::Line(g.stmt(g.kcall(&g.helper("fail", "fail", "Fail"), vec![g.string("range"), g.module_const(), Ex::atom(x.span.line.to_string()), msg])))
                };
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
                } else if self.t() == Target::Rust {
                    // a post's amounts that may be left out are `Option`s, `None` where not given; the
                    // fields in the struct's order
                    let amounts = super::books::optional_amounts_in(Target::Rust, &book, &id.transfer, &id.op);
                    let mut args: Vec<(String, Ex)> = args.into_iter().map(|(n, v)| if amounts.contains(&n) { (n, Ex::call("Some", vec![v])) } else { (n, v) }).collect();
                    for a in &amounts {
                        if !args.iter().any(|(n, _)| n == a) {
                            args.push((a.clone(), g.null()));
                        }
                    }
                    let order: Vec<String> = super::books::op_params(&book, &id.transfer, &id.op).iter().filter_map(|(i, _)| names.get(*i).cloned()).collect();
                    args.sort_by_key(|(n, _)| order.iter().position(|o| o == n));
                    args
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

    fn builtin(&mut self, e: &Expr, name: &str, xs: &[&Expr]) -> Val {
        match (name, xs.len()) {
            ("reduce", 3) => self.reduce(e, xs[0], xs[1], xs[2]),
            (
                "count" | "sum" | "any" | "all" | "map" | "first" | "last" | "max_of" | "min_of" | "running_totals" | "sort_by" | "sort_by_desc" | "max_by" | "min_by" | "flat_map" | "filter_map" | "group_by",
                2,
            ) => self.walk(e, name, xs[0], xs[1]),
            ("filter", 2) if matches!(self.ty(xs[0]).present(), Ty::Dict(..)) => self.dict_filter(e, xs[0], xs[1]),
            ("filter", 2) => self.walk(e, "filter", xs[0], xs[1]),
            ("to_dict", 3) => self.list_to_dict(e, xs[0], xs[1], xs[2]),
            ("map_values", 2) => self.map_values(e, xs[0], xs[1]),
            ("merge_with", 3) => self.merge_with(e, xs[0], xs[1], xs[2]),
            _ => {
                let vals = self.values(xs);
                self.scalar(e, name, xs, vals)
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
        let el_held = if self.t() == Target::Rust && !self.g.is_copy(&elem_ty) { Held::Ref } else { Held::Made };
        self.scope.push((ps.last().map(|(p, _)| p.clone()).unwrap_or_default(), Val::held(Ex::atom(el_name.clone()), elem_ty.clone(), el_held), used.clone()));
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
            Target::Rust => {
                let pat = match (used.get(), self.g.is_copy(&elem_ty)) {
                    (false, _) => "_".to_string(),
                    (true, true) => format!("&{el_name}"),
                    (true, false) => el_name.clone(),
                };
                format!("for {pat} in {}", self.g.iter_rs(&xs.ex))
            }
        };
        Node::Block(head, stmts, Vec::new())
    }

    /// A function given to a builtin, its parameters bound to `args`: its answer, and the statements
    /// that work it out.
    fn apply(&mut self, lam: &Expr, args: Vec<Val>) -> (Val, Vec<Node>) {
        let ExprKind::Lambda(ps, lbody) = &lam.kind else { return (Val::new(self.g.null(), Ty::Error), Vec::new()) };
        let mark = self.scope.len();
        for ((p, _), v) in ps.iter().zip(args) {
            self.scope.push((p.clone(), v, Rc::new(Cell::new(true))));
        }
        let out = self.apart(|s| s.expr(lbody));
        self.scope.truncate(mark);
        out
    }

    /// [`Fx::apply`], with whether the function reads each of its parameters (Rust names one it does
    /// not read `_`).
    fn apply_seen(&mut self, lam: &Expr, args: Vec<Val>) -> (Val, Vec<Node>, Vec<bool>) {
        let ExprKind::Lambda(ps, lbody) = &lam.kind else { return (Val::new(self.g.null(), Ty::Error), Vec::new(), Vec::new()) };
        let mark = self.scope.len();
        let mut cells = Vec::new();
        for ((p, _), v) in ps.iter().zip(args) {
            let c = Rc::new(Cell::new(false));
            cells.push(c.clone());
            self.scope.push((p.clone(), v, c));
        }
        let (v, stmts) = self.apart(|s| s.expr(lbody));
        self.scope.truncate(mark);
        (v, stmts, cells.iter().map(|c| c.get()).collect())
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
            "flat_map" => Some(me),
            "filter_map" => match me {
                Ty::List(t) => Some(Ty::Opt(t)),
                _ => None,
            },
            "running_totals" => match me {
                Ty::List(t) => match *t {
                    Ty::Record(RecId::Running(_, w)) => Some(*w),
                    _ => None,
                },
                _ => None,
            },
            "group_by" => match me {
                Ty::Dict(k, _) => Some(*k),
                _ => None,
            },
            _ => None,
        }
    }

    fn walk(&mut self, e: &Expr, name: &str, list: &Expr, lam: &Expr) -> Val {
        let ty = self.ty(e);
        let xs = self.expr(list);
        let xs = self.spill(xs);
        // `last` walks the list from its end (DESIGN 24.3)
        let xs = if name == "last" {
            let r = match self.t() {
                Target::Python => Ex::Index(Box::new(xs.ex.clone()), Box::new(Ex::atom("::-1"))),
                Target::Rust => self.g.kcall("Reverse", vec![self.g.lent(&xs)]),
                _ => self.g.kcall(&self.g.helper("reverse", "", "Reverse"), vec![xs.ex.clone()]),
            };
            self.spill(Val::new(r, xs.ty.clone()))
        } else {
            xs
        };
        let name = if name == "last" { "first" } else { name };
        let to = self.lambda_place(e, name);
        let lam_ty = self.ty(lam);
        let t = self.t();
        let g = self.g;
        let acc = self.temp();
        let brk = Node::Line(if matches!(t, Target::TypeScript | Target::Rust) { "break;".into() } else { "break".into() });
        let truth = |v: &Val| v.ex.clone();
        let el_of = |lam: &Expr| match &lam.kind {
            ExprKind::Lambda(ps, _) => ps.last().map(|(p, _)| p.clone()).unwrap_or_default(),
            _ => String::new(),
        };
        let elem_ty = match &xs.ty {
            Ty::List(x) => (**x).clone(),
            _ => Ty::Error,
        };
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
                let el = el_of(lam);
                let node = self.each(xs, lam, vec![], |s, v| {
                    let xv = s.lookup(&el);
                    let x = xv.as_ref().map(|x| if s.t() == Target::Rust { s.g.owned(x) } else { x.ex.clone() }).unwrap_or(s.g.null());
                    let x = match (s.t(), &ty) {
                        // an element that is there, where one may be absent
                        (Target::Go, Ty::Opt(_)) if !matches!(s.lookup(&el).map(|x| x.ty), Some(Ty::Opt(_))) => s.g.kcall("Some", vec![x]),
                        (Target::Rust, Ty::Opt(_)) if !matches!(s.lookup(&el).map(|x| x.ty), Some(Ty::Opt(_))) => Ex::call("Some", vec![x]),
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
                let el = el_of(lam);
                let node = self.each(xs, lam, vec![], |s, v| {
                    let x = s.lookup(&el).map(|x| s.g.owned(&x)).unwrap_or(s.g.null());
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
                    vec![s.push(&a2, s.g.owned(&v))]
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
                    let o = if max { Op::Gt } else { Op::Lt };
                    if s.t() == Target::Rust {
                        // the best so far, none or below (above) this one
                        let best = s.g.lent(&Val::held(Ex::atom(a2.clone()), ty.clone(), Held::Kept));
                        let test = best.method("is_none_or", vec![Ex::atom(format!("|b| {}", Ex::bin(o, s.g.lent(&v), Ex::atom("b")).print(Target::Rust)))]);
                        return vec![Node::If(vec![(test, vec![s.assign_line(&a2, Ex::call("Some", vec![s.g.owned(&v)]))])], Vec::new())];
                    }
                    let better = s.order(&present, v.ex.clone(), best, o);
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
            // each element with its key, then in the order of the keys, two of one key in the order of
            // the list (DESIGN 24.6)
            "sort_by" | "sort_by_desc" => {
                let kt = plain_num(&lam_ty);
                let (kf, ef) = (g.flight(&kt), g.flight(&elem_ty));
                let k = g.imp(Import::Kaname);
                let lt = match t {
                    Target::TypeScript => format!("[{kf}, {ef}][]"),
                    Target::Python => format!("list[tuple[{kf}, {ef}]]"),
                    Target::Go => format!("[]{k}.Pair[{kf}, {ef}]"),
                    Target::Rust => format!("Vec<({kf}, {ef})>"),
                };
                self.new_list(&acc, &lt);
                let a2 = acc.clone();
                let el = el_of(lam);
                let node = self.each(xs, lam, vec![], |s, v| {
                    let v = s.adapt(v, &kt, lam);
                    let x = s.lookup(&el).map(|x| if s.t() == Target::Rust { s.g.owned(&x) } else { x.ex }).unwrap_or(s.g.null());
                    let pair = match s.t() {
                        Target::TypeScript => Ex::List(String::new(), vec![v.ex, x]),
                        Target::Python => Ex::atom(format!("({}, {})", v.ex.print(Target::Python), x.print(Target::Python))),
                        Target::Go => Ex::Record(format!("{k}.Pair[{kf}, {ef}]"), vec![("Key".into(), v.ex), ("Item".into(), x)]),
                        Target::Rust => Ex::atom(format!("({}, {})", s.g.owned(&v).print(Target::Rust), x.print(Target::Rust))),
                    };
                    vec![s.push(&a2, pair)]
                });
                self.emit(node);
                let desc = g.boolean(name == "sort_by_desc");
                // Rust: the keys' `Ord`
                if t == Target::Rust {
                    return Val::new(g.kcall("ByKey", vec![Ex::atom(acc), desc]), ty);
                }
                let order = match t {
                    Target::Python => g.py_key_fn(&kt),
                    _ => g.order_fn(&kt, true),
                };
                let f = g.helper("byKey", "by_key", "ByKey");
                Val::new(g.kcall(&f, vec![Ex::atom(acc), order, desc]), ty)
            }
            // the first element of the greatest key, or of the least
            "max_by" | "min_by" => {
                let kt = plain_num(&lam_ty);
                let fty = g.flight(&ty);
                self.mutable(&acc, &fty, g.null());
                let bk = self.temp();
                let found = self.temp();
                match t {
                    Target::Go => {
                        self.declare(&bk, &g.flight(&kt));
                        self.mutable(&found, "bool", g.boolean(false));
                    }
                    _ => {
                        let kf = g.flight(&kt);
                        self.mutable(&bk, &g.flight(&Ty::Opt(Box::new(kt.clone()))), g.null());
                        let _ = kf;
                    }
                }
                let (a2, b2, f2) = (acc.clone(), bk.clone(), found.clone());
                let el = el_of(lam);
                let max = name == "max_by";
                let node = self.each(xs, lam, vec![], |s, v| {
                    let v = s.adapt(v, &kt, lam);
                    let v = s.spill(v);
                    let x = s.lookup(&el).unwrap_or(Val::new(s.g.null(), Ty::Error));
                    if s.t() == Target::Rust {
                        // the greatest (least) key so far, none or below (above) this one
                        let o = if max { Op::Gt } else { Op::Lt };
                        let best = s.g.lent(&Val::held(Ex::atom(b2.clone()), Ty::Opt(Box::new(kt.clone())), Held::Kept));
                        let test = best.method("is_none_or", vec![Ex::atom(format!("|b| {}", Ex::bin(o, s.g.lent(&v), Ex::atom("b")).print(Target::Rust)))]);
                        let store = if matches!(x.ty, Ty::Opt(_)) { s.g.owned(&x) } else { Ex::call("Some", vec![s.g.owned(&x)]) };
                        let then = vec![s.assign_line(&a2, store), s.assign_line(&b2, Ex::call("Some", vec![s.g.owned(&v)]))];
                        return vec![Node::If(vec![(test, then)], Vec::new())];
                    }
                    let cmp = |s: &Self| -> Ex {
                        let o = if max { Op::Gt } else { Op::Lt };
                        match s.t() {
                            Target::Python => Ex::bin(o, s.g.py_key(&kt, v.ex.clone()), s.g.py_key(&kt, Ex::atom(b2.clone()))),
                            _ => Ex::bin(o, s.g.order_ex(&kt, v.ex.clone(), Ex::atom(b2.clone()), true), Ex::atom("0")),
                        }
                    };
                    let first = match s.t() {
                        Target::Go => Ex::Not(Box::new(Ex::atom(f2.clone()))),
                        _ => s.is_null(&Ex::atom(b2.clone())),
                    };
                    let store = match s.t() {
                        Target::Go if !matches!(x.ty, Ty::Opt(_)) => s.g.kcall("Some", vec![x.ex.clone()]),
                        _ => x.ex.clone(),
                    };
                    let mut then = vec![s.assign_line(&a2, store), s.assign_line(&b2, v.ex.clone())];
                    if s.t() == Target::Go {
                        then.push(s.assign_line(&f2, Ex::atom("true")));
                    }
                    vec![Node::If(vec![(Ex::bin(Op::Or, first, cmp(s)), then)], Vec::new())]
                });
                self.emit(node);
                Val::new(Ex::atom(acc), ty)
            }
            // the lists the function answers, one after another
            "flat_map" => {
                let fty = g.flight(&ty);
                self.new_list(&acc, &fty);
                let a2 = acc.clone();
                let to = to.unwrap_or(ty.clone());
                let node = self.each(xs, lam, vec![], |s, v| {
                    let v = s.adapt(v, &to, lam);
                    let t = s.t();
                    vec![Node::Line(match t {
                        Target::TypeScript => format!("{a2}.push(...{});", v.ex.print(t)),
                        Target::Python => format!("{a2}.extend({})", v.ex.print(t)),
                        Target::Go => format!("{a2} = append({a2}, {}...)", v.ex.print(t)),
                        Target::Rust if v.held == Held::Made => format!("{a2}.extend({});", v.ex.print(t)),
                        Target::Rust => format!("{a2}.extend_from_slice({});", s.g.lent(&v).print(t)),
                    })]
                });
                self.emit(node);
                Val::new(Ex::atom(acc), ty)
            }
            // the values the function answers that are there
            "filter_map" => {
                let fty = g.flight(&ty);
                self.new_list(&acc, &fty);
                let a2 = acc.clone();
                let to = to.unwrap_or(ty.clone());
                let node = self.each(xs, lam, vec![], |s, v| {
                    let v = s.adapt(v, &to, lam);
                    if s.t() == Target::Rust {
                        // the value there, taken where it is made, else copied from where it is lent
                        let inner = v.ty.present().clone();
                        let y = s.temp();
                        let by_value = s.g.is_copy(&inner) || v.held == Held::Made;
                        let src = if by_value { v.ex.clone() } else { s.g.lent(&v) };
                        let yv = Val::held(Ex::atom(y.clone()), inner, if by_value { Held::Made } else { Held::Lent });
                        let push = s.push(&a2, s.g.owned(&yv));
                        return vec![Node::Block(format!("if let Some({y}) = {}", super::ex::rust_cond(&src)), vec![push], Vec::new())];
                    }
                    let v = s.spill(v);
                    let there = match s.t() {
                        Target::Python => Ex::atom(format!("{} is not None", v.ex.print(Target::Python))),
                        _ => Ex::bin(Op::Ne, v.ex.clone(), s.g.null()),
                    };
                    let x = match s.t() {
                        Target::Go => Ex::Deref(Box::new(v.ex.clone())),
                        _ => v.ex.clone(),
                    };
                    vec![Node::If(vec![(there, vec![s.push(&a2, x)])], Vec::new())]
                });
                self.emit(node);
                Val::new(Ex::atom(acc), ty)
            }
            // the elements of each key, in the order of the list, as they travel; the dict in the order
            // of its keys
            "group_by" => {
                let (kt, _) = dict_tys(&ty);
                let (kb, eb) = (g.boundary(&kt), g.boundary(&elem_ty));
                let k = g.imp(Import::Kaname);
                match t {
                    Target::TypeScript => self.emit(Node::Line(format!("const {acc} = new Map<{kb}, {eb}[]>();"))),
                    Target::Python => self.emit(Node::Line(format!("{acc}: dict[{kb}, list[{eb}]] = {{}}"))),
                    Target::Go => self.emit(Node::Line(format!("var {acc} = map[{kb}][]{eb}{{}}"))),
                    Target::Rust => self.emit(Node::Line(format!("let mut {acc}: {} = {}::new();", g.dict_type(kb.clone(), format!("Vec<{eb}>")), g.std_type("BTreeMap")))),
                }
                let a2 = acc.clone();
                let el = el_of(lam);
                let et = elem_ty.clone();
                let node = self.each(xs, lam, vec![], |s, v| {
                    let v = s.adapt(v, &kt, lam);
                    let kv = s.to_boundary(v, &kt);
                    let kv = s.spill_as(kv, &kb);
                    let x = s.lookup(&el).unwrap_or(Val::new(s.g.null(), Ty::Error));
                    let xb = s.to_boundary(Val::held(x.ex, et.clone(), x.held), &et);
                    let t = s.t();
                    vec![Node::Line(match t {
                        Target::TypeScript => format!("{k}.groupPush({a2}, {}, {});", kv.print(t), xb.print(t)),
                        Target::Python => format!("{a2}.setdefault({}, []).append({})", kv.print(t), xb.print(t)),
                        Target::Go => format!("{a2}[{0}] = append({a2}[{0}], {1})", kv.print(t), xb.print(t)),
                        Target::Rust => format!("{a2}.entry({}).or_default().push({});", kv.print(t), xb.print(t)),
                    })]
                });
                self.emit(node);
                let out = match t {
                    Target::TypeScript => g.kcall("mapOf", vec![Ex::atom(acc), g.order_fn(&kt, false)]),
                    Target::Python => g.kcall("dict_of", vec![Ex::Call(Box::new(Ex::atom(acc).dot("items")), vec![]), g.py_key_fn(&kt)]),
                    Target::Go | Target::Rust => Ex::atom(acc),
                };
                Val::held(out, ty, Held::Kept)
            }
            _ => Val::new(g.null(), ty),
        }
    }

    /// `d.filter(e => …)`: the function given each pair as an entry, in the order of the keys; the pairs
    /// it holds for, in that order (DESIGN 24.6).
    fn dict_filter(&mut self, e: &Expr, x: &Expr, lam: &Expr) -> Val {
        let ty = self.ty(e);
        let g = self.g;
        let t = self.t();
        let d = self.expr(x);
        let d = self.spill(d);
        let (kt, vt) = dict_tys(&d.ty);
        let entries = match t {
            Target::TypeScript => g.kcall("entriesOf", vec![d.ex.clone(), g.order_fn(&kt, false)]),
            Target::Python => g.kcall("entries_of", vec![d.ex.clone(), g.py_key_fn(&kt)]),
            Target::Go => g.kcall("EntriesOf", vec![d.ex.clone(), g.order_fn(&kt, false)]),
            Target::Rust => g.kcall("EntriesOf", vec![g.lent(&d)]),
        };
        let entries = self.spill(Val::new(entries, Ty::List(Box::new(Ty::Record(RecId::Entry(Box::new(kt.clone()), Box::new(vt.clone())))))));
        let out = self.temp();
        let (kb, vb) = (g.boundary(&kt), g.boundary(&vt));
        match t {
            Target::TypeScript => self.emit(Node::Line(format!("const {out} = new Map<{kb}, {vb}>();"))),
            Target::Python => self.emit(Node::Line(format!("{out}: dict[{kb}, {vb}] = {{}}"))),
            Target::Go => self.emit(Node::Line(format!("var {out} = map[{kb}]{vb}{{}}"))),
            Target::Rust => self.emit(Node::Line(format!("let mut {out}: {} = {}::new();", g.dict_type(kb.clone(), vb.clone()), g.std_type("BTreeMap")))),
        }
        let el = match &lam.kind {
            ExprKind::Lambda(ps, _) => ps.last().map(|(p, _)| p.clone()).unwrap_or_default(),
            _ => String::new(),
        };
        let o2 = out.clone();
        let node = self.each(entries, lam, vec![], |s, v| {
            let en = s.lookup(&el).map(|x| x.ex).unwrap_or(s.g.null());
            let (k, val) = match s.t() {
                Target::Go => (en.clone().dot("Key"), en.dot("Value")),
                _ => (en.clone().dot("key"), en.dot("value")),
            };
            let t = s.t();
            let put = Node::Line(match t {
                Target::TypeScript => format!("{o2}.set({}, {});", k.print(t), val.print(t)),
                // Rust: the pair copied out of the entry
                Target::Rust => {
                    let kx = s.g.owned(&Val::held(k, kt.clone(), Held::Kept));
                    let vx = s.g.owned(&Val::held(val, vt.clone(), Held::Kept));
                    format!("{o2}.insert({}, {});", kx.print(t), vx.print(t))
                }
                _ => format!("{o2}[{}] = {}", k.print(t), val.print(t)),
            });
            vec![Node::If(vec![(v.ex, vec![put])], Vec::new())]
        });
        self.emit(node);
        Val::held(Ex::atom(out), ty, if t == Target::Rust { Held::Kept } else { Held::Made })
    }

    /// `xs.to_dict(x => key, x => value)`: for each element its key, then its value, each as it
    /// travels; the later of two elements of one key wins (DESIGN 24.6).
    fn list_to_dict(&mut self, e: &Expr, list: &Expr, kf: &Expr, vf: &Expr) -> Val {
        let ty = self.ty(e);
        let g = self.g;
        let t = self.t();
        let xs = self.expr(list);
        let xs = self.spill(xs);
        let elem_ty = match &xs.ty {
            Ty::List(x) => (**x).clone(),
            _ => Ty::Error,
        };
        let (kt, vt) = dict_tys(&ty);
        let (kb, vb) = (g.boundary(&kt), g.boundary(&vt));
        let out = self.temp();
        match t {
            Target::TypeScript => self.emit(Node::Line(format!("const {out} = new Map<{kb}, {vb}>();"))),
            Target::Python => self.emit(Node::Line(format!("{out}: dict[{kb}, {vb}] = {{}}"))),
            Target::Go => self.emit(Node::Line(format!("var {out} = map[{kb}]{vb}{{}}"))),
            Target::Rust => self.emit(Node::Line(format!("let mut {out}: {} = {}::new();", g.dict_type(kb.clone(), vb.clone()), g.std_type("BTreeMap")))),
        }
        let el = self.temp();
        if t == Target::Rust {
            // Rust: each element reached through the list, by value where it is `Copy`
            let copy = g.is_copy(&elem_ty);
            let x = Val::held(Ex::atom(el.clone()), elem_ty.clone(), if copy { Held::Made } else { Held::Ref });
            let (k, mut body, ks) = self.apply_seen(kf, vec![x.clone()]);
            let (kx, pre) = self.apart(|s| {
                let k = s.adapt(k, &kt, kf);
                let k = s.to_boundary(k, &kt);
                s.spill_as(k, &kb)
            });
            body.extend(pre);
            let (v, more, vs) = self.apply_seen(vf, vec![x]);
            body.extend(more);
            let (vx, pre) = self.apart(|s| {
                let v = s.adapt(v, &vt, vf);
                s.to_boundary(v, &vt)
            });
            body.extend(pre);
            body.push(Node::Line(format!("{out}.insert({}, {});", kx.print(t), vx.print(t))));
            let read = ks.iter().chain(&vs).any(|b| *b);
            let pat = match (read, copy) {
                (false, _) => "_".to_string(),
                (true, true) => format!("&{el}"),
                (true, false) => el.clone(),
            };
            self.emit(Node::Block(format!("for {pat} in {}", self.g.iter_rs(&xs.ex)), body, Vec::new()));
            return Val::held(Ex::atom(out), ty, Held::Kept);
        }
        let x = Val::new(Ex::atom(el.clone()), elem_ty);
        let (k, mut body) = self.apply(kf, vec![x.clone()]);
        // Go: the element is used, whether the functions read it or not
        if t == Target::Go {
            body.insert(0, Node::Line(format!("_ = {el}")));
        }
        let (kx, pre) = self.apart(|s| {
            let k = s.adapt(k, &kt, kf);
            let k = s.to_boundary(k, &kt);
            s.spill_as(k, &kb)
        });
        body.extend(pre);
        let (v, more) = self.apply(vf, vec![x]);
        body.extend(more);
        let (vx, pre) = self.apart(|s| {
            let v = s.adapt(v, &vt, vf);
            s.to_boundary(v, &vt)
        });
        body.extend(pre);
        body.push(Node::Line(match t {
            Target::TypeScript => format!("{out}.set({}, {});", kx.print(t), vx.print(t)),
            _ => format!("{out}[{}] = {}", kx.print(t), vx.print(t)),
        }));
        let head = match t {
            Target::TypeScript => format!("for (const {el} of {})", xs.ex.print(t)),
            Target::Python => format!("for {el} in {}", xs.ex.print(t)),
            Target::Go | Target::Rust => format!("for _, {el} := range {}", xs.ex.print(t)),
        };
        self.emit(Node::Block(head, body, Vec::new()));
        let ex = match t {
            Target::TypeScript => g.kcall("mapOf", vec![Ex::atom(out), g.order_fn(&kt, false)]),
            Target::Python => g.kcall("dict_of", vec![Ex::Call(Box::new(Ex::atom(out).dot("items")), vec![]), g.py_key_fn(&kt)]),
            Target::Go | Target::Rust => Ex::atom(out),
        };
        Val::new(ex, ty)
    }

    /// `d.map_values(v => …)`: the function given each value, in the order of the keys.
    fn map_values(&mut self, e: &Expr, x: &Expr, lam: &Expr) -> Val {
        let ty = self.ty(e);
        let g = self.g;
        let t = self.t();
        let d = self.expr(x);
        let d = self.spill(d);
        let (kt, vt) = dict_tys(&d.ty);
        let (_, ut) = dict_tys(&ty);
        let (kb, ub) = (g.boundary(&kt), g.boundary(&ut));
        let out = self.temp();
        match t {
            Target::TypeScript => self.emit(Node::Line(format!("const {out} = new Map<{kb}, {ub}>();"))),
            Target::Python => self.emit(Node::Line(format!("{out}: dict[{kb}, {ub}] = {{}}"))),
            Target::Go => self.emit(Node::Line(format!("var {out} = map[{kb}]{ub}{{}}"))),
            Target::Rust => self.emit(Node::Line(format!("let mut {out}: {} = {}::new();", g.dict_type(kb.clone(), ub.clone()), g.std_type("BTreeMap")))),
        }
        // Rust: the pairs in the order of the keys, each reached through the dict
        if t == Target::Rust {
            let (kn, vn) = (self.temp(), self.temp());
            let r = g.through(Ex::atom(vn.clone()), &vt);
            let (v, mut body) = self.apart(|s| s.from_boundary_as(r.ex, &vt, &vt, r.held));
            let (u, more, seen) = self.apply_seen(lam, vec![v]);
            body.extend(more);
            let (ux, pre) = self.apart(|s| {
                let u = s.adapt(u, &ut, lam);
                s.to_boundary(u, &ut)
            });
            body.extend(pre);
            let kx = g.owned(&g.through(Ex::atom(kn.clone()), &kt));
            body.push(Node::Line(format!("{out}.insert({}, {});", kx.print(t), ux.print(t))));
            let vp = if seen.first().copied().unwrap_or(false) { vn } else { "_".to_string() };
            self.emit(Node::Block(format!("for ({kn}, {vp}) in {}.iter()", d.ex.print(t)), body, Vec::new()));
            return Val::held(Ex::atom(out), ty, Held::Kept);
        }
        let k = self.temp();
        let keys = self.sorted_keys(&d.ex, &kt);
        let raw = self.value_of(&d.ex, &Ex::atom(k.clone()), &vt);
        let (v, mut body) = self.apart(|s| {
            let v = s.val_of_raw(raw, &vt, &vt);
            s.spill(v)
        });
        let (u, more) = self.apply(lam, vec![v]);
        body.extend(more);
        let (ux, pre) = self.apart(|s| {
            let u = s.adapt(u, &ut, lam);
            s.to_boundary(u, &ut)
        });
        body.extend(pre);
        body.push(Node::Line(match t {
            Target::TypeScript => format!("{out}.set({k}, {});", ux.print(t)),
            _ => format!("{out}[{k}] = {}", ux.print(t)),
        }));
        let head = match t {
            Target::TypeScript => format!("for (const {k} of {})", keys.print(t)),
            Target::Python => format!("for {k} in {}", keys.print(t)),
            Target::Go | Target::Rust => format!("for _, {k} := range {}", keys.print(t)),
        };
        self.emit(Node::Block(head, body, Vec::new()));
        Val::new(Ex::atom(out), ty)
    }

    /// `d.merge_with(o, (a, b) => …)`: the pairs of both; a key of both given to the function with
    /// `d`'s value and `o`'s, in the order of the keys of `o` (DESIGN 24.6).
    fn merge_with(&mut self, e: &Expr, x: &Expr, o: &Expr, lam: &Expr) -> Val {
        let ty = self.ty(e);
        let g = self.g;
        let t = self.t();
        let vs = self.values(&[x, o]);
        let a = self.adapt(vs[0].clone(), &ty, x);
        let a = self.spill(a);
        let b = self.adapt(vs[1].clone(), &ty, o);
        let b = self.spill(b);
        let (kt, vt) = dict_tys(&ty);
        let (kb, vb) = (g.boundary(&kt), g.boundary(&vt));
        let out = self.temp();
        match t {
            Target::TypeScript => self.emit(Node::Line(format!("const {out} = new Map<{kb}, {vb}>({});", a.ex.print(t)))),
            Target::Python => self.emit(Node::Line(format!("{out}: dict[{kb}, {vb}] = dict({})", a.ex.print(t)))),
            Target::Go => self.emit(Node::Line(format!("var {out} = {}", g.kcall("Merge", vec![a.ex.clone(), Ex::atom(format!("map[{kb}]{vb}{{}}"))]).print(t)))),
            Target::Rust => self.emit(Node::Line(format!("let mut {out}: {} = {};", g.dict_type(kb.clone(), vb.clone()), g.owned(&a).print(t)))),
        }
        // Rust: the pairs of `o` in the order of its keys, each reached through the dict, and the
        // value of `d` where it has the key
        if t == Target::Rust {
            let (kn, bn, an) = (self.temp(), self.temp(), self.temp());
            let (ra, rb) = (g.through(Ex::atom(an.clone()), &vt), g.through(Ex::atom(bn.clone()), &vt));
            let (args, mut both) = self.apart(|s| {
                let p = s.from_boundary_as(ra.ex.clone(), &vt, &vt, ra.held);
                let q = s.from_boundary_as(rb.ex.clone(), &vt, &vt, rb.held);
                vec![p, q]
            });
            let (r, more, seen) = self.apply_seen(lam, args);
            both.extend(more);
            let (rx, pre) = self.apart(|s| {
                let r = s.adapt(r, &vt, lam);
                s.to_boundary(r, &vt)
            });
            both.extend(pre);
            let kx = g.owned(&g.through(Ex::atom(kn.clone()), &kt));
            both.push(Node::Line(format!("{out}.insert({}, {});", kx.print(t), rx.print(t))));
            let only = vec![Node::Line(format!("{out}.insert({}, {});", kx.print(t), g.owned(&rb).print(t)))];
            let node = if seen.first().copied().unwrap_or(false) {
                Node::Block(format!("if let Some({an}) = {}.get({kn})", a.ex.print(t)), both, vec![("else".to_string(), only)])
            } else {
                Node::If(vec![(Ex::Call(Box::new(a.ex.clone().dot("contains_key")), vec![Ex::atom(kn.clone())]), both)], only)
            };
            self.emit(Node::Block(format!("for ({kn}, {bn}) in {}.iter()", b.ex.print(t)), vec![node], Vec::new()));
            return Val::held(Ex::atom(out), ty, Held::Kept);
        }
        let k = self.temp();
        let keys = self.sorted_keys(&b.ex, &kt);
        let ka = Ex::atom(k.clone());
        let (va, vb_) = (self.value_of(&a.ex, &ka, &vt), self.value_of(&b.ex, &ka, &vt));
        let (args, mut both) = self.apart(|s| {
            let p = s.val_of_raw(va.clone(), &vt, &vt);
            let q = s.val_of_raw(vb_.clone(), &vt, &vt);
            vec![p, q]
        });
        let (r, more) = self.apply(lam, args);
        both.extend(more);
        let (rx, pre) = self.apart(|s| {
            let r = s.adapt(r, &vt, lam);
            s.to_boundary(r, &vt)
        });
        both.extend(pre);
        let set = |v: &Ex| {
            Node::Line(match t {
                Target::TypeScript => format!("{out}.set({k}, {});", v.print(t)),
                _ => format!("{out}[{k}] = {}", v.print(t)),
            })
        };
        both.push(set(&rx));
        let has = self.has_key(&a.ex, &ka);
        let body = vec![Node::If(vec![(has, both)], vec![set(&vb_)])];
        let head = match t {
            Target::TypeScript => format!("for (const {k} of {})", keys.print(t)),
            Target::Python => format!("for {k} in {}", keys.print(t)),
            Target::Go | Target::Rust => format!("for _, {k} := range {}", keys.print(t)),
        };
        self.emit(Node::Block(head, body, Vec::new()));
        let ex = match t {
            Target::TypeScript => g.kcall("mapOf", vec![Ex::atom(out), g.order_fn(&kt, false)]),
            Target::Python => g.kcall("dict_of", vec![Ex::Call(Box::new(Ex::atom(out).dot("items")), vec![]), g.py_key_fn(&kt)]),
            Target::Go | Target::Rust => Ex::atom(out),
        };
        Val::new(ex, ty)
    }

    /// A value as it travels that a call answers (`kaname.Get(…)`), as it is worked on: made a name of
    /// the type it travels as first, where it is no name.
    fn val_of_raw(&mut self, raw: Ex, slot: &Ty, ty: &Ty) -> Val {
        let b = self.g.boundary(slot);
        let raw = self.spill_as(raw, &b);
        self.from_boundary(raw, slot, ty)
    }

    /// The keys of a dict, or the members of a set, in their order, as they travel.
    fn sorted_keys(&self, d: &Ex, kt: &Ty) -> Ex {
        let g = self.g;
        match self.t() {
            Target::TypeScript => g.kcall("sortedKeys", vec![d.clone(), g.order_fn(kt, false)]),
            Target::Python => g.kcall("keys_of", vec![d.clone(), g.py_key_fn(kt)]),
            Target::Go => g.kcall("SortedKeys", vec![d.clone(), g.order_fn(kt, false)]),
            // a `BTreeMap`'s keys and a `BTreeSet`'s members are in their order
            Target::Rust => g.kcall("KeysOf", vec![d.clone()]),
        }
    }

    /// The value of a key a dict has, as it travels.
    fn value_of(&self, d: &Ex, k: &Ex, vt: &Ty) -> Ex {
        match self.t() {
            Target::TypeScript => Ex::atom(format!("({}.get({}) as {})", d.print(Target::TypeScript), k.print(Target::TypeScript), self.g.boundary(vt))),
            _ => Ex::Index(Box::new(d.clone()), Box::new(k.clone())),
        }
    }

    /// Whether a dict has a key, or a set a member.
    fn has_key(&self, d: &Ex, k: &Ex) -> Ex {
        match self.t() {
            Target::TypeScript => Ex::Call(Box::new(d.clone().dot("has")), vec![k.clone()]),
            Target::Python => Ex::atom(format!("({} in {})", k.print(Target::Python), d.print(Target::Python))),
            Target::Go => self.g.kcall("Has", vec![d.clone(), k.clone()]),
            Target::Rust => self.g.kcall("Has", vec![d.clone(), k.clone()]),
        }
    }

    /// A dict written out (DESIGN 24.6): each key, then its value, from the left, each as it travels;
    /// the later of two pairs of one key kept, the dict in the order of its keys.
    fn dict_literal(&mut self, e: &Expr, pairs: &[(Expr, Expr)]) -> Val {
        let ty = self.ty(e);
        let g = self.g;
        let t = self.t();
        let (kt, vt) = dict_tys(&ty);
        let (kb, vb) = (g.boundary(&kt), g.boundary(&vt));
        let mut parts: Vec<(Val, Vec<Node>)> = Vec::new();
        for (k, v) in pairs {
            for (x, xt) in [(k, &kt), (v, &vt)] {
                let (v, stmts) = self.apart(|s| {
                    let v = s.expr(x);
                    let v = s.adapt(v, xt, x);
                    let b = s.to_boundary(v, xt);
                    Val::new(b, xt.clone())
                });
                parts.push((v, stmts));
            }
        }
        let later: Vec<bool> = (0..parts.len()).map(|i| parts[i + 1..].iter().any(|(_, s)| !s.is_empty())).collect();
        let mut items = Vec::new();
        for (i, (v, stmts)) in parts.into_iter().enumerate() {
            self.out.extend(stmts);
            let ex = if later[i] {
                let n = self.temp();
                let ty = if i % 2 == 0 { kb.clone() } else { vb.clone() };
                self.bind(&n, &ty, v.ex);
                Ex::atom(n)
            } else {
                v.ex
            };
            items.push(ex);
        }
        let pairs: Vec<(Ex, Ex)> = items.chunks(2).map(|c| (c[0].clone(), c[1].clone())).collect();
        match t {
            Target::TypeScript => {
                if pairs.is_empty() {
                    return Val::new(Ex::atom(format!("new Map<{kb}, {vb}>()")), ty);
                }
                let list = Ex::List(String::new(), pairs.into_iter().map(|(a, b)| Ex::List(String::new(), vec![a, b])).collect());
                Val::new(Ex::Call(Box::new(Ex::atom(format!("{}.mapOf<{kb}, {vb}>", g.imp(Import::Kaname)))), vec![list, g.order_fn(&kt, false)]), ty)
            }
            Target::Python => {
                if pairs.is_empty() {
                    return Val::new(Ex::atom("{}"), ty);
                }
                let list = Ex::atom(format!("[{}]", pairs.iter().map(|(a, b)| format!("({}, {})", a.print(t), b.print(t))).collect::<Vec<_>>().join(", ")));
                Val::new(g.kcall("dict_of", vec![list, g.py_key_fn(&kt)]), ty)
            }
            Target::Go => {
                let n = self.temp();
                self.emit(Node::Line(format!("var {n} = map[{kb}]{vb}{{}}")));
                for (a, b) in pairs {
                    self.emit(Node::Line(format!("{n}[{}] = {}", a.print(t), b.print(t))));
                }
                Val::new(Ex::atom(n), ty)
            }
            // Rust: each pair put in from the left, so the later of one key is kept
            Target::Rust => {
                let m = g.std_type("BTreeMap");
                if pairs.is_empty() {
                    return Val::new(Ex::atom(format!("{m}::new()")), ty);
                }
                let n = self.temp();
                self.emit(Node::Line(format!("let mut {n}: {} = {m}::new();", g.dict_type(kb, vb))));
                for (a, b) in pairs {
                    self.emit(Node::Line(format!("{n}.insert({}, {});", a.print(t), b.print(t))));
                }
                Val::held(Ex::atom(n), ty, Held::Kept)
            }
        }
    }

    /// The builtins of a dict, a set and a list that take no function (DESIGN 24.6), from the values of
    /// their arguments, the dict, the set or the list first.
    fn collection(&mut self, e: &Expr, name: &str, xs: &[&Expr], vals: Vec<Val>) -> Val {
        let me = self.ty(e);
        let g = self.g;
        let t = self.t();
        let t0 = vals[0].ty.clone();
        let k = g.imp(Import::Kaname);
        // the i-th argument after the first, read in `to`, as it is worked on
        let arg = |s: &mut Self, i: usize, to: &Ty| -> Val { s.adapt(vals[i + 1].clone(), to, xs[i + 1]) };
        // Rust: the collection lent to a helper, and a key or a member it looks for lent too
        // (`d.get("a")`, `s.contains(&3)`)
        let lent0 = g.lent(&vals[0]);
        let sought = |s: &mut Self, to: &Ty| -> Ex {
            let a = arg(s, 0, to);
            if s.g.is_copy(to) || !s.g.same_out(to) {
                s.to_boundary(a, to).borrow()
            } else {
                s.g.lent(&a)
            }
        };
        match t0.present().clone() {
            Ty::Dict(kt, vt) => {
                let d = vals[0].ex.clone();
                let key = |s: &mut Self| -> Ex {
                    let a = arg(s, 0, &kt);
                    s.to_boundary(a, &kt)
                };
                match name {
                    "get" if t == Target::Rust => {
                        let kx = sought(self, &kt);
                        let mut raw = Ex::Call(Box::new(d.dot("get")), vec![kx]).method("cloned", vec![]);
                        if matches!(*vt, Ty::Opt(_)) {
                            raw = raw.method("flatten", vec![]);
                        }
                        let slot = match &*vt {
                            Ty::Opt(_) => (*vt).clone(),
                            v => Ty::Opt(Box::new(v.clone())),
                        };
                        self.val_of_raw(raw, &slot, &me)
                    }
                    "has" if t == Target::Rust => {
                        let kx = sought(self, &kt);
                        Val::new(Ex::Call(Box::new(d.dot("contains_key")), vec![kx]), me)
                    }
                    "remove" if t == Target::Rust => {
                        let kx = sought(self, &kt);
                        Val::new(g.kcall("Remove", vec![lent0, kx]), me)
                    }
                    "get" => {
                        let kx = key(self);
                        let raw = match t {
                            Target::TypeScript => g.kcall("get", vec![d, kx]),
                            Target::Python => Ex::Call(Box::new(d.dot("get")), vec![kx]),
                            Target::Go if matches!(*vt, Ty::Opt(_)) => g.kcall("GetOpt", vec![d, kx]),
                            Target::Go | Target::Rust => g.kcall("Get", vec![d, kx]),
                        };
                        let slot = match &*vt {
                            Ty::Opt(_) => (*vt).clone(),
                            v => Ty::Opt(Box::new(v.clone())),
                        };
                        self.val_of_raw(raw, &slot, &me)
                    }
                    "has" => {
                        let kx = key(self);
                        Val::new(self.has_key(&d, &kx), me)
                    }
                    "put" => {
                        let kx = key(self);
                        let kx = self.spill_as(kx, &g.boundary(&kt));
                        let v = arg(self, 1, &vt);
                        let vx = self.to_boundary(v, &vt);
                        let ex = match t {
                            Target::TypeScript => g.kcall("put", vec![d, kx, vx, g.order_fn(&kt, false)]),
                            Target::Python => g.kcall("put", vec![d, kx, vx, g.py_key_fn(&kt)]),
                            Target::Go => g.kcall("Put", vec![d, kx, vx]),
                            Target::Rust => g.kcall("Put", vec![lent0, kx, vx]),
                        };
                        Val::new(ex, me)
                    }
                    "remove" => {
                        let kx = key(self);
                        let ex = match t {
                            Target::TypeScript => g.kcall("remove", vec![d, kx, g.order_fn(&kt, false)]),
                            Target::Python => g.kcall("remove", vec![d, kx]),
                            Target::Go | Target::Rust => g.kcall("Remove", vec![d, kx]),
                        };
                        Val::new(ex, me)
                    }
                    "keys" => {
                        let raw = self.sorted_keys(if t == Target::Rust { &lent0 } else { &d }, &kt);
                        self.val_of_raw(raw, &Ty::List(kt.clone()), &me)
                    }
                    "values" => {
                        let raw = match t {
                            Target::TypeScript => g.kcall("valuesOf", vec![d, g.order_fn(&kt, false)]),
                            Target::Python => g.kcall("values_of", vec![d, g.py_key_fn(&kt)]),
                            Target::Go => g.kcall("ValuesOf", vec![d, g.order_fn(&kt, false)]),
                            Target::Rust => g.kcall("ValuesOf", vec![lent0]),
                        };
                        self.val_of_raw(raw, &Ty::List(vt.clone()), &me)
                    }
                    "entries" => {
                        let ex = match t {
                            Target::TypeScript => g.kcall("entriesOf", vec![d, g.order_fn(&kt, false)]),
                            Target::Python => g.kcall("entries_of", vec![d, g.py_key_fn(&kt)]),
                            Target::Go => g.kcall("EntriesOf", vec![d, g.order_fn(&kt, false)]),
                            Target::Rust => g.kcall("EntriesOf", vec![lent0]),
                        };
                        Val::new(ex, me)
                    }
                    "merge" => {
                        let o = arg(self, 0, &t0);
                        let ex = match t {
                            Target::TypeScript => g.kcall("merge", vec![d, o.ex, g.order_fn(&kt, false)]),
                            Target::Python => g.kcall("merge", vec![d, o.ex, g.py_key_fn(&kt)]),
                            Target::Go => g.kcall("Merge", vec![d, o.ex]),
                            Target::Rust => g.kcall("Merge", vec![lent0, g.lent(&o)]),
                        };
                        Val::new(ex, me)
                    }
                    _ => Val::new(self.size_of(&d), me),
                }
            }
            Ty::Set(et) => {
                let s0 = vals[0].ex.clone();
                let member = |s: &mut Self| -> Ex {
                    let a = arg(s, 0, &et);
                    s.to_boundary(a, &et)
                };
                let set_op = |f: &str| match t {
                    Target::TypeScript => f.to_string(),
                    _ => f.to_string(),
                };
                match name {
                    "has" if t == Target::Rust => {
                        let x = sought(self, &et);
                        Val::new(Ex::Call(Box::new(s0.dot("contains")), vec![x]), me)
                    }
                    "remove" if t == Target::Rust => {
                        let x = sought(self, &et);
                        Val::new(g.kcall("Without", vec![lent0, x]), me)
                    }
                    "has" => {
                        let x = member(self);
                        Val::new(self.has_key(&s0, &x), me)
                    }
                    "add" | "remove" => {
                        let x = member(self);
                        let ex = match (t, name) {
                            (Target::TypeScript, "add") => g.kcall("insert", vec![s0, x, g.order_fn(&et, false)]),
                            (Target::TypeScript, _) => g.kcall("without", vec![s0, x, g.order_fn(&et, false)]),
                            (Target::Python, "add") => Ex::atom(format!("({} | {{{}}})", s0.print(t), x.print(t))),
                            (Target::Python, _) => Ex::atom(format!("({} - {{{}}})", s0.print(t), x.print(t))),
                            (Target::Go, "add") => g.kcall("Insert", vec![s0, x]),
                            (Target::Go, _) => g.kcall("Remove", vec![s0, x]),
                            (Target::Rust, _) => g.kcall("Insert", vec![lent0, x]),
                        };
                        Val::new(ex, me)
                    }
                    "union" | "intersect" | "difference" => {
                        let o = arg(self, 0, &t0);
                        let ex = match t {
                            Target::TypeScript => g.kcall(&set_op(name), vec![s0, o.ex, g.order_fn(&et, false)]),
                            Target::Python => {
                                let op = match name {
                                    "union" => "|",
                                    "intersect" => "&",
                                    _ => "-",
                                };
                                Ex::atom(format!("({} {op} {})", s0.print(t), o.ex.print(t)))
                            }
                            Target::Go | Target::Rust => {
                                let f = match name {
                                    "union" => "Union",
                                    "intersect" => "Intersect",
                                    _ => "Difference",
                                };
                                if t == Target::Rust { g.kcall(f, vec![lent0, g.lent(&o)]) } else { g.kcall(f, vec![s0, o.ex]) }
                            }
                        };
                        Val::new(ex, me)
                    }
                    "to_list" => {
                        let raw = self.sorted_keys(if t == Target::Rust { &lent0 } else { &s0 }, &et);
                        self.val_of_raw(raw, &Ty::List(et.clone()), &me)
                    }
                    _ => Val::new(self.size_of(&s0), me),
                }
            }
            Ty::List(et) => {
                let xs0 = vals[0].ex.clone();
                let opt_elem = matches!(*et, Ty::Opt(_));
                match name {
                    // Rust: a copy of the element there, an element that may be none flattened
                    "first" | "last" if t == Target::Rust => {
                        let mut ex = Ex::Call(Box::new(xs0.dot(name)), vec![]).method("cloned", vec![]);
                        if opt_elem {
                            ex = ex.method("flatten", vec![]);
                        }
                        Val::new(ex, me)
                    }
                    "first" | "last" => {
                        let f = match (t, name, opt_elem) {
                            (Target::Go, "first", true) => "FirstOpt",
                            (Target::Go, "first", false) => "First",
                            (Target::Go, _, true) => "LastOpt",
                            (Target::Go, _, false) => "Last",
                            (_, "first", _) => "first",
                            _ => "last",
                        };
                        Val::new(g.kcall(f, vec![xs0]), me)
                    }
                    "at" => {
                        let i = self.int_arg(&vals[1]);
                        let f = match (t, opt_elem) {
                            (Target::Go | Target::Rust, true) => "ElemAtOpt",
                            (Target::Go | Target::Rust, false) => "ElemAt",
                            _ => "at",
                        };
                        let xs0 = if t == Target::Rust { lent0.clone() } else { xs0 };
                        Val::new(g.kcall(f, vec![xs0, i]), me)
                    }
                    "append" => {
                        let x = arg(self, 0, &et);
                        let ex = match t {
                            Target::Python => Ex::atom(format!("[*{}, {}]", xs0.print(t), x.ex.print(t))),
                            Target::TypeScript => g.kcall("append", vec![xs0, x.ex]),
                            Target::Go => g.kcall("Append", vec![xs0, x.ex]),
                            Target::Rust => g.kcall("Append", vec![lent0, g.owned(&x)]),
                        };
                        Val::new(ex, me)
                    }
                    "slice" | "take" | "drop" => {
                        let mut args = vec![if t == Target::Rust { lent0 } else { xs0 }];
                        for v in &vals[1..] {
                            args.push(self.int_arg(v));
                        }
                        let f = match (t, name) {
                            (Target::Python, "slice") => "slice_of",
                            (Target::Go | Target::Rust, "slice") => "Slice",
                            (Target::Go | Target::Rust, "take") => "Take",
                            (Target::Go | Target::Rust, _) => "Drop",
                            (_, n) => n,
                        };
                        Val::new(g.kcall(f, args), me)
                    }
                    "reverse" => {
                        let ex = match t {
                            Target::Python => Ex::Index(Box::new(xs0), Box::new(Ex::atom("::-1"))),
                            Target::TypeScript => g.kcall("reverse", vec![xs0]),
                            Target::Go => g.kcall("Reverse", vec![xs0]),
                            Target::Rust => g.kcall("Reverse", vec![lent0]),
                        };
                        Val::new(ex, me)
                    }
                    "sort" => {
                        let ex = match t {
                            Target::Python => match &*et {
                                Ty::Enum(_) | Ty::Param(_) => Ex::Call(Box::new(Ex::atom("sorted")), vec![xs0, Ex::atom(format!("key={}", g.py_key_fn(&et).print(t)))]),
                                _ => Ex::call("sorted", vec![xs0]),
                            },
                            Target::TypeScript => g.kcall("sorted", vec![xs0, g.order_fn(&et, true)]),
                            Target::Go => g.kcall("Sorted", vec![xs0, g.order_fn(&et, true)]),
                            // the elements' `Ord`
                            Target::Rust => g.kcall("Sorted", vec![lent0]),
                        };
                        Val::new(ex, me)
                    }
                    "distinct" => {
                        let q = matches!(&*et, Ty::Num(n) if rep(n) == Rep::Q);
                        let ex = match t {
                            Target::Python => g.kcall("distinct", vec![xs0]),
                            Target::TypeScript => g.kcall("distinct", vec![xs0, Ex::atom(format!("{k}.{}", if q { "qKey" } else { "same" }))]),
                            Target::Go => {
                                let key = if q { format!("{k}.QKey") } else { format!("{k}.Same[{}]", g.flight(&et)) };
                                g.kcall("Distinct", vec![xs0, Ex::atom(key)])
                            }
                            // the elements' `PartialEq` (a fraction is held in its lowest terms)
                            Target::Rust => g.kcall("Distinct", vec![lent0]),
                        };
                        Val::new(ex, me)
                    }
                    "contains" => {
                        let x = arg(self, 0, &et);
                        let x = self.spill(x);
                        let found = self.temp();
                        self.mutable(&found, &g.flight(&Ty::Bool), g.boolean(false));
                        let y = self.temp();
                        let test = match t {
                            // Rust: the element reached through the list, compared as the value sought is
                            Target::Rust => {
                                let yv = g.through(Ex::atom(y.clone()), &et);
                                g.eq_ex(g.cmp_side(&yv), g.cmp_side(&x), &et)
                            }
                            _ => g.eq_ex(Ex::atom(y.clone()), x.ex.clone(), &et),
                        };
                        let brk = Node::Line(if matches!(t, Target::TypeScript | Target::Rust) { "break;".into() } else { "break".into() });
                        let body = vec![Node::If(vec![(test, vec![self.assign_line(&found, g.boolean(true)), brk])], Vec::new())];
                        let head = match t {
                            Target::TypeScript => format!("for (const {y} of {})", xs0.print(t)),
                            Target::Python => format!("for {y} in {}", xs0.print(t)),
                            Target::Go => format!("for _, {y} := range {}", xs0.print(t)),
                            Target::Rust => format!("for {y} in {}", g.iter_rs(&xs0)),
                        };
                        self.emit(Node::Block(head, body, Vec::new()));
                        Val::new(Ex::atom(found), me)
                    }
                    "flatten" => Val::new(g.kcall(&g.helper("flatten", "flatten", "Flatten"), vec![if t == Target::Rust { lent0 } else { xs0 }]), me),
                    "indexed" => {
                        let out = self.temp();
                        self.new_list(&out, &g.flight(&me));
                        let (i, x) = (self.temp(), self.temp());
                        let copy = g.is_copy(&et);
                        let xv = match t {
                            Target::Rust => Val::held(Ex::atom(x.clone()), (*et).clone(), if copy { Held::Made } else { Held::Ref }),
                            _ => Val::new(Ex::atom(x.clone()), (*et).clone()),
                        };
                        let (item, mut body) = self.apart(|s| s.to_boundary(xv, &et));
                        let rec = match t {
                            Target::TypeScript => Ex::Record(String::new(), vec![("index".into(), Ex::call("BigInt", vec![Ex::atom(i.clone())])), ("item".into(), item)]),
                            Target::Python => Ex::Record(format!("{k}.Indexed"), vec![("index".into(), Ex::atom(i.clone())), ("item".into(), item)]),
                            Target::Go => Ex::Record(format!("{k}.Indexed[{}]", g.boundary(&et)), vec![("Index".into(), Ex::call("int64", vec![Ex::atom(i.clone())])), ("Item".into(), item)]),
                            Target::Rust => Ex::Record(format!("{k}::Indexed"), vec![("index".into(), Ex::Cast(Box::new(Ex::atom(i.clone())), "i64".into())), ("item".into(), item)]),
                        };
                        body.push(self.push(&out, rec));
                        let head = match t {
                            Target::TypeScript => {
                                body.insert(0, Node::Line(format!("const {x} = {}[{i}];", xs0.print(t))));
                                format!("for (let {i} = 0; {i} < {}.length; {i}++)", xs0.print(t))
                            }
                            Target::Python => format!("for {i}, {x} in enumerate({})", xs0.print(t)),
                            Target::Go => format!("for {i}, {x} := range {}", xs0.print(t)),
                            Target::Rust => format!("for ({i}, {}) in {}.iter().enumerate()", if copy { format!("&{x}") } else { x.clone() }, xs0.print(t)),
                        };
                        self.emit(Node::Block(head, body, Vec::new()));
                        Val::new(Ex::atom(out), me)
                    }
                    // Rust: the elements as they travel, copied into the set
                    "to_set" if t == Target::Rust => {
                        let lt = Ty::List(et.clone());
                        let lb = if g.same_out(&lt) { lent0 } else { self.to_boundary(vals[0].clone(), &lt).borrow() };
                        Val::new(g.kcall("SetOf", vec![lb]), me)
                    }
                    "to_set" => {
                        let lb = self.to_boundary(Val::new(xs0, t0.clone()), &Ty::List(et.clone()));
                        let ex = match t {
                            Target::TypeScript => g.kcall("setOf", vec![lb, g.order_fn(&et, false)]),
                            Target::Python => Ex::call("frozenset", vec![lb]),
                            Target::Go | Target::Rust => g.kcall("SetOf", vec![lb]),
                        };
                        Val::new(ex, me)
                    }
                    "join" => {
                        let sep = vals[1].ex.clone();
                        let ex = match t {
                            Target::TypeScript => Ex::Call(Box::new(xs0.dot("join")), vec![sep]),
                            Target::Python => Ex::Call(Box::new(sep.dot("join")), vec![xs0]),
                            Target::Go => g.kcall("Join", vec![xs0, sep]),
                            Target::Rust => Ex::Call(Box::new(xs0.dot("join")), vec![g.lent(&vals[1])]),
                        };
                        Val::new(ex, me)
                    }
                    _ => Val::new(self.size_of(&xs0), me),
                }
            }
            _ => Val::new(g.null(), me),
        }
    }

    /// The count of a list, a dict or a set, as a whole number of the target.
    fn size_of(&self, x: &Ex) -> Ex {
        match self.t() {
            Target::TypeScript => Ex::call("BigInt", vec![x.clone().dot("size")]),
            Target::Python => Ex::call("len", vec![x.clone()]),
            Target::Go => Ex::call("int64", vec![Ex::call("len", vec![x.clone()])]),
            Target::Rust => Ex::Cast(Box::new(Ex::Call(Box::new(x.clone().dot("len")), vec![])), "i64".into()),
        }
    }

    /// A whole number a builtin takes (`at`, `take`, `numbers`), as the target's integer.
    fn int_arg(&self, v: &Val) -> Ex {
        self.rep_to(v.ex.clone(), v.ty.num().map(rep).unwrap_or(Rep::Int), Rep::Int)
    }

    /// `a op b` of two values of `t` in the order of their type: numbers, dates, an ordered enum.
    fn order(&self, t: &Ty, a: Ex, b: Ex, op: Op) -> Ex {
        let g = self.g;
        match (t, self.t()) {
            // Rust: the derived `Ord` of each type, and `kaname::Q`'s by its value
            (_, Target::Rust) => Ex::bin(op, a, b),
            (Ty::Num(n), _) if rep(n) == Rep::Int => Ex::bin(op, a, b),
            (Ty::Num(_), Target::Python) => Ex::bin(op, a, b),
            (Ty::Num(_), Target::TypeScript) => Ex::bin(op, g.kcall("cmp", vec![a, b]), Ex::atom("0")),
            (Ty::Num(_), Target::Go) => Ex::bin(op, g.kcall("Cmp", vec![a, b]), Ex::atom("0")),
            (Ty::Date, Target::Go) => Ex::bin(op, g.kcall("Compare", vec![a, b]), Ex::atom("0")),
            // JavaScript's `<` compares UTF-16 units; Python's and Go's compare code points (DESIGN 25.7)
            (Ty::Str, Target::TypeScript) => Ex::bin(op, g.kcall("cmpStr", vec![a, b]), Ex::atom("0")),
            (Ty::Enum(id), _) => {
                let r = g.rank_of(id);
                Ex::bin(op, Ex::call(r.clone(), vec![a]), Ex::call(r, vec![b]))
            }
            // a type parameter held to keys: by the order the function is given (DESIGN 26.7)
            (Ty::Param(_), Target::Python) => Ex::bin(op, g.py_key(t, a), g.py_key(t, b)),
            (Ty::Param(_), _) => Ex::bin(op, g.order_ex(t, a, b, false), Ex::atom("0")),
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
            vec![s.push(&w2, s.g.owned(&v))]
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
            Target::Rust => format!("for &{w} in {weights}.iter()"),
        };
        self.emit(Node::Block(head, vec![self.assign_line(&total, next)], Vec::new()));
        let out = self.temp();
        let fty = g.flight(ty);
        self.new_list(&out, &fty);
        let before = self.temp();
        self.mutable(&before, &wf, zero);
        let i = self.temp();
        let upto = self.temp();
        let rust = self.t() == Target::Rust;
        // Rust: each element beside its weight (`xs.iter().zip(weights.iter())`), reached through
        // the list
        let item = match self.t() {
            Target::TypeScript | Target::Python => Ex::Index(Box::new(xs.ex.clone()), Box::new(Ex::atom(i.clone()))),
            Target::Go => Ex::Index(Box::new(xs.ex.clone()), Box::new(Ex::atom(i.clone()))),
            Target::Rust => Ex::atom(i.clone()),
        };
        // Rust: the weight beside the element; the other targets index the weights, and the
        // temporaries after it keep their numbers there
        let wv = if rust { self.temp() } else { String::new() };
        let wi = if rust { Ex::atom(wv.clone()) } else { Ex::Index(Box::new(Ex::atom(weights.clone())), Box::new(Ex::atom(i.clone()))) };
        let item_copy = g.is_copy(item_ty);
        let item_held = if rust && !item_copy { Held::Ref } else { Held::Made };
        let up = add(self, Ex::atom(before.clone()), wi);
        let mut body = Vec::new();
        let (bv, mut pre) = self.apart(|s| {
            let item_b = s.to_boundary(Val::held(item.clone(), (**item_ty).clone(), item_held), item_ty);
            let bb = s.to_boundary(Val::new(Ex::atom(before.clone()), w_ty.clone()), &w_ty);
            let ub = s.to_boundary(Val::new(Ex::atom(upto.clone()), w_ty.clone()), &w_ty);
            let tb = s.to_boundary(Val::new(Ex::atom(total.clone()), w_ty.clone()), &w_ty);
            let (ki, kb, ku, kt) = match s.t() {
                Target::Go => ("Item", "Before", "Upto", "Total"),
                _ => ("item", "before", "upto", "total"),
            };
            let tyname = match s.t() {
                Target::TypeScript => String::new(),
                // Rust: the struct's literal names it without its type arguments
                Target::Rust => format!("{}::Running", s.g.imp(Import::Kaname)),
                _ => s.g.record_type(&RecId::Running(item_ty.clone(), Box::new(w_ty.clone()))),
            };
            Ex::Record(tyname, vec![(ki.into(), item_b), (kb.into(), bb), (ku.into(), ub), (kt.into(), tb)])
        });
        match self.t() {
            Target::TypeScript => body.push(Node::Line(format!("const {upto}: {wf} = {};", up.print(self.t())))),
            Target::Python => body.push(Node::Line(format!("{upto}: {wf} = {}", up.print(self.t())))),
            Target::Go => body.push(Node::Line(format!("var {upto} {wf} = {}", up.print(self.t())))),
            Target::Rust => body.push(Node::Line(format!("let {upto}: {wf} = {};", up.print(self.t())))),
        }
        body.append(&mut pre);
        body.push(self.push(&out, bv));
        body.push(self.assign_line(&before, Ex::atom(upto.clone())));
        let head = match self.t() {
            Target::TypeScript => format!("for (let {i} = 0; {i} < {}.length; {i}++)", xs.ex.print(self.t())),
            Target::Python => format!("for {i} in range(len({}))", xs.ex.print(self.t())),
            Target::Go => format!("for {i} := range {}", xs.ex.print(self.t())),
            Target::Rust => format!("for ({}, &{wv}) in {}.iter().zip({weights}.iter())", if item_copy { format!("&{i}") } else { i.clone() }, xs.ex.print(self.t())),
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
        let start = self.g.owned(&vs[1]);
        self.mutable(&acc, &at, start);
        let a2 = acc.clone();
        let rust = self.t() == Target::Rust;
        let held = if rust && !self.g.is_copy(&first_ty) { Held::Kept } else { Held::Made };
        let accv = Val::held(Ex::atom(acc.clone()), first_ty.clone(), held);
        let node = self.each(xs, lam, vec![accv], |s, v| {
            let v = s.adapt(v, &first_ty, lam);
            vec![s.assign_line(&a2, s.g.owned(&v))]
        });
        self.emit(node);
        // the running value read for the last time: Rust's code takes it as it is
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
            Target::Rust => format!("let mut {name}: {ty} = {};", value.print(t)),
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
            Target::Rust => format!("let mut {name}: {ty} = Vec::new();"),
        };
        self.emit(Node::Line(line));
    }

    fn push(&self, list: &str, v: Ex) -> Node {
        let t = self.t();
        Node::Line(match t {
            Target::TypeScript => format!("{list}.push({});", v.print(t)),
            Target::Python => format!("{list}.append({})", v.print(t)),
            Target::Go => format!("{list} = {}", Ex::call("append", vec![Ex::atom(list), v]).print(t)),
            Target::Rust => format!("{list}.push({});", v.print(t)),
        })
    }

    /// A builtin that takes no function (`interp.rs`'s `scalar`).
    fn scalar(&mut self, e: &Expr, name: &str, xs: &[&Expr], vals: Vec<Val>) -> Val {
        let me = self.ty(e);
        let g = self.g;
        let t = self.t();
        match name {
            // a result made (DESIGN 26.3): its value as it travels in it, of the type the place gives
            "ok" | "err" => {
                let Ty::Result(a, er) = &me else { return Val::new(g.null(), me) };
                let inner = if name == "ok" { (**a).clone() } else { (**er).clone() };
                let v = self.adapt(vals[0].clone(), &inner, xs[0]);
                let b = self.to_boundary(v, &inner);
                Val::new(g.result_value(name == "ok", b, a, er), me)
            }
            "min" | "max" => {
                let a = self.adapt(vals[0].clone(), &me, xs[0]);
                let b = self.adapt(vals[1].clone(), &me, xs[1]);
                let (a, b) = (self.spill(a), self.spill(b));
                let op = if name == "min" { Op::Le } else { Op::Ge };
                // Rust: the two compared as Rust compares them, the one taken copied where it is not made
                if t == Target::Rust {
                    let test = self.order(me.present(), g.cmp_side(&a), g.cmp_side(&b), op);
                    return Val::new(Ex::Cond(Box::new(test), Box::new(g.owned(&a)), Box::new(g.owned(&b))), me);
                }
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
                    // a divisor that is a value (DESIGN 24.3): Euclid's remainder, the error `zero`
                    // where the analysis does not show the divisor is never 0
                    // Rust: the form that cannot fail, else the one that fails with `zero`
                    _ if t == Target::Rust => {
                        let n = self.int_arg(&vals[1]);
                        match g.zero_at(e) {
                            None => g.kcall("ModOf", vec![x, n]),
                            Some(at) => {
                                let mut args = vec![x, n];
                                args.extend(g.at_args(&at));
                                g.kcall("ModOfAt", args).tried()
                            }
                        }
                    }
                    _ => {
                        let n = self.int_arg(&vals[1]);
                        let zero = match (g.zero_at(e), t) {
                            (None, Target::Go) => Ex::atom("nil"),
                            (None, _) => g.null(),
                            (Some(at), Target::Go) => Ex::atom(format!("&{}", Ex::Record(format!("{}.At", g.imp(Import::Kaname)), vec![("Module".into(), g.module_const()), ("Line".into(), Ex::atom(at.line.to_string())), ("Message".into(), g.string(&at.message))]).print(t))),
                            (Some(at), _) => Ex::List(String::new(), g.at_args(&at)),
                        };
                        g.kcall(&g.helper("modOf", "mod_of", "ModOf"), vec![x, n, zero])
                    }
                };
                Val::new(self.rep_to(ex, Rep::Int, me.num().map(rep).unwrap_or(Rep::Int)), me)
            }
            // `numbers(a, b)`: the count held to int64 where the analysis does not show it is (DESIGN 24.3)
            "numbers" if t == Target::Rust => {
                let (a, b) = (self.int_arg(&vals[0]), self.int_arg(&vals[1]));
                let ex = match g.overflow_at(e) {
                    None => g.kcall("Numbers", vec![a, b]),
                    Some(at) => {
                        let mut args = vec![a, b];
                        args.extend(g.at_args(&at));
                        g.kcall("NumbersAt", args).tried()
                    }
                };
                Val::new(ex, me)
            }
            "numbers" => {
                let (a, b) = (self.int_arg(&vals[0]), self.int_arg(&vals[1]));
                let over = match (g.overflow_at(e), t) {
                    (None, Target::Go) => Ex::atom("nil"),
                    (None, _) => g.null(),
                    (Some(at), Target::Go) => Ex::atom(format!("&{}", Ex::Record(format!("{}.At", g.imp(Import::Kaname)), vec![("Module".into(), g.module_const()), ("Line".into(), Ex::atom(at.line.to_string())), ("Message".into(), g.string(&at.message))]).print(t))),
                    (Some(at), _) => Ex::List(String::new(), g.at_args(&at)),
                };
                Val::new(g.kcall(&g.helper("numbers", "numbers", "Numbers"), vec![a, b, over]), me)
            }
            "isqrt" => {
                let x = self.rep_to(vals[0].ex.clone(), vals[0].ty.num().map(rep).unwrap_or(Rep::Int), Rep::Q);
                let checked = g.open(xs[0].id, |w| matches!(w, What::Isqrt));
                let r = me.num().map(rep).unwrap_or(Rep::Int);
                if !checked {
                    let ex = match t {
                        Target::TypeScript => Ex::atom(format!("{}!", g.kcall("isqrt", vec![x]).print(t))),
                        Target::Python => g.kcall("root", vec![x]),
                        Target::Go | Target::Rust => g.kcall("Root", vec![x]),
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
                // Rust: the message made where it fails, and the error returned
                if t == Target::Rust {
                    let parts = template(&tr!("`isqrt` に負の値 \u{1}shown\u{1} を渡しています", "`isqrt` is given \u{1}shown\u{1}, which is below 0"), g.lang)
                        .into_iter()
                        .map(|p| match p {
                            Ok(s) => Ok(s),
                            Err(_) => Err(shown.clone()),
                        })
                        .collect();
                    let fail = g.fail_line("range", e.span.line, g.rs_format(parts));
                    self.emit(Node::Block(format!("let Some({n}) = {} else", g.kcall("Isqrt", vec![xv.ex.clone()]).print(t)), vec![fail], Vec::new()));
                    return Val::new(self.rep_to(Ex::atom(n), Rep::Int, r), me);
                }
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
                    Target::Rust => unreachable!("Rust's `isqrt` is written above"),
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
                        (Some(at), Target::Rust) => Ex::call(
                            "Some",
                            vec![Ex::Record(format!("{}::At", g.imp(Import::Kaname)), vec![("module".into(), g.module_const()), ("line".into(), Ex::atom(at.line.to_string())), ("message".into(), g.string(&at.message))])],
                        ),
                        (Some(at), _) => Ex::List(String::new(), g.at_args(&at)),
                    }
                };
                let (oa, za) = (g.overflow_at(e), g.zero_at(e));
                let v = match t {
                    // Rust: the form that cannot fail, else the one that fails with `overflow` or `zero`
                    Target::Rust if oa.is_none() && za.is_none() => g.kcall("Allocate", vec![tq, c, s]),
                    Target::Rust => g.kcall("AllocateAt", vec![tq, c, s, over(oa), over(za)]).tried(),
                    _ => g.kcall(&g.helper("allocate", "allocate", "Allocate"), vec![tq, c, s, over(oa), over(za)]),
                };
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
                    v = g.held_call(args);
                }
                let nm = me.num().cloned().unwrap_or_else(NumTy::number);
                let v = self.convert(v, &NumTy { step: None, konst: None, ..nx }, &NumTy { step: None, ..nm.clone() }, e, None);
                Val::new(self.rep_to(v, Rep::Q, rep(&nm)), me)
            }
            "count" => {
                let n = match t {
                    // a string's code points (DESIGN 25.7)
                    Target::TypeScript | Target::Go if matches!(vals[0].ty.present(), Ty::Str) => g.kcall(&g.helper("cpCount", "", "CpCount"), vec![vals[0].ex.clone()]),
                    Target::Rust if matches!(vals[0].ty.present(), Ty::Str) => g.kcall("CpCount", vec![g.lent(&vals[0])]),
                    Target::Rust => self.size_of(&vals[0].ex),
                    // a dict's pairs and a set's members (DESIGN 24.3)
                    Target::TypeScript if matches!(vals[0].ty.present(), Ty::Dict(..) | Ty::Set(_)) => self.size_of(&vals[0].ex),
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
                    Target::Rust => Ex::Call(Box::new(vals[0].ex.clone().dot("starts_with")), vec![g.lent(&vals[1])]),
                };
                Val::new(ex, me)
            }
            // lists one after another, each read in the type of the answer (DESIGN 24.3)
            "concat" if matches!(me, Ty::List(_) | Ty::Empty) => {
                // Rust: each list as a slice of one array, joined (`[a.as_slice(), b].concat()`)
                if t == Target::Rust {
                    let parts: Vec<Val> = vals.iter().zip(xs).map(|(v, x)| self.adapt(v.clone(), &me, x)).collect();
                    let slices: Vec<String> = parts.iter().map(|v| if v.held == Held::Lent { v.ex.print(t) } else { v.ex.clone().method("as_slice", vec![]).print(t) }).collect();
                    return Val::new(Ex::atom(format!("[{}].concat()", slices.join(", "))), me);
                }
                let lists: Vec<Ex> = vals.iter().zip(xs).map(|(v, x)| self.adapt(v.clone(), &me, x).ex).collect();
                let ex = match t {
                    Target::Python => Ex::atom(format!("[{}]", lists.iter().map(|l| format!("*{}", l.print(t))).collect::<Vec<_>>().join(", "))),
                    Target::TypeScript => g.kcall("concat", lists),
                    Target::Go => g.kcall("Concat", lists),
                    Target::Rust => unreachable!("Rust joins its lists above"),
                };
                Val::new(ex, me)
            }
            // Rust: the strings lent to one array, joined
            "concat" if t == Target::Rust => {
                let parts: Vec<String> = vals.iter().map(|v| g.lent(v).print(t)).collect();
                Val::new(Ex::atom(format!("[{}].concat()", parts.join(", "))), me)
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
                        Target::Rust => v.ex.clone().method("to_string", vec![]),
                    },
                    Ty::Bool => match t {
                        Target::TypeScript => Ex::call("String", vec![v.ex.clone()]),
                        Target::Python => Ex::Cond(Box::new(v.ex.clone()), Box::new(g.string("true")), Box::new(g.string("false"))),
                        Target::Go => g.kcall("BoolText", vec![v.ex.clone()]),
                        Target::Rust => v.ex.clone().method("to_string", vec![]),
                    },
                    Ty::Enum(id) => match (t, &id.owner) {
                        (Target::TypeScript, _) => v.ex.clone(),
                        (Target::Python, _) => v.ex.clone().dot("value"),
                        (Target::Go, Owner::Module(_)) => Ex::call("string", vec![v.ex.clone()]),
                        (Target::Go, Owner::Rule(_)) => Ex::Call(Box::new(v.ex.clone().dot("String")), vec![]),
                        // the value's name, as both kaname's and rulec's enums give it
                        (Target::Rust, _) => v.ex.clone().method("as_str", vec![]).method("to_string", vec![]),
                    },
                    // Rust: a string as it is held
                    _ if t == Target::Rust => return Val { ty: me, ..v.clone() },
                    _ => v.ex.clone(),
                };
                Val::new(ex, me)
            }
            // `char(n)`: the string of one code point, a constant (DESIGN 25.2)
            "char" => {
                let c = self.ty(xs[0]).num().and_then(|n| n.konst).and_then(|k| u32::try_from(k.num).ok()).and_then(char::from_u32).unwrap_or('?');
                Val::held(g.string(&c.to_string()), me, if t == Target::Rust { Held::Lent } else { Held::Made })
            }
            _ if matches!(vals[0].ty.present(), Ty::Str) || name == "to_text" => self.string(name, vals, me),
            _ => self.collection(e, name, xs, vals),
        }
    }

    /// The builtins of a string (DESIGN 25.7): the helper module's, which count in code points, where
    /// the target's own count in UTF-16 units or bytes; Python's `str` is a sequence of code points
    /// already, and its length, cuts and places are used as they are.
    fn string(&mut self, name: &str, vals: Vec<Val>, me: Ty) -> Val {
        let g = self.g;
        let t = self.t();
        // Rust: the string and the strings it is given lent (`&str`)
        let s0 = if t == Target::Rust { g.lent(&vals[0]) } else { vals[0].ex.clone() };
        let arg = |i: usize| -> Ex { if t == Target::Rust { g.lent(&vals[i]) } else { vals[i].ex.clone() } };
        let int = |s: &Self, i: usize| -> Ex { s.int_arg(&vals[i]) };
        let py_method = |m: &str, args: Vec<Ex>| -> Ex { Ex::Call(Box::new(s0.clone().dot(m)), args) };
        let ex = match (name, t) {
            ("chars", Target::Python) => Ex::call("list", vec![s0]),
            ("chars", _) => g.kcall(&g.helper("chars", "", "Chars"), vec![s0]),
            ("code_points", Target::Python) => Ex::call("list", vec![Ex::call("map", vec![Ex::atom("ord"), s0])]),
            ("code_points", _) => g.kcall(&g.helper("codePoints", "", "CodePoints"), vec![s0]),
            ("to_text", _) => g.kcall(&g.helper("toText", "to_text", "ToText"), vec![s0]),
            ("slice", _) => g.kcall(&g.helper("cpSlice", "text_slice", "TextSlice"), vec![s0, int(self, 1), int(self, 2)]),
            ("take", _) => g.kcall(&g.helper("cpTake", "text_take", "TextTake"), vec![s0, int(self, 1)]),
            ("drop", _) => g.kcall(&g.helper("cpDrop", "text_drop", "TextDrop"), vec![s0, int(self, 1)]),
            ("index_of", _) => g.kcall(&g.helper("cpIndexOf", "index_of", "IndexOf"), vec![s0, arg(1)]),
            ("contains", Target::TypeScript) => Ex::Call(Box::new(s0.dot("includes")), vec![arg(1)]),
            ("contains", Target::Python) => Ex::atom(format!("({} in {})", arg(1).print(t), s0.print(t))),
            ("contains", Target::Go) => g.kcall("TextContains", vec![s0, arg(1)]),
            ("contains", Target::Rust) => Ex::Call(Box::new(vals[0].ex.clone().dot("contains")), vec![arg(1)]),
            ("ends_with", Target::TypeScript) => Ex::Call(Box::new(s0.dot("endsWith")), vec![arg(1)]),
            ("ends_with", Target::Python) => py_method("endswith", vec![arg(1)]),
            ("ends_with", Target::Go) => g.kcall("EndsWith", vec![s0, arg(1)]),
            ("ends_with", Target::Rust) => Ex::Call(Box::new(vals[0].ex.clone().dot("ends_with")), vec![arg(1)]),
            ("split", _) => g.kcall(&g.helper("split", "split", "Split"), vec![s0, arg(1)]),
            ("words", _) => g.kcall(&g.helper("words", "words", "Words"), vec![s0]),
            ("replace", _) => g.kcall(&g.helper("replace", "replace", "Replace"), vec![s0, arg(1), arg(2)]),
            ("trim", _) => g.kcall(&g.helper("trim", "trim", "Trim"), vec![s0]),
            ("trim_start", _) => g.kcall(&g.helper("trimStart", "trim_start", "TrimStart"), vec![s0]),
            ("trim_end", _) => g.kcall(&g.helper("trimEnd", "trim_end", "TrimEnd"), vec![s0]),
            ("lower_ascii", _) => g.kcall(&g.helper("lowerAscii", "lower_ascii", "LowerASCII"), vec![s0]),
            ("upper_ascii", _) => g.kcall(&g.helper("upperAscii", "upper_ascii", "UpperASCII"), vec![s0]),
            ("pad_start" | "pad_end", _) => {
                let at_end = g.boolean(name == "pad_end");
                g.kcall(&g.helper("pad", "pad", "Pad"), vec![s0, int(self, 1), arg(2), at_end])
            }
            ("parse_number", _) => g.kcall(&g.helper("parseNumber", "parse_number", "ParseNumber"), vec![s0]),
            ("parse_date", _) => g.kcall(&g.helper("parseDate", "parse_date", "ParseText"), vec![s0]),
            _ => g.null(),
        };
        Val::new(ex, me)
    }

    // ── The function itself ────────────────────────────────────────────────

    /// The body of the function, its parameters (as they travel) named by `params`.
    pub fn body(self, params: &[String]) -> Vec<Node> {
        self.body_with(params, &[])
    }

    /// The same, the names in `hidden` taken too: the orders a generic function is given (DESIGN 26.7).
    pub fn body_with(mut self, params: &[String], hidden: &[String]) -> Vec<Node> {
        for h in hidden {
            self.locals.taken.insert(h.clone());
        }
        let g = self.g;
        let info = g.c.decls.fns[self.f].clone();
        let mut cells = Vec::new();
        for (p, name) in info.params.iter().zip(params) {
            self.locals.taken.insert(name.clone());
            let held = if self.t() == Target::Rust && !g.is_copy(&p.slot.ty) { Held::Lent } else { Held::Made };
            let v = self.from_boundary_as(Ex::atom(name.clone()), &p.slot.ty, &p.slot.ty, held);
            let start = self.out.len();
            let v = if v.ex.is_simple() { v } else { self.spill(v) };
            // Rust names a parameter the body never reads `_` (DESIGN 36.4)
            let used = Rc::new(Cell::new(self.t() != Target::Rust));
            cells.push((name.clone(), used.clone(), start..self.out.len()));
            self.scope.push((p.name.text.clone(), v, used));
        }
        let decl = &g.c.module.fns[self.f];
        self.block(&decl.body, &Sink::Return);
        if self.t() == Target::Rust {
            // a parameter never read: nor is it read in its type first
            for (_, used, at) in cells.iter().rev() {
                if !used.get() {
                    self.out.drain(at.clone());
                }
            }
            *g.unread.borrow_mut() = cells.iter().filter(|(_, c, _)| !c.get()).map(|(n, ..)| n.clone()).collect();
        }
        self.out
    }
}

/// The type parameters a type holds, each once, in the order they come.
fn params_in(t: &Ty, out: &mut Vec<String>) {
    match t {
        Ty::Param(n) => {
            if !out.contains(n) {
                out.push(n.clone());
            }
        }
        Ty::List(x) | Ty::Set(x) | Ty::Opt(x) => params_in(x, out),
        Ty::Dict(a, b) | Ty::Result(a, b) => {
            params_in(a, out);
            params_in(b, out);
        }
        Ty::Enum(id) => id.args.iter().for_each(|a| params_in(a, out)),
        Ty::Record(r) => match r {
            RecId::Module { args, .. } | RecId::Variant { args, .. } => args.iter().for_each(|a| params_in(a, out)),
            RecId::Running(a, b) | RecId::Entry(a, b) => {
                params_in(a, out);
                params_in(b, out);
            }
            RecId::Indexed(x) => params_in(x, out),
            RecId::Outputs(_) | RecId::Element(..) => {}
        },
        _ => {}
    }
}

/// Rust: whether an expression is the string `""` (compared by `is_empty`).
fn empty_str(x: &Ex) -> bool {
    matches!(x, Ex::Atom(s) if s == "\"\"")
}

/// Rust: whether an expression is a list with nothing in it (`vec![]`, `Vec::new()`).
fn empty_list(x: &Ex) -> bool {
    match x {
        Ex::List(_, xs) => xs.is_empty(),
        Ex::Atom(s) => s == "vec![]" || s == "Vec::new()",
        _ => false,
    }
}

/// A type with no constant in it: the type of the keys a function given to a builtin answers.
fn plain_num(t: &Ty) -> Ty {
    match t {
        Ty::Num(n) => Ty::Num(NumTy { konst: None, ..n.clone() }),
        x => x.clone(),
    }
}

/// The types of a dict's keys and of its values.
fn dict_tys(t: &Ty) -> (Ty, Ty) {
    match t.present() {
        Ty::Dict(k, v) => ((**k).clone(), (**v).clone()),
        _ => (Ty::Error, Ty::Error),
    }
}

/// Go: a composite literal (a date written out) as the operand of `==` and `!=`, in parentheses, which
/// the condition of an `if` needs (`d == (kaname.Date{…})`).
fn go_bare(x: Ex, t: Target) -> Ex {
    match (&x, t) {
        (Ex::Record(..), Target::Go) => Ex::atom(format!("({})", x.print(t))),
        _ => x,
    }
}

/// A literal as written, or one below 0: a number whose value the code is written with.
fn literal(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Num(_) => true,
        ExprKind::Neg(x) => literal(x),
        _ => false,
    }
}

/// The shift between the zeros of two units a value is read across: 32 from ℃ to ℉, −160/9 from ℉
/// to ℃, and 0 for every other pair (a temperature alone has a zero of its own).
fn zero_shift(a: &NumTy, b: &NumTy) -> Rat {
    if a.unit.compares_only() && b.unit.compares_only() && a.unit.offset() != b.unit.offset() {
        crate::ty::convert(Rat::zero(), &a.unit, &b.unit).unwrap_or(Rat::zero())
    } else {
        Rat::zero()
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
