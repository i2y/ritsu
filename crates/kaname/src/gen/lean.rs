//! A module as Lean (DESIGN 38.4): its records and enums as Lean's types, its total functions as
//! Lean's definitions, and its properties as the statements of theorems, which a person or an LLM
//! proves in a file beside the module (38.5) and `--lean` holds to them (38.6).
//!
//! What is not written for Lean yet, and why a property that uses it is W604: the builtins of dicts,
//! sets and strings (a string is written as a value, compared with `=` and `!=`), the builtins that
//! use the order of keys, and a generic function with a type parameter held to keys.

use crate::ast::*;
use crate::check::Checked;
use crate::names::Target;
use crate::ty::*;
use crate::types::Ref;
use crate::value::{Program, Value};
use ritsu_base::text::{Lang, Text};
use ritsu_emit::header::{Comment, Origin};
use ritsu_units::Rat;
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::rc::Rc;

/// The version of Lean the package is written for: the one of `proofs/` (a test holds the two the
/// same).
pub const TOOLCHAIN: &str = "leanprover/lean4:v4.34.0";

/// What keeps a function or a property from Lean (W604): what it is, and where.
#[derive(Clone, Debug, PartialEq)]
pub struct Obstacle {
    /// What is used: "`keys`, a builtin of dicts".
    pub what: Text,
    /// The file it is written in, as the check was given it, and the line.
    pub file: String,
    pub line: usize,
}

impl Obstacle {
    /// Where it is, as a message says it from the file `from`: `line 43`, or `line 3 of lib.kn` for
    /// another file.
    pub fn place(&self, from: &str) -> Text {
        let line = self.line;
        if self.file == from {
            tr!("{line} 行目", "line {line}")
        } else {
            let f = std::path::Path::new(&self.file).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| self.file.clone());
            tr!("{f} の {line} 行目", "line {line} of {f}")
        }
    }
}

/// The builtins that use the order of keys (DESIGN 24.1): written with that order, after it.
const KEY_ORDER: [&str; 9] = ["sort", "sort_by", "sort_by_desc", "distinct", "max_by", "min_by", "to_dict", "group_by", "to_set"];

/// The builtins of dicts and sets.
const OF_DICTS: [&str; 15] = ["get", "put", "remove", "has", "keys", "values", "entries", "map_values", "merge", "merge_with", "add", "union", "intersect", "difference", "to_list"];

/// The builtins of strings, besides those a list has too (`count`, `concat`, `slice`, `take`, `drop`,
/// `contains`), which are of strings where the value is a string.
const OF_STRINGS: [&str; 21] = [
    "starts_with",
    "text",
    "char",
    "join",
    "chars",
    "code_points",
    "to_text",
    "index_of",
    "ends_with",
    "split",
    "words",
    "replace",
    "trim",
    "trim_start",
    "trim_end",
    "lower_ascii",
    "upper_ascii",
    "pad_start",
    "pad_end",
    "parse_number",
    "parse_date",
];

/// Whether a type holds a dict, a set or a dict's entry anywhere.
fn holds_keyed(t: &Ty) -> Option<Text> {
    match t {
        Ty::Dict(..) | Ty::EmptyDict => Some(tr!("辞書", "a dict")),
        Ty::Set(_) => Some(tr!("集合", "a set")),
        Ty::Opt(x) | Ty::List(x) => holds_keyed(x),
        Ty::Result(a, b) => holds_keyed(a).or_else(|| holds_keyed(b)),
        Ty::Fn(ps, r, _) => ps.iter().find_map(holds_keyed).or_else(|| holds_keyed(r)),
        // a dict's entry (what `entries` answers), which no declaration of Lean is written for, as
        // the writer's `keyed` says
        Ty::Record(RecId::Entry(..)) => Some(tr!("辞書の組", "an entry of a dict")),
        Ty::Record(RecId::Running(a, b)) => holds_keyed(a).or_else(|| holds_keyed(b)),
        Ty::Record(RecId::Indexed(a)) => holds_keyed(a),
        _ => None,
    }
}

/// What in the function `f` of `c` itself keeps it from Lean (not what the functions it calls use):
/// the first, in the order written.
pub fn fn_obstacle(c: &Checked, f: usize) -> Option<Obstacle> {
    let info = &c.decls.fns[f];
    let decl = &c.module.fns[f];
    let at = |what: Text, line: usize| Some(Obstacle { what, file: c.path.clone(), line });
    if let Some(p) = info.tparams.iter().find(|p| p.key) {
        let n = &p.name.text;
        return at(tr!("鍵に限った型の引数 `{n}: key`", "the type parameter held to keys `{n}: key`"), decl.line);
    }
    for (prm, ast) in info.params.iter().zip(&decl.params) {
        if let Some(what) = holds_keyed(&prm.slot.ty) {
            return at(what, ast.ty.span.line);
        }
    }
    if let Some(what) = holds_keyed(&info.ret.ty) {
        return at(what, decl.line);
    }
    let mut es = Vec::new();
    decl.body.walk(&mut es);
    for e in es {
        let ty = c.typed.types.get(e.id).unwrap_or(&Ty::Error);
        if let Some(what) = holds_keyed(ty) {
            return at(what, e.span.line);
        }
        match &e.kind {
            ExprKind::Bin(a, op, _) if matches!(op, BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge) && matches!(c.typed.types.get(a.id).map(Ty::present), Some(Ty::Str)) => {
                return at(tr!("文字列の順序", "the order of strings"), e.span.line);
            }
            ExprKind::Call(callee, args) => {
                let name = match c.typed.refs.get(callee.id) {
                    Some(Ref::Builtin(n) | Ref::Method(n)) => *n,
                    _ => continue,
                };
                // the value a builtin is about: its first argument, or the value before the `.`
                let first = match (&callee.kind, c.typed.refs.get(callee.id)) {
                    (ExprKind::Field(base, _, _), Some(Ref::Method(_))) => Some(&**base),
                    _ => args.first().map(Arg::expr),
                };
                let of_str = first.and_then(|x| c.typed.types.get(x.id)).is_some_and(|t| matches!(t.present(), Ty::Str));
                if KEY_ORDER.contains(&name) {
                    return at(tr!("鍵の順序を使う組み込みの関数 `{name}`", "`{name}`, a builtin that uses the order of keys"), e.span.line);
                }
                if OF_DICTS.contains(&name) {
                    return at(tr!("辞書と集合の組み込みの関数 `{name}`", "`{name}`, a builtin of dicts and sets"), e.span.line);
                }
                let str_too = matches!(name, "count" | "concat" | "slice" | "take" | "drop" | "contains" | "min" | "max");
                if OF_STRINGS.contains(&name) || (str_too && (of_str || matches!(ty.present(), Ty::Str))) {
                    return at(tr!("文字列の組み込みの関数 `{name}`", "`{name}`, a builtin of strings"), e.span.line);
                }
            }
            _ => {}
        }
    }
    None
}

/// The functions the function `f` of `m` reaches through its calls, `f` first, each once: its own
/// module's and the modules' it reads, by their modules in `p`.
pub fn reached<'p>(p: &'p Program, m: &Checked, f: usize) -> Vec<(&'p Rc<Checked>, usize)> {
    let mut out: Vec<(&'p Rc<Checked>, usize)> = vec![(p.module(&m.canon), f)];
    let mut seen: BTreeSet<(std::path::PathBuf, usize)> = BTreeSet::from([(m.canon.clone(), f)]);
    let mut i = 0;
    while i < out.len() {
        let (c, g) = (out[i].0, out[i].1);
        i += 1;
        let mut es = Vec::new();
        c.module.fns[g].body.walk(&mut es);
        for e in es {
            let ExprKind::Call(callee, _) = &e.kind else { continue };
            let Some(Ref::Fn(file, name)) = c.typed.refs.get(callee.id) else { continue };
            let callee_c = p.module(file);
            let Some(h) = crate::interp::function(callee_c, name) else { continue };
            if seen.insert((callee_c.canon.clone(), h)) {
                out.push((callee_c, h));
            }
        }
    }
    out
}

/// The first thing the property (or function) `f` of `m` uses, itself or through what it calls, that
/// is not written for Lean (W604).
pub fn obstacle(p: &Program, m: &Checked, f: usize) -> Option<Obstacle> {
    reached(p, m, f).into_iter().find_map(|(c, g)| fn_obstacle(c, g))
}

// ── The writer ──────────────────────────────────────────────────────────────

// ── Writing a package ───────────────────────────────────────────────────────

/// Where the package is written (PLAN 12.2.6), which says how its lakefile reaches the proofs.
#[derive(Clone, Debug)]
pub enum Layout {
    /// In `out` (`generated/lean/`), each proof file read where it is, beside its `.kn`: a library
    /// for each (`proofs_<alias>`, its `srcDir` the path from `out` to the proof's directory). The
    /// proofs are the modules' aliases and their files.
    InPlace { out: PathBuf, proofs: Vec<(String, PathBuf)> },
    /// In a directory of its own, the proofs copied into `Proofs/` (what `--lean` builds).
    Temp,
}

/// How the numbers of the definitions written are held (DESIGN 38.4, 38.9): the expressions written
/// as a whole number of a step other than 1, and those written as `Kaname.Frac`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LeanStats {
    pub scaled: usize,
    pub fractions: usize,
}

impl std::ops::AddAssign for LeanStats {
    fn add_assign(&mut self, o: LeanStats) {
        self.scaled += o.scaled;
        self.fractions += o.fractions;
    }
}

/// The prelude every module's Lean imports.
pub const PRELUDE: &str = include_str!("runtime/lean/Prelude.lean");

/// The words Lean keeps: a name of a module that is one is written in `«»`.
const LEAN_KEYWORDS: [&str; 88] = [
    "abbrev", "at", "attribute", "axiom", "break", "builtin_initialize", "by", "calc", "catch", "class", "continue", "decreasing_by", "def", "deriving", "do",
    "elab", "else", "end", "example", "exists", "export", "extends", "finally", "for", "forall", "from", "fun", "generalizing", "have", "hiding", "if",
    "import", "in", "include", "inductive", "infix", "infixl", "infixr", "initialize", "instance", "let", "local", "macro", "macro_rules", "match", "mut",
    "mutual", "namespace", "noncomputable", "nomatch", "nofun", "notation", "obtain", "omit", "open", "opaque", "partial", "postfix", "prefix", "private",
    "protected", "public", "renaming", "repeat", "return", "rfl", "run_cmd", "run_elab", "run_meta", "scoped", "section", "set_option", "show", "sorry",
    "structure", "suffices", "syntax", "termination_by", "then", "theorem", "try", "universe", "unless", "unsafe", "until", "variable", "where", "with",
];

/// The names the Lean of a module names from the root: one the module's own names hide (a type or a
/// function of the module, or a name a body binds) is written `_root_.<name>` ([`Mw::std`]). `Kaname`
/// is among them: a `let Kaname` would hide the namespace the prelude and the rules are under.
const LEAN_STANDARD: [&str; 35] = [
    "Int", "Nat", "Bool", "String", "List", "Option", "Except", "Prop", "Type",
    "Kaname", "Min", "Max", "Inhabited", "Decidable", "Repr", "DecidableEq", "BEq", "Prod", "ToString",
    "True", "False", "Ne", "Eq", "And", "Or", "Not", "Iff", "Exists", "LE", "LT", "GE", "GT", "Membership", "Unit", "Classical",
];

/// A name as Lean writes it: as it is, or in `«»` where it is no plain identifier or is a word Lean
/// keeps (`«do»`, `«明細»`, `«#1»`). `_root_` can never be an identifier, and kaname has no `'`, so
/// it is `_root_'`.
pub fn ident(s: &str) -> String {
    if s == "_root_" {
        return "_root_'".to_string();
    }
    let plain = s.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '\'');
    if plain && !LEAN_KEYWORDS.contains(&s) && s != "Type" && s != "Prop" && s != "Sort" { s.to_string() } else { format!("«{s}»") }
}

/// The Lean name of a kaname name: `ident`'s answer without its `«»`. `«mk'»` and `mk'` are one name
/// in Lean, and the collision sets compare names, not text.
fn lean_name(s: &str) -> String {
    let n = ident(s);
    n.strip_prefix('«').and_then(|x| x.strip_suffix('»')).map(str::to_string).unwrap_or(n)
}

/// The names Lean gives a structure itself, which no field may take: `mk`, the constructor, and the
/// eliminators Lean would write for it were it inductive (`rec`, `recOn`, `casesOn`, `noConfusion`,
/// `noConfusionType`).
const LEAN_STRUCT_MEMBERS: [&str; 6] = ["mk", "rec", "recOn", "casesOn", "noConfusion", "noConfusionType"];

/// The names Lean gives an inductive itself, which no value of it may take: its eliminators.
const LEAN_INDUCTIVE_MEMBERS: [&str; 5] = ["rec", "recOn", "casesOn", "noConfusion", "noConfusionType"];

/// The field of a structure a name of kaname is: `ident`'s, with `'` until it is none of the names
/// Lean writes for the structure itself (kaname's names have no `'`, so the result is one no name of
/// kaname's takes either).
fn field_name(n: &str) -> String {
    let mut s = n.to_string();
    while LEAN_STRUCT_MEMBERS.contains(&s.as_str()) {
        s.push('\'');
    }
    ident(&s)
}

/// The value of an enum a name of kaname is: `ident`'s, with `'` until it is none of the names Lean
/// writes for the inductive itself.
fn enum_value_name(n: &str) -> String {
    let mut s = n.to_string();
    while LEAN_INDUCTIVE_MEMBERS.contains(&s.as_str()) {
        s.push('\'');
    }
    ident(&s)
}

/// A Lean name with `'` once more: `x` to `x'`, `«x»` to `«x'»` (`«x»'` is no name). No name of
/// kaname's has `'`, so the name is one no name of kaname's takes.
fn prime(n: &str) -> String {
    match n.strip_prefix('«').and_then(|s| s.strip_suffix('»')) {
        Some(inside) => format!("«{inside}'»"),
        _ => format!("{n}'"),
    }
}

/// A term as an argument or an operand: as it is where it is one token, else in parentheses.
fn par(code: &str) -> String {
    let simple = !code.is_empty()
        && (code.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '\'' | '«' | '»'))
            && !code.starts_with('-')
            || (code.starts_with('(') && closes_at_end(code))
            || (code.starts_with('[') && closes_at_end(code))
            || (code.starts_with('"') && code.ends_with('"') && !code[1..code.len() - 1].contains('"')));
    if simple { code.to_string() } else { format!("({code})") }
}

/// Whether the Lean `code` reads the name `n` (as `ident` writes it): a word of it, cut at whatever
/// cannot be in a name, or, for a name in `«»`, the name with its brackets. A word inside a string
/// counts too, which leaves a name that is not read in place, never the other way round.
fn reads(code: &str, n: &str) -> bool {
    if n.starts_with('«') {
        return code.contains(n);
    }
    code.split(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '\'' | '«' | '»'))).any(|w| w == n)
}

/// The function that reads each element `v` as `inner` does: `fun _ => …` where `inner` is a
/// constant.
fn each(inner: &str) -> String {
    format!("fun {} => {inner}", if reads(inner, "v") { "v" } else { "_" })
}

/// Whether the bracket a term opens with closes at its end.
fn closes_at_end(code: &str) -> bool {
    let mut depth = 0i32;
    let mut in_str = false;
    let n = code.chars().count();
    for (i, c) in code.chars().enumerate() {
        match c {
            '"' => in_str = !in_str,
            '(' | '[' | '{' if !in_str => depth += 1,
            ')' | ']' | '}' if !in_str => {
                depth -= 1;
                if depth == 0 && i + 1 < n {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

/// An integer as a Lean term.
fn int(n: i128) -> String {
    n.to_string()
}

/// A string as a Lean literal.
fn str_lit(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn rat_abs(r: Rat) -> Rat {
    if r.num < 0 { Rat { num: -r.num, den: r.den } } else { r }
}

/// The greatest step two positive steps share.
fn gcd_rat(a: Rat, b: Rat) -> Rat {
    crate::ty::share(Some(a), Some(b)).unwrap_or(Rat::int(1))
}

/// How a number is held in Lean: a whole number of a step (`grain`) counted in `unit`, the value
/// being the term times the grain; or a `Kaname.Frac`, the value itself.
#[derive(Clone, Debug, PartialEq)]
enum Num {
    Int { unit: ritsu_units::Unit, grain: Rat },
    Frac { unit: ritsu_units::Unit },
}

impl Num {
    fn unit(&self) -> &ritsu_units::Unit {
        match self {
            Num::Int { unit, .. } | Num::Frac { unit } => unit,
        }
    }
}

/// How a value of `t` is held where its type says it (a parameter, a field, an element, what a
/// function answers): a number of a known step as the whole number of its steps, one of no known
/// step as a `Kaname.Frac`.
fn held_as(t: &Ty) -> Option<Num> {
    match t {
        Ty::Num(n) => Some(match n.step {
            Some(s) if s.num != 0 => Num::Int { unit: n.unit.clone(), grain: rat_abs(s) },
            Some(_) => Num::Int { unit: n.unit.clone(), grain: Rat::int(1) },
            None => Num::Frac { unit: n.unit.clone() },
        }),
        _ => None,
    }
}

/// A Lean term and the value of kaname's it holds: its type, and how a number is held in it.
#[derive(Clone, Debug)]
struct T {
    code: String,
    ty: Ty,
    num: Option<Num>,
}

impl T {
    /// A term holding a value of `ty` as its type says.
    fn held(code: impl Into<String>, ty: &Ty) -> T {
        T { code: code.into(), ty: ty.clone(), num: held_as(ty) }
    }

    fn int(code: impl Into<String>, ty: &Ty, unit: ritsu_units::Unit, grain: Rat) -> T {
        T { code: code.into(), ty: ty.clone(), num: Some(Num::Int { unit, grain }) }
    }

    fn frac(code: impl Into<String>, ty: &Ty, unit: ritsu_units::Unit) -> T {
        T { code: code.into(), ty: ty.clone(), num: Some(Num::Frac { unit }) }
    }
}

/// `v` read from `from` in `to` as `a·v + b`: the factor between the units, and the shift between
/// their zeros (a temperature's); a number and a rate read alike, and a value of no unit is read in
/// the unit it meets (`interp::convert`).
fn affine(from: &ritsu_units::Unit, to: &ritsu_units::Unit) -> (Rat, Rat) {
    let at = |r: Rat| crate::interp::convert(r, from, to).unwrap_or(r);
    let b = at(Rat::zero());
    (at(Rat::int(1)).sub(b), b)
}

/// `code · a + b` as a Lean term, for a whole `code`, where the value is whole (the division, where
/// one is written, is exact).
fn scaled(code: &str, a: Rat, b: Rat) -> String {
    if a.num == 0 {
        return if b.is_int() { int(b.num) } else { format!("{} / {}", b.num, b.den) };
    }
    let d = lcm(a.den, b.den);
    let (an, bn) = (a.num * (d / a.den), b.num * (d / b.den));
    let mut out = match an {
        1 => code.to_string(),
        -1 => format!("-{}", par(code)),
        _ => format!("{} * {}", par(code), an),
    };
    if bn != 0 {
        out = if bn > 0 { format!("{out} + {bn}") } else { format!("{out} - {}", -bn) };
    }
    if d != 1 { format!("({out}) / {d}") } else { out }
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

fn lcm(a: i128, b: i128) -> i128 {
    if a == 0 || b == 0 { 1 } else { (a / gcd(a, b) * b).abs() }
}

/// A number held as `num`, as a `Kaname.Frac` of the same value read in `unit`.
fn to_frac(w: &Mw, code: &str, num: &Num, unit: &ritsu_units::Unit) -> String {
    let (a, b) = affine(num.unit(), unit);
    match num {
        Num::Int { grain, .. } => {
            // value = code · grain · a + b
            let k = grain.mul(a);
            let n = if k.num == 1 { code.to_string() } else { format!("{} * {}", par(code), k.num) };
            let base = format!("{} {} {}", w.kaname("Frac.mk"), par(&n), k.den);
            if b.num == 0 { base } else { format!("{a} ({base}) ({m} {} {})", par(&int(b.num)), b.den, a = w.kaname("Frac.add"), m = w.kaname("Frac.mk")) }
        }
        Num::Frac { .. } => {
            let s = if a == Rat::int(1) { code.to_string() } else { format!("{} {} {} {}", w.kaname("Frac.scale"), par(code), par(&int(a.num)), a.den) };
            if b.num == 0 { s } else { format!("{} {} ({} {} {})", w.kaname("Frac.add"), par(&s), w.kaname("Frac.mk"), par(&int(b.num)), b.den) }
        }
    }
}

/// A number held as `num`, as the whole number of `grain` in `unit` that it is.
fn to_int(w: &Mw, code: &str, num: &Num, unit: &ritsu_units::Unit, grain: Rat) -> String {
    let (a, b) = affine(num.unit(), unit);
    match num {
        Num::Int { grain: g, .. } => scaled(code, g.mul(a).div(grain), b.div(grain)),
        Num::Frac { .. } => {
            let k = a.div(grain);
            let s = if k == Rat::int(1) { code.to_string() } else { format!("{} {} {} {}", w.kaname("Frac.scale"), par(code), par(&int(k.num)), k.den) };
            let s = if b.num == 0 { s } else { format!("{} {} ({} {} {})", w.kaname("Frac.add"), par(&s), w.kaname("Frac.mk"), par(&int(b.div(grain).num)), b.div(grain).den) };
            format!("{} {}", w.kaname("Frac.whole"), par(&s))
        }
    }
}

/// Why a function or a statement is not written: what in it Lean is not given. The comment that
/// stands in its place takes the English; a property's goes to W604 in the check's language.
type Res<X> = Result<X, Text>;

/// The functions written so far across the package's modules: by the module's file and the
/// function's place, its Lean name (from outside the module's namespace) and whether it takes the
/// rules and dates (`inner`).
#[derive(Default)]
struct Done {
    fns: std::cell::RefCell<BTreeMap<(std::path::PathBuf, usize), (String, bool)>>,
    /// The namespace of the types of each rule of the package, by its file on the disk.
    rules: std::cell::RefCell<BTreeMap<std::path::PathBuf, String>>,
    /// The one Lean type of each enum of the rules (DESIGN 3.5, 40), by [`rule_enum_key`]: the
    /// inductive under the namespace of the rule that holds it, which every rule that reads the enum
    /// names it by.
    rule_enums: std::cell::RefCell<BTreeMap<RuleEnumKey, String>>,
    /// The structure of an element of a rule (a file's, a sequence's), under the rule's namespace.
    rule_elements: std::cell::RefCell<BTreeMap<(std::path::PathBuf, String), String>>,
    /// The transfers the modules build.
    books: std::cell::RefCell<Vec<crate::ty::TransferId>>,
}

/// An enum of a rule as one type: the file that declares it, made canonical, its name there, and its
/// values in their order (DESIGN 3.5).
type RuleEnumKey = (std::path::PathBuf, String, Vec<String>);

fn rule_enum_key(canon: &std::path::Path, e: &ritsu_ports::RuleEnum) -> RuleEnumKey {
    let (file, name) = crate::names::rule_enum_declared(canon, e);
    (file, name, e.values.iter().map(|v| v.name.clone()).collect())
}

/// A rule or a dates file a module calls, as `Inner` holds it.
#[derive(Clone)]
enum InnerUse {
    Rule { canon: std::path::PathBuf, facts: Rc<ritsu_ports::RuleFacts> },
    Date { canon: std::path::PathBuf, facts: Rc<ritsu_ports::DateFacts>, date: String },
}

/// One module being written.
struct Mw<'a> {
    p: &'a Program,
    c: &'a Checked,
    alias: String,
    /// The standard names the module's own names hide.
    root: BTreeSet<String>,
    /// The names the module puts where its functions' bodies can reach them: its declarations, as
    /// `ident` writes them, and `Inner` (under its own name, `Inner'` where one is declared). A name
    /// a body binds is given a `'` until it is none ([`Fx::local`]); the names it hides that Lean
    /// names from the root are in `root` and stay reachable.
    outer: BTreeSet<String>,
    /// The structure the rules and dates go by: `Inner`, or `Inner'` where the module declares
    /// `Inner` (kaname has no `'`).
    inner_ty: String,
    stats: std::cell::RefCell<LeanStats>,
    done: &'a Done,
    /// The rules and dates `Inner` holds, as the module's functions call them, with the field each
    /// is in the structure.
    uses: std::cell::RefCell<Vec<(InnerUse, String)>>,
    /// The module's functions that are not written, and why.
    skipped: std::cell::RefCell<Vec<(usize, Text)>>,
    /// The module's properties whose statements are not written, and why (W604 takes these in the
    /// check's language).
    skipped_props: std::cell::RefCell<Vec<(usize, Text)>>,
}

impl<'a> Mw<'a> {
    fn new(p: &'a Program, c: &'a Checked, done: &'a Done) -> Mw<'a> {
        let mut own: BTreeSet<String> = BTreeSet::new();
        let mut outer: BTreeSet<String> = BTreeSet::new();
        let mut decl = |n: &str| {
            own.insert(n.to_string());
            outer.insert(ident(n));
        };
        for r in &c.decls.records {
            decl(r.name.ascii());
        }
        for e in &c.decls.enums {
            decl(e.name.ascii());
        }
        for f in &c.decls.fns {
            decl(f.name.ascii());
        }
        for p in &c.module.properties {
            decl(p.name.ascii());
        }
        own.extend(locals(c));
        // the names of `Inner` the functions' bodies can reach: itself, and the one it takes when a
        // declaration of the module has `Inner`
        let inner_ty = if outer.iter().any(|n| lean_name(n) == "Inner") { "Inner'" } else { "Inner" };
        outer.insert(inner_ty.to_string());
        let root = LEAN_STANDARD.iter().filter(|s| own.contains(**s)).map(|s| s.to_string()).collect();
        Mw { p, c, alias: c.module.name.ascii().to_string(), root, outer, inner_ty: inner_ty.to_string(), stats: Default::default(), done, uses: Default::default(), skipped: Default::default(), skipped_props: Default::default() }
    }

    /// A standard name, as the module's namespace reaches it. `Prop`, `Type` and `Sort` are words
    /// of Lean's own: a declaration of one is `«…»`, never the word, so it says the standard's
    /// wherever it is written.
    fn std(&self, n: &str) -> String {
        if n == "Prop" || n == "Type" || n == "Sort" {
            return n.to_string();
        }
        if self.root.contains(n) { format!("_root_.{n}") } else { n.to_string() }
    }

    /// A name under the `Kaname` namespace, as this module writes it: `_root_.Kaname` where one of
    /// its own names hides `Kaname`.
    fn kaname(&self, n: &str) -> String {
        format!("{}.{}", self.std("Kaname"), n)
    }

    /// The prefix of what a module declares, from this one: nothing for its own, `Kaname.<alias>.`
    /// for another's.
    fn of_module(&self, file: &std::path::Path) -> String {
        if file == self.c.canon {
            String::new()
        } else {
            format!("{}.{}.", self.std("Kaname"), self.p.module(file).module.name.ascii())
        }
    }

    fn rule_facts(&self, file: &std::path::Path) -> Res<Rc<ritsu_ports::RuleFacts>> {
        self.p.rules.get(file).cloned().ok_or_else(|| tr!("口が説明しない規則", "a rule the port does not describe"))
    }

    /// The namespace of a rule's types, in `Kaname/Rules.lean`, as a name of the module writes it:
    /// `Kaname.Rules.<the rule's alias>`, from the root where one of the module's names hides
    /// `Kaname`.
    fn rule_ns(&self, file: &std::path::Path) -> Res<String> {
        let ns = self.rule_ns_raw(file)?;
        Ok(ns.replacen("Kaname", &self.std("Kaname"), 1))
    }

    /// [`rule_ns`] as it declares: no declaration's name is written `_root_.`.
    fn rule_ns_raw(&self, file: &std::path::Path) -> Res<String> {
        self.done.rules.borrow().get(file).cloned().ok_or_else(|| tr!("パッケージのどのモジュールも読まない規則の型", "a type of a rule no module of the package reads"))
    }

    /// The Lean type of an enum of the rule in `file`: the one inductive of every rule that reads the
    /// enum (DESIGN 40), else the rule's own name of it.
    fn rule_enum_type(&self, file: &std::path::Path, e: &ritsu_ports::RuleEnum) -> Res<String> {
        if let Some(one) = self.done.rule_enums.borrow().get(&rule_enum_key(file, e)) {
            return Ok(one.replacen("Kaname", &self.std("Kaname"), 1));
        }
        Ok(format!("{}.{}", self.rule_ns(file)?, ident(&e.alias)))
    }

    /// The name of an enum, as this module writes it, with its type arguments.
    fn enum_name(&self, id: &EnumId) -> Res<String> {
        let base = match &id.owner {
            Owner::Module(file) => {
                let e = self.p.enum_info(id).ok_or_else(|| tr!("どのモジュールも宣言しない列挙 `{}`", "the enum `{}`, which no module declares", id.name))?;
                format!("{}{}", self.of_module(file), ident(e.name.ascii()))
            }
            Owner::Rule(file) => {
                let f = self.rule_facts(file)?;
                let e = f.enums.iter().find(|e| e.name == id.name).ok_or_else(|| tr!("規則の列挙 `{}`", "the enum `{}` of a rule", id.name))?;
                self.rule_enum_type(file, e)?
            }
        };
        self.applied(base, &id.args)
    }

    /// A type's name with its arguments.
    fn applied(&self, base: String, args: &[Ty]) -> Res<String> {
        if args.is_empty() {
            return Ok(base);
        }
        let mut out = base;
        for a in args {
            out.push(' ');
            out.push_str(&par(&self.tname(a)?));
        }
        Ok(out)
    }

    /// A value of an enum, as this module writes it: `Tier.gold`.
    fn enum_value(&self, id: &EnumId, value: &str) -> Res<String> {
        let base = match &id.owner {
            Owner::Module(file) => {
                let e = self.p.enum_info(id).ok_or_else(|| tr!("どのモジュールも宣言しない列挙 `{}`", "the enum `{}`, which no module declares", id.name))?;
                format!("{}{}", self.of_module(file), ident(e.name.ascii()))
            }
            Owner::Rule(file) => {
                let f = self.rule_facts(file)?;
                let e = f.enums.iter().find(|e| e.name == id.name).ok_or_else(|| tr!("規則の列挙 `{}`", "the enum `{}` of a rule", id.name))?;
                self.rule_enum_type(file, e)?
            }
        };
        let (values, _) = self.p.values(id);
        let v = values.iter().find(|v| v.is(value)).ok_or_else(|| tr!("その列挙の持たない値 `{value}`", "the value `{value}`, which its enum does not have"))?;
        Ok(format!("{base}.{}", enum_value_name(v.ascii())))
    }

    /// The structure that holds the fields of a value of an enum: `Order.placed.Fields`.
    fn variant_name(&self, file: &std::path::Path, name: &str, value: &str, args: &[Ty]) -> Res<String> {
        let id = EnumId { owner: Owner::Module(file.to_path_buf()), name: name.to_string(), args: args.to_vec() };
        let e = self.p.enum_info(&id).ok_or_else(|| tr!("どのモジュールも宣言しない列挙 `{name}`", "the enum `{name}`, which no module declares"))?;
        let v = e.values.iter().find(|v| v.text == value).ok_or_else(|| tr!("値 `{value}`", "the value `{value}`"))?;
        self.applied(format!("{}{}.{}.Fields", self.of_module(file), ident(e.name.ascii()), enum_value_name(v.ascii())), args)
    }

    /// A type as Lean writes it.
    fn tname(&self, t: &Ty) -> Res<String> {
        Ok(match t {
            Ty::Num(_) => match held_as(t) {
                Some(Num::Frac { .. }) => self.kaname("Frac"),
                _ => self.std("Int"),
            },
            Ty::Bool => self.std("Bool"),
            Ty::Str => self.std("String"),
            Ty::Date => self.std("Int"),
            Ty::Enum(_) | Ty::Record(RecId::Module { .. } | RecId::Variant { .. }) if self.keyed(t) => return Err(tr!("辞書か集合を持つ型を使います", "a type that holds a dict or a set")),
            Ty::Enum(id) => self.enum_name(id)?,
            Ty::Record(RecId::Module { file, name, args }) => {
                let r = self.p.modules.get(file).and_then(|c| c.decls.records.iter().find(|r| r.name.text == *name)).ok_or_else(|| tr!("レコード `{name}` を使います", "the record `{name}`"))?;
                self.applied(format!("{}{}", self.of_module(file), ident(r.name.ascii())), args)?
            }
            Ty::Record(RecId::Variant { file, name, value, args }) => self.variant_name(file, name, value, args)?,
            Ty::Record(RecId::Running(x, w)) => {
                if !matches!(held_as(w), Some(Num::Int { .. })) {
                    return Err(tr!("刻みの分からない重みの累積を使います", "a running total of weights of no known step"));
                }
                format!("{} {}", self.kaname("Running"), par(&self.tname(x)?))
            }
            Ty::Record(RecId::Indexed(x)) => format!("{} {}", self.kaname("Indexed"), par(&self.tname(x)?)),
            Ty::Record(RecId::Outputs(file)) => format!("{}.Outputs", self.rule_ns(file)?),
            Ty::Record(RecId::Element(file, seq)) => {
                if let Some(n) = self.done.rule_elements.borrow().get(&(file.clone(), seq.clone())) {
                    n.replacen("Kaname", &self.std("Kaname"), 1)
                } else {
                    format!("{}.{}", self.rule_ns(file)?, ident(seq))
                }
            }
            Ty::Record(RecId::Entry(..)) => return Err(tr!("辞書の組を使います", "an entry of a dict")),
            Ty::List(x) => format!("{} {}", self.std("List"), par(&self.tname(x)?)),
            Ty::Opt(x) => format!("{} {}", self.std("Option"), par(&self.tname(x)?)),
            Ty::Result(a, e) => format!("{} {} {}", self.std("Except"), par(&self.tname(e)?), par(&self.tname(a)?)),
            Ty::Fn(ps, r, _) => {
                let mut parts = Vec::new();
                for p in ps {
                    parts.push(par(&self.tname(p)?));
                }
                parts.push(par(&self.tname(r)?));
                parts.join(" → ")
            }
            Ty::Param(n) => ident(n),
            Ty::Transfer(id) => self.transfer_name(id)?,
            Ty::Dict(..) | Ty::EmptyDict => return Err(tr!("辞書を使います", "a dict")),
            Ty::Set(_) => return Err(tr!("集合を使います", "a set")),
            Ty::None | Ty::Empty | Ty::Error => return Err(tr!("型の分からない値があります", "a value of no type")),
        })
    }

    /// The structure a transfer's operation is written as, in `Kaname/Books.lean`, as a name of the
    /// module writes it: `Kaname.Books.ledger.capture.«do»`, from the root where one of the
    /// module's names hides `Kaname`.
    fn transfer_name(&self, id: &crate::ty::TransferId) -> Res<String> {
        Ok(self.transfer_decl_name(id)?.replacen("Kaname", &self.std("Kaname"), 1))
    }

    /// [`transfer_name`] as it declares: no declaration's name is written `_root_.`.
    fn transfer_decl_name(&self, id: &crate::ty::TransferId) -> Res<String> {
        let book = self.p.books.get(&id.book).ok_or_else(|| tr!("口が説明しない帳簿", "a book the port does not describe"))?;
        Ok(format!("Kaname.Books.{}.{}.{}", ident(&book.name), ident(&id.transfer), ident(&id.op)))
    }

    /// The Lean value of a constant read where it goes, when it is one (`10000000`).
    fn konst_as(&self, t: &T, to: &Num) -> Option<String> {
        let Ty::Num(n) = &t.ty else { return None };
        let k = n.konst?;
        let (a, b) = affine(&n.unit, to.unit());
        let v = k.mul(a).add(b);
        match to {
            Num::Int { grain, .. } => {
                let c = v.div(*grain);
                c.is_int().then(|| int(c.num))
            }
            Num::Frac { .. } => Some(format!("{} {} {}", self.kaname("Frac.mk"), par(&int(v.num)), v.den)),
        }
    }

    /// The term `t`, holding its value as the type `to` says one is held where it goes (a parameter,
    /// a field, an element, an answer): its number read in the unit and on the step of `to`, a value
    /// put in `some`, the value of an enum's value wrapped in its value, a list's or an option's
    /// elements read so too.
    fn conv(&self, t: &T, to: &Ty) -> Res<String> {
        match to {
            Ty::Num(_) => {
                let target = held_as(to).expect("a number");
                if let Some(k) = self.konst_as(t, &target) {
                    return Ok(k);
                }
                let Some(num) = &t.num else { return Ok(t.code.clone()) };
                Ok(match (&target, num) {
                    (Num::Int { unit, grain }, Num::Int { unit: u, grain: g }) if g == grain && affine(u, unit) == (Rat::int(1), Rat::zero()) => t.code.clone(),
                    (Num::Int { unit, grain }, n) => to_int(self, &t.code, n, unit, *grain),
                    (Num::Frac { unit }, Num::Frac { unit: u }) if affine(u, unit) == (Rat::int(1), Rat::zero()) => t.code.clone(),
                    (Num::Frac { unit }, n) => to_frac(self, &t.code, n, unit),
                })
            }
            Ty::Opt(x) => match &t.ty {
                Ty::None => Ok(format!("{}.none", self.std("Option"))),
                Ty::Opt(y) => {
                    let inner = self.conv(&T::held("v", y), x)?;
                    Ok(if inner == "v" { t.code.clone() } else { format!("{}.map ({}) {}", self.std("Option"), each(&inner), par(&t.code)) })
                }
                Ty::Error => Ok(t.code.clone()),
                _ => Ok(format!("{}.some {}", self.std("Option"), par(&self.conv(t, x)?))),
            },
            Ty::List(x) => match &t.ty {
                Ty::Empty => Ok(format!("([] : {})", self.tname(to)?)),
                Ty::List(y) => {
                    let inner = self.conv(&T::held("v", y), x)?;
                    Ok(if inner == "v" { t.code.clone() } else { format!("{}.map ({}) {}", self.std("List"), each(&inner), par(&t.code)) })
                }
                _ => Ok(t.code.clone()),
            },
            Ty::Result(a, e) => match &t.ty {
                Ty::Result(b, f) => {
                    let ok = self.conv(&T::held("v", b), a)?;
                    let err = self.conv(&T::held("v", f), e)?;
                    let mut out = t.code.clone();
                    if err != "v" {
                        out = format!("{}.mapError ({}) {}", self.std("Except"), each(&err), par(&out));
                    }
                    if ok != "v" {
                        out = format!("{}.map ({}) {}", self.std("Except"), each(&ok), par(&out));
                    }
                    Ok(out)
                }
                _ => Ok(t.code.clone()),
            },
            Ty::Enum(id) => match &t.ty {
                Ty::Record(RecId::Variant { value, .. }) => Ok(format!("{} {}", self.enum_value(id, value)?, par(&t.code))),
                _ => Ok(t.code.clone()),
            },
            Ty::Fn(ps, r, _) => match &t.ty {
                Ty::Fn(qs, s, _) if qs.len() == ps.len() => {
                    let names: Vec<String> = (0..ps.len()).map(|i| format!("a{i}")).collect();
                    let mut args = Vec::new();
                    let mut same = true;
                    for (i, (p, q)) in ps.iter().zip(qs).enumerate() {
                        let a = self.conv(&T::held(names[i].clone(), p), q)?;
                        same &= a == names[i];
                        args.push(par(&a));
                    }
                    let call = if args.is_empty() { t.code.clone() } else { format!("{} {}", par(&t.code), args.join(" ")) };
                    let back = self.conv(&T::held(call.clone(), s), r)?;
                    if same && back == call {
                        return Ok(t.code.clone());
                    }
                    let binders: Vec<String> = names.iter().zip(ps).map(|(n, p)| Ok(format!("({} : {})", if reads(&back, n) { n } else { "_" }, self.tname(p)?))).collect::<Res<_>>()?;
                    Ok(format!("fun {} => {back}", binders.join(" ")))
                }
                _ => Ok(t.code.clone()),
            },
            _ => Ok(t.code.clone()),
        }
    }

    fn count_num(&self, n: &Num) {
        let mut s = self.stats.borrow_mut();
        match n {
            Num::Int { grain, .. } if *grain != Rat::int(1) => s.scaled += 1,
            Num::Frac { .. } => s.fractions += 1,
            _ => {}
        }
    }
}

impl Mw<'_> {
    /// The field of `Inner` a rule or a date is, remembering it the first time it is called: the
    /// rule's alias, `<file>_<date>` for a date, as [`field_name`] writes it, and `'` more where a
    /// use already has it.
    fn inner_use(&self, u: InnerUse) -> String {
        let mut uses = self.uses.borrow_mut();
        let same = |(a, _): &(InnerUse, String)| match (a, &u) {
            (InnerUse::Rule { canon: x, .. }, InnerUse::Rule { canon: y, .. }) => x == y,
            (InnerUse::Date { canon: x, date: d, .. }, InnerUse::Date { canon: y, date: e, .. }) => x == y && d == e,
            _ => false,
        };
        if let Some((_, f)) = uses.iter().find(|x| same(x)) {
            return f.clone();
        }
        let mut field = match &u {
            InnerUse::Rule { facts, .. } => field_name(&facts.alias),
            InnerUse::Date { facts, date, .. } => {
                let alias = facts.functions.iter().find(|f| f.name == *date).map(|f| f.alias.clone()).unwrap_or_else(|| date.clone());
                field_name(&format!("{}_{}", facts.alias, alias))
            }
        };
        while uses.iter().any(|(_, f)| *f == field) {
            field = prime(&field);
        }
        uses.push((u, field.clone()));
        field
    }

    /// The `Ok` of `Inner` as this module writes it: `Ok`, or `Ok'` where a field of `Inner` has
    /// `Ok` (a rule the module calls by that name).
    fn ok_name(&self) -> &'static str {
        if self.uses.borrow().iter().any(|(_, f)| lean_name(f) == "Ok") { "Ok'" } else { "Ok" }
    }

    /// The `Within` of a record or an enum as this module writes it: `Within`, or `Within'` where
    /// the record has a field of `Within` or the enum a value of `Within` (kaname has no `'`).
    fn within_name(&self, t: &Ty) -> &'static str {
        let taken = match t {
            Ty::Record(id @ (RecId::Module { .. } | RecId::Variant { .. })) => self.p.fields(id).iter().any(|(n, _)| lean_name(n.ascii()) == "Within"),
            Ty::Enum(id) => self.p.values(id).0.iter().any(|v| lean_name(v.ascii()) == "Within"),
            _ => false,
        };
        if taken { "Within'" } else { "Within" }
    }

    /// The `rank` of an ordered enum as this module writes it: `rank`, or `rank'` where a value of
    /// the enum has `rank` (Lean writes the inductive's own `rank` for a value, and a `def` may not
    /// take it).
    fn rank_name(&self, id: &EnumId) -> &'static str {
        if self.p.values(id).0.iter().any(|v| lean_name(v.ascii()) == "rank") { "rank'" } else { "rank" }
    }

    /// The `rank` of an ordered enum as a call names it: `<enum>.rank` or `<enum>.rank'`.
    fn enum_rank(&self, id: &EnumId) -> Res<String> {
        Ok(format!("{}.{}", self.enum_name(&EnumId { args: Vec::new(), ..id.clone() })?, self.rank_name(id)))
    }
}

fn sym(op: BinOp) -> &'static str {
    match op {
        BinOp::Lt => "<",
        BinOp::Le => "≤",
        BinOp::Gt => ">",
        BinOp::Ge => "≥",
        BinOp::Eq => "=",
        BinOp::Ne => "≠",
        _ => "?",
    }
}

/// A pattern of a `match` arm as Lean writes it: what it begins with, and the name it binds (as the
/// module writes it, and as Lean does), which follows it once the arm's body says whether it is read.
struct Pat {
    head: String,
    bound: Option<(String, String)>,
}

/// The body of one function or one statement, being written.
struct Fx<'w, 'a> {
    w: &'w Mw<'a>,
    /// The module whose expressions are written: the module, or, for a statement, the same module
    /// with its properties read as functions.
    m: &'a Checked,
    /// The names in scope, each with its value and whether the body read it.
    env: Vec<(String, T, Cell<bool>)>,
    /// The name the rules and dates go by in the body, where it takes them.
    inner: Option<String>,
    /// The names the body's names may not take, as `ident` writes them ([`Fx::local`]): the module's
    /// declarations and `Inner` ([`Mw::outer`]), the function's type parameters (whose types are
    /// written after the binders), and the name the rules and dates go by.
    outer: BTreeSet<String>,
}

impl<'w, 'a> Fx<'w, 'a> {
    /// The name Lean writes for a parameter, a `let`, a lambda's parameter or a name a pattern gives:
    /// [`ident`]'s, with `'` after it until it is none of the names the body can reach (`(t' : t)`,
    /// `let shown'` beside a function `shown`), so that the names written after it still say what
    /// they said. No name of kaname has `'`.
    fn local(&self, w: &str) -> String {
        let mut s = w.to_string();
        while self.outer.contains(&ident(&s)) {
            s.push('\'');
        }
        ident(&s)
    }

    fn ty(&self, e: &Expr) -> Ty {
        self.m.typed.types.get(e.id).cloned().unwrap_or(Ty::Error)
    }

    fn rf(&self, e: &Expr) -> &'a Ref {
        self.m.typed.refs.get(e.id).unwrap_or(&Ref::None)
    }

    /// Binds `name`, and answers where it is in the environment.
    fn bind(&mut self, name: &str, t: T) -> usize {
        self.env.push((name.to_string(), t, Cell::new(false)));
        self.env.len() - 1
    }

    fn lookup(&self, w: &str) -> Option<T> {
        self.env.iter().rev().find(|(n, ..)| n == w).map(|(_, t, read)| {
            read.set(true);
            t.clone()
        })
    }

    /// Whether the body read what is bound at `k`.
    fn read(&self, k: usize) -> bool {
        self.env.get(k).is_some_and(|(.., read)| read.get())
    }

    /// The name a binder writes for what is bound at `k`, once `text`, where it is in scope, is
    /// written: `_` where the body did not read it, or read it as a constant that `text` holds in its
    /// place, so that Lean's linter of unused variables says nothing of the Lean kaname writes
    /// (DESIGN 38.4).
    fn binder(&self, k: usize, name: &str, text: &str) -> String {
        if self.read(k) && reads(text, name) { name.to_string() } else { "_".into() }
    }

    /// The binder of a parameter `Mw::params` bound: named where `text` reads it by its name or its
    /// alias, or where `named` says something else names it (a hypothesis of its range).
    fn param_binder(&self, (name, ty, at): &(String, String, Vec<usize>), named: bool, text: &str) -> String {
        let shown = if named || (at.iter().any(|&k| self.read(k)) && reads(text, name)) { name.as_str() } else { "_" };
        format!("({shown} : {ty})")
    }

    fn ex(&mut self, e: &Expr) -> Res<T> {
        let t = self.ex_kind(e)?;
        if let Some(n) = &t.num {
            self.w.count_num(n);
        }
        Ok(t)
    }

    fn ex_kind(&mut self, e: &Expr) -> Res<T> {
        let ty = self.ty(e);
        match &e.kind {
            ExprKind::Num(n) => {
                let v = if n.unit == "%" { n.value.div(Rat::int(100)) } else { n.value };
                let Ty::Num(nt) = &ty else { return Err(tr!("型の分からない数があります", "a number of no type")) };
                Ok(T::int(int(v.num), &ty, nt.unit.clone(), Rat::new(1, v.den)))
            }
            ExprKind::Date(d) => Ok(T::held(int(*d as i128), &Ty::Date)),
            ExprKind::Str(s) => Ok(T::held(str_lit(s), &Ty::Str)),
            ExprKind::Bool(b) => Ok(T::held(format!("{}.{b}", self.w.std("Bool")), &Ty::Bool)),
            ExprKind::None => Ok(T { code: format!("{}.none", self.w.std("Option")), ty: Ty::None, num: None }),
            ExprKind::Var(w) => match self.rf(e) {
                Ref::EnumValue(id, v) => self.enum_value_term(id, v),
                _ => self.lookup(w).ok_or_else(|| tr!("その場に値の無い名前 `{w}`", "the name `{w}`, which has no value there")),
            },
            ExprKind::Field(base, f, _) => match self.rf(e) {
                Ref::EnumValue(id, v) => self.enum_value_term(id, v),
                Ref::Field(name) => self.field(base, name),
                _ => Err(tr!("`.{f}` を使います", "`.{f}`")),
            },
            ExprKind::Call(callee, args) => self.call(e, callee, args),
            ExprKind::Record(_, base, inits) => self.record(e, base.as_deref(), inits),
            ExprKind::List(xs) => {
                if xs.is_empty() {
                    return Ok(T::held(format!("([] : {})", self.w.tname(&ty)?), &ty));
                }
                let elem = match &ty {
                    Ty::List(x) => (**x).clone(),
                    _ => return Err(tr!("型の分からないリストがあります", "a list of no type")),
                };
                let mut parts = Vec::new();
                for x in xs {
                    let t = self.ex(x)?;
                    parts.push(self.w.conv(&t, &elem)?);
                }
                Ok(T::held(format!("[{}]", parts.join(", ")), &ty))
            }
            ExprKind::Dict(_) => Err(tr!("辞書を使います", "a dict")),
            ExprKind::Neg(x) => {
                let t = self.ex(x)?;
                match &t.num {
                    Some(Num::Int { unit, grain }) => Ok(T::int(format!("-{}", par(&t.code)), &ty, unit.clone(), *grain)),
                    Some(Num::Frac { unit }) => Ok(T::frac(format!("{} {}", self.w.kaname("Frac.neg"), par(&t.code)), &ty, unit.clone())),
                    None => Err(tr!("数でないものへの `-` があります", "`-` of what is no number")),
                }
            }
            ExprKind::Not(x) => {
                let t = self.ex(x)?;
                Ok(T::held(format!("{}.not {}", self.w.std("Bool"), par(&t.code)), &Ty::Bool))
            }
            ExprKind::Bin(a, op, b) => self.binary(e, a, *op, b),
            ExprKind::In(x, _, _) => {
                let t = self.ex(x)?;
                let Ty::Num(to) = &ty else { return Err(tr!("数でないものへの `in` があります", "`in` of no number")) };
                if let Some(Num::Int { unit, grain }) = &t.num {
                    let (a, b) = affine(unit, &to.unit);
                    if b.num == 0 && a.num > 0 {
                        return Ok(T::int(t.code.clone(), &ty, to.unit.clone(), grain.mul(a)));
                    }
                }
                Ok(T::held(self.w.conv(&t, &ty)?, &ty))
            }
            ExprKind::If(c, t, f) => {
                let cond = self.ex(c)?.code;
                let a = self.block(t, &ty)?;
                let b = self.block(f, &ty)?;
                Ok(T::held(format!("if {cond} then {a} else {b}"), &ty))
            }
            ExprKind::Match(s, arms) => self.matching(e, s, arms),
            ExprKind::Lambda(..) => {
                let code = self.lambda(e, &[], None)?;
                Ok(T::held(code, &ty))
            }
            ExprKind::Block(b) => {
                let code = self.block(b, &ty)?;
                Ok(T::held(code, &ty))
            }
        }
    }

    fn enum_value_term(&self, id: &EnumId, v: &str) -> Res<T> {
        let mut code = self.w.enum_value(id, v)?;
        let t = Ty::Enum(id.clone());
        if !id.args.is_empty() {
            code = format!("({code} : {})", self.w.tname(&t)?);
        }
        Ok(T::held(code, &t))
    }

    fn field(&mut self, base: &Expr, name: &str) -> Res<T> {
        let b = self.ex(base)?;
        let Ty::Record(id) = b.ty.present() else { return Err(tr!("レコードでないもののフィールド `{name}` を読みます", "the field `{name}` of what is no record")) };
        let fields = self.w.p.fields(id);
        let Some((n, slot)) = fields.iter().find(|(n, _)| n.text == name || n.is(name)) else { return Err(tr!("フィールド `{name}` を読みます", "the field `{name}`")) };
        let lean = match id {
            RecId::Running(..) | RecId::Indexed(_) => n.text.clone(),
            _ => field_name(n.ascii()),
        };
        Ok(T::held(format!("{}.{lean}", par(&b.code)), &slot.ty))
    }

    /// A block: its `let` lines, and its answer held as `to` says.
    fn block(&mut self, b: &Block, to: &Ty) -> Res<String> {
        let mark = self.env.len();
        let lets = self.lets(b)?;
        let a = self.ex(&b.answer)?;
        let answer = self.w.conv(&a, to)?;
        let out = self.with_lets(&lets, &answer);
        self.env.truncate(mark);
        Ok(out)
    }

    /// The `let` lines of a block, each bound: where it is in the environment, its name, and its
    /// value as Lean writes it.
    fn lets(&mut self, b: &Block) -> Res<Vec<(usize, String, String)>> {
        let mut out = Vec::new();
        for l in &b.lets {
            let v = self.ex(&l.value)?;
            let name = self.local(&l.name);
            let (code, t) = match self.m.typed.bound.get(&l.value.id) {
                Some(bound) if l.ty.is_some() => (self.w.conv(&v, bound)?, T::held(name.clone(), bound)),
                _ => (v.code.clone(), T { code: name.clone(), ..v }),
            };
            let k = self.bind(&l.name, t);
            out.push((k, name, code));
        }
        Ok(out)
    }

    /// The `let` lines before `answer`, once the answer is written: a name that neither the lines
    /// after it nor the answer read is `_`.
    fn with_lets(&self, lets: &[(usize, String, String)], answer: &str) -> String {
        if lets.is_empty() {
            return answer.to_string();
        }
        let mut lines = String::new();
        for (i, (k, name, code)) in lets.iter().enumerate() {
            let after: String = lets[i + 1..].iter().map(|(_, _, c)| format!("{c} ")).chain([answer.to_string()]).collect();
            lines.push_str(&format!("let {} := {code}; ", self.binder(*k, name, &after)));
        }
        format!("({lines}{answer})")
    }

    /// A lambda, its answer held as `to` says (as its type says, without). A lambda that is a value
    /// has a function type, which gives its parameters' types; one given to a builtin has the type of
    /// its body, and the builtin gives its parameters theirs (`params`).
    fn lambda(&mut self, lam: &Expr, params: &[Ty], to: Option<&Ty>) -> Res<String> {
        let ExprKind::Lambda(ps, body) = &lam.kind else { return Err(tr!("ラムダでない関数の値を渡しました", "a function value given as no lambda")) };
        let (pts, ret) = match self.ty(lam) {
            Ty::Fn(pts, ret, _) => (pts, *ret),
            body_ty => (params.to_vec(), body_ty),
        };
        if pts.len() != ps.len() {
            return Err(tr!("引数の型の分からないラムダがあります", "a lambda of parameters of no type"));
        }
        let mark = self.env.len();
        let mut bound = Vec::new();
        for ((p, _), pt) in ps.iter().zip(&pts) {
            let name = self.local(p);
            let tn = self.w.tname(pt)?;
            let k = self.bind(p, T::held(name.clone(), pt));
            bound.push((k, name, tn));
        }
        let b = self.ex(body)?;
        let out = self.w.conv(&b, to.unwrap_or(&ret))?;
        let binders: Vec<String> = bound.iter().map(|(k, name, tn)| format!("({} : {tn})", self.binder(*k, name, &out))).collect();
        self.env.truncate(mark);
        Ok(format!("fun {} => {out}", binders.join(" ")))
    }

    fn matching(&mut self, e: &Expr, s: &Expr, arms: &[Arm]) -> Res<T> {
        let ty = self.ty(e);
        let st = self.ty(s);
        let scrut = self.ex(s)?;
        let code_s = self.w.conv(&scrut, &st)?;
        if arms.iter().any(|a| a.patterns.iter().any(|p| matches!(p, Pattern::Lit(_)))) {
            return self.match_lits(&ty, &st, &code_s, arms);
        }
        // the values a `match` can take, to know when its arms cover them all
        let all: Vec<String> = match st.present() {
            _ if matches!(st, Ty::Opt(_)) => vec!["none".into(), "some".into()],
            Ty::Bool => vec!["true".into(), "false".into()],
            Ty::Result(..) => vec!["ok".into(), "err".into()],
            Ty::Enum(id) => self.w.p.values(id).0.iter().map(|v| v.text.clone()).collect(),
            _ => Vec::new(),
        };
        let mut covered: BTreeSet<String> = BTreeSet::new();
        let mut lines = Vec::new();
        let mut whole = false;
        for arm in arms {
            if whole {
                break;
            }
            let mark = self.env.len();
            let mut pats = Vec::new();
            for p in &arm.patterns {
                let (key, pat) = self.pattern(p, &st)?;
                match key {
                    None => {
                        whole = true;
                        pats.push(pat);
                    }
                    Some(k) => {
                        if covered.insert(k) {
                            pats.push(pat);
                        }
                    }
                }
            }
            if !all.is_empty() && all.iter().all(|v| covered.contains(v)) {
                whole = true;
            }
            if pats.is_empty() {
                self.env.truncate(mark);
                continue;
            }
            let body = self.block(&arm.body, &ty)?;
            // a name the body did not read is `_`, in every pattern of the arm that binds it
            let read: BTreeSet<String> = self.env[mark..].iter().filter(|(.., r)| r.get()).map(|(n, ..)| n.clone()).collect();
            let pats: Vec<String> = pats
                .into_iter()
                .map(|p| match p.bound {
                    Some((source, name)) => format!("{} {}", p.head, if read.contains(&source) && reads(&body, &name) { name } else { "_".into() }),
                    None => p.head,
                })
                .collect();
            self.env.truncate(mark);
            lines.push(format!("| {} => {body}", pats.join(" | ")));
        }
        if !whole {
            // the analysis showed the values left can not come: Lean is given one for them
            let d = self.w.std("Inhabited");
            lines.push(format!("| _ => {d}.default"));
        }
        Ok(T::held(format!("(match {code_s} with {})", lines.join(" ")), &ty))
    }

    /// A pattern as Lean writes it, and the value it takes (None for `else`); the name it gives is
    /// bound.
    fn pattern(&mut self, p: &Pattern, st: &Ty) -> Res<(Option<String>, Pat)> {
        let plain = |head: String| Pat { head, bound: None };
        Ok(match p {
            Pattern::Else(_) => (None, plain("_".into())),
            Pattern::None(_) => (Some("none".into()), plain(format!("{}.none", self.w.std("Option")))),
            Pattern::Some(x, _) => {
                let inner = match st {
                    Ty::Opt(t) => (**t).clone(),
                    t => t.clone(),
                };
                let name = self.local(x);
                self.bind(x, T::held(name.clone(), &inner));
                (Some("some".into()), Pat { head: format!("{}.some", self.w.std("Option")), bound: Some((x.clone(), name)) })
            }
            Pattern::Bool(b, _) => (Some(b.to_string()), plain(format!("{}.{b}", self.w.std("Bool")))),
            Pattern::Value(path, bind, _) => {
                let word = &path[path.len() - 1].0;
                match st.present() {
                    Ty::Result(a, er) => {
                        let ok = word == "ok";
                        let held = if ok { (**a).clone() } else { (**er).clone() };
                        let ctor = format!("{}.{}", self.w.std("Except"), if ok { "ok" } else { "error" });
                        let pat = match bind {
                            Some((b, _)) => {
                                let n = self.local(b);
                                self.bind(b, T::held(n.clone(), &held));
                                Pat { head: ctor.into(), bound: Some((b.clone(), n)) }
                            }
                            None => plain(format!("{ctor} _")),
                        };
                        (Some(if ok { "ok" } else { "err" }.into()), pat)
                    }
                    Ty::Enum(id) => {
                        let (values, _) = self.w.p.values(id);
                        let v = values.iter().find(|v| v.is(word)).ok_or_else(|| tr!("値 `{word}`", "the value `{word}`"))?;
                        let ctor = self.w.enum_value(id, &v.text)?;
                        let has_fields = self.w.p.enum_info(id).and_then(|e| e.fields_of(&v.text)).is_some_and(|fs| !fs.is_empty());
                        let pat = match (bind, has_fields) {
                            (Some((b, _)), true) => {
                                let n = self.local(b);
                                let Owner::Module(file) = &id.owner else { return Err(tr!("規則の列挙のフィールドを持つ値を使います", "a value with fields of a rule's enum")) };
                                let rt = Ty::Record(RecId::Variant { file: file.clone(), name: id.name.clone(), value: v.text.clone(), args: id.args.clone() });
                                self.bind(b, T::held(n.clone(), &rt));
                                Pat { head: ctor, bound: Some((b.clone(), n)) }
                            }
                            (_, true) => plain(format!("{ctor} _")),
                            (_, false) => plain(ctor),
                        };
                        (Some(v.text.clone()), pat)
                    }
                    _ => return Err(tr!("列挙でない値へのパターンがあります", "a pattern of a value of no enum")),
                }
            }
            Pattern::Lit(_) => return Err(tr!("ほかのパターンの中にリテラルがあります", "a literal among other patterns")),
        })
    }

    /// A `match` of literals: the arms one after another, as `if` does.
    fn match_lits(&mut self, ty: &Ty, st: &Ty, code_s: &str, arms: &[Arm]) -> Res<T> {
        let s = "m₀";
        let mut out = format!("let {s} := {code_s}; ");
        let mut closed = false;
        for arm in arms {
            let mut conds = Vec::new();
            let mut else_ = false;
            for p in &arm.patterns {
                match p {
                    Pattern::Lit(x) => {
                        let lit = self.ex(x)?;
                        let lit = self.w.conv(&lit, st)?;
                        conds.push(format!("{}.beq {s} {}", self.w.std("BEq"), par(&lit)));
                    }
                    Pattern::Else(_) => else_ = true,
                    _ => return Err(tr!("リテラルのそばにパターンがあります", "a pattern beside literals")),
                }
            }
            let body = self.block(&arm.body, ty)?;
            if else_ {
                out.push_str(&body);
                closed = true;
                break;
            }
            out.push_str(&format!("if {} then {body} else ", conds.join(" || ")));
        }
        if !closed {
            out.push_str(&format!("{}.default", self.w.std("Inhabited")));
        }
        Ok(T::held(format!("({out})"), ty))
    }

    /// The step of `t`'s values read in `unit`: None for a fraction, zero for a constant 0.
    fn grain_in(&self, t: &T, unit: &ritsu_units::Unit) -> Option<Rat> {
        if let Ty::Num(n) = &t.ty
            && let Some(k) = n.konst
        {
            let (a, b) = affine(&n.unit, unit);
            return Some(rat_abs(k.mul(a).add(b)));
        }
        match &t.num {
            Some(Num::Int { unit: u, grain }) => {
                let (a, b) = affine(u, unit);
                let g = grain.mul(rat_abs(a));
                Some(if b.num == 0 { g } else { gcd_rat(g, rat_abs(b)) })
            }
            _ => None,
        }
    }

    /// `t` as the whole number of `grain` in `unit` it is.
    fn int_in(&self, t: &T, unit: &ritsu_units::Unit, grain: Rat) -> String {
        let target = Num::Int { unit: unit.clone(), grain };
        if let Some(k) = self.w.konst_as(t, &target) {
            return k;
        }
        match &t.num {
            Some(n) => to_int(self.w, &t.code, n, unit, grain),
            None => t.code.clone(),
        }
    }

    fn frac_in(&self, t: &T, unit: &ritsu_units::Unit) -> String {
        let target = Num::Frac { unit: unit.clone() };
        if let Some(k) = self.w.konst_as(t, &target) {
            return k;
        }
        match &t.num {
            Some(Num::Frac { unit: u }) if affine(u, unit) == (Rat::int(1), Rat::zero()) => t.code.clone(),
            Some(n) => to_frac(self.w, &t.code, n, unit),
            None => t.code.clone(),
        }
    }

    /// The greatest whole number not above `t`'s value, in its own unit.
    fn floor(&self, t: &T) -> String {
        if let Ty::Num(n) = &t.ty
            && let Some(k) = n.konst
        {
            return int(k.num.div_euclid(k.den));
        }
        match &t.num {
            Some(Num::Int { grain, .. }) if grain.is_int() => scaled(&t.code, *grain, Rat::zero()),
            Some(Num::Int { grain, .. }) => format!("({} * {}) / {}", par(&t.code), grain.num, grain.den),
            Some(Num::Frac { .. }) => format!("{} {}", self.w.kaname("Frac.floor"), par(&t.code)),
            None => t.code.clone(),
        }
    }

    fn binary(&mut self, e: &Expr, a: &Expr, op: BinOp, b: &Expr) -> Res<T> {
        let ty = self.ty(e);
        let (x, y) = (self.ex(a)?, self.ex(b)?);
        if matches!(op, BinOp::And | BinOp::Or) {
            let o = if op == BinOp::And { "and" } else { "or" };
            return Ok(T::held(format!("{}.{} {} {}", self.w.std("Bool"), o, par(&x.code), par(&y.code)), &Ty::Bool));
        }
        if op.is_comparison() {
            return Ok(T::held(self.compare(&x, op, &y, false)?, &Ty::Bool));
        }
        if let (Ty::Date, Ty::Date) = (&x.ty, &y.ty) {
            return Ok(T::int(format!("{} - {}", x.code, par(&y.code)), &ty, ritsu_units::Unit::number(), Rat::int(1)));
        }
        self.arith(&ty, &x, op, &y)
    }

    /// A comparison, as a `Bool` (`prop` false) or a `Prop`.
    fn compare(&self, x: &T, op: BinOp, y: &T, prop: bool) -> Res<String> {
        let rel = |l: &str, r: &str| -> String {
            let (l, r) = (par(l), par(r));
            match (op, prop) {
                (BinOp::Eq, false) => format!("{}.beq {l} {r}", self.w.std("BEq")),
                (BinOp::Ne, false) => format!("{}.not ({}.beq {l} {r})", self.w.std("Bool"), self.w.std("BEq")),
                (o, false) => format!("{}.decide ({l} {} {r})", self.w.std("Decidable"), sym(o)),
                (o, true) => format!("{l} {} {r}", sym(o)),
            }
        };
        match (&x.ty, &y.ty) {
            (Ty::Num(_), Ty::Num(_)) => {
                let t = crate::interp::common(&x.ty, &y.ty);
                let unit = crate::interp::unit_of(t);
                match (self.grain_in(x, &unit), self.grain_in(y, &unit)) {
                    (Some(gx), Some(gy)) => {
                        let g = crate::ty::share(Some(gx), Some(gy)).filter(|g| g.num != 0).unwrap_or(Rat::int(1));
                        Ok(rel(&self.int_in(x, &unit, g), &self.int_in(y, &unit, g)))
                    }
                    _ => {
                        let (a, b) = (par(&self.frac_in(x, &unit)), par(&self.frac_in(y, &unit)));
                        let f = match op {
                            BinOp::Eq => format!("{} {a} {b}", self.w.kaname("Frac.eq")),
                            BinOp::Ne => format!("!({} {a} {b})", self.w.kaname("Frac.eq")),
                            BinOp::Lt => format!("{} {a} {b}", self.w.kaname("Frac.lt")),
                            BinOp::Le => format!("{} {a} {b}", self.w.kaname("Frac.le")),
                            BinOp::Gt => format!("{} {b} {a}", self.w.kaname("Frac.lt")),
                            _ => format!("{} {b} {a}", self.w.kaname("Frac.le")),
                        };
                        Ok(if prop { format!("{f} = true") } else { f })
                    }
                }
            }
            (Ty::Date, Ty::Date) => Ok(rel(&x.code, &y.code)),
            (Ty::Enum(id), Ty::Enum(_)) if !matches!(op, BinOp::Eq | BinOp::Ne) => {
                let rank = self.w.enum_rank(id)?;
                Ok(rel(&format!("{rank} {}", par(&x.code)), &format!("{rank} {}", par(&y.code))))
            }
            _ if !matches!(op, BinOp::Eq | BinOp::Ne) => Err(tr!("Lean が順序を持たない値どうしを比べます", "an order of values Lean is not given")),
            _ => {
                let t = crate::interp::common(&x.ty, &y.ty).clone();
                let (a, b) = (self.w.conv(x, &t)?, self.w.conv(y, &t)?);
                Ok(rel(&a, &b))
            }
        }
    }

    /// A whole number and its step, of a term that holds one.
    fn as_int(&self, t: &T) -> Option<(String, Rat)> {
        if let Ty::Num(n) = &t.ty
            && let Some(k) = n.konst
        {
            return Some((int(k.num), Rat::new(1, k.den)));
        }
        match &t.num {
            Some(Num::Int { grain, .. }) => Some((t.code.clone(), *grain)),
            _ => None,
        }
    }

    fn arith(&self, ty: &Ty, x: &T, op: BinOp, y: &T) -> Res<T> {
        let Ty::Num(rn) = ty else { return Err(tr!("数でないものを計算します", "arithmetic of what is no number")) };
        let unit = rn.unit.clone();
        let frac_own = |t: &T| -> String {
            match &t.num {
                Some(n) => {
                    let u = n.unit().clone();
                    self.frac_in(t, &u)
                }
                None => t.code.clone(),
            }
        };
        match op {
            BinOp::Add | BinOp::Sub => {
                let o = if op == BinOp::Add { "+" } else { "-" };
                match (self.grain_in(x, &unit), self.grain_in(y, &unit)) {
                    (Some(gx), Some(gy)) => {
                        let g = crate::ty::share(Some(gx), Some(gy)).filter(|g| g.num != 0).unwrap_or(Rat::int(1));
                        let (a, b) = (self.int_in(x, &unit, g), self.int_in(y, &unit, g));
                        Ok(T::int(format!("{a} {o} {}", par(&b)), ty, unit, g))
                    }
                    _ => {
                        let f = if op == BinOp::Add { "add" } else { "sub" };
                        Ok(T::frac(format!("{} {} {}", self.w.kaname(&format!("Frac.{f}")), par(&self.frac_in(x, &unit)), par(&self.frac_in(y, &unit))), ty, unit))
                    }
                }
            }
            BinOp::Mul => match (self.as_int(x), self.as_int(y)) {
                (Some((cx, gx)), Some((cy, gy))) => {
                    let code = if cx == "1" {
                        cy
                    } else if cy == "1" {
                        cx
                    } else {
                        format!("{} * {}", par(&cx), par(&cy))
                    };
                    Ok(T::int(code, ty, unit, gx.mul(gy)))
                }
                _ => Ok(T::frac(format!("{} {} {}", self.w.kaname("Frac.mul"), par(&frac_own(x)), par(&frac_own(y))), ty, unit)),
            },
            _ => {
                let both = matches!(&x.ty, Ty::Num(a) if a.has_unit()) && matches!(&y.ty, Ty::Num(b) if b.has_unit());
                let common_unit = crate::interp::unit_of(crate::interp::common(&x.ty, &y.ty));
                let konst = match &y.ty {
                    Ty::Num(n) => n.konst.filter(|k| k.num != 0),
                    _ => None,
                };
                if let Some(k) = konst {
                    // by a constant: the step divided by it
                    let (xu, k) = if both {
                        let (a, b) = match &y.ty {
                            Ty::Num(n) => affine(&n.unit, &common_unit),
                            _ => (Rat::int(1), Rat::zero()),
                        };
                        (common_unit.clone(), k.mul(a).add(b))
                    } else {
                        (x.num.as_ref().map(|n| n.unit().clone()).unwrap_or(unit.clone()), k)
                    };
                    let negative = k.num < 0;
                    let k = rat_abs(k);
                    return match self.grain_in(x, &xu) {
                        Some(g) if g.num != 0 => {
                            let c = self.int_in(x, &xu, g);
                            let code = if negative { format!("-{}", par(&c)) } else { c };
                            Ok(T::int(code, ty, unit, g.div(k)))
                        }
                        Some(_) => Ok(T::int("0", ty, unit, Rat::int(1))),
                        None => {
                            let f = self.frac_in(x, &xu);
                            let s = format!("{} {} {} {}", self.w.kaname("Frac.scale"), par(&f), if negative { format!("({})", -k.den) } else { k.den.to_string() }, k.num);
                            Ok(T::frac(s, ty, unit))
                        }
                    };
                }
                let (a, b) = if both { (self.frac_in(x, &common_unit), self.frac_in(y, &common_unit)) } else { (frac_own(x), frac_own(y)) };
                Ok(T::frac(format!("{} {} {}", self.w.kaname("Frac.div"), par(&a), par(&b)), ty, unit))
            }
        }
    }
}

impl Fx<'_, '_> {
    fn record(&mut self, e: &Expr, base: Option<&Expr>, inits: &[FieldInit]) -> Res<T> {
        let Ref::Record(id) = self.rf(e) else { return Err(tr!("型の分からないレコードがあります", "a record of no type")) };
        let rt = Ty::Record(id.clone());
        let fields = self.w.p.fields(id);
        let from = match base {
            Some(b) => Some(self.ex(b)?),
            None => None,
        };
        let mut given: Vec<Option<String>> = vec![None; fields.len()];
        for fi in inits {
            let Some(k) = fields.iter().position(|(n, _)| n.is(&fi.name)) else { continue };
            let v = self.ex(&fi.value)?;
            given[k] = Some(self.w.conv(&v, &fields[k].1.ty)?);
        }
        let tn = self.w.tname(&rt)?;
        let mut parts = Vec::new();
        for ((n, _), g) in fields.iter().zip(&given) {
            match g {
                Some(code) => parts.push(format!("{} := {code}", field_name(n.ascii()))),
                None if from.is_some() => {}
                None => parts.push(format!("{} := none", field_name(n.ascii()))),
            }
        }
        let code = match &from {
            Some(b) => format!("({{ {} with {} }} : {tn})", par(&b.code), parts.join(", ")),
            None => format!("({{ {} }} : {tn})", parts.join(", ")),
        };
        Ok(T::held(code, &rt))
    }

    fn call(&mut self, e: &Expr, callee: &Expr, args: &[Arg]) -> Res<T> {
        if let Ref::CallValue = self.rf(e) {
            let f = self.ex(callee)?;
            let Ty::Fn(ps, r, _) = f.ty.clone() else { return Err(tr!("関数でないものを呼びます", "a call of what is no function")) };
            let mut code = par(&f.code);
            for (a, p) in args.iter().zip(&ps) {
                let v = self.ex(a.expr())?;
                code.push(' ');
                code.push_str(&par(&self.w.conv(&v, p)?));
            }
            return Ok(T::held(code, &r));
        }
        match self.rf(callee) {
            Ref::Fn(file, name) => self.call_fn(e, file, name, args),
            Ref::Rule(file) => self.call_rule(file, args),
            Ref::Date(file, name) => self.call_date(file, name, args),
            Ref::Transfer(id) => self.call_transfer(id, args),
            Ref::Builtin(n) => {
                let xs: Vec<&Expr> = args.iter().map(Arg::expr).collect();
                self.builtin(e, n, &xs)
            }
            Ref::Method(n) => {
                let ExprKind::Field(base, _, _) = &callee.kind else { return Err(tr!("`{n}` を呼びます", "`{n}`")) };
                let mut xs: Vec<&Expr> = vec![&**base];
                xs.extend(args.iter().map(Arg::expr));
                self.builtin(e, n, &xs)
            }
            _ => Err(tr!("関数でないものを呼びます", "a call of what is no function")),
        }
    }

    fn call_fn(&mut self, e: &Expr, file: &std::path::Path, name: &str, args: &[Arg]) -> Res<T> {
        let callee = self.w.p.module(file);
        let fi = crate::interp::function(callee, name).ok_or_else(|| tr!("関数 `{name}` を呼びます", "the function `{name}`"))?;
        let Some((lean, takes)) = self.w.done.fns.borrow().get(&(callee.canon.clone(), fi)).cloned() else {
            return Err(tr!("`{name}` を呼びますが、それは書き出されていません", "it calls `{name}`, which is not written"));
        };
        let info = &callee.decls.fns[fi];
        let names = crate::names::tnames(&info.tparams);
        let inst = self.m.typed.inst.get(&e.id).cloned().unwrap_or_default();
        let mut code = if callee.canon == self.w.c.canon { lean } else { self.w.kaname(&format!("{}.{lean}", callee.module.name.ascii())) };
        for (tp, t) in info.tparams.iter().zip(&inst) {
            code.push_str(&format!(" ({} := {})", ident(&tp.name.text), self.w.tname(t)?));
        }
        if takes {
            if callee.canon != self.w.c.canon {
                return Err(tr!("規則か日付を呼ぶ、ほかのモジュールの `{name}` を呼びます", "it calls `{name}` of another module, which calls rules or dates"));
            }
            let inner = self.inner.clone().ok_or_else(|| tr!("規則と日付の渡されない所で、規則と日付を取る呼び出しがあります", "a call that takes the rules and dates, where none are given"))?;
            code.push(' ');
            code.push_str(&inner);
        }
        for (pi, prm) in info.params.iter().enumerate() {
            let slot_ty = crate::ty::subst(&prm.slot.ty, &names, &inst);
            let arg = args.iter().find_map(|a| match a {
                Arg::Named(n, _, x) if prm.name.is(n) => Some(x),
                _ => None,
            });
            let arg = arg.or_else(|| match args.get(pi) {
                Some(Arg::Pos(x)) => Some(x),
                _ => None,
            });
            let Some(x) = arg else { return Err(tr!("`{name}` の呼び出しに引数 `{}` がありません", "a call of `{name}` without its argument `{}`", prm.name.text)) };
            let v = self.ex(x)?;
            code.push(' ');
            code.push_str(&par(&self.w.conv(&v, &slot_ty)?));
        }
        let ret = crate::ty::subst(&info.ret.ty, &names, &inst);
        Ok(T::held(code, &ret))
    }

    fn call_rule(&mut self, file: &std::path::Path, args: &[Arg]) -> Res<T> {
        let facts = self.w.rule_facts(file)?;
        let inner = self.inner.clone().ok_or_else(|| tr!("規則の渡されない所で規則を呼びます", "a rule, where the rules are not given"))?;
        let u = InnerUse::Rule { canon: file.to_path_buf(), facts: facts.clone() };
        let field = self.w.inner_use(u);
        let mut places: Vec<(String, String, Ty)> = facts.inputs.iter().map(|c| (c.name.clone(), c.alias.clone(), crate::names::column_slot(c, file).ty)).collect();
        for (seq, alias, _) in &facts.elements {
            places.push((seq.clone(), alias.clone(), Ty::List(Box::new(Ty::Record(RecId::Element(file.to_path_buf(), seq.clone()))))));
        }
        let mut given: Vec<Option<String>> = vec![None; places.len()];
        for a in args {
            let Arg::Named(n, _, x) = a else { continue };
            let Some(k) = places.iter().position(|(q, _, _)| q == n).or_else(|| places.iter().position(|(_, al, _)| al == n)) else { continue };
            let v = self.ex(x)?;
            given[k] = Some(self.w.conv(&v, &places[k].2)?);
        }
        let mut code = format!("{inner}.{field}");
        for ((_, _, t), g) in places.iter().zip(given) {
            let a = match g {
                Some(a) => a,
                None if matches!(t, Ty::Opt(_)) => format!("{}.none", self.w.std("Option")),
                None => return Err(tr!("入力を渡さない規則の呼び出しがあります", "a rule called without one of its inputs")),
            };
            code.push(' ');
            code.push_str(&par(&a));
        }
        Ok(T::held(code, &Ty::Record(RecId::Outputs(file.to_path_buf()))))
    }

    fn call_date(&mut self, file: &std::path::Path, date: &str, args: &[Arg]) -> Res<T> {
        let facts = self.w.p.dates.get(file).cloned().ok_or_else(|| tr!("口が説明しない日付のファイル", "a dates file the port does not describe"))?;
        let inner = self.inner.clone().ok_or_else(|| tr!("日付の渡されない所で日付を呼びます", "a date, where the dates are not given"))?;
        let f = facts.functions.iter().find(|d| d.name == date || d.alias == date).cloned().ok_or_else(|| tr!("日付 `{date}` を呼びます", "the date `{date}`"))?;
        let u = InnerUse::Date { canon: file.to_path_buf(), facts: facts.clone(), date: f.name.clone() };
        let field = self.w.inner_use(u);
        let mut code = format!("{inner}.{field}");
        for p in &f.params {
            let input = facts.inputs.iter().find(|i| i.name == *p).ok_or_else(|| tr!("入力 `{p}` を渡します", "the input `{p}`"))?;
            let arg = args.iter().find_map(|a| match a {
                Arg::Named(n, _, x) if *n == input.name || *n == input.alias => Some(x),
                _ => None,
            });
            let a = match arg {
                Some(x) => {
                    let v = self.ex(x)?;
                    match input.kind {
                        ritsu_ports::DateKind::Date => self.w.conv(&v, &Ty::Date)?,
                        ritsu_ports::DateKind::Int => self.floor(&v),
                    }
                }
                None => int(input.min as i128),
            };
            code.push(' ');
            code.push_str(&par(&a));
        }
        Ok(T::held(code, &Ty::Date))
    }

    fn call_transfer(&mut self, id: &crate::ty::TransferId, args: &[Arg]) -> Res<T> {
        let (_, params) = self.w.p.transfer(id).ok_or_else(|| tr!("口が説明しない振替", "a transfer the port does not describe"))?;
        let mut parts = Vec::new();
        for (n, t) in &params {
            let arg = args.iter().find_map(|a| match a {
                Arg::Named(q, _, x) if q == n => Some(x),
                _ => None,
            });
            let Some(x) = arg else {
                parts.push(format!("{} := none", field_name(n)));
                continue;
            };
            let v = self.ex(x)?;
            parts.push(format!("{} := {}.some {}", field_name(n), self.w.std("Option"), par(&self.w.conv(&v, t)?)));
        }
        let t = Ty::Transfer(id.clone());
        Ok(T::held(format!("({{ {} }} : {})", parts.join(", "), self.w.tname(&t)?), &t))
    }

    /// The function given to a builtin, as the list's elements are given to it.
    fn given(&mut self, f: &Expr, params: &[Ty], to: Option<&Ty>) -> Res<String> {
        self.lambda(f, params, to)
    }

    fn builtin(&mut self, e: &Expr, name: &str, xs: &[&Expr]) -> Res<T> {
        let ty = self.ty(e);
        let elem = |t: &Ty| -> Ty {
            match t {
                Ty::List(x) | Ty::Opt(x) => (**x).clone(),
                _ => Ty::Error,
            }
        };
        let whole = |unit: ritsu_units::Unit| (unit, Rat::int(1));
        match (name, xs) {
            ("up" | "down" | "half_up" | "half_down" | "half_even", [x, g]) => {
                let tx = self.ex(x)?;
                let tg = self.ex(g)?;
                let Ty::Num(gn) = &tg.ty else { return Err(tr!("数でない刻みに丸めます", "a rounding to no number")) };
                let Some(k) = gn.konst.filter(|k| k.num > 0) else { return Err(tr!("定数でない刻みに丸めます", "a rounding to a step that is no constant")) };
                let xu = tx.num.as_ref().map(|n| n.unit().clone()).ok_or_else(|| tr!("数でないものを丸めます", "a rounding of no number"))?;
                let (a, b) = affine(&gn.unit, &xu);
                let t = rat_abs(k.mul(a).add(b));
                let way = self.w.kaname(&format!("Rounding.{name}"));
                let code = match self.grain_in(&tx, &xu) {
                    Some(g) => {
                        let r = g.div(t);
                        let c = self.int_in(&tx, &xu, g);
                        if r.is_int() { scaled(&c, r, Rat::zero()) } else { format!("{} {way} {} {}", self.w.kaname("roundDiv"), par(&scaled(&c, Rat::int(r.num), Rat::zero())), r.den) }
                    }
                    None => format!("{} {way} {} {} {}", self.w.kaname("Frac.round"), par(&self.frac_in(&tx, &xu)), t.den, t.num),
                };
                Ok(T::int(code, &ty, xu, t))
            }
            ("allocate", [t, c, s]) => {
                let (tt, tc, ts) = (self.ex(t)?, self.ex(c)?, self.ex(s)?);
                let both = crate::interp::common(&tc.ty, &ts.ty).clone();
                let cu = crate::interp::unit_of(&both);
                let tu = tt.num.as_ref().map(|n| n.unit().clone()).ok_or_else(|| tr!("`allocate` の対象が数ではありません", "an allocation of no number"))?;
                let code = match (self.grain_in(&tt, &tu), self.grain_in(&tc, &cu), self.grain_in(&ts, &cu)) {
                    (Some(gt), Some(gc), Some(gs)) if gt.num != 0 && gs.num != 0 => {
                        let gc = if gc.num == 0 { Rat::int(1) } else { gc };
                        let (ct, cc, cs) = (self.int_in(&tt, &tu, gt), self.int_in(&tc, &cu, gc), self.int_in(&ts, &cu, gs));
                        let r = gt.mul(gc).div(gs);
                        let ct = if r.num == 1 { ct } else { format!("{} * {}", par(&ct), r.num) };
                        let cs = if r.den == 1 { cs } else { format!("{} * {}", par(&cs), r.den) };
                        format!("{} {} {} {}", self.w.kaname("allocate"), par(&ct), par(&cc), par(&cs))
                    }
                    (Some(gt), _, _) if gt.num == 0 => "0".into(),
                    _ => {
                        let (ft, fc, fs) = (self.frac_in(&tt, &tu), self.frac_in(&tc, &cu), self.frac_in(&ts, &cu));
                        format!("{} {d} ({div} ({mul} {} {}) {}) 1 1", self.w.kaname("Frac.round"), par(&ft), par(&fc), par(&fs), d = self.w.kaname("Rounding.down"), div = self.w.kaname("Frac.div"), mul = self.w.kaname("Frac.mul"))
                    }
                };
                let (u, g) = whole(tu);
                Ok(T::int(code, &ty, u, g))
            }
            ("mod", [x, n]) => {
                let (tx, tn) = (self.ex(x)?, self.ex(n)?);
                let (u, g) = whole(crate::interp::unit_of(&ty));
                Ok(T::int(format!("{} % {}", par(&self.floor(&tx)), par(&self.floor(&tn))), &ty, u, g))
            }
            ("isqrt", [x]) => {
                let tx = self.ex(x)?;
                let (u, g) = whole(crate::interp::unit_of(&ty));
                Ok(T::int(format!("{} {}", self.w.kaname("isqrt"), par(&self.floor(&tx))), &ty, u, g))
            }
            ("min" | "max", [a, b]) => {
                let (ta, tb) = (self.ex(a)?, self.ex(b)?);
                let (ca, cb) = (self.w.conv(&ta, &ty)?, self.w.conv(&tb, &ty)?);
                let min = name == "min";
                let code = match (&ty, held_as(&ty)) {
                    (Ty::Num(_), Some(Num::Frac { .. })) => {
                        let le = self.w.kaname("Frac.le");
                        let test = if min { format!("{le} m₁ m₂") } else { format!("{le} m₂ m₁") };
                        format!("(let m₁ := {ca}; let m₂ := {cb}; if {test} then m₁ else m₂)")
                    }
                    (Ty::Enum(id), _) => {
                        let rank = self.w.enum_rank(id)?;
                        let test = if min { format!("{rank} m₁ ≤ {rank} m₂") } else { format!("{rank} m₂ ≤ {rank} m₁") };
                        format!("(let m₁ := {ca}; let m₂ := {cb}; if {test} then m₁ else m₂)")
                    }
                    (Ty::Num(_) | Ty::Date, _) => format!("{}.{} {} {}", self.w.std(if min { "Min" } else { "Max" }), if min { "min" } else { "max" }, par(&ca), par(&cb)),
                    _ => return Err(tr!("Lean が順序を持たない値に `{name}` を使います", "`{name}` of values Lean does not order")),
                };
                Ok(T::held(code, &ty))
            }
            ("count", [x]) => {
                let l = self.ex(x)?;
                if matches!(l.ty, Ty::Str) {
                    return Err(tr!("文字列の組み込みの関数 `count` を使います", "`count`, a builtin of strings"));
                }
                let (u, g) = whole(ritsu_units::Unit::number());
                Ok(T::int(format!("({}.length : {})", par(&l.code), self.w.std("Int")), &ty, u, g))
            }
            ("count", [x, f]) => {
                let l = self.ex(x)?;
                let p = self.given(f, &[elem(&l.ty)], None)?;
                let (u, g) = whole(ritsu_units::Unit::number());
                Ok(T::int(format!("(({}.filter ({p}) {}).length : {})", self.w.std("List"), par(&l.code), self.w.std("Int")), &ty, u, g))
            }
            ("sum", [x, f]) => {
                let l = self.ex(x)?;
                let p = self.given(f, &[elem(&l.ty)], Some(&ty))?;
                Ok(T::held(format!("({}.map ({p}) {}).sum", self.w.std("List"), par(&l.code)), &ty))
            }
            ("any" | "all", [x, f]) => {
                let l = self.ex(x)?;
                let p = self.given(f, &[elem(&l.ty)], None)?;
                Ok(T::held(format!("{}.{name} {} ({p})", self.w.std("List"), par(&l.code)), &Ty::Bool))
            }
            ("map", [x, f]) => {
                let l = self.ex(x)?;
                let p = self.given(f, &[elem(&l.ty)], Some(&elem(&ty)))?;
                Ok(T::held(format!("{}.map ({p}) {}", self.w.std("List"), par(&l.code)), &ty))
            }
            ("filter", [x, f]) => {
                let l = self.ex(x)?;
                if !matches!(l.ty, Ty::List(_)) {
                    return Err(tr!("リストでないものに `filter` を使います", "`filter` of what is no list"));
                }
                let p = self.given(f, &[elem(&l.ty)], None)?;
                Ok(T::held(format!("{}.filter ({p}) {}", self.w.std("List"), par(&l.code)), &ty))
            }
            ("first", [x]) => {
                let l = self.ex(x)?;
                Ok(T::held(format!("{}.head? {}", self.w.std("List"), par(&l.code)), &ty))
            }
            ("last", [x]) => {
                let l = self.ex(x)?;
                Ok(T::held(format!("{}.getLast? {}", self.w.std("List"), par(&l.code)), &ty))
            }
            ("first", [x, f]) => {
                let l = self.ex(x)?;
                let p = self.given(f, &[elem(&l.ty)], None)?;
                Ok(T::held(format!("{}.find? ({p}) {}", self.w.std("List"), par(&l.code)), &ty))
            }
            ("last", [x, f]) => {
                let l = self.ex(x)?;
                let p = self.given(f, &[elem(&l.ty)], None)?;
                Ok(T::held(format!("{}.find? ({p}) ({}.reverse {})", self.w.std("List"), self.w.std("List"), par(&l.code)), &ty))
            }
            ("max_of" | "min_of", [x, f]) => {
                let l = self.ex(x)?;
                let to = ty.present().clone();
                let p = self.given(f, &[elem(&l.ty)], Some(&to))?;
                let max = name == "max_of";
                let (a, b) = if max { ("y", "x") } else { ("x", "y") };
                let better = match (&to, held_as(&to)) {
                    (Ty::Num(_), Some(Num::Frac { .. })) => format!("{} {a} {b}", self.w.kaname("Frac.lt")),
                    (Ty::Enum(id), _) => {
                        let rank = self.w.enum_rank(id)?;
                        format!("{}.decide ({rank} {a} < {rank} {b})", self.w.std("Decidable"))
                    }
                    (Ty::Num(_) | Ty::Date, _) => format!("{}.decide ({a} < {b})", self.w.std("Decidable")),
                    _ => return Err(tr!("Lean が順序を持たない値に `{name}` を使います", "`{name}` of values Lean does not order")),
                };
                let tn = self.w.tname(&to)?;
                Ok(T::held(format!("{} (fun (x y : {tn}) => {better}) ({}.map ({p}) {})", self.w.kaname("best?"), self.w.std("List"), par(&l.code)), &ty))
            }
            ("running_totals", [x, f]) => {
                let l = self.ex(x)?;
                let w = match &ty {
                    Ty::List(r) => match &**r {
                        Ty::Record(RecId::Running(_, w)) => (**w).clone(),
                        _ => Ty::Error,
                    },
                    _ => Ty::Error,
                };
                if !matches!(held_as(&w), Some(Num::Int { .. })) {
                    return Err(tr!("刻みの分からない重みの累積を使います", "a running total of weights of no known step"));
                }
                let p = self.given(f, &[elem(&l.ty)], Some(&w))?;
                Ok(T::held(format!("{} {} ({p})", self.w.kaname("runningTotals"), par(&l.code)), &ty))
            }
            ("reduce", [x, first, f]) => {
                let l = self.ex(x)?;
                let acc = ty.clone();
                let init = self.ex(first)?;
                let init = self.w.conv(&init, &acc)?;
                let p = self.given(f, &[acc.clone(), elem(&l.ty)], Some(&acc))?;
                Ok(T::held(format!("{}.foldl ({p}) {} {}", self.w.std("List"), par(&init), par(&l.code)), &acc))
            }
            ("concat", parts) if matches!(ty, Ty::List(_) | Ty::Empty) => {
                let mut out = Vec::new();
                for x in parts.iter() {
                    let t = self.ex(x)?;
                    out.push(par(&self.w.conv(&t, &ty)?));
                }
                Ok(T::held(out.join(" ++ "), &ty))
            }
            ("numbers", [a, b]) => {
                let (ta, tb) = (self.ex(a)?, self.ex(b)?);
                Ok(T::held(format!("{} {} {}", self.w.kaname("numbers"), par(&self.floor(&ta)), par(&self.floor(&tb))), &ty))
            }
            ("ok" | "err", [x]) => {
                let Ty::Result(a, er) = ty.present().clone() else { return Err(tr!("result でないものに `{name}` を使います", "`{name}` of no result")) };
                let v = self.ex(x)?;
                let rt = Ty::Result(a.clone(), er.clone());
                let code = format!("{}.{} {}", self.w.std("Except"), if name == "ok" { "ok" } else { "error" }, par(&self.w.conv(&v, if name == "ok" { &a } else { &er })?));
                Ok(T::held(format!("({code} : {})", self.w.tname(&rt)?), &rt))
            }
            ("append", [xs_, x]) => {
                let (l, v) = (self.ex(xs_)?, self.ex(x)?);
                Ok(T::held(format!("{} ++ [{}]", par(&self.w.conv(&l, &ty)?), self.w.conv(&v, &elem(&ty))?), &ty))
            }
            ("at", [xs_, i]) => {
                let (l, ti) = (self.ex(xs_)?, self.ex(i)?);
                Ok(T::held(format!("{} {} {}", self.w.kaname("at?"), par(&l.code), par(&self.floor(&ti))), &ty))
            }
            ("slice", [xs_, a, b]) if matches!(self.ty(xs_), Ty::List(_)) => {
                let (l, ta, tb) = (self.ex(xs_)?, self.ex(a)?, self.ex(b)?);
                Ok(T::held(format!("{} {} {} {}", self.w.kaname("slice"), par(&l.code), par(&self.floor(&ta)), par(&self.floor(&tb))), &ty))
            }
            ("take" | "drop", [xs_, n]) if matches!(self.ty(xs_), Ty::List(_)) => {
                let (l, tn) = (self.ex(xs_)?, self.ex(n)?);
                Ok(T::held(format!("{}.{name} ({}.toNat {}) {}", self.w.std("List"), self.w.std("Int"), par(&self.floor(&tn)), par(&l.code)), &ty))
            }
            ("reverse", [xs_]) => {
                let l = self.ex(xs_)?;
                Ok(T::held(format!("{}.reverse {}", self.w.std("List"), par(&l.code)), &ty))
            }
            ("contains", [xs_, x]) if matches!(self.ty(xs_), Ty::List(_)) => {
                let (l, v) = (self.ex(xs_)?, self.ex(x)?);
                let et = elem(&l.ty);
                Ok(T::held(format!("{}.contains {} {}", self.w.std("List"), par(&l.code), par(&self.w.conv(&v, &et)?)), &Ty::Bool))
            }
            ("flatten", [xs_]) => {
                let l = self.ex(xs_)?;
                Ok(T::held(format!("{}.flatten {}", self.w.std("List"), par(&l.code)), &ty))
            }
            ("flat_map", [xs_, f]) => {
                let l = self.ex(xs_)?;
                let p = self.given(f, &[elem(&l.ty)], Some(&ty))?;
                Ok(T::held(format!("{}.flatMap ({p}) {}", self.w.std("List"), par(&l.code)), &ty))
            }
            ("filter_map", [xs_, f]) => {
                let l = self.ex(xs_)?;
                let p = self.given(f, &[elem(&l.ty)], Some(&Ty::Opt(Box::new(elem(&ty)))))?;
                Ok(T::held(format!("{}.filterMap ({p}) {}", self.w.std("List"), par(&l.code)), &ty))
            }
            ("indexed", [xs_]) => {
                let l = self.ex(xs_)?;
                Ok(T::held(format!("{} {}", self.w.kaname("indexed"), par(&l.code)), &ty))
            }
            _ => Err(tr!("`{name}` がありますが、それはまだ Lean に書き出せません", "`{name}`, which is not written for Lean yet")),
        }
    }
}

impl Fx<'_, '_> {
    /// A `bool` expression as the `Prop` it says (DESIGN 38.4): comparisons as relations, `and`,
    /// `or` and `not` as `∧`, `∨` and `¬`, `all` and `any` as `∀ x ∈ xs` and `∃ x ∈ xs`, anything
    /// else as `… = true`.
    fn prop(&mut self, e: &Expr) -> Res<String> {
        match &e.kind {
            ExprKind::Bin(a, BinOp::And, b) => Ok(format!("{} ∧ {}", side(self.prop(a)?), side(self.prop(b)?))),
            ExprKind::Bin(a, BinOp::Or, b) => Ok(format!("{} ∨ {}", side(self.prop(a)?), side(self.prop(b)?))),
            ExprKind::Not(x) => Ok(negated(self.prop(x)?)),
            ExprKind::Bin(a, op, b) if op.is_comparison() => {
                let (x, y) = (self.ex(a)?, self.ex(b)?);
                self.compare(&x, *op, &y, true)
            }
            ExprKind::Bool(b) => Ok(self.w.std(if *b { "True" } else { "False" })),
            ExprKind::Block(b) => self.prop_block(b),
            ExprKind::If(c, t, f) => {
                let cond = self.ex(c)?.code;
                Ok(format!("if {cond} then {} else {}", self.prop_block(t)?, self.prop_block(f)?))
            }
            ExprKind::Call(callee, args) if self.quantified(callee, args).is_some() => {
                let Some((list, ps, body, pt, all)) = self.quantified(callee, args) else { unreachable!("matched above") };
                let l = self.ex(list)?;
                let name = self.local(ps);
                let mark = self.env.len();
                self.bind(ps, T::held(name.clone(), &pt));
                let inner = self.prop(body)?;
                self.env.truncate(mark);
                let q = if all { "∀" } else { "∃" };
                Ok(format!("{q} {name} ∈ {}, {inner}", l.code))
            }
            _ => {
                let t = self.ex(e)?;
                Ok(format!("{} = true", par(&t.code)))
            }
        }
    }

    /// A call of `all` or `any` given a lambda of one parameter: the list, the parameter, the body,
    /// the type of the elements, and whether it is `all`.
    fn quantified<'e>(&self, callee: &'e Expr, args: &'e [Arg]) -> Option<(&'e Expr, &'e str, &'e Expr, Ty, bool)> {
        let all = match self.rf(callee) {
            Ref::Builtin("all") | Ref::Method("all") => true,
            Ref::Builtin("any") | Ref::Method("any") => false,
            _ => return None,
        };
        let (list, lam) = match (self.rf(callee), &callee.kind, args) {
            (Ref::Method(_), ExprKind::Field(base, _, _), [f]) => (&**base, f.expr()),
            (Ref::Builtin(_), _, [l, f]) => (l.expr(), f.expr()),
            _ => return None,
        };
        let ExprKind::Lambda(ps, body) = &lam.kind else { return None };
        let Ty::List(et) = self.ty(list) else { return None };
        match ps.as_slice() {
            [(p, _)] => Some((list, p.as_str(), &**body, (*et).clone(), all)),
            _ => None,
        }
    }

    fn prop_block(&mut self, b: &Block) -> Res<String> {
        let mark = self.env.len();
        let lets = self.lets(b)?;
        let answer = self.prop(&b.answer)?;
        let out = self.with_lets(&lets, &answer);
        self.env.truncate(mark);
        Ok(out)
    }
}

/// The `when` lines of a property, from the condition its function is read with: the `and`s the
/// reading put between them split (DESIGN 38.2).
fn when_lines<'e>(e: &'e Expr, made: &[usize], out: &mut Vec<&'e Expr>) {
    match &e.kind {
        ExprKind::Bin(a, BinOp::And, b) if made.contains(&e.id) => {
            when_lines(a, made, out);
            when_lines(b, made, out);
        }
        _ => out.push(e),
    }
}

fn ceil_div(a: Rat, b: Rat) -> i128 {
    let q = a.div(b);
    -((-q.num).div_euclid(q.den))
}

fn floor_div(a: Rat, b: Rat) -> i128 {
    let q = a.div(b);
    q.num.div_euclid(q.den)
}

/// The names the elements of nested lists are bound by in a hypothesis.
const BOUND: [&str; 6] = ["e₁", "e₂", "e₃", "e₄", "e₅", "e₆"];

impl Mw<'_> {
    /// What a value of `slot` keeps (DESIGN 38.4): the range of its numbers and dates, the count of
    /// a list and of a string's code points, each element held so, and a record's `Within`.
    fn within(&self, term: &str, slot: &Slot, depth: usize) -> Res<Vec<String>> {
        let mut out = Vec::new();
        let count = |out: &mut Vec<String>, len: String| {
            if let Some(c) = &slot.count {
                if let Some(lo) = c.min {
                    out.push(format!("{} ≤ {len}", int(lo.num.div_euclid(lo.den))));
                }
                if let Some(hi) = c.max {
                    out.push(format!("{len} ≤ {}", int(hi.num.div_euclid(hi.den))));
                }
            }
        };
        match &slot.ty {
            Ty::Num(_) => {
                if let (Some(r), Some(Num::Int { grain, .. })) = (&slot.range, held_as(&slot.ty)) {
                    if let Some(lo) = r.min {
                        out.push(format!("{} ≤ {term}", int(ceil_div(lo, grain))));
                    }
                    if let Some(hi) = r.max {
                        out.push(format!("{term} ≤ {}", int(floor_div(hi, grain))));
                    }
                }
            }
            Ty::Date => {
                if let Some(r) = &slot.range {
                    if let Some(lo) = r.min {
                        out.push(format!("{} ≤ {term}", int(lo.num)));
                    }
                    if let Some(hi) = r.max {
                        out.push(format!("{term} ≤ {}", int(hi.num)));
                    }
                }
            }
            Ty::Str => count(&mut out, format!("{}.length {}", self.std("String"), par(term))),
            Ty::List(x) => {
                count(&mut out, format!("{}.length {}", self.std("List"), par(term)));
                let Some(b) = BOUND.get(depth) else { return Ok(out) };
                let inner = self.within(b, &Slot { ty: (**x).clone(), range: slot.range.clone(), count: None }, depth + 1)?;
                if !inner.is_empty() {
                    out.push(format!("(∀ {b} ∈ {term}, {})", inner.join(" ∧ ")));
                }
            }
            Ty::Opt(x) => {
                let Some(b) = BOUND.get(depth) else { return Ok(out) };
                let inner = self.within(b, &Slot { ty: (**x).clone(), range: slot.range.clone(), count: slot.count.clone() }, depth + 1)?;
                if !inner.is_empty() {
                    out.push(format!("(∀ {b}, {term} = {}.some {b} → {})", self.std("Option"), inner.join(" ∧ ")));
                }
            }
            t @ (Ty::Record(RecId::Module { .. }) | Ty::Enum(_)) if self.has_within(t) => out.push(format!("{}.{}", par(term), self.within_name(t))),
            _ => {}
        }
        Ok(out)
    }

    /// Whether a value of a record or an enum of a module keeps anything (a `Within` that says more
    /// than `True`). A type that holds itself is said to keep nothing.
    fn has_within(&self, t: &Ty) -> bool {
        if self.p.reaches_recursive(t) || self.keyed(t) {
            return false;
        }
        match t {
            Ty::Record(id @ (RecId::Module { .. } | RecId::Variant { .. })) => self.p.fields(id).iter().any(|(_, s)| self.within("x", s, 0).is_ok_and(|w| !w.is_empty())),
            Ty::Enum(id) if self.p.is_sum(id) => self.p.values(id).0.iter().any(|v| self.p.value_fields(id, &v.text).iter().any(|(_, s)| self.within("x", s, 0).is_ok_and(|w| !w.is_empty()))),
            _ => false,
        }
    }

    /// The type parameters of a declaration, as its binders: `(T : Type)`, or `{T : Type}`.
    fn tbinders(&self, ps: &[crate::names::TParamInfo], implicit: bool) -> String {
        if ps.is_empty() {
            return String::new();
        }
        let names: Vec<String> = ps.iter().map(|p| ident(&p.name.text)).collect();
        if implicit { format!(" {{{} : {}}}", names.join(" "), self.std("Type")) } else { format!(" ({} : {})", names.join(" "), self.std("Type")) }
    }

    /// The type parameters of a function: each a type whose values compare (`==` and `!=` of them).
    fn fn_tbinders(&self, ps: &[crate::names::TParamInfo]) -> String {
        if ps.is_empty() {
            return String::new();
        }
        let names: Vec<String> = ps.iter().map(|p| ident(&p.name.text)).collect();
        let eqs: String = names.iter().map(|n| format!(" [{} {n}]", self.std("DecidableEq"))).collect();
        format!(" {{{} : {}}}{eqs}", names.join(" "), self.std("Type"))
    }

    /// The module's records and enums, each after the types it holds; those that hold each other in
    /// one `mutual` block. Then each one's `Within`, and the order of an ordered enum.
    fn types(&self) -> Res<String> {
        let c = self.c;
        let nr = c.decls.records.len();
        let n = nr + c.decls.enums.len();
        // the module's types each holds
        let mut deps: Vec<Vec<usize>> = vec![Vec::new(); n];
        let slots_of = |k: usize| -> Vec<Ty> {
            if k < nr {
                c.decls.records[k].fields.iter().map(|f| f.slot.ty.clone()).collect()
            } else {
                c.decls.enums[k - nr].fields.iter().flatten().map(|f| f.slot.ty.clone()).collect()
            }
        };
        for (k, d) in deps.iter_mut().enumerate() {
            let mut found = Vec::new();
            for t in slots_of(k) {
                collect_own(&t, &c.canon, &mut found);
            }
            for (rec, name) in found {
                let j = if rec { c.decls.records.iter().position(|r| r.name.text == name) } else { c.decls.enums.iter().position(|e| e.name.text == name).map(|i| i + nr) };
                if let Some(j) = j
                    && !d.contains(&j)
                {
                    d.push(j);
                }
            }
        }
        let order = sccs(&deps);
        // a name of the module to the member that has it: the records 0..nr, the enums nr..n
        let rec_of: BTreeMap<String, usize> = c.decls.records.iter().enumerate().map(|(k, r)| (r.name.text.clone(), k)).collect();
        let enum_of: BTreeMap<String, usize> = c.decls.enums.iter().enumerate().map(|(k, e)| (e.name.text.clone(), k + nr)).collect();
        let recursive: Vec<bool> = c.decls.records.iter().map(|r| r.recursive).chain(c.decls.enums.iter().map(|e| e.recursive)).collect();
        // of each member, and of each of an enum's values' fields: whether Lean cannot derive
        // `DecidableEq` of it (it is of a `mutual` part or `recursive`, or it holds one it cannot)
        // — a `noncomputable` classical one is written for it — and whether `Inhabited` of it can
        // be written at all, each entity after the ones its fields need
        let mut nc = vec![false; n];
        let mut nc_f: Vec<Vec<bool>> = vec![Vec::new(); n];
        let mut inh = vec![false; n];
        let mut inh_f: Vec<Vec<bool>> = vec![Vec::new(); n];
        let mut out = String::new();
        for group in &order {
            let mutual = group.len() > 1 || deps[group[0]].contains(&group[0]);
            let keyed = group.iter().any(|&k| {
                let t = if k < nr {
                    let r = &c.decls.records[k];
                    Ty::Record(RecId::Module { file: c.canon.clone(), name: r.name.text.clone(), args: r.tparams.iter().map(|p| Ty::Param(p.name.text.clone())).collect() })
                } else {
                    let e = &c.decls.enums[k - nr];
                    Ty::Enum(EnumId { owner: Owner::Module(c.canon.clone()), name: e.name.text.clone(), args: e.tparams.iter().map(|p| Ty::Param(p.name.text.clone())).collect() })
                };
                self.keyed(&t)
            });
            if keyed {
                for &k in group {
                    let n = if k < nr { &c.decls.records[k].name.text } else { &c.decls.enums[k - nr].name.text };
                    out.push_str(&format!("-- `{n}` is not written for Lean: it holds a dict or a set.\n"));
                }
                out.push('\n');
                continue;
            }
            for &k in group {
                nc[k] = mutual || recursive[k] || deps[k].iter().any(|&j| nc[j]);
                if k >= nr {
                    let e = &c.decls.enums[k - nr];
                    nc_f[k] = e.fields.iter().map(|fs| mutual || fs.iter().any(|f| held_nc(&f.slot.ty, &c.canon, &rec_of, &enum_of, &c.decls.enums, nr, &nc, &nc_f))).collect();
                    inh_f[k] = vec![false; e.fields.len()];
                }
            }
            // what `Inhabited` can be written of, each after the entities its fields need: a record
            // where every field can be made, an enum where every field of one of its values can be
            let mut made = Vec::new();
            loop {
                let mut more = false;
                for &k in group {
                    if k < nr {
                        if !inh[k] && c.decls.records[k].fields.iter().all(|f| !needs_inh(&f.slot.ty, &c.canon, &rec_of, &enum_of, &c.decls.enums, nr, &inh, &inh_f)) {
                            inh[k] = true;
                            more = true;
                            made.push(ident(c.decls.records[k].name.ascii()));
                        }
                    } else {
                        let e = &c.decls.enums[k - nr];
                        for (vi, (v, fs)) in e.values.iter().zip(&e.fields).enumerate() {
                            if !fs.is_empty() && !inh_f[k][vi] && fs.iter().all(|f| !needs_inh(&f.slot.ty, &c.canon, &rec_of, &enum_of, &c.decls.enums, nr, &inh, &inh_f)) {
                                inh_f[k][vi] = true;
                                more = true;
                                made.push(format!("{}.{}.Fields", ident(e.name.ascii()), enum_value_name(v.ascii())));
                            }
                        }
                        if !inh[k] && e.fields.iter().enumerate().any(|(vi, fs)| fs.is_empty() || inh_f[k][vi]) {
                            inh[k] = true;
                            more = true;
                            made.push(ident(e.name.ascii()));
                        }
                    }
                }
                if !more {
                    break;
                }
            }
            let mut text = String::new();
            let mut post = String::new();
            let mut names = Vec::new();
            for &k in group {
                if k < nr {
                    let r = &c.decls.records[k];
                    names.push(ident(r.name.ascii()));
                    text.push_str(&self.record_decl(k, &if mutual { Vec::new() } else { classes(nc[k], inh[k]) })?);
                    if nc[k] {
                        post.push_str(&self.nc_dec_eq(&ident(r.name.ascii()), &r.tparams));
                    }
                } else {
                    let e = &c.decls.enums[k - nr];
                    for (v, fs) in e.values.iter().zip(&e.fields) {
                        if !fs.is_empty() {
                            names.push(format!("{}.{}.Fields", ident(e.name.ascii()), enum_value_name(v.ascii())));
                        }
                    }
                    names.push(ident(e.name.ascii()));
                    let fs_cls: Vec<Vec<&'static str>> = e.fields.iter().enumerate().map(|(vi, _)| if mutual { Vec::new() } else { classes(nc_f[k][vi], inh_f[k][vi]) }).collect();
                    text.push_str(&self.enum_decl(k - nr, &fs_cls, &if mutual { Vec::new() } else { classes(nc[k], inh[k]) })?);
                    if nc[k] {
                        post.push_str(&self.nc_dec_eq(&ident(e.name.ascii()), &e.tparams));
                    }
                    for (vi, (v, fs)) in e.values.iter().zip(&e.fields).enumerate() {
                        if !fs.is_empty() && nc_f[k][vi] {
                            post.push_str(&self.nc_dec_eq(&format!("{}.{}.Fields", ident(e.name.ascii()), enum_value_name(v.ascii())), &e.tparams));
                        }
                    }
                }
            }
            let decls = group.len() + group.iter().filter(|&&k| k >= nr).map(|&k| c.decls.enums[k - nr].fields.iter().filter(|f| !f.is_empty()).count()).sum::<usize>();
            if mutual && decls > 1 {
                out.push_str("mutual\n");
                out.push_str(&text);
                out.push_str("end\n");
            } else {
                out.push_str(&text);
            }
            if mutual {
                out.push_str(&format!("deriving instance {}, {} for {}\n", self.std("Repr"), self.std("BEq"), names.join(", ")));
                if !made.is_empty() {
                    out.push_str(&format!("deriving instance {} for {}\n", self.std("Inhabited"), made.join(", ")));
                }
            }
            out.push_str(&post);
            out.push('\n');
        }
        // what each keeps, and the order of an ordered enum, in the order of the groups: a type's
        // `Within` reads those of the types its fields hold
        for group in &order {
            for &k in group {
                if k < nr {
                    let r = &c.decls.records[k];
                    let t = Ty::Record(RecId::Module { file: c.canon.clone(), name: r.name.text.clone(), args: r.tparams.iter().map(|p| Ty::Param(p.name.text.clone())).collect() });
                    if self.has_within(&t) {
                        let mut conj = Vec::new();
                        for f in &r.fields {
                            conj.extend(self.within(&format!("x.{}", field_name(f.name.ascii())), &f.slot, 0)?);
                        }
                        out.push_str(&format!("def {}.{}{} (x : {}) : {} :=\n  {}\n\n", ident(r.name.ascii()), self.within_name(&t), self.tbinders(&r.tparams, true), self.tname(&t)?, self.std("Prop"), conj.join(" ∧ ")));
                    }
                } else {
                    let e = &c.decls.enums[k - nr];
                    let args: Vec<Ty> = e.tparams.iter().map(|p| Ty::Param(p.name.text.clone())).collect();
                    let id = EnumId { owner: Owner::Module(c.canon.clone()), name: e.name.text.clone(), args: args.clone() };
                    let t = Ty::Enum(id.clone());
                    if self.keyed(&t) {
                        continue;
                    }
                    if e.is_sum() && self.has_within(&t) {
                        let mut arms = Vec::new();
                        for (v, fs) in e.values.iter().zip(&e.fields) {
                            let ctor = format!("{}.{}", ident(e.name.ascii()), enum_value_name(v.ascii()));
                            if fs.is_empty() {
                                arms.push(format!("  | {ctor} => {}", self.std("True")));
                                continue;
                            }
                            let mut conj = Vec::new();
                            for f in fs {
                                let s = Slot { ty: crate::ty::subst(&f.slot.ty, &crate::names::tnames(&e.tparams), &args), ..f.slot.clone() };
                                conj.extend(self.within(&format!("x.{}", field_name(f.name.ascii())), &s, 0)?);
                            }
                            arms.push(if conj.is_empty() { format!("  | {ctor} _ => {}", self.std("True")) } else { format!("  | {ctor} x => {}", conj.join(" ∧ ")) });
                        }
                        out.push_str(&format!("def {}.{}{} : {} → {}\n{}\n\n", ident(e.name.ascii()), self.within_name(&t), self.tbinders(&e.tparams, true), self.tname(&t)?, self.std("Prop"), arms.join("\n")));
                    }
                    if e.ordered {
                        let arms: Vec<String> = e.values.iter().enumerate().map(|(i, v)| format!("  | {}.{} => {i}", ident(e.name.ascii()), enum_value_name(v.ascii()))).collect();
                        out.push_str(&format!("def {}.{}{} : {} → {}\n{}\n\n", ident(e.name.ascii()), self.rank_name(&id), self.tbinders(&e.tparams, true), self.tname(&t)?, self.std("Nat"), arms.join("\n")));
                    }
                }
            }
        }
        Ok(out)
    }

    fn deriving(&self, classes: &[&'static str]) -> String {
        if classes.is_empty() { String::new() } else { format!("  deriving {}\n", classes.iter().map(|c| self.std(c)).collect::<Vec<_>>().join(", ")) }
    }

    /// `DecidableEq` of a declaration Lean cannot derive it of, the classical way (the instances
    /// the types it appears in ask for are answered; `=` of it runs on the `BEq` it derives).
    fn nc_dec_eq(&self, name: &str, tparams: &[crate::names::TParamInfo]) -> String {
        let t = if tparams.is_empty() { name.to_string() } else { format!("({name} {})", tparams.iter().map(|p| ident(&p.name.text)).collect::<Vec<_>>().join(" ")) };
        format!("noncomputable instance : {} {t} := fun a b => {}.propDecidable (a = b)\n", self.std("DecidableEq"), self.std("Classical"))
    }

    fn record_decl(&self, k: usize, classes: &[&'static str]) -> Res<String> {
        let r = &self.c.decls.records[k];
        let mut out = format!("structure {}{} where\n", ident(r.name.ascii()), self.tbinders(&r.tparams, false));
        for f in &r.fields {
            out.push_str(&format!("  {} : {}\n", field_name(f.name.ascii()), self.tname(&f.slot.ty)?));
        }
        out.push_str(&self.deriving(classes));
        Ok(out)
    }

    fn enum_decl(&self, k: usize, fields: &[Vec<&'static str>], own: &[&'static str]) -> Res<String> {
        let e = &self.c.decls.enums[k];
        let name = ident(e.name.ascii());
        let targs = if e.tparams.is_empty() { String::new() } else { format!(" {}", e.tparams.iter().map(|p| ident(&p.name.text)).collect::<Vec<_>>().join(" ")) };
        let mut out = String::new();
        for (vi, (v, fs)) in e.values.iter().zip(&e.fields).enumerate() {
            if fs.is_empty() {
                continue;
            }
            out.push_str(&format!("structure {name}.{}.Fields{} where\n", enum_value_name(v.ascii()), self.tbinders(&e.tparams, false)));
            for f in fs {
                out.push_str(&format!("  {} : {}\n", field_name(f.name.ascii()), self.tname(&f.slot.ty)?));
            }
            out.push_str(&self.deriving(&fields[vi]));
        }
        out.push_str(&format!("inductive {name}{} where\n", self.tbinders(&e.tparams, false)));
        for (v, fs) in e.values.iter().zip(&e.fields) {
            if fs.is_empty() {
                out.push_str(&format!("  | {}\n", enum_value_name(v.ascii())));
            } else {
                out.push_str(&format!("  | {} (x : {name}.{}.Fields{targs})\n", enum_value_name(v.ascii()), enum_value_name(v.ascii())));
            }
        }
        out.push_str(&self.deriving(own));
        Ok(out)
    }

    /// The types of a rule, under its namespace: its enums, the elements of the lists it walks, and
    /// its outputs. Each enum of the rules is one inductive (DESIGN 40), written where the first rule
    /// that reads it is (`written` holds those written so far); a rule's name of an enum another rule
    /// holds stands for that inductive, and so does its order.
    fn rule_types(&self, canon: &std::path::Path, facts: &ritsu_ports::RuleFacts, written: &mut BTreeSet<String>) -> Res<String> {
        let mut out = String::new();
        // the inductives this rule's text writes, held as written once the text is
        let mut mine: Vec<String> = Vec::new();
        {
            let ns = self.rule_ns_raw(canon)?;
            let mut taken: BTreeSet<String> = BTreeSet::new();
            taken.insert(format!("{ns}.Outputs"));
            taken.extend(self.done.rule_elements.borrow().values().filter(|o| o.starts_with(&format!("{ns}."))).cloned());
            taken.extend(self.done.rule_enums.borrow().values().filter(|o| o.starts_with(&format!("{ns}."))).cloned());
            for e in &facts.enums {
                let mut a = e.alias.clone();
                while taken.contains(&format!("{ns}.{}", ident(&a))) {
                    a.push('\'');
                }
                let own = format!("{ns}.{}", ident(&a));
                taken.insert(own.clone());
                let one = self.done.rule_enums.borrow().get(&rule_enum_key(canon, e)).cloned().unwrap_or_else(|| own.clone());
                if !written.contains(&one) && !mine.contains(&one) {
                    mine.push(one.clone());
                    out.push_str(&format!("inductive {one} where\n"));
                    for v in &e.values {
                        out.push_str(&format!("  | {}\n", enum_value_name(&v.public)));
                    }
                    out.push_str(&format!("  deriving {}, {}, {}\n\n", self.std("Repr"), self.std("DecidableEq"), self.std("Inhabited")));
                    if e.ordered {
                        let rank = e.values.iter().any(|v| lean_name(&v.public) == "rank");
                        let rank = if rank { "rank'" } else { "rank" };
                        let arms: Vec<String> = e.values.iter().enumerate().map(|(i, v)| format!("  | {one}.{} => {i}", enum_value_name(&v.public))).collect();
                        out.push_str(&format!("def {one}.{rank} : {one} → {}\n{}\n\n", self.std("Nat"), arms.join("\n")));
                    }
                }
                // the rule's own name of the enum stands for the inductive; its `rank` follows it,
                // as `own.rank` reaches `one`'s through the `abbrev`
                if one != own {
                    out.push_str(&format!("abbrev {own} := {one}\n\n"));
                }
            }
            for (seq, _, cols) in &facts.elements {
                let name = self.done.rule_elements.borrow().get(&(canon.to_path_buf(), seq.clone())).cloned().ok_or_else(|| tr!("要素 `{seq}` があります", "the element `{seq}`"))?;
                out.push_str(&format!("structure {name} where\n"));
                for col in cols {
                    out.push_str(&format!("  {} : {}\n", field_name(&col.alias), self.tname(&crate::names::column_slot(col, canon).ty)?));
                }
                out.push_str(&format!("  deriving {}, {}, {}\n\n", self.std("Repr"), self.std("DecidableEq"), self.std("Inhabited")));
            }
            out.push_str(&format!("structure {ns}.Outputs where\n"));
            for col in &facts.outputs {
                out.push_str(&format!("  {} : {}\n", field_name(&col.alias), self.tname(&crate::names::column_slot(col, canon).ty)?));
            }
            out.push_str(&format!("  deriving {}, {}, {}\n\n", self.std("Repr"), self.std("DecidableEq"), self.std("Inhabited")));
        }
        written.extend(mine);
        Ok(out)
    }

    /// The structures of transfers, one for each operation: each argument a value that may be none,
    /// as a call may leave one out.
    fn transfer_types(&self, ids: &[crate::ty::TransferId]) -> Res<String> {
        let mut out = String::new();
        for id in ids {
            let (_, params) = self.p.transfer(id).ok_or_else(|| tr!("口が説明しない振替", "a transfer the port does not describe"))?;
            out.push_str(&format!("structure {} where\n", self.transfer_decl_name(id)?));
            for (n, t) in &params {
                out.push_str(&format!("  {} : {}\n", field_name(n), self.tname(&Ty::Opt(Box::new(t.clone())))?));
            }
            out.push_str(&format!("  deriving {}, {}, {}\n\n", self.std("Repr"), self.std("DecidableEq"), self.std("Inhabited")));
        }
        Ok(out)
    }

    /// The structure of the rules and dates the module's functions call, and what their ports say
    /// of them (`Inner.Ok`): of a rule, that inputs within their ranges give outputs within theirs.
    fn inner_decl(&self) -> Res<String> {
        let uses = self.uses.borrow().clone();
        if uses.is_empty() {
            return Ok(String::new());
        }
        let mut fields = Vec::new();
        let mut oks = Vec::new();
        for (u, field) in &uses {
            match u {
                InnerUse::Rule { canon, facts } => {
                    let mut places: Vec<Slot> = facts.inputs.iter().map(|c| crate::names::column_slot(c, canon)).collect();
                    for (seq, _, _) in &facts.elements {
                        places.push(Slot::of(Ty::List(Box::new(Ty::Record(RecId::Element(canon.clone(), seq.clone()))))));
                    }
                    let mut tys = Vec::new();
                    for s in &places {
                        tys.push(par(&self.tname(&s.ty)?));
                    }
                    let ns = self.rule_ns(canon)?;
                    fields.push(format!("  {field} : {}{ns}.Outputs", tys.iter().map(|t| format!("{t} → ")).collect::<String>()));
                    // inputs within their ranges give outputs within theirs
                    let names: Vec<String> = (1..=places.len()).map(|i| format!("a{i}")).collect();
                    let mut hyps = Vec::new();
                    for (n, s) in names.iter().zip(&places) {
                        hyps.extend(self.within(n, s, 0)?);
                    }
                    let call = if names.is_empty() { format!("inner.{field}") } else { format!("(inner.{field} {})", names.join(" ")) };
                    let mut goals = Vec::new();
                    for col in &facts.outputs {
                        goals.extend(self.within(&format!("{call}.{}", field_name(&col.alias)), &crate::names::column_slot(col, canon), 0)?);
                    }
                    if !goals.is_empty() {
                        let binders: Vec<String> = names.iter().zip(&tys).map(|(n, t)| format!("({n} : {t})")).collect();
                        let lead = if binders.is_empty() { String::new() } else { format!("∀ {}, ", binders.join(" ")) };
                        let hy: String = hyps.iter().map(|h| format!("{h} → ")).collect();
                        oks.push(format!("({lead}{hy}{})", goals.join(" ∧ ")));
                    }
                }
                InnerUse::Date { facts, date, .. } => {
                    let f = facts.functions.iter().find(|f| f.name == *date).ok_or_else(|| tr!("日付 `{date}` を呼びます", "the date `{date}`"))?;
                    let ty = format!("{} → ", self.std("Int")).repeat(f.params.len());
                    fields.push(format!("  {field} : {ty}{}", self.std("Int")));
                }
            }
        }
        let mut out = format!("/-- The rules and the dates the module calls, as functions given from outside. -/\nstructure {} where\n{}\n\n", self.inner_ty, fields.join("\n"));
        out.push_str(&format!(
            "/-- What the ports say of them: of a rule, that inputs within their ranges give outputs within theirs. -/\ndef {}.{} ({} : {}) : {} :=\n  {}\n\n",
            self.inner_ty,
            self.ok_name(),
            if oks.is_empty() { "_inner" } else { "inner" },
            self.inner_ty,
            self.std("Prop"),
            if oks.is_empty() { self.std("True") } else { oks.join(" ∧\n  ") }
        ));
        Ok(out)
    }
}

/// The module's own records (true) and enums (false) a type holds, by name.
fn collect_own(t: &Ty, canon: &std::path::Path, out: &mut Vec<(bool, String)>) {
    match t {
        Ty::Record(RecId::Module { file, name, args }) => {
            if file == canon {
                out.push((true, name.clone()));
            }
            for a in args {
                collect_own(a, canon, out);
            }
        }
        Ty::Record(RecId::Variant { file, name, args, .. }) => {
            if file == canon {
                out.push((false, name.clone()));
            }
            for a in args {
                collect_own(a, canon, out);
            }
        }
        Ty::Enum(id) => {
            if matches!(&id.owner, Owner::Module(f) if f == canon) {
                out.push((false, id.name.clone()));
            }
            for a in &id.args {
                collect_own(a, canon, out);
            }
        }
        Ty::List(x) | Ty::Opt(x) | Ty::Set(x) | Ty::Record(RecId::Indexed(x)) => collect_own(x, canon, out),
        Ty::Dict(a, b) | Ty::Result(a, b) | Ty::Record(RecId::Running(a, b) | RecId::Entry(a, b)) => {
            collect_own(a, canon, out);
            collect_own(b, canon, out);
        }
        Ty::Fn(ps, r, _) => {
            for p in ps {
                collect_own(p, canon, out);
            }
            collect_own(r, canon, out);
        }
        _ => {}
    }
}

/// The classes a declaration derives: `Repr`, `DecidableEq` where Lean can derive it of it (where
/// it cannot, a `noncomputable` one is written beside it and `BEq` keeps `=` running), and
/// `Inhabited` where one can be made.
fn classes(eq: bool, ok: bool) -> Vec<&'static str> {
    let mut v = vec!["Repr"];
    if !eq {
        v.push("DecidableEq");
    }
    if ok {
        v.push("Inhabited");
    }
    if eq {
        v.push("BEq");
    }
    v
}

/// Whether a type holds a member of `canon` whose `DecidableEq` is the noncomputable one (the
/// member's own for a record or an enum, its value's fields' for the type of a value).
#[allow(clippy::too_many_arguments)]
fn held_nc(t: &Ty, canon: &std::path::Path, rec_of: &BTreeMap<String, usize>, enum_of: &BTreeMap<String, usize>, enums: &[crate::names::EnumInfo], nr: usize, nc: &[bool], nc_f: &[Vec<bool>]) -> bool {
    match t {
        Ty::Record(RecId::Module { file, name, args }) => {
            (file == canon && rec_of.get(name).is_some_and(|&j| nc[j])) || args.iter().any(|a| held_nc(a, canon, rec_of, enum_of, enums, nr, nc, nc_f))
        }
        Ty::Record(RecId::Variant { file, name, value, args }) => {
            (file == canon
                && enum_of.get(name).is_some_and(|&j| {
                    enums[j - nr].values.iter().position(|v| v.text == *value).is_some_and(|vi| nc_f[j].get(vi).copied().unwrap_or_default())
                }))
                || args.iter().any(|a| held_nc(a, canon, rec_of, enum_of, enums, nr, nc, nc_f))
        }
        Ty::Enum(id) => {
            (matches!(&id.owner, Owner::Module(f) if f == canon) && enum_of.get(&id.name).is_some_and(|&j| nc[j]))
                || id.args.iter().any(|a| held_nc(a, canon, rec_of, enum_of, enums, nr, nc, nc_f))
        }
        Ty::List(x) | Ty::Opt(x) | Ty::Set(x) | Ty::Record(RecId::Indexed(x)) => held_nc(x, canon, rec_of, enum_of, enums, nr, nc, nc_f),
        Ty::Dict(a, b) | Ty::Result(a, b) | Ty::Record(RecId::Running(a, b) | RecId::Entry(a, b)) => {
            held_nc(a, canon, rec_of, enum_of, enums, nr, nc, nc_f) || held_nc(b, canon, rec_of, enum_of, enums, nr, nc, nc_f)
        }
        Ty::Fn(ps, r, _) => ps.iter().any(|p| held_nc(p, canon, rec_of, enum_of, enums, nr, nc, nc_f)) || held_nc(r, canon, rec_of, enum_of, enums, nr, nc, nc_f),
        _ => false,
    }
}

/// Whether a type needs `Inhabited` of a member of `canon` that is not yet written (a record's or
/// an enum's own, or its value's fields'). A list's or an optional's member asks none: `[]` and
/// `none` are made of it whatever its element is.
#[allow(clippy::too_many_arguments)]
fn needs_inh(t: &Ty, canon: &std::path::Path, rec_of: &BTreeMap<String, usize>, enum_of: &BTreeMap<String, usize>, enums: &[crate::names::EnumInfo], nr: usize, inh: &[bool], inh_f: &[Vec<bool>]) -> bool {
    match t {
        Ty::Opt(_) | Ty::List(_) | Ty::Set(_) => false,
        Ty::Record(RecId::Module { file, name, args }) => {
            (file == canon && rec_of.get(name).is_some_and(|&j| !inh[j])) || args.iter().any(|a| needs_inh(a, canon, rec_of, enum_of, enums, nr, inh, inh_f))
        }
        Ty::Record(RecId::Variant { file, name, value, args }) => {
            (file == canon
                && enum_of.get(name).is_some_and(|&j| {
                    enums[j - nr].values.iter().position(|v| v.text == *value).is_none_or(|vi| !inh_f[j].get(vi).copied().unwrap_or_default())
                }))
                || args.iter().any(|a| needs_inh(a, canon, rec_of, enum_of, enums, nr, inh, inh_f))
        }
        Ty::Enum(id) => {
            (matches!(&id.owner, Owner::Module(f) if f == canon) && enum_of.get(&id.name).is_some_and(|&j| !inh[j]))
                || id.args.iter().any(|a| needs_inh(a, canon, rec_of, enum_of, enums, nr, inh, inh_f))
        }
        Ty::Dict(a, b) | Ty::Result(a, b) | Ty::Record(RecId::Running(a, b) | RecId::Entry(a, b)) => {
            needs_inh(a, canon, rec_of, enum_of, enums, nr, inh, inh_f) || needs_inh(b, canon, rec_of, enum_of, enums, nr, inh, inh_f)
        }
        Ty::Record(RecId::Indexed(x)) => needs_inh(x, canon, rec_of, enum_of, enums, nr, inh, inh_f),
        Ty::Fn(ps, r, _) => ps.iter().any(|p| needs_inh(p, canon, rec_of, enum_of, enums, nr, inh, inh_f)) || needs_inh(r, canon, rec_of, enum_of, enums, nr, inh, inh_f),
        _ => false,
    }
}

fn collect_transfers(t: &Ty, out: &mut Vec<crate::ty::TransferId>) {
    match t {
        Ty::Transfer(id) => {
            if !out.contains(id) {
                out.push(id.clone());
            }
        }
        Ty::List(x) | Ty::Opt(x) | Ty::Record(RecId::Indexed(x)) => collect_transfers(x, out),
        Ty::Result(a, b) | Ty::Record(RecId::Running(a, b)) => {
            collect_transfers(a, out);
            collect_transfers(b, out);
        }
        Ty::Record(RecId::Module { args, .. } | RecId::Variant { args, .. }) => {
            for a in args {
                collect_transfers(a, out);
            }
        }
        _ => {}
    }
}

/// The strongly connected parts of a graph, each after the parts it reaches (Tarjan's).
fn sccs(deps: &[Vec<usize>]) -> Vec<Vec<usize>> {
    struct S<'d> {
        deps: &'d [Vec<usize>],
        index: Vec<Option<usize>>,
        low: Vec<usize>,
        on: Vec<bool>,
        stack: Vec<usize>,
        next: usize,
        out: Vec<Vec<usize>>,
    }
    fn visit(s: &mut S, v: usize) {
        s.index[v] = Some(s.next);
        s.low[v] = s.next;
        s.next += 1;
        s.stack.push(v);
        s.on[v] = true;
        for &w in &s.deps[v].clone() {
            match s.index[w] {
                None => {
                    visit(s, w);
                    s.low[v] = s.low[v].min(s.low[w]);
                }
                Some(i) if s.on[w] => s.low[v] = s.low[v].min(i),
                _ => {}
            }
        }
        if Some(s.low[v]) == s.index[v] {
            let mut group = Vec::new();
            while let Some(w) = s.stack.pop() {
                s.on[w] = false;
                group.push(w);
                if w == v {
                    break;
                }
            }
            group.sort();
            s.out.push(group);
        }
    }
    let n = deps.len();
    let mut s = S { deps, index: vec![None; n], low: vec![0; n], on: vec![false; n], stack: Vec::new(), next: 0, out: Vec::new() };
    for v in 0..n {
        if s.index[v].is_none() {
            visit(&mut s, v);
        }
    }
    s.out
}

/// Every name a function of `c` binds: its parameters, its `let` lines, the parameters of its
/// lambdas and the names its patterns give.
fn locals(c: &Checked) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let lets = |b: &Block, out: &mut BTreeSet<String>| {
        for l in &b.lets {
            out.insert(l.name.clone());
        }
    };
    for (f, info) in c.module.fns.iter().zip(&c.decls.fns) {
        for p in &info.params {
            out.insert(p.name.ascii().to_string());
            out.insert(p.name.text.clone());
        }
        lets(&f.body, &mut out);
        let mut es = Vec::new();
        f.body.walk(&mut es);
        for e in es {
            match &e.kind {
                ExprKind::Block(b) => lets(b, &mut out),
                ExprKind::If(_, t, f) => {
                    lets(t, &mut out);
                    lets(f, &mut out);
                }
                ExprKind::Lambda(ps, _) => out.extend(ps.iter().map(|(p, _)| p.clone())),
                ExprKind::Match(_, arms) => {
                    for a in arms {
                        lets(&a.body, &mut out);
                        for p in &a.patterns {
                            match p {
                                Pattern::Some(x, _) => {
                                    out.insert(x.clone());
                                }
                                Pattern::Value(_, Some((b, _)), _) => {
                                    out.insert(b.clone());
                                }
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// The effect of the function `f` of `c`, when it stops: None for one that may not stop (in a ring
/// of calls, or calling one), whose definition Lean would not take without a proof that it ends.
fn stops(c: &Checked, f: usize) -> Option<&crate::effects::Effect> {
    c.analysis.get().and_then(|a| a.fns.get(f)).map(|x| &x.effect).filter(|e| !e.diverges)
}

/// Whether `s` has, outside parentheses, brackets, braces, strings and `«»`, a place where `pred`
/// holds of the rest.
fn at_top(s: &str, pred: impl Fn(&str) -> bool) -> bool {
    let mut depth = 0i32;
    let mut chars = s.char_indices().peekable();
    while let Some((i, ch)) = chars.next() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '"' => {
                while let Some((_, c)) = chars.next() {
                    match c {
                        '\\' => {
                            chars.next();
                        }
                        '"' => break,
                        _ => {}
                    }
                }
            }
            '«' => {
                for (_, c) in chars.by_ref() {
                    if c == '»' {
                        break;
                    }
                }
            }
            _ if depth == 0 && pred(&s[i..]) => return true,
            _ => {}
        }
    }
    false
}

/// What reaches to the end of what follows it, or binds more loosely than `→`.
const LOOSE: [&str; 8] = ["∀", "∃", "let ", "if ", "fun ", "match ", "↔", "→"];

/// A `Prop` as one hypothesis before `→`.
fn hyp(s: String) -> String {
    if at_top(&s, |r| LOOSE.iter().any(|w| r.starts_with(w))) { format!("({s})") } else { s }
}

/// A `Prop` as a side of `∧` or `∨`.
fn side(s: String) -> String {
    if at_top(&s, |r| LOOSE.iter().any(|w| r.starts_with(w)) || r.starts_with(" ∧ ") || r.starts_with(" ∨ ")) { format!("({s})") } else { s }
}

/// A `Prop` after `¬`.
fn negated(s: String) -> String {
    if at_top(&s, |r| r.starts_with(' ')) { format!("¬({s})") } else { format!("¬{s}") }
}

impl Mw<'_> {
    /// Whether a type holds, through the records and enums of the modules, a dict or a set: what no
    /// declaration of Lean is written for.
    fn keyed(&self, t: &Ty) -> bool {
        fn go(w: &Mw, t: &Ty, seen: &mut Vec<(std::path::PathBuf, String)>) -> bool {
            match t {
                Ty::Dict(..) | Ty::EmptyDict | Ty::Set(_) | Ty::Record(RecId::Entry(..)) => true,
                Ty::List(x) | Ty::Opt(x) | Ty::Record(RecId::Indexed(x)) => go(w, x, seen),
                Ty::Result(a, b) | Ty::Record(RecId::Running(a, b)) => go(w, a, seen) || go(w, b, seen),
                Ty::Fn(ps, r, _) => ps.iter().any(|p| go(w, p, seen)) || go(w, r, seen),
                Ty::Record(RecId::Module { file, name, args }) => {
                    if args.iter().any(|a| go(w, a, seen)) {
                        return true;
                    }
                    let key = (file.clone(), name.clone());
                    if seen.contains(&key) {
                        return false;
                    }
                    seen.push(key);
                    let Some(r) = w.p.modules.get(file).and_then(|c| c.decls.records.iter().find(|r| r.name.text == *name)) else { return false };
                    r.fields.iter().any(|f| go(w, &f.slot.ty, seen))
                }
                Ty::Record(RecId::Variant { file, name, args, .. }) => go(w, &Ty::Enum(EnumId { owner: Owner::Module(file.clone()), name: name.clone(), args: args.clone() }), seen),
                Ty::Enum(id) => {
                    if id.args.iter().any(|a| go(w, a, seen)) {
                        return true;
                    }
                    let Owner::Module(file) = &id.owner else { return false };
                    let key = (file.clone(), id.name.clone());
                    if seen.contains(&key) {
                        return false;
                    }
                    seen.push(key);
                    let Some(e) = w.p.modules.get(file).and_then(|c| c.decls.enums.iter().find(|e| e.name.text == id.name)) else { return false };
                    e.fields.iter().flatten().any(|f| go(w, &f.slot.ty, seen))
                }
                _ => false,
            }
        }
        go(self, t, &mut Vec::new())
    }

    /// Whether the function `f` of the module takes the rules and dates: it calls one, itself or
    /// through the functions it calls.
    fn takes_inner(&self, f: usize) -> bool {
        let (rules, dates) = crate::properties::inner_calls(self.p, self.c, f);
        !rules.is_empty() || !dates.is_empty()
    }

    /// The first `n` functions of the module, each after the functions of the module it calls.
    fn fn_order(&self, n: usize) -> Vec<usize> {
        let c = self.c;
        let mut deps: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (f, d) in deps.iter_mut().enumerate() {
            let mut es = Vec::new();
            c.module.fns[f].body.walk(&mut es);
            for e in es {
                let ExprKind::Call(callee, _) = &e.kind else { continue };
                let Some(Ref::Fn(file, name)) = c.typed.refs.get(callee.id) else { continue };
                if *file != c.canon {
                    continue;
                }
                if let Some(g) = crate::interp::function(c, name)
                    && g < n
                    && !d.contains(&g)
                {
                    d.push(g);
                }
            }
        }
        sccs(&deps).into_iter().flatten().collect()
    }

    /// The name the rules and dates go by in a function: `inner`, with `'` until it is neither one
    /// of the parameters' Lean names nor one of the names the body can reach.
    fn inner_name(params: &[String], outer: &BTreeSet<String>) -> String {
        let mut n = "inner".to_string();
        while params.iter().any(|p| p == &n) || outer.contains(&n) {
            n.push('\'');
        }
        n
    }

    /// Each parameter bound in `fx` by its name (and its alias): the name Lean writes, its type, and
    /// where it is bound in the environment. Its binder is written once the body is
    /// (`Fx::param_binder`).
    fn params(&self, fx: &mut Fx, params: &[crate::names::ParamInfo]) -> Res<Vec<(String, String, Vec<usize>)>> {
        let mut out = Vec::new();
        for p in params {
            let n = fx.local(p.name.ascii());
            let tn = self.tname(&p.slot.ty)?;
            let mut at = vec![fx.bind(&p.name.text, T::held(n.clone(), &p.slot.ty))];
            if let Some((a, _)) = &p.name.alias {
                at.push(fx.bind(a, T::held(n.clone(), &p.slot.ty)));
            }
            out.push((n, tn, at));
        }
        Ok(out)
    }

    /// The function `f` of the module as a Lean definition: of every function that stops, Lean's
    /// answers are kaname's wherever kaname's function answers (Lean's numbers do not overflow, and
    /// what fails in kaname comes to some value in Lean), which is everywhere a property, total, calls
    /// it (DESIGN 38.4).
    fn function(&self, f: usize, takes: bool) -> Res<String> {
        let c = self.c;
        let info = &c.decls.fns[f];
        let decl = &c.module.fns[f];
        if stops(c, f).is_none() {
            return Err(tr!("止まらないことがありえます", "it may not stop"));
        }
        if let Some(o) = fn_obstacle(c, f) {
            return Err(tr!("{} を使います", "it uses {}", o.what.ja; o.what.en));
        }
        let mut fx = Fx {
            w: self,
            m: c,
            env: Vec::new(),
            inner: None,
            outer: info.tparams.iter().map(|p| ident(&p.name.text)).chain(self.outer.iter().cloned()).collect(),
        };
        let ps = self.params(&mut fx, &info.params)?;
        let inner = Self::inner_name(&ps.iter().map(|p| p.0.clone()).collect::<Vec<_>>(), &fx.outer);
        fx.outer.insert(inner.clone());
        fx.inner = takes.then(|| inner.clone());
        let mut binders = if takes { vec![format!("({inner} : {})", self.inner_ty)] } else { Vec::new() };
        let ret = self.tname(&info.ret.ty)?;
        let body = fx.block(&decl.body, &info.ret.ty)?;
        binders.extend(ps.iter().map(|p| fx.param_binder(p, false, &body)));
        let head = [ident(info.name.ascii()) + &self.fn_tbinders(&info.tparams)].into_iter().chain(binders).collect::<Vec<_>>().join(" ");
        Ok(format!("def {head} : {ret} :=\n  {body}\n"))
    }

    /// The statement of a property (DESIGN 38.4): `abbrev <name> : Prop := ∀ …, ranges → when → body`.
    fn statement(&self, cp: &crate::check::CheckedProperty) -> Res<String> {
        let c = self.c;
        let f = cp.lowered;
        let info = &c.decls.fns[f];
        let decl = &c.module.fns[f];
        let takes = self.takes_inner(f);
        // a property has no type parameters (its statement binds none)
        let mut fx = Fx { w: self, m: c, env: Vec::new(), inner: None, outer: self.outer.clone() };
        let ps = self.params(&mut fx, &info.params)?;
        let inner = Self::inner_name(&ps.iter().map(|p| p.0.clone()).collect::<Vec<_>>(), &fx.outer);
        fx.outer.insert(inner.clone());
        fx.inner = takes.then(|| inner.clone());
        let mut lines: Vec<String> = Vec::new();
        // a parameter with a hypothesis of its range is named there, whatever the body reads
        let mut held = Vec::new();
        for (p, param) in info.params.iter().zip(&ps) {
            let hs = self.within(&param.0, &p.slot, 0)?;
            held.push(!hs.is_empty());
            if !hs.is_empty() {
                lines.push(hs.into_iter().map(hyp).collect::<Vec<_>>().join(" → "));
            }
        }
        let made = &cp.decl.made;
        let body: &Block = match &decl.body.answer.kind {
            ExprKind::If(cond, then, _) if made.contains(&decl.body.answer.id) => {
                let mut ws = Vec::new();
                when_lines(cond, made, &mut ws);
                for w in ws {
                    lines.push(hyp(fx.prop(w)?));
                }
                then
            }
            _ => &decl.body,
        };
        let claim = fx.prop_block(body)?;
        let text = format!("{} {claim}", lines.join(" "));
        let binders: Vec<String> = ps.iter().zip(&held).map(|(p, named)| fx.param_binder(p, *named, &text)).collect();
        let mut out = format!("abbrev {} : {} :=\n  ", ident(info.name.ascii()), self.std("Prop"));
        if takes {
            out.push_str(&format!("∀ ({inner} : {}), {inner}.{} →\n  ", self.inner_ty, self.ok_name()));
        }
        if !binders.is_empty() {
            out.push_str(&format!("∀ {},\n    ", binders.join(" ")));
        }
        for l in &lines {
            out.push_str(&format!("{l} →\n    "));
        }
        out.push_str(&claim);
        out.push('\n');
        Ok(out)
    }

    /// The module as Lean, with the names of the properties whose statements are written.
    fn text(&self, own: usize, cps: &[crate::check::CheckedProperty], origin: &Origin) -> (String, Vec<String>) {
        let c = self.c;
        let mut fns = String::new();
        for f in self.fn_order(own) {
            let info = &c.decls.fns[f];
            let takes = self.takes_inner(f);
            match self.function(f, takes) {
                Ok(text) => {
                    fns.push_str(&text);
                    fns.push('\n');
                    self.done.fns.borrow_mut().insert((c.canon.clone(), f), (ident(info.name.ascii()), takes));
                }
                Err(why) => {
                    fns.push_str(&format!("-- `{}` is not written for Lean: {}.\n\n", info.name.text, why.en));
                    self.skipped.borrow_mut().push((f, why));
                }
            }
        }
        let mut stmts = String::new();
        let mut stated = Vec::new();
        for cp in cps {
            let name = &cp.decl.name;
            match self.statement(cp) {
                Ok(text) => {
                    stmts.push_str(&text);
                    stmts.push('\n');
                    stated.push(ident(name.ascii()));
                }
                Err(why) => {
                    stmts.push_str(&format!("-- The property `{}` is not written for Lean: {}.\n\n", name.text, why.en));
                    self.skipped_props.borrow_mut().push((cp.lowered, why));
                }
            }
        }
        let mut out = head(c, origin);
        out.push_str("import Kaname.Prelude\n");
        if !self.done.rules.borrow().is_empty() {
            out.push_str("import Kaname.Rules\n");
        }
        if !self.done.books.borrow().is_empty() {
            out.push_str("import Kaname.Books\n");
        }
        let mut imported = Vec::new();
        for t in &c.uses {
            if let Target::Module(m) = t {
                let a = m.module.name.ascii().to_string();
                if !imported.contains(&a) {
                    out.push_str(&format!("import Kaname.{a}\n"));
                    imported.push(a);
                }
            }
        }
        out.push_str(&format!("\nnamespace Kaname.{}\n\n", self.alias));
        match self.types() {
            Ok(t) => out.push_str(&t),
            Err(why) => out.push_str(&format!("-- Not written for Lean: {}.\n\n", why.en)),
        }
        match self.inner_decl() {
            Ok(t) => out.push_str(&t),
            Err(why) => out.push_str(&format!("-- `Inner` is not written for Lean: {}.\n\n", why.en)),
        }
        out.push_str(&fns);
        out.push_str(&stmts);
        out.push_str(&format!("end Kaname.{}\n", self.alias));
        (out, stated)
    }
}

/// The modules of a package and those they read, each once, each after those it reads: as the
/// stages after the check have them, and with the properties read as functions.
fn in_order(modules: &[Rc<Checked>]) -> Vec<Rc<Checked>> {
    fn visit(c: &Rc<Checked>, seen: &mut BTreeSet<std::path::PathBuf>, out: &mut Vec<Rc<Checked>>) {
        if !seen.insert(c.canon.clone()) {
            return;
        }
        for t in &c.uses {
            if let Target::Module(m) = t {
                visit(m, seen, out);
            }
        }
        out.push(c.clone());
    }
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for c in modules {
        visit(c, &mut seen, &mut out);
    }
    out
}

/// The head of a module's file and of its draft (ritsu's DESIGN 9.2): the line that says it is
/// generated, and the module it is made from, as `origin` names it. In English, as the comments of the
/// Lean are.
fn head(c: &Checked, origin: &Origin) -> String {
    let version = c.module.version.trim_start_matches('v');
    let source = origin.source("module", &c.module.name.text, version).line();
    let mut out = Comment::Dashes.line(&ritsu_emit::header::generated("kaname"));
    out.push_str(&Comment::Dashes.line(source.get(Lang::En)));
    out
}

/// What the head of a module's file names it by when the module is not given with its path from a
/// project's root: its file's name, as a language's own command names it.
pub fn origin(c: &Checked) -> Origin {
    Origin::named(&c.path, ritsu_base::fed::without(&c.src).as_bytes())
}

/// The draft of a module's proofs (DESIGN 38.5): each written property's theorem, with `sorry`. It
/// opens `Kaname`, whose lemmas a proof uses, besides the module, as the examples' proofs do.
fn sample(c: &Checked, stated: &[String], origin: &Origin) -> String {
    let alias = c.module.name.ascii();
    let kn = ritsu_emit::header::file_name(&c.path);
    let mut out = head(c, origin);
    out.push_str(&format!(
        "-- A draft of the proofs: copy it beside {} as {alias}.lean, without these three lines, and write a proof in place of each `sorry`.\nimport Kaname.{alias}\n\nnamespace Proofs.{alias}\nopen Kaname Kaname.{alias}\n\n",
        ritsu_emit::header::one_line(&kn)
    ));
    for p in stated {
        out.push_str(&format!("theorem {p} : Kaname.{alias}.{p} := by\n  sorry\n\n"));
    }
    out.push_str(&format!("end Proofs.{alias}\n"));
    out
}

/// `p` as an absolute path with no `.` and no `..`, as the disk has it: the longest part of it that
/// is there resolved (its links followed), the rest added as written. A path relative to the working
/// directory is read from it.
fn plain(p: &std::path::Path) -> PathBuf {
    use std::path::Component;
    let abs = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(p) };
    let mut lexical = PathBuf::new();
    for c in abs.components() {
        match c {
            Component::ParentDir => {
                lexical.pop();
            }
            Component::CurDir => {}
            c => lexical.push(c.as_os_str()),
        }
    }
    let mut rest = Vec::new();
    let mut there = lexical.clone();
    loop {
        if let Ok(real) = std::fs::canonicalize(&there) {
            let mut out = real;
            for part in rest.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match (there.file_name().map(|f| f.to_os_string()), there.parent().map(|d| d.to_path_buf())) {
            (Some(f), Some(d)) => {
                rest.push(f);
                there = d;
            }
            _ => return lexical,
        }
    }
}

/// The path from the directory `from` to `to`, each as the disk has it ([`plain`]).
fn relative(from: &std::path::Path, to: &std::path::Path) -> String {
    use std::path::Component;
    let (from, to) = (plain(from), plain(to));
    let a: Vec<Component> = from.components().collect();
    let b: Vec<Component> = to.components().collect();
    let same = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<String> = Vec::new();
    for _ in same..a.len() {
        parts.push("..".into());
    }
    for c in &b[same..] {
        parts.push(c.as_os_str().to_string_lossy().to_string());
    }
    if parts.is_empty() { ".".into() } else { parts.join("/") }
}

fn toml_str(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The package Lean reads (DESIGN 38.4): `lean-toolchain`, `lakefile.toml`, `.gitignore`, the
/// prelude, each module of `modules` and of those they read, and the draft of each module's proofs;
/// each file by its path in the package. And how many of the expressions written are held as a
/// count of a step finer than their unit's, and as a fraction.
pub fn package(modules: &[Rc<Checked>], layout: &Layout) -> (Vec<(PathBuf, String)>, LeanStats) {
    let (files, stats) = package_from(&modules.iter().map(|c| (c.clone(), origin(c))).collect::<Vec<_>>(), layout);
    (files.into_iter().map(|(p, t, _)| (p, t)).collect(), stats)
}

/// [`package`], each module with what the head of its file names (`ritsu gen`: its path from the
/// project's root), and each file with the path of the module it is made from, for a module's file
/// and its draft. A module read by one of them and not given is named by its file's name.
pub fn package_from(modules: &[(Rc<Checked>, Origin)], layout: &Layout) -> (Vec<(PathBuf, String, Option<String>)>, LeanStats) {
    let origins: BTreeMap<PathBuf, Origin> = modules.iter().map(|(c, o)| (c.canon.clone(), o.clone())).collect();
    let checked: Vec<Rc<Checked>> = modules.iter().map(|(c, _)| c.clone()).collect();
    let w = write(&checked, &origins, layout);
    (w.files, w.stats)
}

/// The places among a module's properties of those whose statements are written, by the module's
/// file on the disk.
pub type Stated = BTreeMap<PathBuf, Vec<usize>>;

/// The package `--lean` builds (DESIGN 38.6, step 2): [`package`] laid out as `Layout::Temp`, and
/// what of each module's properties it states.
pub fn package_stating(modules: &[Rc<Checked>]) -> (Vec<(PathBuf, String)>, Stated, Vec<(PathBuf, usize, Text)>) {
    let origins: BTreeMap<PathBuf, Origin> = modules.iter().map(|c| (c.canon.clone(), origin(c))).collect();
    let w = write(modules, &origins, &Layout::Temp);
    (w.files.into_iter().map(|(p, t, _)| (p, t)).collect(), w.stated, w.skipped_props)
}

/// The proof files beside the modules (DESIGN 38.5), by their modules' aliases: those that are there.
pub fn proofs_beside(modules: &[Rc<Checked>]) -> Vec<(String, PathBuf)> {
    modules
        .iter()
        .filter(|c| !c.properties.is_empty())
        .map(|c| (c.module.name.ascii().to_string(), crate::properties::proof_path(c)))
        .filter(|(_, p)| ritsu_base::fs::is_file(p))
        .collect()
}

// ── The comparison with the reference interpreter ───────────────────────────

/// A string as JSON writes it, as the comparison's files write it too: `"` and `\` escaped, the
/// characters below U+0020 as `\u00xx`, the rest as they are.
fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The functions the comparison's files print values with (in each file's namespace).
const JSON_HELPERS: &str = r#"-- The names of Lean's own types are written from the root, as the module may have types of the same names.
def int (x : _root_.Int) : _root_.String := ToString.toString x

def frac (x : _root_.Kaname.Frac) : _root_.String :=
  let g : _root_.Int := _root_.Int.ofNat (_root_.Int.gcd x.num x.den)
  ToString.toString (x.num / g) ++ "/" ++ ToString.toString (x.den / g)

def bool (b : _root_.Bool) : _root_.String := if b then "true" else "false"

def hex4 (n : _root_.Nat) : _root_.String :=
  let s := _root_.String.ofList (_root_.Nat.toDigits 16 n)
  "".pushn '0' (4 - s.length) ++ s

def esc (c : _root_.Char) : _root_.String :=
  if c = '"' then "\\\"" else if c = '\\' then "\\\\" else if c.toNat < 32 then "\\u" ++ hex4 c.toNat else c.toString

def str (s : _root_.String) : _root_.String := "\"" ++ _root_.String.join (s.toList.map esc) ++ "\""

def list {α : Type} (f : α → _root_.String) (xs : _root_.List α) : _root_.String := "[" ++ ",".intercalate (xs.map f) ++ "]"

def opt {α : Type} (f : α → _root_.String) : _root_.Option α → _root_.String
  | .none => "null"
  | .some x => f x

def result {ε α : Type} (fe : ε → _root_.String) (fa : α → _root_.String) : _root_.Except ε α → _root_.String
  | .ok a => "{\"ok\":" ++ fa a ++ "}"
  | .error e => "{\"err\":" ++ fe e ++ "}"
"#;

/// A number as a subscript, for the names the comparison's files make.
fn sub(n: usize) -> String {
    n.to_string().chars().map(|c| char::from_u32(0x2080 + c.to_digit(10).unwrap_or(0)).unwrap_or('_')).collect()
}

impl Mw<'_> {
    /// A value of kaname's as a Lean term of `t`, held as a place of `t` holds it.
    fn value(&self, v: &Value, t: &Ty) -> Res<String> {
        Ok(match (v, t) {
            (Value::None, Ty::Opt(_)) => format!("({}.none : {})", self.std("Option"), self.tname(t)?),
            (v, Ty::Opt(x)) => format!("{}.some {}", self.std("Option"), par(&self.value(v, x)?)),
            (Value::Num(r), Ty::Num(_)) => match held_as(t) {
                Some(Num::Int { grain, .. }) => {
                    let c = r.div(grain);
                    if !c.is_int() {
                        return Err(tr!("{}/{} は刻みの整数倍ではありません", "{}/{} is no whole number of its step", r.num, r.den));
                    }
                    int(c.num)
                }
                _ => format!("{} {} {}", self.kaname("Frac.mk"), par(&int(r.num)), r.den),
            },
            (Value::Bool(b), _) => b.to_string(),
            (Value::Str(s), _) => str_lit(s),
            (Value::Date(d), _) => int(*d as i128),
            (Value::Enum(name), Ty::Enum(id)) => {
                let code = self.enum_value(id, name)?;
                if id.args.is_empty() { code } else { format!("({code} : {})", self.tname(t)?) }
            }
            (Value::Variant(tag, fs), Ty::Result(a, e)) => {
                let (ctor, held) = if tag == "ok" { (format!("{}.ok", self.std("Except")), a) } else { (format!("{}.error", self.std("Except")), e) };
                let x = fs.first().map(|(_, x)| x).ok_or_else(|| tr!("値を持たない result があります", "a result with no value"))?;
                format!("({ctor} {} : {})", par(&self.value(x, held)?), self.tname(t)?)
            }
            (Value::Variant(tag, fs), Ty::Enum(id)) => {
                let ctor = self.enum_value(id, tag)?;
                let rid = RecId::variant(id, tag.clone()).ok_or_else(|| tr!("規則の列挙のフィールドを持つ値を使います", "a value with fields of a rule's enum"))?;
                let rec = self.value(&Value::Record(fs.clone()), &Ty::Record(rid))?;
                format!("({ctor} {rec} : {})", self.tname(t)?)
            }
            (Value::Variant(_, fs), Ty::Record(id @ RecId::Variant { .. })) => self.value(&Value::Record(fs.clone()), &Ty::Record(id.clone()))?,
            (Value::Record(fs), Ty::Record(id)) => {
                let mut parts = Vec::new();
                for (n, slot) in self.p.fields(id) {
                    let x = fs.iter().find(|(k, _)| *k == n.text).map(|(_, x)| x).unwrap_or(&Value::None);
                    let lean = match id {
                        RecId::Running(..) | RecId::Indexed(_) => n.text.clone(),
                        _ => field_name(n.ascii()),
                    };
                    parts.push(format!("{lean} := {}", self.value(x, &slot.ty)?));
                }
                format!("({{ {} }} : {})", parts.join(", "), self.tname(t)?)
            }
            (Value::List(xs), Ty::List(x)) => {
                if xs.is_empty() {
                    format!("([] : {})", self.tname(t)?)
                } else {
                    let parts = xs.iter().map(|v| self.value(v, x)).collect::<Res<Vec<_>>>()?;
                    format!("[{}]", parts.join(", "))
                }
            }
            (Value::Transfer(tv), Ty::Transfer(id)) => {
                let (_, params) = self.p.transfer(id).ok_or_else(|| tr!("口が説明しない振替", "a transfer the port does not describe"))?;
                let mut parts = Vec::new();
                for (n, pt) in &params {
                    let x = tv.args.iter().find(|(k, _)| k == n).map(|(_, x)| x).unwrap_or(&Value::None);
                    parts.push(format!("{} := {}", field_name(n), self.value(x, &Ty::Opt(Box::new(pt.clone())))?));
                }
                format!("({{ {} }} : {})", parts.join(", "), self.tname(t)?)
            }
            _ => return Err(tr!("Lean の持たない値があります", "a value Lean is not given")),
        })
    }

    /// The JSON text the comparison expects of a value of `t`: a number as the whole number of its
    /// step (as `num/den` in lowest terms where it is held as a fraction), a date as its day number, a
    /// value of an enum by its alias, a record's fields by their aliases, a value with fields as
    /// `{"<value>": {fields}}`, a result as `{"ok": …}` or `{"err": …}`.
    fn canon(&self, v: &Value, t: &Ty) -> Res<String> {
        Ok(match (v, t) {
            (Value::None, Ty::Opt(_)) => "null".into(),
            (v, Ty::Opt(x)) => self.canon(v, x)?,
            (Value::Num(r), Ty::Num(_)) => match held_as(t) {
                Some(Num::Int { grain, .. }) => {
                    let c = r.div(grain);
                    if c.is_int() { c.num.to_string() } else { format!("{}/{}", c.num, c.den) }
                }
                _ => format!("{}/{}", r.num, r.den),
            },
            (Value::Bool(b), _) => b.to_string(),
            (Value::Str(s), _) => json_str(s),
            (Value::Date(d), _) => d.to_string(),
            (Value::Enum(name), Ty::Enum(id)) => json_str(self.value_alias(id, name)?.as_str()),
            (Value::Variant(tag, fs), Ty::Result(a, e)) => {
                let (key, held) = if tag == "ok" { ("ok", a) } else { ("err", e) };
                let x = fs.first().map(|(_, x)| x).ok_or_else(|| tr!("値を持たない result があります", "a result with no value"))?;
                format!("{{\"{key}\":{}}}", self.canon(x, held)?)
            }
            (Value::Variant(tag, fs), Ty::Enum(id)) => {
                let rid = RecId::variant(id, tag.clone()).ok_or_else(|| tr!("規則の列挙のフィールドを持つ値を使います", "a value with fields of a rule's enum"))?;
                format!("{{{}:{}}}", json_str(&self.value_alias(id, tag)?), self.canon(&Value::Record(fs.clone()), &Ty::Record(rid))?)
            }
            (Value::Variant(_, fs), Ty::Record(id @ RecId::Variant { .. })) => self.canon(&Value::Record(fs.clone()), &Ty::Record(id.clone()))?,
            (Value::Record(fs), Ty::Record(id)) => {
                let mut parts = Vec::new();
                for (n, slot) in self.p.fields(id) {
                    let x = fs.iter().find(|(k, _)| *k == n.text).map(|(_, x)| x).unwrap_or(&Value::None);
                    parts.push(format!("{}:{}", json_str(n.ascii()), self.canon(x, &slot.ty)?));
                }
                format!("{{{}}}", parts.join(","))
            }
            (Value::List(xs), Ty::List(x)) => format!("[{}]", xs.iter().map(|v| self.canon(v, x)).collect::<Res<Vec<_>>>()?.join(",")),
            (Value::Transfer(tv), Ty::Transfer(id)) => {
                let (_, params) = self.p.transfer(id).ok_or_else(|| tr!("口が説明しない振替", "a transfer the port does not describe"))?;
                let mut parts = Vec::new();
                for (n, pt) in &params {
                    let x = tv.args.iter().find(|(k, _)| k == n).map(|(_, x)| x).unwrap_or(&Value::None);
                    parts.push(format!("{}:{}", json_str(n), self.canon(x, &Ty::Opt(Box::new(pt.clone())))?));
                }
                format!("{{{}}}", parts.join(","))
            }
            _ => return Err(tr!("Lean の持たない値があります", "a value Lean is not given")),
        })
    }

    /// The alias of a value of an enum, by the name it is declared with.
    fn value_alias(&self, id: &EnumId, name: &str) -> Res<String> {
        let (values, _) = self.p.values(id);
        values.iter().find(|v| v.is(name)).map(|v| v.ascii().to_string()).ok_or_else(|| tr!("値 `{name}`", "the value `{name}`"))
    }
}

/// The functions that print the values of the types a comparison's file meets, each a `partial def`
/// of one mutual block, named by the type they print.
struct Printers<'w, 'a> {
    w: &'w Mw<'a>,
    named: std::cell::RefCell<Vec<(Ty, String)>>,
    defs: std::cell::RefCell<Vec<String>>,
}

impl Printers<'_, '_> {
    /// A Lean function from `t` to its JSON text.
    fn of(&self, t: &Ty) -> Res<String> {
        Ok(match t {
            Ty::Num(_) => match held_as(t) {
                Some(Num::Int { .. }) => "int".into(),
                _ => "frac".into(),
            },
            Ty::Date => "int".into(),
            Ty::Bool => "bool".into(),
            Ty::Str => "str".into(),
            Ty::List(x) => format!("(list {})", self.of(x)?),
            Ty::Opt(x) => format!("(opt {})", self.of(x)?),
            Ty::Result(a, e) => format!("(result {} {})", self.of(e)?, self.of(a)?),
            Ty::Enum(_) | Ty::Record(_) | Ty::Transfer(_) => self.named(t)?,
            _ => return Err(tr!("Lean の持たない値があります", "a value Lean is not given")),
        })
    }

    fn named(&self, t: &Ty) -> Res<String> {
        if let Some((_, n)) = self.named.borrow().iter().find(|(x, _)| x == t) {
            return Ok(n.clone());
        }
        let name = format!("jv{}", sub(self.named.borrow().len() + 1));
        self.named.borrow_mut().push((t.clone(), name.clone()));
        let w = self.w;
        let tn = w.tname(t)?;
        let field = |key: &str, lean: &str, ft: &Ty| -> Res<String> { Ok(format!("{} ++ {} (x.{lean})", str_lit(&format!("{}:", json_str(key))), self.of(ft)?)) };
        let body = match t {
            Ty::Enum(id) => {
                let (values, _) = w.p.values(id);
                let mut arms = Vec::new();
                for v in &values {
                    let ctor = w.enum_value(id, &v.text)?;
                    let fields = w.p.enum_info(id).and_then(|e| e.fields_of(&v.text)).is_some_and(|fs| !fs.is_empty());
                    if fields {
                        let rid = RecId::variant(id, v.text.clone()).ok_or_else(|| tr!("規則の列挙のフィールドを持つ値を使います", "a value with fields of a rule's enum"))?;
                        let inner = self.of(&Ty::Record(rid))?;
                        arms.push(format!("  | {ctor} y => {} ++ {inner} y ++ \"}}\"", str_lit(&format!("{{{}:", json_str(v.ascii())))));
                    } else {
                        arms.push(format!("  | {ctor} => {}", str_lit(&json_str(v.ascii()))));
                    }
                }
                format!("partial def {name} : {tn} → _root_.String\n{}\n", arms.join("\n"))
            }
            Ty::Record(id) => {
                let mut parts = Vec::new();
                for (n, slot) in w.p.fields(id) {
                    let lean = match id {
                        RecId::Running(..) | RecId::Indexed(_) => n.text.clone(),
                        _ => field_name(n.ascii()),
                    };
                    parts.push(field(n.ascii(), &lean, &slot.ty)?);
                }
                let inside = if parts.is_empty() { "\"\"".to_string() } else { parts.join(" ++ \",\" ++ ") };
                format!("partial def {name} (x : {tn}) : _root_.String := \"{{\" ++ {inside} ++ \"}}\"\n")
            }
            Ty::Transfer(id) => {
                let (_, params) = w.p.transfer(id).ok_or_else(|| tr!("口が説明しない振替", "a transfer the port does not describe"))?;
                let mut parts = Vec::new();
                for (n, pt) in &params {
                    parts.push(field(n, &field_name(n), &Ty::Opt(Box::new(pt.clone())))?);
                }
                let inside = if parts.is_empty() { "\"\"".to_string() } else { parts.join(" ++ \",\" ++ ") };
                format!("partial def {name} (x : {tn}) : _root_.String := \"{{\" ++ {inside} ++ \"}}\"\n")
            }
            _ => return Err(tr!("Lean の持たない値があります", "a value Lean is not given")),
        };
        self.defs.borrow_mut().push(body);
        Ok(name)
    }
}

/// What writing a package found: its files, what it held numbers as, the functions written, the
/// rules and dates each module's `Inner` holds, and why each function that is not written is not.
struct Written {
    /// Each file, with the path the head of the module it is made from names.
    files: Vec<(PathBuf, String, Option<String>)>,
    stats: LeanStats,
    done: Done,
    uses: BTreeMap<PathBuf, Vec<InnerUse>>,
    skipped: Vec<(PathBuf, usize, Text)>,
    /// The properties whose statements are not written, by the module's file and the function's
    /// place, and why.
    skipped_props: Vec<(PathBuf, usize, Text)>,
    /// The places among each module's properties of those whose statements are written, by the
    /// module's file on the disk.
    stated: BTreeMap<PathBuf, Vec<usize>>,
}

/// A file of the comparison: the module's alias, the file's path in the package and its text, and
/// for each line it prints, the function it evaluates and the line the reference interpreter expects.
pub type CompareFile = (String, PathBuf, String, Vec<(String, String)>);

/// One function the comparison runs, and how many of its vectors.
#[derive(Clone, Debug)]
pub struct Compared {
    pub module: String,
    pub function: String,
    pub evaluations: usize,
}

/// The comparison of the Lean with the reference interpreter (DESIGN 38.4, PLAN 12.2.6).
#[derive(Clone, Debug, Default)]
pub struct Comparison {
    /// The package, laid out as `Layout::Temp`.
    pub package: Vec<(PathBuf, String)>,
    /// `Compare/<alias>.lean` for each module with a function compared, by the module's alias: the
    /// file, which prints one line for each evaluation, and the line the reference expects of each,
    /// with the function it is of.
    pub files: Vec<CompareFile>,
    pub compared: Vec<Compared>,
    /// The total functions with nothing in them that is not written for Lean that the writer did not
    /// write, and why: what the comparison has to be told of.
    pub unwritten: Vec<String>,
    /// The vectors whose evaluation the comparison could not write, and why.
    pub left: Vec<String>,
    /// The vectors of functions that are not total that the reference interpreter fails on.
    pub failed: usize,
    pub stats: LeanStats,
}

/// The package and what writing it found.
fn write(modules: &[Rc<Checked>], origins: &BTreeMap<PathBuf, Origin>, layout: &Layout) -> Written {
    let done = Done::default();
    let mut stats = LeanStats::default();
    let mut files: Vec<(PathBuf, String, Option<String>)> = Vec::new();
    let mut samples: Vec<(PathBuf, String, Option<String>)> = Vec::new();
    let mut uses = BTreeMap::new();
    let mut skipped = Vec::new();
    let mut skipped_props = Vec::new();
    let mut stated_by = BTreeMap::new();
    let ordered = in_order(modules);
    // the rules' types and the transfers', each once, in files every module reads
    let mut rules_text = String::new();
    let mut books_text = String::new();
    {
        let mut taken: BTreeSet<String> = BTreeSet::new();
        for c in &ordered {
            for (canon, facts) in &c.world.rules {
                if done.rules.borrow().contains_key(canon) {
                    continue;
                }
                let mut ns = ident(&facts.alias);
                let mut k = 2;
                while taken.contains(&ns) {
                    ns = ident(&format!("{}_{k}", facts.alias));
                    k += 1;
                }
                taken.insert(ns.clone());
                done.rules.borrow_mut().insert(canon.clone(), format!("Kaname.Rules.{ns}"));
            }
            let full = c.lowered.clone().unwrap_or_else(|| c.clone());
            for t in &full.typed.types {
                let mut ids = done.books.borrow_mut();
                collect_transfers(t, &mut ids);
            }
        }
        // each enum of the rules is one inductive (DESIGN 40), held by the file that declares it when
        // a module reads that file as a rule, else by the first rule that reads it
        {
            let rules = done.rules.borrow();
            let mut one = done.rule_enums.borrow_mut();
            for declaring in [true, false] {
                for c in &ordered {
                    for (canon, facts) in &c.world.rules {
                        for e in &facts.enums {
                            let key = rule_enum_key(canon, e);
                            if one.contains_key(&key) || (declaring && key.0 != crate::names::canonical(canon)) {
                                continue;
                            }
                            if let Some(ns) = rules.get(canon) {
                                let taken = |a: &str| {
                                    let n = format!("{ns}.{}", ident(a));
                                    n == format!("{ns}.Outputs")
                                        || facts.elements.iter().any(|(_, el, _)| n == format!("{ns}.{}", ident(el)))
                                        || facts.enums.iter().any(|x| x.name != e.name && n == format!("{ns}.{}", ident(&x.alias)))
                                        || one.values().any(|o| *o == n)
                                };
                                let mut a = e.alias.clone();
                                while taken(&a) {
                                    a.push('\'');
                                }
                                one.insert(key, format!("{ns}.{}", ident(&a)));
                            }
                        }
                    }
                }
            }
        }
        // each element of a rule is one structure, under the rule's namespace: `Outputs` and the
        // enums' and the other elements' names are not its name
        {
            let rules = done.rules.borrow();
            let mut elems = done.rule_elements.borrow_mut();
            for c in &ordered {
                for (canon, facts) in &c.world.rules {
                    let Some(ns) = rules.get(canon) else { continue };
                    let mut here: BTreeSet<String> = elems.values().filter(|o| o.starts_with(&format!("{ns}."))).cloned().collect();
                    for (seq, alias, _) in &facts.elements {
                        let mut a = alias.clone();
                        loop {
                            let n = format!("{ns}.{}", ident(&a));
                            if !(n == format!("{ns}.Outputs")
                                || facts.enums.iter().any(|e| n == format!("{ns}.{}", ident(&e.alias)))
                                || done.rule_enums.borrow().values().any(|o| *o == n)
                                || here.contains(&n))
                            {
                                break;
                            }
                            a.push('\'');
                        }
                        let n = format!("{ns}.{}", ident(&a));
                        here.insert(n.clone());
                        elems.insert((canon.clone(), seq.clone()), n);
                    }
                }
            }
        }
        let mut written: BTreeSet<String> = BTreeSet::new();
        for c in &ordered {
            let full = c.lowered.clone().unwrap_or_else(|| c.clone());
            let p = Program::of(&full);
            let w = Mw::new(&p, &full, &done);
            for (canon, facts) in &c.world.rules {
                if rules_text.contains(&format!("structure {}.Outputs where", w.rule_ns_raw(canon).unwrap_or_default())) {
                    continue;
                }
                match w.rule_types(canon, facts, &mut written) {
                    Ok(t) => rules_text.push_str(&t),
                    Err(why) => rules_text.push_str(&format!("-- The types of the rule `{}` are not written for Lean: {}.\n\n", facts.alias, why.en)),
                }
            }
            let ids: Vec<crate::ty::TransferId> = done.books.borrow().iter().filter(|id| p.books.contains_key(&id.book) && !books_text.contains(&format!("structure {} where", w.transfer_decl_name(id).unwrap_or_default()))).cloned().collect();
            match w.transfer_types(&ids) {
                Ok(t) => books_text.push_str(&t),
                Err(why) => books_text.push_str(&format!("-- Not written for Lean: {}.\n\n", why.en)),
            }
        }
    }
    if !done.rules.borrow().is_empty() {
        files.push((PathBuf::from("Kaname/Rules.lean"), format!("-- {}\n-- The types of the rules the modules call.\nimport Kaname.Prelude\n\n{rules_text}", ritsu_emit::header::generated("kaname")), None));
    }
    if !done.books.borrow().is_empty() {
        files.push((PathBuf::from("Kaname/Books.lean"), format!("-- {}\n-- The transfers the modules build, one structure for each operation.\nimport Kaname.Prelude\n\n{books_text}", ritsu_emit::header::generated("kaname")), None));
    }
    for c in ordered {
        let full = c.lowered.clone().unwrap_or_else(|| c.clone());
        let p = Program::of(&full);
        let w = Mw::new(&p, &full, &done);
        let origin = origins.get(&c.canon).cloned().unwrap_or_else(|| origin(&c));
        let (text, stated) = w.text(c.decls.fns.len(), &c.properties, &origin);
        stated_by.insert(c.canon.clone(), c.properties.iter().enumerate().filter(|(_, cp)| stated.contains(&ident(cp.decl.name.ascii()))).map(|(k, _)| k).collect());
        let alias = c.module.name.ascii().to_string();
        files.push((PathBuf::from(format!("Kaname/{alias}.lean")), text, Some(origin.path.clone())));
        if !stated.is_empty() {
            samples.push((PathBuf::from(format!("samples/{alias}.lean")), sample(&c, &stated, &origin), Some(origin.path.clone())));
        }
        for (f, why) in w.skipped.borrow().iter() {
            skipped.push((c.canon.clone(), *f, why.clone()));
        }
        for (f, why) in w.skipped_props.borrow().iter() {
            skipped_props.push((c.canon.clone(), *f, why.clone()));
        }
        uses.insert(c.canon.clone(), w.uses.borrow().iter().map(|(u, _)| u.clone()).collect::<Vec<_>>());
        stats += w.stats.into_inner();
    }
    let head = format!("# {}\n", ritsu_emit::header::generated("kaname"));
    let mut lake = format!("{head}name = \"kaname\"\n");
    let mut libs = String::from("\n[[lean_lib]]\nname = \"Kaname\"\nglobs = [\"Kaname.+\"]\n");
    let mut targets = vec![toml_str("Kaname")];
    match layout {
        Layout::InPlace { out, proofs } => {
            for (alias, file) in proofs {
                let dir = file.parent().unwrap_or(std::path::Path::new("."));
                let root = file.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| alias.clone());
                let name = format!("proofs_{alias}");
                targets.push(toml_str(&name));
                libs.push_str(&format!("\n[[lean_lib]]\nname = {}\nsrcDir = {}\nroots = [{}]\n", toml_str(&name), toml_str(&relative(out, dir)), toml_str(&root)));
            }
        }
        Layout::Temp => {
            targets.push(toml_str("Proofs"));
            libs.push_str("\n[[lean_lib]]\nname = \"Proofs\"\nglobs = [\"Proofs.+\"]\n");
        }
    }
    lake.push_str(&format!("defaultTargets = [{}]\n", targets.join(", ")));
    lake.push_str(&libs);
    let mut out = vec![
        (PathBuf::from("lean-toolchain"), format!("{TOOLCHAIN}\n"), None),
        (PathBuf::from("lakefile.toml"), lake, None),
        (PathBuf::from(".gitignore"), "/.lake\n".to_string(), None),
        (PathBuf::from("Kaname/Prelude.lean"), format!("-- {}\n{PRELUDE}", ritsu_emit::header::generated("kaname")), None),
    ];
    out.extend(files);
    out.extend(samples);
    Written { files: out, stats, done, uses, skipped, skipped_props, stated: stated_by }
}

/// How many evaluations one list of the comparison's files holds.
const CHUNK: usize = 64;

/// The comparison of the Lean with the reference interpreter (PLAN 12.2.6): every total function
/// of `modules` (and of the modules they read) that is written, on every vector of its parameters
/// (`vectors::choose`, as the generated code's tests run them), evaluated by the reference
/// interpreter with `suite`, and written as a Lean term whose JSON text a file prints, beside the
/// text the interpreter's answer expects. The rules and dates the interpreter called are given to
/// `Inner` as tables of what they answered.
pub fn comparison(modules: &[Rc<Checked>], suite: &crate::suite::Suite) -> Comparison {
    use crate::interp::{Answered, Interp};
    let written = write(modules, &BTreeMap::new(), &Layout::Temp);
    let mut out = Comparison { package: written.files.iter().map(|(p, t, _)| (p.clone(), t.clone())).collect(), stats: written.stats, ..Comparison::default() };
    for c in in_order(modules) {
        let full = c.lowered.clone().unwrap_or_else(|| c.clone());
        let p = Program::of(&full);
        let w = Mw::new(&p, &full, &written.done);
        let alias = c.module.name.ascii().to_string();
        // the total functions with nothing not written for Lean that are not written
        for (canon, f, why) in &written.skipped {
            if *canon == c.canon && obstacle(&p, &full, *f).is_none() && stops(&full, *f).is_some() {
                out.unwritten.push(format!("{alias}.{}: {}", c.decls.fns[*f].name.text, why.en));
            }
        }
        let printers = Printers { w: &w, named: Default::default(), defs: Default::default() };
        let mut cases: Vec<(String, String, String)> = Vec::new();
        let mut answered: Vec<Answered> = Vec::new();
        let run = Program::of(&c);
        for f in 0..c.decls.fns.len() {
            let Some((lean, takes)) = written.done.fns.borrow().get(&(c.canon.clone(), f)).cloned() else { continue };
            let info = &c.decls.fns[f];
            let (vs, _) = crate::vectors::choose(&run, &c, f);
            let mut n = 0;
            for v in &vs {
                let mut it = Interp::new(&run, suite);
                it.answered = Some(Vec::new());
                if it.check_entry(&c, f, &v.input).is_err() {
                    continue;
                }
                // a vector a function that is not total fails on is no evaluation: Lean's answer there is
                // no answer of kaname's
                let Ok(answer) = it.call(&c, f, v.input.clone(), 0) else {
                    out.failed += 1;
                    continue;
                };
                let line = (|| -> Res<(String, String)> {
                    let mut call = w.kaname(&format!("{alias}.{lean}"));
                    if takes {
                        call.push_str(" inner₀");
                    }
                    for (prm, x) in info.params.iter().zip(&v.input) {
                        call.push(' ');
                        call.push_str(&par(&w.value(x, &prm.slot.ty)?));
                    }
                    Ok((format!("{} ({call})", printers.of(&info.ret.ty)?), w.canon(&answer, &info.ret.ty)?))
                })();
                match line {
                    Ok((code, expected)) => {
                        cases.push((info.name.text.clone(), code, expected));
                        answered.extend(it.answered.take().unwrap_or_default());
                        n += 1;
                    }
                    Err(why) => out.left.push(format!("{alias}.{}: {}", info.name.text, why.en)),
                }
            }
            if n > 0 {
                out.compared.push(Compared { module: alias.clone(), function: info.name.text.clone(), evaluations: n });
            }
        }
        if cases.is_empty() {
            continue;
        }
        // the rules and the dates, as tables of what they answered
        let uses = written.uses.get(&c.canon).cloned().unwrap_or_default();
        let inner = if uses.is_empty() {
            String::new()
        } else {
            match inner_table(&w, &uses, &answered) {
                Ok(t) => t,
                Err(why) => {
                    out.left.push(format!("{alias}: the tables of `Inner`: {}", why.en));
                    continue;
                }
            }
        };
        let mut text = format!(
            "-- {}\n-- The comparison of the module `{alias}` with the reference interpreter.\nimport Kaname.{alias}\n\nset_option maxRecDepth 100000\n\nnamespace Kaname.{alias}.Compare\n\n{JSON_HELPERS}\n",
            ritsu_emit::header::generated("kaname")
        );
        let defs = printers.defs.borrow();
        if !defs.is_empty() {
            text.push_str("mutual\n");
            for d in defs.iter() {
                text.push_str(d);
            }
            text.push_str("end\n\n");
        }
        text.push_str(&inner);
        let mut chunks = Vec::new();
        for (k, chunk) in cases.chunks(CHUNK).enumerate() {
            let name = format!("cs{}", sub(k + 1));
            text.push_str(&format!("def {name} : _root_.List _root_.String := [\n  {}]\n\n", chunk.iter().map(|(_, code, _)| code.clone()).collect::<Vec<_>>().join(",\n  ")));
            chunks.push(name);
        }
        text.push_str(&format!("def all : _root_.List _root_.String := _root_.List.flatten [{}]\n\nend Kaname.{alias}.Compare\n\ndef main : IO Unit := do\n  for s in {}.all do\n    IO.println (\"= \" ++ s)\n", chunks.join(", "), w.kaname(&format!("{alias}.Compare"))));
        let expected = cases.into_iter().map(|(f, _, e)| (f, e)).collect();
        out.files.push((alias.clone(), PathBuf::from(format!("Compare/{alias}.lean")), text, expected));
    }
    out
}

/// `inner₀`, an `Inner` whose rules and dates answer, for the values the reference interpreter gave
/// them, what they answered it, and the default for the rest.
fn inner_table(w: &Mw, uses: &[InnerUse], answered: &[crate::interp::Answered]) -> Res<String> {
    use crate::interp::Answered;
    let mut fields = Vec::new();
    for u in uses {
        let field = w.inner_use(u.clone());
        match u {
            InnerUse::Rule { canon, facts } => {
                let mut places: Vec<Ty> = facts.inputs.iter().map(|c| crate::names::column_slot(c, canon).ty).collect();
                for (seq, _, _) in &facts.elements {
                    places.push(Ty::List(Box::new(Ty::Record(RecId::Element(canon.clone(), seq.clone())))));
                }
                let out_ty = Ty::Record(RecId::Outputs(canon.clone()));
                let mut rows: Vec<(Vec<String>, String)> = Vec::new();
                for a in answered {
                    let Answered::Rule { canon: c2, given, out } = a else { continue };
                    if c2 != canon {
                        continue;
                    }
                    let key = places.iter().zip(given).map(|(t, v)| w.value(v, t)).collect::<Res<Vec<_>>>()?;
                    if !rows.iter().any(|(k, _)| *k == key) {
                        rows.push((key, w.value(out, &out_ty)?));
                    }
                }
                fields.push(format!("  {field} := {}", table(w, &places, rows, &format!("{}.default", w.std("Inhabited")))?));
            }
            InnerUse::Date { canon, facts, date } => {
                let f = facts.functions.iter().find(|f| f.name == *date).ok_or_else(|| tr!("日付 `{date}` を呼びます", "the date `{date}`"))?;
                let places: Vec<Ty> = f.params.iter().map(|_| Ty::Date).collect();
                let mut rows: Vec<(Vec<String>, String)> = Vec::new();
                for a in answered {
                    let Answered::Date { canon: c2, date: d2, given, day } = a else { continue };
                    if c2 != canon || d2 != date {
                        continue;
                    }
                    let mut key = Vec::new();
                    for prm in &f.params {
                        let input = facts.inputs.iter().find(|i| i.name == *prm).ok_or_else(|| tr!("入力 `{prm}` を渡します", "the input `{prm}`"))?;
                        let at = given.iter().find(|(n, _)| n == prm).map(|(_, at)| *at).unwrap_or(input.min);
                        key.push(par(&int(at as i128)));
                    }
                    if !rows.iter().any(|(k, _)| *k == key) {
                        rows.push((key, int(*day as i128)));
                    }
                }
                fields.push(format!("  {field} := {}", table(w, &places, rows, "0")?));
            }
        }
    }
    Ok(format!("def inner₀ : {} := {{\n{}\n}}\n\n", w.inner_ty, fields.join(",\n")))
}

/// A function of `places` that answers each row's value for its key, and `otherwise` for the rest.
fn table(w: &Mw, places: &[Ty], rows: Vec<(Vec<String>, String)>, otherwise: &str) -> Res<String> {
    if places.is_empty() {
        return Ok(rows.into_iter().next().map(|(_, v)| v).unwrap_or_else(|| otherwise.to_string()));
    }
    let mut binders = Vec::new();
    let mut names = Vec::new();
    for (i, t) in places.iter().enumerate() {
        let n = format!("a{}", sub(i + 1));
        binders.push(format!("({n} : {})", w.tname(t)?));
        names.push(n);
    }
    let tuple = |xs: &[String]| {
        let mut it = xs.iter().rev();
        let mut out = it.next().cloned().unwrap_or_default();
        for x in it {
            out = format!("{}.mk {} {}", w.std("Prod"), par(x), par(&out));
        }
        out
    };
    let entries: Vec<String> = rows.iter().map(|(k, v)| format!("({}.mk {} {v})", w.std("Prod"), par(&tuple(k)))).collect();
    Ok(format!("fun {} => (({}.lookup {} [{}]).getD {otherwise})", binders.join(" "), w.std("List"), par(&tuple(&names)), entries.join(",\n    ")))
}
