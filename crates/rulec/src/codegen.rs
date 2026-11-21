//! Code generation (§8).
//!
//! Readability of the generated code is defined by the five criteria of §8.1. In particular,
//! "one table row is one branch" and "conditions already known to be true from earlier branches
//! are not dropped" are the promises that let a reader match `.rule` rows to generated lines by
//! eye, so no clever optimization is attempted.

use crate::ast::*;
use crate::num::{Rat, RoundMode, RoundTo};
use crate::types::{Checked, Ty};
use std::collections::{BTreeMap, HashSet};

/// Build the ASCII brand name from a unit. Units are a closed set, so a fixed table suffices: ritsu's
/// table of units (`ritsu_units::table::type_name`), which began as this function's, so that a
/// package of ritsu's names the type of a unit as the rules do (ritsu's DESIGN 9.3).
fn brand_of(ty: &Ty) -> String {
    match ty {
        Ty::Money { cur, tax } => {
            let t = tax.as_deref().and_then(ritsu_units::Tax::parse).map(ritsu_units::Tax::type_suffix).unwrap_or("");
            format!("{}{t}", ritsu_units::table::type_name(cur))
        }
        Ty::Qty { unit, .. } => ritsu_units::table::type_name(unit),
        Ty::Rate => "Rate".into(),
        // A number carries no unit, so there is nothing to brand it apart from. Two counts
        // of different things would get the same brand anyway, and the caller would be left
        // wrapping every integer for no protection at all.
        Ty::Number => "int".into(),
        Ty::Date => "Date".into(),
        Ty::Bool => "bool".into(),
        Ty::Str => "str".into(),
        Ty::Enum(n) => n.clone(),
        Ty::Opt(t) => format!("Optional{}", brand_of(t)),
        Ty::Unknown => "int".into(),
    }
}

/// A value a machine carries along with its state (§15.217), as the generators spell it.
pub(crate) struct MoreCarried {
    /// The input and the output, by their names in the rule.
    pub input: String,
    pub output: String,
    /// The same, by their aliases: what a language names the field and the constant after.
    pub in_alias: String,
    pub out_alias: String,
    pub ty: Ty,
    /// Where a case starts: the value, as the wire writes it, and the integer a number travels
    /// as (0 for a boolean or an enum).
    pub start: crate::eval::Val,
    pub wire: String,
    pub int: i128,
}

impl MoreCarried {
    /// `INITIAL_DAYS_LEFT`: the constant's name where constants are upper case.
    pub(crate) fn upper(&self) -> String {
        format!("INITIAL_{}", self.in_alias.to_uppercase())
    }
    /// `InitialTurnsLeft` (Go) and `initialTurnsLeft` (Swift, where the state's own is
    /// `initialState`, so an input called `state` gets `initialStateValue`).
    pub(crate) fn go(&self) -> String {
        format!("Initial{}", pascal(&self.in_alias))
    }
    pub(crate) fn swift(&self) -> String {
        let n = format!("initial{}", pascal(&self.in_alias));
        if n == "initialState" { format!("{n}Value") } else { n }
    }
    /// The JSON key the traces name it by: the input's name in the rule.
    pub(crate) fn key(&self) -> String {
        crate::json::quote(&self.input)
    }
    /// The word for a boolean start, in the languages that spell it in lower case.
    pub(crate) fn bool_word(&self) -> &'static str {
        match self.start {
            crate::eval::Val::Bool(true) => "true",
            _ => "false",
        }
    }
    /// The enum value it starts at, when it is an enum.
    pub(crate) fn word(&self) -> String {
        match &self.start {
            crate::eval::Val::Enum(w) => w.clone(),
            _ => String::new(),
        }
    }
    /// What kind of value it is, for the code that differs by it.
    pub(crate) fn kind(&self) -> MoreKind {
        match self.ty.present() {
            Ty::Enum(_) => MoreKind::Enum,
            Ty::Bool => MoreKind::Bool,
            Ty::Number => MoreKind::Number,
            _ => MoreKind::Branded,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoreKind {
    Enum,
    Bool,
    /// A plain whole number.
    Number,
    /// An amount, a quantity or a rate: a number with a unit, which most languages brand.
    Branded,
}

/// `member_kind` → `MemberKind`.
fn pascal(s: &str) -> String {
    if let Some(rest) = s.strip_prefix(clash::MARK) {
        return format!("{}{}", clash::MARK, pascal(rest));
    }
    s.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut ch = p.chars();
            match ch.next() {
                Some(c) => c.to_uppercase().collect::<String>() + ch.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// The public surface uses the ASCII alias; internals keep the Japanese name (§8.1).
/// The public spelling of a declared name, for the callers outside this module that have
/// to say the same thing (the page for people names the inputs by their aliases).
pub fn pub_name_of(n: &Name) -> String {
    pub_name(n)
}

/// The type a declared enum is generated as in every language that names types: its alias in
/// PascalCase (§8.1).
pub fn type_name_of(n: &Name) -> String {
    pascal(&pub_name(n))
}

/// The enums a rule declares itself whose type takes a name the rule's own functions or class take
/// (§15.239), each with what it clashes with: in Go, one of the rule's functions (`Size`,
/// `SizeTraced`, `SizeRecord`, `SizeChances`); in Java, the rule's class. An enum read from an
/// `enums` file is not among them: the generated code names it by the file's package or class.
/// What `check` says of them under W121. The generated code writes the two apart since §15.326,
/// the enum's type taking a number after it in those languages, so the warning outlives its cause.
pub fn rule_name_clashes(f: &RuleFile) -> Vec<(&crate::ast::EnumDecl, (Option<String>, Option<String>))> {
    let alias = pub_name(&f.name);
    let fname = pascal(&alias);
    let go_fns = [fname.clone(), format!("{fname}Traced"), format!("{fname}Record"), crate::codegen::chances::chances_fn_name(&alias, "go")];
    let class = crate::codegen::java::java_class(&alias);
    f.enums
        .iter()
        .filter(|e| e.from.is_none())
        .filter_map(|e| {
            let ty = type_name_of(&e.name);
            let go = go_fns.iter().find(|n| **n == ty).cloned();
            let java = (crate::codegen::java::java_class(&pub_name(&e.name)) == class).then(|| class.clone());
            (go.is_some() || java.is_some()).then_some((e, (go, java)))
        })
        .collect()
}

/// The names the Go package of a rule declares at its top level for the rule itself (§15.239): the
/// rule's function and the functions beside it (`Size`, `SizeTraced`, `SizeRecord`,
/// `SizeChances`), the types they take and give (`Input`, `Output`, `Fired`, the elements of its
/// sequences, the two errors, `Chances` and `Way`), and a machine's `Initial`, `Final` and
/// `IsFinal`. One package holds one thing of a name, so an enum's type of one of these names is
/// named by the package that owns it; an enum the rule declares takes a number after its type where
/// its names meet the rule's functions (§15.326), and `check` says one named as one of the types.
pub fn go_rule_names(f: &RuleFile) -> Vec<String> {
    let alias = pub_name(&f.name);
    let fname = pascal(&alias);
    let mut out = vec![
        fname.clone(),
        format!("{fname}Traced"),
        format!("{fname}Record"),
        crate::codegen::chances::chances_fn_name(&alias, "go"),
        "Input".to_string(),
        "Output".to_string(),
        "Fired".to_string(),
        "RuleInputError".to_string(),
        "RuleContradictionError".to_string(),
        "Chances".to_string(),
        "Way".to_string(),
    ];
    if f.elements.len() <= 1 {
        out.push("Element".to_string());
    } else {
        out.extend(f.elements.iter().map(|el| format!("{}Element", pascal(&pub_name(&el.name)))));
    }
    if f.machine.is_some() {
        out.extend(["Initial", "Final", "IsFinal"].map(str::to_string));
    }
    out
}

fn pub_name(n: &Name) -> String {
    n.ascii.clone().unwrap_or_else(|| n.text.clone())
}

/// Python: the `NamedTuple` `class` of the `fields` (each name with its annotation), where the name
/// of a field is a word of the annotation of one after it. In the class's body a field's name is a
/// name from where it is declared, and a type checker reads the later annotation by the field: the
/// class is then written in `NamedTuple`'s other form, whose annotations, held as text, are read at
/// the top of the module (§15.336). None where no annotation names an earlier field. `named_tuple`
/// is how the module names `NamedTuple`.
fn py_named_apart(class: &str, fields: &[(String, String)], named_tuple: &str) -> Option<String> {
    let words = |ty: &str| ty.split(|ch: char| !(ch.is_alphanumeric() || ch == '_')).map(str::to_string).collect::<Vec<_>>();
    let meets = fields.iter().enumerate().any(|(i, (n, _))| fields[i + 1..].iter().any(|(_, ty)| words(ty).contains(n)));
    if !meets {
        return None;
    }
    let listed: Vec<String> = fields.iter().map(|(n, ty)| format!("(\"{n}\", \"{ty}\")")).collect();
    Some(format!("{class} = {named_tuple}(\"{class}\", [{}])\n", listed.join(", ")))
}

/// `base`, with `_` after it as many times as it takes to be none of `taken`: a local a function of
/// the generated code keeps for itself, apart from its parameters (§15.336).
fn free_of(base: &str, taken: &[String]) -> String {
    let mut n = base.to_string();
    while taken.iter().any(|t| *t == n) {
        n.push('_');
    }
    n
}

/// What every door says of a truth value that is not JSON's `true` or `false`, in the language
/// the code is generated in (§15.203).
fn not_bool(name: &str) -> String {
    crate::door::not_bool(name)
}

/// The public name of the value `v` of the enum `en`, as the rule writes it: its alias in
/// parentheses, else its name; a value of an enum the rule takes from the prelude is its name as
/// written. What ritsu's port hands over beside the generated member (`EnumValue::public`).
pub fn public_value(f: &RuleFile, en: &str, v: &str) -> String {
    f.enums.iter().find(|e| e.name.text == en).and_then(|e| e.values.iter().find(|x| x.text == v)).map(pub_name).unwrap_or_else(|| v.to_string())
}

/// Which items a pass over the rule's items writes (§15.58, §15.208, §15.219).
///
/// A rule that walks sequences has items before the walks, items of one element in each walk,
/// and items after each walk, so the body is written in passes over the same list: the items of
/// one call above the first loop and after each walk, and the items of one element inside each
/// loop — the stages the evaluator runs (`types::stages`). Every rule that walks nothing is `All`.
#[derive(Clone, Copy, PartialEq)]
pub enum Phase<'p> {
    All,
    /// The items at these places among the rule's items, in the order written.
    Only(&'p [usize]),
}

/// One step of the walks of a rule, in the order they run (§15.219): the items of one call that
/// run after the walks numbered so far, or one walk.
pub(crate) enum WalkStep {
    Once(Vec<usize>),
    Walk { seq: usize, body: Vec<usize>, sums: Vec<usize> },
}

/// The names one fold's walk binds itself (§15.56): the accumulators, the flags and `held`. The
/// first fold of a rule takes the plain names, so a rule with one fold is written as it always was;
/// a second takes them with its number after them (§15.219).
pub(crate) struct FoldNames {
    pub answer: String,
    pub stopped: String,
    /// Set when the fold has stopped reading, in a walk it shares with other summaries: the loop
    /// goes on for them (§15.219). A fold alone in its walk ends the loop instead.
    pub done: String,
    pub taken: String,
    pub kept: String,
    pub best: String,
    pub held: String,
    pub key: String,
    /// Go, Swift and Java keep "was it ever set" beside an accumulator that has no absent value.
    pub took: String,
    pub kept_one: String,
    pub best_set: String,
    /// The index of the element a `take_unique` took (§15.324): `taken_at` where locals are written
    /// in snake case, `takenAt` where in camel case.
    pub taken_at: String,
    pub taken_at_camel: String,
    /// A pick by weight (§15.226): the running total of the weights, one element's weight, and the
    /// running totals after each element weighed, which the function that counts the answers reads.
    pub total: String,
    pub weight: String,
    pub cuts: String,
}

pub struct Gen<'a> {
    f: &'a RuleFile,
    c: &'a Checked,
    /// Table name → the row pairs reported by W114. Guards are emitted only for these.
    w114: BTreeMap<String, Vec<(usize, usize)>>,
    /// Table output column → storage scale. Rate columns declare no step, so the scale is the
    /// least common multiple of the denominators of that column's literals. Unless it is fixed
    /// once per column, 50% and 100% in the same column would come out at different scales and
    /// the values would be corrupted.
    /// Declared name → the identifier the generated code uses for it (§1.3).
    idents: BTreeMap<String, String>,
    /// Type name → the ASCII alias of that enum (PascalCase).
    enum_names: BTreeMap<String, String>,
    /// Enum value → (type name, ASCII alias).
    /// `(enum, value)` -> the class and member it is generated as. Keyed by the enum as well,
    /// since two enums may both have `automatic` (§15.150).
    value_names: BTreeMap<(String, String), (String, String)>,
    src_hash: String,
    /// The source itself and where it was read from, for the page for people the MCP server
    /// serves (§15.52). The page is rendered from the rule, not from the generated code.
    src: String,
    /// The files the rule cites are found from this one's directory (§15.160).
    path: String,
    /// The rule's file as the head of every generated file names it (ritsu's DESIGN 9.2): its name,
    /// or under `ritsu gen` its path from the project's root (`shown`).
    shown: String,
    /// The stage each item of a walk runs in (§15.208).
    stages: crate::types::Stages,
    /// Set when the module is written into a package of ritsu's (ritsu's DESIGN 9.3): the Go
    /// import path of the package's directory. A module there reads the types of an `enums`
    /// file from its sibling in the package (§15.212); `rulec gen` writes them side by side.
    package: Option<String>,
    /// Where the module goes in that package (ritsu's DESIGN 17.8): under `deps/<name>/` for a rule
    /// of a dependency, and where each `enums` file it reads is, for the paths it imports them by.
    /// Outside a package, and for the module at hand, the package's directory.
    place: ritsu_emit::package::Place,
    /// `rulec gen`'s Python modules go inside a package (`--python-package`, §15.319): the module
    /// reads each `enums` file's module beside it by a relative import. Off for a module written
    /// into a package of ritsu's, whose paths `place` gives.
    python_package: bool,
    /// While one walk's items of one element are written: the names something in that walk reads
    /// (§15.219). A local an item binds and nothing in the walk reads is said to be read in Go,
    /// which refuses to compile it otherwise; the rest of the rule is not looked at there.
    reads_here: std::cell::RefCell<Option<HashSet<String>>>,
    /// While the body of one walk is written: the sequence it walks and the variable the index of
    /// the element is in, which a sentence about the element says (§15.324). `used` is whether one
    /// did, which is when the walk binds the index at all.
    walk_at: std::cell::RefCell<Option<WalkAt>>,
    /// The contract the rule implements (§15.274), read the first time a generator asks for it.
    served: std::cell::OnceCell<Option<crate::implements::Served>>,
    /// The backend whose code this generator writes, when it writes something there otherwise than
    /// the generator of every language: an enum under another type (§15.326), a value or a name of
    /// its own apart from the rule's (§15.336). None for the generator of every language
    /// (`Gen::for_target`).
    made_for: Option<&'static str>,
    /// Of those enums, each by its name with the type it has in every other language: the type the
    /// `.proto` declares it as, and for one of an `enums` file the type its file's module gives it,
    /// which this module hands on under its own (§15.326).
    renamed_from: BTreeMap<String, String>,
    /// The values the rule's functions bind under another identifier in this generator's language
    /// than the one it writes them as, by their names: where the identifier would hide a name the
    /// generated code reads there (`Gen::for_target`, §15.336). Empty for the generator of every
    /// language.
    binds: BTreeMap<String, String>,
    /// The names of the generated code's own that this generator's language writes otherwise, as
    /// it writes them, with what it writes instead: where a name of the rule's that the caller sees
    /// would meet one (§15.336). Empty for the generator of every language.
    owns: BTreeMap<String, String>,
    /// Whether the code this generator writes marks the rule's function where it names it, and has
    /// each enum's type marked in `enum_names`, `value_names` and `renamed_from`: the copy of the
    /// generator [`Gen::apart_text`] writes the module with the second time ([`Gen::marked`]).
    marks: bool,
    /// The helpers TypeScript's module declares under a name that starts with `_`, which a group's
    /// constant stays apart from (`Gen::ts_group`), worked out the first time a group asks.
    ts_helpers: std::cell::OnceCell<Vec<String>>,
}

/// The walk whose body is being written (`Gen::walk_at`).
pub(crate) struct WalkAt {
    /// The sequence, by the key the input's JSON has it under.
    seq: String,
    /// The variable of the generated code that holds the index of the element.
    var: String,
    /// Whether a sentence of the body named the element by its index.
    used: bool,
}

/// One piece of a sentence about an element (§15.324): text, or where the generated code puts the
/// index of the element (`Here`) or of the earlier element of two (`Before`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Piece<'s> {
    Text(&'s str),
    Here,
    Before,
}

/// A sentence cut where `door::HERE` and `door::BEFORE` leave the indices in it. The text between
/// two of them is there even when it is empty, so a language that keeps the pieces apart (Rust's
/// `Said`) has one between every two indices.
pub(crate) fn pieces(s: &str) -> Vec<Piece<'_>> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(k) = rest.find(['\u{1}', '\u{2}']) {
        out.push(Piece::Text(&rest[..k]));
        out.push(if rest[k..].starts_with('\u{1}') { Piece::Here } else { Piece::Before });
        rest = &rest[k + 1..];
    }
    out.push(Piece::Text(rest));
    out
}

/// Whether a sentence names an element by its index (§15.324).
pub(crate) fn names_element(s: &str) -> bool {
    s.contains(['\u{1}', '\u{2}'])
}

/// The sentence with each index put in between `open` and `close`, the way a language that builds a
/// string by interpolation writes it: `${index}` in a template literal, `{index}` in an f-string.
pub(crate) fn interpolate(s: &str, here: &str, before: &str, open: &str, close: &str) -> String {
    pieces(s)
        .into_iter()
        .map(|p| match p {
            Piece::Text(t) => t.to_string(),
            Piece::Here => format!("{open}{here}{close}"),
            Piece::Before => format!("{open}{before}{close}"),
        })
        .collect()
}

mod arith;
pub use arith::Uses;
mod sql;
pub use sql::round_tests_sql;
mod numpy;
pub use numpy::{np_runtime, round_tests_numpy};
mod php;
pub use php::round_tests_php;
mod java;
pub use java::{java_class, round_tests_java};
mod tool;
mod connect;
/// The two functions of a `prerequisites` block in each language (§15.222).
mod prereq;
pub use prereq::prereq_fn_names;
/// The function that counts the answers over the points of the random inputs, in each language
/// (§15.225).
mod chances;
/// How the projection function reads the caller's object (§15.234).
mod from;
/// The names an enum's code takes beside the rule's own functions (§15.326).
mod apart;
mod clash;
use clash::splice_fields;
/// The ways the generators spell the author's names, held against one another (§15.263) and
/// against the names the generated code gives itself (§15.265).
pub mod spellings;
/// The names the generated code of each target gives itself (§15.265).
mod own;
pub use chances::chances_fn_name;
pub use connect::template as connect_template;
pub use connect::{buf_gen_yaml, buf_lock, buf_yaml, BufDeps, Contracts};

impl<'a> Gen<'a> {
    /// The machine the rule is one step of (§15.148): the enum of the carried state, the
    /// state a case starts in, and the ones it ends in, in the order the enum declares them.
    pub(crate) fn machine_consts(&self) -> Option<(String, String, Vec<String>)> {
        let m = self.f.machine.as_ref()?;
        let (cin, _) = m.carried()?;
        let Some(Ty::Enum(en)) = self.c.ty_of(cin) else { return None };
        let init = m.initial.as_ref()?.0.text.clone();
        let vals = self.c.enums.get(&en)?;
        let fins: Vec<String> = vals.iter().filter(|v| m.finals.iter().any(|f| &f.text == *v)).cloned().collect();
        Some((en, init, fins))
    }

    /// The carried input and output, by their names in the rule.
    pub(crate) fn carried(&self) -> Option<(String, String)> {
        let (i, o) = self.f.machine.as_ref()?.carried()?;
        Some((i.to_string(), o.to_string()))
    }

    /// The carried output's name as a language spells a field of its answer: the rule's alias
    /// for the output, which each generator then cases its own way.
    pub(crate) fn carried_out_alias(&self) -> Option<String> {
        let (_, o) = self.carried()?;
        self.f.outputs.iter().find(|d| d.name.text == o).map(|d| pub_name(&d.name))
    }

    /// How a runner reads the next state off an answer (§15.148): `field` is the accessor as
    /// the language writes it (`.next_state`, `->next_state`, `.nextState()`). When the
    /// carried output is the rule's only output, the function returns that value itself rather
    /// than a record of one field, and the answer is the state.
    pub(crate) fn next_state_of(&self, answer: &str, field: &str) -> String {
        if self.f.outputs.len() == 1 {
            answer.to_string()
        } else {
            format!("{answer}{field}")
        }
    }

    /// The values a machine carries along with its state (§15.217), in the order of their
    /// `carry` lines, with where a case starts.
    pub(crate) fn more_carried(&self) -> Vec<MoreCarried> {
        crate::machine::more_starts(self.f, self.c)
            .into_iter()
            .filter_map(|(i, o, start)| {
                let di = self.f.inputs.iter().find(|d| d.name.text == i)?;
                let dout = self.f.outputs.iter().find(|d| d.name.text == o)?;
                let ty = self.ty_of(&i);
                let wire = crate::eval::wval(self.c, &i, &start).json();
                let int = match &start {
                    crate::eval::Val::Num(r) => crate::types::wire_int(*r, self.c.wire_scale(&i)),
                    _ => 0,
                };
                Some(MoreCarried { in_alias: pub_name(&di.name), out_alias: pub_name(&dout.name), input: i, output: o, ty, start, wire, int })
            })
            .collect()
    }

    /// The counts this rule declares, in source order (§15.58).
    fn counts(&self) -> Vec<&crate::ast::AggDecl> {
        self.f.items.iter().filter_map(|it| if let Item::Agg(d) = it { Some(d) } else { None }).collect()
    }

    /// The word a summary is called by, for the comment beside its accumulator.
    fn agg_word(&self, d: &crate::ast::AggDecl) -> String {
        match d.kind {
            crate::ast::AggKind::Sum => tr!("合計", "sum"),
            crate::ast::AggKind::Count => tr!("数え上げ", "count"),
            crate::ast::AggKind::Max => tr!("最大", "max"),
            crate::ast::AggKind::Min => tr!("最小", "min"),
        }
    }

    /// The upper bound a `sum` is held to, on the wire (§15.100). The walk refuses the
    /// moment the running total passes it, which is why the summed column has to be
    /// non-negative: the total only ever rises, so one test at the end is one test
    /// everywhere, and the accumulator never leaves `max + one element`.
    fn sum_caps(&self) -> Vec<(&crate::ast::AggDecl, i128)> {
        self.counts()
            .into_iter()
            .filter(|d| d.kind == crate::ast::AggKind::Sum)
            .filter_map(|d| {
                let hi = self.c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi)?;
                Some((d, crate::types::wire_int(hi, self.c.scales.get(&d.name.text).copied().unwrap_or(1))))
            })
            .collect()
    }

    /// The bottom a `sum` or a `count` is compared with once its walk is over
    /// ([`Checked::walk_floor`], §15.314), as the integer the total is held as. `None` where no
    /// walk comes in under the bottom of the range: no test is written.
    pub(crate) fn walk_floor(&self, d: &crate::ast::AggDecl) -> Option<i128> {
        let lo = self.c.walk_floor(d)?;
        Some(crate::types::wire_int(lo, self.c.scales.get(&d.name.text).copied().unwrap_or(1)))
    }

    /// The ends a `define` is compared with, right where it is worked out, when the rule accepts a
    /// range for it (§15.313), as
    /// the integers the value is held as, on its scale: each end the range writes that what the
    /// value reaches can pass. `None` for a value with no such range, or one whose range holds all
    /// it reaches: no call is refused there, and no test is written. What it reaches fits int64
    /// (E108), so every end written here does too.
    pub(crate) fn accepted_ends(&self, d: &crate::ast::DefineDecl) -> Option<(Option<i128>, Option<i128>)> {
        let (lo, hi) = self.c.refusing_ends(d)?;
        let sc = self.c.scales.get(&d.name.text).copied().unwrap_or(1);
        Some((lo.map(|v| crate::types::wire_int(v, sc)), hi.map(|v| crate::types::wire_int(v, sc))))
    }

    /// Whether the item at `i` is emitted in this phase. The split is the evaluator's: the
    /// stage `types::stages` gives it (§15.208, §15.219). A summary is the walk, not an item a
    /// pass writes.
    fn in_phase(&self, i: usize, it: &Item, phase: Phase) -> bool {
        if matches!(it, Item::Agg(_) | Item::Fold(_)) {
            return false;
        }
        match phase {
            Phase::All => true,
            Phase::Only(ix) => ix.contains(&i),
        }
    }

    /// The type one element of sequence `s` is written as (§15.219): `Element` when the rule
    /// walks one sequence, as it always was, and `<Sequence>Element` for each when it walks more.
    pub(crate) fn element_type(&self, s: usize) -> String {
        if self.f.elements.len() <= 1 {
            "Element".to_string()
        } else {
            format!("{}Element", pascal(&pub_name(&self.f.elements[s].name)))
        }
    }

    /// The loop variable of a walk over sequence `s`: `elem` for the first sequence, `elem2` and on
    /// for the others, so that a language that keeps a loop's variable for the whole function
    /// does not give one name two types (§15.219).
    pub(crate) fn elem_var(&self, s: usize) -> String {
        if s == 0 {
            self.temp("elem")
        } else {
            self.temp(&format!("elem{}", s + 1))
        }
    }

    /// The variable a walk over sequence `s` holds the index of the element in (§15.324).
    pub(crate) fn index_var(&self, s: usize) -> String {
        if s == 0 {
            self.temp("index")
        } else {
            self.temp(&format!("index{}", s + 1))
        }
    }

    /// Start writing the body of a walk over sequence `s`, whose index is in `var` (§15.324).
    pub(crate) fn walk_begin(&self, s: usize, var: &str) {
        *self.walk_at.borrow_mut() = Some(WalkAt { seq: self.f.elements[s].name.text.clone(), var: var.to_string(), used: false });
    }

    /// The body is written: whether a sentence of it named the element by its index, so that the
    /// walk binds the index.
    pub(crate) fn walk_end(&self) -> bool {
        self.walk_at.borrow_mut().take().is_some_and(|w| w.used)
    }

    /// What a sentence calls a field of the element or a `define` worked out for it (§15.324): in
    /// the body of a walk `lines[\u{1}].subtotal`, with the index left for the language to put in;
    /// anywhere else the name.
    pub(crate) fn said_name(&self, name: &str) -> String {
        match self.walk_at.borrow_mut().as_mut() {
            Some(w) => {
                w.used = true;
                crate::door::element(&w.seq, crate::door::HERE, name)
            }
            None => name.to_string(),
        }
    }

    /// The variable the walk being written holds the index of the element in; empty outside one.
    pub(crate) fn here(&self) -> String {
        self.walk_at.borrow().as_ref().map(|w| w.var.clone()).unwrap_or_default()
    }

    /// Mark the index of the walk being written as read by a sentence that does not go through
    /// `said_name`: the two elements of a `unique` field or a `take_unique`.
    pub(crate) fn here_used(&self) -> String {
        match self.walk_at.borrow_mut().as_mut() {
            Some(w) => {
                w.used = true;
                w.var.clone()
            }
            None => String::new(),
        }
    }

    /// The names the fold at `i` among the rule's items binds itself (`FoldNames`).
    pub(crate) fn fold_names(&self, i: usize) -> FoldNames {
        let k = self.f.items[..i].iter().filter(|it| matches!(it, Item::Fold(_))).count();
        let n = |b: &str| if k == 0 { self.temp(b) } else { self.temp(&format!("{b}{}", k + 1)) };
        FoldNames {
            answer: n("answer"),
            stopped: n("stopped"),
            done: n("done"),
            taken: n("taken"),
            kept: n("kept"),
            best: n("best"),
            held: n("held"),
            key: n("key"),
            took: n("tookOne"),
            kept_one: n("keptOne"),
            best_set: n("bestSet"),
            taken_at: n("taken_at"),
            taken_at_camel: n("takenAt"),
            total: n("weights"),
            weight: n("weight"),
            cuts: n("cuts"),
        }
    }

    /// The rule's pick by weight (§15.226), with its place among the items: the fold whose running
    /// totals the function that counts the answers reads. A rule has one at most.
    pub(crate) fn weighted_fold(&self) -> Option<(usize, &crate::ast::FoldDecl)> {
        self.f.items.iter().enumerate().find_map(|(i, it)| match it {
            Item::Fold(d) if d.weighted() => Some((i, d)),
            _ => None,
        })
    }

    /// The name of the function that runs the walk of a rule that picks by weight and gives back the
    /// running totals of the weights beside the answer and the trace (§15.225): `_pick_walked`, or the
    /// language's spelling of it. The public functions are the same as any rule's.
    pub(crate) fn walked_fn(&self, lang: &str) -> String {
        let base = format!("_{}_walked", snake(&pub_name(&self.f.name)));
        match lang {
            "typescript" | "javascript" | "swift" | "java" => format!("_{}", camel(&base[1..])),
            "go" => camel(&base[1..]),
            _ => base,
        }
    }

    /// The cap one sequence is held to: the smallest upper bound any **count** over it declares
    /// (§15.58), each sequence its own (§15.219).
    ///
    /// A count can be as large as the sequence, so a longer sequence could take a count outside
    /// the universe the completeness proof quantified over. The guard is the same shape as the one
    /// a number outside its range gets (§15.43). A `sum` says nothing about the length — what
    /// holds it is the running total itself (§15.100).
    pub(crate) fn count_cap_of(&self, s: usize) -> Option<i128> {
        let seq = &self.f.elements.get(s)?.name.text;
        self.counts()
            .iter()
            .filter(|d| d.kind == crate::ast::AggKind::Count && d.over == *seq)
            .filter_map(|d| self.c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi))
            .map(|hi| crate::types::wire_int(hi, 1))
            .min()
    }

    /// The `empty` value of a `max` or a `min`, in the units its column is held in (§15.219), as an
    /// integer: a number at the column's scale, a date as its day number. `None` for an enum's.
    pub(crate) fn extreme_empty_int(&self, d: &crate::ast::AggDecl) -> Option<String> {
        let ty = self.ty_of(&d.column.text);
        match d.empty.as_ref().map(|(l, _)| l)? {
            Lit::Num(n) => Some(self.int_lit(n, &ty, self.scale(&d.column.text))),
            Lit::Date(y, m, dd) => Some(format!("{}", crate::types::date_ord(*y, *m, *dd).num)),
            _ => None,
        }
    }

    /// The ordered enum a `max` or a `min` reads, when its column is one (§15.219).
    pub(crate) fn extreme_enum(&self, d: &crate::ast::AggDecl) -> Option<String> {
        let ty = self.ty_of(&d.column.text);
        self.c.ordered_enum(&ty).map(str::to_string)
    }

    /// The `empty` word of a `max` or a `min` over an ordered enum.
    pub(crate) fn extreme_empty_word(&self, d: &crate::ast::AggDecl) -> Option<String> {
        match d.empty.as_ref().map(|(l, _)| l)? {
            Lit::Word(w) => Some(w.clone()),
            _ => None,
        }
    }

    /// The definition set a table stands for: the merged table's name is the set's key, and
    /// a table that stands alone is its own set (§15.66).
    fn set_for(&self, t: &Table) -> Option<&crate::defset::DefSet> {
        let name = t.name.as_ref().map(|n| n.text.as_str()).unwrap_or("");
        self.c.sets.iter().find(|s| s.key == name)
    }

    /// The comment over a table's branches: the table and its policy, or, for a merged set,
    /// the tables it was built from in the order they are tried.
    fn table_head(&self, t: &Table) -> String {
        let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        match self.set_for(t) {
            Some(s) if s.merged() => {
                let order: Vec<String> =
                    (0..s.members.len()).rev().map(|mi| format!("{} {}", s.kind_word(mi), s.members[mi])).collect();
                tr!(
                    "{name} を定める表と節。例外を先に試す: {}",
                    "the tables and clauses defining {name}, exceptions first: {}",
                    order.join(if crate::i18n::ja() { "、" } else { ", " })
                )
            }
            _ if t.clause => tr!("節 {name}", "clause {name}"),
            _ => {
                let policy = if t.policy == Policy::Unique { crate::kw::UNIQUE } else { crate::kw::FIRST };
                tr!("表 {name}（{} {policy}）", "table {name} ({} {policy})", crate::kw::POLICY)
            }
        }
    }

    /// Whether row `ri` of `t` was written as a clause: `t` itself is one, or the row of a
    /// merged table came from one.
    fn is_clause_row(&self, t: &Table, ri: usize) -> bool {
        t.clause || self.set_for(t).is_some_and(|s| s.merged() && s.is_clause_row(ri))
    }

    /// How a branch names its row: the position as written, its label, and — in a merged
    /// set — the table it was written in.
    fn row_name(&self, t: &Table, ri: usize) -> String {
        let row = &t.rows[ri];
        if self.is_clause_row(t, ri) {
            let tn = row.origin.clone().unwrap_or_else(|| t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default());
            return tr!("節 {tn}", "clause {tn}");
        }
        let n = match &row.label {
            Some(l) => tr!("行{}（{}）", "row {} ({})", row.index, l.text),
            None => tr!("行{}", "row {}", row.index),
        };
        match &row.origin {
            Some(tn) => tr!("表 {tn} {n}", "table {tn} {n}"),
            None => n,
        }
    }

    /// The comment on one branch: the row as written.
    fn row_head(&self, t: &Table, ri: usize, cells: &str) -> String {
        format!("{}: {cells}", self.row_name(t, ri))
    }

    /// A group's constant in Ruby and in its signature: the ASCII alias when the group has
    /// one, else a name by declaration order. Ruby accepts `GROUP_遠隔地`, but RBS reads only
    /// ASCII identifiers, and the two have to agree (§15.67).
    fn rb_group(&self, name: &str) -> String {
        let id = self.ident(name);
        if id.is_ascii() {
            return id;
        }
        let k = self.f.groups.iter().position(|g| g.name.text == name).map(|i| i + 1).unwrap_or(0);
        format!("g{k}")
    }

    /// What a branch pushes on the trace: the table the row was written in, its position
    /// there, and its label (empty when it has none).
    fn fired_of(&self, t: &Table, ri: usize) -> (String, usize, String) {
        let row = &t.rows[ri];
        let tn = row.origin.clone().unwrap_or_else(|| t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default());
        (tn, row.index, row.label.as_ref().map(|l| l.text.clone()).unwrap_or_default())
    }

    /// `path` is where the rule was read from, as the command line named it, or "" when it was
    /// read from no file. It is an argument and not a setting because the
    /// files a rule cites — a `.proto` it imports an enum from, a contract it reads inputs
    /// out of — are found from the rule's directory, and a generator left to look from the
    /// working directory wrote different code depending on where it was run.
    pub fn new(f: &'a RuleFile, c: &'a Checked, src: &str, path: &str) -> Self {
        let mut g = Gen::bare(f, c, src, path);
        for set in &c.sets {
            let r = crate::region::check_set(set, c, f, "", crate::region::DEFAULT_BUDGET);
            if !r.w114.is_empty() {
                g.w114.insert(set.key.clone(), r.w114);
            }
        }
        g
    }

    /// The generator with no guards of W114 worked out: what the check asks of it, which is how
    /// the code it writes names things and which of its own names it reads (§15.265), and never
    /// the code itself.
    pub(crate) fn for_names(f: &'a RuleFile, c: &'a Checked) -> Self {
        Gen::bare(f, c, "", "")
    }

    fn bare(f: &'a RuleFile, c: &'a Checked, src: &str, path: &str) -> Self {
        let mut enum_names = BTreeMap::new();
        let mut value_names = BTreeMap::new();
        for e in &f.enums {
            // An enum read from an `enums` file is that file's type, whatever this rule calls it
            // (§15.212): the generated code reads it from the file's module.
            let ty = match &e.from {
                Some(o) => pascal(&pub_name(&o.name)),
                None => pascal(&pub_name(&e.name)),
            };
            enum_names.insert(e.name.text.clone(), ty.clone());
            for v in &e.values {
                value_names.insert((e.name.text.clone(), v.text.clone()), (ty.clone(), pascal(&pub_name(v))));
            }
        }
        // The built-in namespaces: the generated names are frozen in the prelude (§15.182).
        for im in crate::prelude::imports(f) {
            enum_names.insert(im.ty.to_string(), im.ns.generated.to_string());
            for d in im.ns.divisions {
                value_names.insert((im.ty.to_string(), im.value(d).to_string()), (im.ns.generated.to_string(), d.member.to_string()));
            }
        }
        let w114: BTreeMap<String, Vec<(usize, usize)>> = BTreeMap::new();
        // §1.3: a name that declared an ASCII alias is written out under that alias, inside
        // the function as well as on the public face.
        let mut idents: BTreeMap<String, String> = BTreeMap::new();
        for n in f
            .inputs
            .iter()
            .map(|i| &i.name)
            .chain(f.outputs.iter().map(|o| &o.name))
            .chain(f.groups.iter().map(|g| &g.name))
            .chain(f.items.iter().flat_map(|it| -> Vec<&crate::ast::Name> {
                match it {
                    Item::Derived(d) => vec![&d.name],
                    Item::Define(d) => vec![&d.name],
                    Item::Agg(d) => vec![&d.name],
                    Item::Fold(d) => d.answer.iter().map(|a| &a.name).collect(),
                    Item::Table(t) => t.outputs.iter().map(|o| &o.name).collect(),
                }
            }))
        {
            if n.ascii.is_some() {
                idents.insert(n.text.clone(), pub_name(n));
            }
        }
        Gen {
            f,
            c,
            w114,
            enum_names,
            value_names,
            idents,
            // A file's `fed by` moves nothing generated, the head's digest included (§15.285).
            src_hash: hash(&ritsu_base::fed::without(src)),
            src: src.to_string(),
            path: path.to_string(),
            shown: ritsu_emit::header::file_name(path),
            stages: crate::types::stages(f),
            package: None,
            place: ritsu_emit::package::Place::here(),
            python_package: false,
            reads_here: std::cell::RefCell::new(None),
            walk_at: std::cell::RefCell::new(None),
            served: std::cell::OnceCell::new(),
            made_for: None,
            renamed_from: BTreeMap::new(),
            binds: BTreeMap::new(),
            owns: BTreeMap::new(),
            marks: false,
            ts_helpers: std::cell::OnceCell::new(),
        }
    }

    /// This generator with the marks [`Gen::apart_text`] reads ([`clash::MARK`]): before the rule's
    /// function where the code names it ([`Gen::fam`]), and before each enum's type wherever the code
    /// names the type, which every way of writing one reads from `enum_names`, `value_names` and
    /// `renamed_from` (the name conversions keep a mark at the head where it is).
    fn marked(&self) -> Gen<'a> {
        let mark = |t: &String| format!("{}{t}", clash::MARK);
        Gen {
            f: self.f,
            c: self.c,
            w114: self.w114.clone(),
            enum_names: self.enum_names.iter().map(|(k, t)| (k.clone(), mark(t))).collect(),
            value_names: self.value_names.iter().map(|(k, (t, m))| (k.clone(), (mark(t), m.clone()))).collect(),
            idents: self.idents.clone(),
            src_hash: self.src_hash.clone(),
            src: self.src.clone(),
            path: self.path.clone(),
            shown: self.shown.clone(),
            stages: crate::types::stages(self.f),
            package: self.package.clone(),
            place: self.place.clone(),
            python_package: self.python_package,
            reads_here: std::cell::RefCell::new(None),
            walk_at: std::cell::RefCell::new(None),
            served: std::cell::OnceCell::new(),
            made_for: self.made_for,
            renamed_from: self.renamed_from.iter().map(|(k, t)| (k.clone(), mark(t))).collect(),
            binds: self.binds.clone(),
            owns: self.owns.clone(),
            marks: true,
            ts_helpers: std::cell::OnceCell::new(),
        }
    }

    /// The same generator, writing a module of a package of ritsu's whose Go import path is
    /// `module` (§15.212).
    pub fn in_package(mut self, module: &str) -> Self {
        self.package = Some(module.to_string());
        self
    }

    /// The same generator, its module put at `place` in the package (ritsu's DESIGN 17.8).
    pub fn at(mut self, place: ritsu_emit::package::Place) -> Self {
        self.place = place;
        self
    }

    /// The same generator, with `rulec gen`'s Python modules put inside a package when `yes`
    /// (`--python-package`, §15.319).
    pub fn in_python_package(mut self, yes: bool) -> Self {
        self.python_package = yes;
        self
    }

    /// The same generator, its heads naming the rule's file as `path` (a path from a project's
    /// root, as `ritsu gen` writes it) rather than by its name.
    pub fn shown(mut self, path: &str) -> Self {
        self.shown = path.to_string();
        self
    }

    /// Which of the helpers of §15.213–§15.215 the rule's expressions call: only those are
    /// written into its module and its unit vectors.
    pub fn arith_uses(&self) -> Uses {
        Uses::of(self.f)
    }

    /// The page for people, as `rulec doc --format html` renders it: the same
    /// renderer, running the same generated JavaScript (§15.37, §15.52).
    pub fn page(&self) -> String {
        crate::doc::render_html(self.f, self.c, &self.src, &self.path, &self.page_javascript())
    }

    /// The JavaScript a page holds: the module, with the module of each `enums` file it reads put
    /// inside it (§15.319). A page is one file, `rulec doc` writes nothing beside it and an MCP host
    /// shows it alone, so `./catalog.mjs` is not there to import. Each `enums` file's module goes
    /// into a function that answers what it exports, and the lines that import from it or hand its
    /// names on take them from that answer.
    pub fn page_javascript(&self) -> String {
        self.page_javascript_with(&|p| crate::enums::read_enums_file(p).ok())
    }

    /// [`Gen::page_javascript`], with each `enums` file the rule reads as `enums_file` gives it: an
    /// engine that keeps the checks of a run gives the file as its check kept it, so that the file
    /// is not checked again (ritsu's DESIGN 4.15). A file it gives none for is left out, as a file
    /// that does not pass its check is.
    pub fn page_javascript_with(&self, enums_file: &dyn Fn(&str) -> Option<crate::enums::EnumsFile>) -> String {
        // the module as the JavaScript target writes it, each enum under the type it takes there
        // (§15.326)
        if let Some(t) = self.for_target("javascript") {
            return t.page_javascript_with(enums_file);
        }
        let mut js = self.javascript();
        for p in crate::enums::owner_paths(self.f) {
            let Some(ef) = enums_file(&p) else { continue };
            let module = pub_name(&ef.file.name);
            let code = strip_types(&Gen::new(&ef.file, &ef.checked, &ef.src, &ef.path).ts_enums_module());
            js = module_inside(&js, &module, &code);
        }
        js
    }

    fn ty_of(&self, n: &str) -> Ty {
        self.c.ty_of(n).unwrap_or(Ty::Unknown)
    }

    /// Every name whose type needs a brand in the generated code: the inputs, the fields of
    /// one element, and the outputs.
    ///
    /// A field of `elements` is as much a value the caller hands over as an input is, and it
    /// is written into a struct of its own — so a brand it does not get is a type the
    /// generated code *names* and never defines. Nothing caught this because every rule that
    /// walks a sequence was written in Japanese and its element fields were all `money`,
    /// whose brand the outputs already pulled in (§15.9: a shape reached in one language is
    /// a shape checked in one language).
    fn branded(&self) -> Vec<&String> {
        self.f
            .inputs
            .iter()
            .map(|i| &i.name.text)
            .chain(self.f.elements.iter().flat_map(|e| e.fields.iter().map(|fd| &fd.name.text)))
            .chain(self.f.outputs.iter().map(|o| &o.name.text))
            .collect()
    }

    /// The columns a table writes that may be absent (`-> fee : money[JPY]?`). Rust, Go and Swift
    /// hold one under the type of its unit (`Option<JPY>`, `*JPY`, `JPY?`), as an input that may be
    /// absent passes on as it stands (§15.249), so their module declares that type though none of
    /// the rule's inputs, fields and outputs carries the unit (§15.343). The other languages hold
    /// such a column as a plain integer.
    fn branded_columns(&self) -> Vec<&String> {
        self.f
            .items
            .iter()
            .filter_map(|it| if let Item::Table(t) = it { Some(t) } else { None })
            .flat_map(|t| t.outputs.iter().map(|o| &o.name.text))
            .filter(|n| matches!(self.ty_of(n), Ty::Opt(_)))
            .collect()
    }

    /// The brands of the module (§8.2), by name, with the type each one brands: those the module
    /// declares, and those it takes from the `units` module of a package of ritsu's (§15.248).
    /// From `rulec gen`, the module declares all of them. In a package, the brands of money and
    /// of the quantities are the package's, one type of each unit that every rule and every book
    /// of the package names, so that what one rule returns is what another takes; a rate keeps
    /// its brand in the module, as its integer counts the step this rule declares. A number gets
    /// no brand: it is a plain integer on purpose, and branding it would shadow the language's
    /// own integer. An optional amount is the same brand when it is there: the runner builds it,
    /// and a rule whose only amount is optional declared none (§15.201).
    fn brands(&self) -> (BTreeMap<String, Ty>, BTreeMap<String, Ty>) {
        let (mut own, mut units) = (BTreeMap::new(), BTreeMap::new());
        for v in self.branded() {
            let ty = self.ty_of(v).present().clone();
            let into = match ty {
                Ty::Money { .. } | Ty::Qty { .. } if self.package.is_some() => &mut units,
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => &mut own,
                _ => continue,
            };
            into.insert(brand_of(&ty), ty);
        }
        (own, units)
    }

    /// The units whose types the module of a package of ritsu's takes from the package's `units`
    /// module (§15.248), as the rule writes them, for the package to declare them there.
    pub fn package_units(&self) -> Vec<ritsu_units::Unit> {
        self.brands().1.values().filter_map(|ty| ty.unit(None)).collect()
    }

    /// The identifier a declared name gets in the generated code. §1.3: the public face
    /// always uses its ASCII alias, and an internal name uses one when the author wrote it
    /// and the name itself when they did not. Every lookup in this file keys on the declared
    /// name; only what is *written out* goes through here.
    ///
    /// In the generator of one language (`Gen::for_target`), a value the rule's functions would
    /// hide a name of the generated code's with is bound under another identifier there (§15.336),
    /// and this is that identifier: every place a generator writes a value as it is held — a
    /// parameter, a variable, a read of one — goes through here. What a caller sees of the value —
    /// a field, a key, a label, a keyword argument — is written from its declared name or alias
    /// (`pub_name`), or by [`Gen::written`], and keeps it.
    fn ident(&self, n: &str) -> String {
        if let Some(b) = self.binds.get(n) {
            return b.clone();
        }
        self.written(n)
    }

    /// The identifier of a declared name as every language writes it (§1.3), whatever a language
    /// binds it under inside the rule's functions ([`Gen::ident`]).
    fn written(&self, n: &str) -> String {
        self.idents.get(n).cloned().unwrap_or_else(|| n.to_string())
    }

    /// The declared name `n` where the rule's functions hold it, in a language that writes a value as
    /// its alias, else its name (`pub_name`): an input, a sequence, a field of an element, a shape.
    /// The identifier it is bound under apart from that (§15.336), where it has one.
    fn held(&self, n: &Name) -> String {
        self.binds.get(&n.text).cloned().unwrap_or_else(|| pub_name(n))
    }

    /// A name of the generated code's own, as this language writes it here: the name, unless a name
    /// of the rule's would meet it, when it is written otherwise (§15.336).
    fn own(&self, name: &str) -> String {
        self.owns.get(name).cloned().unwrap_or_else(|| name.to_string())
    }

    /// `text`, a part of the code that holds no name of the rule's (a helper, a line the module
    /// starts with), with each name of the generated code's own that this language writes otherwise
    /// written so (§15.336).
    fn own_text(&self, text: &str) -> String {
        if self.owns.is_empty() {
            return text.to_string();
        }
        clash::rename_words(text, &self.owns)
    }

    /// The rule's function `name` (its name, or one of the names made of it) where the module calls
    /// or declares it: with [`clash::MARK`] before it while [`Gen::apart_text`] writes the module
    /// again, else as it is.
    pub(crate) fn fam(&self, name: &str) -> String {
        if self.marks && name == pub_name(&self.f.name) {
            format!("{}{name}", clash::MARK)
        } else {
            name.to_string()
        }
    }

    /// The code `write` writes, in a language whose module holds the rule's function and the enums'
    /// types beside the functions of its own and reads builtins by their names (Python, TypeScript
    /// and JavaScript, Ruby, Rust, Swift), with each word of the rule's that the code has for another
    /// name too written apart there (§15.336). Where the rule's function or an enum's type is named
    /// like a builtin, the code reaches the builtin past it (`_builtins.abs`, `globalThis.JSON`,
    /// `::Struct`, `::std::string::String`, `Swift.Int64`, `Gen::hidden_builtins`); where a helper of
    /// the module's has the function's name (`_max`, `round_down`), the helper takes `_` after it, as
    /// many as it takes to meet no word of the module. The code is written again by [`Gen::marked`],
    /// and every word of the name left unmarked is the other's. Code where no name meets one is
    /// written as it always was, and so is code whose second writing is not the first with the
    /// marks in it, which a test's build stops at.
    fn apart_text(&self, syntax: clash::Syntax, write: impl Fn(&Gen<'a>) -> String) -> String {
        self.written_apart(syntax, true, write)
    }

    /// [`Gen::apart_text`] for code that names the enums' types and declares no function of the
    /// rule's: Ruby's signatures.
    fn apart_types(&self, syntax: clash::Syntax, write: impl Fn(&Gen<'a>) -> String) -> String {
        self.written_apart(syntax, false, write)
    }

    fn written_apart(&self, syntax: clash::Syntax, fns: bool, write: impl Fn(&Gen<'a>) -> String) -> String {
        let text = write(self);
        let rule = pub_name(&self.f.name);
        // the builtins an enum's type hides, each with the way the code reaches it
        let mut to: BTreeMap<String, String> =
            self.c.enums.keys().map(|jp| self.enum_type(jp)).filter_map(|t| self.owns.get(&t).map(|w| (t.clone(), w.clone()))).collect();
        let hidden = if fns { self.owns.get(&rule).cloned() } else { None };
        let twice = fns && clash::declares_twice(&text, &rule, syntax);
        if hidden.is_none() && !twice && to.is_empty() {
            return text;
        }
        let marked = write(&self.marked());
        if marked.replace(clash::MARK, "") != text {
            debug_assert!(false, "the code written with the marks is other than the code:\n{}", clash::first_difference(&text, &marked.replace(clash::MARK, "")));
            return text;
        }
        if hidden.is_some() || twice {
            let new = hidden.unwrap_or_else(|| {
                let mut n = format!("{rule}_");
                while clash::has_word(&marked, &n) {
                    n.push('_');
                }
                n
            });
            to.insert(rule, new);
        }
        clash::rename_code(&marked, &to, syntax)
    }

    /// A name for a generated temporary that nothing in the rule already answers to. The
    /// value held before the last rounding used to be called `raw` unconditionally, which
    /// collided the day an output declared the alias `raw` — Python quietly rebound it and
    /// TypeScript refused to parse.
    fn temp(&self, base: &str) -> String {
        let f = self.f;
        let declared: Vec<&crate::ast::Name> = f
            .inputs
            .iter()
            .map(|i| &i.name)
            .chain(f.outputs.iter().map(|o| &o.name))
            .chain(f.groups.iter().map(|g| &g.name))
            .chain(f.items.iter().flat_map(|it| -> Vec<&crate::ast::Name> {
                match it {
                    Item::Derived(d) => vec![&d.name],
                    Item::Define(d) => vec![&d.name],
                    Item::Agg(d) => vec![&d.name],
                    Item::Fold(d) => d.answer.iter().map(|a| &a.name).collect(),
                    Item::Table(t) => t.outputs.iter().map(|o| &o.name).collect(),
                }
            }))
            // The sequence a walk reads is a parameter of every generated function, and the
            // fields of one element are locals inside the loop — so both are names a helper
            // must not take. `elements 候補(rows)` used to collide with the `rows` the record
            // function builds its trace in, and the TypeScript and JavaScript modules then
            // would not parse at all (§15.128).
            .chain(f.elements.iter().flat_map(|el| {
                std::iter::once(&el.name).chain(el.fields.iter().map(|fd| &fd.name))
            }))
            // A `shape` is a parameter of the projection function for the same reason
            // (§15.125).
            .chain(f.shapes.iter().map(|d| &d.name))
            .collect();
        let mut n = base.to_string();
        // A name declared without an alias is its own identifier, so the text counts too, and so
        // does the identifier a value is bound under apart from it (§15.336).
        while declared.iter().any(|d| d.text == n || pub_name(d) == n) || self.binds.values().any(|b| *b == n) {
            n.push('_');
        }
        n
    }

    /// Storage scale of a value (values are held as multiples of 1/k; this is that k). §7.1.
    fn scale(&self, n: &str) -> i128 {
        *self.c.scales.get(n).unwrap_or(&1)
    }

    /// A value written into an output column, brought to that column's scale.
    ///
    /// A name carries the scale of whatever it names — a definition over a rate input is
    /// held in hundredths, say — while the column has a scale of its own, fixed by the
    /// literals that appear in it. Assigning one into the other without this is how a rate
    /// landed in a column a hundred times its size.
    fn rescaled(&self, from: &str, to: &str, text: String) -> String {
        let (a, b) = (self.scale(from), self.scale(to));
        // The column is sized to hold whatever is written into it (see `Gen::new`), so this
        // only ever widens. Anything else would silently drop precision the evaluator keeps.
        if a == b || a == 0 || b % a != 0 {
            text
        } else {
            // No parentheses: this lands on the right of an assignment, and both formatters
            // strip a redundant pair there.
            format!("{text} * {}", b / a)
        }
    }

    /// An integer literal adjusted to the type and the column's scale.
    fn int_lit(&self, n: &crate::lex::Num, ty: &Ty, scale: i128) -> String {
        let v = crate::types::lit_value_in_pub(n, ty).unwrap_or(Rat::zero());
        format!("{}", v.mul(Rat::int(scale)).num)
    }

    /// The head of a file generated from the rule (ritsu's DESIGN 9.2): ritsu's version, and the
    /// rule's file with the digest of its bytes; then what else it was made from.
    fn header(&self, comment: &str) -> String {
        let version = self.f.version.to_string();
        let kind = if self.f.kind == FileKind::Enums { crate::kw::ENUMS } else { crate::kw::RULE };
        let source = ritsu_emit::header::Source { path: &self.shown, kind, name: &self.f.name.text, version: &version, sha256: &self.src_hash, module: self.place.module.as_ref() };
        let lang = if crate::i18n::ja() { ritsu_base::text::Lang::Ja } else { ritsu_base::text::Lang::En };
        let mut h = format!("{comment} {}\n{comment} {}\n", ritsu_emit::header::generated("rulec"), source.line().get(lang));
        // A rule applied by this one is part of what was generated (§15.69): its path and the
        // digest the rule pinned, so the file says which callee it was made from.
        for a in &self.f.applies {
            // The pin as written: `meaning:` from §15.230 on, `sha256:` before.
            let pin = match (&a.meaning, &a.hash) {
                (Some(m), _) => format!("meaning:{m}"),
                (None, h) => format!("sha256:{}", h.as_deref().unwrap_or("")),
            };
            h.push_str(&tr!(
                "{comment} 準用: {} = {} ({pin})\n",
                "{comment} Applies: {} = {} ({pin})\n",
                a.name.text,
                ritsu_emit::header::one_line(&a.path)
            ));
        }
        // The contributions merged into the rule (§15.231), and into a rule it applies, under the
        // apply's name (§15.237): the function is the rule with them, so the file says which ones,
        // from where, at which shape and which bytes.
        let direct = self.f.contributions.iter().filter(|m| m.via.is_none());
        for m in direct.chain(self.f.contributions.iter().filter(|m| m.via.is_some())) {
            h.push_str(&tr!(
                "{comment} 寄与: {} = {} (signature:{}, sha256:{})\n",
                "{comment} Contributions: {} = {} (signature:{}, sha256:{})\n",
                m.shown_name(),
                ritsu_emit::header::one_line(&self.contribution_shown(&m.file)),
                m.signature.as_deref().unwrap_or(""),
                // `fed by` moves nothing generated (§15.285).
                &ritsu_base::fed::hash(&m.src)[..16]
            ));
        }
        // The documents the rule transcribes (§15.68): each law at the date it was read, with
        // the pinned digest of every fragment cited, so the file says which text of the law it
        // was made from — two dates when the rule folds two periods (§15.71).
        for s in &self.f.sources {
            let what = match &s.kind {
                SourceKind::Law { db, id, asof } => {
                    let pins: Vec<String> = s.pins.iter().map(|p| format!("{} sha256:{}", p.fragment, p.hash)).collect();
                    let pinned = if pins.is_empty() { String::new() } else { format!(" ({})", pins.join(", ")) };
                    // The database is named where it is not the one the rule would have read
                    // without saying, so a header written before there was a choice reads the
                    // same as it did.
                    let which = match db {
                        crate::ast::LawDb::Egov => String::new(),
                        d => format!(" {}", d.word()),
                    };
                    format!("{}{which} {id} {} {asof}{pinned}", crate::kw::LAW, crate::kw::ASOF)
                }
                SourceKind::File { path, url, hash } => {
                    let u = url.as_ref().map(|u| format!(" {} {u}", crate::kw::URL)).unwrap_or_default();
                    // The tables taken out of the document, as a law's articles are (§15.82).
                    let pins: Vec<String> = s.pins.iter().map(|p| format!("{} sha256:{}", p.fragment, p.hash)).collect();
                    let pinned = if pins.is_empty() { String::new() } else { format!(" ({})", pins.join(", ")) };
                    format!(
                        "{} {path}{u}{}{pinned}",
                        crate::kw::FILE,
                        hash.as_ref().map(|h| format!(" sha256:{h}")).unwrap_or_default()
                    )
                }
            };
            // what a source file names (a path, a URL) stays on the comment's line (ritsu's DESIGN 9.2)
            let what = ritsu_emit::header::one_line(&what);
            h.push_str(&tr!("{comment} 出典: {} = {what}\n", "{comment} Cites: {} = {what}\n", s.name.text));
        }
        h
    }
}

/// The languages whose generated code writes a declared name as it is, as a parameter, a local
/// or a field, with no escape of their own for a keyword (§15.210): Java and PHP add `_` to one
/// and Swift puts it in backticks, SQL quotes every name, and NumPy's plan is data.
const AS_WRITTEN: &[&str] = &["python", "typescript", "javascript", "rust", "go", "ruby", "wasm"];

/// The languages among [`AS_WRITTEN`] that keep `word` as a keyword, in the order `rulec gen`
/// writes them.
pub fn keyword_in(word: &str) -> Vec<&'static str> {
    crate::backend::ALL.iter().filter(|b| AS_WRITTEN.contains(&b.id) && b.reserved.contains(word)).map(|b| b.name).collect()
}

/// The names the generated code writes as parameters, locals, fields and constants, in the order
/// [`assign_identifiers`] takes them: the inputs, the outputs, each sequence a walk reads with the
/// fields of one element, the groups, the shapes, and what the items name (derived values,
/// definitions, summaries, a fold's answer, the columns a table writes). Each occurrence, so one
/// name may come more than once, with whether a keyword gives it `_` after it (§15.210), which it
/// does not give a group's or a shape's. A character no language takes gives any of them `_` for
/// it (§15.311).
fn value_names(f: &RuleFile) -> Vec<(&Name, bool)> {
    let mut out: Vec<(&Name, bool)> = Vec::new();
    out.extend(f.inputs.iter().map(|i| (&i.name, true)));
    out.extend(f.outputs.iter().map(|o| (&o.name, true)));
    for el in &f.elements {
        out.push((&el.name, true));
        out.extend(el.fields.iter().map(|fd| (&fd.name, true)));
    }
    out.extend(f.groups.iter().map(|g| (&g.name, false)));
    out.extend(f.shapes.iter().map(|s| (&s.name, false)));
    for it in &f.items {
        match it {
            Item::Derived(d) => out.push((&d.name, true)),
            Item::Define(d) => out.push((&d.name, true)),
            Item::Agg(d) => out.push((&d.name, true)),
            Item::Fold(d) => out.extend(d.answer.iter().map(|a| (&a.name, true))),
            Item::Table(t) => out.extend(t.outputs.iter().map(|o| (&o.name, true))),
        }
    }
    out
}

/// Every name of [`value_names`], to be written.
fn each_value_name_mut(f: &mut RuleFile, mut g: impl FnMut(&mut Name)) {
    for i in &mut f.inputs {
        g(&mut i.name);
    }
    for o in &mut f.outputs {
        g(&mut o.name);
    }
    for el in &mut f.elements {
        g(&mut el.name);
        for fd in &mut el.fields {
            g(&mut fd.name);
        }
    }
    for gr in &mut f.groups {
        g(&mut gr.name);
    }
    for s in &mut f.shapes {
        g(&mut s.name);
    }
    for it in &mut f.items {
        match it {
            Item::Derived(d) => g(&mut d.name),
            Item::Define(d) => g(&mut d.name),
            Item::Agg(d) => g(&mut d.name),
            Item::Fold(d) => {
                if let Some(a) = &mut d.answer {
                    g(&mut a.name);
                }
            }
            Item::Table(t) => {
                for oc in &mut t.outputs {
                    g(&mut oc.name);
                }
            }
        }
    }
}

/// The names of the tables and clauses, which the generated code writes into identifiers of its
/// own: the SQL query's `<table>_row` column and the Rust proof's `rows_<table>`.
fn table_names(f: &RuleFile) -> Vec<&Name> {
    f.items.iter().filter_map(|it| if let Item::Table(t) = it { t.name.as_ref() } else { None }).collect()
}

/// Whether a name came in from another file: an `apply` brings a callee's in as `<apply>:<name>`
/// (§15.69) and a contribution its own as `<contribution>:<name>` (§15.231). A name a rule
/// declares has no `:` in it.
fn brought_in(text: &str) -> bool {
    text.contains(':')
}

/// The identifiers a rule's generated code writes for its values, its tables and its enums: each
/// name's alias, else the name itself (§1.3), as [`assign_identifiers`] has left them. `apply`
/// reads a callee's, so that what it brings in is written after the identifier the callee's own
/// code gives it (§15.260): the callee's `値引き(credit)` is `credit` in the callee's code, and
/// `<apply>_credit` in this rule's.
#[derive(Default)]
pub struct Idents {
    values: std::collections::HashMap<String, String>,
    tables: std::collections::HashMap<String, String>,
    enums: std::collections::HashMap<String, String>,
}

impl Idents {
    pub fn of(f: &RuleFile) -> Idents {
        let mut out = Idents::default();
        for (n, _) in value_names(f) {
            if let Some(a) = &n.ascii {
                out.values.insert(n.text.clone(), a.clone());
            }
        }
        for n in table_names(f) {
            if let Some(a) = &n.ascii {
                out.tables.insert(n.text.clone(), a.clone());
            }
        }
        for e in &f.enums {
            if let Some(a) = &e.name.ascii {
                out.enums.insert(e.name.text.clone(), a.clone());
            }
        }
        out
    }

    fn get(map: &std::collections::HashMap<String, String>, n: &Name) -> String {
        map.get(&n.text).cloned().or_else(|| n.ascii.clone()).unwrap_or_else(|| n.text.clone())
    }

    /// The identifier of a value: a parameter, a local, a field.
    pub fn value(&self, n: &Name) -> String {
        Self::get(&self.values, n)
    }

    /// The identifier a table's or a clause's name is written into.
    pub fn table(&self, n: &Name) -> String {
        Self::get(&self.tables, n)
    }

    /// The identifier an enum's type is named after.
    pub fn enum_type(&self, n: &Name) -> String {
        Self::get(&self.enums, n)
    }
}

/// What the identifiers of what an `apply` brings in start with (§15.260): the apply's alias, else
/// its name, with a capital at its head made small, since Ruby reads a name that starts with one as
/// a constant, which a function cannot assign.
pub fn apply_prefix(n: &Name) -> String {
    let p = n.ascii.clone().unwrap_or_else(|| n.text.clone());
    let mut cs = p.chars();
    match cs.next() {
        Some(c) if c.is_ascii_uppercase() => c.to_ascii_lowercase().to_string() + cs.as_str(),
        _ => p,
    }
}

/// The longest head of `id` within `bytes` bytes that ends at the end of a whole character.
fn head_within(id: &str, bytes: usize) -> &str {
    let mut end = 0;
    for (i, c) in id.char_indices() {
        if i + c.len_utf8() > bytes {
            break;
        }
        end = i + c.len_utf8();
    }
    &id[..end]
}

/// The head of an identifier PostgreSQL keeps: it cuts a name at 63 bytes, at the end of a whole
/// character (`NAMEDATALEN`), so two that agree that far are one name to it.
fn pg_head(id: &str) -> &str {
    head_within(id, 63)
}

/// The ways the languages rulec writes spell the identifier of one value, each tagged with the
/// way, so that two identifiers meet where some language spells them alike (§15.260): as it is
/// (Python, TypeScript, JavaScript, Rust, Ruby, Go's locals, SQL, NumPy, Wasm), with the case of
/// its letters set aside (SQLite reads a quoted `"Credit"` and `"credit"` as one column, and the
/// WIT of the Wasm component lowers every name), in camelCase (Swift) and with Java's escape over
/// it, in PascalCase (Go's fields), in PHP's two spellings, and cut where PostgreSQL cuts a name.
fn value_spellings(id: &str) -> [(u8, String); 8] {
    [
        (0, id.to_string()),
        (1, id.to_ascii_lowercase()),
        (2, camel(id)),
        (3, java::java_name_pub(id)),
        (4, pascal(id)),
        (5, php::php_name(id)),
        (6, php::php_var_pub(id)),
        (7, pg_head(id).to_string()),
    ]
}

/// The languages' ways of spelling a value's identifier in which `a` and `b` are one identifier,
/// none when they are two in every language rulec writes (§15.260): `"as written"`, `"case"`,
/// `"camelCase"`, `"Java"`, `"PascalCase"`, `"PHP"`, `"PHP variable"`, `"PostgreSQL"`.
pub fn values_meet(a: &str, b: &str) -> Vec<&'static str> {
    const WAYS: [&str; 8] = ["as written", "case", "camelCase", "Java", "PascalCase", "PHP", "PHP variable", "PostgreSQL"];
    value_spellings(a).iter().zip(value_spellings(b).iter()).filter(|(x, y)| x.1 == y.1).map(|(x, _)| WAYS[x.0 as usize]).collect()
}

/// The same for an enum's type, which every language names in PascalCase, and some (PHP's classes,
/// a file system that does not tell case apart) without telling case apart.
fn type_spellings(id: &str) -> [(u8, String); 2] {
    let p = pascal(id);
    let l = p.to_ascii_lowercase();
    [(0, p), (1, l)]
}

/// The same for a table's name: as it is in the Rust proof's `rows_<table>`, and in the SQL
/// query's `<table>_row` column, which stands among the columns of the values, so it is held to
/// theirs where SQLite reads it without case and PostgreSQL cuts it.
fn table_spellings(id: &str) -> [(u8, String); 3] {
    let col = format!("{id}{SQL_ROW}");
    [(8, id.to_string()), (1, col.to_ascii_lowercase()), (7, pg_head(&col).to_string())]
}

/// What the SQL query's column of a table's row number adds to the table's identifier
/// (`sql::sql_row_col`).
const SQL_ROW: &str = "_row";

/// Whether some language rulec writes keeps `id` as a word of its own, its case set aside as W121
/// sets it aside (§15.103): an identifier rulec makes is never one.
fn reserved_anywhere(id: &str) -> bool {
    crate::backend::ALL.iter().any(|b| b.reserved.iter().any(|w| w.eq_ignore_ascii_case(id)))
}

/// The identifier the made one `base` takes among those already `taken`: `base` itself where no
/// language spells it as one of them, else the first of `base_2`, `base_3`, … that meets none —
/// `class_2`, not `class__2`, after the `_` a keyword is given. A head longer than PostgreSQL keeps
/// is cut so that the number stays inside what it keeps, with `after` the bytes a spelling adds
/// after the identifier (a table's `_row`).
fn free_identifier<const N: usize>(base: &str, after: usize, taken: &std::collections::HashSet<(u8, String)>, spell: impl Fn(&str) -> [(u8, String); N], keyword: impl Fn(&str) -> bool) -> String {
    let fits = |id: &str| !keyword(id) && spell(id).iter().all(|k| !taken.contains(k));
    if fits(base) {
        return base.to_string();
    }
    for k in 2usize.. {
        let tail = if base.ends_with('_') { k.to_string() } else { format!("_{k}") };
        let room = 63 - after - tail.len();
        let head = if base.len() > room { head_within(base, room) } else { base };
        let id = format!("{head}{tail}");
        if fits(&id) {
            return id;
        }
    }
    unreachable!("the numbers run out before the identifiers do")
}

/// Whether every language rulec writes code in takes `id` as a name by its characters: Python and
/// Rust (XID_Start, then XID_Continue), TypeScript and JavaScript (ID_Start, then ID_Continue), and
/// Go, whose names take no mark and no number but a decimal digit — by ritsu-emit's tables of
/// Unicode 17.0 (`ident`). Java takes every name Go does, Swift every one of these, and Ruby and
/// PHP every character outside ASCII. A name of rulec's is a letter or `_` and then any character
/// up to the next that ends a word (`lex.rs`), so it can have one none of them takes: `²` (a
/// number of No), `㎡`, a letter of a later Unicode.
pub fn takes_everywhere(id: &str) -> bool {
    ritsu_emit::ident::rust_takes(id) && ritsu_emit::ident::typescript_takes(id) && ritsu_emit::ident::go_takes(id)
}

/// The identifier made for a name with no alias that some language rulec writes does not take
/// (§15.311): the name with `_` for each character that one does not take where it stands, `v`
/// before one that is all `_`. `差²` is written `差_`, `℃値` `_値`, in every language alike, as a
/// keyword's `_` is (§15.210): only the identifier moves.
pub(crate) fn takeable(name: &str) -> String {
    let s: String = name
        .chars()
        .enumerate()
        .map(|(i, c)| {
            let alone = if i == 0 { c.to_string() } else { format!("_{c}") };
            if takes_everywhere(&alone) { c } else { '_' }
        })
        .collect();
    if s.chars().all(|c| c == '_') { format!("v{s}") } else { s }
}

/// The identifiers rulec makes for a rule, by the name they are for, as [`assign_identifiers`]
/// writes them in: the values, the tables and the enums. A name rulec does not make an identifier
/// for — one with an alias, one written as itself — is not among them.
#[derive(Default)]
pub struct Made {
    pub values: BTreeMap<String, String>,
    pub tables: BTreeMap<String, String>,
    pub enums: BTreeMap<String, String>,
}

/// The identifiers rulec makes for a rule (§15.210, §15.260), worked out without writing them in,
/// so that `check` can say what the generated code writes.
///
/// A name the rule declares is written as its alias, else as itself, and those identifiers are the
/// author's: they are taken first, as they are. Then come the ones rulec makes, in the order the
/// names are declared: a name with no alias that a language rulec writes names in as they are
/// keeps as a keyword, which takes `_` after it (§15.210), one with no alias that has a character
/// some language rulec writes takes in no name, which takes `_` for that character ([`takeable`],
/// §15.311; a group's and a shape's too, whose names no keyword moves), and everything an `apply`
/// or a contribution brings in, which takes the identifier its own file gives it, after the apply's or
/// the contribution's (`apply.rs`, `link.rs`). Each made one goes where no language spells it as
/// one already taken — as it is, in camelCase, in PascalCase, with Java's and PHP's escapes, with
/// case set aside, cut at PostgreSQL's 63 bytes — and is no language's keyword; else it takes the
/// first number after it that is so ([`free_identifier`]). The values, the tables and the enums are
/// three kinds of identifier, each held apart within its kind.
pub fn made_identifiers(f: &RuleFile) -> Made {
    use std::collections::HashSet;
    let mut made = Made::default();
    // The values: each name once, with the alias of its last occurrence that has one, as the
    // generators read it (`Gen::new`).
    let mut order: Vec<&str> = Vec::new();
    let mut alias: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    let mut escapes: HashSet<&str> = HashSet::new();
    for (n, esc) in value_names(f) {
        if !order.contains(&n.text.as_str()) {
            order.push(&n.text);
        }
        if let Some(a) = &n.ascii {
            alias.insert(&n.text, a);
        }
        if esc {
            escapes.insert(&n.text);
        }
    }
    // The values and the tables share one set: a table's SQL row column stands among the values'
    // columns. The rule's own identifiers go in first, the made ones after, each where it meets none.
    let mut taken: HashSet<(u8, String)> = HashSet::new();
    let mut escaped: Vec<(&str, String)> = Vec::new();
    let mut brought: Vec<(&str, String)> = Vec::new();
    for t in &order {
        match alias.get(t) {
            _ if brought_in(t) => brought.push((t, alias.get(t).map(|a| a.to_string()).unwrap_or_else(|| t.to_string()))),
            Some(a) => taken.extend(value_spellings(a)),
            None if escapes.contains(t) && t.is_ascii() && !keyword_in(t).is_empty() => escaped.push((t, format!("{t}_"))),
            None if !takes_everywhere(t) => escaped.push((t, takeable(t))),
            None => taken.extend(value_spellings(t)),
        }
    }
    let mut brought_tables: Vec<(&str, String)> = Vec::new();
    for n in table_names(f) {
        let id = n.ascii.clone().unwrap_or_else(|| n.text.clone());
        if brought_in(&n.text) {
            brought_tables.push((&n.text, id));
        } else {
            taken.extend(table_spellings(&id));
        }
    }
    for (t, base) in escaped.into_iter().chain(brought) {
        let id = free_identifier(&base, 0, &taken, value_spellings, reserved_anywhere);
        taken.extend(value_spellings(&id));
        made.values.insert(t.to_string(), id);
    }
    for (t, base) in brought_tables {
        let id = free_identifier(&base, SQL_ROW.len(), &taken, table_spellings, |_| false);
        taken.extend(table_spellings(&id));
        made.tables.insert(t.to_string(), id);
    }
    // The enums, which name types beside the rule's own function, class and module, the built-in
    // divisions it imports, and the enums it reads from an `enums` file (§15.212).
    let mut taken: HashSet<(u8, String)> = HashSet::new();
    for n in go_rule_names(f) {
        taken.extend(type_spellings(&n));
    }
    for im in crate::prelude::imports(f) {
        taken.extend(type_spellings(im.ns.generated));
    }
    let mut brought: Vec<(&str, String)> = Vec::new();
    for e in &f.enums {
        let id = match &e.from {
            Some(o) => pub_name(&o.name),
            None => pub_name(&e.name),
        };
        if brought_in(&e.name.text) {
            brought.push((&e.name.text, id));
        } else {
            taken.extend(type_spellings(&id));
        }
    }
    let top = |id: &str| reserved_anywhere(id) || crate::backend::ALL.iter().any(|b| b.globals.iter().any(|w| w.eq_ignore_ascii_case(&pascal(id))));
    for (t, base) in brought {
        let id = free_identifier(&base, 0, &taken, type_spellings, top);
        taken.extend(type_spellings(&id));
        made.enums.insert(t.to_string(), id);
    }
    made
}

/// Write into the rule the identifiers rulec makes for it ([`made_identifiers`]), on every
/// occurrence of each name, so that every generator, `rulec api` and the pages read one
/// identifier for one name (§15.210, §15.260). The name on the wire, in the records and in every
/// message stays the one declared: only the identifier moves, and in every language alike, so
/// that `rulec api` names one parameter one way. An alias is the author's own choice of
/// identifier, and is left as it is: W121 says where a language does not take it, as it always
/// did. The rule's own name and an enum's are left to W121 too: they name a module and a type,
/// which the files and the other rules refer to.
pub fn assign_identifiers(f: &mut RuleFile) {
    let made = made_identifiers(f);
    each_value_name_mut(f, |n| {
        if let Some(id) = made.values.get(&n.text) {
            n.ascii = Some(id.clone());
        }
    });
    for it in &mut f.items {
        if let Item::Table(t) = it {
            if let Some(n) = &mut t.name {
                if let Some(id) = made.tables.get(&n.text) {
                    n.ascii = Some(id.clone());
                }
            }
        }
    }
    for e in &mut f.enums {
        if let Some(id) = made.enums.get(&e.name.text) {
            e.name.ascii = Some(id.clone());
        }
    }
}

/// The SHA-256 of the source text, as the header says it is. Until §15.68 the header named
/// SHA-256 and computed FNV-1a; the digest is now the one it names, from `sha256.rs`, with no
/// dependency added.
pub fn hash(s: &str) -> String {
    crate::sha256::hex(s.as_bytes())
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

/// An expression rendered as text together with its scale. Both languages get the same integer
/// arithmetic.
pub(crate) struct Expr2 {
    text: String,
    scale: i128,
}

/// The truth values in the text [`Gen::expr`] writes: marks no identifier can hold, which each
/// language's spelling of the text turns into its own words (§15.264). The text used to carry
/// Python's `True` and `False`, and the other languages turned every `True` in it into `true`,
/// inside an identifier as well: `isTrueFee` was written `istrueFee`, and `True_` `true_`.
pub(crate) const TRUE_MARK: &str = "\u{1d}true\u{1d}";
pub(crate) const FALSE_MARK: &str = "\u{1d}false\u{1d}";

/// The text [`Gen::expr`] wrote, its truth values spelled `t` and `f`.
pub(crate) fn truths(s: &str, t: &str, f: &str) -> String {
    s.replace(TRUE_MARK, t).replace(FALSE_MARK, f)
}

/// The text [`Gen::expr`] wrote, as Python: the truth values are `True` and `False`.
fn py_truths(s: &str) -> String {
    truths(s, "True", "False")
}

impl<'a> Gen<'a> {
    /// The ordered enum an expression's value belongs to (§15.210): a name of one, or `min` or
    /// `max` of such names.
    pub(crate) fn ordered_of(&self, e: &Expr) -> Option<String> {
        match e {
            Expr::Name(n, _) => self.c.ordered_enum(&self.ty_of(n)).map(|s| s.to_string()),
            Expr::Call(name, args, _) if name == crate::kw::MIN || name == crate::kw::MAX => args.first().and_then(|a| self.ordered_of(a)),
            _ => None,
        }
    }

    /// The type an enum is generated as: `Era`.
    pub(crate) fn enum_type(&self, en: &str) -> String {
        self.enum_names.get(en).cloned().unwrap_or_else(|| pascal(en))
    }

    /// The names the Go package of the rule declares at its top level for the rule itself
    /// (§15.239): its functions, the types its functions take and give, and a machine's constants.
    /// An enum's type of one of these names cannot be declared beside them.
    pub(crate) fn go_taken_names(&self) -> Vec<String> {
        crate::codegen::go_rule_names(self.f)
    }

    /// The package that owns an enum read from an `enums` file, when the rule's Go package cannot
    /// declare the enum's type under its own name (§15.239): the rule `size` reading `enum size`
    /// would declare `func Size` and `type Size = sizes.Size` in one package. The package then names
    /// the type by the package that owns it, `sizes.Size`, as Java names it by its class.
    pub(crate) fn go_owner_of(&self, en: &str) -> Option<String> {
        let o = self.f.enums.iter().find(|e| e.name.text == en)?.from.as_ref()?;
        let t = self.enum_type(en);
        self.go_taken_names().contains(&t).then(|| self.go_qualifier(&pub_name(&o.module), &self.owner_dir(o)))
    }

    /// The directory of the module of an `enums` file the rule reads in the package (ritsu's
    /// DESIGN 17.8): empty for one of the module at hand, and outside a package.
    fn owner_dir(&self, o: &crate::ast::EnumOwner) -> String {
        self.place.dir_of(std::path::Path::new(&o.path))
    }

    /// The name the rule's Go package imports the package of an `enums` file by: its package's
    /// name, or, when another `enums` file it reads has a package of that name in another module of
    /// the package, the name with that module's dependencies before it (`pricing_sizes`; ritsu's
    /// DESIGN 17.8).
    fn go_qualifier(&self, module: &str, dir: &str) -> String {
        let pkg = crate::backend::go_package(module);
        let clash = self.foreign_owners().iter().any(|(m, d, _)| d != dir && crate::backend::go_package(m) == pkg);
        if clash { ritsu_emit::package::local_name(dir, &pkg) } else { pkg }
    }

    /// The path a contribution's file is written by in the head: from the rule's directory, as an
    /// `apply` names its callee; in a package of a module, from the root when the two are of two
    /// modules (ritsu's DESIGN 17.8), since a path from a dependency's directory to the module at
    /// hand names no file.
    fn contribution_shown(&self, file: &str) -> String {
        let (own, theirs) = (ritsu_emit::package::deps_dir(&self.shown), self.place.dir_of(std::path::Path::new(file)));
        match self.place.rel_of(std::path::Path::new(file)) {
            Some(rel) if own != theirs => rel,
            _ => crate::link::relative(&self.path, file),
        }
    }

    /// The Go type of an enum as the rule's package writes it (§15.212, §15.239): the type it
    /// declares, or the alias it declares for one an `enums` file owns, or that file's type by its
    /// package where the alias would take a name the rule's own functions have.
    pub(crate) fn go_enum_ty(&self, en: &str) -> String {
        match self.go_owner_of(en) {
            Some(pkg) => format!("{pkg}.{}", self.owner_type(en)),
            None => self.enum_type(en),
        }
    }

    /// The Go type of an enum as the runner writes it, the rule's package imported as `r`: `r.Size`,
    /// or the owner's type by its package where the rule's package names it so (§15.239).
    pub(crate) fn go_runner_enum_ty(&self, en: &str) -> String {
        match self.go_owner_of(en) {
            Some(pkg) => format!("{pkg}.{}", self.owner_type(en)),
            None => format!("r.{}", self.enum_type(en)),
        }
    }

    /// The enums the generated code declares itself, by name in the rule, with their types:
    /// every enum but those read from an `enums` file, whose module declares them (§15.212),
    /// each type once.
    pub(crate) fn own_enums(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        for (jp, ascii) in &self.enum_names {
            if self.c.owners.contains_key(jp) || out.iter().any(|(_, a)| a == ascii) {
                continue;
            }
            out.push((jp.clone(), ascii.clone()));
        }
        out
    }

    /// The enums read from `enums` files, grouped by the module each comes from (§15.212): the
    /// module's name (the alias of the file's heading) and the types it gives this rule.
    pub(crate) fn foreign_enums(&self) -> Vec<(String, Vec<(String, String)>)> {
        self.foreign_owners().into_iter().map(|(m, _, ts)| (m, ts)).collect()
    }

    /// [`Gen::foreign_enums`], each module with the directory of its module in the package (ritsu's
    /// DESIGN 17.8): two `enums` files of one name in two modules are two modules there.
    pub(crate) fn foreign_owners(&self) -> Vec<(String, String, Vec<(String, String)>)> {
        let mut out: Vec<(String, String, Vec<(String, String)>)> = Vec::new();
        for e in &self.f.enums {
            let Some(o) = &e.from else { continue };
            let module = pub_name(&o.module);
            let dir = self.owner_dir(o);
            let ty = self.enum_type(&e.name.text);
            match out.iter_mut().find(|(m, d, _)| *m == module && *d == dir) {
                Some((_, _, ts)) => {
                    if !ts.iter().any(|(_, t)| *t == ty) {
                        ts.push((e.name.text.clone(), ty));
                    }
                }
                None => out.push((module, dir, vec![(e.name.text.clone(), ty)])),
            }
        }
        out
    }

    /// The directory under the package's directory the rule's module is in: `rules/`, or under a
    /// dependency's directory (ritsu's DESIGN 17.8).
    fn rules_dir(&self) -> String {
        format!("{}rules/", self.place.dir)
    }

    /// The enums of the rule that give display names (§15.211), by their names in the rule, each
    /// with its type and the module of the `enums` file it is read from when it is: what a
    /// runner's `--labels` answers for.
    pub(crate) fn labeled_enums(&self) -> Vec<(String, String, Option<String>)> {
        let mut out: Vec<(String, String, Option<String>)> = Vec::new();
        for en in &self.c.enum_order {
            if !self.c.labels.contains_key(en) || out.iter().any(|(n, _, _)| n == en) {
                continue;
            }
            // An enum this module hands on under another type is found in this module, under the
            // names it has here (§15.326).
            let owner = self.c.owners.get(en).filter(|_| !self.handed_here(en)).map(|o| pub_name(&o.module));
            out.push((en.clone(), self.enum_type(en), owner));
        }
        out
    }

    /// The input of a runner's `--labels`: one line per value of every enum that gives display
    /// names, the enum's name and the value's, a tab between (§15.211). `None` for a rule
    /// with none.
    pub fn labels_input(&self) -> Option<(String, String)> {
        let enums = self.labeled_enums();
        if enums.is_empty() {
            return None;
        }
        let (mut input, mut expected) = (String::new(), String::new());
        for (en, _, _) in enums {
            for v in self.c.enums.get(&en).into_iter().flatten() {
                input.push_str(&format!("{en}\t{v}\n"));
                expected.push_str(&format!("{}\n", self.c.shown_value(&en, v)));
            }
        }
        Some((input, expected))
    }

    /// Whether the functions of this rule compare two values of the ordered enum `en`, or take
    /// the later or the earlier of two: the helpers they call are written only then.
    pub(crate) fn enum_helpers(&self, en: &str) -> (bool, bool, bool) {
        let mut rank = false;
        let mut max = false;
        let mut min = false;
        fn walk(g: &Gen, e: &Expr, en: &str, rank: &mut bool, max: &mut bool, min: &mut bool) {
            match e {
                Expr::Bin(l, op, r, _) => {
                    if matches!(op, BinOp::Le | BinOp::Lt | BinOp::Ge | BinOp::Gt)
                        && g.ordered_of(l).or_else(|| g.ordered_of(r)).as_deref() == Some(en)
                    {
                        *rank = true;
                    }
                    walk(g, l, en, rank, max, min);
                    walk(g, r, en, rank, max, min);
                }
                Expr::Call(name, args, _) => {
                    if args.first().and_then(|a| g.ordered_of(a)).as_deref() == Some(en) {
                        if name == crate::kw::MAX {
                            *max = true;
                        } else if name == crate::kw::MIN {
                            *min = true;
                        }
                    }
                    for a in args {
                        walk(g, a, en, rank, max, min);
                    }
                }
                _ => {}
            }
        }
        let mut exprs: Vec<&Expr> = Vec::new();
        for it in &self.f.items {
            match it {
                Item::Derived(d) => exprs.push(&d.expr),
                Item::Define(d) => exprs.push(&d.expr),
                _ => {}
            }
        }
        if let Some(r) = &self.f.result {
            exprs.push(&r.expr);
        }
        for fold in self.f.folds() {
            exprs.extend(fold.empty.iter());
            exprs.extend(fold.exhausted.iter());
            for (_, arm, _) in &fold.arms {
                match arm {
                    Arm::Stop(Some(x)) => exprs.push(x),
                    Arm::Take { expr, .. } => exprs.push(expr),
                    Arm::KeepMax { expr, key } => {
                        exprs.push(expr);
                        exprs.push(key);
                    }
                    Arm::TakeWeighted { expr, weight } => {
                        exprs.push(expr);
                        exprs.push(weight);
                    }
                    _ => {}
                }
            }
        }
        for e in exprs {
            walk(self, e, en, &mut rank, &mut max, &mut min);
        }
        // A `max` or a `min` over a column of the enum compares two of its values (§15.219).
        if self.f.aggs().any(|d| d.kind.extreme() && self.extreme_enum(d).as_deref() == Some(en)) {
            rank = true;
        }
        (rank, max, min)
    }

    fn expr(&self, e: &Expr, local: &dyn Fn(&str) -> String) -> Expr2 {
        match e {
            Expr::Name(n, _) => Expr2 { text: local(n), scale: self.scale(n) },
            Expr::Lit(Lit::Num(n), _) => {
                let ty = crate::types::lit_ty_pub(n);
                let v = crate::types::lit_value_in_pub(n, &ty).unwrap_or(Rat::zero());
                // Not the reduced denominator: the scale `types` records for a name defined
                // over this literal. Storing `3.6%` at 250 and reading the name back at 1000
                // is a silent factor of four (§7.1).
                let den = v.den.max(1);
                let s = crate::types::lit_scale(n).filter(|s| s % den == 0).unwrap_or(den);
                Expr2 { text: format!("{}", v.num * (s / den)), scale: s }
            }
            Expr::Lit(Lit::Word(w), _) if w == crate::kw::TRUE || w == crate::kw::FALSE => {
                Expr2 { text: (if w == crate::kw::TRUE { TRUE_MARK } else { FALSE_MARK }).into(), scale: 1 }
            }
            // A date in an expression is its day number, the same integer the cells use. It
            // used to fall through to `0` with the other literals, so a definition such as
            // `作成日 <= 2027-03-31` compiled to `made <= 0` in every language while the
            // evaluator read the date — the first rule to compare a date with a literal in a
            // definition found it (§15.38).
            Expr::Lit(Lit::Date(y, m, d), _) => {
                Expr2 { text: format!("{}", crate::types::date_ord(*y, *m, *d).num), scale: 1 }
            }
            Expr::Lit(..) => Expr2 { text: "0".into(), scale: 1 },
            // A rounding of a quotient by a value (§15.213): one integer division, by the helper
            // of the mode, of the dividend and the divisor brought to one step with the grid's
            // (`types::division_factors`). It answers whole grid steps, which the grid's own
            // integer brings back to the grid's scale. The divisor is never zero (E132).
            Expr::Call(name, args, _) if crate::types::var_division(name, args).is_some() => {
                let (n, d) = crate::types::var_division(name, args).unwrap();
                let mode = RoundMode::parse(name).unwrap();
                let (ne, de) = (self.expr(n, local), self.expr(d, local));
                let Some(Expr::Lit(Lit::Num(g), _)) = args.get(1) else { panic!("gen: the grid of `{name}` is a literal (E118)") };
                let (gi, sg) = crate::types::grid_int(g).expect("gen: a grid the check read");
                let (ka, kb) = crate::types::division_factors(ne.scale, de.scale, gi, sg);
                let top = if ka == 1 { ne.text } else { format!("({} * {ka})", ne.text) };
                let bot = if kb == 1 { de.text } else { format!("({} * {kb})", de.text) };
                let steps = format!("{}({top}, {bot})", arith::py_div(mode));
                Expr2 { text: if gi == 1 { steps } else { format!("({steps} * {gi})") }, scale: sg }
            }
            Expr::Call(name, args, _) => {
                let a: Vec<Expr2> = args.iter().map(|x| self.expr(x, local)).collect();
                match (name.as_str(), a.as_slice()) {
                    // The later or the earlier of two values of an ordered enum (§15.210), by a
                    // helper each language defines beside the function that calls it.
                    (crate::kw::MIN | crate::kw::MAX, [x, y]) if self.ordered_of(&args[0]).is_some() => {
                        let t = self.ordered_of(&args[0]).map(|en| self.enum_type(&en)).unwrap_or_default();
                        let f = if name == crate::kw::MIN { "_min_" } else { "_max_" };
                        Expr2 { text: format!("{f}{t}({}, {})", x.text, y.text), scale: 1 }
                    }
                    (crate::kw::MIN, [x, y]) | (crate::kw::MAX, [x, y]) => {
                        let s = lcm(x.scale, y.scale);
                        let f = if name == crate::kw::MIN { "_min" } else { "_max" };
                        Expr2 {
                            text: format!("{f}({}, {})", rescale(x, s), rescale(y, s)),
                            scale: s,
                        }
                    }
                    (crate::kw::ALLOCATE, [t, c, s]) => {
                        // `floor(T × C ÷ S)`, a whole number of the unit, so the answer is
                        // held at scale 1. The stored integers carry their own steps: the
                        // product is T·sT × C·sC and the divisor S·sS, so sT·sC/sS is what
                        // is left to undo. All three are non-negative (E117), so the
                        // truncation every target performs is this floor (§15.102).
                        let (num, den) = {
                            let (a, b) = (s.scale, t.scale * c.scale);
                            let g = a * b / lcm(a, b);
                            (a / g, b / g)
                        };
                        let top = match num {
                            1 => format!("({} * {})", t.text, c.text),
                            k => format!("({} * {} * {k})", t.text, c.text),
                        };
                        let bot = match den {
                            1 => s.text.clone(),
                            k => format!("({} * {k})", s.text),
                        };
                        Expr2 { text: format!("({top} // {bot})"), scale: 1 }
                    }
                    (m, [x, g]) if RoundMode::parse(m).is_some() => {
                        // Rounding snaps to a multiple of the grid, so the result is a whole
                        // number of grid steps and comes back at the grid's scale — which is
                        // what the checker records for it (`types::scale`). It used to stay at
                        // x's scale with no division, so a callee output rounded to 1円 from a
                        // value held in 1/20000 円 reached the applying rule 20,000 times too
                        // large in every language (§15.72).
                        let mode = RoundMode::parse(m).unwrap();
                        let s = lcm(x.scale, g.scale);
                        // A literal grid is folded to one number: `(1 * 100)` nested inside
                        // the division below is a product gofmt would tighten.
                        let gs = match g.text.parse::<i128>() {
                            Ok(v) => (v * (s / g.scale)).to_string(),
                            Err(_) => rescale(g, s),
                        };
                        let rounded = format!("_round_{}({}, {})", mode_fn(mode), rescale(x, s), gs);
                        if s == g.scale {
                            Expr2 { text: rounded, scale: s }
                        } else {
                            Expr2 { text: format!("({rounded} // {})", s / g.scale), scale: g.scale }
                        }
                    }
                    // The remainder of a whole number by a positive constant (§15.214). A whole
                    // number held at a finer scale is brought back to whole units first, which
                    // divides exactly; the remainder is then a whole number.
                    (crate::kw::MOD, [x, k]) => {
                        let whole = if x.scale == 1 { x.text.clone() } else { format!("({} // {})", x.text, x.scale) };
                        Expr2 { text: format!("_mod({whole}, {})", k.text), scale: 1 }
                    }
                    // The whole square root (§15.215) of the whole part: the argument is never
                    // negative (E139), so the division to whole units is the floor everywhere.
                    (crate::kw::ISQRT, [x]) => {
                        let whole = if x.scale == 1 { x.text.clone() } else { format!("({} // {})", x.text, x.scale) };
                        Expr2 { text: format!("_isqrt({whole})"), scale: 1 }
                    }
                    // Not a fallback: `check` accepts a fixed set of calls, so anything
                    // else here is a backend that was not taught a new one. Emitting 0
                    // would be a wrong answer that runs (§15.99).
                    (f, a) => panic!("gen: no case for {f} with {} arguments", a.len()),
                }
            }
            Expr::Bin(l, op, r, _) => {
                let a = self.expr(l, local);
                let b = self.expr(r, local);
                use BinOp::*;
                match op {
                    Add | Sub => {
                        let s = lcm(a.scale, b.scale);
                        let o = if *op == Add { "+" } else { "-" };
                        Expr2 { text: format!("({} {o} {})", rescale(&a, s), rescale(&b, s)), scale: s }
                    }
                    Mul => Expr2 {
                        text: format!("({} * {})", a.text, b.text),
                        scale: a.scale * b.scale,
                    },
                    Div => {
                        // Dividing by the constant k leaves the stored integer alone and
                        // multiplies the scale by k, so the result is exact. Emitting `//`
                        // would truncate here and Go's `/` would truncate the other way.
                        let k = b.text.parse::<i128>().ok().filter(|k| b.scale > 0 && k % b.scale == 0);
                        match k {
                            Some(k) => Expr2 { text: a.text, scale: a.scale * (k / b.scale) },
                            None => Expr2 { text: format!("({} // {})", a.text, b.text), scale: a.scale },
                        }
                    }
                    // Two values of an ordered enum compare by their places in its order
                    // (§15.210), which a helper beside the function gives.
                    Le | Lt | Ge | Gt if self.ordered_of(l).or_else(|| self.ordered_of(r)).is_some() => {
                        let t = self.ordered_of(l).or_else(|| self.ordered_of(r)).map(|en| self.enum_type(&en)).unwrap_or_default();
                        let o = match op {
                            Le => "<=",
                            Lt => "<",
                            Ge => ">=",
                            _ => ">",
                        };
                        Expr2 { text: format!("(_rank_{t}({}) {o} _rank_{t}({}))", a.text, b.text), scale: 1 }
                    }
                    Le | Lt | Ge | Gt | Eq => {
                        let s = lcm(a.scale, b.scale);
                        let o = match op {
                            Le => "<=",
                            Lt => "<",
                            Ge => ">=",
                            Gt => ">",
                            _ => "==",
                        };
                        Expr2 {
                            text: format!("({} {o} {})", rescale(&a, s), rescale(&b, s)),
                            scale: 1,
                        }
                    }
                }
            }
        }
    }
}

/// A local for a runner that must not shadow the function the runner has to call.
///
/// The TypeScript and Swift runners call the rule by its bare name, in the same scope as
/// the locals that hold the parsed line. A rule aliased `d` therefore bound the JSON object
/// to `d` and the call resolved to *that* — a type error in Swift, a runtime one in
/// TypeScript. Python, Ruby, Rust and Go all qualify the call (`m.d`, `Mod.d`, `r::d`,
/// `pkg.D`) and never had the problem.
///
/// Only the colliding name moves, so the output of every rule that does not collide is
/// unchanged.
fn runner_local(base: &str, fname: &str) -> String {
    let mut n = base.to_string();
    while n == fname {
        n.push('_');
    }
    n
}

/// The name of the intermediate that holds one output's value before rounding. One output
/// keeps the plain `raw` of the example in §8.2; a second needs a name of its own, or the
/// two assignments would land on the same variable.
/// The doc line of the public function, which decides the same thing as its traced twin
/// and drops the rows that matched.
fn plain_doc(name: &str, version: &str, twin: &str) -> String {
    tr!(
        "規則 {name} v{version}。{twin} と同じ判定で、当てはまった行を返さない。",
        "Rule {name} v{version}: the same decision as {twin}, without the rows that matched."
    )
}

/// The doc line of the traced function, which holds the branches.
fn traced_doc(name: &str, version: &str) -> String {
    tr!(
        "規則 {name} v{version}。分岐はもとの表の行と 1:1 に対応する。出力と、当てはまった行（表ごとに一つ、順に）を返す。",
        "Rule {name} v{version}. Each branch corresponds 1:1 to a row of the rule source. Returns the outputs and the rows that matched, one per table, in order."
    )
}

/// The doc line of the function that runs the walk of a rule that picks by weight (§15.225).
fn walked_doc() -> String {
    tr!(
        "規則の歩きを実行し、出力と当てはまった行に加えて、重みで選ぶ腕が重みを足すたびの合計を返す。確率の関数が読む。",
        "Runs the rule's walk, and returns, beside the outputs and the rows that matched, the running total of the weights each time the arm that picks by weight adds one. The chances function reads it."
    )
}

/// How a value travels on the wire (§10.2), which is all the record function has to know
/// about a type.
enum Wire {
    Enum,
    Bool,
    Int,
    Brand,
    Date,
    Str,
    Opt(Box<Wire>),
}

fn wire_of(ty: &Ty) -> Wire {
    match ty {
        Ty::Enum(_) => Wire::Enum,
        Ty::Bool => Wire::Bool,
        Ty::Date => Wire::Date,
        Ty::Str => Wire::Str,
        Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => Wire::Brand,
        Ty::Opt(t) => Wire::Opt(Box::new(wire_of(t))),
        _ => Wire::Int,
    }
}

/// Each of a record function's own parameters with the `, ` after it, before the ones every record
/// function takes: nothing for a rule that reads no input.
fn leading_params(params: &[String]) -> String {
    params.iter().map(|p| format!("{p}, ")).collect()
}

/// [`leading_params`] of parameters already joined: `params` and a comma, or nothing where there
/// are none (§15.330).
fn leading_param(params: &str) -> String {
    if params.is_empty() {
        String::new()
    } else {
        format!("{params}, ")
    }
}

/// The doc line of the record function.
fn record_doc() -> String {
    tr!(
        "一件を記録の形（docs/formats.md の fixtures）で一行の JSON にする。入力、出た値、当てはまった行。tag は空なら書かない。",
        "One call as a record in the fixtures format (docs/formats.md): the inputs, what came out, and the rows that matched, as one JSON line. An empty tag is left out."
    )
}

/// The doc line of the `Fired` type.
fn fired_doc() -> String {
    tr!("当てはまった行。表の名前と、1 から数えた行番号と、あればラベル。", "A row that matched: the table's name, its 1-based row number, and its label when it has one.")
}

fn raw_base(oi: usize) -> String {
    if oi == 0 {
        "raw".into()
    } else {
        format!("raw{}", oi + 1)
    }
}

fn mode_fn(m: RoundMode) -> &'static str {
    match m {
        RoundMode::Up => "up",
        RoundMode::Down => "down",
        RoundMode::Half => "half",
        RoundMode::Bankers => "bankers",
        RoundMode::HalfDown => "half_down",
    }
}

/// The mode a Python helper's name stands for: the inverse of [`mode_fn`], for the one
/// place that reads the Python spelling back (the SQL translator).
pub(crate) fn mode_from_fn(name: &str) -> Option<RoundMode> {
    [RoundMode::Up, RoundMode::Down, RoundMode::Half, RoundMode::Bankers, RoundMode::HalfDown].into_iter().find(|m| mode_fn(*m) == name)
}

fn rescale(e: &Expr2, to: i128) -> String {
    if e.scale == to {
        e.text.clone()
    } else {
        format!("({} * {})", e.text, to / e.scale)
    }
}

fn lcm(a: i128, b: i128) -> i128 {
    fn g(mut a: i128, mut b: i128) -> i128 {
        while b != 0 {
            let t = a % b;
            a = b;
            b = t;
        }
        a.max(1)
    }
    a / g(a, b) * b
}


// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

impl<'a> Gen<'a> {
    /// The class and member a value is generated as, read in the enum of the place it is
    /// written: two enums may both have `automatic` (§15.150). With no enum there to read it
    /// in, the first enum that has it.
    pub(crate) fn value_name(&self, v: &str, ty: &Ty) -> Option<&(String, String)> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            t => t,
        };
        if let Ty::Enum(e) = inner {
            if let Some(x) = self.value_names.get(&(e.clone(), v.to_string())) {
                return Some(x);
            }
        }
        self.c.enum_order.iter().find_map(|e| self.value_names.get(&(e.clone(), v.to_string())))
    }

    /// Whether a word is a value of some enum.
    pub(crate) fn is_value(&self, v: &str) -> bool {
        self.c.enums.values().any(|vs| vs.iter().any(|x| x == v))
    }

    /// A group's enum, as a type: what its members are read in.
    pub(crate) fn group_ty(&self, g: &str) -> Ty {
        self.c.groups.get(g).map(|(e, _)| Ty::Enum(e.clone())).unwrap_or(Ty::Unknown)
    }

    fn py_value(&self, v: &str, ty: &Ty) -> String {
        match self.value_name(v, ty) {
            Some((ty, alias)) => format!("{ty}.{}", alias.to_uppercase()),
            None => format!("{v:?}"),
        }
    }

    /// The constant the module holds a group in: `_remote` for `remote`. A group called `int` or
    /// `len` would make `_int` or `_len`, the names the module gives the builtins (§15.207), and
    /// one called `min` or `json_str` the module's own helpers; such a name gets one more `_`.
    fn py_group(&self, name: &str) -> String {
        let id = format!("_{}", self.ident(name));
        let taken = PY_BUILTINS.iter().any(|(alias, _)| *alias == id) || PY_HELPERS.contains(&id.as_str());
        if taken { format!("{id}_") } else { id }
    }

    /// The constant TypeScript's and JavaScript's module holds a group in: `_remote` for `remote`, with
    /// `_` after it where a helper of the module's has that name (`_min`, `_civil`, §15.336).
    fn ts_group(&self, name: &str) -> String {
        let mut id = format!("_{}", self.ident(name));
        let helpers = self.ts_helpers.get_or_init(|| own::ts_helpers(&own::Has::of(self.f, self.c)));
        while helpers.contains(&id) {
            id.push('_');
        }
        id
    }

    /// Render a cell as a Python condition. A don't-care yields None (no condition).
    fn py_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        let optional = matches!(ty, Ty::Opt(_));
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "True".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "False".into(),
                Lit::Word(w) => self.py_value(w, ty),
                Lit::Num(n) => self.int_lit(n, inner, col_scale),
                Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => format!("{s:?}"),
            }
        };
        let members = |ls: &Vec<Lit>| -> String {
            let mut out: Vec<String> = Vec::new();
            for l in ls {
                if let Lit::Word(w) = l {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| self.py_value(m, ty)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            format!("{{{}}}", out.join(", "))
        };
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => format!("{var} is None"),
            Cell::Prefix(ps) => ps
                .iter()
                .map(|p| format!("{var}.startswith({})", format!("{p:?}")))
                .collect::<Vec<_>>()
                .join(" or "),
            // A cell that names one group uses the set the module already declares, rather
            // than writing the members out again — otherwise that set is dead code.
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                format!("{var} in {}", self.py_group(w))
            }
            // An optional truth value is `None` when it is absent, and `not None` is true: the
            // `false` row took the absent value (§15.201).
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE && optional => format!("{var} is True"),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE && optional => format!("{var} is False"),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("not {var}"),
            Cell::Lit(l) => format!("{var} == {}", lit(l)),
            Cell::Set(ls) => format!("{var} in {}", members(ls)),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                format!("{var} not in {}", self.py_group(w))
            }
            Cell::Not(ls) => format!("{var} not in {}", members(ls)),
            Cell::Cmp(cs) => {
                let c = cs
                    .iter()
                    .map(|(o, l)| {
                        let op = match o {
                            CmpOp::Le => "<=",
                            CmpOp::Ge => ">=",
                            CmpOp::Lt => "<",
                            CmpOp::Gt => ">",
                        };
                        format!("{var} {op} {}", lit(l))
                    })
                    .collect::<Vec<_>>()
                    .join(" and ");
                // A comparison holds only of a value that is there: `None < 5` raises (§15.201).
                if optional { format!("{var} is not None and {c}") } else { c }
            }
            // A comparison on an ordered enum, written as it was: by the places in the order
            // (§15.210).
            Cell::Ranked { ops, .. } => {
                let en = self.c.ordered_enum(ty)?;
                let r = self.py_rank_fn(en);
                let c = ops
                    .iter()
                    .map(|(o, v)| format!("{r}({var}) {} {r}({})", o.word(), self.py_value(v, ty)))
                    .collect::<Vec<_>>()
                    .join(" and ");
                if optional { format!("{var} is not None and {c}") } else { c }
            }
        })
    }

    pub fn python(&self) -> String {
        self.apart_text(clash::Syntax::Python, |g| g.python_text())
    }

    fn python_text(&self) -> String {
        let mut o = self.header("#");
        // Import from typing only what is used. NamedTuple is needed only with multiple outputs.
        // `enum` only where the module declares an enum of its own: a rule whose enums all come
        // from an `enums` file imports them from its module instead (§15.212).
        let classes = self.own_enums().iter().any(|(jp, _)| self.c.enums.contains_key(jp));
        // Brands. They work with mypy and pyright and cost nothing at runtime. In a package of
        // ritsu's, those of money and of the quantities are the package's, imported (§15.248).
        let (own, units) = self.brands();
        // The readers take the object as the dict it is checked to be (§15.234).
        let mut typing: Vec<&str> = Vec::new();
        if self.projects() {
            typing.push("Any");
        }
        typing.push("NamedTuple");
        if !(own.is_empty() && self.package.is_some()) {
            typing.push("NewType");
        }
        if self.projects() {
            typing.push("cast");
        }
        // What a name of the rule's hides is read through the module it comes from, imported under
        // a name of its own (§15.336).
        typing.retain(|n| self.own(n) == *n);
        let mut imports = String::new();
        if let Some(b) = self.owns.get("import builtins") {
            imports.push_str(&format!("import builtins as {b}\n"));
        }
        if classes {
            match self.owns.get("import enum") {
                Some(e) => imports.push_str(&format!("import enum as {e}\n")),
                None => imports.push_str("import enum\n"),
            }
        }
        if let Some(t) = self.owns.get("import typing") {
            imports.push_str(&format!("import typing as {t}\n"));
        }
        if !typing.is_empty() {
            imports.push_str(&format!("from typing import {}\n", typing.join(", ")));
        }
        let future = match self.owns.get("annotations") {
            Some(a) => format!("annotations as {a}"),
            None => "annotations".to_string(),
        };
        o.push_str(&format!("from __future__ import {future}\n\n{imports}\n"));
        let new_type = self.own("NewType");
        for (b, ty) in &own {
            o.push_str(&format!("{b} = {new_type}(\"{b}\", int)  # {ty}\n"));
        }
        if !units.is_empty() {
            // handed on as the module's own (`X as X`), as the types of an `enums` file are
            let names: Vec<String> = units.keys().map(|b| format!("{b} as {b}")).collect();
            o.push_str(&format!("from {} import {}\n", ritsu_emit::package::py_path(&self.rules_dir(), "units"), names.join(", ")));
        }
        if !own.is_empty() || !units.is_empty() {
            o.push('\n');
        }

        // Enums. Each value carries its Japanese name, used in logs and in the wire format (§10).
        // An enum read from an `enums` file is that file's module's, and imported below
        // (§15.212).
        o.push_str(&self.py_enums());
        o.push_str(&self.py_enum_helpers());
        // Which values need which before them (§15.222).
        o.push_str(&self.py_prereqs());

        // The machine this function is one step of (§15.148). The host keeps the state; what
        // it needs from the rule is where a case starts and where one has ended.
        if let Some((en, init, fins)) = self.machine_consts() {
            let cls = self.enum_names.get(&en).cloned().unwrap_or_default();
            let set = if fins.is_empty() {
                "frozenset()".to_string()
            } else {
                format!("frozenset({{{}}})", fins.iter().map(|v| self.py_value(v, &Ty::Enum(en.clone()))).collect::<Vec<_>>().join(", "))
            };
            // The values carried along with the state, and where a case starts (§15.217).
            let mut more = String::new();
            for mc in self.more_carried() {
                let (ty, v) = match mc.kind() {
                    MoreKind::Enum => (self.py_ty(&mc.ty), self.py_value(&mc.word(), &mc.ty)),
                    MoreKind::Bool => ("bool".to_string(), if mc.bool_word() == "true" { "True".to_string() } else { "False".to_string() }),
                    MoreKind::Number => ("int".to_string(), mc.int.to_string()),
                    MoreKind::Branded => (brand_of(&mc.ty), format!("{}({})", brand_of(&mc.ty), mc.int)),
                };
                more.push_str(&format!("{}: {ty} = {v}\n", mc.upper()));
            }
            o.push_str(&format!(
                "INITIAL: {cls} = {}\nFINAL: frozenset[{cls}] = {set}\n{more}\n\ndef is_final(state: {cls}) -> bool:\n    \"\"\"{}\"\"\"\n    return state in FINAL\n\n\n",
                self.py_value(&init, &Ty::Enum(en.clone())),
                tr!("この状態で案件が終わっているか。", "Whether a case in this state has ended.")
            ));
        }

        // The sentence and the value travel apart (§15.95): the sentence and the value are put
        // together only when the error is printed, and a caller that wants the value does not
        // parse it back out. The value is the `repr()` of what was refused, taken where it is
        // refused and held as a `str`, with `None` for a refusal about no one value (§15.207):
        // an error that took any object, and a sentinel to say "no value", is a type and a
        // statement that code compiled ahead of time cannot have. The message is the same.
        // Where the rule's function is `super`, the classes call their base's `__init__` by the
        // base's name: `super()` with nothing in it finds its class only by that very name, and
        // mypy does not read a `super` reached through the builtins as the one it knows (§15.336).
        let init = |base: &str| if self.owns.contains_key("super") { format!("{}.__init__(self, what)", self.own(base)) } else { "super().__init__(what)".to_string() };
        o.push_str(&format!(
            "class RuleInputError({}):\n    \"\"\"{}\"\"\"\n\n\
             \x20   def __init__(self, what: str, value: str | None = None) -> None:\n\
             \x20       {}\n\
             \x20       self.what = what\n\
             \x20       self.value = value\n\n\
             \x20   def __str__(self) -> str:\n\
             \x20       return self.what if self.value is None else f\"{{self.what}}: {{self.value}}\"\n\n",
            self.own("ValueError"),
            tr!(
                "宣言した範囲の外。呼び出し側の契約違反。value は受け付けなかった値の repr() で、特定の値が理由でないときは None。",
                "Outside the declared input domain: a contract violation by the caller. `value` is the repr() of the value refused, or None when the refusal is about no one value."
            ),
            init("ValueError")
        ));
        o.push_str(&format!(
            "class RuleContradictionError({}):\n    \"\"\"{}\"\"\"\n\n\
             \x20   def __init__(self, what: str) -> None:\n\
             \x20       {}\n\
             \x20       self.what = what\n\n",
            self.own("AssertionError"),
            tr!("規則そのものの矛盾。呼び出し側の誤りではない。", "A contradiction in the rule itself, not a mistake by the caller."),
            init("AssertionError")
        ));
        o.push_str(&format!(
            "class Fired({}):\n    \"\"\"{}\"\"\"\n\n    table: str\n    row: int\n    label: str = \"\"\n\n",
            self.own("NamedTuple"),
            fired_doc()
        ));
        // The alias exists so that the local inside the function can be annotated without
        // naming `list`: an input aliased `list` shadows the builtin there, and mypy then
        // reads the annotation as the parameter (§15.33).
        o.push_str("_Trace = list[Fired]\n\n");

        // Groups
        for g in &self.f.groups {
            let ms: Vec<String> = g.members.iter().map(|m| self.py_value(&m.text, &self.group_ty(&g.name.text))).collect();
            o.push_str(&format!("{} = frozenset({{{}}})\n", self.py_group(&g.name.text), ms.join(", ")));
        }
        if !self.f.groups.is_empty() {
            o.push('\n');
        }

        // The functions first: which builtins they call decides which names are bound above them.
        let mut body = round_py();
        body.push_str(&arith::py_helpers(&arith::Uses::of(self.f)));
        let functions = self.py_fn();
        // W121 says a rule's name `_enumerate` meets the walk where `py_walks_count` says so.
        debug_assert_eq!(
            has_word(&functions, "_enumerate"),
            self.py_walks_count(),
            "rule {}: whether the walk counts the elements, as written and as `own` reads it",
            self.f.name.text
        );
        body.push_str(&functions);
        body.push_str("\n\n");
        body.push_str(&self.py_record());
        // The function that counts the answers over the points of the random inputs (§15.225).
        body.push_str(&self.py_chances());
        if self.projects() {
            body.push_str("\n\n");
            body.push_str(&self.py_from());
        }
        o.push_str(&py_builtins(&body));
        o.push_str(&body);
        pep8_blanks(&self.py_imports(o))
    }

    /// The `enum.Enum` classes of the enums this module declares, each with its order and its
    /// display names when it has them (§15.210, §15.211).
    fn py_enums(&self) -> String {
        let mut o = String::new();
        for (jp, ascii) in self.own_enums() {
            let Some(vals) = self.c.enums.get(&jp) else { continue };
            o.push_str(&format!("class {ascii}(enum.Enum):\n"));
            for v in vals {
                o.push_str(&format!("    {} = \"{v}\"\n", self.py_member(&jp, v)));
            }
            o.push('\n');
            o.push_str(&self.py_enum_fns(&jp, &ascii));
        }
        o
    }

    /// The member a value is: `ECONOMY`.
    fn py_member(&self, en: &str, v: &str) -> String {
        self.value_names.get(&(en.to_string(), v.to_string())).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| v.to_string())
    }

    /// The function that gives the place of a value of an ordered enum in its order:
    /// `era_rank`.
    fn py_rank_fn(&self, en: &str) -> String {
        format!("{}_rank", snake(&self.enum_type(en)))
    }

    /// The function that gives a value's display name: `unit_name_label`.
    fn py_label_fn(&self, en: &str) -> String {
        format!("{}_label", snake(&self.enum_type(en)))
    }

    /// The order of an ordered enum and the display names of one that gives any, as one
    /// function each over the members, if/elif in the order declared (§15.210, §15.211). A
    /// value with no display name is shown by its name, which is also what it is on the wire.
    fn py_enum_fns(&self, jp: &str, ascii: &str) -> String {
        let Some(vals) = self.c.enums.get(jp) else { return String::new() };
        let mut o = String::new();
        let chain = |o: &mut String, ret: &dyn Fn(usize, &String) -> String| {
            if vals.len() == 1 {
                o.push_str(&format!("    return {}\n", ret(0, &vals[0])));
                return;
            }
            for (i, v) in vals.iter().enumerate() {
                let m = self.py_member(jp, v);
                if i + 1 == vals.len() {
                    o.push_str(&format!("    else:\n        return {}\n", ret(i, v)));
                } else {
                    let kw = if i == 0 { "if" } else { "elif" };
                    o.push_str(&format!("    {kw} x is {ascii}.{m}:\n        return {}\n", ret(i, v)));
                }
            }
        };
        if self.c.ordered.contains(jp) {
            o.push_str(&format!(
                "\n\ndef {}(x: {ascii}) -> int:\n    \"\"\"{}\"\"\"\n",
                self.py_rank_fn(jp),
                tr!("x が {ascii} の順序の何番目か（0 から）。", "The place of x in the order of {ascii}, counting from 0.")
            ));
            chain(&mut o, &|i, _| i.to_string());
        }
        if let Some(labels) = self.c.labels.get(jp) {
            o.push_str(&format!(
                "\n\ndef {}(x: {ascii}) -> str:\n    \"\"\"{}\"\"\"\n",
                self.py_label_fn(jp),
                tr!("x の表示名。表示名の無い値は値の名前。", "The display name of x: a value without one is shown by its name.")
            ));
            chain(&mut o, &|i, v| str_lit(&labels.get(i).cloned().flatten().unwrap_or_else(|| v.clone())));
        }
        if !o.is_empty() {
            o.push_str("\n\n");
        }
        o
    }

    /// The helpers the functions call for the expressions over an ordered enum (§15.210): its
    /// place, and the later or the earlier of two values. Written only for what is called.
    fn py_enum_helpers(&self) -> String {
        let mut o = String::new();
        for en in self.c.enum_order.iter().filter(|e| self.c.ordered.contains(*e)) {
            let (rank, max, min) = self.enum_helpers(en);
            let (t, r) = (self.enum_type(en), self.py_rank_fn(en));
            if rank {
                o.push_str(&format!("\n\ndef _rank_{t}(x: {t}) -> int:\n    return {r}(x)\n"));
            }
            if max {
                o.push_str(&format!("\n\ndef _max_{t}(a: {t}, b: {t}) -> {t}:\n    return a if {r}(a) >= {r}(b) else b\n"));
            }
            if min {
                o.push_str(&format!("\n\ndef _min_{t}(a: {t}, b: {t}) -> {t}:\n    return a if {r}(a) <= {r}(b) else b\n"));
            }
        }
        if !o.is_empty() {
            o.push_str("\n\n");
        }
        o
    }

    /// The module with the import of each `enums` file it reads from, after the `typing` line
    /// (§15.212): the type, and the functions of its order and its display names. In a package of
    /// ritsu's the module beside it; from `rulec gen`, the module in the same directory, by its name,
    /// or by a relative import when the module goes inside a package (`--python-package`, §15.319).
    fn py_imports(&self, o: String) -> String {
        let foreign = self.foreign_owners();
        if foreign.is_empty() {
            return o;
        }
        let mut lines = String::new();
        for (module, dir, types) in foreign {
            let mut names: Vec<String> = Vec::new();
            for (jp, t) in &types {
                for n in [t.clone(), self.py_rank_fn(jp), self.py_label_fn(jp)] {
                    if has_word(&o, &n) && !names.contains(&n) {
                        names.push(n);
                    }
                }
            }
            // What the module has of each enum, as a module that declares the enum has it, after
            // what the rule calls (§15.319).
            for (jp, t) in &types {
                let mut handed = vec![t.clone()];
                if self.c.ordered.contains(jp) {
                    handed.push(self.py_rank_fn(jp));
                }
                if self.c.labels.contains_key(jp) {
                    handed.push(self.py_label_fn(jp));
                }
                for n in handed {
                    if !names.contains(&n) {
                        names.push(n);
                    }
                }
            }
            let from = if self.package.is_some() {
                self.place.py_path(&self.rules_dir(), &format!("{dir}rules/{module}"))
            } else if self.python_package {
                format!(".{module}")
            } else {
                module.clone()
            };
            // All of it is the module's to hand on (`X as X`, an explicit re-export): a caller, the
            // runner and the MCP server name it through the rule's module, as they name what the
            // rule's own enums have. An enum whose names meet the rule's own is handed on under
            // the type it takes here (`Size as Size2`, §15.326).
            let theirs = |n: &String| types.iter().map(|(jp, _)| self.theirs("python", jp, n)).find(|o| o != n).unwrap_or_else(|| n.clone());
            let names: Vec<String> = names.iter().map(|n| format!("{} as {n}", theirs(n))).collect();
            lines.push_str(&format!("from {from} import {}\n", names.join(", ")));
        }
        match o.find("from typing import") {
            Some(at) => {
                let end = o[at..].find('\n').map(|e| at + e + 1).unwrap_or(o.len());
                format!("{}{lines}{}", &o[..end], &o[end..])
            }
            // A module that reads all it reads of `typing` through the module (§15.336): after its
            // imports, which follow `from __future__ import …` and a blank line.
            None => match o.find("from __future__ import") {
                Some(at) => {
                    let line_end = |from: usize| o[from..].find('\n').map(|e| from + e + 1).unwrap_or(o.len());
                    let mut end = line_end(at);
                    if o[end..].starts_with('\n') {
                        end += 1;
                    }
                    while o[end..].starts_with("import ") || o[end..].starts_with("from ") {
                        end = line_end(end);
                    }
                    format!("{}{lines}{}", &o[..end], &o[end..])
                }
                None => format!("{lines}{o}"),
            },
        }
    }
}

/// A sentence as a Go string: `fmt.Sprintf` with the indices put in as `%d` where it names an element
/// by its index (§15.324), and a plain string otherwise (`plain` writes that one).
fn go_said_with(s: &str, here: &str, before: &str, plain: &dyn Fn(&str) -> String) -> String {
    if !names_element(s) {
        return plain(s);
    }
    let (mut form, mut args) = (String::new(), Vec::new());
    for p in pieces(s) {
        match p {
            Piece::Text(t) => form.push_str(&t.replace('%', "%%")),
            Piece::Here => {
                form.push_str("%d");
                args.push(here);
            }
            Piece::Before => {
                form.push_str("%d");
                args.push(before);
            }
        }
    }
    format!("fmt.Sprintf({form:?}, {})", args.join(", "))
}

/// [`go_said_with`] as the module writes a sentence: a plain one between quotes as it is.
fn go_said(s: &str, here: &str, before: &str) -> String {
    go_said_with(s, here, before, &|x| format!("\"{x}\""))
}

/// A sentence as the Rust module's `Said` (§15.324): `Said::of` the sentence, or where it names an
/// element by its index, `Said::at` and `Said::two` with the pieces of text around the indices.
/// The indices stay integers until the sentence is shown: a string built where the error is made
/// is a loop the model checker of the proof harness does not come back from (§15.95).
fn rs_said(s: &str, here: &str, before: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    let mut places: Vec<&str> = Vec::new();
    for p in pieces(s) {
        match p {
            Piece::Text(t) => words.push(format!("\"{t}\"")),
            Piece::Here => places.push(here),
            Piece::Before => places.push(before),
        }
    }
    match places.as_slice() {
        [] => format!("Said::of({})", words[0]),
        [i] => format!("Said::at({}, {i}, {})", words[0], words[1]),
        [i, j, ..] => format!("Said::two({}, {i}, {}, {j}, {})", words[0], words[1], words[2]),
    }
}

/// A sentence as a Swift string, the indices put in as `\(…)` (§15.324).
fn sw_said(s: &str, here: &str, before: &str) -> String {
    format!("\"{}\"", interpolate(s, here, before, "\\(", ")"))
}

/// A sentence as a Ruby string, the indices put in as `#{…}` (§15.324).
fn rb_said(s: &str, here: &str, before: &str) -> String {
    format!("\"{}\"", interpolate(s, here, before, "#{", "}"))
}

/// A sentence as the body of a TypeScript template literal, the indices put in as `${…}` (§15.324).
fn ts_said(s: &str, here: &str, before: &str) -> String {
    interpolate(s, here, before, "${", "}")
}

/// A sentence as a Python string: an f-string with the indices put in where it names an element by
/// its index (§15.324), and a plain string otherwise.
fn py_said(s: &str, here: &str, before: &str) -> String {
    if names_element(s) {
        format!("f\"{}\"", interpolate(s, here, before, "{", "}"))
    } else {
        format!("\"{s}\"")
    }
}

/// The builtins the functions that hold the rule's names call, and the name the module gives
/// each (§15.207). Such a function reaches a builtin only through its name here, so a name the
/// rule declares (an input aliased `int`, a table output called `len`) cannot stand in its way:
/// the function the rule's alias names, its traced twin, the record function and the
/// projection function have those names in scope. The helpers below them (rounding, `_json_str`,
/// `_proto`, …) have none, and call the builtins as they are; an alias of the rule or of an
/// enum that hides a builtin is W121's. `min` and `max` are the rule language's own functions
/// and are written as `_min` and `_max`.
const PY_BUILTINS: [(&str, &str); 12] = [
    ("_isinstance", "isinstance"),
    ("_int", "int"),
    ("_float", "float"),
    ("_bool", "bool"),
    ("_str", "str"),
    ("_repr", "repr"),
    ("_len", "len"),
    ("_any", "any"),
    ("_all", "all"),
    ("_sum", "sum"),
    ("_set", "set"),
    ("_enumerate", "enumerate"),
];

/// The other names the module defines at its top level beside the bound builtins: the helpers.
const PY_HELPERS: [&str; 40] = [
    "_Trace",
    "_min",
    "_max",
    "_round_down",
    "_round_up",
    "_round_half",
    "_round_half_down",
    "_round_bankers",
    "_div_round_down",
    "_div_round_up",
    "_div_round_half",
    "_div_round_half_down",
    "_div_round_bankers",
    "_mod",
    "_isqrt",
    "_json_str",
    "_civil",
    "_proto",
    "_dig",
    "_days",
    "_largest",
    "_smallest",
    // What the projection function reads the caller's object with (§15.234).
    "_at",
    "_opt",
    "_pget",
    "_seq_of",
    "_seq_at",
    "_int_of",
    "_int_at",
    "_int_pb",
    "_bool_of",
    "_bool_at",
    "_str_of",
    "_str_at",
    "_date_of",
    "_date_at",
    "_num_of",
    "_num_at",
    // Written until §15.244, which reads a value of each enum with a function of its own
    // (`_shipping_zone_of`, named in `from.rs`): kept, so that a group's constant keeps its name.
    "_enum_of",
    "_E",
];

/// Whether `word` stands in `text` as a name of its own, not as part of a longer one.
fn has_word(text: &str, word: &str) -> bool {
    let part = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(word).any(|(i, _)| {
        !text[..i].chars().next_back().is_some_and(part) && !text[i + word.len()..].chars().next().is_some_and(part)
    })
}

/// The bindings of the builtins that `body` calls, under the comment that says why: one line
/// each (`_int = int`), and only the ones the module uses.
fn py_builtins(body: &str) -> String {
    let used: String =
        PY_BUILTINS.iter().filter(|(alias, _)| has_word(body, alias)).map(|(alias, name)| format!("{alias} = {name}\n")).collect();
    if used.is_empty() {
        return String::new();
    }
    format!(
        "{}\n{used}\n",
        tr!(
            "# 規則の名前を持つ関数は、組み込み関数を、ここで付けた名前（`_int = int` など）か、下の補助関数\n\
             # （`_min`、`_max`）でしか呼ばない。入力・定義・表の出力の別名が `int`、`len`、`min`、\n\
             # `list` のような名前でも、壊れないようにするため。",
            "# A function that holds the rule's names calls a builtin function only through a name bound\n\
             # here (`_int = int` and the like) or through a helper below (`_min`, `_max`), so that an\n\
             # input, a definition or a table output aliased `int`, `len`, `min` or `list` does not break it."
        )
    )
}

/// The five modes of §7.3. Negative values and exact halves are pinned to the spec as well;
/// nothing is left to the language's native division (Python's `//` goes toward −∞, Go's
/// toward 0).
fn round_py() -> String {
    format!(
        r#"
def _min(a: int, b: int) -> int:
    return a if a < b else b


def _max(a: int, b: int) -> int:
    return a if a > b else b


def _round_down(x: int, g: int) -> int:
    """{down}"""
    v = abs(x) // g * g
    return -v if x < 0 else v


def _round_up(x: int, g: int) -> int:
    """{up}"""
    q, r = abs(x) // g, abs(x) % g
    v = (q + 1) * g if r != 0 else q * g
    return -v if x < 0 else v


def _round_half(x: int, g: int) -> int:
    """{half}"""
    q, r = abs(x) // g, abs(x) % g
    v = (q + 1) * g if 2 * r >= g else q * g
    return -v if x < 0 else v


def _round_half_down(x: int, g: int) -> int:
    """{half_down}"""
    q, r = abs(x) // g, abs(x) % g
    v = (q + 1) * g if 2 * r > g else q * g
    return -v if x < 0 else v


def _round_bankers(x: int, g: int) -> int:
    """{bankers}"""
    q, r = abs(x) // g, abs(x) % g
    if 2 * r > g or (2 * r == g and q % 2 == 1):
        q += 1
    v = q * g
    return -v if x < 0 else v

"#,
        down = tr!("0 へ寄せる。-4.8円 → -4円。", "Toward zero: -4.8 yen → -4 yen."),
        up = tr!("0 から遠ざける。-4.2円 → -5円。", "Away from zero: -4.2 yen → -5 yen."),
        half = tr!("半分ちょうどは 0 から遠ざける。", "An exact half goes away from zero."),
        half_down = tr!("半分ちょうどは 0 へ寄せる。", "An exact half goes toward zero."),
        bankers = tr!("半分ちょうどは偶数へ。", "An exact half goes to the even neighbor."),
    )
}

impl<'a> Gen<'a> {
    /// The Python annotation for a type. The public surface uses brands; enums are classes.
    fn py_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => self.enum_names.get(n).cloned().unwrap_or_else(|| "str".into()),
            Ty::Bool => "bool".into(),
            Ty::Date => "int".into(),
            Ty::Str => "str".into(),
            Ty::Opt(t) => format!("{} | None", self.py_ty(t)),
            _ => brand_of(ty),
        }
    }

    /// Storage scale of an output: a single integer in the declared unit (§7.1), and for a
    /// rate the step it travels at (§15.144).
    fn out_scale(&self, name: &str) -> i128 {
        match self.ty_of(name).present() {
            Ty::Rate => self.c.wire_scale(name),
            _ => 1,
        }
    }

    /// Whether output `oi` may answer that there is no value: an optional output the tables
    /// decide, whose column holds the absent value or one of the type it wraps (§15.249). Each
    /// language takes the column's value through the output's steps — the scale, the rounding,
    /// the brand — only when it is there.
    fn opt_out(&self, oi: usize) -> bool {
        let Some(od) = self.f.outputs.get(oi) else { return false };
        matches!(self.ty_of(&od.name.text), Ty::Opt(_)) && !(oi == 0 && self.f.result.is_some())
    }

    /// The scale an output's value is held at before its rounding: one that the step it
    /// travels at and its rounding grid both divide, so the rounding lands on whole units and
    /// the division down to the wire is exact (§15.144). A `result` comes at the scale of its
    /// own operands, which know nothing of either: `result 率 = 基本率` over whole percents,
    /// rounded to 0.1%, had a grid of no units at all.
    fn wire_target(&self, scale: i128, os: i128, od: &crate::ast::OutDecl) -> i128 {
        let ty = self.ty_of(&od.name.text);
        let to = lcm(scale, os);
        match od.rounding.as_ref().and_then(|rd| crate::types::lit_value_in_pub(&rd.grid, &ty)) {
            Some(g) => lcm(to, g.den),
            None => to,
        }
    }

    /// [`Self::wire_target`] applied to the value itself.
    fn onto_wire(&self, res: Expr2, os: i128, od: &crate::ast::OutDecl) -> Expr2 {
        let to = self.wire_target(res.scale, os, od);
        if to == res.scale {
            res
        } else {
            Expr2 { text: rescale(&res, to), scale: to }
        }
    }

    /// Indent a block that was emitted at the base level, so it can go inside a loop. The
    /// emitters write their own indentation into the format strings, so the body of a walk is
    /// built first and moved in afterwards — one place instead of a parameter threaded
    /// through every line of six backends (§15.56).
    fn indent_block(body: &str, by: &str) -> String {
        body.lines()
            .map(|l| if l.trim().is_empty() { l.to_string() } else { format!("{by}{l}") })
            .collect::<Vec<_>>()
            .join("\n")
            + if body.ends_with('\n') { "\n" } else { "" }
    }

    /// The fields of one element of sequence `s`, in declaration order.
    fn element_fields(&self, s: usize) -> Vec<&crate::ast::VarDecl> {
        self.f.elements.get(s).map(|e| e.fields.iter().collect()).unwrap_or_default()
    }

    /// The fields of sequence `s` marked `unique` (§15.290), in declaration order, for the walk
    /// whose summaries are `sums` when it is the first walk over `s`: that walk refuses an element
    /// with the value of one that an element before it had, right after the guards of the element.
    /// A later walk over the sequence reads only elements the first let through.
    pub(crate) fn unique_fields(&self, s: usize, sums: &[usize]) -> Vec<&crate::ast::VarDecl> {
        let first = self
            .walk_steps()
            .into_iter()
            .find_map(|w| match w {
                WalkStep::Walk { seq, sums: ss, .. } if seq == s => Some(ss.as_slice() == sums),
                _ => None,
            })
            .unwrap_or(true);
        if !first {
            return Vec::new();
        }
        self.element_fields(s).into_iter().filter(|fd| fd.unique.is_some()).collect()
    }

    /// The walk plan as the generators write it (§15.219): for each walk in the order it runs,
    /// its number, its sequence, the items of one element worked out on every element, and the
    /// summaries it reduces — with the items of one call that run after each number of walks.
    /// The sequence whose walk works out the item at `i` once for each element, if one does: what
    /// a sentence about the item names it under (§15.324).
    pub(crate) fn walked_in(&self, i: usize) -> Option<usize> {
        self.walk_steps().into_iter().find_map(|w| match w {
            WalkStep::Walk { seq, body, .. } if body.contains(&i) => Some(seq),
            _ => None,
        })
    }

    pub(crate) fn walk_steps(&self) -> Vec<WalkStep> {
        let st = &self.stages;
        let mut out = vec![WalkStep::Once(st.once(0))];
        for level in 1..=st.depth {
            for (w, s) in st.walks() {
                if w == level {
                    out.push(WalkStep::Walk { seq: s, body: st.body(self.f, s, level), sums: st.summaries_in(self.f, s, level) });
                }
            }
            out.push(WalkStep::Once(st.once(level)));
        }
        out
    }

    /// Whether the fold at `i` is the only summary of its walk: it then ends the loop when it
    /// stops, as a walk always did (§15.56). In a walk with other summaries it stops reading and
    /// the loop goes on for them (§15.219).
    pub(crate) fn fold_alone(&self, i: usize) -> bool {
        match self.stages.at(i) {
            crate::types::Stage::Each { seq, walk } => self.stages.summaries_in(self.f, seq, walk).len() == 1,
            _ => true,
        }
    }

    /// Whether the walk of the fold at `i`, alone in it, goes on past a `stop` (§15.328): the
    /// fold's work ends there, as do the items of one element, and the door still looks at each
    /// element after it — its fields inside their ranges and its `unique` fields — when the walk is
    /// the first over the sequence and the door has one of those to look at: a number, a date whose
    /// range writes an end, a field marked `unique`. The walk then binds the fold's `done` and goes
    /// on to the next element once it is set. A walk with nothing to look at ends at the `stop`.
    pub(crate) fn stop_keeps_door(&self, i: usize) -> bool {
        let crate::types::Stage::Each { seq, walk } = self.stages.at(i) else { return false };
        let Some(Item::Fold(fold)) = self.f.items.get(i) else { return false };
        if !self.fold_alone(i) || !fold.arms.iter().any(|(_, a, _)| matches!(a, Arm::Stop(_))) {
            return false;
        }
        let first = self.stages.walks().iter().filter(|(_, s)| *s == seq).map(|(w, _)| *w).min() == Some(walk);
        first
            && self.element_fields(seq).iter().any(|fd| {
                fd.unique.is_some()
                    || match self.ty_of(&fd.name.text).present() {
                        Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number => true,
                        Ty::Date => self.c.ranges.get(&fd.name.text).is_some_and(|(lo, hi)| lo.is_some() || hi.is_some()),
                        _ => false,
                    }
            })
    }

    /// The fold of the walk whose summaries are `sums` that goes on past its `stop` with the door
    /// alone (`stop_keeps_door`), if there is one: where the walk writes its `done`.
    pub(crate) fn walk_goes_past_stop(&self, sums: &[usize]) -> Option<usize> {
        sums.iter().copied().find(|&i| self.stop_keeps_door(i))
    }

    /// The name a fold's answer is bound to, its type, and whether that type is one of numbers.
    pub(crate) fn fold_target(&self, fold: &crate::ast::FoldDecl) -> (String, Ty) {
        let name = fold.answer_name(self.f).unwrap_or_default().to_string();
        let ty = self.ty_of(&name);
        (name, ty)
    }

    /// The rule's items — derived values, definitions and tables — as Python, at the base
    /// indentation. `local` decides how a name is spelled, which is where a walk puts the
    /// element's own fields.
    fn py_items(&self, local: &dyn Fn(&str) -> String, trace: &str, phase: Phase) -> String {
        let mut o = String::new();
        for (i, it) in self.f.items.iter().enumerate() {
            if !self.in_phase(i, it, phase) {
                continue;
            }
            match it {
                Item::Derived(d) => {
                    let e = self.expr(&d.expr, local);
                    o.push_str(&format!("    {} = {}  # {}\n", self.ident(&d.name.text), unparen(&py_truths(&e.text)), tr!("導出", "derived value")));
                }
                Item::Define(d) => {
                    let e = self.expr(&d.expr, local);
                    o.push_str(&format!("    {} = {}  # {}\n", self.ident(&d.name.text), unparen(&py_truths(&e.text)), tr!("定義", "definition")));
                    // The range the rule accepts for it, held where it is worked out (§15.313).
                    if let Some(test) = self.accepted_ends(d).and_then(|ends| py_out_of(&self.ident(&d.name.text), ends)) {
                        let n = self.ident(&d.name.text);
                        let said = py_said(&crate::door::out_of_range(&self.said_name(&d.name.text)), &self.here(), "");
                        o.push_str(&format!("    if {test}:\n        raise RuleInputError({said}, _repr({n}))\n"));
                    }
                }
                Item::Agg(_) | Item::Fold(_) => {}
                Item::Table(t) => {
                    if let Some(t) = self.c.table_at(t) {
                        o.push_str(&self.py_table(t, local, trace));
                    }
                }
            }
        }
        o
    }

    /// Every walk of a rule that walks sequences, in Python (§15.56, §15.58, §15.219): the cap on
    /// each sequence a count holds, the items before the first walk, and then each walk with the
    /// items that read what it answered after it. A rule with one walk is written as it always was:
    /// the fold's accumulators, or the counters, a loop with the items of one element inside it, and
    /// the answer bound to its name, so that the rounding and the return below are the ones every
    /// rule uses.
    fn py_walks(&self, outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = String::new();
        for (s, el) in self.f.elements.iter().enumerate() {
            if let Some(cap) = self.count_cap_of(s) {
                let seq = self.held(&el.name);
                o.push_str(&format!(
                    "    if _len({seq}) > {cap}:\n        raise RuleInputError(\"{}\", _repr(_len({seq})))\n",
                    crate::door::too_many(&el.name.text, cap)
                ));
            }
        }
        for step in self.walk_steps() {
            match step {
                // What a walk does not depend on is worked out once, before it (§15.208).
                WalkStep::Once(ix) => o.push_str(&self.py_items(outer, trace, Phase::Only(&ix))),
                WalkStep::Walk { seq, body, sums, .. } => o.push_str(&self.py_walk(seq, &body, &sums, outer, trace)),
            }
        }
        o
    }

    /// Whether a walk of the Python function counts the elements of its sequence, which it does
    /// with `enumerate` under the name the module gives it, `_enumerate` (§15.207): where a sentence
    /// of the walk names an element (§15.324). That is a field the walk checks (any type but
    /// `Unknown` — a string's kind too, §15.339), a `unique` field, a `define` worked out for each
    /// element with a range the rule accepts for it, or a `take_unique`. `own` lists the name for
    /// W121 by this; the module is held to it where it is written.
    pub(crate) fn py_walks_count(&self) -> bool {
        let checked = |fd: &&crate::ast::VarDecl| {
            matches!(
                self.ty_of(&fd.name.text).present(),
                Ty::Enum(_) | Ty::Bool | Ty::Str | Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date
            )
        };
        let fields = (0..self.f.elements.len()).any(|s| {
            let fs = self.element_fields(s);
            fs.iter().any(checked) || fs.iter().any(|fd| fd.unique.is_some())
        });
        let defines = self.f.items.iter().enumerate().any(|(i, it)| match it {
            Item::Define(d) => {
                self.walked_in(i).is_some() && self.accepted_ends(d).is_some_and(|(lo, hi)| lo.is_some() || hi.is_some())
            }
            _ => false,
        });
        let takes = self.f.folds().any(|fold| fold.arms.iter().any(|(_, arm, _)| matches!(arm, Arm::Take { unique: true, .. })));
        fields || defines || takes
    }

    /// One walk over sequence `s` (§15.219): each summary's accumulator, one pass over the elements
    /// with the guards of one element, the items of one element and each summary's step, and each
    /// summary's answer bound to its name once the loop is over.
    fn py_walk(&self, s: usize, body_items: &[usize], sums: &[usize], outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let el = &self.f.elements[s];
        let seq = self.held(&el.name);
        let fields: BTreeMap<String, String> = el.fields.iter().map(|fd| (fd.name.text.clone(), pub_name(&fd.name))).collect();
        let e = self.elem_var(s);
        // Inside the walk a field is read off the element.
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => format!("{e}.{f}"),
                None => outer(n),
            }
        };
        let mut o = String::new();
        let index = self.index_var(s);
        self.walk_begin(s, &index);
        let mut body = self.py_element_ranges(s, &local);
        let (seen, step) = self.py_unique(s, sums, &local);
        o.push_str(&seen);
        body.push_str(&step);
        // Past a `stop`, the door alone (§15.328).
        if let Some(i) = self.walk_goes_past_stop(sums) {
            body.push_str(&format!("    if {}:\n        continue\n", self.fold_names(i).done));
        }
        body.push_str(&self.py_items(&local, trace, Phase::Only(body_items)));
        let mut after = String::new();
        for &i in sums {
            match &self.f.items[i] {
                Item::Agg(d) if d.kind.extreme() => {
                    let (init, step) = self.py_extreme(d, &local);
                    o.push_str(&init);
                    body.push_str(&step);
                }
                Item::Agg(d) => {
                    o.push_str(&format!("    {} = 0  # {}\n", self.ident(&d.name.text), self.agg_word(d)));
                    let v = local(&d.column.text);
                    if d.kind == crate::ast::AggKind::Sum {
                        body.push_str(&format!("    {} += {v}  # {}\n", self.ident(&d.name.text), d.name.text));
                    } else {
                        let test = self.py_test(&v, &d.column.text, &d.value);
                        body.push_str(&format!("    if {test}:  # {}\n", d.name.text));
                        body.push_str(&format!("        {} += 1\n", self.ident(&d.name.text)));
                    }
                    if let Some((_, cap)) = self.sum_caps().into_iter().find(|(x, _)| x.name.text == d.name.text) {
                        let n = self.ident(&d.name.text);
                        after.push_str(&format!(
                            "    if {n} > {cap}:\n        raise RuleInputError(\"{}\", _repr({n}))\n",
                            crate::door::out_of_range(&d.name.text)
                        ));
                    }
                    // The bottom of its range, where a walk can come in under it (§15.314).
                    if let Some(floor) = self.walk_floor(d) {
                        let n = self.ident(&d.name.text);
                        after.push_str(&format!(
                            "    if {n} < {floor}:\n        raise RuleInputError(\"{}\", _repr({n}))\n",
                            crate::door::out_of_range(&d.name.text)
                        ));
                    }
                }
                Item::Fold(fold) => {
                    let (init, step, fin) = self.py_fold(i, fold, &seq, &local);
                    o.push_str(&init);
                    body.push_str(&step);
                    after.push_str(&fin);
                }
                _ => {}
            }
        }
        // The index of the element is bound where a sentence of the walk says it (§15.324).
        if self.walk_end() {
            o.push_str(&format!("    for {index}, {e} in _enumerate({seq}):\n"));
        } else {
            o.push_str(&format!("    for {e} in {seq}:\n"));
        }
        o.push_str(&Self::indent_block(&body, "    "));
        o.push_str(&after);
        o
    }

    /// A test on a column of one element, as `count` and the `where` of `max` and `min` write it:
    /// `<column> = <value>` for an enum, the column itself for a bool (§15.58).
    fn py_test(&self, v: &str, col: &str, value: &Option<crate::ast::Name>) -> String {
        let ty = self.ty_of(col);
        match (&ty, value) {
            (Ty::Bool, Some(w)) if w.text == crate::kw::FALSE => format!("not {v}"),
            (Ty::Bool, _) => v.to_string(),
            (_, Some(w)) => {
                let cls = self.py_ty(&ty);
                let member = self.value_name(&w.text, &ty).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| w.text.clone());
                format!("{v} is {cls}.{member}")
            }
            (_, None) => v.to_string(),
        }
    }

    /// A `max` or a `min` in Python (§15.219): the answer starts at the `empty` value, and the
    /// first element the `where` lets in replaces it; each one after it replaces it when it is
    /// larger (smaller). Returns the lines before the loop and the lines inside it.
    fn py_extreme(&self, d: &crate::ast::AggDecl, local: &dyn Fn(&str) -> String) -> (String, String) {
        let n = self.ident(&d.name.text);
        let set = self.temp(&format!("{n}_set"));
        let v = local(&d.column.text);
        let ty = self.ty_of(&d.column.text);
        let (start, better) = match self.extreme_enum(d) {
            Some(en) => {
                let t = self.enum_type(&en);
                let w = self.extreme_empty_word(d).unwrap_or_default();
                let op = if d.kind == crate::ast::AggKind::Max { ">" } else { "<" };
                (self.py_value(&w, &ty), format!("_rank_{t}({v}) {op} _rank_{t}({n})"))
            }
            None => {
                let op = if d.kind == crate::ast::AggKind::Max { ">" } else { "<" };
                (self.extreme_empty_int(d).unwrap_or_else(|| "0".into()), format!("{v} {op} {n}"))
            }
        };
        let init = format!("    {n} = {start}  # {}\n    {set} = False\n", self.agg_word(d));
        let mut step = String::new();
        let pad = match &d.filter {
            Some((w, want)) => {
                step.push_str(&format!("    if {}:  # {}\n", self.py_test(&local(&w.text), &w.text, want), d.name.text));
                "    "
            }
            None => "",
        };
        step.push_str(&format!("{pad}    if not {set} or {better}:\n"));
        step.push_str(&format!("{pad}        {n} = {v}\n{pad}        {set} = True\n"));
        (init, step)
    }

    /// A fold in Python (§15.56): the lines before the loop, the arms inside it, and the answer
    /// bound to its name after it. A fold alone in its walk ends the loop when it stops, unless the
    /// door is still to look at the elements after it (§15.328); one that shares its walk stops
    /// reading and lets the loop go on (§15.219).
    fn py_fold(&self, i: usize, fold: &crate::ast::FoldDecl, seq: &str, local: &dyn Fn(&str) -> String) -> (String, String, String) {
        let names = self.fold_names(i);
        let FoldNames { answer, stopped, done, taken, kept, best, held, key, taken_at, .. } = &names;
        let alone = self.fold_alone(i);
        // Ends the loop at a `stop`: a fold alone in its walk with no door to keep past it.
        let ends = alone && !self.stop_keeps_door(i);
        // `held` is the accumulator, and it is read only where the walk is over.
        let with_held = |n: &str| -> String { if n == crate::kw::HELD { held.clone() } else { local(n) } };
        let (out_name, out_ty) = self.fold_target(fold);
        let acc_ty = if out_ty.is_numeric() { "int".to_string() } else { self.py_ty(&out_ty) };
        // An answer is read in the type the fold answers with: a value of its enum is that
        // enum's member (§15.208).
        let val = |x: &Expr, l: &dyn Fn(&str) -> String| -> String {
            match x {
                Expr::Lit(Lit::Word(w), _) if w != crate::kw::TRUE && w != crate::kw::FALSE => self.py_value(w, &out_ty),
                _ => unparen(&py_truths(&self.expr(x, l).text)).to_string(),
            }
        };
        let text = |x: &Option<Expr>| -> String { x.as_ref().map(|x| val(x, &with_held)).unwrap_or_else(|| "0".to_string()) };
        // Only the accumulators an arm uses are declared: one nothing reads is an unused local to
        // a strict type checker.
        let takes = fold.arms.iter().any(|(_, a, _)| matches!(a, Arm::Take { .. } | Arm::TakeWeighted { .. }));
        let keeps = fold.arms.iter().any(|(_, a, _)| matches!(a, Arm::KeepMax { .. }));
        let weighted = fold.weighted();
        let draw = fold.draw.as_ref().map(|d| local(&d.text)).unwrap_or_default();

        let mut init = String::new();
        init.push_str(&format!("    {answer}: {acc_ty} = {}  # {}\n", text(&fold.empty), tr!("要素ゼロ件の答え", "the answer for no elements")));
        init.push_str(&format!("    {stopped} = False\n"));
        if !ends {
            init.push_str(&format!("    {done} = False\n"));
        }
        if takes {
            init.push_str(&format!("    {taken}: {acc_ty} | None = None\n"));
        }
        // The element a `take_unique` took, which the sentence says beside the one that matched
        // again (§15.324).
        let unique = fold.arms.iter().any(|(_, a, _)| matches!(a, Arm::Take { unique: true, .. }));
        if unique {
            init.push_str(&format!("    {taken_at} = 0\n"));
        }
        if weighted {
            init.push_str(&format!("    {}: int = 0  # {}\n", names.total, tr!("重みの合計", "the running total of the weights")));
            init.push_str(&format!("    {}: list[int] = []\n", names.cuts));
        }
        if keeps {
            init.push_str(&format!("    {kept}: {acc_ty} | None = None\n"));
            init.push_str(&format!("    {best}: int | None = None\n"));
        }

        // The verdict the table wrote for this element, and what the walk does about it.
        let mut arms = String::new();
        let v = local(&fold.verdict);
        for (name, arm, _) in &fold.arms {
            let cls = self.py_ty(&self.ty_of(&fold.verdict));
            let member = self.value_name(&name.text, &self.ty_of(&fold.verdict)).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| name.text.clone());
            arms.push_str(&format!("    if {v} is {cls}.{member}:  # {}\n", name.text));
            match arm {
                Arm::Next => arms.push_str("        pass\n"),
                Arm::Stop(None) if ends => arms.push_str("        break\n"),
                Arm::Stop(None) => arms.push_str(&format!("        {done} = True\n")),
                Arm::Stop(Some(x)) => {
                    arms.push_str(&format!("        {answer} = {}\n", val(x, local)));
                    if ends {
                        arms.push_str(&format!("        {stopped} = True\n        break\n"));
                    } else {
                        arms.push_str(&format!("        {stopped} = True\n        {done} = True\n"));
                    }
                }
                Arm::Take { expr, unique } => {
                    if *unique {
                        let here = self.here_used();
                        arms.push_str(&format!("        if {taken} is not None:\n"));
                        arms.push_str(&format!(
                            "            raise RuleContradictionError({})\n",
                            py_said(&crate::door::fold_contradiction(&fold.verdict, &fold.over, crate::door::BEFORE, crate::door::HERE), &here, taken_at)
                        ));
                        arms.push_str(&format!("        {taken} = {}\n", val(expr, local)));
                        arms.push_str(&format!("        {taken_at} = {here}\n"));
                    } else {
                        arms.push_str(&format!("        if {taken} is None:\n"));
                        arms.push_str(&format!("            {taken} = {}\n", val(expr, local)));
                    }
                }
                Arm::KeepMax { expr, key: k } => {
                    arms.push_str(&format!("        {key} = {}\n", unparen(&py_truths(&self.expr(k, local).text))));
                    arms.push_str(&format!("        if {best} is None or {key} > {best}:\n"));
                    arms.push_str(&format!("            {best} = {key}\n"));
                    arms.push_str(&format!("            {kept} = {}\n", val(expr, local)));
                }
                // The weight goes on the running total, and the element is taken where the drawn
                // value is below the total for the first time (§15.226). A total past int64 is
                // refused there, before another element is read: the other languages hold it in an
                // int64, and every language answers alike.
                Arm::TakeWeighted { expr, weight } => {
                    let (w, t) = (&names.weight, &names.total);
                    arms.push_str(&format!("        {w} = {}\n", unparen(&py_truths(&self.expr(weight, local).text))));
                    arms.push_str(&format!("        {t} += {w}\n"));
                    arms.push_str(&format!("        if {t} > 9223372036854775807:\n            raise RuleInputError(\"{}\")\n", crate::door::weights_past(&fold.over)));
                    arms.push_str(&format!("        {}.append({t})\n", names.cuts));
                    arms.push_str(&format!("        if {taken} is None and {draw} < {t}:\n"));
                    arms.push_str(&format!("            {taken} = {}\n", val(expr, local)));
                }
            }
        }
        let step = if alone { arms } else { format!("    if not {done}:  # {}\n{}", fold.verdict, Self::indent_block(&arms, "    ")) };

        // The drawn value of a pick by weight is below the total of the weights, unless they add up
        // to 0 (§15.226): the caller was to draw it so, and the door says so once the walk is over.
        let mut fin = String::new();
        if weighted {
            let t = &names.total;
            let what = fold.draw.as_ref().map(|d| crate::door::draw_past(&d.text)).unwrap_or_default();
            fin.push_str(&format!("    if {t} > 0 and {draw} >= {t}:\n        raise RuleInputError(\"{what}\", _repr({draw}))\n"));
        }
        // A sequence with nothing in it answers `empty` and never reaches `exhausted`; a walk
        // that ended on `stop with` has its answer already.
        fin.push_str(&format!("    if _len({seq}) > 0 and not {stopped}:\n"));
        let ex = text(&fold.exhausted);
        if ex.contains(held.as_str()) {
            let fallback = match (takes, keeps) {
                (true, true) => format!("{taken} if {taken} is not None else ({kept} if {kept} is not None else {})", text(&fold.empty)),
                (true, false) => format!("{taken} if {taken} is not None else {}", text(&fold.empty)),
                (false, true) => format!("{kept} if {kept} is not None else {}", text(&fold.empty)),
                (false, false) => text(&fold.empty),
            };
            fin.push_str(&format!("        {held} = {fallback}\n"));
        }
        fin.push_str(&format!("        {answer} = {ex}\n"));
        fin.push_str(&format!("    {} = {answer}\n", self.ident(&out_name)));
        (init, step, fin)
    }

    /// The head of the test of a field that may be absent, after [`Self::py_element_whole`] has
    /// bound the element again: mypy reads the new element's field as the optional it is declared,
    /// though the test is inside the `if … is not None` of the field, so the test says it again
    /// (§15.343). Nothing for a field that is always there, or one no whole float is made over.
    fn py_still_there(v: &str, optional: bool) -> String {
        if optional { format!("{v} is not None and ") } else { String::new() }
    }

    /// A whole float of an element's field is the integer it is, here as at the inputs (§15.205).
    /// The element is a NamedTuple: what reads it next reads one that holds the integer.
    fn py_element_whole(&self, v: &str, ty: &Ty) -> String {
        let Some((e, field)) = v.rsplit_once('.') else { return String::new() };
        let to = self.py_ty(ty);
        let int = if to == "int" { format!("_int({v})") } else { format!("{to}(_int({v}))") };
        format!("    if _isinstance({v}, _float) and _float({v}).is_integer():\n        {e} = {e}._replace({field}={int})\n")
    }

    /// The first look at each element of every sequence, in Python (§15.328): before any input is
    /// held to its range, each element in turn, and each of its fields in the order the rule
    /// declares them, of its kind — a value of its enum, a truth value, a string (§15.339), an
    /// integer, and a number one that int64 holds, as a reader of the wire takes it.
    fn py_element_kinds(&self) -> String {
        let mut out = String::new();
        for (s, el) in self.f.elements.iter().enumerate() {
            let fields: BTreeMap<String, String> = el.fields.iter().map(|fd| (fd.name.text.clone(), pub_name(&fd.name))).collect();
            let e = self.elem_var(s);
            let local = |n: &str| -> String { fields.get(n).map(|f| format!("{e}.{f}")).unwrap_or_else(|| n.to_string()) };
            let index = self.index_var(s);
            self.walk_begin(s, &index);
            let mut o = String::new();
            let here = self.here();
            for i in self.element_fields(s) {
                let v = local(&i.name.text);
                // The sentences name the field by its place on the element (§15.324).
                let said = || self.said_name(&i.name.text);
                let ty = self.ty_of(&i.name.text);
                // An optional field is looked at as the type it wraps when it has a value (§15.201).
                let optional = matches!(ty, Ty::Opt(_));
                let ty = ty.present().clone();
                let mut o = Guarded::new(&mut o, optional.then(|| (format!("    if {v} is not None:\n"), String::new())), "    ");
                match &ty {
                    Ty::Enum(n) => o.push_str(&format!(
                        "    if not _isinstance({v}, {}):\n        raise RuleInputError({}, _repr({v}))\n",
                        self.py_ty(&ty),
                        py_said(&crate::door::not_in_enum(&said(), n), &here, "")
                    )),
                    Ty::Bool => o.push_str(&format!(
                        "    if not _isinstance({v}, _bool):\n        raise RuleInputError({}, _repr({v}))\n",
                        py_said(&not_bool(&said()), &here, "")
                    )),
                    Ty::Str => o.push_str(&format!(
                        "    if not _isinstance({v}, _str):\n        raise RuleInputError({}, _repr({v}))\n",
                        py_said(&crate::door::not_string(&said()), &here, "")
                    )),
                    Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
                        if ty != Ty::Date {
                            o.push_str(&self.py_element_whole(&v, &ty));
                        }
                        o.push_str(&format!(
                            "    if not _isinstance({v}, _int) or _isinstance({v}, _bool):\n        raise RuleInputError({}, _repr({v}))\n",
                            py_said(&crate::door::not_integer(&said()), &here, "")
                        ));
                        if ty != Ty::Date {
                            o.push_str(&format!(
                                "    if {}{}:\n        raise RuleInputError({}, _repr({v}))\n",
                                Self::py_still_there(&v, optional),
                                py_past_int64(&v),
                                py_said(&crate::door::out_of_range(&said()), &here, "")
                            ));
                        }
                    }
                    _ => {}
                }
            }
            let used = self.walk_end();
            if !o.is_empty() {
                let seq = self.held(&el.name);
                let head = if used { format!("    for {index}, {e} in _enumerate({seq}):\n") } else { format!("    for {e} in {seq}:\n") };
                out.push_str(&head);
                out.push_str(&Self::indent_block(&o, "    "));
            }
        }
        out
    }

    /// The door's look at one element where the walk comes to it (§15.328): each field inside the
    /// range it declares, in the order the rule declares them, the same ends an input is held to,
    /// on the values the first look found of their kinds.
    fn py_element_ranges(&self, s: usize, local: &dyn Fn(&str) -> String) -> String {
        let mut o = String::new();
        let here = self.here();
        for i in self.element_fields(s) {
            let v = local(&i.name.text);
            // The sentences name the field by its place on the element (§15.324). Asked for only
            // where one is written: a walk binds the index only where a sentence reads it.
            let said = || self.said_name(&i.name.text);
            let ty = self.ty_of(&i.name.text);
            // An optional field is guarded as the type it wraps when it has a value (§15.201).
            let optional = matches!(ty, Ty::Opt(_));
            let ty = ty.present().clone();
            let mut o = Guarded::new(&mut o, optional.then(|| (format!("    if {v} is not None:\n"), String::new())), "    ");
            if let Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date = &ty {
                // The walk reads the element again from the sequence, so a whole float is made the
                // integer it is again.
                if ty != Ty::Date {
                    o.push_str(&self.py_element_whole(&v, &ty));
                }
                if let Some(test) = py_out_of(&v, crate::door::ends(self.c, &i.name.text, true)) {
                    o.push_str(&format!(
                        "    if {}{test}:\n        raise RuleInputError({}, _repr({v}))\n",
                        Self::py_still_there(&v, optional && ty != Ty::Date),
                        py_said(&crate::door::out_of_range(&said()), &here, "")
                    ));
                }
            }
        }
        o
    }

    /// The fields marked `unique` (§15.290), in Python: the values seen so far, kept from before
    /// the first walk over the sequence, and in the walk, after the guards of one element, the
    /// refusal of a value an element before it had, which names the element the value was first
    /// seen on (§15.324). An enum or a truth value is kept in a list: the walk stops at the first
    /// value seen twice, so the list never holds more than the values of the type, and a list is
    /// what a Python that compiles the module ahead of time takes where it refuses a set, whose
    /// order a compiled run would not reproduce. The list holds one value per element, at the
    /// element's index, and the refusal walks it to the value: such a compiler takes neither a
    /// dict in a function nor a list's `index()`. Another type is kept in a dict from the value to
    /// the index of the element it was first seen on, so that a long sequence is not walked once
    /// for each element. Returns the lines before the loop and the lines inside it.
    fn py_unique(&self, s: usize, sums: &[usize], local: &dyn Fn(&str) -> String) -> (String, String) {
        let seq = &self.f.elements[s].name.text;
        let (mut init, mut step) = (String::new(), String::new());
        for fd in self.unique_fields(s, sums) {
            let seen = self.temp(&format!("{}_seen", self.ident(&fd.name.text)));
            let v = local(&fd.name.text);
            let ty = self.ty_of(&fd.name.text);
            let here = self.here_used();
            let (start, keep, first, find) = match ty {
                Ty::Enum(_) | Ty::Bool => {
                    let at = self.temp(&format!("{}_first", self.ident(&fd.name.text)));
                    let find = format!("        {at} = 0\n        while {seen}[{at}] != {v}:\n            {at} += 1\n");
                    (format!("{seen}: list[{}] = []", self.py_ty(&ty)), format!("{seen}.append({v})"), at, find)
                }
                _ => (format!("{seen}: dict[{}, int] = {{}}", self.py_ty(&ty)), format!("{seen}[{v}] = {here}"), format!("{seen}[{v}]"), String::new()),
            };
            init.push_str(&format!("    {start}\n"));
            step.push_str(&format!(
                "    if {v} in {seen}:\n{find}        raise RuleInputError({}, _repr({v}))\n    {keep}\n",
                py_said(&crate::door::duplicate(seq, &fd.name.text, crate::door::BEFORE, crate::door::HERE), &here, &first)
            ));
        }
        (init, step)
    }

    /// A float whose value is a whole number, `1000.0` from a caller's JSON, taken as the integer
    /// it is before the integer check (§15.205), and kept as the type the parameter declares.
    fn py_whole(&self, v: &str, ty: &Ty) -> String {
        let to = self.py_ty(ty);
        let int = if to == "int" { format!("_int({v})") } else { format!("{to}(_int({v}))") };
        format!("    if _isinstance({v}, _float) and _float({v}).is_integer():\n        {v} = {int}\n")
    }

    /// The parameters of a function of the Python module that takes the rule's inputs and
    /// sequences, under the identifiers its body reads them by, and those identifiers, where some
    /// of them is bound apart from its name (§15.336): None where each is its name.
    fn py_held_params(&self) -> Option<(Vec<String>, Vec<String>)> {
        let apart = self.f.inputs.iter().map(|i| &i.name).chain(self.f.elements.iter().map(|el| &el.name)).any(|n| self.held(n) != pub_name(n));
        if !apart {
            return None;
        }
        let mut params: Vec<String> = self.f.inputs.iter().map(|i| format!("{}: {}", self.held(&i.name), self.py_ty(&self.ty_of(&i.name.text)))).collect();
        let mut args: Vec<String> = self.f.inputs.iter().map(|i| self.held(&i.name)).collect();
        for (s, el) in self.f.elements.iter().enumerate() {
            params.push(format!("{}: list[{}]", self.held(&el.name), self.element_type(s)));
            args.push(self.held(&el.name));
        }
        Some((params, args))
    }

    /// The function of its own that a public function `name` of the Python module hands its
    /// arguments on to (`Gen::py_held_params`): `_` before its name, and after it as many as it
    /// takes to be no name of the rule's.
    fn py_inner(&self, name: &str) -> String {
        let names = value_names(self.f);
        let mut n = format!("_{name}");
        while names.iter().any(|(x, _)| x.text == n || pub_name(x) == n) || self.binds.values().any(|b| *b == n) {
            n.push('_');
        }
        n
    }

    fn py_fn(&self) -> String {
        let fname = pub_name(&self.f.name);
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", pub_name(&i.name), self.py_ty(&self.ty_of(&i.name.text))))
            .collect();
        // The sequences a walk reads come last, after the values that hold for the whole call
        // (§15.56), in the order the rule declares them (§15.219).
        for (s, el) in self.f.elements.iter().enumerate() {
            params.push(format!("{}: list[{}]", pub_name(&el.name), self.element_type(s)));
        }
        let outs = &self.f.outputs;
        let ret = if outs.len() == 1 {
            self.py_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let mut o = String::new();
        let named_tuple = self.own("NamedTuple");

        // One element is a row of inputs, so it is the same NamedTuple an `Output` is.
        for (s, el) in self.f.elements.iter().enumerate() {
            let fields: Vec<(String, String)> = el.fields.iter().map(|fd| (pub_name(&fd.name), self.py_ty(&self.ty_of(&fd.name.text)))).collect();
            let doc = tr!("{} の一件", "one of {}", el.name.text);
            match py_named_apart(&self.element_type(s), &fields, &named_tuple) {
                Some(t) => o.push_str(&format!("# {doc}\n{t}\n")),
                None => {
                    o.push_str(&format!("class {}({named_tuple}):\n", self.element_type(s)));
                    o.push_str(&format!("    \"\"\"{doc}\"\"\"\n\n"));
                    for (n, ty) in &fields {
                        o.push_str(&format!("    {n}: {ty}\n"));
                    }
                }
            }
            o.push_str("\n\n");
        }

        // Multiple outputs are a NamedTuple (§8.5).
        if outs.len() > 1 {
            let fields: Vec<(String, String)> = outs.iter().map(|od| (pub_name(&od.name), self.py_ty(&self.ty_of(&od.name.text)))).collect();
            match py_named_apart("Output", &fields, &named_tuple) {
                Some(t) => o.push_str(&t),
                None => {
                    o.push_str(&format!("class Output({named_tuple}):\n"));
                    for (n, ty) in &fields {
                        o.push_str(&format!("    {n}: {ty}\n"));
                    }
                }
            }
            o.push('\n');
        }

        // The public function keeps the plain signature; the branches live in its traced
        // twin, which also returns the rows that matched (§15.33).
        let mut args: Vec<String> = self.f.inputs.iter().map(|i| pub_name(&i.name)).collect();
        for el in &self.f.elements {
            args.push(pub_name(&el.name));
        }
        let traced = format!("{fname}_traced");
        // What the plain function keeps the answer in is none of its parameters (§15.336).
        let (res, rest) = (free_of("out", &args), free_of("_", &args));
        o.push_str(&format!("def {}({}) -> {ret}:\n", self.fam(&fname), params.join(", ")));
        o.push_str(&format!("    \"\"\"{}\"\"\"\n", plain_doc(&self.f.name.text, &self.f.version, &traced)));
        o.push_str(&format!("    {res}, {rest} = {traced}({})\n    return {res}\n\n\n", args.join(", ")));
        o.push_str(&format!("def {traced}({}) -> tuple[{ret}, list[Fired]]:\n", params.join(", ")));
        o.push_str(&format!("    \"\"\"{}\"\"\"\n", traced_doc(&self.f.name.text, &self.f.version)));
        // Where some input is bound apart from its name inside the functions, the traced function
        // hands the arguments on to one of its own that takes them under the identifiers the
        // body reads: a caller still names them as the rule does (§15.336).
        let (params, args) = match self.py_held_params() {
            Some((held, held_args)) => {
                let inner = self.py_inner(&traced);
                o.push_str(&format!("    return {inner}({})\n\n\n", args.join(", ")));
                o.push_str(&format!("def {inner}({}) -> tuple[{ret}, list[Fired]]:\n", held.join(", ")));
                (held, held_args)
            }
            None => (params, args),
        };
        // A rule that picks by weight runs its walk in a function of its own, which gives back the
        // running totals of the weights too: what the function that counts the answers reads
        // (§15.225).
        let walked = self.weighted_fold().map(|(i, _)| (self.walked_fn("python"), self.fold_names(i).cuts));
        if let Some((w, _)) = &walked {
            let (a, b, c) = (free_of("out", &args), free_of("trace", &args), free_of("_", &args));
            o.push_str(&format!("    {a}, {b}, {c} = {w}({})\n    return {a}, {b}\n\n\n", args.join(", ")));
            o.push_str(&format!("def {w}({}) -> tuple[{ret}, list[Fired], list[int]]:\n", params.join(", ")));
            o.push_str(&format!("    \"\"\"{}\"\"\"\n", walked_doc()));
        }

        // Entry guards (§8.5). They enforce at runtime what the proof assumes: inputs lie within
        // their declared domains. An input with two or more things wrong with it is refused for
        // the first of them in the order every door looks (§15.328): each input of its kind, then
        // each element of each sequence, then each input inside its range, then the constraints.
        let local = |n: &str| -> String { self.ident(n) };
        for i in &self.f.inputs {
            let v = self.held(&i.name);
            let ty = self.ty_of(&i.name.text);
            // An optional input is guarded as the type it wraps whenever it has a value: its
            // range, its step and its type were never looked at, so 2025-01-01 was answered for a
            // date declared over 2026 (§15.201).
            let optional = matches!(ty, Ty::Opt(_));
            let ty = ty.present().clone();
            let mut o = Guarded::new(&mut o, optional.then(|| (format!("    if {v} is not None:\n"), String::new())), "    ");
            match &ty {
                // The enum by the name the rule declares it under, as every door says it (§15.204).
                Ty::Enum(n) => o.push_str(&format!(
                    "    if not _isinstance({v}, {}):\n        raise RuleInputError(\"{}\", _repr({v}))\n",
                    self.py_ty(&ty),
                    crate::door::not_in_enum(&i.name.text, n)
                )),
                // A truth value is `True` or `False` and nothing else (§15.203).
                Ty::Bool => o.push_str(&format!(
                    "    if not _isinstance({v}, _bool):\n        raise RuleInputError(\"{}\", _repr({v}))\n",
                    not_bool(&i.name.text)
                )),
                // A string is a `str` and nothing else (§15.339): a number given for it is equal
                // to no cell of the table, and fell to another row without a word.
                Ty::Str => o.push_str(&format!(
                    "    if not _isinstance({v}, _str):\n        raise RuleInputError(\"{}\", _repr({v}))\n",
                    crate::door::not_string(&i.name.text)
                )),
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
                    // Not an integer is refused before the range is looked at (§15.43): a
                    // float sits inside any range, and 18.3 for a rate declared in steps of
                    // 0.1% would otherwise be taken as 1.83% and answered without a word.
                    // `bool` is an `int` in Python, and a bool here is a mistake too. A float
                    // whose value is whole, `1000.0` from a caller's JSON, is that integer
                    // (§15.205).
                    if ty != Ty::Date {
                        o.push_str(&self.py_whole(&v, &ty));
                    }
                    o.push_str(&format!(
                        "    if not _isinstance({v}, _int) or _isinstance({v}, _bool):\n        raise RuleInputError(\"{}\", _repr({v}))\n",
                        crate::door::not_integer(&i.name.text)
                    ));
                    // A whole number int64 does not hold, where a reader of the wire refuses it.
                    if ty != Ty::Date {
                        o.push_str(&format!(
                            "    if {}:\n        raise RuleInputError(\"{}\", _repr({v}))\n",
                            py_past_int64(&v),
                            crate::door::out_of_range(&i.name.text)
                        ));
                    }
                }
                _ => {}
            }
        }
        o.push_str(&self.py_element_kinds());
        for i in &self.f.inputs {
            let v = self.held(&i.name);
            let ty = self.ty_of(&i.name.text);
            let optional = matches!(ty, Ty::Opt(_));
            let ty = ty.present().clone();
            let mut o = Guarded::new(&mut o, optional.then(|| (format!("    if {v} is not None:\n"), String::new())), "    ");
            match &ty {
                // The values the range of an ordered enum lets in, first to last (§15.210).
                Ty::Enum(n) => {
                    if let Some((lo, hi)) = self.c.enum_ranges.get(&i.name.text).and_then(|vs| Some((vs.first()?, vs.last()?))) {
                        let r = self.py_rank_fn(n);
                        o.push_str(&format!(
                            "    if not {r}({}) <= {r}({v}) <= {r}({}):\n        raise RuleInputError(\"{}\", _repr({v}))\n",
                            self.py_value(lo, &ty),
                            self.py_value(hi, &ty),
                            crate::door::out_of_range(&i.name.text)
                        ));
                    }
                }
                // A date is an ordinal, and its declared range is the universe the
                // completeness proof used, so it is guarded like any other number. Without
                // this, a date outside the declared range falls into whichever branch happens
                // to catch it and the caller gets a silently wrong answer.
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
                    // Each end the range writes, and nothing past ±(2^63−1) (§15.295). The bounds
                    // are in the units the argument arrives in, which for a rate is the number of
                    // steps. Taking the numerator alone turned `range >=0% <=100%` into
                    // `0 <= r <= 1` and refused every value above one step.
                    if let Some(test) = py_out_of(&v, crate::door::ends(self.c, &i.name.text, true)) {
                        o.push_str(&format!(
                            "    if {test}:\n        raise RuleInputError(\"{}\", _repr({v}))\n",
                            crate::door::out_of_range(&i.name.text)
                        ));
                    }
                }
                _ => {}
            }
        }

        o.push_str(&self.constraint_guards(&local, Lang::Py, "    ", |m| {
            format!("        raise RuleInputError(\"{m}\")\n")
        }));

        let trace = self.temp("trace");
        o.push_str(&format!("    {trace}: _Trace = []\n"));

        // Derived values and definitions, in declaration order (define-before-use, §5.1).
        // A walk runs the same body once per element, so it is written once and moved in
        // (§15.56).
        if self.f.walks() {
            o.push_str(&self.py_walks(&local, &trace));
        } else {
            o.push_str(&self.py_items(&local, &trace, Phase::All));
        }

        // Every output takes the same three steps: its source — the `result` expression for
        // the first output, the binding of its own name otherwise — brought to the wire
        // scale, then its declared rounding applied exactly once. The one-output and the
        // many-output returns each used to compute this, and the second copy had neither
        // step: a `result` was dropped from any rule with two outputs, and a value held in
        // hundredths came back a hundred times too large (§15.16).
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            // An optional output is its column's value when there is one, taken through the same
            // steps, and `None` when there is none (§15.249).
            let absent = self.opt_out(oi).then(|| local(out_name));
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let res = self.onto_wire(res, os, od);
            let res = Expr2 { text: py_truths(&res.text), scale: res.scale };
            let ty = self.ty_of(out_name);
            let body = match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    // Bind the expression to an intermediate instead of nesting it (the same shape as
                    // the example in §8.2). Criterion 1 demands readability, and deep nesting breaks
                    // the visual correspondence with the generated code. An optional value is
                    // brought over only when it is there, so it stays in the one expression.
                    if res.scale != os && absent.is_none() {
                        let raw = self.temp(&raw_base(oi));
                        o.push_str(&format!(
                            "    {raw} = {}  # {}\n",
                            unparen(&res.text),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("_round_{}({raw}, {}) // {}", mode_fn(m), grid_i, res.scale / os)
                    } else if res.scale != os {
                        format!("_round_{}({}, {}) // {}", mode_fn(m), unparen(&res.text), grid_i, res.scale / os)
                    } else {
                        format!("_round_{}({}, {})", mode_fn(m), res.text, grid_i)
                    }
                }
                None => res.text.clone(),
            };
            // The brand has to be put back on. Arithmetic on a `NewType` yields the supertype —
            // `YenInclTax * int` is `int` — so the value that comes out of the rounding helper
            // is a plain `int` and assigning it to a branded output is exactly the mistake the
            // brand exists to catch. Eight of the thirteen corpus rules did not pass
            // `mypy --strict` because of this (§15.22). `NewType.__call__` returns its argument.
            let branded = if matches!(ty.present(), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                format!("{}({body})", brand_of(ty.present()))
            } else {
                body
            };
            finals.push(match absent {
                Some(v) if branded != v => format!("None if {v} is None else {branded}"),
                _ => branded,
            });
        }
        let branded = finals;
        let cuts = walked.as_ref().map(|(_, c)| format!(", {c}")).unwrap_or_default();
        if outs.len() == 1 {
            o.push_str(&format!("    return {}, {trace}{cuts}\n", branded[0]));
        } else {
            // Multiple outputs are a NamedTuple (§8.5); each carries its own rounding.
            o.push_str(&format!("    return Output({}), {trace}{cuts}\n", branded.join(", ")));
        }
        o
    }

    fn py_table(&self, t: &Table, local: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = format!("    # {}\n", self.table_head(t));
        // An optional column is annotated before its rows: the first row to bind it could be
        // `None` or a value, and mypy takes a local's type from its first binding (§15.249).
        for oc in &t.outputs {
            if let Ty::Opt(inner) = self.ty_of(&oc.name.text) {
                let raw = match inner.as_ref() {
                    Ty::Enum(_) | Ty::Bool | Ty::Str => self.py_ty(&inner),
                    _ => "int".to_string(),
                };
                o.push_str(&format!("    {}: {raw} | None\n", self.ident(&oc.name.text)));
            }
        }
        // What each row binds each column to.
        let value = |ri: usize, oi: usize| -> String {
            let oc = &t.outputs[oi];
            match t.rows[ri].outs.get(oi) {
                Some(OutCell::Lit(Lit::Num(n))) => {
                    let ty = self.ty_of(&oc.name.text);
                    self.int_lit(n, &ty, self.scale(&oc.name.text))
                }
                Some(OutCell::Lit(l)) => match l {
                    Lit::Word(w) if w == crate::kw::TRUE => "True".into(),
                    Lit::Word(w) if w == crate::kw::FALSE => "False".into(),
                    Lit::Word(w) => self.py_value(w, &self.ty_of(&oc.name.text)),
                    Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                    Lit::Str(x) => str_lit(x),
                    _ => "0".into(),
                },
                Some(OutCell::Name(w)) => {
                    if w == crate::kw::TRUE {
                        "True".into()
                    } else if w == crate::kw::FALSE {
                        "False".into()
                    } else if self.is_value(w) {
                        self.py_value(w, &self.ty_of(&oc.name.text))
                    } else {
                        let v = local(w);
                        match self.rescaled(w, &oc.name.text, v.clone()) {
                            // A value that may be absent is scaled only when it is there.
                            r if r != v && matches!(self.ty_of(w), Ty::Opt(_)) => format!("None if {v} is None else {r}"),
                            r => r,
                        }
                    }
                }
                Some(OutCell::Nothing) => "None".into(),
                None => "0".into(),
            }
        };
        let values: Vec<Vec<String>> = (0..t.rows.len()).map(|ri| (0..t.outputs.len()).map(|oi| value(ri, oi)).collect()).collect();
        // A column of money, a quantity or a rate is read as the whole number it holds whatever its
        // rows. mypy takes a local's type from its first binding, so a first row handing on a name
        // as it stands — an input or a field of an element, of the unit's type — made the column
        // that type, and `price` then `0` failed under --strict where `0` then `price` passed. Such
        // a column is annotated before its rows; a first row of a whole number, or of a value
        // brought to another step, is read as one already (§15.329). A column of an enum or a bool
        // is annotated with its type the same way: where the first row tests the name it hands on,
        // mypy has narrowed that name to the one value tested (`Literal[Level.CALM]`), and the
        // column's other values failed (§15.340).
        for (oi, oc) in t.outputs.iter().enumerate() {
            let named = match t.rows.first().and_then(|r| r.outs.get(oi)) {
                Some(OutCell::Name(w)) => w != crate::kw::TRUE && w != crate::kw::FALSE && !self.is_value(w) && values[0][oi] == local(w),
                _ => false,
            };
            let declared = match self.ty_of(&oc.name.text) {
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => Some("int".to_string()),
                Ty::Enum(n) if self.enum_names.contains_key(&n) => Some(self.py_ty(&Ty::Enum(n))),
                Ty::Bool => Some("bool".to_string()),
                _ => None,
            };
            if let (true, Some(declared)) = (named, declared) {
                o.push_str(&format!("    {}: {declared}\n", self.ident(&oc.name.text)));
            }
        }
        for (ri, row) in t.rows.iter().enumerate() {
            // §8.1 criterion 1: write out every cell. Conditions already known to be true from
            // earlier branches are not dropped either.
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.py_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            let cond = if conds.is_empty() { "True".into() } else { conds.join(" and ") };
            let kw = if ri == 0 { "if" } else { "elif" };
            let cells: Vec<String> = row
                .cells
                .iter()
                .map(cell_src)
                .chain(row.outs.iter().map(out_src))
                .collect();
            o.push_str(&format!("    {kw} {cond}:  # {}\n", self.row_head(t, ri, &cells.join(" | "))));
            for (oi, oc) in t.outputs.iter().enumerate() {
                o.push_str(&format!("        {} = {}\n", self.ident(&oc.name.text), values[ri][oi]));
            }
            let (tn, rn, lbl) = self.fired_of(t, ri);
            if lbl.is_empty() {
                o.push_str(&format!("        {trace}.append(Fired({tn:?}, {rn}))\n"));
            } else {
                o.push_str(&format!("        {trace}.append(Fired({tn:?}, {rn}, {lbl:?}))\n"));
            }
        }
        o.push_str(&format!(
            "    else:\n        raise {}(\"{}\")\n",
            self.own("AssertionError"),
            tr!("到達不能: 完全性は rulec が静的に検査済み", "unreachable: completeness was statically checked by rulec")
        ));
        o.push_str(&self.guards(t, local, Lang::Py, "    ", |name, i, j| {
            format!(
                "        raise RuleContradictionError(\"{}\")\n",
                tr!("表 {name}: {i} と {j} が同時に当てはまりました", "table {name}: {i} and {j} matched at the same time")
            )
        }));
        o
    }

    /// The door for the `constraint` lines (§15.55).
    ///
    /// The checker was told these combinations do not happen, and it believed it: it demanded
    /// no row for them. So a caller that sends one reaches branches that have nothing to
    /// answer with. Refusing at the door is the same line as the range and the integer checks
    /// — the entry guard enforces exactly what the proof assumed (§15.43, §15.45).
    ///
    /// Both sides are inputs of one type, but not always of one step: a rate in 1% steps and
    /// one in 0.1% steps hold 50% as 50 and as 500, and comparing those integers as they stand
    /// let 50% ≤ 10% through. So each side is brought to the step the two share first
    /// (§15.139); E108 has already proved the product fits.
    ///
    /// A side that adds inputs up (§15.216) adds them on that step too. `tight` writes the sum
    /// and the products without spaces, the way `gofmt` prints the operands of a comparison.
    fn constraint_sides(&self, k: &crate::ast::Constraint, name: &dyn Fn(&str) -> String, tight: bool) -> (String, String) {
        let common = k.names().map(|n| self.scale(n).max(1)).fold(1, lcm);
        let (plus, times) = if tight { ("+", "*") } else { (" + ", " * ") };
        let term = |n: &String| {
            let s = self.scale(n).max(1);
            if common / s == 1 { name(n) } else { format!("{}{times}{}", name(n), common / s) }
        };
        let side = |ns: &[String]| ns.iter().map(term).collect::<Vec<_>>().join(plus);
        (side(&k.left), side(&k.right))
    }

    fn constraint_guards(
        &self,
        local: &dyn Fn(&str) -> String,
        lang: Lang,
        indent: &str,
        raise: impl Fn(&str) -> String,
    ) -> String {
        let sp = lang.spelling();
        let mut o = String::new();
        for k in &self.f.constraints {
            let (a, b) = self.constraint_sides(k, local, lang == Lang::Go);
            let op = k.op.word();
            let said = k.said();
            o.push_str(&format!(
                "{indent}{} {}\n",
                sp.comment,
                tr!("制約: {said}", "constraint: {said}")
            ));
            o.push_str(&format!("{indent}{}\n", (sp.if_head)(&format!("{}({a} {op} {b})", lang.not()))));
            o.push_str(&raise(&crate::door::constraint(&said)));
            if !sp.close.is_empty() {
                o.push_str(&format!("{indent}{}\n", sp.close));
            }
        }
        for n in self.f.door_nevers() {
            let said = n.said();
            let conds = self.never_conds(n, lang, local);
            o.push_str(&format!("{indent}{} {}\n", sp.comment, tr!("起こらない組み合わせ: {said}", "does not happen: {said}")));
            o.push_str(&format!("{indent}{}\n", (sp.if_head)(&conds.join(sp.and))));
            o.push_str(&raise(&crate::door::never(&said)));
            if !sp.close.is_empty() {
                o.push_str(&format!("{indent}{}\n", sp.close));
            }
        }
        o.push_str(&self.day_guards(local, lang, indent, &raise));
        o
    }

    /// What one `never` line tests, one condition for each cell, as a row of a table tests it
    /// (§15.282).
    fn never_conds(&self, n: &crate::ast::NeverLine, lang: Lang, local: &dyn Fn(&str) -> String) -> Vec<String> {
        n.conds
            .iter()
            .filter_map(|k| {
                let name = &k.name.text;
                self.cell(lang, &k.cell, &local(name), &self.ty_of(name), self.scale(name))
            })
            .collect()
    }

    /// The door for a date input whose range is the days of a koyomi date (§15.174): the checks
    /// held the tables to those days and no other, so any other day is refused here, as a
    /// combination a `constraint` rules out is. The days are written as runs of consecutive
    /// ordinals, so a set of business days stays a few comparisons long.
    fn day_guards(&self, local: &dyn Fn(&str) -> String, lang: Lang, indent: &str, raise: &dyn Fn(&str) -> String) -> String {
        let sp = lang.spelling();
        let mut o = String::new();
        let mut names: Vec<&String> = self.c.day_sets.keys().collect();
        names.sort();
        for name in names {
            let set = &self.c.day_sets[name];
            let v = local(name);
            let lit = |n: i64| if lang == Lang::Ts { format!("{n}n") } else { n.to_string() };
            let eq = match lang {
                Lang::Ts => "===",
                Lang::Sql => "=",
                _ => "==",
            };
            let or = match lang {
                Lang::Py => " or ",
                Lang::Sql => " OR ",
                _ => " || ",
            };
            let tests: Vec<String> = day_runs(&set.days)
                .into_iter()
                .map(|(a, b)| if a == b { format!("{v} {eq} {}", lit(a)) } else { format!("({}{}{})", format!("{} <= {v}", lit(a)), sp.and, format!("{v} <= {}", lit(b))) })
                .collect();
            let from = set.from.as_ref().map(|fr| format!("koyomi \"{}\" date {}", fr.file, fr.date)).unwrap_or_default();
            o.push_str(&format!("{indent}{} {}\n", sp.comment, tr!("{name} は {from} がとる日だけ（{} 日）", "{name} takes only the days {from} comes to ({} days)", set.days.len())));
            o.push_str(&format!("{indent}{}\n", (sp.if_head)(&format!("{}({})", lang.not(), tests.join(or)))));
            // The message goes inside a string literal of every language as it is, like the
            // constraint's: the file is written without the characters a literal would need escaped.
            let (file, date) = set.from.as_ref().map(|fr| (fr.file.replace(['"', '\\', '`', '\'', '$'], ""), fr.date.clone())).unwrap_or_default();
            o.push_str(&raise(&crate::door::not_a_day(&name, &file, &date)));
            if !sp.close.is_empty() {
                o.push_str(&format!("{indent}{}\n", sp.close));
            }
        }
        o
    }

    /// Guards (§8.1). Emitted only for row pairs whose exclusivity could not be proven statically.
    /// No vector exercises this branch: had such an input been constructible, it would have been
    /// an E105.
    fn guards(
        &self,
        t: &Table,
        local: &dyn Fn(&str) -> String,
        lang: Lang,
        indent: &str,
        raise: impl Fn(&str, &str, &str) -> String,
    ) -> String {
        let Some(pairs) = self.w114.get(&t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default()) else {
            return String::new();
        };
        let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        let guard = tr!("ガード", "guard");
        let mut o = String::new();
        for (i, j) in pairs {
            let (ni, nj) = (self.row_name(t, *i), self.row_name(t, *j));
            let mut conds: Vec<String> = Vec::new();
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                let ty = self.ty_of(col);
                let sc = self.scale(col);
                for r in [*i, *j] {
                    let Some(cell) = t.rows[r].cells.get(ci) else { continue };
                    let c = self.cell(lang, cell, &local(col), &ty, sc);
                    if let Some(c) = c {
                        if !conds.contains(&c) {
                            conds.push(c);
                        }
                    }
                }
            }
            if conds.is_empty() {
                continue;
            }
            let sp = lang.spelling();
            let joined = conds.join(sp.and);
            o.push_str(&format!(
                "{indent}{} {guard}: {}\n",
                sp.comment,
                tr!(
                    "W114（表 {name} {ni} × {nj}）。重ならないことを静的に証明できなかった行の対",
                    "W114 (table {name}, {ni} × {nj}): a pair of rows whose exclusivity could not be proven statically"
                )
            ));
            o.push_str(&format!("{indent}{}\n", (sp.if_head)(&joined)));
            o.push_str(&raise(&name, &ni, &nj));
            if !sp.close.is_empty() {
                o.push_str(&format!("{indent}{}\n", sp.close));
            }
        }
        o
    }
}

/// The source spelling of a cell, used in diagnostics and in generated comments.
fn cell_src(c: &Cell) -> String {
    match c {
        Cell::DontCare => "-".into(),
        Cell::Nothing => crate::kw::NONE.into(),
        Cell::Lit(l) => lit_src(l),
        Cell::Set(ls) => ls.iter().map(lit_src).collect::<Vec<_>>().join(", "),
        Cell::Not(ls) => format!("{}: {}", crate::kw::NOT, ls.iter().map(lit_src).collect::<Vec<_>>().join(", ")),
        Cell::Prefix(ps) => format!("{} {}", crate::kw::STARTS_WITH, ps.iter().map(|p| format!("\"{p}\"")).collect::<Vec<_>>().join(", ")),
        Cell::Ranked { ops, .. } => crate::ast::ranked_text(ops),
        Cell::Cmp(cs) => cs
            .iter()
            .map(|(o, l)| {
                let op = match o {
                    CmpOp::Le => "<=",
                    CmpOp::Ge => ">=",
                    CmpOp::Lt => "<",
                    CmpOp::Gt => ">",
                };
                format!("{op}{}", lit_src(l))
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn out_src(o: &OutCell) -> String {
    match o {
        OutCell::Lit(l) => lit_src(l),
        OutCell::Name(n) => n.clone(),
        OutCell::Nothing => crate::kw::NONE.into(),
    }
}

/// A string literal for the generated code. Every target but SQL writes a string between
/// double quotes and escapes `"` and `\\` the same way, and a `.rule` string cannot hold a
/// newline (the lexer reads it on one line), so one helper serves all of them.
///
/// Until §15.88 there was none: the lowering of an output cell had no arm for `Lit::Str`, so
/// every string fell through to `_ => "0"` and a `string` output answered **0** in every
/// language, with the row's own comment beside it printing the value it should have had.
pub(crate) fn str_lit(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn lit_src(l: &Lit) -> String {
    match l {
        Lit::Num(n) => n.raw.clone(),
        Lit::Word(w) => w.clone(),
        Lit::Str(s) => format!("\"{s}\""),
        Lit::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02}"),
    }
}

// ---------------------------------------------------------------------------
// Go
// ---------------------------------------------------------------------------

impl<'a> Gen<'a> {
    fn go_value(&self, v: &str, ty: &Ty) -> String {
        match self.value_name(v, ty) {
            Some((ty, alias)) => format!("{ty}{alias}"),
            None => format!("{v:?}"),
        }
    }

    /// Go's zero value for a return type, for the early return of a guard. `return 0` does
    /// not compile when the rule answers with a boolean or a string.
    fn go_zero(&self, ty: &Ty) -> String {
        match ty {
            Ty::Bool => "false".into(),
            Ty::Str => "\"\"".into(),
            Ty::Opt(_) => "nil".into(),
            _ => "0".into(),
        }
    }

    /// A blank assignment for a value nothing downstream reads. The value is still computed,
    /// so that the generated code and the rule stay line for line, but Go will not compile a
    /// local that is never read — so say out loud that it is on purpose. W111 has already
    /// named the declaration.
    fn go_unread(&self, name: &str) -> String {
        if self.is_read(name) {
            String::new()
        } else {
            format!("\t_ = {}\n", self.ident(name))
        }
    }

    /// The same line in Rust, for the same values: rustc warns of a local nothing reads, and a
    /// crate built with `-D warnings` stops there. A definition nothing reads is said by nothing
    /// before here, an output of a rule this one applies and does not read among them (§15.340).
    fn rs_unread(&self, name: &str) -> String {
        if self.is_read(name) {
            String::new()
        } else {
            format!("    let _ = {};\n", self.ident(name))
        }
    }

    /// The same line in Ruby, which warns under `-w` of a local assigned and never read (§15.340).
    fn rb_unread(&self, name: &str) -> String {
        if self.is_read(name) {
            String::new()
        } else {
            format!("    _ = {}\n", self.ident(name))
        }
    }

    /// The same line in TypeScript and JavaScript: `tsc --noUnusedLocals` refuses a local nothing
    /// reads, and `void` reads it (§15.342). `+=` is a read there, as in Ruby, so what a walk adds
    /// up needs none.
    fn ts_unread(&self, name: &str) -> String {
        if self.is_read(name) {
            String::new()
        } else {
            format!("  void {};\n", self.ident(name))
        }
    }

    /// Whether anything downstream reads a table's output column. A column nothing reads is
    /// still assigned, so that the branch and the row stay 1:1 (§8.2), but Go refuses to
    /// compile a local that is never read. W111 reports the column itself.
    fn is_read(&self, name: &str) -> bool {
        if self.only_named(name) {
            return false;
        }
        if let Some(r) = self.reads_here.borrow().as_ref() {
            return r.contains(name);
        }
        self.c.used.contains(name) || self.f.outputs.iter().any(|o| o.name.text == name)
    }

    /// Whether `name` is read only as a column of tables that write `-` in every row of it. Such a
    /// table names the value and tests nothing of it, so the code it is written as never reads it,
    /// though the check counts the column as a use (W111 does not say it). Go did not compile such
    /// a rule (§15.340).
    fn only_named(&self, name: &str) -> bool {
        if self.f.outputs.iter().any(|o| o.name.text == name) {
            return false;
        }
        let mut named = false;
        for it in &self.f.items {
            if let Item::Table(t) = it {
                if let Some(ci) = t.inputs.iter().position(|(n, _)| n == name) {
                    if !t.rows.iter().all(|r| matches!(r.cells.get(ci), Some(Cell::DontCare) | None)) {
                        return false;
                    }
                    named = true;
                }
                if t.rows.iter().any(|r| r.outs.iter().any(|o| matches!(o, OutCell::Name(w) if w == name))) {
                    return false;
                }
                continue;
            }
            if crate::types::item_reads(it).iter().any(|n| n == name) {
                return false;
            }
        }
        named
    }

    /// The names the items and the summaries of one walk read (§15.219).
    fn walk_reads(&self, items: &[usize], sums: &[usize]) -> HashSet<String> {
        items.iter().chain(sums.iter()).flat_map(|&i| self.stages.reads_of(i).iter().cloned()).collect()
    }

    fn go_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) if self.enum_names.contains_key(n) => self.go_enum_ty(n),
            Ty::Enum(_) => "string".into(),
            Ty::Bool => "bool".into(),
            Ty::Number | Ty::Date => "int64".into(),
            Ty::Str => "string".into(),
            Ty::Opt(t) => format!("*{}", self.go_ty(t)),
            _ => brand_of(ty),
        }
    }

    fn go_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        // An optional column is a pointer in Go, so every test but `none` reads through it
        // and needs the nil guard the type system asks for. Without it the package did not
        // compile at all: `in.Memo == KindA` is `*Kind` against `Kind` (DESIGN §15.88). The
        // guard flips for a negated cell, because `not 甲` holds of the absent value — `T?`
        // is the enum with one more value, and that is how the checker reads it too.
        let optional = matches!(ty, Ty::Opt(_));
        let raw = var.to_string();
        let deref = if optional { format!("(*{var})") } else { raw.clone() };
        let var = deref.as_str();
        let guarded = |c: String, neg: bool| -> String {
            if !optional {
                c
            } else if neg {
                format!("({raw} == nil || {c})")
            } else {
                format!("({raw} != nil && {c})")
            }
        };
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                Lit::Word(w) => self.go_value(w, ty),
                Lit::Num(n) => self.int_lit(n, inner, col_scale),
                Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => format!("{s:?}"),
            }
        };
        let set = |ls: &Vec<Lit>, neg: bool| -> String {
            let mut out: Vec<String> = Vec::new();
            for l in ls {
                if let Lit::Word(w) = l {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| self.go_value(m, ty)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            let op = if neg { "!=" } else { "==" };
            let join = if neg { " && " } else { " || " };
            format!(
                "({})",
                out.iter().map(|v| format!("{var} {op} {v}")).collect::<Vec<_>>().join(join)
            )
        };
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => return Some(format!("{raw} == nil")),
            Cell::Prefix(ps) => ps
                .iter()
                .map(|p| format!("strings.HasPrefix({var}, {})", format!("{p:?}")))
                .collect::<Vec<_>>()
                .join(" || "),
            // A cell that names one group calls the predicate the package already declares,
            // rather than writing the members out again — otherwise that function is dead.
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                guarded(format!("is{}({var})", pascal(&self.ident(w))), false)
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => guarded(var.to_string(), false),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => guarded(format!("!{var}"), false),
            Cell::Lit(l) => guarded(format!("{var} == {}", lit(l)), false),
            Cell::Set(ls) => guarded(set(ls, false), false),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                guarded(format!("!is{}({var})", pascal(&self.ident(w))), true)
            }
            Cell::Not(ls) => guarded(set(ls, true), true),
            // A comparison on an ordered enum: its constants are in the order declared (§15.210).
            Cell::Ranked { ops, .. } => guarded(
                ops.iter().map(|(o, v)| format!("{var} {} {}", o.word(), self.go_value(v, ty))).collect::<Vec<_>>().join(" && "),
                false,
            ),
            // A comparison reads through the pointer too, so it takes the nil guard as every other
            // test does: without it a nil value panicked (§15.201).
            Cell::Cmp(cs) => guarded(
                cs.iter()
                    .map(|(o, l)| {
                        let op = match o {
                            CmpOp::Le => "<=",
                            CmpOp::Ge => ">=",
                            CmpOp::Lt => "<",
                            CmpOp::Gt => ">",
                        };
                        format!("{var} {op} {}", lit(l))
                    })
                    .collect::<Vec<_>>()
                    .join(" && "),
                false,
            ),
        })
    }

    pub fn go(&self) -> String {
        let alias = pub_name(&self.f.name);
        let pkg = crate::backend::go_package(&alias);
        let mut o = self.header("//");
        o.push_str(&format!("\npackage {pkg}\n\n{IMPORT_MARK}"));

        // A brand is a defined type (not an alias): the compiler rejects YenInclTax + YenExclTax.
        // In a package of ritsu's, those of money and of the quantities are the package's defined
        // types, under the same names (§15.248).
        let (mut own, units) = self.brands();
        // A column a table writes that may be absent is held under the unit's type (`*JPY`): a
        // rule none of whose inputs, fields and outputs carry the unit declares it (§15.343).
        for v in self.branded_columns() {
            let ty = self.ty_of(v).present().clone();
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) && !units.contains_key(&brand_of(&ty)) {
                own.entry(brand_of(&ty)).or_insert(ty);
            }
        }
        for (b, ty) in &own {
            o.push_str(&format!("type {b} int64{CELL}// {ty}\n"));
        }
        for (b, ty) in &units {
            o.push_str(&format!("type {b} = units.{b}{CELL}// {ty}\n"));
        }
        if !own.is_empty() || !units.is_empty() {
            o.push('\n');
        }

        for (jp, ascii) in self.own_enums() {
            o.push_str(&self.go_enum(&jp, &ascii));
        }
        o.push_str(&self.go_foreign_enums());
        o.push_str(&self.go_enum_helpers());
        // Which values need which before them (§15.222).
        o.push_str(&self.go_prereqs());

        // The machine this function is one step of (§15.148).
        if let Some((en, init, fins)) = self.machine_consts() {
            let cls = self.go_enum_ty(&en);
            o.push_str(&format!(
                "// Initial {}\nconst Initial = {}\n\n// Final {}\nvar Final = []{cls}{{{}}}\n\n\
                 // IsFinal {}\nfunc IsFinal(s {cls}) bool {{\n\tfor _, f := range Final {{\n\t\tif f == s {{\n\t\t\treturn true\n\t\t}}\n\t}}\n\treturn false\n}}\n\n",
                tr!("は案件が始まる状態。", "is the state a case starts in."),
                self.go_value(&init, &Ty::Enum(en.clone())),
                tr!("は案件が終わる状態。", "is the set of states a case ends in."),
                fins.iter().map(|v| self.go_value(v, &Ty::Enum(en.clone()))).collect::<Vec<_>>().join(", "),
                tr!("はこの状態で案件が終わっているかを返す。", "reports whether a case in this state has ended.")
            ));
            // The values carried along with the state, and where a case starts (§15.217).
            for mc in self.more_carried() {
                let (ty, v) = match mc.kind() {
                    MoreKind::Enum => (String::new(), self.go_value(&mc.word(), &mc.ty)),
                    MoreKind::Bool => (String::new(), mc.bool_word().to_string()),
                    MoreKind::Number => (" int64".to_string(), mc.int.to_string()),
                    MoreKind::Branded => (format!(" {}", self.go_ty(&mc.ty)), mc.int.to_string()),
                };
                o.push_str(&format!(
                    "// {} {}\nconst {}{ty} = {v}\n\n",
                    mc.go(),
                    tr!("は、持ち越す {} の始まりの値。", "is where the carried {} starts.", mc.input),
                    mc.go()
                ));
            }
        }

        // Groups. They are unexported, so the Japanese identifiers can stay (§8.1).
        for g in &self.f.groups {
            let ty = self
                .c
                .groups
                .get(&g.name.text)
                .and_then(|(owner, _)| self.enum_names.contains_key(owner).then(|| self.go_enum_ty(owner)))
                .unwrap_or_else(|| "int".into());
            o.push_str(&format!(
                "func is{}(v {ty}) bool {{\n\tswitch v {{\n\tcase ",
                pascal(&self.ident(&g.name.text))
            ));
            let ms: Vec<String> = g.members.iter().map(|m| self.go_value(&m.text, &self.group_ty(&g.name.text))).collect();
            o.push_str(&ms.join(", "));
            o.push_str(":\n\t\treturn true\n\t}\n\treturn false\n}\n\n");
        }

        o.push_str(&format!("// Fired {}\ntype Fired struct {{\n\tTable{CELL}string\n\tRow{CELL}int\n\tLabel{CELL}string\n}}\n\n", fired_doc()));
        o.push_str(&round_go());
        o.push_str(&arith::go_helpers(&arith::Uses::of(self.f)));
        o.push_str(&self.go_fn());
        o.push('\n');
        o.push_str(&self.go_record());
        // The function that counts the answers (§15.225).
        if self.f.has_random() {
            o.push('\n');
            o.push_str(self.go_chances().trim_end());
            o.push('\n');
        }
        // The record function uses fmt whatever the rule looks like, so the import is
        // unconditional now. `strings` comes in only where a cell tests a prefix
        // (§15.101) — an unused import does not compile in Go.
        // The package of each `enums` file it reads its types from (§15.212).
        let mut paths: Vec<String> = vec!["fmt".to_string()];
        if o.contains("strings.HasPrefix(") {
            paths.push("strings".to_string());
        }
        let mut named: Vec<(String, String)> = Vec::new();
        for (m, dir, _) in self.foreign_owners() {
            let (path, q) = (self.go_owner_path(&m, &dir), self.go_qualifier(&m, &dir));
            let spec = if q == crate::backend::go_package(&m) { format!("{path:?}") } else { format!("{q} {path:?}") };
            named.push((path, spec));
        }
        // The package's types of the units (§15.248).
        if !units.is_empty() {
            paths.push(match &self.package {
                Some(m) if !m.is_empty() => format!("{m}/units"),
                _ => "units".to_string(),
            });
        }
        // gofmt keeps the specs of one block in order, by their paths
        let mut specs: Vec<(String, String)> = paths.into_iter().map(|p| (p.clone(), format!("{p:?}"))).chain(named).collect();
        specs.sort();
        let imports = if specs.len() == 1 {
            format!("import {}\n\n", specs[0].1)
        } else {
            format!("import (\n{})\n\n", specs.iter().map(|(_, s)| format!("\t{s}\n")).collect::<String>())
        };
        let o = o.replace(IMPORT_MARK, &imports);
        align(&o)
    }

    /// One enum as Go: an int type whose constants are in the order declared, its validity, its
    /// wire names, its parser, and its display names when it has them (§15.211). The constants
    /// are the order an ordered enum compares in (§15.210).
    pub(crate) fn go_enum(&self, jp: &str, ascii: &str) -> String {
        let Some(vals) = self.c.enums.get(jp) else { return String::new() };
        let mut o = String::new();
        o.push_str(&format!("type {ascii} int\n\nconst (\n"));
        for (i, v) in vals.iter().enumerate() {
            let n = self.go_value(v, &Ty::Enum(jp.to_string()));
            if i == 0 {
                o.push_str(&format!("\t{n}{CELL}{ascii} = iota{CELL}// {v}\n"));
            } else {
                o.push_str(&format!("\t{n}{CELL}{CELL}// {v}\n"));
            }
        }
        o.push_str(")\n\n");
        o.push_str(&format!(
            "func (v {ascii}) Valid() bool {{ return v >= 0 && v < {} }}\n\n",
            vals.len()
        ));
        o.push_str(&format!("func (v {ascii}) String() string {{\n\tswitch v {{\n"));
        for v in vals {
            o.push_str(&format!("\tcase {}:\n\t\treturn {:?}\n", self.go_value(v, &Ty::Enum(jp.to_string())), v));
        }
        o.push_str("\t}\n\treturn \"?\"\n}\n\n");
        if let Some(labels) = self.c.labels.get(jp) {
            o.push_str(&format!(
                "// Label {}\nfunc (v {ascii}) Label() string {{\n\tswitch v {{\n",
                tr!("は表示名を返す。表示名の無い値は値の名前。", "returns the display name: a value without one is shown by its name.")
            ));
            for (i, v) in vals.iter().enumerate() {
                let l = labels.get(i).cloned().flatten().unwrap_or_else(|| v.clone());
                o.push_str(&format!("\tcase {}:\n\t\treturn {}\n", self.go_value(v, &Ty::Enum(jp.to_string())), str_lit(&l)));
            }
            o.push_str("\t}\n\treturn \"?\"\n}\n\n");
        }
        // The wire format of §10 uses the Japanese names, so provide a way back from them.
        o.push_str(&format!("func Parse{ascii}(s string) ({ascii}, bool) {{\n\tswitch s {{\n"));
        for v in vals {
            o.push_str(&format!("\tcase {:?}:\n\t\treturn {}, true\n", v, self.go_value(v, &Ty::Enum(jp.to_string()))));
        }
        o.push_str(&format!("\t}}\n\treturn {}(0), false\n}}\n\n", ascii));
        o
    }

    /// The import path of the package an `enums` file's types are in (§15.212): beside the
    /// rule's package in a package of ritsu's, and a module of its own beside it from `rulec gen`.
    fn go_owner_path(&self, module: &str, dir: &str) -> String {
        let pkg = crate::backend::go_package(module);
        match &self.package {
            Some(m) if !m.is_empty() => self.place.go_path(m, &format!("{dir}rules/{pkg}")),
            _ => pkg,
        }
    }

    /// The types of an `enums` file as this package names them: an alias of each, so that a
    /// value of one is a value of the other, its constants, and its parser (§15.212).
    fn go_foreign_enums(&self) -> String {
        let mut o = String::new();
        for (module, dir, types) in self.foreign_owners() {
            let pkg = self.go_qualifier(&module, &dir);
            for (jp, ascii) in types {
                let Some(vals) = self.c.enums.get(&jp) else { continue };
                // What the package that owns it calls each name: the same, unless the type is
                // handed on under another here (§15.326).
                let theirs = |n: &str| self.theirs("go", &jp, n);
                let owned = theirs(&ascii);
                // Where the rule's own functions take the type's name, the package names it by the
                // package that owns it and declares no alias of it (§15.239).
                if self.go_owner_of(&jp).is_some() {
                    o.push_str("const (\n");
                } else {
                    o.push_str(&format!(
                        "// {ascii} {}\ntype {ascii} = {pkg}.{owned}\n\nconst (\n",
                        tr!("は {pkg}.{owned} の別名で、同じ型。", "is {pkg}.{owned} under another name: the same type.")
                    ));
                }
                for v in vals {
                    let n = self.go_value(v, &Ty::Enum(jp.clone()));
                    o.push_str(&format!("\t{n}{CELL}= {pkg}.{}\n", theirs(&n)));
                }
                o.push_str(")\n\n");
                // On one line where gofmt keeps it on one: its header and its one statement
                // within 100 bytes (go/printer's `funcBody`). A longer one gofmt breaks, and so
                // does this, or `gofmt -l` names the file (§15.222).
                let parse = theirs(&format!("Parse{ascii}"));
                let (head, stmt) = (format!("func Parse{ascii}(s string) ({}, bool)", self.go_enum_ty(&jp)), format!("return {pkg}.{parse}(s)"));
                let body = if head.len() + stmt.len() <= 100 { format!("{head} {{ {stmt} }}") } else { format!("{head} {{\n\t{stmt}\n}}") };
                o.push_str(&format!(
                    "// Parse{ascii} {}\n{body}\n\n",
                    tr!("は {pkg}.{parse} と同じ。", "is {pkg}.{parse}.")
                ));
            }
        }
        o
    }

    /// The helpers the functions call for the expressions over an ordered enum (§15.210). The
    /// constants are in the order declared, so the place of a value is the value.
    fn go_enum_helpers(&self) -> String {
        let mut o = String::new();
        for en in self.c.enum_order.iter().filter(|e| self.c.ordered.contains(*e)) {
            let (rank, max, min) = self.enum_helpers(en);
            // The helper is named by the type's own name, and takes the type as the package writes it.
            let (t, ty) = (self.enum_type(en), self.go_enum_ty(en));
            if rank {
                o.push_str(&format!("func _rank_{t}(x {ty}) int64 {{\n\treturn int64(x)\n}}\n\n"));
            }
            if max {
                o.push_str(&format!("func _max_{t}(a, b {ty}) {ty} {{\n\tif a >= b {{\n\t\treturn a\n\t}}\n\treturn b\n}}\n\n"));
            }
            if min {
                o.push_str(&format!("func _min_{t}(a, b {ty}) {ty} {{\n\tif a <= b {{\n\t\treturn a\n\t}}\n\treturn b\n}}\n\n"));
            }
        }
        o
    }

    /// The `go.mod` of the rule's package, and of its runner: each requires the module of every
    /// `enums` file the rule reads its types from, beside it (§15.212).
    pub fn go_mod(&self, pkg: &str, runner: bool) -> String {
        let owners: Vec<String> = self.foreign_enums().into_iter().map(|(m, _)| crate::backend::go_package(&m)).collect();
        let mut requires: Vec<String> = Vec::new();
        let mut replaces: Vec<String> = Vec::new();
        if runner {
            requires.push(format!("require {pkg} v0.0.0\n"));
            replaces.push(format!("replace {pkg} => ../{pkg}\n"));
        }
        for m in &owners {
            requires.push(format!("require {m} v0.0.0\n"));
            replaces.push(format!("replace {m} => ../{m}\n"));
        }
        let name = if runner { format!("{pkg}runner") } else { pkg.to_string() };
        let mut o = format!("module {name}\n\ngo {}\n", crate::backend::GO_MIN);
        if !requires.is_empty() {
            o.push('\n');
            o.push_str(&requires.concat());
            o.push('\n');
            o.push_str(&replaces.concat());
        }
        o
    }

    /// The rule's items as Go, at the base indentation (§15.56).
    fn go_items(&self, local: &dyn Fn(&str) -> String, trace: &str, phase: Phase, zero: &str) -> String {
        let mut o = String::new();
        for (i, it) in self.f.items.iter().enumerate() {
            if !self.in_phase(i, it, phase) {
                continue;
            }
            match it {
                Item::Derived(d) => {
                    let e = self.expr(&d.expr, local);
                    o.push_str(&format!("\t{} := {}{CELL}// {}\n", self.ident(&d.name.text), go_typed(&go_expr(&e.text)), tr!("導出", "derived value")));
                    o.push_str(&self.go_unread(&d.name.text));
                }
                Item::Define(d) => {
                    let e = self.expr(&d.expr, local);
                    o.push_str(&format!("\t{} := {}{CELL}// {}\n", self.ident(&d.name.text), go_typed(&go_expr(&e.text)), tr!("定義", "definition")));
                    // The range the rule accepts for it, held where it is worked out (§15.313).
                    let n = self.ident(&d.name.text);
                    if let Some(test) = self.accepted_ends(d).and_then(|ends| or_out_of(&n, ends, "")) {
                        o.push_str(&format!(
                            "\tif {test} {{\n\t\treturn {zero}, nil, &RuleInputError{{What: {}, Value: {n}, HasValue: true}}\n\t}}\n",
                            go_said(&crate::door::out_of_range(&self.said_name(&d.name.text)), &self.here(), "")
                        ));
                    }
                    o.push_str(&self.go_unread(&d.name.text));
                }
                Item::Agg(_) | Item::Fold(_) => {}
                Item::Table(t) => {
                    if let Some(t) = self.c.table_at(t) {
                        o.push_str(&self.go_table(t, local, trace));
                    }
                }
            }
        }
        o
    }

    /// Every walk of a rule that walks sequences, in Go (§15.56, §15.58, §15.219). The shape is
    /// the one `py_walks` writes.
    fn go_walks(&self, outer: &dyn Fn(&str) -> String, trace: &str, zero: &str) -> String {
        let mut o = String::new();
        for (s, el) in self.f.elements.iter().enumerate() {
            if let Some(cap) = self.count_cap_of(s) {
                let seq = format!("in.{}", pascal(&pub_name(&el.name)));
                o.push_str(&format!(
                    "\tif len({seq}) > {cap} {{\n\t\treturn {zero}, nil, &RuleInputError{{What: \"{}\", Value: int64(len({seq})), HasValue: true}}\n\t}}\n",
                    crate::door::too_many(&el.name.text, cap)
                ));
            }
        }
        for step in self.walk_steps() {
            match step {
                // What a walk does not depend on is worked out once, before it (§15.208).
                WalkStep::Once(ix) => o.push_str(&self.go_items(outer, trace, Phase::Only(&ix), zero)),
                WalkStep::Walk { seq, body, sums, .. } => o.push_str(&self.go_walk(seq, &body, &sums, outer, trace, zero)),
            }
        }
        o
    }

    /// One walk over sequence `s`, in Go (§15.219).
    #[allow(clippy::too_many_arguments)]
    fn go_walk(&self, s: usize, body_items: &[usize], sums: &[usize], outer: &dyn Fn(&str) -> String, trace: &str, zero: &str) -> String {
        let el = &self.f.elements[s];
        let seq = format!("in.{}", pascal(&pub_name(&el.name)));
        let fields: BTreeMap<String, String> = el.fields.iter().map(|fd| (fd.name.text.clone(), pascal(&pub_name(&fd.name)))).collect();
        let e = self.elem_var(s);
        // Inside the walk a field is read off the element.
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => {
                    let v = format!("{e}.{f}");
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                        format!("int64({v})")
                    } else {
                        v
                    }
                }
                None => outer(n),
            }
        };
        let mut o = String::new();
        let index = self.index_var(s);
        self.walk_begin(s, &index);
        let mut body = self.go_element_ranges(s, &local, zero);
        let (seen, step) = self.go_unique(s, sums, &local, zero);
        o.push_str(&seen);
        body.push_str(&step);
        // Past a `stop`, the door alone (§15.328).
        if let Some(i) = self.walk_goes_past_stop(sums) {
            body.push_str(&format!("\tif {} {{\n\t\tcontinue\n\t}}\n", self.fold_names(i).done));
        }
        // Go refuses to compile a local nothing reads, so what the walk's own items bind and no
        // one in the walk reads is said to be read (§15.219).
        let reads = self.walk_reads(body_items, sums);
        *self.reads_here.borrow_mut() = Some(reads);
        body.push_str(&self.go_items(&local, trace, Phase::Only(body_items), zero));
        *self.reads_here.borrow_mut() = None;
        let (mut after, mut unread) = (String::new(), String::new());
        for &i in sums {
            match &self.f.items[i] {
                Item::Agg(d) if d.kind.extreme() => {
                    let (init, step) = self.go_extreme(d, &local);
                    o.push_str(&init);
                    body.push_str(&step);
                    unread.push_str(&self.go_unread(&d.name.text));
                }
                Item::Agg(d) => {
                    o.push_str(&format!("\tvar {} int64 = 0{CELL}// {}\n", self.ident(&d.name.text), self.agg_word(d)));
                    let v = local(&d.column.text);
                    if d.kind == crate::ast::AggKind::Sum {
                        body.push_str(&format!("\t{} += {v}{CELL}// {}\n", self.ident(&d.name.text), d.name.text));
                    } else {
                        let test = self.go_test(&v, &d.column.text, &d.value);
                        body.push_str(&format!("\tif {test} {{{CELL}// {}\n\t\t{}++\n\t}}\n", d.name.text, self.ident(&d.name.text)));
                    }
                    if let Some((_, cap)) = self.sum_caps().into_iter().find(|(x, _)| x.name.text == d.name.text) {
                        let n = self.ident(&d.name.text);
                        after.push_str(&format!(
                            "\tif {n} > {cap} {{\n\t\treturn {zero}, nil, &RuleInputError{{What: \"{}\", Value: {n}, HasValue: true}}\n\t}}\n",
                            crate::door::out_of_range(&d.name.text)
                        ));
                    }
                    // The bottom of its range, where a walk can come in under it (§15.314).
                    if let Some(floor) = self.walk_floor(d) {
                        let n = self.ident(&d.name.text);
                        after.push_str(&format!(
                            "\tif {n} < {floor} {{\n\t\treturn {zero}, nil, &RuleInputError{{What: \"{}\", Value: {n}, HasValue: true}}\n\t}}\n",
                            crate::door::out_of_range(&d.name.text)
                        ));
                    }
                    unread.push_str(&self.go_unread(&d.name.text));
                }
                Item::Fold(fold) => {
                    let (init, step, fin) = self.go_fold(i, fold, &seq, &local, zero);
                    o.push_str(&init);
                    body.push_str(&step);
                    after.push_str(&fin);
                }
                _ => {}
            }
        }
        // The index of the element is bound where a sentence of the walk says it (§15.324): Go
        // refuses to compile one nothing reads. So it goes with an element no line of the walk
        // reads, as where each row of its tables tests a field with `-`, which is said to be left
        // unread on purpose (§15.340).
        let at = if self.walk_end() { index.as_str() } else { "_" };
        if !clash::reads_word(&body, &e, clash::Syntax::Script) {
            body.insert_str(0, &format!("\t_ = {e}\n"));
        }
        o.push_str(&format!("\tfor {at}, {e} := range {seq} {{\n"));
        o.push_str(&Self::indent_block(&body, "\t"));
        o.push_str("\t}\n");
        o.push_str(&after);
        o.push_str(&unread);
        o
    }

    /// A test on a column of one element, in Go: `count`'s, and the `where` of `max` and `min`.
    fn go_test(&self, v: &str, col: &str, value: &Option<crate::ast::Name>) -> String {
        let ty = self.ty_of(col);
        match (&ty, value) {
            (Ty::Bool, Some(w)) if w.text == crate::kw::FALSE => format!("!{v}"),
            (Ty::Bool, _) => v.to_string(),
            (_, Some(w)) => format!("{v} == {}", self.go_value(&w.text, &ty)),
            (_, None) => v.to_string(),
        }
    }

    /// A `max` or a `min`, in Go (§15.219): the lines before the loop and the lines inside it.
    fn go_extreme(&self, d: &crate::ast::AggDecl, local: &dyn Fn(&str) -> String) -> (String, String) {
        let n = self.ident(&d.name.text);
        let set = self.temp(&format!("{n}Set"));
        let v = local(&d.column.text);
        let ty = self.ty_of(&d.column.text);
        let op = if d.kind == crate::ast::AggKind::Max { ">" } else { "<" };
        let (go_t, start, better) = match self.extreme_enum(d) {
            Some(en) => {
                let t = self.enum_type(&en);
                let w = self.extreme_empty_word(d).unwrap_or_default();
                (self.go_ty(&ty), self.go_value(&w, &ty), format!("_rank_{t}({v}) {op} _rank_{t}({n})"))
            }
            None => ("int64".to_string(), self.extreme_empty_int(d).unwrap_or_else(|| "0".into()), format!("{v} {op} {n}")),
        };
        let init = format!("\tvar {n} {go_t} = {start}{CELL}// {}\n\t{set} := false\n", self.agg_word(d));
        let mut step = String::new();
        let pad = match &d.filter {
            Some((w, want)) => {
                step.push_str(&format!("\tif {} {{{CELL}// {}\n", self.go_test(&local(&w.text), &w.text, want), d.name.text));
                "\t"
            }
            None => "",
        };
        step.push_str(&format!("{pad}\tif !{set} || {better} {{\n{pad}\t\t{n} = {v}\n{pad}\t\t{set} = true\n{pad}\t}}\n"));
        if !pad.is_empty() {
            step.push_str("\t}\n");
        }
        (init, step)
    }

    /// A fold, in Go (§15.56): the lines before the loop, the arms inside it, and the answer bound
    /// to its name after it. Go has no `None`, so each accumulator carries its own "was it ever
    /// set" flag.
    fn go_fold(&self, i: usize, fold: &crate::ast::FoldDecl, seq: &str, local: &dyn Fn(&str) -> String, zero: &str) -> (String, String, String) {
        let names = self.fold_names(i);
        let FoldNames { answer, stopped, done, taken, kept, best, held, key, took, kept_one, best_set, taken_at_camel: taken_at, .. } = &names;
        let alone = self.fold_alone(i);
        // Ends the loop at a `stop`: a fold alone in its walk with no door to keep past it (§15.328).
        let ends = alone && !self.stop_keeps_door(i);
        let with_held = |n: &str| -> String { if n == crate::kw::HELD { held.clone() } else { local(n) } };
        let (out_name, out_ty) = self.fold_target(fold);
        let acc_ty = if out_ty.is_numeric() { "int64".to_string() } else { self.go_ty(&out_ty) };
        let acc_zero = if out_ty.is_numeric() { "0".to_string() } else { self.go_zero(&out_ty) };
        let weighted = fold.weighted();
        let draw = fold.draw.as_ref().map(|d| go_i64(&local(&d.text))).unwrap_or_default();
        // An answer is read in the type the fold answers with: a value of its enum is that
        // enum's member (§15.208).
        let val = |x: &Expr, l: &dyn Fn(&str) -> String| -> String {
            match x {
                Expr::Lit(Lit::Word(w), _) if w != crate::kw::TRUE && w != crate::kw::FALSE => self.go_value(w, &out_ty),
                _ => go_expr(&self.expr(x, l).text),
            }
        };
        let text = |x: &Option<Expr>| -> String { x.as_ref().map(|x| val(x, &with_held)).unwrap_or_else(|| "0".to_string()) };
        // Go refuses to compile a local nothing reads, so only the arms that are written get one.
        let takes = fold.arms.iter().any(|(_, a, _)| matches!(a, Arm::Take { .. } | Arm::TakeWeighted { .. }));
        let keeps = fold.arms.iter().any(|(_, a, _)| matches!(a, Arm::KeepMax { .. }));

        let mut init = String::new();
        init.push_str(&format!("\tvar {answer} {acc_ty} = {}{CELL}// {}\n", text(&fold.empty), tr!("要素ゼロ件の答え", "the answer for no elements")));
        init.push_str(&format!("\t{stopped} := false\n"));
        if !ends {
            init.push_str(&format!("\t{done} := false\n"));
        }
        if takes {
            init.push_str(&format!("\tvar {taken} {acc_ty} = {acc_zero}\n\t{took} := false\n"));
        }
        // The element a `take_unique` took, which the sentence says beside the one that matched
        // again (§15.324).
        if fold.arms.iter().any(|(_, a, _)| matches!(a, Arm::Take { unique: true, .. })) {
            init.push_str(&format!("\t{taken_at} := 0\n"));
        }
        if weighted {
            init.push_str(&format!("\tvar {} int64 = 0{CELL}// {}\n", names.total, tr!("重みの合計", "the running total of the weights")));
            init.push_str(&format!("\tvar {} []int64\n", names.cuts));
        }
        if keeps {
            init.push_str(&format!("\tvar {kept} {acc_ty} = {acc_zero}\n\t{kept_one} := false\n"));
            init.push_str(&format!("\tvar {best} int64 = 0\n\t{best_set} := false\n"));
        }

        // The verdict the table wrote for this element, and what the walk does about it.
        let mut arms = String::new();
        let v = local(&fold.verdict);
        for (name, arm, _) in &fold.arms {
            arms.push_str(&format!("\tif {} == {} {{{CELL}// {}\n", v, self.go_value(&name.text, &self.ty_of(&fold.verdict)), name.text));
            match arm {
                Arm::Next => arms.push_str(&format!("\t\t// {}\n", crate::kw::NEXT)),
                Arm::Stop(None) if ends => arms.push_str("\t\tbreak\n"),
                Arm::Stop(None) => arms.push_str(&format!("\t\t{done} = true\n")),
                Arm::Stop(Some(x)) => {
                    arms.push_str(&format!("\t\t{answer} = {}\n", val(x, local)));
                    if ends {
                        arms.push_str(&format!("\t\t{stopped} = true\n\t\tbreak\n"));
                    } else {
                        arms.push_str(&format!("\t\t{stopped} = true\n\t\t{done} = true\n"));
                    }
                }
                Arm::Take { expr, unique } => {
                    if *unique {
                        let here = self.here_used();
                        arms.push_str(&format!("\t\tif {took} {{\n"));
                        arms.push_str(&format!(
                            "\t\t\treturn {zero}, nil, &RuleContradictionError{{What: {}}}\n\t\t}}\n",
                            go_said(&crate::door::fold_contradiction(&fold.verdict, &fold.over, crate::door::BEFORE, crate::door::HERE), &here, taken_at)
                        ));
                        arms.push_str(&format!("\t\t{taken} = {}\n\t\t{took} = true\n\t\t{taken_at} = {here}\n", val(expr, local)));
                    } else {
                        arms.push_str(&format!("\t\tif !{took} {{\n"));
                        arms.push_str(&format!("\t\t\t{taken} = {}\n\t\t\t{took} = true\n\t\t}}\n", val(expr, local)));
                    }
                }
                Arm::KeepMax { expr, key: k } => {
                    arms.push_str(&format!("\t\t{key} := {}\n", go_expr(&self.expr(k, local).text)));
                    arms.push_str(&format!("\t\tif !{best_set} || {key} > {best} {{\n"));
                    arms.push_str(&format!("\t\t\t{best} = {key}\n\t\t\t{best_set} = true\n"));
                    arms.push_str(&format!("\t\t\t{kept} = {}\n\t\t\t{kept_one} = true\n\t\t}}\n", val(expr, local)));
                }
                // The weight goes on the running total, and the element is taken where the drawn
                // value is below the total for the first time (§15.226). A weight that would take
                // the total past int64 is refused before it goes on, and before another element is
                // read.
                Arm::TakeWeighted { expr, weight } => {
                    let (w, t, c) = (&names.weight, &names.total, &names.cuts);
                    arms.push_str(&format!("\t\t{w} := int64({})\n", go_expr(&self.expr(weight, local).text)));
                    arms.push_str(&format!(
                        "\t\tif {w} > 9223372036854775807-{t} {{\n\t\t\treturn {zero}, nil, &RuleInputError{{What: \"{}\"}}\n\t\t}}\n",
                        crate::door::weights_past(&fold.over)
                    ));
                    arms.push_str(&format!("\t\t{t} += {w}\n\t\t{c} = append({c}, {t})\n"));
                    arms.push_str(&format!("\t\tif !{took} && {draw} < {t} {{\n"));
                    arms.push_str(&format!("\t\t\t{taken} = {}\n\t\t\t{took} = true\n\t\t}}\n", val(expr, local)));
                }
            }
            arms.push_str("\t}\n");
        }
        let step = if alone { arms } else { format!("\tif !{done} {{{CELL}// {}\n{}\t}}\n", fold.verdict, Self::indent_block(&arms, "\t")) };

        // The drawn value of a pick by weight is below the total of the weights, unless they add up
        // to 0 (§15.226).
        let mut fin = String::new();
        if weighted {
            let t = &names.total;
            let what = fold.draw.as_ref().map(|d| crate::door::draw_past(&d.text)).unwrap_or_default();
            fin.push_str(&format!("\tif {t} > 0 && {draw} >= {t} {{\n\t\treturn {zero}, nil, &RuleInputError{{What: \"{what}\", Value: {draw}, HasValue: true}}\n\t}}\n"));
        }
        // A sequence with nothing in it answers `empty` and never reaches `exhausted`; a walk
        // that ended on `stop with` has its answer already.
        fin.push_str(&format!("\tif len({seq}) > 0 && !{stopped} {{\n"));
        fin.push_str(&format!("\t\tvar {held} {acc_ty} = {}\n", text(&fold.empty)));
        if takes {
            fin.push_str(&format!("\t\tif {took} {{\n\t\t\t{held} = {taken}\n\t\t}}\n"));
        }
        if keeps {
            if takes {
                // Chain onto the `if` just written, so a taken value wins over a kept one.
                fin = fin.trim_end_matches("\t\t}\n").to_string();
                fin.push_str(&format!("\t\t}} else if {kept_one} {{\n\t\t\t{held} = {kept}\n\t\t}}\n"));
            } else {
                fin.push_str(&format!("\t\tif {kept_one} {{\n\t\t\t{held} = {kept}\n\t\t}}\n"));
            }
        }
        let ex = text(&fold.exhausted);
        if !ex.contains(held.as_str()) {
            fin.push_str(&format!("\t\t_ = {held}\n"));
        }
        fin.push_str(&format!("\t\t{answer} = {ex}\n\t}}\n"));
        fin.push_str(&format!("\t{} := {answer}\n", self.ident(&out_name)));
        fin.push_str(&self.go_unread(&out_name));
        (init, step, fin)
    }

    /// One field of an element as the door reads it in Go: the value, and where the field is
    /// optional the lines that read what its pointer points at, under the name of the field.
    fn go_field(&self, name: &str, v: String) -> (String, Option<(String, String)>) {
        if matches!(self.ty_of(name), Ty::Opt(_)) {
            let x = self.ident(name);
            (x.clone(), Some((format!("\tif {v} != nil {{\n\t\t{x} := *{v}\n"), "\t}\n".to_string())))
        } else {
            (v, None)
        }
    }

    /// The first look at each element of every sequence, in Go (§15.328): before any input is held
    /// to its range, each element in turn, and each field of an enum in the order the rule declares
    /// them, a value of its enum. The other kinds are the kinds of the struct's fields.
    fn go_element_kinds(&self, zero: &str) -> String {
        let mut out = String::new();
        for (s, el) in self.f.elements.iter().enumerate() {
            let fields: BTreeMap<String, String> = el.fields.iter().map(|fd| (fd.name.text.clone(), pascal(&pub_name(&fd.name)))).collect();
            let e = self.elem_var(s);
            let local = |n: &str| -> String { fields.get(n).map(|f| format!("{e}.{f}")).unwrap_or_else(|| n.to_string()) };
            let index = self.index_var(s);
            self.walk_begin(s, &index);
            let mut o = String::new();
            let here = self.here();
            for i in self.element_fields(s) {
                let Ty::Enum(n) = self.ty_of(&i.name.text).present().clone() else { continue };
                // The sentences name the field by its place on the element (§15.324).
                let said = || self.said_name(&i.name.text);
                let (v, wrap) = self.go_field(&i.name.text, local(&i.name.text));
                let val = go_i64(&v);
                let mut o = Guarded::new(&mut o, wrap, "\t");
                o.push_str(&format!(
                    "\tif !{v}.Valid() {{\n\t\treturn {zero}, nil, &RuleInputError{{What: {}, Value: {val}, HasValue: true}}\n\t}}\n",
                    go_said(&crate::door::not_in_enum(&said(), &n), &here, "")
                ));
            }
            let at = if self.walk_end() { index.as_str() } else { "_" };
            if !o.is_empty() {
                out.push_str(&format!("\tfor {at}, {e} := range in.{} {{\n", pascal(&pub_name(&el.name))));
                out.push_str(&Self::indent_block(&o, "\t"));
                out.push_str("\t}\n");
            }
        }
        out
    }

    /// The door's look at one element where the walk comes to it (§15.328): each field inside the
    /// range it declares, in the order the rule declares them.
    fn go_element_ranges(&self, s: usize, local: &dyn Fn(&str) -> String, zero: &str) -> String {
        let mut o = String::new();
        let here = self.here();
        for i in self.element_fields(s) {
            // The sentences name the field by its place on the element (§15.324). Asked for only
            // where one is written: a walk binds the index only where a sentence reads it.
            let said = || self.said_name(&i.name.text);
            let ty = self.ty_of(&i.name.text).present().clone();
            if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                continue;
            }
            // An optional input is guarded as the type it wraps whenever it has a value (§15.201).
            let (v, wrap) = self.go_field(&i.name.text, local(&i.name.text));
            let val = go_i64(&v);
            if let Some(test) = or_out_of(&v, crate::door::ends(self.c, &i.name.text, false), "") {
                let mut o = Guarded::new(&mut o, wrap, "\t");
                o.push_str(&format!(
                    "\tif {test} {{\n\t\treturn {zero}, nil, &RuleInputError{{What: {}, Value: {val}, HasValue: true}}\n\t}}\n",
                    go_said(&crate::door::out_of_range(&said()), &here, "")
                ));
            }
        }
        o
    }

    /// The fields marked `unique` (§15.290), in Go: a map for each before the first walk over the
    /// sequence, and in the walk, after the guards of one element, the refusal of a value an element
    /// before it had.
    fn go_unique(&self, s: usize, sums: &[usize], local: &dyn Fn(&str) -> String, zero: &str) -> (String, String) {
        let seq = &self.f.elements[s].name.text;
        let (mut init, mut step) = (String::new(), String::new());
        for fd in self.unique_fields(s, sums) {
            let seen = self.temp(&format!("{}Seen", self.ident(&fd.name.text)));
            let v = local(&fd.name.text);
            // Where each value was first seen, which the sentence says beside the element it came
            // round again on (§15.324).
            let here = self.here_used();
            let first = self.temp("first");
            init.push_str(&format!("\t{seen} := map[any]int{{}}\n"));
            step.push_str(&format!(
                "\tif {first}, ok := {seen}[{v}]; ok {{\n\t\treturn {zero}, nil, &RuleInputError{{What: {}}}\n\t}}\n\t{seen}[{v}] = {here}\n",
                go_said(&crate::door::duplicate(seq, &fd.name.text, crate::door::BEFORE, crate::door::HERE), &here, &first)
            ));
        }
        (init, step)
    }

    fn go_fn(&self) -> String {
        let fname = pascal(&pub_name(&self.f.name));
        let outs = &self.f.outputs;
        let mut o = String::new();

        o.push_str(&format!(
            "// {}\ntype Input struct {{\n",
            tr!(
                "Input は入力をまとめて受ける。同型の int が並ぶのを避けるため。",
                "Input bundles the inputs, so that a parameter list of same-typed ints is avoided."
            )
        ));
        for i in &self.f.inputs {
            // The field holds the wire integer, so the range is shown in the same units as
            // the guard below it, not in the units the declaration was written in.
            let sc = self.c.wire_scale(&i.name.text);
            let ty = self.ty_of(&i.name.text);
            // An ordered enum's range, by the constants the guard compares it with (§15.251).
            let ends = self.c.enum_ranges.get(&i.name.text).and_then(|vs| Some((vs.first()?, vs.last()?)));
            // A range that writes one end shows that end, as the guard holds the field to it.
            let written = self.c.ranges.get(&i.name.text).copied().filter(|(lo, hi)| lo.is_some() || hi.is_some());
            let doc = match (written, ends) {
                (Some((lo, hi)), _) => tr!(
                    "// {} 範囲 {}..{}",
                    "// {} range {}..{}",
                    i.name.text,
                    lo.map(|l| crate::types::wire_int(l, sc).to_string()).unwrap_or_default(),
                    hi.map(|h| crate::types::wire_int(h, sc).to_string()).unwrap_or_default()
                ),
                (_, Some((lo, hi))) => tr!(
                    "// {} 範囲 {}..{}",
                    "// {} range {}..{}",
                    i.name.text,
                    self.go_value(lo, ty.present()),
                    self.go_value(hi, ty.present())
                ),
                _ => format!("// {}", i.name.text),
            };
            o.push_str(&format!("\t{}{CELL}{}{CELL}{doc}\n", pascal(&pub_name(&i.name)), self.go_ty(&ty)));
        }
        // A sequence a walk reads is a field like the rest (§15.56), each its own (§15.219).
        for (s, el) in self.f.elements.iter().enumerate() {
            o.push_str(&format!(
                "\t{}{CELL}[]{}{CELL}// {}\n",
                pascal(&pub_name(&el.name)),
                self.element_type(s),
                el.name.text
            ));
        }
        o.push_str("}\n\n");
        // One element is a row of inputs, so it is a struct like `Input`.
        for (s, el) in self.f.elements.iter().enumerate() {
            o.push_str(&format!("// {}\ntype {} struct {{\n", tr!("{} の一件", "one of {}", el.name.text), self.element_type(s)));
            for fd in &el.fields {
                o.push_str(&format!(
                    "\t{}{CELL}{}{CELL}// {}\n",
                    pascal(&pub_name(&fd.name)),
                    self.go_ty(&self.ty_of(&fd.name.text)),
                    fd.name.text
                ));
            }
            o.push_str("}\n\n");
        }

        let ret = if outs.len() == 1 {
            self.go_ty(&self.ty_of(&outs[0].name.text))
        } else {
            o.push_str("type Output struct {\n");
            for od in outs {
                o.push_str(&format!("\t{}{CELL}{}\n", pascal(&pub_name(&od.name)), self.go_ty(&self.ty_of(&od.name.text))));
            }
            o.push_str("}\n\n");
            "Output".into()
        };
        let zero = if outs.len() == 1 {
            self.go_zero(&self.ty_of(&outs[0].name.text))
        } else {
            "Output{}".to_string()
        };

        // Go's defined types cannot be mixed in arithmetic. Brands are enforced on the public
        // surface (Input and the return value) while internal arithmetic runs on plain int64 (the
        // flip side of the asymmetry in §8.3).
        let local = |n: &str| -> String {
            match self.f.inputs.iter().find(|i| i.name.text == n) {
                Some(i) => {
                    let f = format!("in.{}", pascal(&pub_name(&i.name)));
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                        format!("int64({f})")
                    } else {
                        f
                    }
                }
                None => self.ident(n),
            }
        };

        // The public function keeps the plain signature; the branches live in its traced
        // twin, which also returns the rows that matched (§15.33).
        let traced = format!("{fname}Traced");
        o.push_str(&format!("// {fname} {}\n", plain_doc(&self.f.name.text, &self.f.version, &traced)));
        o.push_str(&format!(
            "func {fname}(in Input) ({ret}, error) {{\n\tout, _, err := {traced}(in)\n\treturn out, err\n}}\n\n"
        ));
        o.push_str(&format!("// {traced} {}\n", traced_doc(&self.f.name.text, &self.f.version)));
        // A rule that picks by weight runs its walk in a function of its own, which gives back the
        // running totals of the weights too (§15.225). Every early return there has one value more.
        let walked = self.weighted_fold().map(|(i, _)| (self.walked_fn("go"), self.fold_names(i).cuts));
        let zero = match &walked {
            Some(_) => format!("{zero}, nil"),
            None => zero,
        };
        match &walked {
            Some((w, _)) => {
                o.push_str(&format!("func {traced}(in Input) ({ret}, []Fired, error) {{\n\tout, trace, _, err := {w}(in)\n\treturn out, trace, err\n}}\n\n"));
                o.push_str(&format!("// {w} {}\n", walked_doc()));
                o.push_str(&format!("func {w}(in Input) ({ret}, []Fired, []int64, error) {{\n"));
            }
            None => o.push_str(&format!("func {traced}(in Input) ({ret}, []Fired, error) {{\n")),
        }

        // An input with two or more things wrong with it is refused for the first of them in the
        // order every door looks (§15.328): each input of its kind, then each element of each
        // sequence, then each input inside its range, then the constraints. In Go the kind to look
        // at is an enum's: the struct holds the rest.
        for kinds in [true, false] {
            for i in &self.f.inputs {
                let v = local(&i.name.text);
                let ty = self.ty_of(&i.name.text);
                // An optional input is guarded as the type it wraps whenever it has a value (§15.201).
                let optional = matches!(ty, Ty::Opt(_));
                let ty = ty.present().clone();
                // A pointer: the guard reads what it points at, under the name of the input.
                let (v, wrap) = if optional {
                    let x = self.ident(&i.name.text);
                    (x.clone(), Some((format!("\tif {v} != nil {{\n\t\t{x} := *{v}\n"), "\t}\n".to_string())))
                } else {
                    (v, None)
                };
                let val = go_i64(&v);
                let mut o = Guarded::new(&mut o, wrap, "\t");
                match &ty {
                    Ty::Enum(n) if kinds => o.push_str(&format!(
                        "\tif !{v}.Valid() {{\n\t\treturn {zero}, nil, &RuleInputError{{What: \"{}\", Value: {val}, HasValue: true}}\n\t}}\n",
                        crate::door::not_in_enum(&i.name.text, n)
                    )),
                    // The values the range of an ordered enum lets in, first to last (§15.210).
                    Ty::Enum(_) => {
                        if let Some((lo, hi)) = self.c.enum_ranges.get(&i.name.text).and_then(|vs| Some((vs.first()?, vs.last()?))) {
                            o.push_str(&format!(
                                "\tif {v} < {} || {v} > {} {{\n\t\treturn {zero}, nil, &RuleInputError{{What: \"{}\", Value: {val}, HasValue: true}}\n\t}}\n",
                                self.go_value(lo, &ty),
                                self.go_value(hi, &ty),
                                crate::door::out_of_range(&i.name.text)
                            ));
                        }
                    }
                    // Each end the range writes, and −(2^63−1) where it writes no lower one: int64
                    // holds one value below that, and the door refuses it (§15.295).
                    Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date if !kinds => {
                        if let Some(test) = or_out_of(&v, crate::door::ends(self.c, &i.name.text, false), "") {
                            o.push_str(&format!(
                                "\tif {test} {{\n\t\treturn {zero}, nil, &RuleInputError{{What: \"{}\", Value: {val}, HasValue: true}}\n\t}}\n",
                                crate::door::out_of_range(&i.name.text)
                            ));
                        }
                    }
                    _ => {}
                }
            }
            if kinds {
                o.push_str(&self.go_element_kinds(&zero));
            }
        }

        o.push_str(&self.constraint_guards(&local, Lang::Go, "\t", |m| {
            format!("\t\treturn {zero}, nil, &RuleInputError{{What: \"{m}\"}}\n")
        }));

        let trace = self.temp("trace");
        o.push_str(&format!("\tvar {trace} []Fired\n"));

        if self.f.walks() {
            o.push_str(&self.go_walks(&local, &trace, &zero));
        } else {
            o.push_str(&self.go_items(&local, &trace, Phase::All, &zero));
        }

        // Every output takes the same three steps: its source — the `result` expression for
        // the first output, the binding of its own name otherwise — brought to the wire
        // scale, then its declared rounding applied exactly once (§15.16).
        let mut finals: Vec<String> = Vec::new();
        // The outputs that are already the pointer they are returned as (§15.249).
        let mut bare: Vec<bool> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let ty = self.ty_of(out_name);
            // An optional output is its column's pointer. A number on it goes through the
            // output's steps only when it is there, into a pointer of the output's own type.
            let opt = self.opt_out(oi);
            let numeric = ty.present().is_numeric();
            if opt && !numeric {
                finals.push(local(out_name));
                bare.push(true);
                continue;
            }
            let (pre, indent) = if opt { (format!("*{}", local(out_name)), "\t\t") } else { (local(out_name), "\t") };
            let mut block = String::new();
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => Expr2 { text: pre, scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let res = self.onto_wire(res, os, od);
            let body = match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    if res.scale != os {
                        let raw = self.temp(&raw_base(oi));
                        block.push_str(&format!(
                            // The cast needs its own parentheses: an expression that begins with
                            // a call rather than a `(` glued itself to the type name.
                            "{indent}{raw} := int64({}){CELL}// {}\n",
                            go_expr(&res.text),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("round{}({raw}, {}) / {}", pascal(mode_fn(m)), grid_i, res.scale / os)
                    } else {
                        format!("round{}(int64({}), {})", pascal(mode_fn(m)), go_expr(&res.text), grid_i)
                    }
                }
                // Only a number needs the widening cast; `int64(可否)` does not compile.
                None if !ty.is_numeric() && !opt => go_expr(&res.text),
                None => format!("int64({})", go_expr(&res.text)),
            };
            if opt {
                let v = local(out_name);
                let present = self.go_ty(ty.present());
                let out = self.temp(&format!("{v}Out"));
                let one = self.temp("v");
                o.push_str(&format!("\tvar {out} *{present}\n\tif {v} != nil {{\n{block}\t\t{one} := {present}({body})\n\t\t{out} = &{one}\n\t}}\n"));
                finals.push(out);
                bare.push(true);
            } else {
                o.push_str(&block);
                finals.push(body);
                bare.push(false);
            }
        }
        let cuts = walked.as_ref().map(|(_, c)| format!(", {c}")).unwrap_or_default();
        if outs.len() == 1 {
            let v = if bare[0] { finals[0].clone() } else { format!("{ret}({})", finals[0]) };
            o.push_str(&format!("\treturn {v}, {trace}{cuts}, nil\n}}\n"));
        } else {
            // Each output gets its own rounding, applied exactly once (§7.2).
            let fields: Vec<String> = outs
                .iter()
                .zip(&finals)
                .zip(&bare)
                .map(|((od, body), bare)| {
                    let v = if *bare { body.clone() } else { format!("{}({body})", self.go_ty(&self.ty_of(&od.name.text))) };
                    format!("{}: {v}", pascal(&pub_name(&od.name)))
                })
                .collect();
            o.push_str(&format!("\treturn Output{{{}}}, {trace}{cuts}, nil\n}}\n", fields.join(", ")));
        }
        o
    }

    fn go_table(&self, t: &Table, local: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = format!("\t// {}\n", self.table_head(t));
        // In Go a variable declared inside an if does not escape it, so declare them up front.
        for oc in &t.outputs {
            let t2 = self.ty_of(&oc.name.text);
            let ty = if matches!(t2, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                "int64".to_string()
            } else {
                self.go_ty(&t2)
            };
            o.push_str(&format!("\tvar {} {ty}\n", self.ident(&oc.name.text)));
        }
        for (ri, row) in t.rows.iter().enumerate() {
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.go_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            // A set cell comes in parentheses so that it can sit beside other conditions; alone
            // on the `if`, gofmt strips them, so they are stripped here.
            let cond: String = match conds.as_slice() {
                [] => "true".to_string(),
                [one] => unparen(one).to_string(),
                many => many.join(" && "),
            };
            let kw = if ri == 0 { "\tif" } else { " else if" };
            let cells: Vec<String> = row.cells.iter().map(cell_src).chain(row.outs.iter().map(out_src)).collect();
            let line = self.row_head(t, ri, &cells.join(" | "));
            if ri == 0 {
                o.push_str(&format!("{kw} {cond} {{ // {line}\n"));
            } else {
                o.push_str(&format!("\t}}{kw} {cond} {{ // {line}\n"));
            }
            for (oi, oc) in t.outputs.iter().enumerate() {
                let col = self.ident(&oc.name.text);
                let col_ty = self.ty_of(&oc.name.text);
                // An optional column is a pointer, `nil` when there is no value (§15.249). Go takes
                // no address of a constant, so a value is put where `new` points.
                let opt = |v: String| -> String {
                    let Ty::Opt(inner) = &col_ty else { return format!("\t\t{col} = {v}\n") };
                    let inner_go = self.go_ty(inner);
                    let v = if matches!(inner.as_ref(), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) { format!("{inner_go}({})", go_expr(&v)) } else { v };
                    format!("\t\t{col} = new({inner_go})\n\t\t*{col} = {v}\n")
                };
                let line = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => opt(self.int_lit(n, &col_ty, self.scale(&oc.name.text))),
                    Some(OutCell::Lit(l)) => opt(match l {
                        Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                        Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                        Lit::Word(w) => self.go_value(w, &col_ty),
                        Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                        Lit::Str(x) => str_lit(x),
                        _ => "0".into(),
                    }),
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            opt("true".into())
                        } else if w == crate::kw::FALSE {
                            opt("false".into())
                        } else if self.is_value(w) {
                            opt(self.go_value(w, &col_ty))
                        } else if let (Ty::Opt(inner), Ty::Opt(_)) = (&col_ty, self.ty_of(w)) {
                            // A value that may be absent passes on as the pointer it is, and is
                            // scaled only when it is there.
                            let v = local(w);
                            let numeric = matches!(inner.as_ref(), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date);
                            let scaled = self.rescaled(w, &oc.name.text, format!("int64(*{v})"));
                            if !numeric || scaled == format!("int64(*{v})") {
                                format!("\t\t{col} = {v}\n")
                            } else {
                                let inner_go = self.go_ty(inner);
                                format!("\t\t{col} = nil\n\t\tif {v} != nil {{\n\t\t\t{col} = new({inner_go})\n\t\t\t*{col} = {inner_go}({})\n\t\t}}\n", go_expr(&scaled))
                            }
                        } else {
                            opt(self.rescaled(w, &oc.name.text, local(w)))
                        }
                    }
                    Some(OutCell::Nothing) => format!("\t\t{col} = nil\n"),
                    None => format!("\t\t{col} = 0\n"),
                };
                o.push_str(&line);
            }
            let (tn, rn, lbl) = self.fired_of(t, ri);
            o.push_str(&format!("\t\t{trace} = append({trace}, Fired{{{tn:?}, {rn}, {lbl:?}}})\n"));
        }
        o.push_str(&format!(
            "\t}} else {{\n\t\tpanic(\"{}\")\n\t}}\n",
            tr!("到達不能: 完全性は rulec が静的に検査済み", "unreachable: completeness was statically checked by rulec")
        ));
        for oc in &t.outputs {
            o.push_str(&self.go_unread(&oc.name.text));
        }
        o.push_str(&self.guards(t, local, Lang::Go, "\t", |name, i, j| {
            format!(
                "\t\treturn {}, nil, &RuleContradictionError{{What: \"{}\"}}\n",
                if self.f.outputs.len() == 1 {
                    self.go_zero(&self.ty_of(&self.f.outputs[0].name.text))
                } else {
                    "Output{}".into()
                },
                tr!("表 {name}: {i} と {j} が同時に当てはまりました", "table {name}: {i} and {j} matched at the same time")
            )
        }));
        o
    }
}

/// Adapt an expression built for Python to Go's spelling. Only integer division differs.
/// A definition that is one literal (`define 介護保険料率 = 1.62%`) would be declared `int`
/// by `:=`, and the first `+` with an `int64` neighbour would not compile (§15.72).
fn go_typed(s: &str) -> String {
    if s.parse::<i128>().is_ok() { format!("int64({s})") } else { s.to_string() }
}

fn go_expr(s: &str) -> String {
    let t = arith::go_expr(&truths(s, "true", "false"))
        .replace(" // ", " / ")
        .replace("_round_down(", "roundDown(")
        .replace("_round_up(", "roundUp(")
        .replace("_round_half_down(", "roundHalfDown(")
        .replace("_round_half(", "roundHalf(")
        .replace("_round_bankers(", "roundBankers(")
        .replace("_min(", "minInt(")
        .replace("_max(", "maxInt(");
    gofmt_spacing(&t).unwrap_or_else(|| tighten_nested_products(&t))
}

/// An expression as `gofmt` spaces it (§8.5): the blanks around a binary operator are kept or
/// dropped by the rule `go/printer` follows, read off the expression itself. The generator writes
/// every operator with a blank on each side and parenthesizes every subexpression; `gofmt` drops
/// the blanks of an operator deep enough in the expression — inside a call of two arguments or
/// more, an operand of another operator — where they would not help a reader see its shape:
/// `(roundHalfDown((line_total * 10), 100) / 100)` comes back as `(roundHalfDown((line_total*10), 100) / 100)`.
/// The depth is `go/printer`'s: 1 at the top, one more for each argument list of two or more
/// and each operand of an operator of another precedence, one less inside a pair of parentheses.
/// None for a shape it does not read (a brace, an index), which keeps the older rule
/// (`tighten_nested_products`).
fn gofmt_spacing(s: &str) -> Option<String> {
    #[derive(Debug)]
    enum Node {
        Atom(String),
        Paren(Box<Node>),
        Call(String, Vec<Node>),
        Unary(String, Box<Node>),
        Binary(String, Box<Node>, Box<Node>),
    }
    fn prec(op: &str) -> Option<u8> {
        Some(match op {
            "||" => 1,
            "&&" => 2,
            "==" | "!=" | "<" | "<=" | ">" | ">=" => 3,
            "+" | "-" | "|" | "^" => 4,
            "*" | "/" | "%" | "<<" | ">>" | "&" | "&^" => 5,
            _ => return None,
        })
    }
    // The tokens: an atom (a name, a selector, a number, a string), a parenthesis, a comma, an operator.
    let cs: Vec<char> = s.chars().collect();
    let mut toks: Vec<String> = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == ' ' {
            i += 1;
        } else if c == '"' {
            let start = i;
            i += 1;
            while i < cs.len() && cs[i] != '"' {
                if cs[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            toks.push(cs[start..i.min(cs.len())].iter().collect());
        } else if c.is_alphanumeric() || c == '_' || c == '.' {
            let start = i;
            while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_' || cs[i] == '.') {
                i += 1;
            }
            toks.push(cs[start..i].iter().collect());
        } else if c == '(' || c == ')' || c == ',' {
            toks.push(c.to_string());
            i += 1;
        } else {
            let two: String = cs[i..(i + 2).min(cs.len())].iter().collect();
            if ["||", "&&", "==", "!=", "<=", ">=", "<<", ">>", "&^"].contains(&two.as_str()) {
                toks.push(two);
                i += 2;
            } else if "+-*/%<>|^&!".contains(c) {
                toks.push(c.to_string());
                i += 1;
            } else {
                return None;
            }
        }
    }
    struct P {
        toks: Vec<String>,
        at: usize,
    }
    impl P {
        fn peek(&self) -> Option<&str> {
            self.toks.get(self.at).map(|t| t.as_str())
        }
        fn next(&mut self) -> Option<String> {
            let t = self.toks.get(self.at).cloned();
            self.at += 1;
            t
        }
        fn expr(&mut self, min: u8) -> Option<Node> {
            let mut l = self.unary()?;
            while let Some(op) = self.peek().map(str::to_string) {
                let Some(p) = prec(&op) else { break };
                if p < min {
                    break;
                }
                self.at += 1;
                let r = self.expr(p + 1)?;
                l = Node::Binary(op, Box::new(l), Box::new(r));
            }
            Some(l)
        }
        fn unary(&mut self) -> Option<Node> {
            match self.peek()? {
                "-" | "!" | "+" | "^" => {
                    let op = self.next()?;
                    Some(Node::Unary(op, Box::new(self.unary()?)))
                }
                _ => self.primary(),
            }
        }
        fn primary(&mut self) -> Option<Node> {
            let t = self.next()?;
            if t == "(" {
                let e = self.expr(1)?;
                (self.next()? == ")").then(|| Node::Paren(Box::new(e)))
            } else if t.chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '"') {
                if self.peek() == Some("(") && !t.starts_with('"') {
                    self.at += 1;
                    let mut args = Vec::new();
                    if self.peek() == Some(")") {
                        self.at += 1;
                        return Some(Node::Call(t, args));
                    }
                    loop {
                        args.push(self.expr(1)?);
                        match self.next()?.as_str() {
                            "," => continue,
                            ")" => break,
                            _ => return None,
                        }
                    }
                    Some(Node::Call(t, args))
                } else {
                    Some(Node::Atom(t))
                }
            } else {
                None
            }
        }
    }
    let mut p = P { toks, at: 0 };
    let root = p.expr(1)?;
    if p.at != p.toks.len() {
        return None;
    }
    // `go/printer`: walkBinary, cutoff, diffPrec, reduceDepth, and the cases of expr1.
    fn walk(e: &Node) -> (bool, bool, u8) {
        let Node::Binary(op, l, r) = e else { return (false, false, 0) };
        let p = prec(op).unwrap_or(0);
        let (mut has4, mut has5, mut problem) = (p == 4, p == 5, 0u8);
        if let Node::Binary(lop, ..) = &**l {
            if prec(lop).unwrap_or(0) >= p {
                let (a, b, m) = walk(l);
                has4 |= a;
                has5 |= b;
                problem = problem.max(m);
            }
        }
        match &**r {
            Node::Binary(rop, ..) if prec(rop).unwrap_or(0) > p => {
                let (a, b, m) = walk(r);
                has4 |= a;
                has5 |= b;
                problem = problem.max(m);
            }
            Node::Unary(u, _) => match format!("{op}{u}").as_str() {
                "&&" | "&^" => problem = 5,
                "++" | "--" => problem = problem.max(4),
                _ => {}
            },
            _ => {}
        }
        (has4, has5, problem)
    }
    fn cutoff(e: &Node, depth: u32) -> u8 {
        let (has4, has5, problem) = walk(e);
        if problem > 0 {
            problem + 1
        } else if has4 && has5 {
            if depth == 1 { 5 } else { 4 }
        } else if depth == 1 {
            6
        } else {
            4
        }
    }
    fn print(e: &Node, depth: u32, out: &mut String) {
        match e {
            Node::Atom(a) => out.push_str(a),
            Node::Paren(x) => {
                // gofmt drops a pair around what is already in a pair
                if matches!(**x, Node::Paren(_)) {
                    print(x, depth, out);
                } else {
                    out.push('(');
                    print(x, depth.saturating_sub(1).max(1), out);
                    out.push(')');
                }
            }
            Node::Call(f, args) => {
                let d = if args.len() > 1 { depth + 1 } else { depth };
                out.push_str(f);
                out.push('(');
                for (k, a) in args.iter().enumerate() {
                    if k > 0 {
                        out.push_str(", ");
                    }
                    print(a, d, out);
                }
                out.push(')');
            }
            Node::Unary(op, x) => {
                out.push_str(op);
                print(x, depth, out);
            }
            Node::Binary(op, l, r) => {
                let p = prec(op).unwrap_or(0);
                let blank = p < cutoff(e, depth);
                let diff = match &**l {
                    Node::Binary(lop, ..) if prec(lop) == Some(p) => 0,
                    _ => 1,
                };
                print(l, depth + diff, out);
                out.push_str(if blank { " " } else { "" });
                out.push_str(op);
                out.push_str(if blank { " " } else { "" });
                print(r, depth + 1, out);
            }
        }
    }
    let mut out = String::new();
    print(&root, 1, &mut out);
    Some(out)
}

/// Match one habit of `gofmt`: a product written inside a group that sits two or more call
/// arguments deep loses the spaces around its operator.
///
/// The generator formats its own Go rather than shelling out to `gofmt` (§8.5: the output
/// must not depend on the version of a tool installed on the machine), and a test then holds
/// it to `gofmt -l` being empty. That test is what turned this up: `roundDown(minInt(x, (int64(in.Cap) * 100)), (1 * 100))`
/// comes back from `gofmt` with the first product tightened and the second left alone. The
/// rule below reproduces that on the shapes the generator emits — at one call deep the spaces
/// stay, which is what `gofmt` does too.
fn tighten_nested_products(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    // For every open paren still on the stack: was it a call's, or a grouping's?
    let mut stack: Vec<bool> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == '(' {
            let call = i > 0 && (b[i - 1].is_alphanumeric() || b[i - 1] == '_');
            stack.push(call);
        } else if c == ')' {
            stack.pop();
        }
        let calls = stack.iter().filter(|x| **x).count();
        let in_group = stack.last() == Some(&false);
        if in_group && calls >= 2 && c == ' ' && i + 2 < b.len() && (b[i + 1] == '*' || b[i + 1] == '/') && b[i + 2] == ' ' {
            out.push(b[i + 1]);
            i += 3;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// The value in a Go error is an `int64`; a wire value already is one, so the cast is
/// written only when it is not (an enum, a length).
fn go_i64(v: &str) -> String {
    if v.starts_with("int64(") && v.ends_with(')') { v.to_string() } else { format!("int64({v})") }
}

fn round_go() -> String {
    tr!(
        "// 丸めの五つのモード。負の向きと半分ちょうどまで仕様どおりに固定する。\n\
         // 言語の素の除算に任せない（Python の // は −∞ 方向、Go は 0 方向）。\n",
        "// The five rounding modes. Negative values and exact halves are pinned to the spec as well;\n\
         // nothing is left to the language's native division (Python's // goes toward −∞, Go's toward 0).\n"
    ) + ROUND_GO_BODY
}

const ROUND_GO_BODY: &str = r#"
// RuleInputError is a contract violation by the caller: outside the declared input domain.
// The sentence and the value travel apart, so a caller can react to which input
// was refused without parsing the text back.
type RuleInputError struct {
	What     string
	Value    int64
	HasValue bool
}

func (e *RuleInputError) Error() string {
	if !e.HasValue {
		return e.What
	}
	return fmt.Sprintf("%s: %d", e.What, e.Value)
}

// RuleContradictionError is a contradiction in the rule itself, not a mistake by the caller.
type RuleContradictionError struct {
	What string
}

func (e *RuleContradictionError) Error() string {
	return e.What
}
func absMod(x, g int64) (int64, int64, bool) {
	neg := x < 0
	if neg {
		x = -x
	}
	return x / g, x % g, neg
}

func roundDown(x, g int64) int64 {
	q, _, neg := absMod(x, g)
	v := q * g
	if neg {
		return -v
	}
	return v
}

func roundUp(x, g int64) int64 {
	q, r, neg := absMod(x, g)
	if r != 0 {
		q++
	}
	v := q * g
	if neg {
		return -v
	}
	return v
}

func roundHalf(x, g int64) int64 {
	q, r, neg := absMod(x, g)
	if 2*r >= g {
		q++
	}
	v := q * g
	if neg {
		return -v
	}
	return v
}

func roundHalfDown(x, g int64) int64 {
	q, r, neg := absMod(x, g)
	if 2*r > g {
		q++
	}
	v := q * g
	if neg {
		return -v
	}
	return v
}

func minInt(a, b int64) int64 {
	if a < b {
		return a
	}
	return b
}

func maxInt(a, b int64) int64 {
	if a > b {
		return a
	}
	return b
}

func roundBankers(x, g int64) int64 {
	q, r, neg := absMod(x, g)
	if 2*r > g || (2*r == g && q%2 == 1) {
		q++
	}
	v := q * g
	if neg {
		return -v
	}
	return v
}

"#;

/// Our own column alignment, matching gofmt's tabwriter.
///
/// Running gofmt as a later stage would make the generated code depend on whichever gofmt is
/// installed in that environment, breaking the determinism of §8.5. Alignment is local to a run
/// of declarations, so doing it ourselves is enough. The control character used as a separator
/// marks consecutive lines with the same number of columns as one block to align together.
const CELL: char = '\u{1f}';
const IMPORT_MARK: &str = "\u{1e}IMPORTS\u{1e}";

/// Column width is measured in runes. That is how Go's text/tabwriter counts, so measuring by
/// display width (`diag::width`, which counts a full-width character as 2) would drift from gofmt
/// by one character wherever Japanese identifiers line up. §8.5 requires `gofmt -l` to print
/// nothing, so we follow gofmt's logic rather than the visual one.
fn cells_wide(s: &str) -> usize {
    s.chars().count()
}

/// Drop the parentheses wrapping the whole expression. The generator parenthesizes every
/// subexpression uniformly, which leaves one redundant outer pair on the right-hand side of an
/// assignment. Python's formatter wants it gone, so to keep `ruff format --check` green we drop
/// it at generation time (honoring §8.5's "do not rely on a later formatting stage" on the Python
/// side too). Go is left alone, since gofmt does not remove it.
fn unparen(s: &str) -> &str {
    let t = s.trim();
    let Some(inner) = t.strip_prefix('(').and_then(|x| x.strip_suffix(')')) else { return s };
    // Drop the outer pair only when it really matches itself. `(a) + (b)` is left alone.
    let mut depth = 0i32;
    for ch in inner.chars() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return s;
                }
            }
            _ => {}
        }
    }
    if depth == 0 { inner } else { s }
}

/// Fix up PEP 8 blank lines (E301–E305) in one place. Output assembly is scattered across the
/// generator, and forgetting a single blank line turns the formatter red. Rather than being
/// careful at every emitting site, run one pass at the end: two blank lines around top-level
/// `class` / `def` / decorators, at most one before any other top-level statement. A comment
/// stays against what it is about: the lines of one comment are not parted, and the line under
/// it is not parted from it. Nor is a run of names bound to other names (`_int = int`).
fn pep8_blanks(src: &str) -> String {
    let lines: Vec<&str> = src.trim_end().split('\n').collect();
    let top = |l: &str| !l.is_empty() && !l.starts_with(char::is_whitespace);
    let is_def = |l: &str| l.starts_with("class ") || l.starts_with("def ") || l.starts_with('@');
    let is_name = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_');
    let is_binding = |l: &str| l.split_once(" = ").is_some_and(|(a, b)| is_name(a) && is_name(b));
    let mut out: Vec<String> = Vec::new();
    let mut in_block = false; // whether the previous top-level construct was a def / class body
    // the comments the file begins with, the head (ritsu's DESIGN 9.2), stay together
    let mut in_head = true;
    let mut after_comment = false; // the previous top-level line is a comment, and not of the head
    let mut after_binding = false; // the previous top-level line binds a name to another name
    for (i, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            in_head = false;
            continue; // blank lines are dropped and reinserted where needed
        }
        in_head = in_head && line.starts_with('#');
        if top(line) {
            let comment = line.starts_with('#');
            // A comment is about the line under it, and that decides what stands above the comment.
            let about = if comment { lines[i + 1..].iter().find(|l| top(l) && !l.starts_with('#')).unwrap_or(&"") } else { line };
            let starts = is_def(about);
            let want = if in_head || after_comment || (after_binding && is_binding(line)) {
                0
            } else if starts || in_block {
                2
            } else {
                1
            };
            if !out.is_empty() {
                let have = out.iter().rev().take_while(|l| l.is_empty()).count();
                for _ in have..want {
                    out.push(String::new());
                }
            }
            if is_def(line) {
                in_block = true;
            } else if !comment {
                in_block = false;
            }
            after_comment = comment && !in_head;
            after_binding = !comment && is_binding(line);
        }
        out.push(line.to_string());
    }
    // Drop blank lines at the start of the file.
    while out.first().is_some_and(|l| l.is_empty()) {
        out.remove(0);
    }
    out.push(String::new());
    out.join("\n")
}

fn align(src: &str) -> String {
    let lines: Vec<&str> = src.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        if !lines[i].contains(CELL) {
            out.push(lines[i].to_string());
            i += 1;
            continue;
        }
        let n = lines[i].matches(CELL).count();
        // gofmt aligns a run of lines at one indentation: a line that opens a block and the
        // first line inside it are two runs, each aligned on its own.
        let indent = |l: &str| l.len() - l.trim_start_matches('\t').len();
        let depth = indent(lines[i]);
        let start = i;
        while i < lines.len() && lines[i].contains(CELL) && lines[i].matches(CELL).count() == n && indent(lines[i]) == depth {
            i += 1;
        }
        let rows: Vec<Vec<&str>> = lines[start..i].iter().map(|l| l.split(CELL).collect()).collect();
        let mut w = vec![0usize; n];
        for r in &rows {
            for (k, c) in r.iter().take(n).enumerate() {
                w[k] = w[k].max(cells_wide(c));
            }
        }
        for r in rows {
            let mut line = String::new();
            for (k, c) in r.iter().enumerate() {
                line.push_str(c);
                if k < n && r[k + 1..].iter().any(|x| !x.trim().is_empty()) {
                    line.push_str(&" ".repeat(w[k] - cells_wide(c) + 1));
                }
            }
            out.push(line.trim_end().to_string());
        }
    }
    out.join("\n")
}

// ---------------------------------------------------------------------------
// Runners (§9.3: feed the vectors through every generated language and compare the answers)
// ---------------------------------------------------------------------------

impl<'a> Gen<'a> {
    /// A Python runner that reads JSONL from stdin and prints just the outputs, one record
    /// per line. It is also the worked example of the calling convention, so it constructs
    /// each branded argument the way a caller has to — `mypy --strict` checks both (§15.22).
    /// One value off the wire, as the Python the module takes.
    ///
    /// One reader, used for the rule's own inputs, for an element's fields, and by the
    /// Connect runner (§15.112). There used to be a second copy inline in `python_runner`,
    /// and it was the one that ran — which is how the optional input got no arm anywhere and
    /// came out as `int(d[…])` (§15.88).
    /// One input read off the wire for the Python module, the way every door reads it
    /// (§15.204): there at all, of its kind, and only then built into the module's type. `src`
    /// is the object it is read from and `name` the input; a refusal is the module's own
    /// `RuleInputError`, with the sentence every language says. What is left to the module's
    /// door is what it holds a caller to as well: the range, the constraints, the days.
    pub fn py_read(&self, ty: &Ty, src: &str, name: &str) -> String {
        self.py_read_as(ty, src, name, name)
    }

    /// [`Gen::py_read`], with the sentences calling the value `said`: for a field of an element
    /// `lines[\u{1}].amount`, which the reader of the elements puts its index `i` into (§15.324).
    fn py_read_as(&self, ty: &Ty, src: &str, name: &str, said: &str) -> String {
        let lit = |x: String| -> String { if names_element(&x) { py_said(&x, "i", "") } else { format!("{x:?}") } };
        let optional = matches!(ty, Ty::Opt(_));
        let take = format!(
            "_take({src}, {name:?}, {}{})",
            lit(crate::door::missing(said)),
            if optional { ", True" } else { "" }
        );
        let one = |t: &Ty, v: &str| -> String {
            match t {
                Ty::Enum(n) => {
                    let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                    format!("_enum(m.{cls}, {v}, {})", lit(crate::door::not_in_enum(said, n)))
                }
                Ty::Bool => format!("_bool({v}, {})", lit(crate::door::not_bool(said))),
                Ty::Date => format!("_date({v}, {})", lit(crate::door::not_date(said))),
                Ty::Str => format!("_str({v}, {})", lit(crate::door::not_string(said))),
                // A branded input has to be constructed, exactly as a caller must.
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => {
                    format!("m.{}(_int({v}, {}, {}))", brand_of(t), lit(crate::door::not_integer(said)), lit(crate::door::out_of_range(said)))
                }
                _ => format!("_int({v}, {}, {})", lit(crate::door::not_integer(said)), lit(crate::door::out_of_range(said))),
            }
        };
        match ty {
            // The absent value is `null` on the wire (§10.2), and `None` in Python.
            Ty::Opt(inner) => format!("_maybe({take}, lambda x: {})", one(inner, "x")),
            t => one(t, &take),
        }
    }

    /// The elements of the sequence a walk reads, read the way one call's inputs are (§15.56,
    /// §15.204): the sequence an array, each element an object, each field as an input. A sentence
    /// about an element names it by its index `i` (§15.324).
    pub fn py_read_elements(&self, s: usize) -> String {
        let el = &self.f.elements[s];
        let seq = &el.name.text;
        let e = format!("_obj(e, {})", py_said(&crate::door::not_object(seq, crate::door::HERE), "i", ""));
        let fields: Vec<String> = el
            .fields
            .iter()
            .map(|fd| self.py_read_as(&self.ty_of(&fd.name.text), &e, &fd.name.text, &crate::door::element(seq, crate::door::HERE, &fd.name.text)))
            .collect();
        // `m.Element(x)` is a call, not a tuple: the trailing comma a one-element tuple needs
        // would only make the formatter explode the line (§15.105).
        format!(
            "[m.{}({}) for i, e in enumerate(_seq(_take(d, {seq:?}, {:?}), {:?}))]",
            self.element_type(s),
            fields.join(", "),
            crate::door::missing(seq),
            crate::door::not_sequence(seq)
        )
    }

    /// The arguments of one call as a Python runner reads them off the wire, in the order the
    /// function takes them, which is the order every reader takes them in (§15.328): the inputs
    /// `ins`, read where they stand; then each sequence a walk reads, built the same way one element
    /// at a time (§15.56), under a name of its own (§15.219). The lines that read them before the
    /// call, at `indent`, and what the tuple of the arguments is made of.
    pub(crate) fn py_call(&self, ins: &[String], indent: &str) -> (String, Vec<String>) {
        if self.f.elements.is_empty() {
            return (String::new(), ins.to_vec());
        }
        let mut prelude = String::new();
        let mut args: Vec<String> = Vec::new();
        if !ins.is_empty() {
            let tuple = if ins.len() == 1 { format!("{},", ins[0]) } else { ins.join(", ") };
            prelude.push_str(&format!("{indent}ins = ({tuple})\n"));
            args.push("*ins".to_string());
        }
        for s in 0..self.f.elements.len() {
            let rows = if s == 0 { "rows".to_string() } else { format!("rows{}", s + 1) };
            prelude.push_str(&format!("{indent}{rows} = {}\n", self.py_read_elements(s)));
            args.push(rows);
        }
        (prelude, args)
    }

    pub fn python_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        let mut ins: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            ins.push(self.py_read(&ty, "d", &i.name.text));
        }
        // `--chances` reads a line's inputs as the vectors are read (§15.225).
        let chances = {
            let (prelude, args) = self.py_call(&ins, "        ");
            self.py_chances_mode(&args, &prelude)
        };
        // A machine's traces (§15.148): the carried input is the state this language answered
        // to the call before, handed over as the value it is, not read back off the wire.
        let machine = self.machine_consts().zip(self.carried()).zip(self.carried_out_alias());
        let (mut head, mut tail, mut meta) = (String::new(), String::new(), String::new());
        if let Some((((en, _, _), (cin, _)), out)) = machine {
            let cls = self.enum_names.get(&en).cloned().unwrap_or_default();
            if let Some(k) = self.f.inputs.iter().position(|i| i.name.text == cin) {
                ins[k] = format!("(state if \"step\" in v else {})", ins[k]);
            }
            // The values carried along with the state (§15.217): each held as the module's own
            // value from one call to the next, read off the trace's first line where it starts.
            let more = self.more_carried();
            let (mut more_head, mut more_meta, mut more_start, mut more_tail) = (String::new(), String::new(), String::new(), String::new());
            for mc in &more {
                let local = format!("carried_{}", mc.in_alias);
                if let Some(k) = self.f.inputs.iter().position(|i| i.name.text == mc.input) {
                    ins[k] = format!("({local} if \"step\" in v else {})", ins[k]);
                }
                let ty = match mc.kind() {
                    MoreKind::Enum | MoreKind::Branded => format!("m.{}", self.py_ty(&mc.ty)),
                    _ => self.py_ty(&mc.ty),
                };
                more_head.push_str(&format!("{local}: {ty} = m.{}\n", mc.upper()));
                let shown = if mc.kind() == MoreKind::Enum { format!("m.{}.value", mc.upper()) } else { format!("m.{}", mc.upper()) };
                more_meta.push_str(&format!("{}: {shown}, ", mc.key()));
                more_start.push_str(&format!("        {local} = {}\n", self.py_read(&mc.ty, "v[\"carried\"]", &mc.input)));
                more_tail.push_str(&format!("        {local} = r.{}\n", mc.out_alias));
            }
            let carried_meta = if more.is_empty() { String::new() } else { format!(", \"carried\": {{{}}}", more_meta.trim_end_matches(", ")) };
            head = format!("state: m.{cls} = m.INITIAL\n{more_head}");
            meta = format!(
                "    if \"machine\" in v:\n        \
                     print(json.dumps({{\"initial\": m.INITIAL.value, \"final\": [s.value for s in m.{cls} if m.is_final(s)]{carried_meta}}}, ensure_ascii=False, separators=(\",\", \":\")))\n        \
                     continue\n    \
                 d = v[\"in\"]\n    \
                 if v.get(\"step\") == \"start\":\n        \
                     state = m.{cls}(v[\"state\"])\n{more_start}"
            );
            tail = format!("    if \"step\" in v:\n        state = {}\n{more_tail}", self.next_state_of("r", &format!(".{out}")));
        }
        // The runner prints the record the module itself writes, so the agreement test
        // holds the record function — the wire form of every input, dates included — to
        // the reference evaluator in every language (§15.35). An input the door refuses gets a
        // line saying so in the place of the record, and the next line is read (§15.204).
        let (owners, labels) = self.py_labels_mode();
        // And with `--prerequisites`, what each block's functions answer (§15.222).
        let labels = labels + &self.py_prereq_mode() + &chances;
        // The rule's module, by its name; or as a module of the package this directory makes,
        // where the module goes inside a package (§15.327).
        let package = self.py_reads_as_package();
        let only = |line: &'static str| if package { line } else { "" };
        let (importlib, os, types) = (only("import importlib\n"), only("import os\n"), only("import types\n"));
        let module = if package { py_package_lines() } else { String::new() } + &self.py_read_module(&alias, "m");
        let (prelude, args) = self.py_call(&ins, "        ");
        format!(
            "# Code generated by rulec {}. DO NOT EDIT.\n\
             import datetime\n\
             import enum\n\
             {importlib}\
             import json\n\
             {os}\
             import sys\n\
             {types}\
             from typing import Any, Callable, TypeVar\n\n\
             {module}\
             {owners}\n\
             {}\
             {labels}\
             {head}\
             for line in sys.stdin:\n    \
                 line = line.strip()\n    \
                 if not line:\n        \
                     continue\n\
                 {}    \
                 try:\n\
                 {}        \
                     args = ({})\n        \
                     r, trace = m.{alias}_traced(*args)\n    \
                 except m.RuleInputError as e:\n        \
                     _refused(\"input\", e.what)\n        \
                     continue\n    \
                 except m.RuleContradictionError as e:\n        \
                     _refused(\"contradiction\", e.what)\n        \
                     continue\n    \
                 print(m.{alias}_record(*args, r, trace))\n\
                 {tail}",
            ritsu_emit::header::VERSION,
            PY_WIRE,
            if meta.is_empty() { "    d = json.loads(line)[\"in\"]\n".to_string() } else { format!("    v = json.loads(line)\n{meta}") },
            prelude,
            if args.len() == 1 { format!("{},", args[0]) } else { args.join(", ") }
        )
    }

    /// What the Python runner answers with `--labels` (§15.211): for each line, an enum's name
    /// and a value's, the display name its language gives. The imports of the `enums` modules
    /// it reads them from come first.
    fn py_labels_mode(&self) -> (String, String) {
        let enums = self.labeled_enums();
        if enums.is_empty() {
            return (String::new(), String::new());
        }
        let mut owners: Vec<String> = Vec::new();
        let mut arms = String::new();
        for (i, (en, ty, owner)) in enums.iter().enumerate() {
            let m = match owner {
                Some(o) => {
                    if !owners.contains(o) {
                        owners.push(o.clone());
                    }
                    o.clone()
                }
                None => "m".to_string(),
            };
            let kw = if i == 0 { "if" } else { "elif" };
            arms.push_str(&format!("        {kw} en == {}:\n            print({m}.{}({m}.{ty}(v)))\n", str_lit(en), self.py_label_fn(en)));
        }
        (
            owners.iter().map(|o| self.py_read_module(o, o)).collect(),
            format!(
                "if \"--labels\" in sys.argv[1:]:\n    for line in sys.stdin:\n        en, v = line.rstrip(\"\\n\").split(\"\\t\")\n{arms}    sys.exit(0)\n\n"
            ),
        )
    }

    /// Whether the Python beside the rule's module (the runner, the MCP server) reads the module as
    /// a module of the package its directory makes (§15.327): where the module goes inside a
    /// package (`--python-package`) and reads the modules of `enums` files by relative imports,
    /// which a module read by its name at the top of a directory cannot do. A module that reads no
    /// `enums` file is the same either way, and is read by its name.
    pub(crate) fn py_reads_as_package(&self) -> bool {
        self.python_package && !self.foreign_owners().is_empty()
    }

    /// The line that reads the module `module` beside this one as `name`: by its name, or from the
    /// package of [`py_package_lines`] (§15.327).
    pub(crate) fn py_read_module(&self, module: &str, name: &str) -> String {
        if self.py_reads_as_package() {
            format!("{name} = importlib.import_module(\"{PY_PACKAGE}.{module}\")\n")
        } else if module == name {
            format!("import {module}\n")
        } else {
            format!("import {module} as {name}\n")
        }
    }

    /// A Go runner that does the same. encoding/json is in the standard library. An input is
    /// read the way every door reads it (§15.204) — the numbers as the digits that were written,
    /// so that 1.5 is not read as 1 — and one the door refuses gets a line saying so in the
    /// place of the record.
    pub fn go_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        let pkg = crate::backend::go_package(&alias);
        let fname = pascal(&alias);
        // One field of a JSON object into one field of a struct. The element loop below reads
        // the same shapes out of its own object, so the two stay one piece of code.
        // `said` is what the sentences call the value: the name, or for a field of an element its
        // place, `lines[\u{1}].amount`, which takes the index `i` of the element (§15.324).
        let field = |name: &crate::ast::Name, said: &str, ty: &Ty, into: &str, src: &str, ind: &str| -> String {
            let jp = &name.text;
            let lit = |x: String| -> String { go_said_with(&x, "i", "", &|p| format!("{p:?}")) };
            let g = pascal(&pub_name(name));
            let optional = matches!(ty, Ty::Opt(_));
            let take = format!("take({src}, {jp:?}, {}, {optional})", lit(crate::door::missing(said)));
            let one = |v: &str| -> String {
                match ty.present() {
                    Ty::Enum(n) => {
                        let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                        format!("member(r.Parse{cls}, {v}, {})", lit(crate::door::not_in_enum(said, n)))
                    }
                    Ty::Bool => format!("boolean({v}, {})", lit(crate::door::not_bool(said))),
                    Ty::Date => format!("date({v}, {})", lit(crate::door::not_date(said))),
                    Ty::Str => format!("text({v}, {})", lit(crate::door::not_string(said))),
                    // A number is a plain int64, not a type the package declares, so it must
                    // not be qualified with the package name.
                    Ty::Number => format!("integer({v}, {}, {})", lit(crate::door::not_integer(said)), lit(crate::door::out_of_range(said))),
                    other => format!("r.{}(integer({v}, {}, {}))", self.go_ty(other), lit(crate::door::not_integer(said)), lit(crate::door::out_of_range(said))),
                }
            };
            if optional {
                // `null` on the wire is a nil pointer (§15.88).
                format!("{ind}if v := {take}; v != nil {{\n{ind}\tv{g} := {}\n{ind}\t{into}.{g} = &v{g}\n{ind}}}\n", one("v"))
            } else {
                format!("{ind}{into}.{g} = {}\n", one(&take))
            }
        };
        let mut fields: Vec<String> = Vec::new();
        let carried = self.carried().map(|(i, _)| i);
        // The values carried along with the state (§15.217), each in a local of its own.
        let more = if self.machine_consts().is_some() { self.more_carried() } else { Vec::new() };
        for i in &self.f.inputs {
            let one = field(&i.name, &i.name.text, &self.ty_of(&i.name.text), "in", "d", "\t\t\t\t");
            // A machine's traces (§15.148): the carried input is the state this language
            // answered to the call before, handed over as the value it is.
            if carried.as_deref() == Some(i.name.text.as_str()) {
                let g = pascal(&pub_name(&i.name));
                fields.push(format!("\t\t\tif stepping {{\n\t\t\t\tin.{g} = state\n\t\t\t}} else {{\n{one}\t\t\t}}\n"));
            } else if let Some(mc) = more.iter().find(|mc| mc.input == i.name.text) {
                let g = pascal(&pub_name(&i.name));
                fields.push(format!("\t\t\tif stepping {{\n\t\t\t\tin.{g} = carried{}\n\t\t\t}} else {{\n{one}\t\t\t}}\n", pascal(&mc.in_alias)));
            } else {
                fields.push(field(&i.name, &i.name.text, &self.ty_of(&i.name.text), "in", "d", "\t\t\t"));
            }
        }
        // A sequence arrives as an array of objects (§10.2), one per element; each its own (§15.219).
        for (s, el) in self.f.elements.iter().enumerate() {
            let jp = &el.name.text;
            let g = pascal(&pub_name(&el.name));
            // A sentence about an element names it by its index `i` (§15.324).
            let mut body = format!(
                "\t\t\tfor i, ev := range list(take(d, {jp:?}, {:?}, false), {:?}) {{\n\t\t\t\te := object(ev, {})\n\t\t\t\tvar el r.{}\n",
                crate::door::missing(jp),
                crate::door::not_sequence(jp),
                go_said_with(&crate::door::not_object(jp, crate::door::HERE), "i", "", &|p| format!("{p:?}")),
                self.element_type(s)
            );
            for fd in &el.fields {
                body.push_str(&field(&fd.name, &crate::door::element(jp, crate::door::HERE, &fd.name.text), &self.ty_of(&fd.name.text), "el", "e", "\t\t\t\t"));
            }
            body.push_str(&format!("\t\t\t\tin.{g} = append(in.{g}, el)\n\t\t\t}}\n"));
            fields.push(body);
        }
        // `--chances` reads every input of a line as the vectors are read (§15.225): the carried
        // ones too, plainly.
        let chances_fields: String = self
            .f
            .inputs
            .iter()
            .map(|i| field(&i.name, &i.name.text, &self.ty_of(&i.name.text), "in", "d", "\t\t\t"))
            .chain(fields[self.f.inputs.len()..].iter().cloned())
            .collect();
        // A rule that reads no input reads nothing out of the record's `in`, which Go does not let
        // stand unread (§15.330).
        let unread = if self.f.inputs.is_empty() && self.f.elements.is_empty() { "_ = rec.In" } else { "d := rec.In" };
        let (mut head, mut read, mut tail) = (
            String::new(),
            format!(
                "\t\tvar rec struct {{\n\t\t\tIn map[string]any `json:\"in\"`\n\t\t}}\n\t\t\
                 if err := decode(sc.Bytes(), &rec); err != nil {{\n\t\t\tpanic(err)\n\t\t}}\n\t\t\
                 {unread}\n"
            ),
            String::new(),
        );
        if let (Some((en, _, _)), Some(out)) = (self.machine_consts(), self.carried_out_alias()) {
            let cls = self.enum_names.get(&en).cloned().unwrap_or_default();
            // The state's type as the runner writes it: the rule's package's, or the owner's (§15.239).
            let state_ty = self.go_runner_enum_ty(&en);
            let all: Vec<String> = self.c.enums.get(&en).into_iter().flatten().map(|v| format!("r.{}", self.go_value(v, &Ty::Enum(en.clone())))).collect();
            // The values carried along with the state (§15.217): a local each, from the
            // module's constant; the line the traces begin with says where each starts, and the
            // first call of each trace reads it there.
            let (mut more_head, mut more_meta, mut more_start, mut more_tail) = (String::new(), Vec::<String>::new(), String::new(), String::new());
            for mc in &more {
                let local = format!("carried{}", pascal(&mc.in_alias));
                more_head.push_str(&format!("\t{local} := r.{}\n", mc.go()));
                let key = mc.key().replace('\\', "\\\\").replace('"', "\\\"");
                let shown = match mc.kind() {
                    MoreKind::Enum => format!("strconv.Quote(r.{}.String())", mc.go()),
                    MoreKind::Bool => format!("strconv.FormatBool(r.{})", mc.go()),
                    _ => format!("strconv.FormatInt(int64(r.{}), 10)", mc.go()),
                };
                more_meta.push(format!("\"{key}:\" + {shown}"));
                let decl = crate::ast::Name { text: mc.input.clone(), ascii: Some(mc.in_alias.clone()), span: crate::diag::Span::new(0, 0, 0) };
                let one = field(&decl, &mc.input, &mc.ty, "start", "cv", "\t\t\t\t");
                more_start.push_str(&format!("\t\t\t{{\n\t\t\t\tvar start r.Input\n{one}\t\t\t\t{local} = start.{}\n\t\t\t}}\n", pascal(&mc.in_alias)));
                more_tail.push_str(&format!("\t\t\t\t{local} = got.{}\n", pascal(&mc.out_alias)));
            }
            head = format!("\tstate := r.Initial\n{more_head}");
            let print_meta = if more.is_empty() {
                "fmt.Println(string(meta))".to_string()
            } else {
                format!("fmt.Println(string(meta[:len(meta)-1]) + \",\\\"carried\\\":{{\" + {} + \"}}}}\")", more_meta.join(" + \",\" + "))
            };
            if !more_start.is_empty() {
                more_start = format!("\t\t\tcv, _ := top[\"carried\"].(map[string]any)\n{more_start}");
            }
            read = format!(
                "\t\tvar top map[string]any\n\t\t\
                 if err := decode(sc.Bytes(), &top); err != nil {{\n\t\t\tpanic(err)\n\t\t}}\n\t\t\
                 if _, ok := top[\"machine\"]; ok {{\n\t\t\t\
                     fin := []string{{}}\n\t\t\t\
                     for _, s := range []{state_ty}{{{}}} {{\n\t\t\t\tif r.IsFinal(s) {{\n\t\t\t\t\tfin = append(fin, s.String())\n\t\t\t\t}}\n\t\t\t}}\n\t\t\t\
                     meta, _ := json.Marshal(struct {{\n\t\t\t\tInitial string   `json:\"initial\"`\n\t\t\t\tFinal   []string `json:\"final\"`\n\t\t\t}}{{r.Initial.String(), fin}})\n\t\t\t\
                     {print_meta}\n\t\t\t\
                     continue\n\t\t}}\n\t\t\
                 d, _ := top[\"in\"].(map[string]any)\n\t\t\
                 step, stepping := top[\"step\"].(string)\n\t\t\
                 if step == \"start\" {{\n\t\t\tstate, _ = r.Parse{cls}(top[\"state\"].(string))\n{more_start}\t\t}}\n",
                all.join(", ")
            );
            tail = format!("\t\t\tif stepping {{\n\t\t\t\tstate = {}\n{more_tail}\t\t\t}}\n", self.next_state_of("got", &format!(".{}", pascal(&out))));
        }
        // The packages of `enums` files whose types the runner names by their package, where the
        // rule's package names them so (§15.239): a machine's state, an enum a block of
        // prerequisites is over.
        // gofmt keeps the specs of one group sorted by path, so the rule's package goes among them.
        let mut ours: Vec<(String, String)> = vec![(pkg.clone(), format!("\tr {pkg:?}\n"))];
        let named = self.machine_consts().map(|(en, _, _)| en).into_iter().chain(self.prereq_graphs().into_iter().map(|g| g.over));
        for en in named {
            if let (Some(owner), Some(o)) = (self.go_owner_of(&en), self.f.enums.iter().find(|e| e.name.text == en).and_then(|e| e.from.as_ref())) {
                let path = self.go_owner_path(&pub_name(&o.module), &self.owner_dir(o));
                let line = if path == owner { format!("\t{path:?}\n") } else { format!("\t{owner} {path:?}\n") };
                if !ours.iter().any(|(_, l)| *l == line) {
                    ours.push((path, line));
                }
            }
        }
        ours.sort();
        let ours: String = ours.into_iter().map(|(_, l)| l).collect();
        format!(
            "// Code generated by rulec {}. DO NOT EDIT.\n\
             package main\n\n\
             import (\n\t\"bufio\"\n\t\"bytes\"\n\t\"encoding/json\"\n\t\"fmt\"\n\t\"os\"\n\t\"strconv\"\n\t\"strings\"\n\t\"time\"\n\n{ours})\n\n\
             {GO_WIRE}\
             func main() {{\n\t\
                 sc := bufio.NewScanner(os.Stdin)\n\t\
                 sc.Buffer(make([]byte, 1<<20), 1<<20)\n\
                 {}\
                 {head}\t\
                 for sc.Scan() {{\n\t\t\
                     if len(sc.Bytes()) == 0 {{\n\t\t\tcontinue\n\t\t}}\n\
                 {read}\t\t\
                     func() {{\n\t\t\t\
                         defer refuse()\n\t\t\t\
                         var in r.Input\n{}\t\t\t\
                         got, trace, err := r.{fname}Traced(in)\n\t\t\t\
                         if err != nil {{\n\t\t\t\tpanic(err)\n\t\t\t}}\n\t\t\t\
                         fmt.Println(r.{fname}Record(in, got, trace, \"\"))\n\
                 {tail}\t\t\
                     }}()\n\t\
                 }}\n}}\n",
            ritsu_emit::header::VERSION,
            self.go_labels_mode() + &self.go_prereq_mode() + &self.go_chances_mode(&chances_fields),
            fields.join("")
        )
    }

    /// What the Go runner answers with `--labels` (§15.211): for each line, an enum's name and a
    /// value's, the display name the package gives.
    fn go_labels_mode(&self) -> String {
        let enums = self.labeled_enums();
        if enums.is_empty() {
            return String::new();
        }
        let mut arms = String::new();
        for (en, ty, _) in &enums {
            arms.push_str(&format!("\t\t\tcase {}:\n\t\t\t\tx, _ := r.Parse{ty}(v)\n\t\t\t\tfmt.Println(x.Label())\n", str_lit(en)));
        }
        format!(
            "\tif len(os.Args) > 1 && os.Args[1] == \"--labels\" {{\n\t\tfor sc.Scan() {{\n\t\t\ten, v, _ := strings.Cut(sc.Text(), \"\\t\")\n\t\t\tswitch en {{\n{arms}\t\t\t}}\n\t\t}}\n\t\treturn\n\t}}\n"
        )
    }
}


// ---------------------------------------------------------------------------
// TypeScript (§8.3)
// ---------------------------------------------------------------------------

/// Turn the shared expression text into TypeScript. The same post-processing shape as
/// `go_expr`: one grammar is emitted once and each language adjusts the spellings it does
/// not share.
///
/// Every integer becomes a `bigint` literal. The overflow proof (E108) is against int64, and
/// a JavaScript `number` is exact only to 2^53, so using one would put a silent wrong answer
/// above nine quadrillion into a tool whose whole claim is that it does not do that.
fn ts_expr(s: &str) -> String {
    let t = arith::ts_expr(&truths(s, "true", "false"))
        .replace(" // ", " / ")
        .replace("_round_down(", "_roundDown(")
        .replace("_round_up(", "_roundUp(")
        .replace("_round_half_down(", "_roundHalfDown(")
        .replace("_round_half(", "_roundHalf(")
        .replace("_round_bankers(", "_roundBankers(")
        .replace("_min(", "_min(")
        .replace("_max(", "_max(");
    bigint_literals(&t)
}

/// Append `n` to every integer literal, and only to those. A run of digits that touches a
/// letter, `_` or `.` on either side belongs to a name (`SizeClass.S60`, `項目1`) and is left
/// alone.
fn bigint_literals(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let part = |c: char| c.is_alphanumeric() || c == '_' || c == '.';
    let mut out = String::with_capacity(s.len() + 8);
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() && (i == 0 || !part(b[i - 1])) {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let touches_name = i < b.len() && part(b[i]);
            out.extend(&b[start..i]);
            if !touches_name {
                out.push('n');
            }
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// The TypeScript parser of the enum `ascii`: a string that is one of its values is that value, and
/// any other is refused with `error`, `RuleInputError` in a rule's module (§15.212, §15.319).
fn ts_parser(ascii: &str, error: &str) -> String {
    format!(
        "export function parse{ascii}(s: string): {ascii} {{\n  \
         const v = Object.values({ascii}).find((x) => x === s);\n  \
         if (v === undefined) {{\n    \
         throw new {error}(`{}`);\n  }}\n  \
         return v;\n}}\n\n",
        tr!("${{s}} は列挙 {ascii} の値ではありません", "${{s}} is not a value of enum {ascii}")
    )
}

/// The helpers of the TypeScript module that are written only where the module calls them
/// (§15.342): the two comparisons and the five modes of [`round_ts`], and the date of a record.
/// `tsc --noUnusedLocals` refuses a function nothing reads.
const TS_CALLED: &[&str] = &["_min", "_max", "_roundDown", "_roundUp", "_roundHalf", "_roundHalfDown", "_roundBankers", "_civil"];

/// The five modes of §7.3 on `bigint`. `/` on a bigint truncates toward zero, the same as
/// Go's integer division, so these mirror the Go helpers rather than the Python ones.
fn round_ts() -> String {
    format!(
        r#"
export const NO_VALUE = Symbol("no value");

export class RuleInputError extends Error {{
  readonly what: string;
  readonly value: unknown;
  constructor(what: string, value: unknown = NO_VALUE) {{
    super(value === NO_VALUE ? what : `${{what}}: ${{String(value)}}`);
    this.name = "RuleInputError";
    this.what = what;
    this.value = value;
  }}
}}

export class RuleContradictionError extends Error {{
  readonly what: string;
  constructor(what: string) {{
    super(what);
    this.name = "RuleContradictionError";
    this.what = what;
  }}
}}

function _min(a: bigint, b: bigint): bigint {{
  return a < b ? a : b;
}}

function _max(a: bigint, b: bigint): bigint {{
  return a > b ? a : b;
}}

/** {down} */
function _roundDown(x: bigint, g: bigint): bigint {{
  const a = x < 0n ? -x : x;
  const v = (a / g) * g;
  return x < 0n ? -v : v;
}}

/** {up} */
function _roundUp(x: bigint, g: bigint): bigint {{
  const a = x < 0n ? -x : x;
  const v = a % g === 0n ? (a / g) * g : (a / g + 1n) * g;
  return x < 0n ? -v : v;
}}

/** {half} */
function _roundHalf(x: bigint, g: bigint): bigint {{
  const a = x < 0n ? -x : x;
  const v = 2n * (a % g) >= g ? (a / g + 1n) * g : (a / g) * g;
  return x < 0n ? -v : v;
}}

/** {half_down} */
function _roundHalfDown(x: bigint, g: bigint): bigint {{
  const a = x < 0n ? -x : x;
  const v = 2n * (a % g) > g ? (a / g + 1n) * g : (a / g) * g;
  return x < 0n ? -v : v;
}}

/** {bankers} */
function _roundBankers(x: bigint, g: bigint): bigint {{
  const a = x < 0n ? -x : x;
  let q = a / g;
  const r = a % g;
  if (2n * r > g || (2n * r === g && q % 2n === 1n)) {{
    q += 1n;
  }}
  const v = q * g;
  return x < 0n ? -v : v;
}}
"#,
        down = tr!("0 へ寄せる。-4.8円 → -4円。", "Toward zero: -4.8 yen → -4 yen."),
        up = tr!("0 から遠ざける。-4.2円 → -5円。", "Away from zero: -4.2 yen → -5 yen."),
        half = tr!("半分ちょうどは 0 から遠ざける。", "An exact half goes away from zero."),
        half_down = tr!("半分ちょうどは 0 へ寄せる。", "An exact half goes toward zero."),
        bankers = tr!("半分ちょうどは偶数へ。", "An exact half goes to the even neighbor."),
    )
}

/// A sorted set of days as runs of consecutive ones, each its first and last.
/// The entry guards of one optional input, held back until they are written and then put under
/// the test that the input has a value (§15.201). The guard of the type it wraps is the same text
/// either way, so each language writes it once: `o.push_str` reads as it does on a `String`.
pub(crate) struct Guarded<'a> {
    out: &'a mut String,
    buf: String,
    /// The line that opens the test and the one that closes it; `None` for an input that is not
    /// optional, whose guards go out as they are.
    wrap: Option<(String, String)>,
    indent: &'static str,
}

impl<'a> Guarded<'a> {
    pub(crate) fn new(out: &'a mut String, wrap: Option<(String, String)>, indent: &'static str) -> Guarded<'a> {
        Guarded { out, buf: String::new(), wrap, indent }
    }

    pub(crate) fn push_str(&mut self, s: &str) {
        self.buf.push_str(s);
    }
}

impl Drop for Guarded<'_> {
    fn drop(&mut self) {
        match &self.wrap {
            Some((open, close)) if !self.buf.trim().is_empty() => {
                self.out.push_str(open);
                for l in self.buf.lines() {
                    if l.trim().is_empty() {
                        self.out.push_str(l);
                    } else {
                        self.out.push_str(self.indent);
                        self.out.push_str(l);
                    }
                    self.out.push('\n');
                }
                self.out.push_str(close);
            }
            _ => self.out.push_str(&self.buf),
        }
    }
}

pub(crate) fn day_runs(days: &[i64]) -> Vec<(i64, i64)> {
    let mut runs: Vec<(i64, i64)> = Vec::new();
    for &d in days {
        match runs.last_mut() {
            Some((_, b)) if *b + 1 == d => *b = d,
            _ => runs.push((d, d)),
        }
    }
    runs
}

/// Which language a shared emitter is writing for. `guards` is the one body three languages
/// share, and it used to tell Python from Go by whether the indent was a tab — which quietly
/// handed TypeScript the Python spelling.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Py,
    Go,
    Ts,
    Rs,
    Rb,
    Sw,
    Sql,
    Php,
    Java,
}

/// The few spellings shared code needs to know per language. Keeping them in one row each
/// is what lets `guards()` be written once: it used to carry three separate `match`es, and
/// a sixth language meant finding all three (§15.20).
struct Spelling {
    /// How a line comment starts.
    comment: &'static str,
    /// How two conditions are joined.
    and: &'static str,
    /// The condition as an `if` head, without the indent.
    if_head: fn(&str) -> String,
    /// What closes the block, or "" for a language that closes by indentation.
    close: &'static str,
}

impl Lang {
    /// How the language spells "not" in front of a parenthesised condition.
    fn not(self) -> &'static str {
        match self {
            Lang::Py => "not ",
            Lang::Sql => "NOT ",
            _ => "!",
        }
    }

    fn spelling(self) -> Spelling {
        match self {
            Lang::Py => Spelling {
                comment: "#",
                and: " and ",
                if_head: |c| format!("if {c}:"),
                close: "",
            },
            Lang::Rb => Spelling {
                comment: "#",
                and: " && ",
                if_head: |c| format!("if {c}"),
                close: "end",
            },
            // PHP is spelled like TypeScript here — braces, `&&`, `//` — and differs only
            // where §15.77 says it does: `intdiv` for division, and typed parameters.
            Lang::Ts | Lang::Php | Lang::Java => Spelling {
                comment: "//",
                and: " && ",
                if_head: |c| format!("if ({c}) {{"),
                close: "}",
            },
            Lang::Sql => Spelling {
                comment: "--",
                and: " AND ",
                if_head: |c| format!("WHEN {c} THEN"),
                close: "",
            },
            Lang::Go | Lang::Rs | Lang::Sw => Spelling {
                comment: "//",
                and: " && ",
                if_head: |c| format!("if {c} {{"),
                close: "}",
            },
        }
    }
}

impl<'a> Gen<'a> {
    /// The TypeScript type for a value. Numbers are branded `bigint`s: a brand costs nothing
    /// at runtime and still refuses `YenInclTax` where `YenExclTax` was meant.
    fn ts_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => self.enum_names.get(n).cloned().unwrap_or_else(|| "string".into()),
            Ty::Bool => "boolean".into(),
            Ty::Str => "string".into(),
            Ty::Number | Ty::Date => "bigint".into(),
            Ty::Opt(t) => format!("{} | null", self.ts_ty(t)),
            _ => brand_of(ty),
        }
    }

    fn ts_value(&self, v: &str, ty: &Ty) -> String {
        match self.value_name(v, ty) {
            Some((ty, alias)) => format!("{ty}.{}", alias.to_uppercase()),
            None => format!("{v:?}"),
        }
    }

    /// A value of an enum or a truth value as the code writes it, which is how the values a local
    /// holds are told apart below.
    fn ts_key(&self, w: &str, ty: &Ty) -> String {
        if w == crate::kw::TRUE {
            "true".into()
        } else if w == crate::kw::FALSE {
            "false".into()
        } else {
            self.ts_value(w, ty)
        }
    }

    /// The values a local of an enum or a truth value holds as `tsc` reads the code (§15.342).
    /// `tsc` narrows a local of a union type to the values it was given, and refuses a `===` with
    /// a value outside them (TS2367). A table's column holds what its rows write (`null` for
    /// `none`, and what a local a row hands on holds); a value worked out as another local, what
    /// that one holds; a fold's answer, what its arms, its `empty` and its `exhausted` give, where
    /// the verdict an arm hands on holds that arm's value; a `max` or a `min`, its `empty` and
    /// what the column it reads holds, which a `where` that tests that column for a value narrows
    /// to that value. None where one of them is a value read as its whole type (an input, or what
    /// an expression works out), and for every other local, which holds its whole type.
    fn ts_narrowed(&self, name: &str) -> Option<std::collections::BTreeSet<String>> {
        self.ts_narrowed_in(name, 0)
    }

    /// `ts_narrowed`, `depth` locals into what hands a value on to another.
    fn ts_narrowed_in(&self, name: &str, depth: usize) -> Option<std::collections::BTreeSet<String>> {
        use std::collections::BTreeSet;
        let ty = self.ty_of(name);
        if !matches!(ty.present(), Ty::Enum(_) | Ty::Bool) || depth > 64 {
            return None;
        }
        let value = |w: &str| (w == crate::kw::TRUE || w == crate::kw::FALSE || self.is_value(w)).then(|| self.ts_key(w, &ty));
        // What a value written, or a local read, holds.
        let holds = |w: &str| value(w).map(|k| BTreeSet::from([k])).or_else(|| self.ts_narrowed_in(w, depth + 1));
        for (i, it) in self.f.items.iter().enumerate() {
            match it {
                Item::Table(t) => {
                    let Some(t) = self.c.table_at(t) else { continue };
                    let Some(oi) = t.outputs.iter().position(|oc| oc.name.text == name) else { continue };
                    let mut s = BTreeSet::new();
                    for row in &t.rows {
                        match row.outs.get(oi)? {
                            OutCell::Nothing => {
                                s.insert("null".to_string());
                            }
                            OutCell::Lit(Lit::Word(w)) | OutCell::Name(w) => s.extend(holds(w)?),
                            _ => return None,
                        }
                    }
                    return Some(s);
                }
                Item::Derived(crate::ast::DerivedDecl { name: n, expr, .. }) | Item::Define(crate::ast::DefineDecl { name: n, expr, .. })
                    if n.text == name =>
                {
                    return match expr {
                        Expr::Name(other, _) => self.ts_narrowed_in(other, depth + 1),
                        _ => None,
                    };
                }
                Item::Agg(d) if d.name.text == name && d.kind.extreme() => {
                    let column = self.ts_narrowed_in(&d.column.text, depth + 1);
                    // A `where` that tests the column it reads for a value narrows the column to that
                    // value where it is read, unless the test reads it as its whole type.
                    let read = match &d.filter {
                        Some((w, Some(want))) if w.text == d.column.text => {
                            let k = self.ts_key(&want.text, &self.ty_of(&w.text));
                            if Self::ts_refuses(column.as_ref(), &k) {
                                column
                            } else {
                                Some(match column {
                                    Some(c) => c.into_iter().filter(|v| *v == k).collect(),
                                    None => BTreeSet::from([k]),
                                })
                            }
                        }
                        _ => column,
                    };
                    let mut s = read?;
                    s.insert(value(&self.extreme_empty_word(d)?)?);
                    return Some(s);
                }
                Item::Fold(fold) if self.fold_target(fold).0 == name => {
                    let ends = self.fold_alone(i) && !self.stop_keeps_door(i);
                    let vty = self.ty_of(&fold.verdict);
                    // What an answer holds. Inside the arm for a value (`arm`: that value, and what
                    // the verdict holds at that arm), the verdict holds that value, unless the arm
                    // reads it as its whole type.
                    let answers = |x: &Expr, arm: Option<(&String, &Option<BTreeSet<String>>)>| -> Option<BTreeSet<String>> {
                        match (x, arm) {
                            (Expr::Name(n, _), Some((k, at))) if *n == fold.verdict => {
                                if Self::ts_refuses(at.as_ref(), k) {
                                    at.clone()
                                } else {
                                    Some(match at {
                                        Some(s) => s.iter().filter(|v| *v == k).cloned().collect(),
                                        None => BTreeSet::from([k.clone()]),
                                    })
                                }
                            }
                            (Expr::Lit(Lit::Word(w), _) | Expr::Name(w, _), _) => holds(w),
                            _ => None,
                        }
                    };
                    let mut s = answers(fold.empty.as_ref()?, None)?;
                    // What the arms take or keep, which only `held` hands on.
                    let mut taken = Some(BTreeSet::new());
                    // What the verdict holds at each arm, as the arms test it (`ts_fold`).
                    let mut at = self.ts_narrowed_in(&fold.verdict, depth + 1);
                    for (arm_name, arm, _) in &fold.arms {
                        let k = self.ts_key(&arm_name.text, &vty);
                        match arm {
                            Arm::Stop(Some(x)) => s.extend(answers(x, Some((&k, &at)))?),
                            Arm::Take { expr, .. } | Arm::KeepMax { expr, .. } | Arm::TakeWeighted { expr, .. } => {
                                let got = answers(expr, Some((&k, &at)));
                                taken = taken.zip(got).map(|(mut t, g)| {
                                    t.extend(g);
                                    t
                                });
                            }
                            _ => {}
                        }
                        if ends && matches!(arm, Arm::Stop(_)) {
                            if let Some(a) = at.as_mut() {
                                a.remove(&k);
                            }
                        }
                    }
                    match fold.exhausted.as_ref()? {
                        // What is held: what the arms took or kept, or else the `empty` answer.
                        Expr::Name(n, _) if n == crate::kw::HELD => s.extend(taken?),
                        x => s.extend(answers(x, None)?),
                    }
                    return Some(s);
                }
                _ => {}
            }
        }
        None
    }

    /// Whether `tsc` refuses a `===` of a local that holds `narrowed` with `key` (TS2367): it
    /// holds a value, and not that one. A local that holds nothing, or only `null`, compares with
    /// any value.
    fn ts_refuses(narrowed: Option<&std::collections::BTreeSet<String>>, key: &str) -> bool {
        narrowed.is_some_and(|s| s.iter().any(|v| v != "null") && !s.contains(key))
    }

    /// The value a cell of a table tests its column for with `===`, if it tests one so: a value of
    /// an enum, `true` or `false` of a truth value that may be absent, and `null` for `none`.
    fn ts_took(&self, cell: &Cell, col: &str) -> Option<String> {
        let ty = self.ty_of(col);
        match cell {
            Cell::Nothing => Some("null".into()),
            Cell::Lit(Lit::Word(w)) if !self.c.groups.contains_key(w) => match ty.present() {
                Ty::Enum(_) => Some(self.ts_key(w, &ty)),
                Ty::Bool if matches!(ty, Ty::Opt(_)) => Some(self.ts_key(w, &ty)),
                _ => None,
            },
            _ => None,
        }
    }

    /// The local a cell tests, as the code names it there (§15.342). Where the local never holds
    /// the value the cell tests it for, which a table that another rule's apply brings in can ask
    /// (its rows are the other rule's), `tsc --strict` refuses the comparison (TS2367); the local
    /// is read there as its whole type, which the JavaScript leaves out with the other types.
    fn ts_compared(&self, cell: &Cell, col: &str, var: &str, narrowed: Option<&std::collections::BTreeSet<String>>) -> String {
        match self.ts_took(cell, col) {
            Some(key) if Self::ts_refuses(narrowed, &key) => format!("({var} as {})", self.ts_ty(&self.ty_of(col))),
            _ => var.to_string(),
        }
    }

    /// The text of an expression with each `=` of two locals that never hold the same value read
    /// as the type of the first (§15.342): `tsc --strict` refuses `lift == floor` where `lift` was
    /// only ever given silver or gold and `floor` only basic (TS2367).
    fn ts_meeting(&self, x: &Expr, mut text: String, local: &dyn Fn(&str) -> String) -> String {
        fn pairs<'e>(x: &'e Expr, out: &mut Vec<(&'e str, &'e str)>) {
            match x {
                Expr::Bin(a, op, b, _) => {
                    if let (BinOp::Eq, Expr::Name(a, _), Expr::Name(b, _)) = (op, a.as_ref(), b.as_ref()) {
                        out.push((a, b));
                    }
                    pairs(a, out);
                    pairs(b, out);
                }
                Expr::Call(_, args, _) => args.iter().for_each(|a| pairs(a, out)),
                _ => {}
            }
        }
        let mut found = Vec::new();
        pairs(x, &mut found);
        for (a, b) in found {
            let (Some(sa), Some(sb)) = (self.ts_narrowed(a), self.ts_narrowed(b)) else { continue };
            if sa.iter().any(|v| v != "null") && sb.iter().any(|v| v != "null") && sa.is_disjoint(&sb) {
                let (la, lb) = (local(a), local(b));
                text = text.replace(&format!("({la} == {lb})"), &format!("(({la} as {}) == {lb})", self.ts_ty(&self.ty_of(a))));
            }
        }
        text
    }

    /// Render a cell as a TypeScript condition. A don't-care yields None (no condition).
    fn ts_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        let numeric = inner.is_numeric() || matches!(inner, Ty::Date);
        // An optional number, date or truth value is `null` when it is absent: a comparison and
        // a set are about the value that is there, and `!null` is true (§15.201).
        let present = matches!(ty, Ty::Opt(_)) && !matches!(inner, Ty::Enum(_));
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                Lit::Word(w) => self.ts_value(w, ty),
                Lit::Num(n) => format!("{}n", self.int_lit(n, inner, col_scale)),
                Lit::Date(y, m, d) => format!("{}n", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => format!("{s:?}"),
            }
        };
        let members = |ls: &Vec<Lit>| -> String {
            let mut out: Vec<String> = Vec::new();
            for l in ls {
                if let Lit::Word(w) = l {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| self.ts_value(m, ty)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            format!("[{}]", out.join(", "))
        };
        // An array of an enum's members has the type of those members only (`Event.SHIP` is
        // the literal `"出荷"`, since the enum's object is `as const`), and `includes` takes an
        // element of the array's type: `[Event.SHIP].includes(event)` does not pass
        // `tsc --strict` for an `event` of the whole enum. The array is widened to the
        // column's type; the cast goes with the other types, so the JavaScript only gains the
        // parentheses (§15.171).
        let widened = |arr: String| -> String {
            if matches!(inner, Ty::Enum(_)) { format!("({arr} as Array<{}>)", self.ts_ty(ty)) } else { arr }
        };
        // `===` on a bigint and on a string are both value comparisons, so one spelling does
        // for every type here.
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => format!("{var} === null"),
            Cell::Prefix(ps) => ps
                .iter()
                .map(|p| format!("{var}.startsWith({})", format!("{p:?}")))
                .collect::<Vec<_>>()
                .join(" || "),
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                format!("{}.has({var})", self.ts_group(w))
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE && present => format!("{var} === true"),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE && present => format!("{var} === false"),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("!{var}"),
            Cell::Lit(l) => format!("{var} === {}", lit(l)),
            Cell::Set(ls) if present => format!("{var} !== null && {}.includes({var})", members(ls)),
            Cell::Not(ls) if present => format!("({var} === null || !{}.includes({var}))", members(ls)),
            Cell::Set(ls) => format!("{}.includes({var})", widened(members(ls))),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                format!("!{}.has({var})", self.ts_group(w))
            }
            Cell::Not(ls) => format!("!{}.includes({var})", widened(members(ls))),
            Cell::Cmp(cs) => {
                let c = cs
                    .iter()
                    .map(|(o, l)| {
                        let op = match o {
                            CmpOp::Le => "<=",
                            CmpOp::Ge => ">=",
                            CmpOp::Lt => "<",
                            CmpOp::Gt => ">",
                        };
                        let _ = numeric;
                        format!("{var} {op} {}", lit(l))
                    })
                    .collect::<Vec<_>>()
                    .join(" && ");
                // `null <= 5n` is true in JavaScript, so the absent value took the row (§15.201).
                if present { format!("{var} !== null && {c}") } else { c }
            }
            // A comparison on an ordered enum, by the places in its order (§15.210).
            Cell::Ranked { ops, .. } => {
                let en = self.c.ordered_enum(ty)?;
                let r = self.ts_rank_fn(en);
                let c = ops
                    .iter()
                    .map(|(o, v)| format!("{r}({var}) {} {r}({})", o.word(), self.ts_value(v, ty)))
                    .collect::<Vec<_>>()
                    .join(" && ");
                if matches!(ty, Ty::Opt(_)) { format!("{var} !== null && {c}") } else { c }
            }
        })
    }

    pub fn typescript(&self) -> String {
        self.apart_text(clash::Syntax::Script, |g| g.typescript_text())
    }

    fn typescript_text(&self) -> String {
        let mut o = self.header("//");
        o.push('\n');

        // Brands. A branded bigint is still a bigint at runtime; the brand exists only for
        // the type checker, exactly as `NewType` does on the Python side. In a package of
        // ritsu's, those of money and of the quantities are the package's, imported and handed on
        // under the same names (§15.248).
        let (own, units) = self.brands();
        for (b, ty) in &own {
            o.push_str(&format!("export type {b} = bigint & {{ readonly __rulec: \"{b}\" }}; // {ty}\n"));
        }
        if !units.is_empty() {
            let names: Vec<&str> = units.keys().map(String::as_str).collect();
            o.push_str(&format!("import type {{ {0} }} from \"{1}\";\nexport type {{ {0} }};\n", names.join(", "), ritsu_emit::package::ts_path(&self.rules_dir(), "units")));
        }
        if !own.is_empty() || !units.is_empty() {
            o.push('\n');
        }

        o.push_str(&format!("/** {} */\nexport type Fired = {{ readonly table: string; readonly row: number; readonly label?: string }};\n\n", fired_doc()));

        // The error classes come first: the enum parsers below throw them.
        o.push_str(&round_ts());
        o.push_str(&arith::ts_helpers(&arith::Uses::of(self.f)));
        o.push('\n');

        // Enums. A frozen object plus a union type, not `enum`: that keeps the file to
        // erasable syntax, so `node file.ts` runs it with no build step at all. An enum read
        // from an `enums` file is that file's module's, imported above (§15.212).
        o.push_str(&self.ts_enums());
        o.push_str(&self.ts_enum_helpers());
        // Which values need which before them (§15.222).
        o.push_str(&self.ts_prereqs());

        // The machine this function is one step of (§15.148).
        if let Some((en, init, fins)) = self.machine_consts() {
            let cls = self.enum_names.get(&en).cloned().unwrap_or_default();
            // The values carried along with the state, and where a case starts (§15.217).
            let mut more = String::new();
            for mc in self.more_carried() {
                let v = match mc.kind() {
                    MoreKind::Enum => self.ts_value(&mc.word(), &mc.ty),
                    MoreKind::Bool => mc.bool_word().to_string(),
                    MoreKind::Number => format!("{}n", mc.int),
                    MoreKind::Branded => format!("{}n as {}", mc.int, self.ts_ty(&mc.ty)),
                };
                more.push_str(&format!("export const {}: {} = {v};\n", mc.upper(), self.ts_ty(&mc.ty)));
            }
            o.push_str(&format!(
                "export const INITIAL: {cls} = {};\n\
                 export const FINAL: ReadonlySet<{cls}> = new Set([{}]);\n{more}\n\
                 /** {} */\n\
                 export function isFinal(state: {cls}): boolean {{\n  return FINAL.has(state);\n}}\n\n",
                self.ts_value(&init, &Ty::Enum(en.clone())),
                fins.iter().map(|v| self.ts_value(v, &Ty::Enum(en.clone()))).collect::<Vec<_>>().join(", "),
                tr!("この状態で案件が終わっているか。", "Whether a case in this state has ended.")
            ));
        }

        for g in &self.f.groups {
            let ms: Vec<String> = g.members.iter().map(|m| self.ts_value(&m.text, &self.group_ty(&g.name.text))).collect();
            o.push_str(&format!("const {}: ReadonlySet<string> = new Set([{}]);\n", self.ts_group(&g.name.text), ms.join(", ")));
        }
        if !self.f.groups.is_empty() {
            o.push('\n');
        }

        o.push_str(&self.ts_fn());
        o.push('\n');
        o.push_str(&self.ts_record());
        if self.projects() {
            o.push('\n');
            o.push_str(&self.ts_from());
        }
        // The function that counts the answers (§15.225).
        if self.f.has_random() {
            o.push('\n');
            o.push_str(&self.ts_chances());
        }
        let o = self.ts_called(o);
        self.ts_imports(o)
    }

    /// The module without the helpers of [`TS_CALLED`] it does not call (§15.342). Where the
    /// rule's function has the name of one, only the module written with the marks tells a call of
    /// the one from a call of the other (§15.336), and the module written without them leaves out
    /// what that one leaves out: the second writing is the first with the marks in it.
    fn ts_called(&self, text: String) -> String {
        if !self.marks && TS_CALLED.contains(&pub_name(&self.f.name).as_str()) {
            let marked = self.marked().typescript_text();
            let gone: Vec<&str> = TS_CALLED.iter().copied().filter(|h| !from::ts_declares(&marked, h)).collect();
            return from::ts_without(&text, &gone);
        }
        from::ts_without_unread(&text, TS_CALLED)
    }

    /// The frozen objects and union types of the enums this module declares, each with its
    /// parser, its order and its display names when it has them (§15.210, §15.211).
    fn ts_enums(&self) -> String {
        let mut o = String::new();
        for (jp, ascii) in self.own_enums() {
            let Some(vals) = self.c.enums.get(&jp) else { continue };
            o.push_str(&format!("export const {ascii} = {{\n"));
            for v in vals {
                o.push_str(&format!("  {}: {v:?},\n", self.py_member(&jp, v)));
            }
            o.push_str("} as const;\n");
            o.push_str(&format!(
                "export type {ascii} = (typeof {ascii})[keyof typeof {ascii}];\n\n"
            ));
            // A file of enums has no door of its own, and so no `RuleInputError`: what it refuses
            // is the language's own error for a value outside a range (§15.212).
            let error = if self.f.kind == FileKind::Enums { "RangeError" } else { "RuleInputError" };
            o.push_str(&ts_parser(&ascii, error));
            o.push_str(&self.ts_enum_fns(&jp, &ascii));
        }
        // An enum of an `enums` file is that file's module's, imported at the top (§15.212). Its
        // parser is the rule's as well, as one of the rule's own enums has (§15.319): what imports
        // the rule's module finds the same names for both, and a value outside the enum is the
        // rule's `RuleInputError` there.
        for (_, _, types) in self.foreign_owners() {
            for (_, ascii) in types {
                o.push_str(&ts_parser(&ascii, "RuleInputError"));
            }
        }
        o
    }

    /// `eraRank`: the place of a value of an ordered enum in its order.
    fn ts_rank_fn(&self, en: &str) -> String {
        format!("{}Rank", lower_lead(&self.enum_type(en)))
    }

    /// `unitNameLabel`: a value's display name.
    fn ts_label_fn(&self, en: &str) -> String {
        format!("{}Label", lower_lead(&self.enum_type(en)))
    }

    /// An enum's order and display names, one function each over its members (§15.210,
    /// §15.211).
    fn ts_enum_fns(&self, jp: &str, ascii: &str) -> String {
        let Some(vals) = self.c.enums.get(jp) else { return String::new() };
        let mut o = String::new();
        let chain = |o: &mut String, ret: &dyn Fn(usize, &String) -> String| {
            // Over an enum of one value, the answer does not look at `x` (§15.342).
            if vals.len() == 1 {
                o.push_str("  void x;\n");
            }
            for (i, v) in vals.iter().enumerate() {
                if i + 1 == vals.len() {
                    o.push_str(&format!("  return {};\n}}\n\n", ret(i, v)));
                } else {
                    o.push_str(&format!("  if (x === {ascii}.{}) {{\n    return {};\n  }}\n", self.py_member(jp, v), ret(i, v)));
                }
            }
        };
        if self.c.ordered.contains(jp) {
            o.push_str(&format!(
                "/** {} */\nexport function {}(x: {ascii}): number {{\n",
                tr!("x が {ascii} の順序の何番目か（0 から）。", "The place of x in the order of {ascii}, counting from 0."),
                self.ts_rank_fn(jp)
            ));
            chain(&mut o, &|i, _| i.to_string());
        }
        if let Some(labels) = self.c.labels.get(jp) {
            o.push_str(&format!(
                "/** {} */\nexport function {}(x: {ascii}): string {{\n",
                tr!("x の表示名。表示名の無い値は値の名前。", "The display name of x: a value without one is shown by its name."),
                self.ts_label_fn(jp)
            ));
            chain(&mut o, &|i, v| str_lit(&labels.get(i).cloned().flatten().unwrap_or_else(|| v.clone())));
        }
        o
    }

    /// The helpers the functions call for the expressions over an ordered enum (§15.210).
    fn ts_enum_helpers(&self) -> String {
        let mut o = String::new();
        for en in self.c.enum_order.iter().filter(|e| self.c.ordered.contains(*e)) {
            let (rank, max, min) = self.enum_helpers(en);
            let (t, r) = (self.enum_type(en), self.ts_rank_fn(en));
            if rank {
                o.push_str(&format!("function _rank_{t}(x: {t}): number {{\n  return {r}(x);\n}}\n\n"));
            }
            if max {
                o.push_str(&format!("function _max_{t}(a: {t}, b: {t}): {t} {{\n  return {r}(a) >= {r}(b) ? a : b;\n}}\n\n"));
            }
            if min {
                o.push_str(&format!("function _min_{t}(a: {t}, b: {t}): {t} {{\n  return {r}(a) <= {r}(b) ? a : b;\n}}\n\n"));
            }
        }
        o
    }

    /// The module with the import of what it uses of each `enums` file it reads from, after its
    /// head (§15.212). What the module has of each such enum is exported again, so that what
    /// imports this rule's module finds it where it finds what the rule's own enums have
    /// (§15.319): the type, and the functions of its order and its display names. The parser is
    /// the module's own (`ts_enums`).
    fn ts_imports(&self, o: String) -> String {
        let foreign = self.foreign_owners();
        if foreign.is_empty() {
            return o;
        }
        let mut lines = String::new();
        for (module, dir, types) in foreign {
            let mut names: Vec<String> = Vec::new();
            let mut again: Vec<String> = Vec::new();
            // the functions of the enums, as a module that declares them has them
            let mut handed: Vec<String> = Vec::new();
            for (jp, t) in &types {
                if has_word(&o, t) {
                    names.push(t.clone());
                    again.push(t.clone());
                }
                for n in [self.ts_rank_fn(jp), self.ts_label_fn(jp)] {
                    if has_word(&o, &n) && !names.contains(&n) {
                        names.push(n);
                    }
                }
                if self.c.ordered.contains(jp) {
                    handed.push(self.ts_rank_fn(jp));
                }
                if self.c.labels.contains_key(jp) {
                    handed.push(self.ts_label_fn(jp));
                }
            }
            if names.is_empty() {
                continue;
            }
            // An enum whose names meet the rule's own is taken under the type it has here
            // (`Size as Size2`, §15.326).
            let named = |ns: &[String]| -> String {
                ns.iter()
                    .map(|n| match types.iter().map(|(jp, _)| self.theirs("typescript", jp, n)).find(|o| o != n) {
                        Some(o) => format!("{o} as {n}"),
                        None => n.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let from = if self.package.is_some() { self.place.ts_path(&self.rules_dir(), &format!("{dir}rules/{module}")) } else { format!("./{module}.ts") };
            lines.push_str(&format!("import {{ {} }} from \"{from}\";\n", named(&names)));
            if !again.is_empty() {
                lines.push_str(&format!("export {{ {} }};\n", again.join(", ")));
            }
            if !handed.is_empty() {
                lines.push_str(&format!("export {{ {} }} from \"{from}\";\n", named(&handed)));
            }
        }
        // after the head: its comment lines and the blank line under them
        let head = o.lines().take_while(|l| l.starts_with("//")).map(|l| l.len() + 1).sum::<usize>();
        let head = (head + 1).min(o.len());
        format!("{}{lines}\n{}", &o[..head], &o[head..])
    }


    /// Every walk of a rule that walks sequences, in TypeScript (§15.56, §15.58, §15.219). The
    /// shape is the one `py_walks` writes.
    fn ts_walks(&self, outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = String::new();
        for (s, el) in self.f.elements.iter().enumerate() {
            if let Some(cap) = self.count_cap_of(s) {
                let seq = self.held(&el.name);
                o.push_str(&format!(
                    "  if ({seq}.length > {cap}) {{\n    throw new RuleInputError(`{}`, {seq}.length);\n  }}\n",
                    crate::door::too_many(&el.name.text, cap)
                ));
            }
        }
        for step in self.walk_steps() {
            match step {
                // What a walk does not depend on is worked out once, before it (§15.208).
                WalkStep::Once(ix) => o.push_str(&self.ts_items(outer, trace, Phase::Only(&ix))),
                WalkStep::Walk { seq, body, sums, .. } => o.push_str(&self.ts_walk(seq, &body, &sums, outer, trace)),
            }
        }
        o
    }

    /// One walk over sequence `s`, in TypeScript (§15.219).
    fn ts_walk(&self, s: usize, body_items: &[usize], sums: &[usize], outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let el = &self.f.elements[s];
        let seq = self.held(&el.name);
        let fields: BTreeMap<String, String> = el.fields.iter().map(|fd| (fd.name.text.clone(), pub_name(&fd.name))).collect();
        let e = self.elem_var(s);
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => format!("{e}.{f}"),
                None => outer(n),
            }
        };
        let mut o = String::new();
        let index = self.index_var(s);
        self.walk_begin(s, &index);
        let mut body = self.ts_element_ranges(s, &local);
        let (seen, step) = self.ts_unique(s, sums, &local);
        o.push_str(&seen);
        body.push_str(&step);
        // Past a `stop`, the door alone (§15.328).
        if let Some(i) = self.walk_goes_past_stop(sums) {
            body.push_str(&format!("  if ({}) {{\n    continue;\n  }}\n", self.fold_names(i).done));
        }
        // What the walk's own items bind and no one in the walk reads is said to be read, as Go
        // says it (§15.219, §15.342).
        *self.reads_here.borrow_mut() = Some(self.walk_reads(body_items, sums));
        body.push_str(&self.ts_items(&local, trace, Phase::Only(body_items)));
        *self.reads_here.borrow_mut() = None;
        let mut after = String::new();
        for &i in sums {
            match &self.f.items[i] {
                Item::Agg(d) if d.kind.extreme() => {
                    let (init, step) = self.ts_extreme(d, &local);
                    o.push_str(&init);
                    body.push_str(&step);
                }
                Item::Agg(d) => {
                    o.push_str(&format!("  let {}: bigint = 0n; // {}\n", self.ident(&d.name.text), self.agg_word(d)));
                    let v = local(&d.column.text);
                    if d.kind == crate::ast::AggKind::Sum {
                        body.push_str(&format!("  {} += {v}; // {}\n", self.ident(&d.name.text), d.name.text));
                    } else {
                        let test = self.ts_test(&v, &d.column.text, &d.value);
                        body.push_str(&format!("  if ({test}) {{ // {}\n    {} += 1n;\n  }}\n", d.name.text, self.ident(&d.name.text)));
                    }
                    if let Some((_, cap)) = self.sum_caps().into_iter().find(|(x, _)| x.name.text == d.name.text) {
                        let n = self.ident(&d.name.text);
                        after.push_str(&format!(
                            "  if ({n} > {cap}n) {{\n    throw new RuleInputError(`{}`, {n});\n  }}\n",
                            crate::door::out_of_range(&d.name.text)
                        ));
                    }
                    // The bottom of its range, where a walk can come in under it (§15.314).
                    if let Some(floor) = self.walk_floor(d) {
                        let n = self.ident(&d.name.text);
                        after.push_str(&format!(
                            "  if ({n} < {floor}n) {{\n    throw new RuleInputError(`{}`, {n});\n  }}\n",
                            crate::door::out_of_range(&d.name.text)
                        ));
                    }
                }
                Item::Fold(fold) => {
                    let (init, step, fin) = self.ts_fold(i, fold, &seq, &local);
                    o.push_str(&init);
                    body.push_str(&step);
                    after.push_str(&fin);
                }
                _ => {}
            }
        }
        // An element no line of the walk reads, as where each row of its tables tests a field with
        // `-`, is said to be left unread, which `tsc --noUnusedLocals` asks for, as Go, Rust and
        // Swift say it (§15.342).
        if !clash::reads_word(&body, &e, clash::Syntax::Script) {
            body.insert_str(0, &format!("  void {e};\n"));
        }
        // The index of the element is bound where a sentence of the walk says it (§15.324).
        if self.walk_end() {
            o.push_str(&format!("  for ({} [{index}, {e}] of {seq}.entries()) {{\n", self.ts_elem_binding(s)));
        } else {
            o.push_str(&format!("  for ({} {e} of {seq}) {{\n", self.ts_elem_binding(s)));
        }
        o.push_str(&Self::indent_block(&body, "  "));
        o.push_str("  }\n");
        o.push_str(&after);
        o
    }

    /// A test on a column of one element, in TypeScript: `count`'s, and the `where` of `max` and
    /// `min`.
    fn ts_test(&self, v: &str, col: &str, value: &Option<crate::ast::Name>) -> String {
        let ty = self.ty_of(col);
        match (&ty, value) {
            (Ty::Bool, Some(w)) if w.text == crate::kw::FALSE => format!("!{v}"),
            (Ty::Bool, _) => v.to_string(),
            (_, Some(w)) => {
                let cls = self.ts_ty(&ty);
                let member = self.value_name(&w.text, &ty).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| w.text.clone());
                // A column never given that value is read as its whole type there (§15.342).
                if Self::ts_refuses(self.ts_narrowed(col).as_ref(), &self.ts_key(&w.text, &ty)) {
                    format!("({v} as {cls}) === {cls}.{member}")
                } else {
                    format!("{v} === {cls}.{member}")
                }
            }
            (_, None) => v.to_string(),
        }
    }

    /// A `max` or a `min`, in TypeScript (§15.219).
    fn ts_extreme(&self, d: &crate::ast::AggDecl, local: &dyn Fn(&str) -> String) -> (String, String) {
        let n = self.ident(&d.name.text);
        let set = self.temp(&format!("{n}Set"));
        let v = local(&d.column.text);
        let ty = self.ty_of(&d.column.text);
        let op = if d.kind == crate::ast::AggKind::Max { ">" } else { "<" };
        let (ts_t, start, better) = match self.extreme_enum(d) {
            Some(en) => {
                let t = self.enum_type(&en);
                let w = self.extreme_empty_word(d).unwrap_or_default();
                (self.ts_ty(&ty), self.ts_value(&w, &ty), format!("_rank_{t}({v}) {op} _rank_{t}({n})"))
            }
            None => ("bigint".to_string(), format!("{}n", self.extreme_empty_int(d).unwrap_or_else(|| "0".into())), format!("{v} {op} {n}")),
        };
        let init = format!("  let {n}: {ts_t} = {start}; // {}\n  let {set} = false;\n", self.agg_word(d));
        let mut step = String::new();
        let pad = match &d.filter {
            Some((w, want)) => {
                step.push_str(&format!("  if ({}) {{ // {}\n", self.ts_test(&local(&w.text), &w.text, want), d.name.text));
                "  "
            }
            None => "",
        };
        step.push_str(&format!("{pad}  if (!{set} || {better}) {{\n{pad}    {n} = {v};\n{pad}    {set} = true;\n{pad}  }}\n"));
        if !pad.is_empty() {
            step.push_str("  }\n");
        }
        (init, step)
    }

    /// A fold, in TypeScript (§15.56).
    fn ts_fold(&self, i: usize, fold: &crate::ast::FoldDecl, seq: &str, local: &dyn Fn(&str) -> String) -> (String, String, String) {
        let names = self.fold_names(i);
        let FoldNames { answer, stopped, done, taken, kept, best, held, key, taken_at_camel: taken_at, .. } = &names;
        let alone = self.fold_alone(i);
        // Ends the loop at a `stop`: a fold alone in its walk with no door to keep past it (§15.328).
        let ends = alone && !self.stop_keeps_door(i);
        let with_held = |n: &str| -> String { if n == crate::kw::HELD { held.clone() } else { local(n) } };
        let (out_name, out_ty) = self.fold_target(fold);
        let acc_ty = if out_ty.is_numeric() { "bigint".to_string() } else { self.ts_ty(&out_ty) };
        // An answer is read in the type the fold answers with: a value of its enum is that
        // enum's member (§15.208).
        let val = |x: &Expr, l: &dyn Fn(&str) -> String| -> String {
            match x {
                Expr::Lit(Lit::Word(w), _) if w != crate::kw::TRUE && w != crate::kw::FALSE => self.ts_value(w, &out_ty),
                _ => ts_expr(unparen(&self.expr(x, l).text)).to_string(),
            }
        };
        let text = |x: &Option<Expr>| -> String { x.as_ref().map(|x| val(x, &with_held)).unwrap_or_else(|| "0n".to_string()) };
        let weighted = fold.weighted();
        let draw = fold.draw.as_ref().map(|d| local(&d.text)).unwrap_or_default();

        let mut init = String::new();
        init.push_str(&format!("  let {answer}: {acc_ty} = {}; // {}\n", text(&fold.empty), tr!("要素ゼロ件の答え", "the answer for no elements")));
        init.push_str(&format!("  let {stopped} = false;\n"));
        if !ends {
            init.push_str(&format!("  let {done} = false;\n"));
        }
        init.push_str(&format!("  let {taken}: {acc_ty} | null = null;\n"));
        // The element a `take_unique` took, which the sentence says beside the one that matched
        // again (§15.324).
        if fold.arms.iter().any(|(_, a, _)| matches!(a, Arm::Take { unique: true, .. })) {
            init.push_str(&format!("  let {taken_at} = 0;\n"));
        }
        init.push_str(&format!("  let {kept}: {acc_ty} | null = null;\n"));
        // The key kept so far, which only `keep_max` reads (§15.342).
        if fold.arms.iter().any(|(_, a, _)| matches!(a, Arm::KeepMax { .. })) {
            init.push_str(&format!("  let {best}: bigint | null = null;\n"));
        }
        if weighted {
            init.push_str(&format!("  let {} = 0n; // {}\n", names.total, tr!("重みの合計", "the running total of the weights")));
            init.push_str(&format!("  const {}: bigint[] = [];\n", names.cuts));
        }

        let mut arms = String::new();
        let v = local(&fold.verdict);
        // What the verdict holds at each arm: an arm that leaves the loop leaves its value out of
        // the arms after it, and an arm for a value it never holds reads it as its whole type
        // (§15.342).
        let mut narrowed = self.ts_narrowed(&fold.verdict);
        for (name, arm, _) in &fold.arms {
            let cls = self.ts_ty(&self.ty_of(&fold.verdict));
            let member = self.value_name(&name.text, &self.ty_of(&fold.verdict)).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| name.text.clone());
            let value = self.ts_key(&name.text, &self.ty_of(&fold.verdict));
            let tested = if Self::ts_refuses(narrowed.as_ref(), &value) { format!("({v} as {cls})") } else { v.clone() };
            if ends && matches!(arm, Arm::Stop(_)) {
                if let Some(s) = narrowed.as_mut() {
                    s.remove(&value);
                }
            }
            arms.push_str(&format!("  if ({tested} === {cls}.{member}) {{ // {}\n", name.text));
            match arm {
                Arm::Next => arms.push_str("    // next\n"),
                Arm::Stop(None) if ends => arms.push_str("    break;\n"),
                Arm::Stop(None) => arms.push_str(&format!("    {done} = true;\n")),
                Arm::Stop(Some(x)) => {
                    arms.push_str(&format!("    {answer} = {};\n", val(x, local)));
                    if ends {
                        arms.push_str(&format!("    {stopped} = true;\n    break;\n"));
                    } else {
                        arms.push_str(&format!("    {stopped} = true;\n    {done} = true;\n"));
                    }
                }
                Arm::Take { expr, unique } => {
                    if *unique {
                        let here = self.here_used();
                        arms.push_str(&format!("    if ({taken} !== null) {{\n"));
                        arms.push_str(&format!(
                            "      throw new RuleContradictionError(`{}`);\n    }}\n",
                            ts_said(&crate::door::fold_contradiction(&fold.verdict, &fold.over, crate::door::BEFORE, crate::door::HERE), &here, taken_at)
                        ));
                        arms.push_str(&format!("    {taken} = {};\n", val(expr, local)));
                        arms.push_str(&format!("    {taken_at} = {here};\n"));
                    } else {
                        arms.push_str(&format!("    if ({taken} === null) {{\n"));
                        arms.push_str(&format!("      {taken} = {};\n    }}\n", val(expr, local)));
                    }
                }
                Arm::KeepMax { expr, key: k } => {
                    arms.push_str(&format!("    const {key} = {};\n", ts_expr(unparen(&self.expr(k, local).text))));
                    arms.push_str(&format!("    if ({best} === null || {key} > {best}) {{\n"));
                    arms.push_str(&format!("      {best} = {key};\n"));
                    arms.push_str(&format!("      {kept} = {};\n    }}\n", val(expr, local)));
                }
                // The weight goes on the running total, and the element is taken where the drawn
                // value is below the total for the first time (§15.226). A total past int64 is
                // refused there, before another element is read: the other languages hold it in an
                // int64, and every language answers alike.
                Arm::TakeWeighted { expr, weight } => {
                    let (w, t, c) = (&names.weight, &names.total, &names.cuts);
                    arms.push_str(&format!("    const {w} = {};\n", ts_expr(unparen(&self.expr(weight, local).text))));
                    arms.push_str(&format!("    {t} += {w};\n"));
                    arms.push_str(&format!(
                        "    if ({t} > 9223372036854775807n) {{\n      throw new RuleInputError(`{}`);\n    }}\n",
                        crate::door::weights_past(&fold.over)
                    ));
                    arms.push_str(&format!("    {c}.push({t});\n"));
                    arms.push_str(&format!("    if ({taken} === null && {draw} < {t}) {{\n"));
                    arms.push_str(&format!("      {taken} = {};\n    }}\n", val(expr, local)));
                }
            }
            arms.push_str("  }\n");
        }
        let step = if alone { arms } else { format!("  if (!{done}) {{ // {}\n{}  }}\n", fold.verdict, Self::indent_block(&arms, "  ")) };

        // The drawn value of a pick by weight is below the total of the weights, unless they add up
        // to 0 (§15.226).
        let mut fin = String::new();
        if weighted {
            let t = &names.total;
            let what = fold.draw.as_ref().map(|d| crate::door::draw_past(&d.text)).unwrap_or_default();
            fin.push_str(&format!("  if ({t} > 0n && {draw} >= {t}) {{\n    throw new RuleInputError(`{what}`, {draw});\n  }}\n"));
        }
        fin.push_str(&format!("  if ({seq}.length > 0 && !{stopped}) {{\n"));
        fin.push_str(&format!("    const {held} = {taken} !== null ? {taken} : ({kept} !== null ? {kept} : {});\n", text(&fold.empty)));
        let ex = text(&fold.exhausted);
        // An answer that does not read what is held, where Go says it too (§15.342).
        if !has_word(&ex, held) {
            fin.push_str(&format!("    void {held};\n"));
        }
        fin.push_str(&format!("    {answer} = {ex};\n  }}\n"));
        fin.push_str(&format!("  const {} = {answer};\n", self.ident(&out_name)));
        fin.push_str(&self.ts_unread(&out_name));
        (init, step, fin)
    }

    /// A number whose value is whole, `1000` from a caller's `JSON.parse`, is that integer, here as
    /// at the inputs (§15.205): what reads the element next reads a copy that holds it as a bigint.
    fn ts_element_whole(&self, v: &str, ty: &Ty) -> String {
        let Some((e, field)) = v.rsplit_once('.') else { return String::new() };
        format!("  if (typeof {v} === \"number\" && Number.isInteger({v})) {{\n    {e} = {{ ...{e}, {field}: BigInt({v}) as {} }};\n  }}\n", self.ts_ty(ty))
    }

    /// The first look at each element of every sequence, in TypeScript (§15.328): before any input
    /// is held to its range, each element in turn, and each of its fields in the order the rule
    /// declares them, of its kind — a value of its enum, a truth value, a string (§15.339), an
    /// integer, and a number one that int64 holds, as a reader of the wire takes it.
    fn ts_element_kinds(&self) -> String {
        let mut out = String::new();
        for (s, el) in self.f.elements.iter().enumerate() {
            let fields: BTreeMap<String, String> = el.fields.iter().map(|fd| (fd.name.text.clone(), pub_name(&fd.name))).collect();
            let e = self.elem_var(s);
            let local = |n: &str| -> String { fields.get(n).map(|f| format!("{e}.{f}")).unwrap_or_else(|| n.to_string()) };
            let index = self.index_var(s);
            self.walk_begin(s, &index);
            let mut o = String::new();
            let here = self.here();
            for i in self.element_fields(s) {
                let v = local(&i.name.text);
                // The sentences name the field by its place on the element (§15.324).
                let said = || self.said_name(&i.name.text);
                let ty = self.ty_of(&i.name.text);
                // An optional field is looked at as the type it wraps when it has a value (§15.201).
                let optional = matches!(ty, Ty::Opt(_));
                let ty = ty.present().clone();
                let mut o = Guarded::new(&mut o, optional.then(|| (format!("  if ({v} !== null) {{\n"), "  }\n".to_string())), "  ");
                match &ty {
                    Ty::Bool => o.push_str(&format!(
                        "  if (typeof {v} !== \"boolean\") {{\n    throw new RuleInputError(`{}`, {v});\n  }}\n",
                        ts_said(&not_bool(&said()), &here, "")
                    )),
                    Ty::Str => o.push_str(&format!(
                        "  if (typeof {v} !== \"string\") {{\n    throw new RuleInputError(`{}`, {v});\n  }}\n",
                        ts_said(&crate::door::not_string(&said()), &here, "")
                    )),
                    Ty::Enum(n) => {
                        let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                        o.push_str(&format!(
                            "  if (!Object.values({cls}).includes({v})) {{\n    throw new RuleInputError(`{}`, {v});\n  }}\n",
                            ts_said(&crate::door::not_in_enum(&said(), n), &here, "")
                        ));
                    }
                    Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
                        if ty != Ty::Date {
                            o.push_str(&self.ts_element_whole(&v, &ty));
                        }
                        o.push_str(&format!(
                            "  if (typeof {v} !== \"bigint\") {{\n    throw new RuleInputError(`{}`, {v});\n  }}\n",
                            ts_said(&crate::door::not_integer(&said()), &here, "")
                        ));
                        if ty != Ty::Date {
                            o.push_str(&format!(
                                "  if ({}) {{\n    throw new RuleInputError(`{}`, {v});\n  }}\n",
                                or_out_of(&v, INT64_ENDS, "n").unwrap_or_default(),
                                ts_said(&crate::door::out_of_range(&said()), &here, "")
                            ));
                        }
                    }
                    _ => {}
                }
            }
            let used = self.walk_end();
            if !o.is_empty() {
                let seq = self.held(&el.name);
                let bind = self.ts_elem_binding(s);
                if used {
                    out.push_str(&format!("  for ({bind} [{index}, {e}] of {seq}.entries()) {{\n"));
                } else {
                    out.push_str(&format!("  for ({bind} {e} of {seq}) {{\n"));
                }
                out.push_str(&Self::indent_block(&o, "  "));
                out.push_str("  }\n");
            }
        }
        out
    }

    /// The door's look at one element where the walk comes to it (§15.328): each field inside the
    /// range it declares, in the order the rule declares them, on the values the first look found
    /// of their kinds.
    fn ts_element_ranges(&self, s: usize, local: &dyn Fn(&str) -> String) -> String {
        let mut o = String::new();
        let here = self.here();
        for i in self.element_fields(s) {
            let v = local(&i.name.text);
            // The sentences name the field by its place on the element (§15.324). Asked for only
            // where one is written: a walk binds the index only where a sentence reads it.
            let said = || self.said_name(&i.name.text);
            let ty = self.ty_of(&i.name.text);
            // An optional input is guarded as the type it wraps whenever it has a value (§15.201).
            let optional = matches!(ty, Ty::Opt(_));
            let ty = ty.present().clone();
            let mut o = Guarded::new(&mut o, optional.then(|| (format!("  if ({v} !== null) {{\n"), "  }\n".to_string())), "  ");
            if let Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date = &ty {
                // The walk reads the element again from the sequence, so a whole number is made the
                // bigint it is again.
                if ty != Ty::Date {
                    o.push_str(&self.ts_element_whole(&v, &ty));
                }
                if let Some(test) = or_out_of(&v, crate::door::ends(self.c, &i.name.text, true), "n") {
                    // A copy of the element made above reads the field as the optional it is
                    // declared, inside the `!== null` of the field, so the test says it again
                    // (§15.343).
                    let test = if optional && ty != Ty::Date { format!("{v} !== null && ({test})") } else { test };
                    o.push_str(&format!(
                        "  if ({test}) {{\n    throw new RuleInputError(`{}`, {v});\n  }}\n",
                        ts_said(&crate::door::out_of_range(&said()), &here, "")
                    ));
                }
            }
        }
        o
    }

    /// The fields marked `unique` (§15.290), in TypeScript: a set for each before the first walk
    /// over the sequence, and in the walk, after the guards of one element, the refusal of a value
    /// an element before it had. A bigint and a string compare by value in a `Set`.
    fn ts_unique(&self, s: usize, sums: &[usize], local: &dyn Fn(&str) -> String) -> (String, String) {
        let seq = &self.f.elements[s].name.text;
        let (mut init, mut step) = (String::new(), String::new());
        for fd in self.unique_fields(s, sums) {
            let seen = self.temp(&format!("{}Seen", self.ident(&fd.name.text)));
            let v = local(&fd.name.text);
            // Where each value was first seen, which the sentence says beside the element it came
            // round again on (§15.324).
            let here = self.here_used();
            init.push_str(&format!("  const {seen}: Map<unknown, number> = new Map();\n"));
            step.push_str(&format!(
                "  if ({seen}.has({v})) {{\n    throw new RuleInputError(`{}`, {v});\n  }}\n  {seen}.set({v}, {here});\n",
                ts_said(&crate::door::duplicate(seq, &fd.name.text, crate::door::BEFORE, crate::door::HERE), &here, &format!("{seen}.get({v})"))
            ));
        }
        (init, step)
    }

    /// How the walk binds each element: `let` when the element door may rebind it to a copy that
    /// holds a whole number as a bigint (§15.205), `const` otherwise.
    fn ts_elem_binding(&self, s: usize) -> &'static str {
        let rebinds = self.element_fields(s).iter().any(|i| {
            matches!(self.ty_of(&i.name.text).present(), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number)
        });
        if rebinds { "let" } else { "const" }
    }

    /// The rule's items as TypeScript, at the base indentation. `local` decides how a name
    /// is spelled, which is where a walk puts the element's own fields (§15.56).
    fn ts_items(&self, local: &dyn Fn(&str) -> String, trace: &str, phase: Phase) -> String {
        let mut o = String::new();
        for (i, it) in self.f.items.iter().enumerate() {
            if !self.in_phase(i, it, phase) {
                continue;
            }
            match it {
                Item::Derived(d) => {
                    let e = self.ts_meeting(&d.expr, self.expr(&d.expr, local).text, local);
                    o.push_str(&format!(
                        "  const {} = {}; // {}\n",
                        self.ident(&d.name.text),
                        ts_expr(unparen(&e)),
                        tr!("導出", "derived value")
                    ));
                    o.push_str(&self.ts_unread(&d.name.text));
                }
                Item::Define(d) => {
                    let e = self.ts_meeting(&d.expr, self.expr(&d.expr, local).text, local);
                    o.push_str(&format!(
                        "  const {} = {}; // {}\n",
                        self.ident(&d.name.text),
                        ts_expr(unparen(&e)),
                        tr!("定義", "definition")
                    ));
                    // The range the rule accepts for it, held where it is worked out (§15.313).
                    let n = self.ident(&d.name.text);
                    match self.accepted_ends(d).and_then(|ends| or_out_of(&n, ends, "n")) {
                        Some(test) => {
                            let said = ts_said(&crate::door::out_of_range(&self.said_name(&d.name.text)), &self.here(), "");
                            o.push_str(&format!("  if ({test}) {{\n    throw new RuleInputError(`{said}`, {n});\n  }}\n"));
                        }
                        // What the check does not read either (§15.342).
                        None => o.push_str(&self.ts_unread(&d.name.text)),
                    }
                }
                Item::Agg(_) | Item::Fold(_) => {}
                Item::Table(t) => {
                    if let Some(t) = self.c.table_at(t) {
                        o.push_str(&self.ts_table(t, local, trace));
                    }
                }
            }
        }
        o
    }

    fn ts_fn(&self) -> String {
        let fname = pub_name(&self.f.name);
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", self.held(&i.name), self.ts_ty(&self.ty_of(&i.name.text))))
            .collect();
        for (s, el) in self.f.elements.iter().enumerate() {
            params.push(format!("{}: readonly {}[]", self.held(&el.name), self.element_type(s)));
        }
        let outs = &self.f.outputs;
        let ret = if outs.len() == 1 {
            self.ts_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let mut o = String::new();

        // One element is a row of inputs, so it gets the same shape an `Output` gets.
        for (s, el) in self.f.elements.iter().enumerate() {
            o.push_str(&format!("/** {} */\n", tr!("{} の一件", "one of {}", el.name.text)));
            o.push_str(&format!("export interface {} {{\n", self.element_type(s)));
            for fd in &el.fields {
                o.push_str(&format!(
                    "  {}: {};\n",
                    pub_name(&fd.name),
                    self.ts_ty(&self.ty_of(&fd.name.text))
                ));
            }
            o.push_str("}\n\n");
        }

        if outs.len() > 1 {
            o.push_str("export interface Output {\n");
            for od in outs {
                o.push_str(&format!(
                    "  {}: {};\n",
                    pub_name(&od.name),
                    self.ts_ty(&self.ty_of(&od.name.text))
                ));
            }
            o.push_str("}\n\n");
        }

        // The public function keeps the plain signature; the branches live in its traced
        // twin, which also returns the rows that matched (§15.33).
        let mut args: Vec<String> = self.f.inputs.iter().map(|i| self.held(&i.name)).collect();
        for el in &self.f.elements {
            args.push(self.held(&el.name));
        }
        let traced = format!("{fname}_traced");
        o.push_str(&format!("/** {} */\n", plain_doc(&self.f.name.text, &self.f.version, &traced)));
        o.push_str(&format!(
            "export function {}({}): {ret} {{\n  return {traced}({})[0];\n}}\n\n",
            self.fam(&fname),
            params.join(", "),
            args.join(", ")
        ));
        o.push_str(&format!("/** {} */\n", traced_doc(&self.f.name.text, &self.f.version)));
        o.push_str(&format!("export function {traced}({}): [{ret}, Fired[]] {{\n", params.join(", ")));
        // A rule that picks by weight runs its walk in a function of its own, which gives back the
        // running totals of the weights too (§15.225).
        let walked = self.weighted_fold().map(|(i, _)| (self.walked_fn("typescript"), self.fold_names(i).cuts));
        if let Some((w, _)) = &walked {
            o.push_str(&format!("  const [out, trace] = {w}({});\n  return [out, trace];\n}}\n\n", args.join(", ")));
            o.push_str(&format!("/** {} */\nexport function {w}({}): [{ret}, Fired[], bigint[]] {{\n", walked_doc(), params.join(", ")));
        }

        // An input with two or more things wrong with it is refused for the first of them in the
        // order every door looks (§15.328): each input of its kind, then each element of each
        // sequence, then each input inside its range, then the constraints.
        let local = |n: &str| -> String { self.ident(n) };
        for i in &self.f.inputs {
            let v = self.held(&i.name);
            let ty = self.ty_of(&i.name.text);
            // An optional input is guarded as the type it wraps whenever it has a value (§15.201).
            let optional = matches!(ty, Ty::Opt(_));
            let ty = ty.present().clone();
            let mut o = Guarded::new(&mut o, optional.then(|| (format!("  if ({v} !== null) {{\n"), "  }\n".to_string())), "  ");
            match &ty {
                // A truth value is `true` or `false` and nothing else (§15.203).
                Ty::Bool => o.push_str(&format!(
                    "  if (typeof {v} !== \"boolean\") {{\n    throw new RuleInputError(`{}`, {v});\n  }}\n",
                    not_bool(&i.name.text)
                )),
                // A string is a string and nothing else (§15.339): a number given for it is equal
                // to no cell of the table, and fell to another row without a word.
                Ty::Str => o.push_str(&format!(
                    "  if (typeof {v} !== \"string\") {{\n    throw new RuleInputError(`{}`, {v});\n  }}\n",
                    crate::door::not_string(&i.name.text)
                )),
                Ty::Enum(n) => {
                    let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                    o.push_str(&format!(
                        "  if (!Object.values({cls}).includes({v})) {{\n    \
                         throw new RuleInputError(`{}`, {v});\n  }}\n",
                        crate::door::not_in_enum(&i.name.text, n)
                    ));
                }
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
                    // The type says bigint, but a JavaScript caller can pass anything, and a
                    // number would fail somewhere inside the arithmetic instead of here
                    // (§15.43). A number whose value is whole, `1000` from a caller's
                    // `JSON.parse`, is that integer (§15.205).
                    if ty != Ty::Date {
                        o.push_str(&format!(
                            "  if (typeof {v} === \"number\" && Number.isInteger({v})) {{\n    {v} = BigInt({v}) as {};\n  }}\n",
                            self.ts_ty(&ty)
                        ));
                    }
                    o.push_str(&format!(
                        "  if (typeof {v} !== \"bigint\") {{\n    throw new RuleInputError(`{}`, {v});\n  }}\n",
                        crate::door::not_integer(&i.name.text)
                    ));
                    // A whole number int64 does not hold, where a reader of the wire refuses it.
                    if ty != Ty::Date {
                        o.push_str(&format!(
                            "  if ({}) {{\n    throw new RuleInputError(`{}`, {v});\n  }}\n",
                            or_out_of(&v, INT64_ENDS, "n").unwrap_or_default(),
                            crate::door::out_of_range(&i.name.text)
                        ));
                    }
                }
                _ => {}
            }
        }
        o.push_str(&self.ts_element_kinds());
        for i in &self.f.inputs {
            let v = self.held(&i.name);
            let ty = self.ty_of(&i.name.text);
            let optional = matches!(ty, Ty::Opt(_));
            let ty = ty.present().clone();
            let mut o = Guarded::new(&mut o, optional.then(|| (format!("  if ({v} !== null) {{\n"), "  }\n".to_string())), "  ");
            match &ty {
                // The values the range of an ordered enum lets in, first to last (§15.210).
                Ty::Enum(n) => {
                    if let Some((lo, hi)) = self.c.enum_ranges.get(&i.name.text).and_then(|vs| Some((vs.first()?, vs.last()?))) {
                        let r = self.ts_rank_fn(n);
                        o.push_str(&format!(
                            "  if ({r}({v}) < {r}({}) || {r}({v}) > {r}({})) {{\n    throw new RuleInputError(`{}`, {v});\n  }}\n",
                            self.ts_value(lo, &ty),
                            self.ts_value(hi, &ty),
                            crate::door::out_of_range(&i.name.text)
                        ));
                    }
                }
                // Each end the range writes, and nothing past ±(2^63−1), which a bigint can be and
                // no int64 can (§15.295).
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
                    if let Some(test) = or_out_of(&v, crate::door::ends(self.c, &i.name.text, true), "n") {
                        o.push_str(&format!(
                            "  if ({test}) {{\n    throw new RuleInputError(`{}`, {v});\n  }}\n",
                            crate::door::out_of_range(&i.name.text)
                        ));
                    }
                }
                _ => {}
            }
        }

        o.push_str(&self.constraint_guards(&local, Lang::Ts, "  ", |m| {
            format!("    throw new RuleInputError(`{m}`);\n")
        }));

        let trace = self.temp("trace");
        o.push_str(&format!("  const {trace}: Fired[] = [];\n"));

        if self.f.walks() {
            o.push_str(&self.ts_walks(&local, &trace));
        } else {
            o.push_str(&self.ts_items(&local, &trace, Phase::All));
        }

        let cast = |ty: &Ty, body: String| -> String {
            match ty {
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => format!("({body}) as {}", self.ts_ty(ty)),
                _ => body,
            }
        };
        // Every output takes the same three steps: its source — the `result` expression for
        // the first output, the binding of its own name otherwise — brought to the wire
        // scale, then its declared rounding applied exactly once (§15.16).
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            // An optional output is its column's value when there is one, taken through the same
            // steps, and `null` when there is none (§15.249).
            let opt = self.opt_out(oi);
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let res = self.onto_wire(res, os, od);
            let ty = self.ty_of(out_name);
            let body = match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    if res.scale != os && !opt {
                        let raw = self.temp(&raw_base(oi));
                        o.push_str(&format!(
                            "  const {raw} = {}; // {}\n",
                            ts_expr(unparen(&res.text)),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("_round{}({raw}, {grid_i}n) / {}n", pascal(mode_fn(m)), res.scale / os)
                    } else if res.scale != os {
                        format!("_round{}({}, {grid_i}n) / {}n", pascal(mode_fn(m)), ts_expr(unparen(&res.text)), res.scale / os)
                    } else {
                        format!("_round{}({}, {grid_i}n)", pascal(mode_fn(m)), ts_expr(&res.text))
                    }
                }
                None => ts_expr(&res.text),
            };
            let v = local(out_name);
            finals.push(match cast(ty.present(), body) {
                b if opt && b != v => format!("{v} === null ? null : ({b})"),
                b => b,
            });
        }
        let cuts = walked.as_ref().map(|(_, c)| format!(", {c}")).unwrap_or_default();
        if outs.len() == 1 {
            o.push_str(&format!("  return [{}, {trace}{cuts}];\n}}\n", finals[0]));
        } else {
            let fields: Vec<String> = outs
                .iter()
                .zip(&finals)
                .map(|(od, body)| format!("{}: {body}", pub_name(&od.name)))
                .collect();
            o.push_str(&format!("  return [{{ {} }}, {trace}{cuts}];\n}}\n", fields.join(", ")));
        }
        o
    }

    fn ts_table(&self, t: &Table, local: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = format!("  // {}\n", self.table_head(t));
        // `let` up front: a binding made inside a branch does not leave it.
        for oc in &t.outputs {
            let ty = self.ty_of(&oc.name.text);
            let decl = |ty: &Ty| match ty {
                Ty::Bool => "boolean".to_string(),
                Ty::Str => "string".to_string(),
                Ty::Enum(_) => self.ts_ty(ty),
                _ => "bigint".to_string(),
            };
            // An optional column holds `null` when there is no value (§15.249).
            let decl = match &ty {
                Ty::Opt(inner) => format!("{} | null", decl(inner)),
                ty => decl(ty),
            };
            o.push_str(&format!("  let {}: {decl};\n", self.ident(&oc.name.text)));
        }
        // What each column's local holds on the branch a row is tested on (§15.342).
        let mut narrowed: Vec<_> = t.inputs.iter().map(|(col, _)| self.ts_narrowed(col)).collect();
        for (ri, row) in t.rows.iter().enumerate() {
            let tests: Vec<(usize, String)> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    let cell = row.cells.get(ci)?;
                    let var = self.ts_compared(cell, col, &local(col), narrowed[ci].as_ref());
                    self.ts_cell(cell, &var, &ty, self.scale(col)).map(|c| (ci, c))
                })
                .collect();
            // A row that tests one column alone leaves the value it tests out of that local on the
            // rows after it, as `tsc` reads an `else`.
            if let [(ci, _)] = tests.as_slice() {
                let took = row.cells.get(*ci).and_then(|cell| self.ts_took(cell, &t.inputs[*ci].0));
                if let (Some(s), Some(k)) = (narrowed[*ci].as_mut(), took) {
                    s.remove(&k);
                }
            }
            let conds: Vec<String> = tests.into_iter().map(|(_, c)| c).collect();
            let cond = if conds.is_empty() { "true".into() } else { conds.join(" && ") };
            let cells: Vec<String> = row.cells.iter().map(cell_src).chain(row.outs.iter().map(out_src)).collect();
            let head = if ri == 0 { "  if" } else { " else if" };
            let line = self.row_head(t, ri, &cells.join(" | "));
            if ri == 0 {
                o.push_str(&format!("{head} ({cond}) {{ // {line}\n"));
            } else {
                o.push_str(&format!("  }}{head} ({cond}) {{ // {line}\n"));
            }
            for (oi, oc) in t.outputs.iter().enumerate() {
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => {
                        let oty = self.ty_of(&oc.name.text);
                        format!("{}n", self.int_lit(n, &oty, self.scale(&oc.name.text)))
                    }
                    Some(OutCell::Lit(l)) => match l {
                        Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                        Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                        Lit::Word(w) => self.ts_value(w, &self.ty_of(&oc.name.text)),
                        Lit::Date(y, m, d) => format!("{}n", crate::types::date_ord(*y, *m, *d).num),
                        Lit::Str(x) => str_lit(x),
                        _ => "0n".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            "true".into()
                        } else if w == crate::kw::FALSE {
                            "false".into()
                        } else if self.is_value(w) {
                            self.ts_value(w, &self.ty_of(&oc.name.text))
                        } else {
                            let v = local(w);
                            match ts_expr(&self.rescaled(w, &oc.name.text, v.clone())) {
                                // A value that may be absent is scaled only when it is there.
                                r if r != v && matches!(self.ty_of(w), Ty::Opt(_)) => format!("{v} === null ? null : {r}"),
                                r => r,
                            }
                        }
                    }
                    Some(OutCell::Nothing) => "null".into(),
                    None => "0n".into(),
                };
                o.push_str(&format!("    {} = {v};\n", self.ident(&oc.name.text)));
            }
            let (tn, rn, lbl) = self.fired_of(t, ri);
            if lbl.is_empty() {
                o.push_str(&format!("    {trace}.push({{ table: {tn:?}, row: {rn} }});\n"));
            } else {
                o.push_str(&format!("    {trace}.push({{ table: {tn:?}, row: {rn}, label: {lbl:?} }});\n"));
            }
        }
        o.push_str(&format!(
            "  }} else {{\n    throw new Error(\"{}\");\n  }}\n",
            tr!("到達不能: 完全性は rulec が静的に検査済み", "unreachable: completeness was statically checked by rulec")
        ));
        for oc in &t.outputs {
            o.push_str(&self.ts_unread(&oc.name.text));
        }
        o.push_str(&self.guards(t, local, Lang::Ts, "  ", |name, i, j| {
            format!(
                "      throw new RuleContradictionError(\"{}\");\n",
                tr!("表 {name}: {i} と {j} が同時に当てはまりました", "table {name}: {i} and {j} matched at the same time")
            )
        }));
        o
    }

    /// The runner imports the enums' types by their names, which hide a global of a name of theirs
    /// there as well (§15.336).
    pub fn ts_runner(&self) -> String {
        self.apart_types(clash::Syntax::Script, |g| g.ts_runner_text())
    }

    fn ts_runner_text(&self) -> String {
        let alias = pub_name(&self.f.name);
        let d = runner_local("d", &alias);
        let r = runner_local("r", &alias);
        let line = runner_local("line", &alias);
        let lines = runner_local("lines", &alias);
        let trace = runner_local("trace", &alias);
        let a = runner_local("a", &alias);
        let mut args: Vec<String> = Vec::new();
        let mut imports: Vec<String> =
            vec![format!("{alias}_traced"), format!("{alias}_record"), "RuleInputError".into(), "RuleContradictionError".into()];
        // A brand is a type, and node's type stripping can only erase a whole `import type`
        // statement — a type name mixed into a value import is a syntax error there.
        let mut type_imports: Vec<String> = Vec::new();
        // One input read off the wire the way every door reads it (§15.204): there at all, of
        // its kind, and then built into the module's type. A refusal is the module's own
        // `RuleInputError`, with the sentence every language says.
        // `said` is what the sentences call the value: the name, or for a field of an element its
        // place, `lines[\u{1}].amount`, which takes the index `at` of the element (§15.324).
        #[allow(clippy::too_many_arguments)]
        fn ts_read(g: &Gen, ty: &Ty, src: &str, name: &str, said: &str, at: &str, imports: &mut Vec<String>, type_imports: &mut Vec<String>) -> String {
            let lit = |x: String| -> String { if names_element(&x) { format!("`{}`", ts_said(&x, at, "")) } else { format!("{x:?}") } };
            let optional = matches!(ty, Ty::Opt(_));
            let take = format!("_take({src}, {name:?}, {}{})", lit(crate::door::missing(said)), if optional { ", true" } else { "" });
            let q = if optional { "q" } else { "" };
            match ty.present() {
                Ty::Enum(n) => {
                    let cls = g.enum_names.get(n).cloned().unwrap_or_default();
                    if !imports.contains(&cls) {
                        imports.push(cls.clone());
                    }
                    format!("_enum{q}(Object.values({cls}), {take}, {}) as {cls}", lit(crate::door::not_in_enum(said, n)))
                }
                Ty::Bool => format!("_bool{q}({take}, {})", lit(crate::door::not_bool(said))),
                Ty::Date => format!("_date{q}({take}, {})", lit(crate::door::not_date(said))),
                Ty::Str => format!("_str{q}({take}, {})", lit(crate::door::not_string(said))),
                Ty::Number => format!("_int{q}({take}, {}, {})", lit(crate::door::not_integer(said)), lit(crate::door::out_of_range(said))),
                t => {
                    let brand = g.ts_ty(t);
                    if !type_imports.contains(&brand) {
                        type_imports.push(brand.clone());
                    }
                    format!("_int{q}({take}, {}, {}) as {brand}", lit(crate::door::not_integer(said)), lit(crate::door::out_of_range(said)))
                }
            }
        }
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            args.push(ts_read(self, &ty, &d, &i.name.text, &i.name.text, "", &mut imports, &mut type_imports));
        }
        // The sequence a walk reads, one element at a time (§15.56), after the inputs: every reader
        // takes the arguments in the order the function does (§15.328).
        let mut sequences: Vec<(String, String)> = Vec::new();
        for (s, el) in self.f.elements.iter().enumerate() {
            let seq = &el.name.text;
            let e = runner_local("e", &alias);
            // A sentence about an element names it by its index (§15.324).
            let at = runner_local("i", &alias);
            let obj = format!("_obj({e}, `{}`)", ts_said(&crate::door::not_object(seq, crate::door::HERE), &at, ""));
            let fields: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    let said = crate::door::element(seq, crate::door::HERE, &fd.name.text);
                    format!("{}: {}", pub_name(&fd.name), ts_read(self, &ty, &obj, &fd.name.text, &said, &at, &mut imports, &mut type_imports))
                })
                .collect();
            let rows = runner_local(&if s == 0 { "rows".to_string() } else { format!("rows{}", s + 1) }, &alias);
            sequences.push((
                rows,
                format!(
                    "_seq(_take({d}, {seq:?}, {:?}), {:?}).map(({e}, {at}) => ({{ {} }}))",
                    crate::door::missing(seq),
                    crate::door::not_sequence(seq),
                    fields.join(", ")
                ),
            ));
        }
        // The lines that read the arguments before the call, and what the tuple of them is made
        // of: the inputs read first, into a tuple of their own, and then each sequence.
        let ins_name = runner_local("ins", &alias);
        let call = |ins: &[String]| -> (String, Vec<String>) {
            if sequences.is_empty() {
                return (String::new(), ins.to_vec());
            }
            let (mut prelude, mut args) = (String::new(), Vec::new());
            if !ins.is_empty() {
                prelude.push_str(&format!("    const {ins_name} = [{}] as const;\n", ins.join(", ")));
                args.push(format!("...{ins_name}"));
            }
            for (rows, read) in &sequences {
                prelude.push_str(&format!("    const {rows} = {read};\n"));
                args.push(rows.clone());
            }
            (prelude, args)
        };
        // `--chances` reads a line's inputs as the vectors are read (§15.225).
        let (chance_prelude, read_args) = call(&args);
        // A machine's traces (§15.148): the carried input is the state this language answered
        // to the call before, handed over as the value it is.
        let v = runner_local("v", &alias);
        let state = runner_local("state", &alias);
        let (mut head, mut tail) = (String::new(), String::new());
        // A rule that takes nothing reads nothing of what a line hands it (§15.342).
        let unread = if args.is_empty() && sequences.is_empty() { format!("  void {d};\n") } else { String::new() };
        let mut body = format!("  const {d} = JSON.parse({line}, _exact).in as unknown;\n{unread}");
        // The parser of a machine's state read from an `enums` file is imported from that file's
        // module, once (§15.238). The rule's module has one of its own for the enum (§15.319), which
        // the runner does not import beside it.
        let mut state_owner: Option<(String, String)> = None;
        if let (Some((en, _, _)), Some((cin, _)), Some(out)) = (self.machine_consts(), self.carried(), self.carried_out_alias()) {
            let cls = self.enum_names.get(&en).cloned().unwrap_or_default();
            let parse = format!("parse{cls}");
            let mut ws = vec!["INITIAL".to_string(), "isFinal".to_string(), cls.clone()];
            // the module's own parser for an enum it hands on under another type (§15.326)
            match self.c.owners.get(&en).filter(|_| !self.handed_here(&en)) {
                Some(o) => state_owner = Some((pub_name(&o.module), parse)),
                None => ws.push(parse),
            }
            for w in ws {
                if !imports.contains(&w) {
                    imports.push(w);
                }
            }
            if let Some(k) = self.f.inputs.iter().position(|i| i.name.text == cin) {
                args[k] = format!("(\"step\" in {v} ? {state} : {})", args[k]);
            }
            // The values carried along with the state (§15.217): a local each, from the
            // module's constant, read where each trace starts and handed on from the answer.
            let (mut more_head, mut more_meta, mut more_start, mut more_tail) = (String::new(), Vec::<String>::new(), String::new(), String::new());
            for mc in self.more_carried() {
                let local = runner_local(&format!("carried_{}", mc.in_alias), &alias);
                if !imports.contains(&mc.upper()) {
                    imports.push(mc.upper());
                }
                if let Some(k) = self.f.inputs.iter().position(|i| i.name.text == mc.input) {
                    args[k] = format!("(\"step\" in {v} ? {local} : {})", args[k]);
                }
                more_head.push_str(&format!("let {local}: {} = {};\n", self.ts_ty(&mc.ty), mc.upper()));
                more_meta.push(format!("{}: {}", mc.key(), mc.upper()));
                let read = ts_read(self, &mc.ty, &format!("({v}.carried as unknown)"), &mc.input, &mc.input, "", &mut imports, &mut type_imports);
                more_start.push_str(&format!("    {local} = {read};\n"));
                more_tail.push_str(&format!("      {local} = {r}.{};\n", mc.out_alias));
            }
            let carried_meta = if more_meta.is_empty() { String::new() } else { format!(", carried: {{ {} }}", more_meta.join(", ")) };
            head = format!("let {state}: {cls} = INITIAL;\n{more_head}");
            body = format!(
                "  const {v} = JSON.parse({line}, _exact) as Record<string, unknown>;\n  \
                 if (\"machine\" in {v}) {{\n    \
                   console.log(JSON.stringify({{ initial: INITIAL, final: Object.values({cls}).filter((s) => isFinal(s)){carried_meta} }}, _raw));\n    \
                   continue;\n  }}\n  \
                 const {d} = {v}.in as unknown;\n{unread}  \
                 if ({v}.step === \"start\") {{\n    {state} = parse{cls}(String({v}.state));\n{more_start}  }}\n"
            );
            tail = format!("    if (\"step\" in {v}) {{\n      {state} = {};\n{more_tail}    }}\n", self.next_state_of(&r, &format!(".{out}")));
        }
        let (owner_imports, labels) = self.ts_labels_mode(&lines, &mut imports);
        // And with `--prerequisites`, what each block's functions answer (§15.222): a parser of an
        // `enums` file's enum the labels have not imported already comes from that file's module.
        let mut prereq_owners: Vec<(String, Vec<String>)> = Vec::new();
        let labels = labels + &self.ts_prereq_mode(&lines, &mut imports, &mut prereq_owners) + &self.ts_chances_mode(&d, &lines, &read_args, &chance_prelude, &mut imports);
        let (prelude, args) = call(&args);
        if let Some((m, n)) = state_owner {
            match prereq_owners.iter_mut().find(|(x, _)| *x == m) {
                Some((_, ns)) if ns.contains(&n) => {}
                Some((_, ns)) => ns.push(n),
                None => prereq_owners.push((m, vec![n])),
            }
        }
        let more_owners: String = prereq_owners
            .into_iter()
            .filter_map(|(m, ns)| {
                let ns: Vec<String> = ns.into_iter().filter(|n| !has_word(&owner_imports, n) && !imports.contains(n)).collect();
                (!ns.is_empty()).then(|| format!("import {{ {} }} from \"./{m}.ts\";\n", ns.join(", ")))
            })
            .collect();
        let owner_imports = owner_imports + &more_owners;
        let text = format!(
            "// Code generated by rulec {}. DO NOT EDIT.\n\
             import {{ readFileSync }} from \"node:fs\";\n\
             import {{ {} }} from \"./{alias}.ts\";\n{owner_imports}{}\n\
             {TS_WIRE}\
             const {lines} = readFileSync(0, \"utf8\").split(\"\\n\");\n\
             {labels}\
             {head}\
             for (const {line} of {lines}) {{\n  \
                 if ({line}.trim() === \"\") {{\n    continue;\n  }}\n\
                 {body}  \
                 try {{\n\
                 {}    \
                     const {a} = [{}] as const;\n    \
                     const [{r}, {trace}] = {alias}_traced(...{a});\n    \
                     console.log({alias}_record(...{a}, {r}, {trace}));\n\
                 {tail}  \
                 }} catch (e) {{\n    \
                     if (e instanceof RuleInputError) {{\n      _refused(\"input\", e.what);\n    \
                     }} else if (e instanceof RuleContradictionError) {{\n      _refused(\"contradiction\", e.what);\n    \
                     }} else {{\n      throw e;\n    }}\n  }}\n\
                 }}\n",
            ritsu_emit::header::VERSION,
            imports.join(", "),
            if type_imports.is_empty() {
                String::new()
            } else {
                format!("import type {{ {} }} from \"./{alias}.ts\";\n", type_imports.join(", "))
            },
            prelude,
            args.join(", ")
        );
        // Only the readers of the wire the rule's inputs go through, and what they read (§15.342).
        from::ts_without_unread(&text, TS_WIRE_READS)
    }
}


impl<'a> Gen<'a> {
    /// What the TypeScript runner answers with `--labels` (§15.211): for each line, an enum's
    /// name and a value's, the display name the module gives. The functions of the rule's own
    /// enums join its import; those of an `enums` file are imported from that module.
    fn ts_labels_mode(&self, lines: &str, imports: &mut Vec<String>) -> (String, String) {
        let enums = self.labeled_enums();
        if enums.is_empty() {
            return (String::new(), String::new());
        }
        let mut owners: Vec<(String, Vec<String>)> = Vec::new();
        let mut arms = String::new();
        for (en, ty, owner) in &enums {
            let (label, parse) = (self.ts_label_fn(en), format!("parse{ty}"));
            match owner {
                Some(o) => match owners.iter_mut().find(|(m, _)| m == o) {
                    Some((_, ns)) => ns.extend([label.clone(), parse.clone()]),
                    None => owners.push((o.clone(), vec![label.clone(), parse.clone()])),
                },
                None => {
                    for n in [&label, &parse] {
                        if !imports.contains(n) {
                            imports.push(n.clone());
                        }
                    }
                }
            }
            arms.push_str(&format!("    if (en === {}) {{\n      console.log({label}({parse}(v)));\n    }}\n", str_lit(en)));
        }
        // A name the runner imports from the rule's module already is not imported a second time
        // from the owner (§15.238).
        (
            owners
                .iter()
                .filter_map(|(m, ns)| {
                    let ns: Vec<&String> = ns.iter().filter(|n| !imports.contains(n)).collect();
                    (!ns.is_empty()).then(|| format!("import {{ {} }} from \"./{m}.ts\";\n", ns.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(", ")))
                })
                .collect(),
            format!(
                "if (process.argv.includes(\"--labels\")) {{\n  for (const l of {lines}) {{\n    if (l === \"\") {{\n      continue;\n    }}\n    const [en, v] = l.split(\"\\t\");\n{arms}  }}\n  process.exit(0);\n}}\n\n"
            ),
        )
    }
}

// ---------------------------------------------------------------------------
// Rust (§8.3)
// ---------------------------------------------------------------------------

/// Turn the shared expression text into Rust. The same post-processing shape as `go_expr`
/// and `ts_expr`. Integer division truncates toward zero here, exactly as it does in Go and
/// on a JavaScript bigint, so the rounding helpers are the Go ones transliterated.
fn rs_expr(s: &str) -> String {
    arith::rs_expr(&truths(s, "true", "false"))
        .replace(" // ", " / ")
        .replace("_round_down(", "round_down(")
        .replace("_round_up(", "round_up(")
        .replace("_round_half_down(", "round_half_down(")
        .replace("_round_half(", "round_half(")
        .replace("_round_bankers(", "round_bankers(")
        .replace("_min(", "min_i64(")
        .replace("_max(", "max_i64(")
}

/// The error type and the five modes of §7.3.
fn round_rs() -> String {
    format!(
        r#"
/// {err}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleError {{
    /// {input}
    Input {{ what: Said, value: Option<i64> }},
    /// {contra}
    Contradiction {{ what: Said }},
}}

impl std::fmt::Display for RuleError {{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        match self {{
            RuleError::Input {{ what, value: Some(v) }} => write!(f, "{{what}}: {{v}}"),
            RuleError::Input {{ what, .. }} | RuleError::Contradiction {{ what }} => write!(f, "{{what}}"),
        }}
    }}
}}

impl std::error::Error for RuleError {{}}

/// {said}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Said {{
    words: [&'static str; 3],
    places: [usize; 2],
    count: usize,
}}

impl Said {{
    /// {said_of}
    pub const fn of(sentence: &'static str) -> Said {{
        Said {{ words: [sentence, "", ""], places: [0, 0], count: 0 }}
    }}

    /// {said_at}
    pub const fn at(before: &'static str, index: usize, after: &'static str) -> Said {{
        Said {{ words: [before, after, ""], places: [index, 0], count: 1 }}
    }}

    /// {said_two}
    pub const fn two(first: &'static str, i: usize, between: &'static str, j: usize, last: &'static str) -> Said {{
        Said {{ words: [first, between, last], places: [i, j], count: 2 }}
    }}

    /// {said_words}
    pub fn words(&self) -> &[&'static str] {{
        &self.words[..self.count + 1]
    }}

    /// {said_places}
    pub fn places(&self) -> &[usize] {{
        &self.places[..self.count]
    }}
}}

impl std::fmt::Display for Said {{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        f.write_str(self.words[0])?;
        for k in 0..self.count {{
            write!(f, "{{}}{{}}", self.places[k], self.words[k + 1])?;
        }}
        Ok(())
    }}
}}

impl PartialEq<&str> for Said {{
    fn eq(&self, other: &&str) -> bool {{
        // {said_rest}
        struct Rest<'a>(&'a str);
        impl std::fmt::Write for Rest<'_> {{
            fn write_str(&mut self, piece: &str) -> std::fmt::Result {{
                self.0 = self.0.strip_prefix(piece).ok_or(std::fmt::Error)?;
                Ok(())
            }}
        }}
        let mut rest = Rest(other);
        std::fmt::write(&mut rest, format_args!("{{self}}")).is_ok() && rest.0.is_empty()
    }}
}}

impl From<&'static str> for Said {{
    fn from(sentence: &'static str) -> Said {{
        Said::of(sentence)
    }}
}}

pub(crate) fn min_i64(a: i64, b: i64) -> i64 {{
    if a < b {{ a }} else {{ b }}
}}

pub(crate) fn max_i64(a: i64, b: i64) -> i64 {{
    if a > b {{ a }} else {{ b }}
}}

/// {down}
pub(crate) fn round_down(x: i64, g: i64) -> i64 {{
    let v = x.abs() / g * g;
    if x < 0 {{ -v }} else {{ v }}
}}

/// {up}
pub(crate) fn round_up(x: i64, g: i64) -> i64 {{
    let a = x.abs();
    let v = if a % g == 0 {{ a / g * g }} else {{ (a / g + 1) * g }};
    if x < 0 {{ -v }} else {{ v }}
}}

/// {half}
pub(crate) fn round_half(x: i64, g: i64) -> i64 {{
    let a = x.abs();
    let v = if 2 * (a % g) >= g {{ (a / g + 1) * g }} else {{ a / g * g }};
    if x < 0 {{ -v }} else {{ v }}
}}

/// {half_down}
pub(crate) fn round_half_down(x: i64, g: i64) -> i64 {{
    let a = x.abs();
    let v = if 2 * (a % g) > g {{ (a / g + 1) * g }} else {{ a / g * g }};
    if x < 0 {{ -v }} else {{ v }}
}}

/// {bankers}
pub(crate) fn round_bankers(x: i64, g: i64) -> i64 {{
    let a = x.abs();
    let (mut q, r) = (a / g, a % g);
    if 2 * r > g || (2 * r == g && q % 2 == 1) {{
        q += 1;
    }}
    let v = q * g;
    if x < 0 {{ -v }} else {{ v }}
}}
"#,
        err = tr!("この規則が返しうる誤り。", "Everything this rule can go wrong with."),
        input = tr!("宣言した範囲の外。呼び出し側の契約違反。", "Outside the declared input domain: a contract violation by the caller."),
        contra = tr!("規則そのものの矛盾。呼び出し側の誤りではない。", "A contradiction in the rule itself, not a mistake by the caller."),
        said = tr!(
            "誤りの文。並びの要素を言う文は、要素の番号（0 から）を整数のまま持ち、表示するときに文に入れる。文字列と == で比べられる。",
            "The sentence of an error. One that names an element of a sequence holds the element's index (from 0)\n/// as an integer and puts it into the sentence when shown. It compares with a string by ==."
        ),
        said_of = tr!("番号の無い文。", "A sentence with no index in it."),
        said_at = tr!("要素を一つ言う文: before、番号、after。", "A sentence that names one element: before, the index, after."),
        said_two = tr!("要素を二つ言う文: 前の要素の番号 i と、あとの要素の番号 j。", "A sentence that names two elements: the earlier one's index i, the later one's j."),
        said_words = tr!("番号の前と間とあとの文の切れ。番号の数より一つ多い。", "The pieces of the sentence before, between and after the indices: one more than there are indices."),
        said_places = tr!("文が言う要素の番号。言う順に。", "The indices of the elements the sentence names, in the order it names them."),
        said_rest = tr!(
            "比べる文字列の、まだ比べていない残り。表示する文の切れを一つずつ頭から比べ、文字列は作らない。",
            "What is left of the string to compare: each piece the sentence is shown in is held to its head, and no string is made."
        ),
        down = tr!("0 へ寄せる。-4.8円 → -4円。", "Toward zero: -4.8 yen → -4 yen."),
        up = tr!("0 から遠ざける。-4.2円 → -5円。", "Away from zero: -4.2 yen → -5 yen."),
        half = tr!("半分ちょうどは 0 から遠ざける。", "An exact half goes away from zero."),
        half_down = tr!("半分ちょうどは 0 へ寄せる。", "An exact half goes toward zero."),
        bankers = tr!("半分ちょうどは偶数へ。", "An exact half goes to the even neighbor."),
    )
}

impl<'a> Gen<'a> {
    /// The Rust type for a value. A unit is a newtype over `i64`: the compiler refuses
    /// `YenExclTax` where `YenInclTax` was meant, and it costs nothing at run time.
    fn rs_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => self.enum_names.get(n).cloned().unwrap_or_else(|| "String".into()),
            Ty::Bool => "bool".into(),
            Ty::Str => "String".into(),
            Ty::Number | Ty::Date => "i64".into(),
            Ty::Opt(t) => format!("Option<{}>", self.rs_ty(t)),
            _ => brand_of(ty),
        }
    }

    fn rs_value(&self, v: &str, ty: &Ty) -> String {
        match self.value_name(v, ty) {
            Some((ty, alias)) => format!("{ty}::{}", pascal(alias)),
            None => format!("{v:?}"),
        }
    }

    /// Render a cell as a Rust condition. A don't-care yields None (no condition).
    /// `Copy` for a struct of these fields, or just `Clone`. A `String` is not `Copy`, so a
    /// rule with two outputs one of which is a string did not compile at all — and no rule
    /// with a string output was ever generated until §15.88.
    fn rs_derive(&self, mut names: impl Iterator<Item = String>) -> &'static str {
        let owned = names.any(|n| matches!(self.ty_of(&n), Ty::Str | Ty::Opt(_)));
        if owned {
            "#[derive(Clone, PartialEq, Eq, Debug)]"
        } else {
            "#[derive(Clone, Copy, PartialEq, Eq, Debug)]"
        }
    }

    fn rs_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        // An optional column is an `Option<T>`, and every test but `none` is about the value
        // inside it. `memo == Kind::A` does not compile against `Option<Kind>`, so the module
        // itself would not build (DESIGN §15.88). The tests below are written against a bound
        // `__v` and wrapped in `matches!`; a negated cell is the negation of the positive
        // one, because `not 甲` holds of the absent value — `T?` is the enum with one more
        // value, and that is how the checker reads it too.
        let optional = matches!(ty, Ty::Opt(_));
        let raw = var.to_string();
        // A branded value is an i64 inside, which is what a cell compares (§15.201).
        let branded = matches!(inner, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate);
        let bound = if optional { if branded { "__v.0".to_string() } else { "__v".to_string() } } else { raw.clone() };
        let var = bound.as_str();
        let wrap = |c: String, neg: bool| -> String {
            if !optional {
                return c;
            }
            let m = format!("matches!({raw}, Some(__v) if {c})");
            if neg { format!("!{m}") } else { m }
        };
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                Lit::Word(w) => self.rs_value(w, ty),
                Lit::Num(n) => self.int_lit(n, inner, col_scale),
                Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => format!("{s:?}"),
            }
        };
        let any_of = |ls: &Vec<Lit>, neg: bool| -> String {
            let mut out: Vec<String> = Vec::new();
            for l in ls {
                if let Lit::Word(w) = l {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| self.rs_value(m, ty)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            let (op, join) = if neg { ("!=", " && ") } else { ("==", " || ") };
            format!("({})", out.iter().map(|v| format!("{var} {op} {v}")).collect::<Vec<_>>().join(join))
        };
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => return Some(format!("{raw}.is_none()")),
            Cell::Prefix(ps) => ps
                .iter()
                .map(|p| format!("{var}.starts_with({})", format!("{p:?}")))
                .collect::<Vec<_>>()
                .join(" || "),
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                wrap(format!("is_{}({var})", self.ident(w)), false)
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => wrap(var.to_string(), false),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => wrap(format!("!{var}"), false),
            Cell::Lit(l) => wrap(format!("{var} == {}", lit(l)), false),
            Cell::Set(ls) => wrap(any_of(ls, false), false),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                if optional {
                    wrap(format!("is_{}({var})", self.ident(w)), true)
                } else {
                    format!("!is_{}({var})", self.ident(w))
                }
            }
            Cell::Not(ls) => {
                if optional { wrap(any_of(ls, false), true) } else { any_of(ls, true) }
            }
            // A comparison on an ordered enum, by the places its variants were declared in
            // (§15.210). The ranks are compared as integers, not through a derived `PartialOrd`:
            // a model checker reads plain integer comparisons, and says nothing about the rest.
            Cell::Ranked { ops, .. } => wrap(
                ops.iter()
                    .map(|(o, v)| format!("{var}.rank() {} {}.rank()", o.word(), self.rs_value(v, ty)))
                    .collect::<Vec<_>>()
                    .join(" && "),
                false,
            ),
            // A comparison is about the value inside the `Option` too: it was written against
            // `__v` and never put in `matches!`, so the module did not build (§15.201).
            Cell::Cmp(cs) => wrap(
                cs.iter()
                    .map(|(o, l)| {
                        let op = match o {
                            CmpOp::Le => "<=",
                            CmpOp::Ge => ">=",
                            CmpOp::Lt => "<",
                            CmpOp::Gt => ">",
                        };
                        format!("{var} {op} {}", lit(l))
                    })
                    .collect::<Vec<_>>()
                    .join(" && "),
                false,
            ),
        })
    }

    pub fn rust(&self) -> String {
        self.apart_text(clash::Syntax::Rust, |g| g.rust_with(false))
    }

    /// The Rust module as the Wasm crate holds it: the crate is the rule's alone, so the types
    /// of an `enums` file are written into it rather than read from a sibling (§15.212).
    pub fn rust_wasm(&self) -> String {
        self.apart_text(clash::Syntax::Rust, |g| g.rust_with(true))
    }

    fn rust_with(&self, inline: bool) -> String {
        // The module of a package of ritsu's (ritsu's DESIGN 9.3): one module of a library crate,
        // beside the other rules', so what one rule does not call of its helpers is no warning
        // there, nor is a lint of clippy's on the shapes every rule's module shares. The module
        // `rulec gen` writes allows the same itself, as every file of Rust it writes does: no one
        // edits it, the helpers and the tables keep the shapes the proof reads, and a lint a later
        // clippy adds does not stop a crate built with `-D warnings` (§15.340). The letters of the
        // names are a lint of the crate's (`uncommon_codepoints`), which this line allows where
        // the module is the crate; where it is a module of one, as in the runner and the WASI
        // module beside it, rustc says the line is ignored there, which `unused_attributes` keeps
        // from a build with `-D warnings`.
        let package = self.package.is_some() && !inline;
        let mut o = self.header("//");
        o.push_str(if package {
            RS_PACKAGE_ALLOW
        } else {
            "\n#![allow(non_snake_case, uncommon_codepoints, unused_attributes, unused_parens, dead_code, clippy::all)]\n\n"
        });
        // The types of an `enums` file are its module's, which the crate declares at its root
        // (`#[path = "content.rs"] mod content;`, as the runner here does), and are re-exported
        // so that a caller of this module finds them where it finds the rule's own (§15.212). In a
        // package, the module is the rules' sibling: `crate::rules::content`, under a dependency's
        // directory for a dependency's file (ritsu's DESIGN 17.8).
        if !inline {
            for (module, dir, types) in self.foreign_owners() {
                let ts: Vec<String> = types.iter().map(|(_, t)| t.clone()).collect();
                let one = if ts.len() == 1 { ts[0].clone() } else { format!("{{{}}}", ts.join(", ")) };
                let from = if package { ritsu_emit::package::rust_path(&format!("{dir}rules/{module}")) } else { format!("crate::{module}") };
                o.push_str(&format!("pub use {from}::{one};\n"));
            }
            if !self.foreign_enums().is_empty() {
                o.push('\n');
            }
        }

        // Brands: a newtype over i64, which is what the overflow proof is stated in. In a package,
        // the brands of money and of the quantities are the package's `units` (§15.248), read
        // from it and handed on, so that what one rule returns is what another rule and a book
        // take; a rate keeps its brand here.
        let mut brands: BTreeMap<String, String> = BTreeMap::new();
        for v in self.branded().into_iter().chain(self.branded_columns()) {
            // An optional amount is the same brand when it is there: the runner builds it, and
            // a rule whose only amount is optional declared none (§15.201), nor one whose only
            // amount is a column a table writes that may be absent (§15.343).
            let ty = self.ty_of(v).present().clone();
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                brands.insert(brand_of(&ty), format!("{ty}"));
            }
        }
        if package {
            let units: Vec<String> = self.brands().1.into_keys().collect();
            brands.retain(|b, _| !units.contains(b));
            if !units.is_empty() {
                let one = if units.len() == 1 { units[0].clone() } else { format!("{{{}}}", units.join(", ")) };
                o.push_str(&format!("pub use crate::units::{one};\n\n"));
            }
        }
        for (b, doc) in &brands {
            o.push_str(&format!(
                "/// {doc}\n#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]\npub struct {b}(pub i64);\n\n"
            ));
        }
        o.push_str(&format!(
            "/// {}\n#[derive(Clone, Copy, PartialEq, Eq, Debug)]\npub struct Fired {{\n    pub table: &'static str,\n    pub row: u32,\n    pub label: &'static str,\n}}\n\n",
            fired_doc()
        ));

        // Enums. Each carries its Japanese name, which is what the wire format uses (§10).
        let enums: Vec<(String, String)> = if inline {
            let mut all = self.own_enums();
            for (_, ts) in self.foreign_enums() {
                all.extend(ts);
            }
            all
        } else {
            self.own_enums()
        };
        for (jp, ascii) in enums {
            o.push_str(&self.rs_enum(&jp, &ascii));
        }
        o.push_str(&self.rs_enum_helpers());
        // Which values need which before them (§15.222). Not in the Wasm crate: its one entry has
        // no room for them.
        if !inline {
            o.push_str(&self.rs_prereqs());
        }

        // The machine this function is one step of (§15.148).
        if let Some((en, init, fins)) = self.machine_consts() {
            let cls = self.enum_names.get(&en).cloned().unwrap_or_default();
            // The values carried along with the state, and where a case starts (§15.217).
            let mut more = String::new();
            for mc in self.more_carried() {
                let v = match mc.kind() {
                    MoreKind::Enum => self.rs_value(&mc.word(), &mc.ty),
                    MoreKind::Bool => mc.bool_word().to_string(),
                    MoreKind::Number => mc.int.to_string(),
                    MoreKind::Branded => format!("{}({})", self.rs_ty(&mc.ty), mc.int),
                };
                more.push_str(&format!(
                    "/// {}\npub const {}: {} = {v};\n",
                    tr!("持ち越す {} の始まりの値。", "Where the carried {} starts.", mc.input),
                    mc.upper(),
                    self.rs_ty(&mc.ty)
                ));
            }
            o.push_str(&format!(
                "/// {}\npub const INITIAL: {cls} = {};\n/// {}\npub const FINAL: &[{cls}] = &[{}];\n{more}\n\
                 /// {}\npub fn is_final(state: {cls}) -> bool {{\n    FINAL.contains(&state)\n}}\n\n",
                tr!("案件が始まる状態。", "The state a case starts in."),
                self.rs_value(&init, &Ty::Enum(en.clone())),
                tr!("案件が終わる状態。", "The states a case ends in."),
                fins.iter().map(|v| self.rs_value(v, &Ty::Enum(en.clone()))).collect::<Vec<_>>().join(", "),
                tr!("この状態で案件が終わっているか。", "Whether a case in this state has ended.")
            ));
        }

        o.push_str(round_rs().trim_start_matches('\n'));
        o.push_str(&arith::rs_helpers(&arith::Uses::of(self.f)));
        o.push('\n');

        for g in &self.f.groups {
            let ty = self
                .c
                .groups
                .get(&g.name.text)
                .and_then(|(owner, _)| self.enum_names.get(owner).cloned())
                .unwrap_or_else(|| "i64".into());
            let ms: Vec<String> = g.members.iter().map(|m| self.rs_value(&m.text, &self.group_ty(&g.name.text))).collect();
            o.push_str(&format!(
                "pub(crate) fn is_{}(v: {ty}) -> bool {{\n    matches!(v, {})\n}}\n\n",
                self.ident(&g.name.text),
                ms.join(" | ")
            ));
        }

        o.push_str(&self.rs_fn());
        o.push('\n');
        o.push_str(&self.rs_record());
        // The function that counts the answers (§15.225).
        if self.f.has_random() {
            o.push('\n');
            o.push_str(&self.rs_chances());
        }
        o
    }

    /// One enum as Rust: the type, its wire names, its parser, and its order and display
    /// names when it has them (§15.210, §15.211). An ordered enum derives `PartialOrd` and
    /// `Ord`, which put its variants in the order they are declared.
    pub(crate) fn rs_enum(&self, jp: &str, ascii: &str) -> String {
        let Some(vals) = self.c.enums.get(jp) else { return String::new() };
        let mut o = String::new();
        let members: Vec<String> = vals
            .iter()
            .map(|v| self.value_names.get(&(jp.to_string(), v.clone())).map(|(_, a)| pascal(a)).unwrap_or_else(|| v.clone()))
            .collect();
        let ordered = self.c.ordered.contains(jp);
        // In a package of ritsu's every enum is in the order its values are declared in: a value of
        // one is a key of a dict or a member of a set of kaname's code there (kaname's DESIGN 24.1),
        // which Rust keeps in that order. `rank` stays the ordered enums' alone.
        let derive = if ordered || self.package.is_some() { "Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug" } else { "Clone, Copy, PartialEq, Eq, Debug" };
        o.push_str(&format!(
            "#[derive({derive})]\npub enum {ascii} {{\n{}}}\n\n",
            members.iter().map(|m| format!("    {m},\n")).collect::<String>()
        ));
        o.push_str(&format!("impl {ascii} {{\n    pub fn as_str(self) -> &'static str {{\n        match self {{\n"));
        for (m, v) in members.iter().zip(vals) {
            o.push_str(&format!("            {ascii}::{m} => {v:?},\n"));
        }
        o.push_str("        }\n    }\n\n");
        o.push_str(&format!("    pub fn parse(s: &str) -> Option<{ascii}> {{\n        match s {{\n"));
        for (m, v) in members.iter().zip(vals) {
            o.push_str(&format!("            {v:?} => Some({ascii}::{m}),\n"));
        }
        o.push_str("            _ => None,\n        }\n    }\n");
        if ordered {
            o.push_str(&format!(
                "\n    /// {}\n    pub fn rank(self) -> i64 {{\n        self as i64\n    }}\n",
                tr!("宣言の順序の何番目か（0 から）。", "The place in the order declared, counting from 0.")
            ));
        }
        if let Some(labels) = self.c.labels.get(jp) {
            o.push_str(&format!(
                "\n    /// {}\n    pub fn label(self) -> &'static str {{\n        match self {{\n",
                tr!("表示名。表示名の無い値は値の名前。", "The display name: a value without one is shown by its name.")
            ));
            for (i, (m, v)) in members.iter().zip(vals).enumerate() {
                let l = labels.get(i).cloned().flatten().unwrap_or_else(|| v.clone());
                o.push_str(&format!("            {ascii}::{m} => {},\n", str_lit(&l)));
            }
            o.push_str("        }\n    }\n");
        }
        o.push_str("}\n\n");
        o
    }

    /// The helpers the functions call for the expressions over an ordered enum (§15.210).
    fn rs_enum_helpers(&self) -> String {
        let mut o = String::new();
        for en in self.c.enum_order.iter().filter(|e| self.c.ordered.contains(*e)) {
            let (rank, max, min) = self.enum_helpers(en);
            let t = self.enum_type(en);
            if rank {
                o.push_str(&format!("pub(crate) fn _rank_{t}(x: {t}) -> i64 {{\n    x.rank()\n}}\n\n"));
            }
            if max {
                o.push_str(&format!("pub(crate) fn _max_{t}(a: {t}, b: {t}) -> {t} {{\n    if a.rank() >= b.rank() {{ a }} else {{ b }}\n}}\n\n"));
            }
            if min {
                o.push_str(&format!("pub(crate) fn _min_{t}(a: {t}, b: {t}) -> {t} {{\n    if a.rank() <= b.rank() {{ a }} else {{ b }}\n}}\n\n"));
            }
        }
        o
    }

    /// The roots of a crate that holds this module declare the modules of the `enums` files
    /// it reads its types from, beside it (§15.212).
    pub(crate) fn rs_owner_mods(&self) -> String {
        self.foreign_enums().iter().map(|(m, _)| format!("#[path = \"{m}.rs\"]\nmod {m};\n")).collect()
    }

    /// The rule's items as Rust, at the base indentation (§15.56).
    fn rs_items(&self, local: &dyn Fn(&str) -> String, trace: &str, phase: Phase) -> String {
        let mut o = String::new();
        for (i, it) in self.f.items.iter().enumerate() {
            if !self.in_phase(i, it, phase) {
                continue;
            }
            match it {
                Item::Derived(d) => {
                    o.push_str(&format!(
                        "    let {} = {}; // {}\n",
                        self.ident(&d.name.text),
                        rs_expr(unparen(&self.expr(&d.expr, local).text)),
                        tr!("導出", "derived value")
                    ));
                    o.push_str(&self.rs_unread(&d.name.text));
                }
                Item::Define(d) => {
                    o.push_str(&format!(
                        "    let {} = {}; // {}\n",
                        self.ident(&d.name.text),
                        rs_expr(unparen(&self.expr(&d.expr, local).text)),
                        tr!("定義", "definition")
                    ));
                    // The range the rule accepts for it, held where it is worked out (§15.313).
                    let n = self.ident(&d.name.text);
                    match self.accepted_ends(d).and_then(|ends| or_out_of(&n, ends, "")) {
                        Some(test) => o.push_str(&format!(
                            "    if {test} {{\n        return Err(RuleError::Input {{ what: {}, value: Some({n}) }});\n    }}\n",
                            rs_said(&crate::door::out_of_range(&self.said_name(&d.name.text)), &self.here(), "")
                        )),
                        // What the check does not read either (§15.340).
                        None => o.push_str(&self.rs_unread(&d.name.text)),
                    }
                }
                Item::Agg(_) | Item::Fold(_) => {}
                Item::Table(t) => {
                    if let Some(t) = self.c.table_at(t) {
                        o.push_str(&self.rs_table(t, local, trace));
                    }
                }
            }
        }
        o
    }

    /// Every walk of a rule that walks sequences, in Rust (§15.56, §15.58, §15.219). The shape is
    /// the one `py_walks` writes.
    fn rs_walks(&self, outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = String::new();
        for (s, el) in self.f.elements.iter().enumerate() {
            if let Some(cap) = self.count_cap_of(s) {
                let seq = self.held(&el.name);
                o.push_str(&format!(
                    "    if {seq}.len() > {cap} {{\n        return Err(RuleError::Input {{ what: Said::of(\"{}\"), value: Some({seq}.len() as i64) }});\n    }}\n",
                    crate::door::too_many(&el.name.text, cap)
                ));
            }
        }
        for step in self.walk_steps() {
            match step {
                // What a walk does not depend on is worked out once, before it (§15.208).
                WalkStep::Once(ix) => o.push_str(&self.rs_items(outer, trace, Phase::Only(&ix))),
                WalkStep::Walk { seq, body, sums, .. } => o.push_str(&self.rs_walk(seq, &body, &sums, outer, trace)),
            }
        }
        o
    }

    /// One walk over sequence `s`, in Rust (§15.219). A unit is a newtype here, so a field read in
    /// an expression is unwrapped exactly as an input is (§8.1).
    fn rs_walk(&self, s: usize, body_items: &[usize], sums: &[usize], outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let el = &self.f.elements[s];
        let seq = self.held(&el.name);
        let fields: BTreeMap<String, String> = el.fields.iter().map(|fd| (fd.name.text.clone(), pub_name(&fd.name))).collect();
        let e = self.elem_var(s);
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => {
                    let v = format!("{e}.{f}");
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                        format!("{v}.0")
                    } else {
                        v
                    }
                }
                None => outer(n),
            }
        };
        let mut o = String::new();
        let index = self.index_var(s);
        self.walk_begin(s, &index);
        let mut body = self.rs_element_guards(s, &local);
        let (seen, step) = self.rs_unique(s, sums, &local);
        o.push_str(&seen);
        body.push_str(&step);
        // Past a `stop`, the door alone (§15.328).
        if let Some(i) = self.walk_goes_past_stop(sums) {
            body.push_str(&format!("    if {} {{\n        continue;\n    }}\n", self.fold_names(i).done));
        }
        body.push_str(&self.rs_items(&local, trace, Phase::Only(body_items)));
        let (mut after, mut unread) = (String::new(), String::new());
        for &i in sums {
            match &self.f.items[i] {
                Item::Agg(d) if d.kind.extreme() => {
                    let (init, step) = self.rs_extreme(d, &local);
                    o.push_str(&init);
                    body.push_str(&step);
                }
                Item::Agg(d) => {
                    o.push_str(&format!("    let mut {}: i64 = 0; // {}\n", self.ident(&d.name.text), self.agg_word(d)));
                    let v = local(&d.column.text);
                    if d.kind == crate::ast::AggKind::Sum {
                        body.push_str(&format!("    {} += {v}; // {}\n", self.ident(&d.name.text), d.name.text));
                    } else {
                        let test = self.rs_test(&v, &d.column.text, &d.value);
                        body.push_str(&format!("    if {test} {{ // {}\n        {} += 1;\n    }}\n", d.name.text, self.ident(&d.name.text)));
                    }
                    let mut checked = false;
                    if let Some((_, cap)) = self.sum_caps().into_iter().find(|(x, _)| x.name.text == d.name.text) {
                        let n = self.ident(&d.name.text);
                        after.push_str(&format!(
                            "    if {n} > {cap} {{\n        return Err(RuleError::Input {{ what: Said::of(\"{}\"), value: Some({n}) }});\n    }}\n",
                            crate::door::out_of_range(&d.name.text)
                        ));
                        checked = true;
                    }
                    // The bottom of its range, where a walk can come in under it (§15.314).
                    if let Some(floor) = self.walk_floor(d) {
                        let n = self.ident(&d.name.text);
                        after.push_str(&format!(
                            "    if {n} < {floor} {{\n        return Err(RuleError::Input {{ what: Said::of(\"{}\"), value: Some({n}) }});\n    }}\n",
                            crate::door::out_of_range(&d.name.text)
                        ));
                        checked = true;
                    }
                    // rustc does not count `+=` as a read (§15.340).
                    if !checked {
                        unread.push_str(&self.rs_unread(&d.name.text));
                    }
                }
                Item::Fold(fold) => {
                    let (init, step, fin) = self.rs_fold(i, fold, &seq, &local);
                    o.push_str(&init);
                    body.push_str(&step);
                    after.push_str(&fin);
                }
                _ => {}
            }
        }
        // The index of the element is bound where a sentence of the walk says it (§15.324). An
        // element no line of the walk reads is said to be left unread on purpose: rustc warns of
        // a local nothing reads (§15.340).
        if !clash::reads_word(&body, &e, clash::Syntax::Rust) {
            body.insert_str(0, &format!("    let _ = {e};\n"));
        }
        if self.walk_end() {
            o.push_str(&format!("    for ({index}, {e}) in {seq}.iter().enumerate() {{\n"));
        } else {
            o.push_str(&format!("    for {e} in {seq} {{\n"));
        }
        o.push_str(&Self::indent_block(&body, "    "));
        o.push_str("    }\n");
        o.push_str(&after);
        o.push_str(&unread);
        o
    }

    /// A test on a column of one element, in Rust: `count`'s, and the `where` of `max` and `min`.
    fn rs_test(&self, v: &str, col: &str, value: &Option<crate::ast::Name>) -> String {
        let ty = self.ty_of(col);
        match (&ty, value) {
            (Ty::Bool, Some(w)) if w.text == crate::kw::FALSE => format!("!{v}"),
            (Ty::Bool, _) => v.to_string(),
            (_, Some(w)) => {
                let cls = self.rs_ty(&ty);
                let member = self.value_name(&w.text, &ty).map(|(_, a)| a.clone()).unwrap_or_else(|| w.text.clone());
                format!("{v} == {cls}::{member}")
            }
            (_, None) => v.to_string(),
        }
    }

    /// A `max` or a `min`, in Rust (§15.219).
    fn rs_extreme(&self, d: &crate::ast::AggDecl, local: &dyn Fn(&str) -> String) -> (String, String) {
        let n = self.ident(&d.name.text);
        let set = self.temp(&format!("{n}_set"));
        let v = local(&d.column.text);
        let ty = self.ty_of(&d.column.text);
        let op = if d.kind == crate::ast::AggKind::Max { ">" } else { "<" };
        let (rs_t, start, better) = match self.extreme_enum(d) {
            Some(en) => {
                let t = self.enum_type(&en);
                let w = self.extreme_empty_word(d).unwrap_or_default();
                (self.rs_ty(&ty), self.rs_value(&w, &ty), format!("_rank_{t}({v}) {op} _rank_{t}({n})"))
            }
            None => ("i64".to_string(), self.extreme_empty_int(d).unwrap_or_else(|| "0".into()), format!("{v} {op} {n}")),
        };
        let init = format!("    let mut {n}: {rs_t} = {start}; // {}\n    let mut {set} = false;\n", self.agg_word(d));
        let mut step = String::new();
        let pad = match &d.filter {
            Some((w, want)) => {
                step.push_str(&format!("    if {} {{ // {}\n", self.rs_test(&local(&w.text), &w.text, want), d.name.text));
                "    "
            }
            None => "",
        };
        step.push_str(&format!("{pad}    if !{set} || {better} {{\n{pad}        {n} = {v};\n{pad}        {set} = true;\n{pad}    }}\n"));
        if !pad.is_empty() {
            step.push_str("    }\n");
        }
        (init, step)
    }

    /// A fold, in Rust (§15.56).
    fn rs_fold(&self, i: usize, fold: &crate::ast::FoldDecl, seq: &str, local: &dyn Fn(&str) -> String) -> (String, String, String) {
        let names = self.fold_names(i);
        let FoldNames { answer, stopped, done, taken, kept, best, held, key, .. } = &names;
        let alone = self.fold_alone(i);
        // Ends the loop at a `stop`: a fold alone in its walk with no door to keep past it (§15.328).
        let ends = alone && !self.stop_keeps_door(i);
        let with_held = |n: &str| -> String { if n == crate::kw::HELD { held.clone() } else { local(n) } };
        let (out_name, out_ty) = self.fold_target(fold);
        let acc_ty = if out_ty.is_numeric() { "i64".to_string() } else { self.rs_ty(&out_ty) };
        // An answer is read in the type the fold answers with: a value of its enum is that
        // enum's member (§15.208).
        let val = |x: &Expr, l: &dyn Fn(&str) -> String| -> String {
            match x {
                Expr::Lit(Lit::Word(w), _) if w != crate::kw::TRUE && w != crate::kw::FALSE => self.rs_value(w, &out_ty),
                _ => rs_expr(unparen(&self.expr(x, l).text)).to_string(),
            }
        };
        let text = |x: &Option<Expr>| -> String { x.as_ref().map(|x| val(x, &with_held)).unwrap_or_else(|| "0".to_string()) };
        let weighted = fold.weighted();
        let draw = fold.draw.as_ref().map(|d| local(&d.text)).unwrap_or_default();

        // A flag or a value no arm of the fold sets is not `mut`, and the key of a `keep_max` is
        // declared only with one: rustc says so of each, and a crate built with `-D warnings`
        // stops there (§15.340).
        let has = |f: &dyn Fn(&Arm) -> bool| fold.arms.iter().any(|(_, a, _)| f(a));
        let mutable = |set: bool| if set { "mut " } else { "" };
        let mut init = String::new();
        init.push_str(&format!("    let mut {answer}: {acc_ty} = {}; // {}\n", text(&fold.empty), tr!("要素ゼロ件の答え", "the answer for no elements")));
        init.push_str(&format!("    let {}{stopped} = false;\n", mutable(has(&|a| matches!(a, Arm::Stop(Some(_)))))));
        if !ends {
            init.push_str(&format!("    let {}{done} = false;\n", mutable(has(&|a| matches!(a, Arm::Stop(_))))));
        }
        init.push_str(&format!("    let {}{taken}: Option<{acc_ty}> = None;\n", mutable(has(&|a| matches!(a, Arm::Take { .. } | Arm::TakeWeighted { .. })))));
        let keeps = has(&|a| matches!(a, Arm::KeepMax { .. }));
        init.push_str(&format!("    let {}{kept}: Option<{acc_ty}> = None;\n", mutable(keeps)));
        if keeps {
            init.push_str(&format!("    let mut {best}: Option<i64> = None;\n"));
        }
        // The element a `take_unique` took, which the sentence says beside the one that matched
        // again (§15.324).
        if fold.arms.iter().any(|(_, a, _)| matches!(a, Arm::Take { unique: true, .. })) {
            init.push_str(&format!("    let mut {}: usize = 0;\n", names.taken_at));
        }
        if weighted {
            init.push_str(&format!("    let mut {}: i64 = 0; // {}\n", names.total, tr!("重みの合計", "the running total of the weights")));
            init.push_str(&format!("    let mut {}: Vec<i64> = Vec::new();\n", names.cuts));
        }

        let mut arms = String::new();
        let v = local(&fold.verdict);
        for (name, arm, _) in &fold.arms {
            let cls = self.rs_ty(&self.ty_of(&fold.verdict));
            let member = self.value_name(&name.text, &self.ty_of(&fold.verdict)).map(|(_, a)| a.clone()).unwrap_or_else(|| name.text.clone());
            arms.push_str(&format!("    if {v} == {cls}::{member} {{ // {}\n", name.text));
            match arm {
                Arm::Next => arms.push_str("        // next\n"),
                Arm::Stop(None) if ends => arms.push_str("        break;\n"),
                Arm::Stop(None) => arms.push_str(&format!("        {done} = true;\n")),
                Arm::Stop(Some(x)) => {
                    arms.push_str(&format!("        {answer} = {};\n", val(x, local)));
                    if ends {
                        arms.push_str(&format!("        {stopped} = true;\n        break;\n"));
                    } else {
                        arms.push_str(&format!("        {stopped} = true;\n        {done} = true;\n"));
                    }
                }
                Arm::Take { expr, unique } => {
                    if *unique {
                        let here = self.here_used();
                        arms.push_str(&format!("        if {taken}.is_some() {{\n"));
                        arms.push_str(&format!(
                            "            return Err(RuleError::Contradiction {{ what: {} }});\n        }}\n",
                            rs_said(&crate::door::fold_contradiction(&fold.verdict, &fold.over, crate::door::BEFORE, crate::door::HERE), &here, &names.taken_at)
                        ));
                        arms.push_str(&format!("        {taken} = Some({});\n        {} = {here};\n", val(expr, local), names.taken_at));
                    } else {
                        arms.push_str(&format!("        if {taken}.is_none() {{\n"));
                        arms.push_str(&format!("            {taken} = Some({});\n        }}\n", val(expr, local)));
                    }
                }
                Arm::KeepMax { expr, key: k } => {
                    arms.push_str(&format!("        let {key} = {};\n", rs_expr(unparen(&self.expr(k, local).text))));
                    arms.push_str(&format!("        if {best}.is_none() || {key} > {best}.unwrap() {{\n"));
                    arms.push_str(&format!("            {best} = Some({key});\n"));
                    arms.push_str(&format!("            {kept} = Some({});\n        }}\n", val(expr, local)));
                }
                // The weight goes on the running total, and the element is taken where the drawn
                // value is below the total for the first time (§15.226). A weight that would take
                // the total past int64 is refused before it goes on, and before another element is
                // read.
                Arm::TakeWeighted { expr, weight } => {
                    let (w, t, c) = (&names.weight, &names.total, &names.cuts);
                    arms.push_str(&format!("        let {w}: i64 = {};\n", rs_expr(unparen(&self.expr(weight, local).text))));
                    arms.push_str(&format!(
                        "        if {w} > i64::MAX - {t} {{\n            return Err(RuleError::Input {{ what: Said::of(\"{}\"), value: None }});\n        }}\n",
                        crate::door::weights_past(&fold.over)
                    ));
                    arms.push_str(&format!("        {t} += {w};\n        {c}.push({t});\n"));
                    arms.push_str(&format!("        if {taken}.is_none() && {draw} < {t} {{\n"));
                    arms.push_str(&format!("            {taken} = Some({});\n        }}\n", val(expr, local)));
                }
            }
            arms.push_str("    }\n");
        }
        let step = if alone { arms } else { format!("    if !{done} {{ // {}\n{}    }}\n", fold.verdict, Self::indent_block(&arms, "    ")) };

        // The drawn value of a pick by weight is below the total of the weights, unless they add up
        // to 0 (§15.226).
        let mut fin = String::new();
        if weighted {
            let t = &names.total;
            let what = fold.draw.as_ref().map(|d| crate::door::draw_past(&d.text)).unwrap_or_default();
            fin.push_str(&format!("    if {t} > 0 && {draw} >= {t} {{\n        return Err(RuleError::Input {{ what: Said::of(\"{what}\"), value: Some({draw}) }});\n    }}\n"));
        }
        fin.push_str(&format!("    if !{seq}.is_empty() && !{stopped} {{\n"));
        fin.push_str(&format!("        let {held} = {taken}.or({kept}).unwrap_or({});\n", text(&fold.empty)));
        // An `exhausted` that does not read what was held, as Go's (§15.340).
        let ex = text(&fold.exhausted);
        if !clash::has_word(&ex, held) {
            fin.push_str(&format!("        let _ = {held};\n"));
        }
        fin.push_str(&format!("        {answer} = {ex};\n    }}\n"));
        fin.push_str(&format!("    let {} = {answer};\n", self.ident(&out_name)));
        fin.push_str(&self.rs_unread(&out_name));
        (init, step, fin)
    }

    fn rs_element_guards(&self, s: usize, local: &dyn Fn(&str) -> String) -> String {
        let mut o = String::new();
        for i in self.element_fields(s) {
            let v = local(&i.name.text);
            let ty = self.ty_of(&i.name.text);
            // An optional input is guarded as the type it wraps whenever it has a value (§15.201).
            let optional = matches!(ty, Ty::Opt(_));
            let ty = ty.present().clone();
            let inside = if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) { "__v.0" } else { "__v" };
            let (v, wrap) = if optional {
                (inside.to_string(), Some((format!("    if let Some(__v) = {v} {{\n"), "    }\n".to_string())))
            } else {
                (v, None)
            };
            let mut o = Guarded::new(&mut o, wrap, "    ");
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                if let Some(test) = or_out_of(&v, crate::door::ends(self.c, &i.name.text, false), "") {
                    // The sentence names the field by its place on the element (§15.324).
                    o.push_str(&format!(
                        "    if {test} {{\n        return Err(RuleError::Input {{ what: {}, value: Some({v}) }});\n    }}\n",
                        rs_said(&crate::door::out_of_range(&self.said_name(&i.name.text)), &self.here(), "")
                    ));
                }
            }
        }
        o
    }

    /// The fields marked `unique` (§15.290), in Rust: a list of the values met for each before the
    /// first walk over the sequence, and in the walk, after the guards of one element, the refusal
    /// of a value an element before it had. A list and `contains`, as an enum here is not `Hash`.
    fn rs_unique(&self, s: usize, sums: &[usize], local: &dyn Fn(&str) -> String) -> (String, String) {
        let seq = &self.f.elements[s].name.text;
        let (mut init, mut step) = (String::new(), String::new());
        for fd in self.unique_fields(s, sums) {
            let seen = self.temp(&format!("{}_seen", self.ident(&fd.name.text)));
            let v = local(&fd.name.text);
            let ty = self.ty_of(&fd.name.text);
            let (t, kept) = match &ty {
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => ("i64".to_string(), v.clone()),
                Ty::Str => ("String".to_string(), format!("{v}.clone()")),
                other => (self.rs_ty(other), v.clone()),
            };
            // The list holds one value per element, at the element's index: where a value was first
            // seen is the sentence's earlier element (§15.324).
            let here = self.here_used();
            let first = self.temp("first");
            init.push_str(&format!("    let mut {seen}: Vec<{t}> = Vec::new();\n"));
            step.push_str(&format!(
                "    if let Some({first}) = {seen}.iter().position(|x| *x == {v}) {{\n        return Err(RuleError::Input {{ what: {}, value: None }});\n    }}\n    {seen}.push({kept});\n",
                rs_said(&crate::door::duplicate(seq, &fd.name.text, crate::door::BEFORE, crate::door::HERE), &here, &first)
            ));
        }
        (init, step)
    }

    fn rs_fn(&self) -> String {
        let fname = pub_name(&self.f.name);
        let outs = &self.f.outputs;
        let mut o = String::new();

        // One element is a row of inputs, so it is a struct like `Output`.
        for (s, el) in self.f.elements.iter().enumerate() {
            o.push_str(&format!("/// {}\n", tr!("{} の一件", "one of {}", el.name.text)));
            o.push_str(&format!(
                "{}\npub struct {} {{\n",
                self.rs_derive(el.fields.iter().map(|fd| fd.name.text.clone())),
                self.element_type(s)
            ));
            for fd in &el.fields {
                o.push_str(&format!(
                    "    pub {}: {},\n",
                    pub_name(&fd.name),
                    self.rs_ty(&self.ty_of(&fd.name.text))
                ));
            }
            o.push_str("}\n\n");
        }

        if outs.len() > 1 {
            o.push_str(&format!(
                "{}\npub struct Output {{\n",
                self.rs_derive(outs.iter().map(|od| od.name.text.clone()))
            ));
            for od in outs {
                o.push_str(&format!(
                    "    pub {}: {},\n",
                    pub_name(&od.name),
                    self.rs_ty(&self.ty_of(&od.name.text))
                ));
            }
            o.push_str("}\n\n");
        }

        let ret = if outs.len() == 1 {
            self.rs_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", self.held(&i.name), self.rs_ty(&self.ty_of(&i.name.text))))
            .collect();
        for (s, el) in self.f.elements.iter().enumerate() {
            params.push(format!("{}: &[{}]", self.held(&el.name), self.element_type(s)));
        }

        // The public function keeps the plain signature; the branches live in its traced
        // twin, which also returns the rows that matched (§15.33).
        let mut args: Vec<String> = self.f.inputs.iter().map(|i| self.held(&i.name)).collect();
        for el in &self.f.elements {
            args.push(self.held(&el.name));
        }
        let traced = format!("{fname}_traced");
        o.push_str(&format!("/// {}\n", plain_doc(&self.f.name.text, &self.f.version, &format!("`{traced}`"))));
        o.push_str(&format!(
            "pub fn {}({}) -> Result<{ret}, RuleError> {{\n    {traced}({}).map(|(out, _)| out)\n}}\n\n",
            self.fam(&fname),
            params.join(", "),
            args.join(", ")
        ));
        o.push_str(&format!("/// {}\n", traced_doc(&self.f.name.text, &self.f.version)));
        o.push_str(&format!(
            "pub fn {traced}({}) -> Result<({ret}, Vec<Fired>), RuleError> {{\n",
            params.join(", ")
        ));
        // A rule that picks by weight runs its walk in a function of its own, which gives back the
        // running totals of the weights too (§15.225).
        let walked = self.weighted_fold().map(|(i, _)| (self.walked_fn("rust"), self.fold_names(i).cuts));
        if let Some((w, _)) = &walked {
            o.push_str(&format!("    {w}({}).map(|(out, trace, _)| (out, trace))\n}}\n\n", args.join(", ")));
            o.push_str(&format!("/// {}\nfn {w}({}) -> Result<({ret}, Vec<Fired>, Vec<i64>), RuleError> {{\n", walked_doc(), params.join(", ")));
        }
        // Where the body that reads the inputs begins.
        let body_at = o.len();

        // A branded input is an i64 inside; unwrap it once, where it is read.
        let local = |n: &str| -> String {
            match self.f.inputs.iter().find(|i| i.name.text == n) {
                Some(i) => {
                    let v = self.held(&i.name);
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                        format!("{v}.0")
                    } else {
                        v
                    }
                }
                None => self.ident(n),
            }
        };
        // Entry guards. An enum needs none: in Rust a value of an enum type is one of its
        // variants by construction, so the check the other three languages have to make at
        // run time is already made by the compiler.
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            // An optional input is guarded as the type it wraps whenever it has a value (§15.201).
            let optional = matches!(ty, Ty::Opt(_));
            let ty = ty.present().clone();
            // An ordered enum held to the values its range lets in, first to last (§15.210).
            if let Some((lo, hi)) = self.c.enum_ranges.get(&i.name.text).and_then(|vs| Some((vs.first()?, vs.last()?))) {
                let v = local(&i.name.text);
                let (v, wrap) = if optional {
                    ("__v".to_string(), Some((format!("    if let Some(__v) = {v} {{\n"), "    }\n".to_string())))
                } else {
                    (v, None)
                };
                let mut o = Guarded::new(&mut o, wrap, "    ");
                o.push_str(&format!(
                    "    if {v} < {} || {v} > {} {{\n        return Err(RuleError::Input {{ what: Said::of(\"{}\"), value: None }});\n    }}\n",
                    self.rs_value(lo, &ty),
                    self.rs_value(hi, &ty),
                    crate::door::out_of_range(&i.name.text)
                ));
                continue;
            }
            if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                continue;
            }
            let v = local(&i.name.text);
            // A branded value is an i64 inside, which is what the guard compares.
            let inside = if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) { "__v.0" } else { "__v" };
            let (v, wrap) = if optional {
                (inside.to_string(), Some((format!("    if let Some(__v) = {v} {{\n"), "    }\n".to_string())))
            } else {
                (v, None)
            };
            // Each end the range writes, and −(2^63−1) where it writes no lower one: an i64
            // holds one value below that, and the door refuses it (§15.295).
            let Some(test) = or_out_of(&v, crate::door::ends(self.c, &i.name.text, false), "") else {
                continue;
            };
            let mut o = Guarded::new(&mut o, wrap, "    ");
            o.push_str(&format!(
                "    if {test} {{\n        return Err(RuleError::Input {{ what: Said::of(\"{}\"), value: Some({v}) }});\n    }}\n",
                crate::door::out_of_range(&i.name.text)
            ));
        }

        o.push_str(&self.constraint_guards(&local, Lang::Rs, "    ", |m| {
            format!("        return Err(RuleError::Input {{ what: Said::of(\"{m}\"), value: None }});\n")
        }));

        let trace = self.temp("trace");
        let has_table = self.f.items.iter().any(|i| matches!(i, Item::Table(_)));
        o.push_str(&format!("    let {}{trace}: Vec<Fired> = Vec::new();\n", if has_table { "mut " } else { "" }));

        if self.f.walks() {
            o.push_str(&self.rs_walks(&local, &trace));
        } else {
            o.push_str(&self.rs_items(&local, &trace, Phase::All));
        }

        let wrap = |ty: &Ty, body: String| -> String {
            match ty {
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => format!("{}({body})", self.rs_ty(ty)),
                _ => body,
            }
        };
        // Every output takes the same three steps: its source — the `result` expression for
        // the first output, the binding of its own name otherwise — brought to the wire
        // scale, then its declared rounding applied exactly once (§15.16).
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let ty = self.ty_of(out_name);
            // An optional output is its column's `Option`, taken through the output's steps only
            // when it is there (§15.249). An enum, a truth value, a string and a date take no step.
            let opt = self.opt_out(oi);
            if opt && !ty.present().is_numeric() {
                finals.push(local(out_name));
                continue;
            }
            // The value inside the `Option`: the brand's integer, or the integer a `number` is.
            let inside = if matches!(ty.present(), Ty::Number) { "__v" } else { "__v.0" };
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ if opt => Expr2 { text: inside.into(), scale: self.scale(out_name) },
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let res = self.onto_wire(res, os, od);
            let body = match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    if res.scale != os && !opt {
                        let raw = self.temp(&raw_base(oi));
                        o.push_str(&format!(
                            "    let {raw} = {}; // {}\n",
                            rs_expr(unparen(&res.text)),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("round_{}({raw}, {grid_i}) / {}", mode_fn(m), res.scale / os)
                    } else if res.scale != os {
                        format!("round_{}({}, {grid_i}) / {}", mode_fn(m), rs_expr(unparen(&res.text)), res.scale / os)
                    } else {
                        format!("round_{}({}, {grid_i})", mode_fn(m), rs_expr(&res.text))
                    }
                }
                None => rs_expr(&res.text),
            };
            if opt {
                finals.push(format!("{}.map(|__v| {})", local(out_name), wrap(ty.present(), body)));
            } else {
                finals.push(wrap(&ty, body));
            }
        }
        if outs.len() == 1 {
            let cuts = walked.as_ref().map(|(_, c)| format!(", {c}")).unwrap_or_default();
            o.push_str(&format!("    Ok(({}, {trace}{cuts}))\n}}\n", finals[0]));
        } else {
            let fields: Vec<String> = outs
                .iter()
                .zip(&finals)
                .map(|(od, body)| format!("{}: {body}", pub_name(&od.name)))
                .collect();
            let cuts = walked.as_ref().map(|(_, c)| format!(", {c}")).unwrap_or_default();
            o.push_str(&format!("    Ok((Output {{ {} }}, {trace}{cuts}))\n}}\n", fields.join(", ")));
        }
        // An input or a sequence nothing in the body reads: no table names it, and no check of
        // the door does (§15.340).
        let unread: String = self
            .f
            .inputs
            .iter()
            .map(|i| &i.name)
            .chain(self.f.elements.iter().map(|el| &el.name))
            .map(|n| self.held(n))
            .filter(|h| !clash::reads_word(&o[body_at..], h, clash::Syntax::Rust))
            .map(|h| format!("    let _ = {h};\n"))
            .collect();
        o.insert_str(body_at, &unread);
        o
    }

    fn rs_table(&self, t: &Table, local: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = format!("    // {}\n", self.table_head(t));
        for oc in &t.outputs {
            let ty = self.ty_of(&oc.name.text);
            let decl = match ty {
                Ty::Bool => "bool".to_string(),
                Ty::Str => "String".to_string(),
                Ty::Enum(_) => self.rs_ty(&ty),
                // An optional column is the `Option` an optional input is, of the brand
                // (§15.249): an input that may be absent passes on as it stands.
                Ty::Opt(_) => self.rs_ty(&ty),
                _ => "i64".to_string(),
            };
            o.push_str(&format!("    let {}: {decl};\n", self.ident(&oc.name.text)));
        }
        for (ri, row) in t.rows.iter().enumerate() {
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.rs_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            let cond = if conds.is_empty() { "true".into() } else { conds.join(" && ") };
            let cells: Vec<String> = row.cells.iter().map(cell_src).chain(row.outs.iter().map(out_src)).collect();
            let line = self.row_head(t, ri, &cells.join(" | "));
            let head = if ri == 0 { "    if" } else { " else if" };
            if ri == 0 {
                o.push_str(&format!("{head} {cond} {{ // {line}\n"));
            } else {
                o.push_str(&format!("    }}{head} {cond} {{ // {line}\n"));
            }
            for (oi, oc) in t.outputs.iter().enumerate() {
                let col_ty = self.ty_of(&oc.name.text);
                // A value written into an optional column is `Some` of the brand; `none` is `None`
                // (§15.249).
                let some = |v: String| -> String {
                    match &col_ty {
                        Ty::Opt(inner) if matches!(inner.as_ref(), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) => {
                            format!("Some({}({v}))", self.rs_ty(inner))
                        }
                        Ty::Opt(_) => format!("Some({v})"),
                        _ => v,
                    }
                };
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => some(self.int_lit(n, &col_ty, self.scale(&oc.name.text))),
                    Some(OutCell::Lit(l)) => some(match l {
                        Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                        Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                        Lit::Word(w) => self.rs_value(w, &col_ty),
                        Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                        // Rust's string output is a `String`, so the literal is owned here
                        // rather than at every use.
                        Lit::Str(x) => format!("{}.to_string()", str_lit(x)),
                        _ => "0".into(),
                    }),
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            some("true".into())
                        } else if w == crate::kw::FALSE {
                            some("false".into())
                        } else if self.is_value(w) {
                            some(self.rs_value(w, &col_ty))
                        } else if let (Ty::Opt(inner), Ty::Opt(_)) = (&col_ty, self.ty_of(w)) {
                            // A value that may be absent passes on as the `Option` it is, and is
                            // scaled only when it is there.
                            let v = local(w);
                            let branded = matches!(inner.as_ref(), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate);
                            let raw = if branded { "__v.0" } else { "__v" };
                            let scaled = self.rescaled(w, &oc.name.text, raw.to_string());
                            if scaled == raw {
                                v
                            } else if branded {
                                format!("{v}.map(|__v| {}({}))", self.rs_ty(inner), rs_expr(&scaled))
                            } else {
                                format!("{v}.map(|__v| {})", rs_expr(&scaled))
                            }
                        } else {
                            some(rs_expr(&self.rescaled(w, &oc.name.text, local(w))))
                        }
                    }
                    Some(OutCell::Nothing) => "None".into(),
                    None => "0".into(),
                };
                o.push_str(&format!("        {} = {v};\n", self.ident(&oc.name.text)));
            }
            let (tn, rn, lbl) = self.fired_of(t, ri);
            o.push_str(&format!("        {trace}.push(Fired {{ table: {tn:?}, row: {rn}, label: {lbl:?} }});\n"));
        }
        o.push_str(&format!(
            "    }} else {{\n        unreachable!(\"{}\");\n    }}\n",
            tr!("到達不能: 完全性は rulec が静的に検査済み", "unreachable: completeness was statically checked by rulec")
        ));
        for oc in &t.outputs {
            o.push_str(&self.rs_unread(&oc.name.text));
        }
        o.push_str(&self.guards(t, local, Lang::Rs, "    ", |name, i, j| {
            format!(
                "        return Err(RuleError::Contradiction {{ what: Said::of(\"{}\") }});\n",
                tr!("表 {name}: {i} と {j} が同時に当てはまりました", "table {name}: {i} and {j} matched at the same time")
            )
        }));
        o
    }

    /// The runner. Rust has no JSON in its standard library, and the generated code takes no
    /// dependencies, so the reader below is written out here: the wire format is one flat
    /// object of numbers, strings and booleans (§10.2), which is small enough to scan.
    /// The proof harnesses, for the Kani Rust Verifier (§15.95). A file of its own: a person
    /// reading the rule should not have to read the scaffolding, and `rustc` never compiles
    /// it. `kani <alias>_proof.rs` — or `rulec test`, when kani is on PATH.
    ///
    /// Three of the five things `check` proves are already written into the generated Rust
    /// as assertions, so the harness only has to reach them: the `unreachable!` that closes
    /// every table is completeness, the guard that returns `Contradiction` is the W114 pair,
    /// and every arithmetic operation is the int64 claim, which Kani checks by default. The
    /// fourth, the overlap, is *not* in the artifact — the if/else chain has already settled
    /// the priority — so the rows are counted here instead.
    /// The proofs read the module's names through `use super::r::*`, where an enum's type hides one
    /// of the prelude's (§15.336).
    pub fn rs_proof(&self) -> String {
        self.apart_types(clash::Syntax::Rust, |g| g.rs_proof_text())
    }

    fn rs_proof_text(&self) -> String {
        let alias = pub_name(&self.f.name);
        let mut o = self.header("//");
        o.push_str(&format!(
            "//\n// {}\n//\n// {}\n//   * {}\n//   * {}\n//   * {}\n//   * {}\n//\n// {}\n\n",
            tr!(
                "Kani（https://model-checking.github.io/kani/）の証明ハーネス。`kani {alias}_proof.rs`、または `rulec test --proofs`。すべて `#[cfg(kani)]` の中なので rustc は読まない。",
                "Proof harnesses for Kani (https://model-checking.github.io/kani/): `kani {alias}_proof.rs`, or `rulec test --proofs`. Everything is behind `#[cfg(kani)]`, so rustc never reads it."
            ),
            tr!("宣言した範囲のすべての入力について、次を検査する。", "Over every input in the declared domain, these are checked:"),
            tr!(
                "完全性（E101）— 表の末尾の `unreachable!`。Kani は到達可能な panic を既定で探す。",
                "completeness (E101) — the `unreachable!` that closes every table; Kani looks for a reachable panic by default."
            ),
            tr!(
                "W114 のガード — `RuleError::Contradiction` が返らないこと。静的に閉じなかった行の対が、ここで閉じる（あるいは反例が出る）。",
                "the W114 guards — that `RuleError::Contradiction` is never returned. A pair the checker could not close either closes here, or a counterexample comes back."
            ),
            tr!(
                "int64 — オーバーフロー。Kani は算術のオーバーフローを既定で検査する。",
                "int64 — overflow. Kani checks arithmetic overflow by default."
            ),
            tr!(
                "重なり（E105）— `rows_*` が、当たる行を数える。`unique` はちょうど一つ、`first` は一つ以上。",
                "overlap (E105) and completeness from the rows' side — `rows_*` counts the rows that match: exactly one for `unique`, at least one for `first`."
            ),
            tr!(
                "証明の対象はこの Rust であって、表でも検査器でもない。",
                "What is proved is this Rust, not the table and not the checker."
            ),
        ));
        o.push_str("#![allow(non_snake_case, uncommon_codepoints, unused_parens, unused_mut, unused_variables, dead_code, clippy::all)]\n\n");
        // The modules of the `enums` files the rule reads its types from sit at the root, beside
        // it (§15.212).
        let owners: String = self.rs_owner_mods().lines().map(|l| if l.starts_with("#[path") { format!("#[cfg(kani)]\n{l}\n") } else { format!("{l}\n") }).collect();
        o.push_str(&format!("{owners}#[cfg(kani)]\n#[path = \"{alias}.rs\"]\nmod r;\n\n#[cfg(kani)]\nmod proof {{\n    use super::r::*;\n\n"));

        // The inputs, made symbolic once: the same lines serve every harness in the file.
        let mut decls = String::new();
        let mut args: Vec<String> = Vec::new();
        let mut params: Vec<String> = Vec::new();
        // Whether a harness can be written at all, and why not when it cannot.
        let blocked = self.no_harness();
        let mut whole = blocked.is_none();
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            params.push(format!("{}: {}", self.held(&i.name), self.rs_ty(&ty)));
            match self.rs_any(&i.name.text, &self.held(&i.name), &ty, "        ") {
                Some((d, e)) => {
                    decls.push_str(&d);
                    args.push(e);
                }
                None => whole = false,
            }
        }
        // A `constraint` (§15.55) says a combination the rule was never asked to answer, and
        // the generated code refuses it at the door. It is part of the declared domain, so
        // the harness assumes it too; without that, the harness hands the rule an input its
        // own entry guard turns away. Both sides are inputs (E018), and an input here is the
        // plain i64, before it is branded for the call.
        for k in &self.f.constraints {
            let name = |n: &str| self.f.inputs.iter().find(|i| i.name.text == n).map(|i| self.held(&i.name));
            if k.names().all(|n| name(n).is_some()) {
                let (a, b) = self.constraint_sides(k, &|n| name(n).unwrap_or_default(), false);
                decls.push_str(&format!("        kani::assume({a} {} {b});\n", k.op.word()));
            }
        }
        // A `never` line is part of the declared domain the same way (§15.282).
        for n in self.f.door_nevers() {
            let name = |m: &str| self.f.inputs.iter().find(|i| i.name.text == m).map(|i| self.held(&i.name)).unwrap_or_default();
            let conds = self.never_conds(n, Lang::Rs, &name);
            decls.push_str(&format!("        kani::assume(!({}));\n", conds.join(" && ")));
        }
        let mut unwind = String::new();
        let mut unwound = 0;
        for (s, el) in self.f.elements.iter().enumerate() {
            let cap = self.count_cap_of(s).unwrap_or(0);
            let seq = self.held(&el.name);
            let n = if s == 0 { "n".to_string() } else { format!("n{}", s + 1) };
            let ty = self.element_type(s);
            let mut lines = String::new();
            let mut inits: Vec<String> = Vec::new();
            for fd in &el.fields {
                let ty = self.ty_of(&fd.name.text);
                match self.rs_any(&fd.name.text, &format!("{}_v", pub_name(&fd.name)), &ty, "            ") {
                    Some((d, e)) => {
                        lines.push_str(&d);
                        inits.push(format!("{}: {e}", pub_name(&fd.name)));
                    }
                    None => whole = false,
                }
            }
            // The cap is the entry guard's, so the walk is covered whole rather than up to
            // some length picked here: `unwind` one past it leaves nothing bounded away.
            decls.push_str(&format!(
                "        let {n}: usize = kani::any();\n        kani::assume({n} <= {cap});\n        \
                 let {seq}: [{ty}; {cap}] = core::array::from_fn(|_| {{\n{lines}            {ty} {{ {} }}\n        }});\n",
                inits.join(", ")
            ));
            args.push(format!("&{seq}[..{n}]"));
            unwound = unwound.max(cap + 2);
            unwind = format!("    #[kani::unwind({unwound})]\n");
        }

        if !whole {
            let why = blocked.unwrap_or_else(|| {
                tr!(
                    "この規則には、あらゆる値を一度に置けない入力があるので、ハーネスは出していない。",
                    "This rule takes an input a harness cannot quantify over, so none is written."
                )
            });
            o.push_str(&format!("    // {why}\n}}\n"));
            return o;
        }

        // The rows of each table, where the rule is a straight line. A rule that walks a
        // sequence has tables whose columns belong to one element and tables that read what
        // the walk counted, and neither can be replayed outside the walk; the harness above
        // still reaches the `unreachable!` of both.
        let mut harnesses = String::new();
        if self.f.elements.is_empty() {
            for (k, it) in self.f.items.iter().enumerate() {
                let Item::Table(t0) = it else { continue };
                let Some(t) = self.c.table_at(t0) else { continue };
                let Some(rows) = self.rs_rows_fn(k, t, &params.join(", ")) else { continue };
                let tn = t.name.as_ref().map(pub_name).unwrap_or_else(|| format!("t{k}"));
                o.push_str(&rows);
                let (want, doc) = if t.policy == crate::ast::Policy::Unique {
                    (
                        format!("        assert_eq!(rows_{tn}({}), Ok(1));\n", args.join(", ")),
                        tr!("表 {} の行は、宣言した範囲を隙間なく、重なりなく網羅する。", "The rows of {} cover the declared domain with no gap and no overlap.", t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default()),
                    )
                } else {
                    (
                        format!("        assert!(matches!(rows_{tn}({}), Ok(n) if n >= 1));\n", args.join(", ")),
                        tr!("表 {} の行は、宣言した範囲を網羅する（`first` は、重なったときは上の行が勝つ）。", "The rows of {} cover the declared domain (`first` settles an overlap by order).", t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default()),
                    )
                };
                harnesses.push_str(&format!(
                    "    /// {doc}\n    #[kani::proof]\n{unwind}    fn {tn}_rows() {{\n{decls}{want}    }}\n\n"
                ));
            }
        }

        // A value the rule works out outside the range it accepts for it is refused where it is
        // worked out (§15.313), so a call that comes to one is outside the declared domain as one
        // a `constraint` rules out is. The harness assumes away the refusal the code gives for
        // it — that refusal, and only with a value outside the range, so a door that refused a
        // value inside would still be caught — and holds every other call to an answer. The
        // sentence is compared as bytes of a fixed length: two strings are compared by a loop
        // over a length the model checker cannot bound, and it does not come back. The bytes are
        // a loop of that length, so a harness that bounds its loops for a walk bounds them past
        // the longest sentence too. A rule with none is written as before.
        // A `define` worked out for each element is said with the element's index (§15.324): the
        // pieces of the sentence around the index are compared one by one, the index left free.
        let mut assumed = String::new();
        let mut longest = 0;
        let said_is = |sentence: &str| -> (String, i128) {
            let words: Vec<&str> = pieces(sentence).into_iter().filter_map(|p| if let Piece::Text(t) = p { Some(t) } else { None }).collect();
            let test = words.iter().enumerate().map(|(k, w)| format!(" && said::<{}>(what.words()[{k}], \"{w}\")", w.len())).collect::<String>();
            (format!("what.words().len() == {}{test}", words.len()), words.iter().map(|w| w.len() as i128).max().unwrap_or(0))
        };
        for (i, it) in self.f.items.iter().enumerate() {
            let Item::Define(d) = it else { continue };
            let Some(test) = self.accepted_ends(d).and_then(|ends| or_out_of("refused", ends, "")) else { continue };
            let name = match self.walked_in(i) {
                Some(s) => crate::door::element(&self.f.elements[s].name.text, crate::door::HERE, &d.name.text),
                None => d.name.text.clone(),
            };
            let (is, len) = said_is(&crate::door::out_of_range(&name));
            longest = longest.max(len);
            assumed.push_str(&format!(
                "        kani::assume(!matches!(answer, Err(RuleError::Input {{ what, value: Some(refused) }}) if {is} && ({test})));\n"
            ));
        }
        // So is a call whose `sum` or `count` ends its walk outside the range it declares
        // (§15.100, §15.314): past the top of a sum, where the longest sequence the harness
        // builds, every element at the top of what is summed, passes it; or under the bottom of
        // either, where a walk can come in under it. The refusal is assumed away with a total
        // outside the range only, so a walk that refused a total inside it is still caught.
        let mut summaries = false;
        for d in self.f.aggs().filter(|d| !d.kind.extreme()) {
            let longest_walk = self.f.elements.iter().position(|el| el.name.text == d.over).and_then(|s| self.count_cap_of(s)).unwrap_or(0);
            let held = self.c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi);
            let each = self.c.ranges.get(&d.column.text).and_then(|(_, hi)| *hi);
            let passed = match (held, each) {
                (Some(hi), Some(each)) => each.checked_mul(crate::num::Rat::int(longest_walk)).is_none_or(|r| r.cmp_to(hi) == std::cmp::Ordering::Greater),
                _ => true,
            };
            let top = self.sum_caps().into_iter().find(|(x, _)| x.name.text == d.name.text).map(|(_, cap)| cap).filter(|_| passed);
            let Some(test) = or_out_of("refused", (self.walk_floor(d), top), "") else { continue };
            let (is, len) = said_is(&crate::door::out_of_range(&d.name.text));
            longest = longest.max(len);
            summaries = true;
            assumed.push_str(&format!(
                "        kani::assume(!matches!(answer, Err(RuleError::Input {{ what, value: Some(refused) }}) if {is} && ({test})));\n"
            ));
        }
        if !unwind.is_empty() && !assumed.is_empty() {
            unwind = format!("    #[kani::unwind({})]\n", unwound.max(longest + 2));
        }
        let (call, accepted_doc) = if assumed.is_empty() {
            (format!("        assert!({alias}({}).is_ok());\n", args.join(", ")), String::new())
        } else {
            o.push_str(&format!(
                "    /// {}\n    fn said<const N: usize>(what: &str, said: &str) -> bool {{\n        use std::convert::TryFrom;\n        match (<[u8; N]>::try_from(what.as_bytes()), <[u8; N]>::try_from(said.as_bytes())) {{\n            (Ok(a), Ok(b)) => a == b,\n            _ => false,\n        }}\n    }}\n\n",
                tr!(
                    "`what` が `said` か。長さ N のバイトの並びとして比べる。文字列どうしの比較は長さの決まらないループになり、モデル検査器が返ってこない。",
                    "Whether `what` is `said`, compared as N bytes: comparing two strings is a loop over a length the model checker cannot bound, and it does not come back."
                )
            ));
            (
                format!("        let answer = {alias}({});\n{assumed}        assert!(answer.is_ok());\n", args.join(", ")),
                format!(
                    "    /// {}\n",
                    if summaries {
                        tr!(
                            "計算した値や、並びの `sum`・`count` が、規則がその値に受け付ける範囲の外になる呼び出しは、宣言した範囲の外にある。`constraint` を満たさない呼び出しと同じに、コードがその値で返す誤りを仮定で除く。範囲の中の値で返る誤りは除かないので、ここで見つかる。",
                            "A call on which a value the rule works out, or a `sum` or a `count` over a sequence, falls outside the range the rule accepts for it is outside the declared domain, and is assumed away by the refusal the code gives with that value, as one outside a `constraint` is. A refusal with a value inside the range is not, and is caught here."
                        )
                    } else {
                        tr!(
                            "計算した値が、規則がその値に受け付ける範囲の外になる呼び出しは、宣言した範囲の外にある。`constraint` を満たさない呼び出しと同じに、コードがその値で返す誤りを仮定で除く。範囲の中の値で返る誤りは除かないので、ここで見つかる。",
                            "A call whose value the rule works out falls outside the range it accepts for that value is outside the declared domain, and is assumed away by the refusal the code gives with that value, as one outside a `constraint` is. A refusal with a value inside the range is not, and is caught here."
                        )
                    }
                ),
            )
        };
        o.push_str(&format!(
            "    /// {}\n{accepted_doc}    #[kani::proof]\n{unwind}    fn {alias}_answers() {{\n{decls}{call}    }}\n\n",
            tr!(
                "宣言した範囲のどの入力にも、この規則は答えを返す。返らないのは、完全性に穴があるとき、W114 の対が実在したとき、入口のガードに当たったときの三つ。三つめが起きないことは、ここで仮定した範囲が宣言した範囲そのものだ、という確認でもある。",
                "For every input in the declared domain the rule answers. There are three ways not to — a gap in the completeness, a W114 pair that was real, and the entry guard — and the third not happening is also the check that the domain assumed here is the declared one."
            ),
        ));
        o.push_str(&harnesses);
        o.push_str("}\n");
        o
    }

    /// The names of the harnesses `rs_proof` writes, in the order they appear.
    /// Why no proof harness can be written for this rule, where that is so. `rs_proof`
    /// prints the reason in the file it writes and the list `rulec api` gives asks the same
    /// question here, so the two cannot disagree — they did, and `api` would have named
    /// harnesses the file does not contain.
    fn no_harness(&self) -> Option<String> {
        // A share divides by a value rather than by a constant, and two of them subtracted —
        // which is how one line's share is written — puts two symbolic divisions under one
        // assertion. CBMC bit-blasts both, and the shape does not come back: measured, the
        // corpus rule does not finish in ten minutes even with every range cut to a
        // thousand yen, where the same rule with one share verifies in one second (§15.102).
        // A harness that hangs is worse than one that is not written, so this says why.
        fn shares(e: &Expr) -> bool {
            match e {
                Expr::Call(n, args, _) => n == crate::kw::ALLOCATE || args.iter().any(shares),
                Expr::Bin(l, _, r, _) => shares(l) || shares(r),
                _ => false,
            }
        }
        let in_items = self.f.items.iter().any(|it| match it {
            Item::Derived(d) => shares(&d.expr),
            Item::Define(d) => shares(&d.expr),
            _ => false,
        });
        if in_items || self.f.result.as_ref().is_some_and(|r| shares(&r.expr)) {
            return Some(tr!(
                "この規則は `allocate` で配分を出している。変数で割る式が二つ入るとモデル検査器が返ってこないので、ハーネスは出していない。",
                "This rule works out a share with `allocate`. Two divisions by a value rather than by a constant do not come back from the model checker, so no harness is written."
            ));
        }
        // A rounded quotient by a value is a division of the same kind (§15.213), and a whole
        // square root is a search that runs up to 32 rounds of a 64-bit square (§15.215): the
        // model checker bit-blasts both, and a harness that does not come back is not written.
        let uses = self.arith_uses();
        if !uses.div.is_empty() {
            return Some(tr!(
                "この規則は、値で割った商を丸めている。値で割る式はモデル検査器が返ってこないことがあるので、ハーネスは出していない。",
                "This rule rounds a quotient by a value. A division by a value need not come back from the model checker, so no harness is written."
            ));
        }
        if uses.root {
            return Some(tr!(
                "この規則は `isqrt` を使っている。平方根を二分探索で求めるループはモデル検査器が返ってこないことがあるので、ハーネスは出していない。",
                "This rule takes an `isqrt`. The search the square root runs is a loop that need not come back from the model checker, so no harness is written."
            ));
        }
        // Anything the harness cannot make symbolic — a string, in an input or in a field of
        // one element — takes the whole file with it: a harness that fixes one input is not
        // over the declared domain.
        let opaque = |n: &str| self.rs_any(n, "x", &self.ty_of(n), "").is_none();
        let fields = self.f.elements.iter().flat_map(|e| e.fields.iter());
        if self.f.inputs.iter().any(|i| opaque(&i.name.text)) || fields.clone().any(|f| opaque(&f.name.text)) {
            return Some(tr!(
                "この規則には string の入力があり、あらゆる文字列を一度には置けないので、ハーネスは出していない。",
                "This rule takes an input a harness cannot quantify over (a string), so none is written."
            ));
        }
        None
    }

    fn proof_harnesses(&self) -> Vec<String> {
        let alias = pub_name(&self.f.name);
        if self.no_harness().is_some() {
            return Vec::new();
        }
        let mut v = vec![format!("{alias}_answers")];
        if self.f.elements.is_empty() {
            for (k, it) in self.f.items.iter().enumerate() {
                let Item::Table(t0) = it else { continue };
                let Some(t) = self.c.table_at(t0) else { continue };
                v.push(format!("{}_rows", t.name.as_ref().map(pub_name).unwrap_or_else(|| format!("t{k}"))));
            }
        }
        v
    }

    /// The members of an enum, as the paths the generated Rust writes.
    fn enum_members(&self, n: &str) -> Vec<String> {
        if let Some(e) = self.f.enums.iter().find(|e| e.name.text == n) {
            return e.values.iter().map(|v| self.rs_value(&v.text, &Ty::Enum(n.to_string()))).collect();
        }
        if let Some(im) = crate::prelude::import_of(self.f, n) {
            return im.values().iter().map(|v| self.rs_value(v, &Ty::Enum(n.to_string()))).collect();
        }
        Vec::new()
    }

    /// One value made symbolic for a harness: the lines that declare it, and the expression
    /// that goes in as the argument. `None` for a type there is no useful way to quantify
    /// over (a string), which drops the harness rather than pretending (§15.95).
    fn rs_any(&self, name: &str, ident: &str, ty: &Ty, ind: &str) -> Option<(String, String)> {
        match ty {
            Ty::Bool => Some((format!("{ind}let {ident}: bool = kani::any();\n"), ident.into())),
            Ty::Str => None,
            Ty::Enum(n) => {
                let mut vs = self.enum_members(n);
                // An input of an ordered enum with a range takes only the values inside it
                // (§15.210): the rule refuses the rest at its door, as it does a number outside
                // its range, so the harness assumes the range as it does for a number.
                if let Some(allowed) = self.c.enum_ranges.get(name) {
                    let words: Vec<String> = if let Some(e) = self.f.enums.iter().find(|e| e.name.text == *n) {
                        e.values.iter().map(|v| v.text.clone()).collect()
                    } else if let Some(im) = crate::prelude::import_of(self.f, n) {
                        im.values().iter().map(|v| v.to_string()).collect()
                    } else {
                        Vec::new()
                    };
                    vs = words.iter().zip(vs).filter(|(w, _)| allowed.contains(w)).map(|(_, v)| v).collect();
                }
                if vs.is_empty() {
                    return None;
                }
                let arms: Vec<String> = vs
                    .iter()
                    .enumerate()
                    .map(|(k, v)| if k + 1 == vs.len() { format!("_ => {v}") } else { format!("{k} => {v}") })
                    .collect();
                Some((
                    format!("{ind}let {ident} = match kani::any::<u8>() {{ {} }};\n", arms.join(", ")),
                    ident.into(),
                ))
            }
            Ty::Opt(t) => {
                let (d, e) = self.rs_any(name, &format!("{ident}_v"), t, ind)?;
                Some((
                    format!("{d}{ind}let {ident} = if kani::any::<bool>() {{ Some({e}) }} else {{ None }};\n"),
                    ident.into(),
                ))
            }
            // Money, quantities, rates, numbers and dates all travel as an i64 (§10.2), and
            // the harness assumes what the entry guard lets in: each end the range writes, and
            // −(2^63−1) where it writes no lower one (§15.295).
            _ => {
                let mut d = format!("{ind}let {ident}: i64 = kani::any();\n");
                let (lo, hi) = crate::door::ends(self.c, name, false);
                let mut b: Vec<String> = Vec::new();
                if let Some(lo) = lo {
                    b.push(format!("{lo} <= {ident}"));
                }
                if let Some(hi) = hi {
                    b.push(format!("{ident} <= {hi}"));
                }
                if !b.is_empty() {
                    d.push_str(&format!("{ind}kani::assume({});\n", b.join(" && ")));
                }
                let brand = self.rs_ty(ty);
                let e = if brand == "i64" { ident.to_string() } else { format!("{brand}({ident})") };
                Some((d, e))
            }
        }
    }

    /// How many rows of one table match, as a function of the rule's inputs. Everything the
    /// rule computes before the table is computed here the same way, so the dependencies
    /// between derived values — the ones the axis space drops, and W114 with them — are
    /// present as arithmetic (§15.95).
    fn rs_rows_fn(&self, upto: usize, t: &Table, params: &str) -> Option<String> {
        // The same reading of a name as the rule's own body: a branded input is an i64
        // inside, unwrapped where it is read.
        let local = |n: &str| -> String {
            match self.f.inputs.iter().find(|i| i.name.text == n) {
                Some(i) => {
                    let v = self.held(&i.name);
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                        format!("{v}.0")
                    } else {
                        v
                    }
                }
                None => self.ident(n),
            }
        };
        let trace = self.temp("trace");
        let mut body = String::new();
        for (k, it) in self.f.items.iter().enumerate() {
            if k >= upto {
                break;
            }
            match it {
                Item::Derived(d) => body.push_str(&format!(
                    "    let {} = {};\n",
                    self.ident(&d.name.text),
                    rs_expr(unparen(&self.expr(&d.expr, &local).text))
                )),
                Item::Define(d) => {
                    body.push_str(&format!(
                        "    let {} = {};\n",
                        self.ident(&d.name.text),
                        rs_expr(unparen(&self.expr(&d.expr, &local).text))
                    ));
                    // A call whose value falls outside the range the rule accepts for it never
                    // reaches the table (§15.313): it is outside the domain the rows cover.
                    let n = self.ident(&d.name.text);
                    if let Some(test) = self.accepted_ends(d).and_then(|ends| or_out_of(&n, ends, "")) {
                        body.push_str(&format!("    kani::assume(!({test}));\n"));
                    }
                }
                Item::Agg(_) | Item::Fold(_) => {}
                Item::Table(t) => {
                    let t = self.c.table_at(t)?;
                    body.push_str(&self.rs_table(t, &local, &trace));
                }
            }
        }
        let tname = t.name.as_ref().map(pub_name).unwrap_or_else(|| format!("t{upto}"));
        let mut o = format!(
            "    /// {}\n    fn rows_{tname}({params}) -> Result<u32, RuleError> {{\n",
            tr!(
                "表 {} の、当たる行の数。{}",
                "How many rows of {} match. {}",
                t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default(),
                if t.policy == crate::ast::Policy::Unique {
                    tr!("`policy unique` なら、どの入力でもちょうど一行。", "`policy unique` says exactly one.")
                } else {
                    tr!("`policy first` なら、どの入力でも一行以上。", "`policy first` says at least one.")
                }
            )
        );
        if !body.is_empty() {
            o.push_str(&format!("        let mut {trace}: Vec<Fired> = Vec::new();\n"));
            o.push_str(&Self::indent_block(&body, "    "));
        }
        o.push_str("        Ok(0\n");
        for (ri, row) in t.rows.iter().enumerate() {
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.rs_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            let cond = if conds.is_empty() { "true".into() } else { conds.join(" && ") };
            let cells: Vec<String> = row.cells.iter().map(cell_src).chain(row.outs.iter().map(out_src)).collect();
            o.push_str(&format!("            + (({cond}) as u32) // {}\n", self.row_head(t, ri, &cells.join(" | "))));
        }
        o.push_str("        )\n    }\n\n");
        Some(o)
    }
    /// One input read off the wire for the Rust module, the way every door reads it (§15.204):
    /// an expression that `?` stops with the module's own `RuleError`, inside a closure that
    /// returns one. The Rust runner, its WASI build and the Wasm entry all read with it.
    fn rs_bind(&self, ty: &Ty, src: &str, name: &str) -> String {
        self.rs_bind_as(ty, src, name, name, "")
    }

    /// [`Gen::rs_bind`], with the sentences calling the value `said`: for a field of an element
    /// `lines[\u{1}].amount`, which takes the index `at` of the element as `r::Said` (§15.324).
    fn rs_bind_as(&self, ty: &Ty, src: &str, name: &str, said: &str, at: &str) -> String {
        let lit = |x: String| -> String { if names_element(&x) { format!("r::{}", rs_said(&x, at, "")) } else { format!("{x:?}") } };
        let optional = matches!(ty, Ty::Opt(_));
        let take = format!("take({src}, {name:?}, {}, {optional})?", lit(crate::door::missing(said)));
        let one = |v: &str| -> String {
            match ty.present() {
                Ty::Enum(n) => {
                    let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                    format!("member(r::{cls}::parse, {v}, {})?", lit(crate::door::not_in_enum(said, n)))
                }
                Ty::Bool => format!("boolean({v}, {})?", lit(crate::door::not_bool(said))),
                Ty::Date => format!("date({v}, {})?", lit(crate::door::not_date(said))),
                Ty::Number => format!("int({v}, {}, {})?", lit(crate::door::not_integer(said)), lit(crate::door::out_of_range(said))),
                Ty::Str => format!("text({v}, {})?", lit(crate::door::not_string(said))),
                other => format!("r::{}(int({v}, {}, {})?)", self.rs_ty(other), lit(crate::door::not_integer(said)), lit(crate::door::out_of_range(said))),
            }
        };
        if optional {
            // `null` on the wire is `None`: the scanner keeps an unquoted token as its text and a
            // string with its quote, so the four letters are the test (§15.88).
            format!("{{ let v = {take}; if v == \"null\" {{ None }} else {{ Some({}) }} }}", one("v"))
        } else {
            one(&take)
        }
    }

    /// The statements that read the inputs into `a0`, `a1`, …, and the sequence into the one
    /// after them, for the closure `rs_bind` writes into.
    fn rs_binds(&self, d: &str, indent: &str, carried: &[(usize, String)]) -> String {
        let mut o = String::new();
        for (i, inp) in self.f.inputs.iter().enumerate() {
            let read = self.rs_bind(&self.ty_of(&inp.name.text), d, &inp.name.text);
            match carried.iter().find(|(k, _)| *k == i) {
                Some((_, state)) => o.push_str(&format!("{indent}let a{i} = if stepping {{ {state} }} else {{ {read} }};\n")),
                None => o.push_str(&format!("{indent}let a{i} = {read};\n")),
            }
        }
        // A sentence about an element names it by its index `i` (§15.324).
        for (s, el) in self.f.elements.iter().enumerate() {
            let n = self.f.inputs.len() + s;
            let seq = &el.name.text;
            let ty = self.element_type(s);
            let fields: Vec<String> = el
                .fields
                .iter()
                .map(|fd| format!("{}: {}", pub_name(&fd.name), self.rs_bind_as(&self.ty_of(&fd.name.text), "e", &fd.name.text, &crate::door::element(seq, crate::door::HERE, &fd.name.text), "i")))
                .collect();
            o.push_str(&format!(
                "{indent}let mut a{n}: Vec<r::{ty}> = Vec::new();\n{indent}for (i, e) in list(take({d}, {seq:?}, {:?}, false)?, {:?}, |i| r::{})?.iter().enumerate() {{\n{indent}    a{n}.push(r::{ty} {{ {} }});\n{indent}}}\n",
                crate::door::missing(seq),
                crate::door::not_sequence(seq),
                rs_said(&crate::door::not_object(seq, crate::door::HERE), "i", ""),
                fields.join(", ")
            ));
        }
        o
    }

    /// The arguments of one call: every input, the sequence borrowed, a string cloned for the
    /// first of two calls.
    fn rs_pass(&self, clone: bool) -> String {
        let n_ins = self.f.inputs.len();
        let mut v: Vec<String> = self
            .f
            .inputs
            .iter()
            .enumerate()
            .map(|(i, inp)| if clone && matches!(self.ty_of(&inp.name.text), Ty::Str) { format!("a{i}.clone()") } else { format!("a{i}") })
            .collect();
        for s in 0..self.f.elements.len() {
            v.push(format!("&a{}", n_ins + s));
        }
        v.join(", ")
    }

    pub fn rs_runner(&self) -> String {
        // A rule that reads no input reads nothing out of what it is given, which Rust warns of
        // (§15.330).
        let d_var = if self.f.inputs.is_empty() && self.f.elements.is_empty() { "_d" } else { "d" };
        let alias = pub_name(&self.f.name);
        let (first, second) = (self.rs_pass(true), self.rs_pass(false));
        // the record call's own arguments, each with the `, ` after it: none for a rule that
        // reads no input
        let second = if second.is_empty() { second } else { format!("{second}, ") };
        // A machine's traces (§15.148): the carried input is the state this language answered
        // to the call before, handed over as the value it is.
        let (mut head, mut meta, mut tail, mut keep, mut next) = (String::new(), String::new(), String::new(), String::new(), "()".to_string());
        let mut carried: Vec<(usize, String)> = Vec::new();
        if let (Some((en, _, _)), Some((cin, _)), Some(out)) = (self.machine_consts(), self.carried(), self.carried_out_alias()) {
            let cls = self.enum_names.get(&en).cloned().unwrap_or_default();
            let all: Vec<String> = self.c.enums.get(&en).into_iter().flatten().map(|v| format!("r::{}", self.rs_value(v, &Ty::Enum(en.clone())))).collect();
            carried.extend(self.f.inputs.iter().position(|i| i.name.text == cin).map(|k| (k, "state".to_string())));
            // The values carried along with the state (§15.217): a local each, from the
            // module's constant, read where each trace starts and handed on from the answer.
            let more = self.more_carried();
            let (mut more_head, mut more_meta, mut more_start, mut more_next, mut more_tail) = (String::new(), Vec::<String>::new(), String::new(), Vec::<String>::new(), String::new());
            for (j, mc) in more.iter().enumerate() {
                let local = format!("carried_{}", mc.in_alias);
                carried.extend(self.f.inputs.iter().position(|i| i.name.text == mc.input).map(|k| (k, local.clone())));
                more_head.push_str(&format!("    let mut {local} = r::{};\n", mc.upper()));
                let shown = match mc.kind() {
                    MoreKind::Enum => format!("format!(\"\\\"{{}}\\\"\", r::{}.as_str())", mc.upper()),
                    MoreKind::Bool | MoreKind::Number => format!("r::{}.to_string()", mc.upper()),
                    MoreKind::Branded => format!("r::{}.0.to_string()", mc.upper()),
                };
                more_meta.push(format!("format!(\"{}:{{}}\", {shown})", mc.key().replace('\\', "\\\\").replace('"', "\\\"").replace('{', "{{").replace('}', "}}")));
                more_start.push_str(&format!(
                    "            {local} = (|| -> Result<_, r::RuleError> {{ Ok({}) }})().expect(\"carried\");\n",
                    self.rs_bind(&mc.ty, "&cv", &mc.input)
                ));
                more_next.push(format!("got.{}", mc.out_alias));
                more_tail.push_str(&format!("                    {local} = next.{};\n", j + 1));
            }
            head = format!("    let mut state = r::INITIAL;\n{more_head}");
            let print_meta = if more.is_empty() {
                "println!(\"{{\\\"initial\\\":\\\"{}\\\",\\\"final\\\":[{}]}}\", r::INITIAL.as_str(), fin.join(\",\"));".to_string()
            } else {
                format!(
                    "println!(\"{{{{\\\"initial\\\":\\\"{{}}\\\",\\\"final\\\":[{{}}],\\\"carried\\\":{{{{{{}}}}}}}}}}\", r::INITIAL.as_str(), fin.join(\",\"), [{}].join(\",\"));",
                    more_meta.join(", ")
                )
            };
            if !more_start.is_empty() {
                more_start = format!("            let cv = pairs_from(&get(\"carried\").unwrap_or_default().chars().collect::<Vec<char>>(), 0).0;\n{more_start}");
            }
            meta = format!(
                "        let top = pairs_from(&line.chars().collect::<Vec<char>>(), 0).0;\n        \
                 let get = |k: &str| top.iter().find(|(a, _)| a == k).map(|(_, v)| v.strip_prefix('\"').unwrap_or(v).to_string());\n        \
                 if get(\"machine\").is_some() {{\n            \
                     let fin: Vec<String> = [{}].iter().filter(|s| r::is_final(**s)).map(|s| format!(\"\\\"{{}}\\\"\", s.as_str())).collect();\n            \
                     {print_meta}\n            \
                     continue;\n        }}\n        \
                 let stepping = get(\"step\").is_some();\n        \
                 if get(\"step\").as_deref() == Some(\"start\") {{\n            \
                     state = r::{cls}::parse(&get(\"state\").unwrap_or_default()).expect(\"state\");\n{more_start}        }}\n",
                all.join(", ")
            );
            if more.is_empty() {
                keep = format!("            let next = {};\n", self.next_state_of("got", &format!(".{out}")));
                tail = "                if stepping {\n                    state = next;\n                }\n".to_string();
            } else {
                keep = format!("            let next = ({}, {});\n", self.next_state_of("got", &format!(".{out}")), more_next.join(", "));
                tail = format!("                if stepping {{\n                    state = next.0;\n{more_tail}                }}\n");
            }
            next = "next".to_string();
        }
        let binds = self.rs_binds("&d", "            ", &carried);
        let ok = if next == "()" { "Ok((record, ()))" } else { "Ok((record, next))" };
        let owners = self.rs_owner_mods();
        let labels = self.rs_labels_mode() + &self.rs_prereq_mode() + &self.rs_chances_mode(&self.rs_binds("&d", "                ", &[]));
        format!(
            r#"// Code generated by rulec {ver}. DO NOT EDIT.
#![allow(non_snake_case, uncommon_codepoints, unused_parens, dead_code, clippy::all)]

{owners}#[path = "{alias}.rs"]
mod r;

use std::io::Read;

{helpers}{json_str}fn main() {{
    let mut src = String::new();
    std::io::stdin().read_to_string(&mut src).unwrap();
{labels}{head}    for line in src.lines() {{
        if line.trim().is_empty() {{
            continue;
        }}
{meta}        let {d_var} = fields(line);
        // One call, or the refusal its door gives, in the place of the record (§15.204).
        let res = (|| -> Result<_, r::RuleError> {{
{binds}            let (got, trace) = r::{alias}_traced({first})?;
{keep}            Ok((r::{alias}_record({second}got, &trace, ""), {next}))
        }})();
        match res {{
            {ok} => {{
                println!("{{record}}");
{tail}            }}
            Err(r::RuleError::Input {{ what, .. }}) => println!("{{{{\"refused\":\"input\",\"error\":{{}}}}}}", json_str(&what.to_string())),
            Err(r::RuleError::Contradiction {{ what }}) => println!("{{{{\"refused\":\"contradiction\",\"error\":{{}}}}}}", json_str(&what.to_string())),
        }}
    }}
}}
"#,
            ver = ritsu_emit::header::VERSION,
            helpers = rs_json_helpers(!self.f.elements.is_empty()),
            json_str = RS_JSON_STR,
        )
    }

    /// What the Rust runner answers with `--labels` (§15.211): for each line, an enum's name and
    /// a value's, the display name the module gives.
    fn rs_labels_mode(&self) -> String {
        let enums = self.labeled_enums();
        if enums.is_empty() {
            return String::new();
        }
        let arms: String = enums
            .iter()
            .map(|(en, ty, _)| format!("                {} => println!(\"{{}}\", r::{ty}::parse(v).map(|x| x.label()).unwrap_or(\"?\")),\n", str_lit(en)))
            .collect();
        format!(
            "    if std::env::args().any(|a| a == \"--labels\") {{\n        for line in src.lines() {{\n            let Some((en, v)) = line.split_once('\\t') else {{ continue }};\n            match en {{\n{arms}                _ => {{}}\n            }}\n        }}\n        return;\n    }}\n"
        )
    }

    /// The rule as one Wasm module for any host (§15.64): the module `rust/` gets, behind the
    /// canonical ABI of `call: func(input: string) -> string`. A JavaScript host drives the
    /// core module through `cabi_realloc`, `call` and `cabi_post_call`; with the `.wit` beside
    /// it, `wasm-tools component new` makes a component of the same file without a change.
    /// An input outside the contract is answered as the refusal the runners print (§15.204),
    /// bytes that are not UTF-8 among them: a host gets a line it can read, not a trap. The three
    /// functions of the ABI are ritsu-emit's (§15.278).
    pub fn rs_wasm(&self) -> String {
        // A rule that reads no input reads nothing out of what it is given, which Rust warns of
        // (§15.330).
        let d_var = if self.f.inputs.is_empty() && self.f.elements.is_empty() { "_d" } else { "d" };
        let alias = pub_name(&self.f.name);
        let (first, second) = (self.rs_pass(true), self.rs_pass(false));
        // the record call's own arguments, each with the `, ` after it: none for a rule that
        // reads no input
        let second = if second.is_empty() { second } else { format!("{second}, ") };
        let binds = self.rs_binds("&d", "        ", &[]);
        let mut o = format!(
            "// Code generated by rulec {}. DO NOT EDIT.\n#![allow(non_snake_case, uncommon_codepoints, unused_parens, dead_code, clippy::all)]\n\n#[path = \"{alias}.rs\"]\nmod r;\n\n{}\n",
            ritsu_emit::header::VERSION,
            ritsu_emit::wasm::ALLOC_USE
        );
        o.push_str(&rs_json_helpers(!self.f.elements.is_empty()));
        o.push_str(RS_JSON_STR);
        o.push_str(&rs_wasm_abi());
        o.push_str(&format!(
            "/// The inputs read, the rule called, the record line written — or the refusal.\nfn answer(input: &str) -> String {{\n    let chars: Vec<char> = input.chars().collect();\n    let top = pairs_from(&chars, 0).0;\n    // A record carries the inputs under \"in\"; a bare object is the inputs themselves.\n    let inner: Option<String> = top.iter().find(|(k, _)| k == \"in\").map(|(_, v)| v.clone());\n    let {d_var} = match inner {{\n        Some(v) => pairs_from(&v.chars().collect::<Vec<_>>(), 0).0,\n        None => top,\n    }};\n    let res = (|| -> Result<String, r::RuleError> {{\n{binds}        let (got, trace) = r::{alias}_traced({first})?;\n        Ok(r::{alias}_record({second}got, &trace, \"\"))\n    }})();\n    match res {{\n        Ok(record) => record,\n        Err(e) => refusal(&e),\n    }}\n}}\n"
        ));
        o
    }

    /// The rule's interface for the component model (§15.64): one world exporting
    /// `call: func(input: string) -> string`, the JSON wire on both sides, with the inputs and
    /// outputs in the doc comments so that a reader of the `.wit` knows what to send. The world is
    /// the alias as ritsu-emit's `wasm::world` spells it, which WIT reads for every alias (§15.278).
    pub fn wit(&self) -> String {
        let alias = pub_name(&self.f.name);
        let world = ritsu_emit::wasm::world(&alias);
        let mut o = format!("package {};\n\n", wit_package(&alias, &self.f.version));
        o.push_str(&format!("/// Rule {} v{}", self.f.name.text, self.f.version));
        if let Some(d) = &self.f.description {
            o.push_str(&format!(": {}", d.replace('\n', " ")));
        }
        o.push_str(&format!("\nworld {world} {{\n"));
        o.push_str(
            "    /// One call. `input` is a JSON object with the rule's inputs by their names, in the\n    /// wire form (whole numbers in the declared unit, enum values by name, dates as\n    /// YYYY-MM-DD); a record with them under \"in\" is read the same way. Back comes the\n    /// record line {\"in\":…,\"out\":…,\"trace\":[…]}, or {\"refused\":\"input\",\"error\":\"…\"} for an\n    /// input outside the contract (\"contradiction\" for one the rule cannot answer).\n    ///\n    /// Inputs:\n",
        );
        for i in &self.f.inputs {
            o.push_str(&format!("    ///   {}: {}\n", i.name.text, self.ty_of(&i.name.text)));
        }
        for el in &self.f.elements {
            let fields: Vec<String> =
                el.fields.iter().map(|fd| format!("{}: {}", fd.name.text, self.ty_of(&fd.name.text))).collect();
            o.push_str(&format!("    ///   {}: a list of objects, each with {}\n", el.name.text, fields.join(", ")));
        }
        o.push_str("    /// Outputs:\n");
        for od in &self.f.outputs {
            o.push_str(&format!("    ///   {}: {}\n", od.name.text, self.ty_of(&od.name.text)));
        }
        o.push_str(&format!("    export {};\n}}\n", ritsu_emit::wasm::CALL_SIGNATURE));
        o
    }

    /// The `wasi` entry, inside the Rust entry of `rulec api`: the *other* shape of Wasm
    /// (§15.63) — the runner itself built as a command that reads stdin and writes stdout, the
    /// shape Fastly Compute, Spin or a batch step in a sandbox takes. It sits under the
    /// language and not at the top level because the registry carries it as a property of a
    /// backend (`Backend::wasi`), so a second language that ever earned the column would get
    /// the same key.
    pub fn api_wasi(&self) -> String {
        let alias = pub_name(&self.f.name);
        crate::json::Obj::new()
            .str("source", &format!("{alias}_runner.rs"))
            .str("module", &format!("{alias}_runner.wasm"))
            .str(
                "build",
                &format!("rustc --edition 2021 {WASI_RUSTC_FLAGS} {alias}_runner.rs -o {alias}_runner.wasm"),
            )
            .str("run", &format!("wasmtime {alias}_runner.wasm"))
            .str(
                "wire",
                "one vectors line on stdin, one fixtures record per line on stdout, or {\"refused\":…,\"error\":…} in the place of the record for a line it refuses",
            )
            .raw("needs", crate::json::strs(&["wasmtime", "rustup target add wasm32-wasip1"]))
            .finish()
    }

    /// The `wasm` entry of `rulec api`: the files, the build line, and the names of the
    /// exports a host needs.
    pub fn api_wasm(&self) -> String {
        let alias = pub_name(&self.f.name);
        let world = ritsu_emit::wasm::world(&alias);
        crate::json::Obj::new()
            .str("source", &format!("{alias}_wasm.rs"))
            .str("module", &format!("{alias}.wasm"))
            .str("build", &format!("rustc --edition 2021 {WASM_RUSTC_FLAGS} {alias}_wasm.rs -o {alias}.wasm"))
            .str("wit", &format!("{alias}.wit"))
            .str("package", &wit_package(&alias, &self.f.version))
            .str("world", &world)
            .str("call", "call")
            .str("call_signature", ritsu_emit::wasm::CALL_SIGNATURE)
            .str("post_return", "cabi_post_call")
            .str("realloc", "cabi_realloc")
            .str("memory", "memory")
            .str("runner", &format!("{alias}_runner.mjs"))
            .str("component", &ritsu_emit::wasm::component_line(&alias))
            // What `call` answers (§15.206): the record of the call, or in its place the line of a
            // refusal, which says which of the two the door refused for — the caller's input, or
            // the rule's own contradiction — and the sentence. The module raises nothing: the
            // answer is its only way of saying either, so the inventory says both.
            .raw("answers", api_wasm_answers())
            .finish()
    }
}

/// The two shapes of the Wasm module's answer (§15.206), as `rulec api` states them: the record,
/// with the keys every language's record function writes, and the refusal, with its two keys and
/// the values `refused` takes — `input` and `contradiction`, the `kind`s of `error_types`.
fn api_wasm_answers() -> String {
    use crate::json::{Obj, arr, strs};
    arr(&[
        Obj::new().str("kind", "record").raw("keys", strs(&["in", "observed", "trace"])).finish(),
        Obj::new()
            .str("kind", "refusal")
            .raw("keys", strs(&["refused", "error"]))
            .raw("refused", strs(&["input", "contradiction"]))
            .finish(),
    ])
}

/// The package the Python runner and MCP server make of their own directory, to read the modules
/// written to go inside a package (§15.327). Each of them is a program of its own, so the name
/// meets nothing of the caller's.
const PY_PACKAGE: &str = "_rulec";

/// The lines, after the imports, that make the directory of the file a package of the name
/// [`PY_PACKAGE`] (§15.327): the modules beside the file are then read as the modules of that
/// package, and their relative imports find one another. They want `os`, `sys` and `types`.
pub(crate) fn py_package_lines() -> String {
    format!(
        "{}_package = sys.modules.setdefault(\"{PY_PACKAGE}\", types.ModuleType(\"{PY_PACKAGE}\"))\n\
         _package.__path__ = [os.path.dirname(os.path.abspath(__file__))]\n",
        tr!(
            "# 規則のモジュールはパッケージの中に置く形で書いてあり（--python-package）、隣のモジュールを\n# 相対の import で読む。この置き場を一つのパッケージとして読む。\n",
            "# The rule's module is written to go inside a package (--python-package) and reads the\n# modules beside it by relative imports, so this directory is read as a package.\n"
        )
    )
}

/// How the generated Python runners read the wire (§15.204): an input there at all, of its
/// kind, refused with the module's own `RuleInputError` and the sentence every language says;
/// and the line a refusal prints in the place of the record. The Python runner and the Connect
/// runner share it, so the two read a line the same way.
pub(crate) const PY_WIRE: &str = r#"E = TypeVar("E", bound=enum.Enum)
T = TypeVar("T")


def _ord(s: str) -> int:
    y, mo, d = (int(x) for x in s.split("-"))
    return (datetime.date(y, mo, d) - datetime.date(1970, 1, 1)).days


def _take(d: Any, k: str, missing: str, optional: bool = False) -> Any:
    if not isinstance(d, dict) or k not in d or (d[k] is None and not optional):
        raise m.RuleInputError(missing)
    return d[k]


def _int(v: Any, what: str, past: str) -> int:
    # A whole number written with a point or an exponent, 1000.0 or 1e3, is that integer.
    if isinstance(v, float) and v.is_integer():
        v = int(v)
    if not isinstance(v, int) or isinstance(v, bool):
        raise m.RuleInputError(what, repr(v))
    # One int64 does not hold is past what a reader of every language takes, there and not later.
    if not -9223372036854775808 <= v <= 9223372036854775807:
        raise m.RuleInputError(past, repr(v))
    return v


def _bool(v: Any, what: str) -> bool:
    if not isinstance(v, bool):
        raise m.RuleInputError(what, repr(v))
    return v


def _str(v: Any, what: str) -> str:
    if not isinstance(v, str):
        raise m.RuleInputError(what, repr(v))
    return v


def _date(v: Any, what: str) -> int:
    digits = isinstance(v, str) and len(v) == 10 and v[4] == "-" and v[7] == "-"
    if not digits or not all(c in "0123456789" for c in v[:4] + v[5:7] + v[8:]):
        raise m.RuleInputError(what, repr(v))
    try:
        return _ord(v)
    except ValueError:
        raise m.RuleInputError(what, repr(v)) from None


def _enum(cls: type[E], v: Any, what: str) -> E:
    try:
        return cls(v)
    except (ValueError, TypeError):
        raise m.RuleInputError(what, repr(v)) from None


def _seq(v: Any, what: str) -> list[Any]:
    if not isinstance(v, list):
        raise m.RuleInputError(what, repr(v))
    return v


def _obj(v: Any, what: str) -> dict[str, Any]:
    if not isinstance(v, dict):
        raise m.RuleInputError(what, repr(v))
    return v


def _maybe(v: Any, read: Callable[[Any], T]) -> T | None:
    return None if v is None else read(v)


def _refused(kind: str, what: str) -> None:
    print(json.dumps({"refused": kind, "error": what}, ensure_ascii=False, separators=(",", ":")))


"#;

/// How the generated TypeScript runner reads the wire (§15.204), and the JavaScript one with its
/// types taken off: the readers of `PY_WIRE`, one per kind, with a second for an optional input
/// that takes `null`, and no generics, which the type stripping does not read.
/// The functions and the types of [`TS_WIRE`] a runner holds only where it reads them (§15.342).
const TS_WIRE_READS: &[&str] = &[
    "_take", "JsonSource", "JsonRaw", "_exact", "_raw", "_int", "_intq", "_bool", "_boolq", "_str", "_strq", "_date", "_dateq", "_enum", "_enumq", "_seq", "_obj",
    "_refused",
];

pub(crate) const TS_WIRE: &str = r#"function _take(d: unknown, k: string, missing: string, optional = false): unknown {
  if (typeof d !== "object" || d === null || Array.isArray(d) || !(k in d)) {
    throw new RuleInputError(missing);
  }
  const v = (d as Record<string, unknown>)[k];
  if (v === null && !optional) {
    throw new RuleInputError(missing);
  }
  return v;
}

type JsonSource = { source?: string };
type JsonRaw = { rawJSON?(text: string): unknown };

// An integer of the line past 2^53 is read from the digits written, which Node 21 and later hand
// a reviver, and not from the float64 JSON.parse makes of it, which holds every integer only up to
// 2^53; and a bigint is written back as its digits (§15.297). Node 20 and older hand no digits and
// have no `JSON.rawJSON`, so there a number goes as a float64 both ways, as it went before.
function _exact(_k: string, v: unknown, at: JsonSource = {}): unknown {
  if (typeof v === "number" && !Number.isSafeInteger(v) && at.source !== undefined && /^-?[0-9]+$/.test(at.source)) {
    return BigInt(at.source);
  }
  return v;
}

function _raw(_k: string, v: unknown): unknown {
  if (typeof v !== "bigint") {
    return v;
  }
  const json = JSON as unknown as JsonRaw;
  return json.rawJSON !== undefined ? json.rawJSON(String(v)) : Number(v);
}

function _int(v: unknown, what: string, past: string): bigint {
  const n = typeof v === "bigint" ? v : typeof v === "number" && Number.isInteger(v) ? BigInt(v) : null;
  if (n === null) {
    throw new RuleInputError(what, v);
  }
  // One int64 does not hold is past what a reader of every language takes, there and not later.
  if (n < -9223372036854775808n || n > 9223372036854775807n) {
    throw new RuleInputError(past, v);
  }
  return n;
}

function _intq(v: unknown, what: string, past: string): bigint | null {
  return v === null ? null : _int(v, what, past);
}

function _bool(v: unknown, what: string): boolean {
  if (typeof v !== "boolean") {
    throw new RuleInputError(what, v);
  }
  return v;
}

function _boolq(v: unknown, what: string): boolean | null {
  return v === null ? null : _bool(v, what);
}

function _str(v: unknown, what: string): string {
  if (typeof v !== "string") {
    throw new RuleInputError(what, v);
  }
  return v;
}

function _strq(v: unknown, what: string): string | null {
  return v === null ? null : _str(v, what);
}

function _date(v: unknown, what: string): bigint {
  if (typeof v !== "string" || !/^[0-9]{4}-[0-9]{2}-[0-9]{2}$/.test(v)) {
    throw new RuleInputError(what, v);
  }
  const [y, m, d] = v.split("-").map(Number);
  // The year as written: Date.UTC reads a year from 0 to 99 as one of the 1900s.
  const at = new Date(0);
  at.setUTCFullYear(y, m - 1, d);
  if (y < 1 || at.getUTCFullYear() !== y || at.getUTCMonth() !== m - 1 || at.getUTCDate() !== d) {
    throw new RuleInputError(what, v);
  }
  return BigInt(Math.round(at.getTime() / 86400000));
}

function _dateq(v: unknown, what: string): bigint | null {
  return v === null ? null : _date(v, what);
}

function _enum(values: readonly unknown[], v: unknown, what: string): unknown {
  if (!values.includes(v)) {
    throw new RuleInputError(what, v);
  }
  return v;
}

function _enumq(values: readonly unknown[], v: unknown, what: string): unknown {
  return v === null ? null : _enum(values, v, what);
}

function _seq(v: unknown, what: string): unknown[] {
  if (!Array.isArray(v)) {
    throw new RuleInputError(what, v);
  }
  return v;
}

function _obj(v: unknown, what: string): Record<string, unknown> {
  if (typeof v !== "object" || v === null || Array.isArray(v)) {
    throw new RuleInputError(what, v);
  }
  return v as Record<string, unknown>;
}

function _refused(kind: string, what: string): void {
  console.log(JSON.stringify({ refused: kind, error: what }));
}

"#;

/// How the generated Go runner reads the wire (§15.204): the readers of the Python runner's,
/// panicking with the package's own `RuleInputError`, and `refuse`, which turns that panic — or
/// either of the package's errors returned — into the line a refusal prints.
const GO_WIRE: &str = r#"// The line one call is read from, with its numbers kept as the digits that were written.
func decode(b []byte, into any) error {
	dec := json.NewDecoder(bytes.NewReader(b))
	dec.UseNumber()
	return dec.Decode(into)
}

func take(d map[string]any, k string, missing string, optional bool) any {
	v, ok := d[k]
	if !ok || (v == nil && !optional) {
		panic(&r.RuleInputError{What: missing})
	}
	return v
}

func integer(v any, what, past string) int64 {
	n, ok := v.(json.Number)
	if !ok {
		panic(&r.RuleInputError{What: what})
	}
	i, whole, fits := wholeNumber(string(n))
	if !whole {
		panic(&r.RuleInputError{What: what})
	}
	if !fits {
		panic(&r.RuleInputError{What: past})
	}
	return i
}

// wholeNumber reads a JSON number whose value is a whole number as that integer: 1000, and
// 1000.0, 1e3 or -0.0 too (§15.205). It says whether the value is whole, and then whether an
// int64 holds it: a whole number past 64 bits is out of range (§15.298).
func wholeNumber(s string) (int64, bool, bool) {
	if i, err := strconv.ParseInt(s, 10, 64); err == nil {
		return i, true, true
	}
	sign := ""
	if strings.HasPrefix(s, "-") {
		sign, s = "-", s[1:]
	}
	exp := 0
	if k := strings.IndexAny(s, "eE"); k >= 0 {
		e, err := strconv.Atoi(strings.TrimPrefix(s[k+1:], "+"))
		if ne, ok := err.(*strconv.NumError); ok && ne.Err == strconv.ErrRange {
			// An exponent past an int: zero is still zero, a value with one far below zero has a
			// fraction, and one far above zero is past 64 bits.
			e = 1 << 40
			if strings.HasPrefix(s[k+1:], "-") {
				e = -e
			}
		} else if err != nil {
			return 0, false, false
		}
		exp, s = e, s[:k]
	}
	digits := s
	if k := strings.IndexByte(s, '.'); k >= 0 {
		digits = s[:k] + s[k+1:]
		exp -= len(s) - k - 1
	}
	digits = strings.TrimLeft(digits, "0")
	for exp < 0 && strings.HasSuffix(digits, "0") {
		digits, exp = digits[:len(digits)-1], exp+1
	}
	if digits == "" {
		return 0, true, true
	}
	if exp < 0 {
		return 0, false, false
	}
	if len(digits)+exp > 19 {
		return 0, true, false
	}
	i, err := strconv.ParseInt(sign+digits+strings.Repeat("0", exp), 10, 64)
	return i, true, err == nil
}

func boolean(v any, what string) bool {
	b, ok := v.(bool)
	if !ok {
		panic(&r.RuleInputError{What: what})
	}
	return b
}

func text(v any, what string) string {
	s, ok := v.(string)
	if !ok {
		panic(&r.RuleInputError{What: what})
	}
	return s
}

func date(v any, what string) int64 {
	s, ok := v.(string)
	if !ok || len(s) != 10 {
		panic(&r.RuleInputError{What: what})
	}
	t, err := time.Parse("2006-01-02", s)
	if err != nil || t.Year() < 1 {
		panic(&r.RuleInputError{What: what})
	}
	// From the seconds, not t.Sub: a Duration holds about 292 years, and stops there.
	return t.Unix() / 86400
}

func member[T any](parse func(string) (T, bool), v any, what string) T {
	s, ok := v.(string)
	if !ok {
		panic(&r.RuleInputError{What: what})
	}
	e, ok := parse(s)
	if !ok {
		panic(&r.RuleInputError{What: what})
	}
	return e
}

func list(v any, what string) []any {
	a, ok := v.([]any)
	if !ok {
		panic(&r.RuleInputError{What: what})
	}
	return a
}

func object(v any, what string) map[string]any {
	m, ok := v.(map[string]any)
	if !ok {
		panic(&r.RuleInputError{What: what})
	}
	return m
}

// A refusal, in the place of the record: which of the two the door raised, and its sentence.
func refuse() {
	x := recover()
	if x == nil {
		return
	}
	kind, what := "", ""
	switch e := x.(type) {
	case *r.RuleInputError:
		kind, what = "input", e.What
	case *r.RuleContradictionError:
		kind, what = "contradiction", e.What
	default:
		panic(x)
	}
	var b bytes.Buffer
	enc := json.NewEncoder(&b)
	enc.SetEscapeHTML(false)
	_ = enc.Encode(struct {
		Refused string `json:"refused"`
		Error   string `json:"error"`
	}{kind, what})
	fmt.Print(b.String())
}

"#;

/// How the generated Swift runner reads the wire (§15.204): the readers of the Python runner's,
/// throwing the module's own `RuleError.input`. Foundation reads a number and a truth value
/// both as an NSNumber; the type code tells them apart (`c` is a truth value, `d` a fraction).
const SWIFT_WIRE: &str = r#"    static func _take(_ d: Any?, _ k: String, _ missing: String, _ optional: Bool = false) throws -> Any {
        guard let o = d as? [String: Any], let v = o[k], optional || !(v is NSNull) else {
            throw RuleError.input(what: missing, value: nil)
        }
        return v
    }

    static func _int(_ v: Any, _ what: String, _ past: String) throws -> Int64 {
        guard let n = v as? NSNumber else {
            throw RuleError.input(what: what, value: nil)
        }
        // Foundation on Linux gives a negative integer of 19 digits as an NSDecimalNumber, which
        // holds it exact where its `doubleValue` would round it: it is read from its digits
        // (§15.297). Digits that are no Int64 are a whole number past 64 bits, out of range as the
        // reference evaluator says of it (§15.298).
        if let dec = n as? NSDecimalNumber {
            let s = dec.stringValue
            if let i = Int64(s) {
                return i
            }
            let digits = s.hasPrefix("-") ? s.dropFirst() : Substring(s)
            guard !digits.isEmpty, digits.allSatisfy({ $0 >= "0" && $0 <= "9" }) else {
                throw RuleError.input(what: what, value: nil)
            }
            throw RuleError.input(what: past, value: nil)
        }
        let t = String(cString: n.objCType)
        // A UInt64 past Int64 is out of range: `int64Value` would keep its low 64 bits, and 2^64−1
        // would be −1 (§15.297).
        if t == "Q" && n.uint64Value > UInt64(Int64.max) {
            throw RuleError.input(what: past, value: nil)
        }
        if ["i", "s", "l", "q", "I", "S", "L", "Q"].contains(t) {
            return n.int64Value
        }
        // A whole number written with a point or an exponent, 1000.0 or 1e3, is that integer
        // (§15.205), and one no Int64 holds is out of range.
        let d = n.doubleValue
        guard t == "d" || t == "f", d.rounded() == d else {
            throw RuleError.input(what: what, value: nil)
        }
        guard d >= -9223372036854775808.0, d < 9223372036854775808.0 else {
            throw RuleError.input(what: past, value: nil)
        }
        return Int64(d)
    }

    static func _bool(_ v: Any, _ what: String) throws -> Bool {
        guard let n = v as? NSNumber, String(cString: n.objCType) == "c" else {
            throw RuleError.input(what: what, value: nil)
        }
        return n.boolValue
    }

    static func _str(_ v: Any, _ what: String) throws -> String {
        guard let s = v as? String else {
            throw RuleError.input(what: what, value: nil)
        }
        return s
    }

    static func _date(_ v: Any, _ what: String) throws -> Int64 {
        guard let s = v as? String else {
            throw RuleError.input(what: what, value: nil)
        }
        let b = Array(s.utf8)
        let digit = { (c: UInt8) in c >= 48 && c <= 57 }
        guard b.count == 10, b[4] == 45, b[7] == 45, b.enumerated().allSatisfy({ $0.offset == 4 || $0.offset == 7 || digit($0.element) }) else {
            throw RuleError.input(what: what, value: nil)
        }
        let (y, m, d) = (Int64(s.prefix(4))!, Int64(s.dropFirst(5).prefix(2))!, Int64(s.suffix(2))!)
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
        let last: Int64 = [1, 3, 5, 7, 8, 10, 12].contains(m) ? 31 : [4, 6, 9, 11].contains(m) ? 30 : m == 2 ? (leap ? 29 : 28) : 0
        guard y >= 1, d >= 1, d <= last else {
            throw RuleError.input(what: what, value: nil)
        }
        return _ord(s)
    }

    static func _enum<T: RawRepresentable>(_ t: T.Type, _ v: Any, _ what: String) throws -> T where T.RawValue == String {
        guard let s = v as? String, let e = T(rawValue: s) else {
            throw RuleError.input(what: what, value: nil)
        }
        return e
    }

    static func _seq(_ v: Any, _ what: String) throws -> [Any] {
        guard let a = v as? [Any] else {
            throw RuleError.input(what: what, value: nil)
        }
        return a
    }

    static func _obj(_ v: Any, _ what: String) throws -> [String: Any] {
        guard let o = v as? [String: Any] else {
            throw RuleError.input(what: what, value: nil)
        }
        return o
    }

    static func _maybe<T>(_ v: Any, _ read: (Any) throws -> T) rethrows -> T? {
        v is NSNull ? nil : try read(v)
    }

    /// A refusal, in the place of the record: which of the two the door raised, and its sentence.
    static func _refused(_ kind: String, _ what: String) {
        var o = "\""
        for c in what.unicodeScalars {
            switch c {
            case "\"": o += "\\\""
            case "\\": o += "\\\\"
            default:
                if c.value < 0x20 {
                    o += String(format: "\\u%04x", c.value)
                } else {
                    o.unicodeScalars.append(c)
                }
            }
        }
        print("{\"refused\":\"\(kind)\",\"error\":\(o)\"}")
    }

    /// A date as its day number from 1970-01-01, by the same civil-date arithmetic the tool
    /// uses. Foundation's own parsers carry a calendar and a time zone; this carries none.
    static func _ord(_ s: String) -> Int64 {
        let p = s.split(separator: "-").map { Int64($0) ?? 0 }
        let (y, m, d) = (p[0], p[1], p[2])
        let y2 = m <= 2 ? y - 1 : y
        let era = (y2 >= 0 ? y2 : y2 - 399) / 400
        let yoe = y2 - era * 400
        let mp = (m + 9) % 12
        let doy = (153 * mp + 2) / 5 + d - 1
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy
        return era * 146097 + doe - 719468
    }

"#;

/// The JSON string escaper the generated Rust module and the Wasm entry share.
pub(crate) const RS_JSON_STR: &str = "fn json_str(s: &str) -> String {\n    let mut o = String::from(\"\\\"\");\n    for ch in s.chars() {\n        match ch {\n            '\"' => o.push_str(\"\\\\\\\"\"),\n            '\\\\' => o.push_str(\"\\\\\\\\\"),\n            c if (c as u32) < 32 => o.push_str(&format!(\"\\\\u{:04x}\", c as u32)),\n            c => o.push(c),\n        }\n    }\n    o.push('\"');\n    o\n}\n\n";

/// The flags the Wasm module is built with, as one line for `rulec api` and as arguments for
/// `rulec test`: size over speed, no unwinding, no symbols — 22 KB for a tariff rather than
/// 1.4 MB. They are ritsu-emit's, which koyomi's module of a dates file is built with too.
pub const WASM_RUSTC_FLAGS: &str = ritsu_emit::wasm::RUSTC_FLAGS;

/// The flags the Rust runner is built with as a WASI command (§15.63), as one line for
/// `rulec api` and as arguments for `rulec test`. Nothing is tuned for size here: this is the
/// same program the native runner is, reached through stdin and stdout.
pub const WASI_RUSTC_FLAGS: &str = "-O --target wasm32-wasip1";

/// The JSON reading the generated Rust runner and the Wasm entry share: one flat object as
/// (key, value) pairs, values kept as their source text, and a reader for each wire type.
/// [`RS_JSON_HELPERS`] for a rule that reads sequences: a reader takes the sentence it refuses with
/// as anything that is a `Said`, so that a field of an element is refused at its index (§15.324),
/// and the reader of a sequence says an element that is not an object at its index. A rule with no
/// sequence keeps the readers that take a sentence of text, as before.
fn rs_json_helpers(walks: bool) -> String {
    if !walks {
        return RS_JSON_HELPERS.to_string();
    }
    let mut h = RS_JSON_HELPERS.to_string();
    for (old, new) in [
        ("missing: &'static str, optional: bool", "missing: impl Into<r::Said>, optional: bool"),
        ("what: r::Said::of(missing), value: None", "what: missing.into(), value: None"),
        ("fn refused(what: &'static str) -> r::RuleError {\n    r::RuleError::Input { what: r::Said::of(what), value: None }", "fn refused(what: impl Into<r::Said>) -> r::RuleError {\n    r::RuleError::Input { what: what.into(), value: None }"),
        ("fn int(v: &str, what: &'static str, past: &'static str)", "fn int(v: &str, what: impl Into<r::Said>, past: impl Into<r::Said>)"),
        ("fn boolean(v: &str, what: &'static str)", "fn boolean(v: &str, what: impl Into<r::Said>)"),
        ("fn text(v: &str, what: &'static str)", "fn text(v: &str, what: impl Into<r::Said>)"),
        ("fn date(v: &str, what: &'static str)", "fn date(v: &str, what: impl Into<r::Said> + Copy)"),
        ("v: &str, what: &'static str) -> Result<T, r::RuleError>", "v: &str, what: impl Into<r::Said>) -> Result<T, r::RuleError>"),
        (
            "/// The elements of an array, each an object read as its pairs: the sequence a walk reads.\nfn list(v: &str, not_sequence: &'static str, not_object: &'static str)",
            "/// The elements of an array, each an object read as its pairs: the sequence a walk reads. An\n/// element that is not an object is said at its index (§15.324).\nfn list(v: &str, not_sequence: &'static str, not_object: impl Fn(usize) -> r::Said)",
        ),
        ("return Err(refused(not_object));", "return Err(refused(not_object(out.len())));"),
    ] {
        debug_assert_eq!(h.matches(old).count(), 1, "{old}");
        h = h.replacen(old, new, 1);
    }
    h
}

pub(crate) const RS_JSON_HELPERS: &str = r##"/// One flat JSON object as (key, value) pairs, values kept as their source text.
fn fields(line: &str) -> Vec<(String, String)> {
    let b: Vec<char> = line.chars().collect();
    let mut i = 0usize;
    // Step into the object under "in" and read the pairs of the one level below it.
    while i < b.len() && !(b[i] == '"' && b[i..].starts_with(&['"', 'i', 'n', '"'])) {
        i += 1;
    }
    pairs_from(&b, i).0
}

/// The pairs of the object starting at or after `i`, and the index just past its closing
/// brace. A value that is itself an object or an array is kept whole, as its source text.
fn pairs_from(b: &[char], mut i: usize) -> (Vec<(String, String)>, usize) {
    let mut out = Vec::new();
    while i < b.len() && b[i] != '{' {
        i += 1;
    }
    i += 1;
    while i < b.len() && b[i] != '}' {
        while i < b.len() && b[i] != '"' && b[i] != '}' {
            i += 1;
        }
        if i >= b.len() || b[i] == '}' {
            break;
        }
        let (k, ni) = string_at(b, i);
        i = ni;
        while i < b.len() && b[i] != ':' {
            i += 1;
        }
        i += 1;
        while i < b.len() && b[i].is_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        // A string keeps its opening quote, so that the text `true` is not the value `true`
        // (§15.203); `s` takes it off.
        let v = if b[i] == '"' {
            let (v, ni) = string_at(b, i);
            i = ni;
            format!("\"{v}")
        } else if b[i] == '[' || b[i] == '{' {
            let (v, ni) = balanced(b, i);
            i = ni;
            v
        } else {
            let s = i;
            while i < b.len() && b[i] != ',' && b[i] != '}' {
                i += 1;
            }
            b[s..i].iter().collect::<String>().trim().to_string()
        };
        out.push((k, v));
    }
    (out, i + 1)
}

/// The source text of one bracketed value, quotes respected.
fn balanced(b: &[char], i: usize) -> (String, usize) {
    let (open, close) = if b[i] == '[' { ('[', ']') } else { ('{', '}') };
    let (mut depth, mut j) = (0i32, i);
    while j < b.len() {
        if b[j] == '"' {
            let (_, nj) = string_at(b, j);
            j = nj;
            continue;
        }
        if b[j] == open {
            depth += 1;
        }
        if b[j] == close {
            depth -= 1;
            if depth == 0 {
                j += 1;
                break;
            }
        }
        j += 1;
    }
    (b[i..j].iter().collect(), j)
}

/// The string starting at `i`, and the index just past its closing quote. Every escape JSON
/// has is read, `\u` with its surrogate pairs too: a host's encoder may write `入金` as
/// `\u5165\u91d1`, and Python's does unless told otherwise.
fn string_at(b: &[char], i: usize) -> (String, usize) {
    let hex = |j: usize| -> Option<u32> {
        b.get(j..j + 4).and_then(|h| u32::from_str_radix(&h.iter().collect::<String>(), 16).ok())
    };
    let (mut i, mut s) = (i + 1, String::new());
    while i < b.len() && b[i] != '"' {
        if b[i] == '\\' && i + 1 < b.len() {
            i += 1;
            match b[i] {
                'n' => s.push('\n'),
                't' => s.push('\t'),
                'r' => s.push('\r'),
                'b' => s.push('\u{8}'),
                'f' => s.push('\u{c}'),
                'u' => {
                    let mut c = hex(i + 1);
                    i += 4;
                    if let Some(hi) = c.filter(|h| (0xD800..0xDC00).contains(h)) {
                        if b.get(i + 1) == Some(&'\\') && b.get(i + 2) == Some(&'u') {
                            if let Some(lo) = hex(i + 3).filter(|l| (0xDC00..0xE000).contains(l)) {
                                c = Some(0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00));
                                i += 6;
                            }
                        }
                    }
                    s.push(c.and_then(char::from_u32).unwrap_or('\u{FFFD}'));
                }
                c => s.push(c),
            }
        } else {
            s.push(b[i]);
        }
        i += 1;
    }
    (s, i + 1)
}

/// The value under `k` as it was written — a string with its opening quote, anything else as
/// its text — or the input refused as missing: a key the object does not have, or `null` for an
/// input that is not optional (§15.204).
fn take<'a>(d: &'a [(String, String)], k: &str, missing: &'static str, optional: bool) -> Result<&'a str, r::RuleError> {
    match d.iter().find(|(a, _)| a == k) {
        Some((_, v)) if optional || v != "null" => Ok(v.as_str()),
        _ => Err(r::RuleError::Input { what: r::Said::of(missing), value: None }),
    }
}

fn refused(what: &'static str) -> r::RuleError {
    r::RuleError::Input { what: r::Said::of(what), value: None }
}

/// A JSON number whose value is a whole number, as that integer: digits with a minus before them
/// at most, or the same value written with a point or an exponent — `1000.0`, `1e3`, `-0.0`
/// (§15.205). A string keeps its quote, so `"12"` is not one, and neither is `1.5`. A whole number
/// past 64 bits is out of range, as the reference evaluator says of it (§15.298).
fn int(v: &str, what: &'static str, past: &'static str) -> Result<i64, r::RuleError> {
    match whole(v) {
        Whole::Int(n) => Ok(n),
        Whole::Past => Err(refused(past)),
        Whole::No => Err(refused(what)),
    }
}

/// What a JSON number is as a whole number: the i64 it is, a whole number past what an i64 holds,
/// or no whole number.
enum Whole {
    Int(i64),
    Past,
    No,
}

fn whole(v: &str) -> Whole {
    let b = v.as_bytes();
    let (neg, b) = match b.first() {
        Some(b'-') => (true, &b[1..]),
        _ => (false, b),
    };
    let e = b.iter().position(|&c| c == b'e' || c == b'E').unwrap_or(b.len());
    let (mant, ex) = b.split_at(e);
    // The exponent, held below 2^40: a whole value that is not zero is past 64 bits from 10^19 on.
    let mut shift: i64 = 0;
    if let Some((_, ex)) = ex.split_first() {
        let (down, digits) = match ex.first() {
            Some(b'-') => (true, &ex[1..]),
            Some(b'+') => (false, &ex[1..]),
            _ => (false, ex),
        };
        if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
            return Whole::No;
        }
        for &d in digits {
            shift = (shift * 10 + i64::from(d - b'0')).min(1 << 40);
        }
        if down {
            shift = -shift;
        }
    }
    let point = mant.iter().position(|&c| c == b'.');
    let (int, frac) = match point {
        Some(k) => (&mant[..k], &mant[k + 1..]),
        None => (mant, &mant[mant.len()..]),
    };
    if int.is_empty() || (point.is_some() && frac.is_empty()) || !int.iter().chain(frac).all(u8::is_ascii_digit) {
        return Whole::No;
    }
    // The digits from the first that is not zero to the last, and the power of ten they are
    // multiplied by: the zeros after them add to it, and each digit after the point takes one
    // away. Whether the value is whole is known before its size is looked at.
    let digits = || int.iter().chain(frac).copied();
    let Some(first) = digits().position(|d| d != b'0') else { return Whole::Int(0) };
    let zeros = digits().rev().position(|d| d != b'0').unwrap_or(0);
    let shown = int.len() + frac.len() - zeros - first;
    let shift = shift - frac.len() as i64 + zeros as i64;
    if shift < 0 {
        return Whole::No;
    }
    if shown as i64 + shift > 19 {
        return Whole::Past;
    }
    // The digits as a negative number, so that -2^63 fits.
    let mut n: i64 = 0;
    for d in digits().skip(first).take(shown) {
        let Some(m) = n.checked_mul(10).and_then(|x| x.checked_sub(i64::from(d - b'0'))) else { return Whole::Past };
        n = m;
    }
    for _ in 0..shift {
        let Some(m) = n.checked_mul(10) else { return Whole::Past };
        n = m;
    }
    let n = if neg { Some(n) } else { n.checked_neg() };
    n.map_or(Whole::Past, Whole::Int)
}

/// JSON's `true` or `false` and nothing else (§15.203).
fn boolean(v: &str, what: &'static str) -> Result<bool, r::RuleError> {
    match v {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(refused(what)),
    }
}

fn text(v: &str, what: &'static str) -> Result<String, r::RuleError> {
    v.strip_prefix('"').map(str::to_string).ok_or_else(|| refused(what))
}

/// A `YYYY-MM-DD` string of a day the calendar has, as its day number.
fn date(v: &str, what: &'static str) -> Result<i64, r::RuleError> {
    let s = v.strip_prefix('"').ok_or_else(|| refused(what))?;
    let b = s.as_bytes();
    let shape = b.len() == 10 && b[4] == b'-' && b[7] == b'-';
    if !shape || !b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()) {
        return Err(refused(what));
    }
    let (y, m, d): (i64, i64, i64) = (s[0..4].parse().unwrap_or(0), s[5..7].parse().unwrap_or(0), s[8..10].parse().unwrap_or(0));
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let last = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return Err(refused(what)),
    };
    if y < 1 || !(1..=last).contains(&d) {
        return Err(refused(what));
    }
    Ok(ord(s))
}

fn member<T>(parse: fn(&str) -> Option<T>, v: &str, what: &'static str) -> Result<T, r::RuleError> {
    v.strip_prefix('"').and_then(parse).ok_or_else(|| refused(what))
}

/// The elements of an array, each an object read as its pairs: the sequence a walk reads.
fn list(v: &str, not_sequence: &'static str, not_object: &'static str) -> Result<Vec<Vec<(String, String)>>, r::RuleError> {
    let b: Vec<char> = v.chars().collect();
    if b.first() != Some(&'[') {
        return Err(refused(not_sequence));
    }
    let (mut i, mut out) = (1usize, Vec::new());
    loop {
        while i < b.len() && (b[i].is_whitespace() || b[i] == ',') {
            i += 1;
        }
        if i >= b.len() || b[i] == ']' {
            return Ok(out);
        }
        if b[i] != '{' {
            return Err(refused(not_object));
        }
        let (p, ni) = pairs_from(&b, i);
        out.push(p);
        i = ni;
    }
}

/// A date as its day number from 1970-01-01, by the same civil-date arithmetic the tool uses.
fn ord(s: &str) -> i64 {
    let p: Vec<i64> = s.split('-').map(|x| x.parse().unwrap_or(0)).collect();
    let (y, m, d) = (p[0], p[1], p[2]);
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = if y2 >= 0 { y2 } else { y2 - 399 } / 400;
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

"##;

/// The canonical ABI of `call: func(input: string) -> string`, written once for every rule: the
/// three functions of ritsu-emit's `wasm` (§15.278), which koyomi's module of a dates file and
/// kaname's package are written with too, handing the input to `answer`, and the refusals the
/// module answers in the place of a record. Their comments are in English, as the rest of the
/// generated code's are.
fn rs_wasm_abi() -> String {
    format!(
        "{}{}",
        ritsu_emit::wasm::abi(ritsu_base::text::Lang::En, "answer(text)", "not_utf8()"),
        RS_WASM_REFUSAL.replace("@NOT_UTF8@", &str_lit(&crate::door::not_utf8()))
    )
}

/// What the module answers in the place of a record: the refusal of one of the rule's two errors,
/// and that of bytes that are not UTF-8, whose sentence is put in for `@NOT_UTF8@`.
const RS_WASM_REFUSAL: &str = r##"/// A refusal, as the line the runners print in the place of a record (§15.204): which of the
/// module's two errors it is, and its sentence, the value after it.
fn refusal(e: &r::RuleError) -> String {
    let kind = match e {
        r::RuleError::Input { .. } => "input",
        r::RuleError::Contradiction { .. } => "contradiction",
    };
    format!("{{\"refused\":\"{kind}\",\"error\":{}}}", json_str(&e.to_string()))
}

/// The refusal of bytes that are not UTF-8, which the module answers before it reads an input.
fn not_utf8() -> String {
    format!("{{\"refused\":\"input\",\"error\":{}}}", json_str(@NOT_UTF8@))
}

"##;

/// The package the `.wit` declares, `rulec:<world>@<version>`: the world as ritsu-emit's
/// `wasm::world` spells the alias, with `-` for a run of `_`, none for one at either end, and `%`
/// before a word WIT keeps for itself (§15.278), and the rule's version as a semver.
fn wit_package(alias: &str, version: &str) -> String {
    ritsu_emit::wasm::package("rulec", alias, Some(&wit_version(version)))
}

/// `1` as `1.0.0`: a WIT package version is a semver.
fn wit_version(v: &str) -> String {
    let mut parts: Vec<String> = v
        .split('.')
        .map(|p| p.chars().filter(|c| c.is_ascii_digit()).collect::<String>())
        .map(|p| if p.is_empty() { "0".to_string() } else { p })
        .collect();
    parts.truncate(3);
    while parts.len() < 3 {
        parts.push("0".to_string());
    }
    parts.join(".")
}

/// The Node runner `rulec test` drives: the module built from `<alias>_wasm.rs`, one vector per
/// line of stdin through `call`, the record line out.
pub fn wasm_runner_js(alias: &str) -> String {
    wasm_runner_with(alias, "null", "null", "")
}

/// The runner with the machine's constants handed to it (§15.148). The module's one door is
/// `call`, so it keeps no constants of its own; the line the traces begin with is the rule's.
/// `more` lists the values carried along with the state (§15.217), as `[input, output]` pairs,
/// or is empty.
fn wasm_runner_with(alias: &str, machine: &str, carry: &str, more: &str) -> String {
    let (decl, start, pass, next) = if more.is_empty() {
        (String::new(), String::new(), String::new(), String::new())
    } else {
        (
            format!("\nconst MORE = {more};\nlet held = {{}};"),
            "\n    if (v.step === \"start\") held = { ...v.carried };".to_string(),
            ", ...held".to_string(),
            "\n    const obs = JSON.parse(out, exact).observed;\n    held = Object.fromEntries(MORE.map(([i, o]) => [i, obs[o]]));".to_string(),
        )
    };
    format!(
        "// Code generated by rulec {}. DO NOT EDIT.\n{}",
        ritsu_emit::header::VERSION,
        WASM_RUNNER_JS
            .replace("@CALL@", ritsu_emit::wasm::NODE_CALL)
            .replace("ALIAS", alias)
            .replace("@MACHINE@", machine)
            .replace("@CARRY@", carry)
            .replace("@MORE_DECL@", &decl)
            .replace("@MORE_START@", &start)
            .replace("@MORE_PASS@", &pass)
            .replace("@MORE_NEXT@", &next)
    )
}

impl<'a> Gen<'a> {
    /// `wasm/<alias>_runner.mjs`, with the machine's constants when there is a machine.
    pub fn wasm_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        match (self.machine_consts(), self.carried()) {
            (Some((_, init, fins)), Some((cin, cout))) => {
                let more = self.more_carried();
                let meta = format!(
                    "{{\"initial\":{},\"final\":[{}]{}}}",
                    crate::json::quote(&init),
                    fins.iter().map(|f| crate::json::quote(f)).collect::<Vec<_>>().join(","),
                    if more.is_empty() {
                        String::new()
                    } else {
                        format!(",\"carried\":{{{}}}", more.iter().map(|mc| format!("{}:{}", mc.key(), mc.wire)).collect::<Vec<_>>().join(","))
                    }
                );
                let pairs = if more.is_empty() {
                    String::new()
                } else {
                    format!("[{}]", more.iter().map(|mc| format!("[{}, {}]", mc.key(), crate::json::quote(&mc.output))).collect::<Vec<_>>().join(", "))
                };
                wasm_runner_with(&alias, &crate::json::quote(&meta), &format!("[{}, {}]", crate::json::quote(&cin), crate::json::quote(&cout)), &pairs)
            }
            _ => wasm_runner_js(&alias),
        }
    }
}

const WASM_RUNNER_JS: &str = r#"import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const { instance } = await WebAssembly.instantiate(readFileSync(join(here, "ALIAS.wasm")), {});
const ex = instance.exports;
const enc = new TextEncoder();
const dec = new TextDecoder();

// One call through the canonical ABI of `call: func(input: string) -> string`. The views on
// the memory are taken after the call, because the memory may have grown during it.
@CALL@
// A machine's constants, and which input the answer carries back as which.
const MACHINE = @MACHINE@;
const CARRY = @CARRY@;
let state = null;@MORE_DECL@

// An integer past 2^53 is read from the digits written, which Node 21 and later hand a reviver,
// and written back as its digits, so a trace hands the module what it answered (§15.297). Node 20
// and older hand no digits and have no `JSON.rawJSON`: there a number goes as a float64, as before.
const exact = (_k, v, at = {}) =>
  typeof v === "number" && !Number.isSafeInteger(v) && at.source !== undefined && /^-?[0-9]+$/.test(at.source) ? BigInt(at.source) : v;
const raw = (_k, v) => (typeof v !== "bigint" ? v : JSON.rawJSON !== undefined ? JSON.rawJSON(String(v)) : Number(v));

for (const line of readFileSync(0, "utf8").split("\n")) {
  if (!line.trim()) continue;
  const v = JSON.parse(line, exact);
  if (MACHINE !== null && "machine" in v) {
    console.log(MACHINE);
    continue;
  }
  let text = line;
  if (CARRY !== null && "step" in v) {
    // A trace's call is passed the state the module answered to the call before.
    if (v.step === "start") state = v.state;@MORE_START@
    text = JSON.stringify({ in: { ...v.in, [CARRY[0]]: state@MORE_PASS@ } }, raw);
  }
  const out = call(text);
  console.log(out);
  // A refusal is the module's own line, in the place of the record (§15.204).
  if (out.startsWith('{"refused":')) continue;
  if (CARRY !== null && "step" in v) {
    state = JSON.parse(out, exact).observed[CARRY[1]];@MORE_NEXT@
  }
}
"#;

/// The rounding helpers as a module of their own, so that the Wasm side runs the same unit
/// vectors as every other language (§8.5).
pub fn round_wasm_rust(u: &Uses) -> String {
    let mut o = format!(
        "// Code generated by rulec {}. DO NOT EDIT.\n// {}\n#![allow(dead_code, unused_parens, clippy::all)]\n",
        ritsu_emit::header::VERSION,
        tr!(
            "丸めの五つのモード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five rounding modes, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str(round_rs().trim_start_matches('\n'));
    o.push_str(&arith::rs_helpers(u));
    o.push_str(&arith::wasm_exports(u));
    for m in ["down", "up", "half", "half_down", "bankers"] {
        o.push_str(&format!("\n#[no_mangle]\npub extern \"C\" fn rulec_round_{m}(x: i64, g: i64) -> i64 {{\n    round_{m}(x, g)\n}}\n"));
    }
    o
}

pub fn round_tests_wasm_js(u: &Uses) -> String {
    let mut o = format!(
        "// Code generated by rulec {}. DO NOT EDIT.\n// {}\nimport {{ readFileSync }} from \"node:fs\";\nimport {{ dirname, join }} from \"node:path\";\nimport {{ fileURLToPath }} from \"node:url\";\n\nconst here = dirname(fileURLToPath(import.meta.url));\nconst {{ instance }} = await WebAssembly.instantiate(readFileSync(join(here, \"_round.wasm\")), {{}});\nconst ex = instance.exports;\n\nconst CASES = [\n",
        ritsu_emit::header::VERSION,
        tr!(
            "丸めの五つのモード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five rounding modes, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("  [\"{}\", {x}n, {g}n, {want}n],\n", mode_fn(m)));
    }
    o.push_str(
        "];\n\nlet bad = 0;\nfor (const [mode, x, g, want] of CASES) {\n  const got = ex[\"rulec_round_\" + mode](x, g);\n  if (got !== want) {\n    console.log(`NG ${mode}(${x}, ${g}) = ${got}, want ${want}`);\n    bad += 1;\n  }\n}\n",
    );
    let (arith, n) = arith::wasm_tests(u);
    o.push_str(&arith);
    o.push_str("if (bad > 0) {\n  process.exit(1);\n}\n");
    let count = if n == 0 { "${CASES.length}".to_string() } else { "${CASES.length + ARITH.length}".to_string() };
    let line = tr!("ok {{}} 件", "ok {{}} cases").replace("{}", &count);
    o.push_str(&format!("console.log(`{line}`);\n"));
    o
}

pub fn round_tests_rust(u: &Uses) -> String {
    let mut o = format!(
        "// Code generated by rulec {}. DO NOT EDIT.\n// {}\n#![allow(unused_parens, dead_code, clippy::all)]\n",
        ritsu_emit::header::VERSION,
        tr!(
            "丸めの五つのモード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five rounding modes, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str(round_rs().trim_start_matches('\n'));
    o.push_str(&arith::rs_helpers(u));
    o.push_str("\nconst CASES: &[(&str, i64, i64, i64)] = &[\n");
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("    (\"{}\", {x}, {g}, {want}),\n", mode_fn(m)));
    }
    o.push_str(
        "];\n\nfn main() {\n    let mut bad = 0;\n    for (mode, x, g, want) in CASES {\n        \
         let got = match *mode {\n            \"down\" => round_down(*x, *g),\n            \
         \"up\" => round_up(*x, *g),\n            \"half\" => round_half(*x, *g),\n            \
         \"half_down\" => round_half_down(*x, *g),\n            _ => round_bankers(*x, *g),\n        };\n        if got != *want {\n            \
         println!(\"NG {mode}({x}, {g}) = {got}, want {want}\");\n            bad += 1;\n        \
         }\n    }\n",
    );
    let (arith, n) = arith::rs_tests(u);
    o.push_str(&arith);
    o.push_str("    if bad > 0 {\n        std::process::exit(1);\n    }\n");
    let line = tr!("ok {{}} 件", "ok {{}} cases");
    let count = if n == 0 { "CASES.len()".to_string() } else { format!("CASES.len() + {n}") };
    o.push_str(&format!("    println!(\"{line}\", {count});\n}}\n"));
    o
}

// ---------------------------------------------------------------------------
// Unit vectors for the rounding helpers (§8.5)
// ---------------------------------------------------------------------------

/// Five modes × grids × values. Negative values and exact halves are always included.
/// Table-level agreement alone would hide a broken helper on a table that never produces a
/// fraction.
fn round_cases() -> Vec<(RoundMode, i128, i128, i128)> {
    let mut out = Vec::new();
    for m in [RoundMode::Down, RoundMode::Up, RoundMode::Half, RoundMode::HalfDown, RoundMode::Bankers] {
        for g in [1i128, 10, 100] {
            for x in [-25i128, -20, -15, -11, -10, -5, -1, 0, 1, 5, 10, 11, 15, 20, 25, 105, 150, 250] {
                let want = Rat::int(x).round_to(m, Rat::int(g));
                out.push((m, x, g, want.num / want.den));
            }
        }
    }
    out
}

pub fn round_tests_python(u: &Uses) -> String {
    let mut o = format!(
        "# Code generated by rulec {}. DO NOT EDIT.\n\
         # {}\n\
         import sys\n\n\n",
        ritsu_emit::header::VERSION,
        tr!(
            "丸めの五つのモード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five rounding modes, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str(round_py().trim_start_matches('\n'));
    o.push_str(&arith::py_helpers(u));
    o.push_str("\nCASES = [\n");
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("    (\"{}\", {x}, {g}, {want}),\n", mode_fn(m)));
    }
    o.push_str("]\n\nFN = {\n    \"down\": _round_down,\n    \"up\": _round_up,\n    \"half\": _round_half,\n    \"half_down\": _round_half_down,\n    \"bankers\": _round_bankers,\n}\n\n");
    o.push_str(
        "bad = 0\nfor mode, x, g, want in CASES:\n    \
         got = FN[mode](x, g)\n    \
         if got != want:\n        \
             print(f\"NG {mode}({x}, {g}) = {got}, want {want}\")\n        \
             bad += 1\n",
    );
    let (arith, n) = arith::py_tests(u);
    o.push_str(&arith);
    o.push_str("if bad:\n    sys.exit(1)\n");
    let count = if n == 0 { "len(CASES)" } else { "len(CASES) + len(ARITH)" };
    o.push_str(&format!("print(f\"{}\")\n", tr!("ok {{{count}}} 件", "ok {{{count}}} cases")));
    o
}

pub fn round_tests_typescript(u: &Uses) -> String {
    let mut o = format!(
        "// Code generated by rulec {}. DO NOT EDIT.\n// {}\n",
        ritsu_emit::header::VERSION,
        tr!(
            "丸めの五つのモード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five rounding modes, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str(round_ts().trim_start_matches('\n'));
    o.push_str(&arith::ts_helpers(u));
    o.push_str("\nconst CASES: [string, bigint, bigint, bigint][] = [\n");
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("  [\"{}\", {x}n, {g}n, {want}n],\n", mode_fn(m)));
    }
    o.push_str(
        "];\n\nconst FN: Record<string, (x: bigint, g: bigint) => bigint> = {\n  \
         down: _roundDown,\n  up: _roundUp,\n  half: _roundHalf,\n  half_down: _roundHalfDown,\n  bankers: _roundBankers,\n};\n\n\
         let bad = 0;\n\
         for (const [mode, x, g, want] of CASES) {\n  \
             const got = FN[mode](x, g);\n  \
             if (got !== want) {\n    \
                 console.log(`NG ${mode}(${x}, ${g}) = ${got}, want ${want}`);\n    \
                 bad += 1;\n  }\n}\n",
    );
    let (arith, n) = arith::ts_tests(u);
    o.push_str(&arith);
    o.push_str("if (bad > 0) {\n  process.exit(1);\n}\n");
    let count = if n == 0 { "CASES.length" } else { "CASES.length + ARITH.length" };
    o.push_str(&format!("console.log(`{}`);\n", tr!("ok ${{{count}}} 件", "ok ${{{count}}} cases")));
    // The two comparisons, which the cases do not call (§15.342).
    from::ts_without_unread(&o, TS_CALLED)
}

pub fn round_tests_go(pkg: &str, u: &Uses) -> String {
    let mut o = format!(
        "// Code generated by rulec {}. DO NOT EDIT.\n\
         // {}\n\
         package {pkg}\n\nimport \"testing\"\n\n\
         func TestRoundingModes(t *testing.T) {{\n\t\
         cases := []struct {{\n\t\tmode{CELL}string\n\t\tx, g, want{CELL}int64\n\t}}{{\n",
        ritsu_emit::header::VERSION,
        tr!(
            "丸めの五つのモード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five rounding modes, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("\t\t{{{:?}, {x}, {g}, {want}}},\n", mode_fn(m)));
    }
    o.push_str(
        "\t}\n\tfor _, c := range cases {\n\t\tvar got int64\n\t\tswitch c.mode {\n\t\t\
         case \"down\":\n\t\t\tgot = roundDown(c.x, c.g)\n\t\t\
         case \"up\":\n\t\t\tgot = roundUp(c.x, c.g)\n\t\t\
         case \"half\":\n\t\t\tgot = roundHalf(c.x, c.g)\n\t\t\
         case \"half_down\":\n\t\t\tgot = roundHalfDown(c.x, c.g)\n\t\t\
         case \"bankers\":\n\t\t\tgot = roundBankers(c.x, c.g)\n\t\t}\n\t\t\
         if got != c.want {\n\t\t\tt.Errorf(\"%s(%d, %d) = %d, want %d\", c.mode, c.x, c.g, got, c.want)\n\t\t}\n\t}\n}\n",
    );
    o.push_str(&arith::go_tests(u).0);
    align(&o)
}


// ── The API inventory (`rulec api`) ──────────────────────────────────────
//
// What a caller has to know to invoke the generated code: the module and function names,
// the parameters in order with their brands, units and ranges, the outputs with their
// rounding, the enum members under the spelling the generated code gives them, and the
// errors it can raise.
//
// It lives here, next to the generator, on purpose. Anywhere else it would be a second
// description of the same thing and would start drifting the first time a name changes; here
// it is built from the same `pub_name`, `pascal`, `py_ty` and `go_ty` the emitters use, and a
// test runs the generated code of every language against it.

impl Gen<'_> {
    /// The projection, for the inventory: the contracts, where each input stands in them,
    /// and the function that reads them in each language that has one. `null` for a rule
    /// that takes its inputs as they come.
    fn projection_json(&self) -> String {
        if !self.projects() {
            return "null".into();
        }
        let alias = pub_name(&self.f.name);
        let shapes: Vec<String> = self
            .f
            .shapes
            .iter()
            .map(|d| {
                crate::json::Obj::new()
                    .str("name", &d.name.text)
                    .str("alias", &pub_name_of_shape(d))
                    .str("kind", match d.source {
                        crate::ast::EnumSource::Proto => crate::kw::PROTO,
                        crate::ast::EnumSource::JsonSchema => crate::kw::JSONSCHEMA,
                    })
                    .str("file", &d.file)
                    .str("at", &d.at)
                    .finish()
            })
            .collect();
        let inputs: Vec<String> = self
            .f
            .inputs
            .iter()
            .filter_map(|i| {
                let p = i.from.as_ref()?;
                Some(
                    crate::json::Obj::new()
                        .str("name", &i.name.text)
                        .str("alias", &pub_name(&i.name))
                        .str("from", &p.text())
                        .finish(),
                )
            })
            .collect();
        // The sequences taken out of an object (§15.223): where each comes from, and where each
        // field stands in an element. The list is empty for a rule that takes none, and never
        // absent.
        let elements: Vec<String> = self
            .f
            .elements
            .iter()
            .filter_map(|el| {
                let p = el.from.as_ref()?;
                let fields: Vec<String> = el
                    .fields
                    .iter()
                    .filter_map(|fd| {
                        let q = fd.from.as_ref()?;
                        Some(crate::json::Obj::new().str("name", &fd.name.text).str("alias", &pub_name(&fd.name)).str("from", &q.text()).finish())
                    })
                    .collect();
                Some(
                    crate::json::Obj::new()
                        .str("name", &el.name.text)
                        .str("alias", &pub_name(&el.name))
                        .str("from", &p.text())
                        .raw("fields", crate::json::arr(&fields))
                        .finish(),
                )
            })
            .collect();
        let one = |f: &str, sig: String| crate::json::Obj::new().str("function", f).str("signature", &sig).finish();
        let py_ret = self.py_ret();
        let py_params = self.py_from_params();
        let ts_ret = match self.f.outputs.len() {
            1 => self.ts_ty(&self.ty_of(&self.f.outputs[0].name.text)),
            _ => "Output".into(),
        };
        let ts_params = self.ts_from_params();
        let plain = self.plain_from_params();
        let fname = self.ident(&alias);
        crate::json::Obj::new()
            .raw("shapes", crate::json::arr(&shapes))
            .raw("inputs", crate::json::arr(&inputs))
            .raw("elements", crate::json::arr(&elements))
            .raw("python", one(&format!("{alias}_from"), format!("def {alias}_from({}) -> {py_ret}:", py_params.join(", "))))
            .raw(
                "typescript",
                one(&format!("{fname}_from"), format!("export function {fname}_from({}): {ts_ret}", ts_params.join(", "))),
            )
            .raw(
                "javascript",
                one(&format!("{fname}_from"), format!("export function {fname}_from({})", plain.join(", "))),
            )
            .raw(
                "ruby",
                one(&format!("{fname}_from"), format!("{}.{fname}_from({})", self.rb_module(), plain.join(", "))),
            )
            .raw("php", one(&format!("{alias}_from"), self.php_from_signature()))
            .raw("without", {
                let mut without = crate::json::Obj::new();
                for b in crate::backend::ALL.iter().filter(|b| !b.projects) {
                    without = without.str(b.id, &proj_why(b.id));
                }
                without.finish()
            })
            .finish()
    }

    /// The unit as it is written in the rule (`円`, `g`, `%`), or absent for a type that has
    /// none.
    fn unit_of(ty: &Ty) -> Option<String> {
        match ty {
            Ty::Money { cur, .. } => Some(cur.clone()),
            Ty::Qty { unit, .. } => Some(unit.clone()),
            Ty::Rate => Some("%".into()),
            Ty::Opt(t) => Self::unit_of(t),
            _ => None,
        }
    }

    /// The range of a parameter or a field, as the integers that travel ([`Checked::wire_range`]).
    /// For an input it is **the range the generated guard actually enforces**, not a second
    /// computation of it. Both entry guards above take `lo.num` and `hi.num`, so this does too: an
    /// inventory that disagreed with the guard would send a caller values the code then rejects.
    /// A test holds these two numbers to the text of the generated guard (§8.6). For an output it
    /// is the range the rule's answers lie in, the one the port gives the other languages and the
    /// schema states (§15.271): a range written on the output's line, or what decides the output
    /// with both ends rounded the way it rounds, a `result` included. The generated code does not
    /// hold its own answers to it; a caller that checks them does. An input of an ordered enum held
    /// to a range has the same keys, with the first and the last value its range lets in, by the
    /// names that travel: the guard compares the value's place with those two (§15.210, §15.251).
    fn range_json(&self, name: &str) -> Option<String> {
        if let Some(vs) = self.c.enum_ranges.get(name) {
            return Some(crate::json::Obj::new().str("min", vs.first()?).str("max", vs.last()?).finish());
        }
        let (lo, hi) = self.c.wire_range(name)?;
        Some(crate::json::Obj::new().int("min", lo?).int("max", hi?).finish())
    }

    /// `{"mode":"down","grid":1}` — the grid as the integer the value itself travels as, so it
    /// can be compared against a value straight away. For a rate that is steps, the scale
    /// `range_json` puts `min` and `max` on: `round half_up(0.1%)` came out as `0` when the grid
    /// was read as a fraction and cut to an integer.
    fn rounding_json(&self, od: &OutDecl) -> Option<String> {
        let rd = od.rounding.as_ref()?;
        let ty = self.ty_of(&od.name.text);
        let g = crate::types::lit_value_in_pub(&rd.grid, &ty)?;
        let sc = self.c.wire_scale(&od.name.text);
        Some(
            crate::json::Obj::new()
                .str("mode", &rd.mode)
                .int("grid", crate::types::wire_int(g, sc))
                .finish(),
        )
    }

    /// One parameter or field, described the same way on both sides.
    fn value_json(&self, jp: &str, alias: &str, ty_name: &str, ty: &Ty) -> String {
        crate::json::Obj::new()
            .str("name", jp)
            .str("alias", alias)
            .str("type", ty_name)
            .opt_str("unit", Self::unit_of(ty))
            .opt_raw("range", self.range_json(jp))
            .bool("optional", matches!(ty, Ty::Opt(_)))
            .finish()
    }

    /// The sequence a walk reads, as one entry of the inventory's parameter list: the
    /// parameter itself, and the fields one element carries, each stated the way an input is
    /// (§15.56). A caller that reads the inventory rather than the code needs both.
    fn elements_json(
        &self,
        ty_name: &str,
        alias: impl Fn(&crate::ast::Name) -> String,
        fty: impl Fn(&Ty) -> String,
    ) -> Vec<String> {
        // One entry per sequence, its type named as the language names it (§15.219): the
        // template's `Element` is the sequence's own element type.
        let mut out = Vec::new();
        for (s, el) in self.f.elements.iter().enumerate() {
            let fields: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    let one = self.value_json(&fd.name.text, &alias(&fd.name), &fty(&ty), &ty);
                    // A field no two elements may share says so (§15.290); the others say nothing.
                    match fd.unique {
                        Some(_) => format!("{},\"unique\":true}}", one.strip_suffix('}').unwrap_or(&one)),
                        None => one,
                    }
                })
                .collect();
            let head = crate::json::Obj::new()
                .str("name", &el.name.text)
                .str("alias", &alias(&el.name))
                .str("type", &ty_name.replace("Element", &self.element_type(s)))
                .bool("optional", false)
                .finish();
            out.push(format!("{},\"elements\":{}}}", head.trim_end_matches('}'), crate::json::arr(&fields)));
        }
        out
    }

    /// The enums, under the spelling each language gives their members: each enum's name and
    /// class, and each value's name and member. An enum two names share a class with is listed
    /// once.
    fn enum_list(&self, member: impl Fn(&str, &str) -> String) -> Vec<ritsu_ports::CallEnum> {
        let mut out = Vec::new();
        let mut done: Vec<String> = Vec::new();
        for (jp, ascii) in &self.enum_names {
            if done.contains(ascii) {
                continue;
            }
            done.push(ascii.clone());
            let Some(vals) = self.c.enums.get(jp) else { continue };
            let values = vals
                .iter()
                .map(|v| {
                    let a = self.value_names.get(&(jp.clone(), v.clone())).map(|(_, a)| a.clone()).unwrap_or_else(|| v.clone());
                    (v.clone(), member(ascii, &a))
                })
                .collect();
            out.push(ritsu_ports::CallEnum { name: jp.clone(), alias: ascii.clone(), values });
        }
        out
    }

    /// The enums, under the spelling each language gives their members. An ordered enum says so
    /// and names what gives a value's place in that language (§15.210); one that gives display
    /// names has each value's, and names what gives it (§15.211); one read from an `enums` file
    /// names the file and the module its type is in (§15.212).
    fn enums_json(&self, lang: &str, member: impl Fn(&str, &str) -> String) -> String {
        let out: Vec<String> = self
            .enum_list(member)
            .iter()
            .map(|e| {
                let en = &e.name;
                let members: Vec<String> = e
                    .values
                    .iter()
                    .map(|(v, a)| {
                        let label = self.c.labels.get(en).map(|_| self.c.shown_value(en, v));
                        crate::json::Obj::new().str("name", v).str("alias", a).opt_str("label", label.as_deref()).finish()
                    })
                    .collect();
                let (rank, label) = self.enum_fns(lang, en);
                let mut o = crate::json::Obj::new().str("name", en).str("alias", &e.alias).raw("values", crate::json::arr(&members));
                if self.c.ordered.contains(en) {
                    o = o.bool("ordered", true).opt_str("rank", rank.as_deref());
                }
                if self.c.labels.contains_key(en) {
                    o = o.opt_str("label", label.as_deref());
                }
                if let Some(owner) = self.c.owners.get(en) {
                    o = o.str("from", &owner.file).str("module", &pub_name(&owner.module));
                }
                o.finish()
            })
            .collect();
        crate::json::arr(&out)
    }

    /// What gives a value's place in its order and its display name in one language (§15.210,
    /// §15.211): a function, a method or a property, as the generated code spells it. `None`
    /// where the language needs none — Go's constants are the order themselves.
    fn enum_fns(&self, lang: &str, en: &str) -> (Option<String>, Option<String>) {
        let t = self.enum_type(en);
        let owner = self.c.owners.get(en).map(|o| pub_name(&o.module));
        // the file's module's name for a function of an enum of an `enums` file (§15.326)
        let qualified = |lang: &str, f: String| match &owner {
            Some(m) => format!("{m}.{}", self.theirs(lang, en, &f)),
            None => f,
        };
        match lang {
            "python" => (Some(qualified("python", self.py_rank_fn(en))), Some(qualified("python", self.py_label_fn(en)))),
            "typescript" | "javascript" => (Some(qualified("typescript", self.ts_rank_fn(en))), Some(qualified("typescript", self.ts_label_fn(en)))),
            "rust" => (Some(format!("{t}::rank")), Some(format!("{t}::label"))),
            "ruby" => {
                let m = self.rb_module();
                let q = |f: String| if f.contains('.') { f } else { format!("{m}.{f}") };
                (Some(q(self.rb_rank_fn(en))), Some(q(self.rb_label_fn(en))))
            }
            "php" => (Some(self.php_fn_of_pub(en, "rank")), Some(self.php_fn_of_pub(en, "label"))),
            "swift" => (Some(format!("{t}.rank")), Some(format!("{t}.label"))),
            "java" => (Some(format!("{}.ordinal", crate::codegen::java_class(&t))), Some(format!("{}.label", crate::codegen::java_class(&t)))),
            "go" => (None, Some(format!("{t}.Label"))),
            _ => (None, None),
        }
    }

    /// Every enum the rule knows, by name: its class in the generated code, and each value with
    /// the alias the generated code builds the member's name from (`Gold`, which TypeScript and
    /// Python write `GOLD`); the enum's public name as the rule writes it, whether its values are
    /// ordered (§15.210), and the `enums` file it is read from whole (§15.212). For ritsu's port of
    /// rules (`crate::ports`).
    pub fn rule_enums(&self) -> Vec<ritsu_ports::RuleEnum> {
        let mut names: Vec<&String> = self.c.enums.keys().collect();
        names.sort();
        names
            .into_iter()
            .map(|jp| {
                // the enum as the rule declares it, or reads it whole from an `enums` file (§15.212)
                let decl = self.f.enums.iter().find(|e| e.name.text == *jp);
                ritsu_ports::RuleEnum {
                    name: jp.clone(),
                    alias: self.enum_names.get(jp).cloned().unwrap_or_else(|| pascal(jp)),
                    values: self.c.enums[jp]
                        .iter()
                        .enumerate()
                        .map(|(i, v)| ritsu_ports::EnumValue {
                            name: v.clone(),
                            alias: self.value_names.get(&(jp.clone(), v.clone())).map(|(_, a)| a.clone()).unwrap_or_else(|| v.clone()),
                            public: public_value(self.f, jp, v),
                            // the display name written after the value (§15.211), as the generated
                            // label functions return it
                            title: self.c.labels.get(jp).and_then(|ls| ls.get(i).cloned().flatten()),
                        })
                        .collect(),
                    public: decl.map(|e| pub_name(&e.name)).unwrap_or_else(|| jp.clone()),
                    ordered: self.c.ordered.contains(jp),
                    origin: decl.and_then(|e| e.from.as_ref()).map(|o| (std::path::PathBuf::from(&o.path), o.name.text.clone())),
                }
            })
            .collect()
    }

    /// The parameters and outputs of the generated code in one language, as `api` lists them:
    /// each name, its alias there, its type there, and whether it may be absent; the list a rule
    /// walks comes last among the parameters, typed `elements`.
    fn param_list(&self, alias: impl Fn(&crate::ast::Name) -> String, ty_name: impl Fn(&Ty) -> String, elements: &str) -> (Vec<ritsu_ports::Param>, Vec<ritsu_ports::Param>) {
        let param = |n: &crate::ast::Name| {
            let ty = self.ty_of(&n.text);
            ritsu_ports::Param { name: n.text.clone(), alias: alias(n), ty: ty_name(&ty), optional: matches!(ty, Ty::Opt(_)) }
        };
        let mut params: Vec<ritsu_ports::Param> = self.f.inputs.iter().map(|i| param(&i.name)).collect();
        for (s, el) in self.f.elements.iter().enumerate() {
            params.push(ritsu_ports::Param { name: el.name.text.clone(), alias: alias(&el.name), ty: elements.replace("Element", &self.element_type(s)), optional: false });
        }
        (params, self.f.outputs.iter().map(|o| param(&o.name)).collect())
    }

    /// How the generated code is called in TypeScript, Python, Go and Rust: the same names `api`
    /// lists, from the same helpers (for ritsu's port of rules, `crate::ports`).
    pub fn calls(&self) -> (ritsu_ports::Call, ritsu_ports::Call, ritsu_ports::Call, ritsu_ports::Call) {
        let alias = pub_name(&self.f.name);
        // each language's call with its enums under the types they take there (§15.326)
        let (ts, py, go) = (self.for_target("typescript"), self.for_target("python"), self.for_target("go"));
        let (ts, py, go) = (ts.as_ref().unwrap_or(self), py.as_ref().unwrap_or(self), go.as_ref().unwrap_or(self));
        let (ts_params, ts_outputs) = ts.param_list(|n| pub_name(n), |t| ts.ts_ty(t), "readonly Element[]");
        let typescript = ritsu_ports::Call {
            module: format!("{alias}.ts"),
            function: alias.clone(),
            input_type: String::new(),
            params: ts_params,
            outputs: ts_outputs,
            enums: ts.enum_list(|_, a| a.to_uppercase()),
        };
        let (py_params, py_outputs) = py.param_list(|n| pub_name(n), |t| py.py_ty(t), "list[Element]");
        let python = ritsu_ports::Call {
            module: alias.clone(),
            function: alias.clone(),
            input_type: String::new(),
            params: py_params,
            outputs: py_outputs,
            enums: py.enum_list(|_, a| a.to_uppercase()),
        };
        let (go_params, go_outputs) = go.param_list(|n| pascal(&pub_name(n)), |t| go.go_ty(t), "[]Element");
        let go = ritsu_ports::Call {
            module: crate::backend::go_package(&alias),
            function: pascal(&alias),
            input_type: "Input".into(),
            params: go_params,
            outputs: go_outputs,
            enums: go.enum_list(|t, a| format!("{t}{a}")),
        };
        // a Rust enum's member is the value's alias in Pascal case on the enum's type, as `api`
        // lists it
        let (rs_params, rs_outputs) = self.param_list(|n| pub_name(n), |t| self.rs_ty(t), "&[Element]");
        let rust = ritsu_ports::Call {
            module: format!("{alias}.rs"),
            function: alias.clone(),
            input_type: String::new(),
            params: rs_params,
            outputs: rs_outputs,
            enums: self.enum_list(|_, a| pascal(a)),
        };
        (typescript, python, go, rust)
    }

    /// `rulec api <file.rule> --format json`. Language independent throughout: every field is
    /// a name or a number that the generated code really uses.
    /// `machine` in `rulec api` (§15.148), or `null`.
    fn machine_json(&self) -> String {
        let (Some(m), Some((en, init, fins)), Some((cin, cout))) = (self.f.machine.as_ref(), self.machine_consts(), self.carried()) else {
            return "null".into();
        };
        let states = self.c.enums.get(&en).cloned().unwrap_or_default();
        let names = |initial: &str, fin: &str, is: &str| {
            crate::json::Obj::new().str("initial", initial).str("final", fin).str("is_final", is).finish()
        };
        let constants = crate::json::Obj::new()
            .raw("python", names("INITIAL", "FINAL", "is_final"))
            .raw("typescript", names("INITIAL", "FINAL", "isFinal"))
            .raw("javascript", names("INITIAL", "FINAL", "isFinal"))
            .raw("rust", names("INITIAL", "FINAL", "is_final"))
            .raw("ruby", names("INITIAL", "FINAL", "final?"))
            // `final` is a keyword in PHP whatever its case.
            .raw("php", names("INITIAL", "FINAL_STATES", "is_final"))
            .raw("go", names("Initial", "Final", "IsFinal"))
            // …and in Swift.
            .raw("swift", names("initialState", "finalStates", "isFinal"))
            .raw("java", names("INITIAL", "FINAL", "isFinal"))
            .finish();
        // The values carried along with the state (§15.217): which input comes back as which
        // output, of what type, where a case starts, and what each language calls that start.
        let more: Vec<String> = self
            .more_carried()
            .iter()
            .map(|mc| {
                let upper = mc.upper();
                let consts = crate::json::Obj::new()
                    .str("python", &upper)
                    .str("typescript", &upper)
                    .str("javascript", &upper)
                    .str("rust", &upper)
                    .str("ruby", &upper)
                    .str("php", &upper)
                    .str("go", &mc.go())
                    .str("swift", &mc.swift())
                    .str("java", &upper)
                    .finish();
                crate::json::Obj::new()
                    .str("input", &mc.input)
                    .str("output", &mc.output)
                    .str("type", mc.ty.to_string())
                    .raw("initial", &mc.wire)
                    .raw("constants", consts)
                    .finish()
            })
            .collect();
        crate::json::Obj::new()
            .str("name", &m.name.text)
            .opt_str("alias", m.name.ascii.as_deref())
            .str("over", m.over.as_ref().map(|n| n.text.as_str()).unwrap_or(""))
            .raw("carry", crate::json::Obj::new().str("input", &cin).str("output", &cout).str("enum", &en).finish())
            // The inputs a case passes unchanged from its first call to its last (§15.149).
            .raw("held", crate::json::strs(&m.held.iter().map(|n| n.text.clone()).collect::<Vec<_>>()))
            .raw("states", crate::json::strs(&states))
            .str("initial", &init)
            .raw("final", crate::json::strs(&fins))
            .raw(
                "never",
                crate::json::arr(
                    &m.nevers
                        .iter()
                        .map(|nv| {
                            crate::json::Obj::new()
                                .raw("states", crate::json::strs(&nv.states.iter().map(|n| n.text.clone()).collect::<Vec<_>>()))
                                .raw("after", crate::json::strs(&nv.after.iter().map(|n| n.text.clone()).collect::<Vec<_>>()))
                                .finish()
                        })
                        .collect::<Vec<_>>(),
                ),
            )
            .raw(
                "once",
                crate::json::arr(
                    &m.onces
                        .iter()
                        .map(|on| crate::json::Obj::new().str("output", &on.output.text).str("cell", &crate::machine::cell_text(&on.cell)).finish())
                        .collect::<Vec<_>>(),
                ),
            )
            .raw("constants", constants)
            .opt_raw("carried", (!more.is_empty()).then(|| crate::json::arr(&more)))
            .opt_raw(
                "always",
                (!m.alwayses.is_empty()).then(|| crate::json::arr(
                    &m.alwayses
                        .iter()
                        .map(|al| {
                            crate::json::Obj::new()
                                .raw(
                                    "tests",
                                    crate::json::arr(
                                        &al.tests
                                            .iter()
                                            .map(|(n, cell, _)| crate::json::Obj::new().str("input", &n.text).str("cell", &crate::machine::cell_text(cell)).finish())
                                            .collect::<Vec<_>>(),
                                    ),
                                )
                                .raw("when", crate::json::strs(&al.when.iter().map(|n| n.text.clone()).collect::<Vec<_>>()))
                                .finish()
                        })
                        .collect::<Vec<_>>(),
                )),
            )
            // What is to happen (§15.287), as written; `check` has found the calls.
            .opt_raw("possible", (!m.possibles.is_empty()).then(|| crate::possible::machine_api(self.f, m)))
            .finish()
    }

    pub fn api(&self) -> String {
        if self.f.kind == FileKind::Enums {
            return self.api_enums();
        }
        let whole = self.api_whole();
        // What it says of a language where some enum takes another type than elsewhere (§15.326)
        // comes from the generator of that language, field by field under the language's name.
        let mut spliced: Option<crate::json::Json> = None;
        let langs: [(&str, &[&str]); 6] = [
            ("python", &["python", "numpy"]),
            ("typescript", &["typescript", "javascript"]),
            ("go", &["go"]),
            ("java", &["java"]),
            ("ruby", &["ruby"]),
            ("php", &["php"]),
        ];
        for (id, keys) in langs {
            let Some(t) = self.for_target(id) else { continue };
            let Ok(theirs) = crate::json::parse(&t.api_whole()) else { continue };
            if spliced.is_none() {
                spliced = crate::json::parse(&whole).ok();
            }
            if let Some(base) = spliced.as_mut() {
                splice_fields(base, &theirs, keys);
            }
        }
        match spliced {
            Some(j) => j.compact(),
            None => whole,
        }
    }

    /// [`Gen::api`], every language as this generator writes it.
    fn api_whole(&self) -> String {
        let alias = pub_name(&self.f.name);
        let pkg = crate::backend::go_package(&alias);
        // The two errors of the languages that raise or return one of their own (§15.202). Most
        // carry the sentence and the refused value under these two names.
        let (what_value, what): (&[&str], &[&str]) = (&["what", "value"], &["what"]);
        let outs = &self.f.outputs;

        // The record function's own parameter names (§15.35): a temporary that no declared
        // name answers to, so an input aliased `out` does not collide.
        let (r_out, r_trace, r_tag) = (self.temp("out"), self.temp("trace"), self.temp("tag"));

        // --- Python
        let params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &pub_name(&i.name), &self.py_ty(&ty), &ty)
            })
            .chain(self.elements_json("list[Element]", |n| pub_name(n), |t| self.py_ty(t)))
            .collect();
        let py_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pub_name(&od.name), &self.py_ty(&ty), &ty);
                // An output carries its rounding as well; `value_json` is shared with the
                // inputs, so it is spliced in here rather than made optional there.
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let py_ret = if outs.len() == 1 {
            self.py_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        // The sequence a walk reads is the last parameter (§15.56), in the inventory as in
        // the code, so an argument list built from here is the one the module takes.
        let py_args: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", pub_name(&i.name), self.py_ty(&self.ty_of(&i.name.text))))
            .chain(self.f.elements.iter().enumerate().map(|(s, el)| format!("{}: list[{}]", pub_name(&el.name), self.element_type(s))))
            .collect();
        let py_sig = format!("def {alias}({}) -> {py_ret}:", py_args.join(", "));
        let py_traced = py_sig
            .replacen(&format!("def {alias}("), &format!("def {alias}_traced("), 1)
            .replace(&format!(") -> {py_ret}:"), &format!(") -> tuple[{py_ret}, list[Fired]]:"));
        let python = crate::json::Obj::new()
            .str("module", &alias)
            .str("mcp", &format!("{alias}_mcp.py"))
            .str("page", &format!("{alias}_page.html"))
            .str("function", &alias)
            .str("signature", &py_sig)
            .str("traced", &format!("{alias}_traced"))
            .str("traced_signature", &py_traced)
            .str("record", &format!("{alias}_record"))
            .str("record_signature", &format!(
                "def {alias}_record({}{r_out}: {py_ret}, {r_trace}: _Trace, {r_tag}: str = \"\") -> str:",
                leading_params(&py_args)
            ))
            .raw("params", crate::json::arr(&params))
            .str("returns", &py_ret)
            .raw("outputs", crate::json::arr(&py_outs))
            // A Python enum member is the alias in upper case (`CouponKind.PERCENT`).
            .raw("enums", self.enums_json("python", |_, a| a.to_uppercase()))
            .raw("errors", crate::json::strs(&["RuleInputError", "RuleContradictionError"]))
            .raw("error_types", api_error_types(("RuleInputError", what_value), ("RuleContradictionError", what)))
            .finish();

        // --- TypeScript
        let ts_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &pub_name(&i.name), &self.ts_ty(&ty), &ty)
            })
            .chain(self.elements_json("readonly Element[]", |n| pub_name(n), |t| self.ts_ty(t)))
            .collect();
        let ts_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pub_name(&od.name), &self.ts_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let ts_ret = if outs.len() == 1 {
            self.ts_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let ts_args: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", pub_name(&i.name), self.ts_ty(&self.ty_of(&i.name.text))))
            .chain(self.f.elements.iter().enumerate().map(|(s, el)| format!("{}: readonly {}[]", pub_name(&el.name), self.element_type(s))))
            .collect();
        let ts_sig = format!("export function {alias}({}): {ts_ret}", ts_args.join(", "));
        let ts_traced = ts_sig
            .replacen(&format!("export function {alias}("), &format!("export function {alias}_traced("), 1)
            .replace(&format!("): {ts_ret}"), &format!("): [{ts_ret}, Fired[]]"));
        let typescript = crate::json::Obj::new()
            .str("module", &format!("{alias}.ts"))
            .str("mcp", &format!("{alias}_mcp.ts"))
            .str("page", &format!("{alias}_page.html"))
            .str("function", &alias)
            .str("signature", &ts_sig)
            .str("traced", &format!("{alias}_traced"))
            .str("traced_signature", &ts_traced)
            .str("record", &format!("{alias}_record"))
            .str("record_signature", &format!(
                "export function {alias}_record({}{r_out}: {ts_ret}, {r_trace}: Fired[], {r_tag} = \"\"): string",
                leading_params(&ts_args)
            ))
            .raw("params", crate::json::arr(&ts_in))
            .str("returns", &ts_ret)
            .raw("outputs", crate::json::arr(&ts_outs))
            // A TypeScript enum member is the alias in upper case on a frozen object
            // (`CouponKind.PERCENT`), the same spelling Python uses.
            .raw("enums", self.enums_json("typescript", |_, a| a.to_uppercase()))
            .raw("errors", crate::json::strs(&["RuleInputError", "RuleContradictionError"]))
            .raw("error_types", api_error_types(("RuleInputError", what_value), ("RuleContradictionError", what)))
            .finish();

        // --- JavaScript: the TypeScript names without the types (§15.36). A value is what it
        // is at run time there: a bigint for every number and date, a string for an enum
        // member (its name) and for a string, a boolean.
        let js_ty = |ty: &Ty| -> String {
            match ty {
                Ty::Enum(_) | Ty::Str => "string".into(),
                Ty::Bool => "boolean".into(),
                Ty::Opt(t) => format!("{} | null", match **t {
                    Ty::Enum(_) | Ty::Str => "string",
                    Ty::Bool => "boolean",
                    _ => "bigint",
                }),
                _ => "bigint".into(),
            }
        };
        let js_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &pub_name(&i.name), &js_ty(&ty), &ty)
            })
            .chain(self.elements_json("Element[]", |n| pub_name(n), &js_ty))
            .collect();
        let js_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pub_name(&od.name), &js_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let js_params = self
            .f
            .inputs
            .iter()
            .map(|i| pub_name(&i.name))
            .chain(self.f.elements.iter().map(|el| pub_name(&el.name)))
            .collect::<Vec<_>>()
            .join(", ");
        let js_ret = if outs.len() == 1 { js_ty(&self.ty_of(&outs[0].name.text)) } else { "Output".into() };
        let javascript = crate::json::Obj::new()
            .str("module", &format!("{alias}.mjs"))
            .str("mcp", &format!("{alias}_mcp.mjs"))
            .str("page", &format!("{alias}_page.html"))
            .str("function", &alias)
            .str("signature", &format!("export function {alias}({js_params})"))
            .str("traced", &format!("{alias}_traced"))
            .str("traced_signature", &format!("export function {alias}_traced({js_params})"))
            .str("record", &format!("{alias}_record"))
            .str("record_signature", &format!("export function {alias}_record({}{r_out}, {r_trace}, {r_tag} = \"\")", leading_param(&js_params)))
            .raw("params", crate::json::arr(&js_in))
            .str("returns", &js_ret)
            .raw("outputs", crate::json::arr(&js_outs))
            .raw("enums", self.enums_json("javascript", |_, a| a.to_uppercase()))
            .raw("errors", crate::json::strs(&["RuleInputError", "RuleContradictionError"]))
            .raw("error_types", api_error_types(("RuleInputError", what_value), ("RuleContradictionError", what)))
            .finish();

        // --- Rust
        let rs_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &pub_name(&i.name), &self.rs_ty(&ty), &ty)
            })
            .chain(self.elements_json("&[Element]", |n| pub_name(n), |t| self.rs_ty(t)))
            .collect();
        let rs_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pub_name(&od.name), &self.rs_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let rs_ret = if outs.len() == 1 {
            self.rs_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let rs_args: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", pub_name(&i.name), self.rs_ty(&self.ty_of(&i.name.text))))
            .chain(self.f.elements.iter().enumerate().map(|(s, el)| format!("{}: &[{}]", pub_name(&el.name), self.element_type(s))))
            .collect();
        let rs_sig = format!("pub fn {alias}({}) -> Result<{rs_ret}, RuleError>", rs_args.join(", "));
        let rs_traced = rs_sig
            .replacen(&format!("pub fn {alias}("), &format!("pub fn {alias}_traced("), 1)
            .replace(&format!(") -> Result<{rs_ret}, RuleError>"), &format!(") -> Result<({rs_ret}, Vec<Fired>), RuleError>"));
        let rust = crate::json::Obj::new()
            .str("module", &format!("{alias}.rs"))
            .str("function", &alias)
            .str("signature", &rs_sig)
            .str("traced", &format!("{alias}_traced"))
            .str("traced_signature", &rs_traced)
            .str("record", &format!("{alias}_record"))
            .str("record_signature", &format!(
                "pub fn {alias}_record({}{r_out}: {rs_ret}, {r_trace}: &[Fired], {r_tag}: &str) -> String",
                leading_params(&rs_args)
            ))
            .raw("params", crate::json::arr(&rs_in))
            .str("returns", &rs_ret)
            .raw("outputs", crate::json::arr(&rs_outs))
            // A Rust enum member is the alias in PascalCase on the enum's own type.
            .raw("enums", self.enums_json("rust", |_, a| pascal(a)))
            .raw("errors", crate::json::strs(&["RuleError::Input", "RuleError::Contradiction"]))
            // The two variants are struct variants; these are their fields.
            .raw("error_types", api_error_types(("RuleError::Input", what_value), ("RuleError::Contradiction", what)))
            // The proof harnesses beside the module (§15.95): the file, and the name of
            // every harness in it, so `kani --harness <name>` can be written from here.
            .str("proof", &format!("{alias}_proof.rs"))
            .raw("harnesses", crate::json::strs(&self.proof_harnesses().iter().map(|s| s.as_str()).collect::<Vec<_>>()))
            .raw("wasi", self.api_wasi())
            .finish();

        // --- Ruby. The unit is not in the type here (§15.20), so the entry states it
        // the way the others do and the generated comment repeats it for a reader.
        let rb_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &pub_name(&i.name), &self.rb_api_ty(&ty), &ty)
            })
            .chain(self.elements_json("Array[Element]", |n| pub_name(n), |t| self.rb_api_ty(t)))
            .collect();
        let rb_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pub_name(&od.name), &self.rb_api_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let rb_ret = if outs.len() == 1 {
            self.rb_api_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let rb_args = self
            .f
            .inputs
            .iter()
            .map(|i| pub_name(&i.name))
            .chain(self.f.elements.iter().map(|el| pub_name(&el.name)))
            .collect::<Vec<_>>()
            .join(", ");
        let rb_sig = format!("{}.{alias}({rb_args})", self.rb_module());
        let rb_traced = rb_sig.replacen(&format!(".{alias}("), &format!(".{alias}_traced("), 1);
        let ruby = crate::json::Obj::new()
            .str("module", &self.rb_module())
            .str("function", &alias)
            .str("signature", &rb_sig)
            .str("traced", &format!("{alias}_traced"))
            .str("traced_signature", &rb_traced)
            .str("record", &format!("{alias}_record"))
            .str("record_signature", &format!(
                "{}.{alias}_record({}{r_out}, {r_trace}, {r_tag} = \"\")",
                self.rb_module(),
                leading_param(&rb_args)
            ))
            // The signature file that ships with it; `steep` reads this, not the entry.
            .str("rbs", &format!("sig/{alias}.rbs"))
            .raw("params", crate::json::arr(&rb_in))
            .str("returns", &rb_ret)
            .raw("outputs", crate::json::arr(&rb_outs))
            // A Ruby enum member is a constant under the type's module (`Band::SHORT`).
            .raw("enums", self.enums_json("ruby", |_, a| a.to_uppercase()))
            .raw("errors", crate::json::strs(&["RuleInputError", "RuleContradictionError"]))
            .raw("error_types", api_error_types(("RuleInputError", what_value), ("RuleContradictionError", what)))
            .finish();

        // --- PHP. The type is written down (§15.77) but carries no unit, so a param's
        // `type` is what the signature says and the `unit` field beside it says the rest.
        let php_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &pub_name(&i.name), &self.php_api_ty(&ty), &ty)
            })
            // The alias here is the declared name, as in Python and Ruby: PHP takes its
            // arguments by position, and `$` belongs to the spelling, not to the name.
            .chain(self.elements_json("Element[]", |n| pub_name(n), |t| self.php_api_ty(t)))
            .collect();
        let php_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pub_name(&od.name), &self.php_api_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let php_fname = self.php_fname();
        let php_ret = if outs.len() == 1 {
            self.php_api_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let php_args: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                format!(
                    "{} {}",
                    self.php_api_ty(&self.ty_of(&i.name.text)),
                    crate::codegen::php::php_var_pub(&pub_name(&i.name))
                )
            })
            .chain(
                self.f
                    .elements
                    .iter()
                    .map(|el| format!("array {}", crate::codegen::php::php_var_pub(&pub_name(&el.name)))),
            )
            .collect();
        let php_sig = format!("function {php_fname}({}): {php_ret}", php_args.join(", "));
        let php_traced = format!("function {php_fname}_traced({}): array", php_args.join(", "));
        let php = crate::json::Obj::new()
            .str("module", &format!("{alias}.php"))
            // The namespace every name below sits in: a caller writes `use` for it, or
            // qualifies the call, and nothing here needs an autoloader.
            .str("namespace", &self.php_ns())
            .str("function", &php_fname)
            .str("signature", &php_sig)
            .str("traced", &format!("{php_fname}_traced"))
            .str("traced_signature", &php_traced)
            .str("record", &format!("{php_fname}_record"))
            .str("record_signature", &format!(
                "function {php_fname}_record({}{php_ret} {}, array {}, string {} = ''): string",
                leading_params(&php_args),
                crate::codegen::php::php_var_pub(&r_out),
                crate::codegen::php::php_var_pub(&r_trace),
                crate::codegen::php::php_var_pub(&r_tag)
            ))
            .raw("params", crate::json::arr(&php_in))
            .str("returns", &php_ret)
            .raw("outputs", crate::json::arr(&php_outs))
            // A PHP enum member is a case on the enum's own type, spelled in upper case as
            // Python and TypeScript spell theirs (`CouponKind::PERCENT`).
            .raw("enums", self.enums_json("php", |t, a| format!("{}::{}", crate::codegen::php::php_name(t), crate::codegen::php::php_name(&a.to_uppercase()))))
            .raw("errors", crate::json::strs(&["RuleInputError", "RuleContradictionError"]))
            .raw("error_types", api_error_types(("RuleInputError", what_value), ("RuleContradictionError", what)))
            .finish();

        // --- Swift. The unit is in the type here, as it is in Rust, so a param's `type`
        // is the brand and the `unit` field beside it repeats what that brand stands for.
        let sw_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &sw_name(&pub_name(&i.name)), &self.sw_api_ty(&ty), &ty)
            })
            .chain(self.elements_json("[Element]", |n| sw_name(&pub_name(n)), |t| self.sw_api_ty(t)))
            .collect();
        let sw_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &sw_name(&pub_name(&od.name)), &self.sw_api_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let sw_fname = sw_name(&pub_name(&self.f.name));
        let sw_ret = if outs.len() == 1 {
            self.sw_api_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let sw_args: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", sw_name(&pub_name(&i.name)), self.sw_ty(&self.ty_of(&i.name.text))))
            .chain(self.f.elements.iter().enumerate().map(|(s, el)| format!("{}: [{}]", sw_name(&pub_name(&el.name)), self.element_type(s))))
            .collect();
        let sw_sig = format!("func {sw_fname}({}) throws -> {sw_ret}", sw_args.join(", "));
        let sw_traced = sw_sig
            .replacen(&format!("func {sw_fname}("), &format!("func {sw_fname}Traced("), 1)
            .replace(&format!(") throws -> {sw_ret}"), &format!(") throws -> ({sw_ret}, [Fired])"));
        let swift = crate::json::Obj::new()
            .str("module", &format!("{alias}.swift"))
            .str("function", &sw_fname)
            .str("signature", &sw_sig)
            .str("traced", &format!("{sw_fname}Traced"))
            .str("traced_signature", &sw_traced)
            .str("record", &format!("{sw_fname}Record"))
            .str("record_signature", &format!(
                "func {sw_fname}Record({}{}: {sw_ret}, {}: [Fired], {}: String = \"\") -> String",
                leading_params(&sw_args),
                sw_name(&r_out),
                sw_name(&r_trace),
                sw_name(&r_tag)
            ))
            .raw("params", crate::json::arr(&sw_in))
            .str("returns", &sw_ret)
            .raw("outputs", crate::json::arr(&sw_outs))
            // A Swift enum member is the alias in the language's own spelling, on the
            // enum's own type (`Band.short`).
            .raw("enums", self.enums_json("swift", |_, a| sw_name(a)))
            .raw("errors", crate::json::strs(&["RuleError.input", "RuleError.contradiction"]))
            // The two cases carry associated values; these are their labels.
            .raw("error_types", api_error_types(("RuleError.input", what_value), ("RuleError.contradiction", what)))
            .finish();

        // --- Java. `long` is the int64 the proof is about, and no brand rides in the
        // type (§15.78), so a param's `type` is what the signature says and `unit` says the
        // rest.
        let jv_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &self.java_name_of(&pub_name(&i.name)), &self.java_api_ty(&ty), &ty)
            })
            .chain(self.elements_json("List<Element>", |n| crate::codegen::java::java_name_pub(&pub_name(n)), |t| self.java_api_ty(t)))
            .collect();
        let jv_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pub_name(&od.name), &self.java_api_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let jv_cls = self.java_class();
        let jv_fname = self.java_fname();
        let jv_ret = if outs.len() == 1 {
            self.java_api_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let jv_args: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{} {}", self.java_api_ty(&self.ty_of(&i.name.text)), self.java_name_of(&pub_name(&i.name))))
            .chain(self.f.elements.iter().enumerate().map(|(s, el)| format!("List<{}> {}", self.element_type(s), self.java_name_of(&pub_name(&el.name)))))
            .collect();
        let java = crate::json::Obj::new()
            .str("module", &format!("{jv_cls}.java"))
            .str("class", &jv_cls)
            .str("function", &jv_fname)
            .str("signature", &format!("public static {jv_ret} {jv_fname}({})", jv_args.join(", ")))
            .str("traced", &format!("{jv_fname}Traced"))
            .str("traced_signature", &format!("public static Traced {jv_fname}Traced({})", jv_args.join(", ")))
            .str("record", &format!("{jv_fname}Record"))
            .str("record_signature", &format!(
                "public static String {jv_fname}Record({}{jv_ret} {}, List<Fired> {}, String {})",
                leading_params(&jv_args),
                self.java_name_of(&r_out),
                self.java_name_of(&r_trace),
                self.java_name_of(&r_tag)
            ))
            // Everything is nested in the one class, so a caller says `Rule.Member` where
            // Python says `Module.Member`.
            .str("build", "javac --release 17 -encoding UTF-8 -d classes *.java")
            .raw("params", crate::json::arr(&jv_in))
            .str("returns", &jv_ret)
            .raw("outputs", crate::json::arr(&jv_outs))
            .raw("enums", self.enums_json("java", |t, a| format!("{}.{}", crate::codegen::java_class(t), a.to_uppercase())))
            .raw("errors", crate::json::strs(&["RuleInputError", "RuleContradictionError"]))
            .raw("error_types", api_error_types(("RuleInputError", what_value), ("RuleContradictionError", what)))
            .finish();

        // --- Go
        let go_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(
                    &i.name.text,
                    &pascal(&pub_name(&i.name)),
                    &self.go_ty(&ty),
                    &ty,
                )
            })
            .chain(self.elements_json("[]Element", |n| pascal(&pub_name(n)), |t| self.go_ty(t)))
            .collect();
        let go_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pascal(&pub_name(&od.name)), &self.go_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let go_fn = pascal(&alias);
        let go_ret = if outs.len() == 1 {
            self.go_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let go_sig = format!("func {go_fn}(in Input) ({go_ret}, error)");
        let go_traced = format!("func {go_fn}Traced(in Input) ({go_ret}, []Fired, error)");
        let go = crate::json::Obj::new()
            .str("package", &pkg)
            .str("func", &go_fn)
            .str("signature", &go_sig)
            .str("traced", &format!("{go_fn}Traced"))
            .str("traced_signature", &go_traced)
            .str("record", &format!("{go_fn}Record"))
            .str("record_signature", &format!("func {go_fn}Record(in Input, out {go_ret}, trace []Fired, tag string) string"))
            .str("input_type", "Input")
            .raw("input_fields", crate::json::arr(&go_in))
            .str("output_type", &go_ret)
            .raw("output_fields", crate::json::arr(&go_outs))
            // A Go enum member is the type name followed by the alias (`CouponKindPercent`).
            .raw("enums", self.enums_json("go", |t, a| format!("{t}{a}")))
            // The error comes back as a pointer to one of two types, which `errors.As` tells
            // apart (§15.202). `HasValue` says whether `Value` holds the refused value.
            .raw("errors", crate::json::strs(&["*RuleInputError", "*RuleContradictionError"]))
            .raw(
                "error_types",
                api_error_types(("*RuleInputError", &["What", "Value", "HasValue"]), ("*RuleContradictionError", &["What"])),
            )
            .finish();

        crate::json::Obj::new()
            .str("rule", &self.f.name.text)
            .str("alias", &alias)
            .str("version", &self.f.version)
            .str("source_sha256", &self.src_hash)
            // What the entry guard holds a caller to that `rulec schema` cannot say. A
            // caller that validated the shape and stopped is not done (§15.116).
            .raw("preconditions", crate::verify::preconditions_json(self.f, self.c))
            // What one call can have at once (§15.287): no door, so no precondition.
            .opt_raw("possible", (!self.f.possibles.is_empty()).then(|| crate::possible::lines_api(self.f)))
            .raw("applies", crate::json::arr(&self.f.applies.iter().map(|a| {
                crate::json::Obj::new()
                    .str("name", &a.name.text)
                    .str("path", &a.path)
                    .str("sha256", a.hash.as_deref().unwrap_or(""))
                    .opt_str("meaning", a.meaning.as_deref())
                    .finish()
            }).collect::<Vec<_>>()))
            // The contributions merged into the function (§15.231), each with the inputs it added,
            // and the open values with where each term comes from. Written only for a rule that has
            // them, so no other description changes.
            .opt_raw("contributions", (!self.f.contributions.is_empty()).then(|| crate::link::contributions_json(self.f, &self.path)))
            .opt_raw("opens", (!self.f.opens.is_empty()).then(|| crate::link::opens_json(self.f)))
            // Where the caller's object holds each input, and the one function per language
            // that reads it (§15.125). Absent when the rule takes its inputs as they come.
            .raw("projection", self.projection_json())
            // Where hand-written code takes an input or a field of an element from (§15.285):
            // `ritsu check` holds the range of what can come from there, and nothing holds the code
            // to carrying it. Written only for a rule that says, so no other description changes.
            .opt_raw("fed_by", {
                let entry = |el: Option<&str>, i: &VarDecl| {
                    let fb = i.fed.as_ref()?;
                    let o = crate::json::Obj::new();
                    let o = match el {
                        Some(el) => o.str("elements", el),
                        None => o,
                    };
                    Some(o.str("name", &i.name.text).str("alias", &pub_name(&i.name)).str("source", &fb.written).finish())
                };
                let fields = self.f.elements.iter().flat_map(|el| el.fields.iter().filter_map(|fd| entry(Some(&el.name.text), fd)));
                let all: Vec<String> = self.f.inputs.iter().filter_map(|i| entry(None, i)).chain(fields).collect();
                (!all.is_empty()).then(|| crate::json::arr(&all))
            })
            // The machine the function is one step of (§15.148), and what each language calls
            // the constants a host needs to keep the state: where a case starts, where it ends.
            .raw("machine", self.machine_json())
            // The documents transcribed, as the header names them (§15.71).
            .raw("sources", crate::json::arr(&self.f.sources.iter().map(|s| {
                let o = crate::json::Obj::new().str("name", &s.name.text);
                match &s.kind {
                    SourceKind::Law { db, id, asof } => o
                        .str("kind", crate::kw::LAW)
                        .str("db", db.word())
                        .str("id", id)
                        .str("asof", asof)
                        .raw("pins", crate::json::arr(&s.pins.iter().map(|p| {
                            crate::json::Obj::new().str("fragment", &p.fragment).str("sha256", &p.hash).finish()
                        }).collect::<Vec<_>>()))
                        .finish(),
                    SourceKind::File { path, url, hash } => o
                        .str("kind", crate::kw::FILE)
                        .str("path", path)
                        .str("url", url.as_deref().unwrap_or(""))
                        .str("sha256", hash.as_deref().unwrap_or(""))
                        .raw("pins", crate::json::arr(&s.pins.iter().map(|p| {
                            crate::json::Obj::new().str("fragment", &p.fragment).str("sha256", &p.hash).finish()
                        }).collect::<Vec<_>>()))
                        .finish(),
                }
            }).collect::<Vec<_>>()))
            .raw("python", python)
            .raw("typescript", typescript)
            .raw("javascript", javascript)
            .raw("rust", rust)
            .raw("ruby", ruby)
            .raw("php", php)
            .raw("go", go)
            .raw("swift", swift)
            .raw("java", java)
            .raw("numpy", self.api_numpy())
            .raw("connect", self.api_connect())
            .raw("sql", self.api_sql())
            .raw("wasm", self.api_wasm())
            // Which values need which before them, and the two functions in each language
            // (§15.222). Written only for a rule that has a block.
            .opt_raw("prerequisites", self.prereq_api())
            // The random inputs, and the function that counts the answers in each language
            // (§15.225). Written only for a rule that draws.
            .opt_raw("chances", self.chances_api())
            .finish()
    }
}

impl Gen<'_> {
    /// `rulec api` of an `enums` file (§15.212). There is no function in it: what each language
    /// gets is one module of types, which `rulec gen` writes beside every rule that reads one of
    /// them, and the enums in it as a rule's own `enums` lists them. NumPy, SQL, the Connect
    /// service and Wasm take an enum as its name, so they get no module.
    fn api_enums(&self) -> String {
        let alias = pub_name(&self.f.name);
        let pkg = crate::backend::go_package(&alias);
        let lang = |module: &str| crate::json::Obj::new().str("module", module);
        crate::json::Obj::new()
            .str("enums", &self.f.name.text)
            .str("alias", &alias)
            .str("version", &self.f.version)
            .str("source_sha256", &self.src_hash)
            .raw("python", lang(&alias).raw("enums", self.enums_json("python", |_, a| a.to_uppercase())).finish())
            .raw("typescript", lang(&format!("{alias}.ts")).raw("enums", self.enums_json("typescript", |_, a| a.to_uppercase())).finish())
            .raw("javascript", lang(&format!("{alias}.mjs")).raw("enums", self.enums_json("javascript", |_, a| a.to_uppercase())).finish())
            .raw("rust", lang(&format!("{alias}.rs")).raw("enums", self.enums_json("rust", |_, a| pascal(a))).finish())
            .raw(
                "ruby",
                lang(&self.rb_module())
                    .str("file", &format!("{alias}.rb"))
                    .str("rbs", &format!("sig/{alias}.rbs"))
                    .raw("enums", self.enums_json("ruby", |_, a| a.to_uppercase()))
                    .finish(),
            )
            .raw(
                "php",
                lang(&format!("{alias}.php"))
                    .str("namespace", &self.php_ns())
                    .raw("enums", self.enums_json("php", |t, a| format!("{}::{}", crate::codegen::php::php_name(t), crate::codegen::php::php_name(&a.to_uppercase()))))
                    .finish(),
            )
            .raw("go", crate::json::Obj::new().str("package", &pkg).str("module", &format!("{pkg}/{alias}.go")).raw("enums", self.enums_json("go", |t, a| format!("{t}{a}"))).finish())
            .raw("swift", lang(&format!("{alias}.swift")).raw("enums", self.enums_json("swift", |_, a| sw_name(a))).finish())
            .raw(
                "java",
                lang(&format!("{}.java", self.java_class()))
                    .str("class", &self.java_class())
                    .raw("enums", self.enums_json("java", |t, a| format!("{}.{}", crate::codegen::java_class(t), a.to_uppercase())))
                    .finish(),
            )
            // The functions of its `prerequisites` blocks, which its modules hold (§15.222).
            .opt_raw("prerequisites", self.prereq_api())
            .finish()
    }
}

/// `error_types` in `rulec api` (§15.202): the two errors a language's code raises or returns,
/// in the order `errors` names them, each with `kind` — `input` for an input the entry guard
/// refuses, `contradiction` for the W114 guard — its name, and the fields a caller reads, so
/// that catching the caller's own mistake and reading the refused value takes no reading of
/// the sentence.
pub(crate) fn api_error_types(input: (&str, &[&str]), contradiction: (&str, &[&str])) -> String {
    let one = |kind: &str, (name, fields): (&str, &[&str])| {
        crate::json::Obj::new().str("kind", kind).str("name", name).raw("fields", crate::json::strs(fields)).finish()
    };
    crate::json::arr(&[one("input", input), one("contradiction", contradiction)])
}

// ---------------------------------------------------------------------------
// Ruby (§15.20). The closest relative of the Python backend: dynamically typed,
// and `/` on Integers rounds toward −∞ exactly as Python's `//` does. What is
// different is the constant rule — a Ruby constant must begin with an uppercase
// ASCII letter, so a Japanese name cannot be one and groups take a prefix.

/// Turn a name into something Ruby will accept as a constant.
/// The name of the rule's Ruby module, from its alias.
pub(crate) fn ruby_module(alias: &str) -> String {
    rb_const(&pascal(alias))
}

fn rb_const(n: &str) -> String {
    // an enum's type that `Gen::apart_text` marks keeps the mark at its head
    if let Some(rest) = n.strip_prefix(clash::MARK) {
        return format!("{}{}", clash::MARK, rb_const(rest));
    }
    if n.starts_with(|c: char| c.is_ascii_uppercase()) {
        n.to_string()
    } else {
        format!("C_{n}")
    }
}

fn rb_expr(s: &str) -> String {
    // `//` becomes `/`: Ruby's integer division already floors toward −∞.
    truths(s, "true", "false").replace(" // ", " / ")
}

/// The five modes of §7.3, and the two comparisons. Nothing is left to Ruby's own
/// division: `abs` first, then the sign is carried back, so every mode means the
/// same thing it means in the other four.
fn round_rb() -> String {
    format!(
        r#"
  def self._min(a, b)
    a < b ? a : b
  end

  def self._max(a, b)
    a > b ? a : b
  end

  # {down}
  def self._round_down(x, g)
    v = x.abs / g * g
    x < 0 ? -v : v
  end

  # {up}
  def self._round_up(x, g)
    q, r = x.abs / g, x.abs % g
    v = r.zero? ? q * g : (q + 1) * g
    x < 0 ? -v : v
  end

  # {half}
  def self._round_half(x, g)
    q, r = x.abs / g, x.abs % g
    v = 2 * r >= g ? (q + 1) * g : q * g
    x < 0 ? -v : v
  end

  # {half_down}
  def self._round_half_down(x, g)
    q, r = x.abs / g, x.abs % g
    v = 2 * r > g ? (q + 1) * g : q * g
    x < 0 ? -v : v
  end

  # {bankers}
  def self._round_bankers(x, g)
    q, r = x.abs / g, x.abs % g
    q += 1 if 2 * r > g || (2 * r == g && q.odd?)
    v = q * g
    x < 0 ? -v : v
  end

  private_class_method :_min, :_max, :_round_down, :_round_up, :_round_half, :_round_half_down, :_round_bankers
"#,
        down = tr!("0 へ寄せる。-4.8円 → -4円。", "Toward zero: -4.8 yen -> -4 yen."),
        up = tr!("0 から遠ざける。-4.2円 → -5円。", "Away from zero: -4.2 yen -> -5 yen."),
        half = tr!("半分ちょうどは 0 から遠ざける。", "An exact half goes away from zero."),
        half_down = tr!("半分ちょうどは 0 へ寄せる。", "An exact half goes toward zero."),
        bankers = tr!("半分ちょうどは偶数へ。", "An exact half goes to the even neighbor."),
    )
}

impl<'a> Gen<'a> {
    /// An enum value, written as the constant the module declares for it.
    fn rb_value(&self, v: &str, ty: &Ty) -> String {
        match self.value_name(v, ty) {
            Some((ty, alias)) => format!("{}::{}", rb_const(ty), rb_const(&alias.to_uppercase())),
            None => format!("{v:?}"),
        }
    }

    /// Render a cell as a Ruby condition. A don't-care yields None (no condition).
    fn rb_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        // An optional value is `nil` when it is absent: `!nil` is true and `nil < 5` raises, so a
        // truth value is compared and a comparison asks first (§15.201).
        let optional = matches!(ty, Ty::Opt(_));
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                Lit::Word(w) => self.rb_value(w, ty),
                Lit::Num(n) => self.int_lit(n, inner, col_scale),
                Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => format!("{s:?}"),
            }
        };
        // A set is compared value by value. `[A, B].include?(x)` would do, but steep types
        // that literal as `Array[A | B]` and refuses `x`, whose type is the whole enum.
        let any_of = |ls: &Vec<Lit>, neg: bool| -> String {
            let mut out: Vec<String> = Vec::new();
            for l in ls {
                if let Lit::Word(w) = l {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| self.rb_value(m, ty)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            let (op, join) = if neg { ("!=", " && ") } else { ("==", " || ") };
            format!("({})", out.iter().map(|v| format!("{var} {op} {v}")).collect::<Vec<_>>().join(join))
        };
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => format!("{var}.nil?"),
            Cell::Prefix(ps) => ps
                .iter()
                .map(|p| format!("{var}.start_with?({})", format!("{p:?}")))
                .collect::<Vec<_>>()
                .join(" || "),
            // A cell naming one group calls the constant the module already declares,
            // rather than writing the members out again.
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                format!("GROUP_{}.include?({var})", self.rb_group(w))
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE && optional => format!("{var} == true"),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE && optional => format!("{var} == false"),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("!{var}"),
            Cell::Lit(l) => format!("{var} == {}", lit(l)),
            Cell::Set(ls) => any_of(ls, false),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                format!("!GROUP_{}.include?({var})", self.rb_group(w))
            }
            Cell::Not(ls) => any_of(ls, true),
            Cell::Cmp(cs) => {
                let c = cs
                    .iter()
                    .map(|(o, l)| {
                        let op = match o {
                            CmpOp::Le => "<=",
                            CmpOp::Ge => ">=",
                            CmpOp::Lt => "<",
                            CmpOp::Gt => ">",
                        };
                        format!("{var} {op} {}", lit(l))
                    })
                    .collect::<Vec<_>>()
                    .join(" && ");
                if optional { format!("!{var}.nil? && {c}") } else { c }
            }
            // A comparison on an ordered enum, by the places in its order (§15.210).
            Cell::Ranked { ops, .. } => {
                let en = self.c.ordered_enum(ty)?;
                let r = self.rb_rank_fn(en);
                let c = ops
                    .iter()
                    .map(|(o, v)| format!("{r}({var}) {} {r}({})", o.word(), self.rb_value(v, ty)))
                    .collect::<Vec<_>>()
                    .join(" && ");
                if optional { format!("!{var}.nil? && {c}") } else { c }
            }
        })
    }
}

/// A Ruby string literal in single quotes, which interpolates nothing.
fn rb_str(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

impl<'a> Gen<'a> {
    /// One enum as Ruby: a module of frozen constants, its members in the order declared, and
    /// beside it the functions for its order and its display names when it has them
    /// (§15.210, §15.211).
    pub(crate) fn rb_enum(&self, jp: &str, ascii: &str) -> String {
        let Some(vals) = self.c.enums.get(jp) else { return String::new() };
        let mut o = format!("  module {}\n", rb_const(ascii));
        let mut names: Vec<String> = Vec::new();
        for v in vals {
            let name = rb_const(&self.py_member(jp, v));
            o.push_str(&format!("    {name} = {v:?}\n"));
            names.push(name);
        }
        o.push_str(&format!("    ALL = [{}].freeze\n  end\n\n", names.join(", ")));
        let cls = rb_const(ascii);
        let chain = |o: &mut String, ret: &dyn Fn(usize, &String) -> String| {
            // An enum of one value has no `when` to write, and Ruby reads no `case` without one:
            // the one value's answer is the function's.
            if vals.len() == 1 {
                o.push_str(&format!("    {}\n  end\n\n", ret(0, &vals[0])));
                return;
            }
            o.push_str("    case x\n");
            for (i, (v, n)) in vals.iter().zip(&names).enumerate() {
                if i + 1 == vals.len() {
                    o.push_str(&format!("    else {}\n", ret(i, v)));
                } else {
                    o.push_str(&format!("    when {cls}::{n} then {}\n", ret(i, v)));
                }
            }
            o.push_str("    end\n  end\n\n");
        };
        if self.c.ordered.contains(jp) {
            o.push_str(&format!(
                "  # {}\n  def self.{}(x)\n",
                tr!("x が {ascii} の順序の何番目か（0 から）。", "The place of x in the order of {ascii}, counting from 0."),
                self.rb_rank_fn(jp)
            ));
            chain(&mut o, &|i, _| i.to_string());
        }
        if let Some(labels) = self.c.labels.get(jp) {
            o.push_str(&format!(
                "  # {}\n  def self.{}(x)\n",
                tr!("x の表示名。表示名の無い値は値の名前。", "The display name of x: a value without one is shown by its name."),
                self.rb_label_fn(jp)
            ));
            chain(&mut o, &|i, v| rb_str(&labels.get(i).cloned().flatten().unwrap_or_else(|| v.clone())));
        }
        o
    }

    /// The functions a module has for an enum it declares, beside the type (§15.210, §15.211): the
    /// place of a value in its order, when the values are ordered, and a value's display name, when
    /// the enum gives any. Each by its name in snake case (`tier_rank`, `kind_label`), whether it is
    /// the order's, and the sentence that says what it is. What a rule's module hands on of an enum
    /// of an `enums` file in Ruby and PHP, which name the functions so (§15.319).
    pub(crate) fn handed_fns(&self, jp: &str, ascii: &str) -> Vec<(String, bool, String)> {
        let mut out = Vec::new();
        if self.c.ordered.contains(jp) {
            out.push((
                format!("{}_rank", snake(&self.enum_type(jp))),
                true,
                tr!("x が {ascii} の順序の何番目か（0 から）。", "The place of x in the order of {ascii}, counting from 0."),
            ));
        }
        if self.c.labels.contains_key(jp) {
            out.push((
                format!("{}_label", snake(&self.enum_type(jp))),
                false,
                tr!("x の表示名。表示名の無い値は値の名前。", "The display name of x: a value without one is shown by its name."),
            ));
        }
        out
    }

    /// `era_rank`, the place of a value of an ordered enum, as this module calls it: the
    /// module of an `enums` file holds its own (§15.212).
    fn rb_rank_fn(&self, en: &str) -> String {
        self.rb_qualified(en, &format!("{}_rank", snake(&self.enum_type(en))))
    }

    /// `unit_name_label`, a value's display name.
    fn rb_label_fn(&self, en: &str) -> String {
        self.rb_qualified(en, &format!("{}_label", snake(&self.enum_type(en))))
    }

    fn rb_qualified(&self, en: &str, f: &str) -> String {
        match self.c.owners.get(en) {
            Some(o) => format!("{}.{}", ruby_module(&pub_name(&o.module)), self.theirs("ruby", en, f)),
            None => f.to_string(),
        }
    }

    /// The helpers the functions call for the expressions over an ordered enum (§15.210).
    fn rb_enum_helpers(&self) -> String {
        let mut o = String::new();
        let mut private: Vec<String> = Vec::new();
        for en in self.c.enum_order.iter().filter(|e| self.c.ordered.contains(*e)) {
            let (rank, max, min) = self.enum_helpers(en);
            let (t, r) = (self.enum_type(en), self.rb_rank_fn(en));
            if rank {
                o.push_str(&format!("  def self._rank_{t}(x)\n    {r}(x)\n  end\n\n"));
                private.push(format!(":_rank_{t}"));
            }
            if max {
                o.push_str(&format!("  def self._max_{t}(a, b)\n    {r}(a) >= {r}(b) ? a : b\n  end\n\n"));
                private.push(format!(":_max_{t}"));
            }
            if min {
                o.push_str(&format!("  def self._min_{t}(a, b)\n    {r}(a) <= {r}(b) ? a : b\n  end\n\n"));
                private.push(format!(":_min_{t}"));
            }
        }
        if !private.is_empty() {
            o.push_str(&format!("  private_class_method {}\n\n", private.join(", ")));
        }
        o
    }

    /// The module name: the rule's ASCII alias in PascalCase, like the Go package.
    fn rb_module(&self) -> String {
        rb_const(&pascal(&pub_name(&self.f.name)))
    }

    /// How a value's type is written for a reader. Ruby has no zero-cost brand, so the
    /// unit is documented rather than enforced (§15.20); `Integer` is exact at any size.
    fn rb_doc_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => rb_const(&self.enum_names.get(n).cloned().unwrap_or_else(|| "String".into())),
            Ty::Bool => "true/false".into(),
            Ty::Str => "String".into(),
            Ty::Opt(t) => format!("{} | nil", self.rb_doc_ty(t)),
            _ => format!("Integer  # {ty}"),
        }
    }

    pub fn ruby(&self) -> String {
        self.apart_text(clash::Syntax::Ruby, |g| g.ruby_text())
    }

    fn ruby_text(&self) -> String {
        let mut o = self.header("#");
        o.insert_str(0, "# frozen_string_literal: true\n");
        // The module of each `enums` file the rule reads its types from, beside it (§15.212).
        for (module, _) in self.foreign_enums() {
            o.push_str(&format!("require_relative {:?}\n", module));
        }
        o.push_str(&format!("module {} {}\n", self.rb_module(), "".trim()));
        // `module X` on its own line; the format above keeps it simple.
        o = o.replace(&format!("module {} \n", self.rb_module()), &format!("module {}\n", self.rb_module()));

        // Enums. A frozen constant per member, carrying the source name, which is also the
        // wire value (§10.2) — so the runner needs no conversion in either direction.
        for (jp, ascii) in self.own_enums() {
            o.push_str(&self.rb_enum(&jp, &ascii));
        }
        // An enum of an `enums` file is that module's, named here as well (§15.212), with the
        // functions of its order and its display names as the rule's own enums have them, each
        // calling that module's (§15.319).
        for (module, types) in self.foreign_enums() {
            let owner = ruby_module(&module);
            for (jp, ascii) in types {
                // what the file's module calls each, should the enum be handed on under another
                // type here (§15.326)
                o.push_str(&format!("  {} = {owner}::{}\n\n", rb_const(&ascii), rb_const(&self.owner_type(&jp))));
                for (f, _, doc) in self.handed_fns(&jp, &ascii) {
                    o.push_str(&format!("  # {doc}\n  def self.{f}(x)\n    {owner}.{}(x)\n  end\n\n", self.theirs("ruby", &jp, &f)));
                }
            }
        }
        o.push_str(&self.rb_enum_helpers());
        // Which values need which before them (§15.222).
        o.push_str(&self.rb_prereqs());

        // The machine this function is one step of (§15.148).
        if let Some((en, init, fins)) = self.machine_consts() {
            // The values carried along with the state, and where a case starts (§15.217).
            let mut more = String::new();
            for mc in self.more_carried() {
                let v = match mc.kind() {
                    MoreKind::Enum => self.rb_value(&mc.word(), &mc.ty),
                    MoreKind::Bool => mc.bool_word().to_string(),
                    MoreKind::Number | MoreKind::Branded => mc.int.to_string(),
                };
                more.push_str(&format!("  # {}\n  {} = {v}\n", tr!("持ち越す {} の始まりの値。", "Where the carried {} starts.", mc.input), mc.upper()));
            }
            // A machine with no final state has an empty list, and an empty literal has no
            // element type for `steep` to read: it is made with `Array.new` instead.
            let finals = if fins.is_empty() {
                "Array.new(0).freeze".to_string()
            } else {
                format!("[{}].freeze", fins.iter().map(|v| self.rb_value(v, &Ty::Enum(en.clone()))).collect::<Vec<_>>().join(", "))
            };
            o.push_str(&format!(
                "  # {}\n  INITIAL = {}\n  # {}\n  FINAL = {finals}\n{more}\n  # {}\n  def self.final?(state)\n    FINAL.include?(state)\n  end\n\n",
                tr!("案件が始まる状態。", "The state a case starts in."),
                self.rb_value(&init, &Ty::Enum(en.clone())),
                tr!("案件が終わる状態。", "The states a case ends in."),
                tr!("この状態で案件が終わっているか。", "Whether a case in this state has ended.")
            ));
        }

        o.push_str(&format!(
            "  # {}\n  #\n  # {}\n  class RuleInputError < ArgumentError\n\
             \x20   NO_VALUE = Object.new\n\n\
             \x20   attr_reader :what, :value\n\n\
             \x20   def initialize(what, value = NO_VALUE)\n\
             \x20     @what = what\n\
             \x20     @value = value\n\
             \x20     super(value.equal?(NO_VALUE) ? what : \"#{{what}}: #{{value.inspect}}\")\n\
             \x20   end\n  end\n\n",
            tr!("宣言した範囲の外。呼び出し側の契約違反。", "Outside the declared input domain: a contract violation by the caller."),
            tr!("メッセージと値を別々に持つ。", "The sentence and the value travel apart.")
        ));
        o.push_str(&format!(
            "  # {}\n  class RuleContradictionError < RuntimeError\n\
             \x20   attr_reader :what\n\n\
             \x20   def initialize(what)\n\
             \x20     @what = what\n\
             \x20     super\n\
             \x20   end\n  end\n\n",
            tr!("規則そのものの矛盾。呼び出し側の誤りではない。", "A contradiction in the rule itself, not a mistake by the caller.")
        ));
        o.push_str(&format!("  # {}\n  Fired = Struct.new(:table, :row, :label)\n\n", fired_doc()));

        // Groups. A Ruby constant has to begin with an uppercase ASCII letter, so a
        // Japanese group name cannot be one on its own; the prefix is what makes it legal.
        for g in &self.f.groups {
            let ms: Vec<String> = g.members.iter().map(|m| self.rb_value(&m.text, &self.group_ty(&g.name.text))).collect();
            o.push_str(&format!("  GROUP_{} = [{}].freeze\n", self.rb_group(&g.name.text), ms.join(", ")));
        }
        if !self.f.groups.is_empty() {
            o.push('\n');
        }

        o.push_str(&round_rb());
        o.push_str(&arith::rb_helpers(&arith::Uses::of(self.f)));
        o.push('\n');
        o.push_str(&self.rb_fn());
        o.push('\n');
        o.push_str(&self.rb_record());
        if self.projects() {
            o.push('\n');
            o.push_str(&self.rb_from());
        }
        // The method that counts the answers (§15.225).
        if self.f.has_random() {
            o.push('\n');
            o.push_str(self.rb_chances().trim_end());
            o.push('\n');
        }
        o.push_str("end\n");
        o
    }

    /// The rule's items as Ruby, at the base indentation (§15.56).
    fn rb_items(&self, local: &dyn Fn(&str) -> String, trace: &str, phase: Phase) -> String {
        let mut o = String::new();
        for (i, it) in self.f.items.iter().enumerate() {
            if !self.in_phase(i, it, phase) {
                continue;
            }
            match it {
                Item::Derived(d) => {
                    let e = self.expr(&d.expr, local);
                    o.push_str(&format!("    {} = {}  # {}\n", self.ident(&d.name.text), rb_expr(unparen(&e.text)), tr!("導出", "derived value")));
                    o.push_str(&self.rb_unread(&d.name.text));
                }
                Item::Define(d) => {
                    let e = self.expr(&d.expr, local);
                    o.push_str(&format!("    {} = {}  # {}\n", self.ident(&d.name.text), rb_expr(unparen(&e.text)), tr!("定義", "definition")));
                    // The range the rule accepts for it, held where it is worked out (§15.313).
                    let n = self.ident(&d.name.text);
                    match self.accepted_ends(d).and_then(|ends| rb_out_of(&n, ends)) {
                        Some(test) => {
                            let said = rb_said(&crate::door::out_of_range(&self.said_name(&d.name.text)), &self.here(), "");
                            o.push_str(&format!("    raise RuleInputError.new({said}, {n}) {test}\n"));
                        }
                        // What the check does not read either (§15.340).
                        None => o.push_str(&self.rb_unread(&d.name.text)),
                    }
                }
                Item::Agg(_) | Item::Fold(_) => {}
                Item::Table(t) => {
                    if let Some(t) = self.c.table_at(t) {
                        o.push_str(&self.rb_table(t, local, trace));
                    }
                }
            }
        }
        o
    }

    /// Every walk of a rule that walks sequences, in Ruby (§15.56, §15.58, §15.219). The shape is
    /// the one `py_walks` writes.
    fn rb_walks(&self, outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = String::new();
        for (s, el) in self.f.elements.iter().enumerate() {
            if let Some(cap) = self.count_cap_of(s) {
                let seq = self.held(&el.name);
                o.push_str(&format!(
                    "    raise RuleInputError.new(\"{}\", {seq}.length) if {seq}.length > {cap}\n",
                    crate::door::too_many(&el.name.text, cap)
                ));
            }
        }
        for step in self.walk_steps() {
            match step {
                // What a walk does not depend on is worked out once, before it (§15.208).
                WalkStep::Once(ix) => o.push_str(&self.rb_items(outer, trace, Phase::Only(&ix))),
                WalkStep::Walk { seq, body, sums, .. } => o.push_str(&self.rb_walk(seq, &body, &sums, outer, trace)),
            }
        }
        o
    }

    /// One walk over sequence `s`, in Ruby (§15.219).
    fn rb_walk(&self, s: usize, body_items: &[usize], sums: &[usize], outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let el = &self.f.elements[s];
        let seq = self.held(&el.name);
        let fields: BTreeMap<String, String> = el.fields.iter().map(|fd| (fd.name.text.clone(), pub_name(&fd.name))).collect();
        let e = self.elem_var(s);
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => format!("{e}.{f}"),
                None => outer(n),
            }
        };
        let mut o = String::new();
        let index = self.index_var(s);
        self.walk_begin(s, &index);
        let mut body = self.rb_element_ranges(s, &local);
        let (seen, step) = self.rb_unique(s, sums, &local);
        o.push_str(&seen);
        body.push_str(&step);
        // Past a `stop`, the door alone (§15.328).
        if let Some(i) = self.walk_goes_past_stop(sums) {
            body.push_str(&format!("    next if {}\n", self.fold_names(i).done));
        }
        body.push_str(&self.rb_items(&local, trace, Phase::Only(body_items)));
        let mut after = String::new();
        for &i in sums {
            match &self.f.items[i] {
                Item::Agg(d) if d.kind.extreme() => {
                    let (init, step) = self.rb_extreme(d, &local);
                    o.push_str(&init);
                    body.push_str(&step);
                }
                Item::Agg(d) => {
                    o.push_str(&format!("    {} = 0  # {}\n", self.ident(&d.name.text), self.agg_word(d)));
                    let v = local(&d.column.text);
                    if d.kind == crate::ast::AggKind::Sum {
                        body.push_str(&format!("    {} += {v}  # {}\n", self.ident(&d.name.text), d.name.text));
                    } else {
                        let test = self.rb_test(&v, &d.column.text, &d.value);
                        body.push_str(&format!("    if {test}  # {}\n      {} += 1\n    end\n", d.name.text, self.ident(&d.name.text)));
                    }
                    if let Some((_, cap)) = self.sum_caps().into_iter().find(|(x, _)| x.name.text == d.name.text) {
                        let n = self.ident(&d.name.text);
                        after.push_str(&format!(
                            "    raise RuleInputError.new(\"{}\", {n}) if {n} > {cap}\n",
                            crate::door::out_of_range(&d.name.text)
                        ));
                    }
                    // The bottom of its range, where a walk can come in under it (§15.314).
                    if let Some(floor) = self.walk_floor(d) {
                        let n = self.ident(&d.name.text);
                        after.push_str(&format!(
                            "    raise RuleInputError.new(\"{}\", {n}) if {n} < {floor}\n",
                            crate::door::out_of_range(&d.name.text)
                        ));
                    }
                }
                Item::Fold(fold) => {
                    let (init, step, fin) = self.rb_fold(i, fold, &seq, &local);
                    o.push_str(&init);
                    body.push_str(&step);
                    after.push_str(&fin);
                }
                _ => {}
            }
        }
        // The index of the element is bound where a sentence of the walk says it (§15.324).
        if self.walk_end() {
            o.push_str(&format!("    {seq}.each_with_index do |{e}, {index}|\n"));
        } else {
            o.push_str(&format!("    {seq}.each do |{e}|\n"));
        }
        o.push_str(&Self::indent_block(&body, "    "));
        o.push_str("    end\n");
        o.push_str(&after);
        o
    }

    /// A test on a column of one element, in Ruby: `count`'s, and the `where` of `max` and `min`.
    fn rb_test(&self, v: &str, col: &str, value: &Option<crate::ast::Name>) -> String {
        let ty = self.ty_of(col);
        match (&ty, value) {
            (Ty::Bool, Some(w)) if w.text == crate::kw::FALSE => format!("!{v}"),
            (Ty::Bool, _) => v.to_string(),
            (_, Some(w)) => format!("{v} == {}", self.rb_value(&w.text, &ty)),
            (_, None) => v.to_string(),
        }
    }

    /// A `max` or a `min`, in Ruby (§15.219).
    fn rb_extreme(&self, d: &crate::ast::AggDecl, local: &dyn Fn(&str) -> String) -> (String, String) {
        let n = self.ident(&d.name.text);
        let set = self.temp(&format!("{n}_set"));
        let v = local(&d.column.text);
        let ty = self.ty_of(&d.column.text);
        let op = if d.kind == crate::ast::AggKind::Max { ">" } else { "<" };
        let (start, better) = match self.extreme_enum(d) {
            Some(en) => {
                let t = self.enum_type(&en);
                let w = self.extreme_empty_word(d).unwrap_or_default();
                (self.rb_value(&w, &ty), format!("_rank_{t}({v}) {op} _rank_{t}({n})"))
            }
            None => (self.extreme_empty_int(d).unwrap_or_else(|| "0".into()), format!("{v} {op} {n}")),
        };
        let init = format!("    {n} = {start}  # {}\n    {set} = false\n", self.agg_word(d));
        let mut step = String::new();
        let pad = match &d.filter {
            Some((w, want)) => {
                step.push_str(&format!("    if {}  # {}\n", self.rb_test(&local(&w.text), &w.text, want), d.name.text));
                "  "
            }
            None => "",
        };
        step.push_str(&format!("{pad}    if !{set} || {better}\n{pad}      {n} = {v}\n{pad}      {set} = true\n{pad}    end\n"));
        if !pad.is_empty() {
            step.push_str("    end\n");
        }
        (init, step)
    }

    /// A fold, in Ruby (§15.56).
    fn rb_fold(&self, i: usize, fold: &crate::ast::FoldDecl, seq: &str, local: &dyn Fn(&str) -> String) -> (String, String, String) {
        let names = self.fold_names(i);
        let FoldNames { answer, stopped, done, taken, kept, best, held, key, taken_at, .. } = &names;
        let alone = self.fold_alone(i);
        // Ends the loop at a `stop`: a fold alone in its walk with no door to keep past it (§15.328).
        let ends = alone && !self.stop_keeps_door(i);
        let with_held = |n: &str| -> String { if n == crate::kw::HELD { held.clone() } else { local(n) } };
        let (out_name, out_ty) = self.fold_target(fold);
        // An answer is read in the type the fold answers with: a value of its enum is that
        // enum's member (§15.208).
        let val = |x: &Expr, l: &dyn Fn(&str) -> String| -> String {
            match x {
                Expr::Lit(Lit::Word(w), _) if w != crate::kw::TRUE && w != crate::kw::FALSE => self.rb_value(w, &out_ty),
                _ => rb_expr(unparen(&self.expr(x, l).text)).to_string(),
            }
        };
        let text = |x: &Option<Expr>| -> String { x.as_ref().map(|x| val(x, &with_held)).unwrap_or_else(|| "0".to_string()) };

        let mut init = String::new();
        init.push_str(&format!("    {answer} = {}  # {}\n", text(&fold.empty), tr!("要素ゼロ件の答え", "the answer for no elements")));
        init.push_str(&format!("    {stopped} = false\n"));
        if !ends {
            init.push_str(&format!("    {done} = false\n"));
        }
        // steep reads the first assignment as the whole type of a local, so a `nil` that
        // later holds a value needs the annotation (§15.23). The value is the fold's answer; a
        // `keep_max` key is ordered, so it is always an integer.
        let acc = if out_ty == Ty::Unknown { "untyped".to_string() } else { self.rbs_ty(&out_ty) };
        init.push_str(&format!("    {taken} = nil #: {acc}?\n    {kept} = nil #: {acc}?\n"));
        // The key of a `keep_max`, only with one: Ruby warns under `-w` of a local never read (§15.340).
        if fold.arms.iter().any(|(_, a, _)| matches!(a, Arm::KeepMax { .. })) {
            init.push_str(&format!("    {best} = nil #: Integer?\n"));
        }
        // The element a `take_unique` took, which the sentence says beside the one that matched
        // again (§15.324).
        if fold.arms.iter().any(|(_, a, _)| matches!(a, Arm::Take { unique: true, .. })) {
            init.push_str(&format!("    {taken_at} = 0\n"));
        }
        let weighted = fold.weighted();
        let draw = fold.draw.as_ref().map(|d| local(&d.text)).unwrap_or_default();
        if weighted {
            init.push_str(&format!("    {} = 0  # {}\n", names.total, tr!("重みの合計", "the running total of the weights")));
            init.push_str(&format!("    {} = [] #: Array[Integer]\n", names.cuts));
        }

        let mut arms = String::new();
        let v = local(&fold.verdict);
        for (name, arm, _) in &fold.arms {
            let is = self.rb_value(&name.text, &self.ty_of(&fold.verdict));
            arms.push_str(&format!("    if {v} == {is}  # {}\n", name.text));
            match arm {
                Arm::Next => arms.push_str("      # next\n"),
                Arm::Stop(None) if ends => arms.push_str("      break\n"),
                Arm::Stop(None) => arms.push_str(&format!("      {done} = true\n")),
                Arm::Stop(Some(x)) => {
                    arms.push_str(&format!("      {answer} = {}\n", val(x, local)));
                    if ends {
                        arms.push_str(&format!("      {stopped} = true\n      break\n"));
                    } else {
                        arms.push_str(&format!("      {stopped} = true\n      {done} = true\n"));
                    }
                }
                Arm::Take { expr, unique } => {
                    if *unique {
                        let here = self.here_used();
                        arms.push_str(&format!(
                            "      raise RuleContradictionError.new({}) unless {taken}.nil?\n",
                            rb_said(&crate::door::fold_contradiction(&fold.verdict, &fold.over, crate::door::BEFORE, crate::door::HERE), &here, taken_at)
                        ));
                        arms.push_str(&format!("      {taken} = {}\n      {taken_at} = {here}\n", val(expr, local)));
                    } else {
                        arms.push_str(&format!("      {taken} = {} if {taken}.nil?\n", val(expr, local)));
                    }
                }
                Arm::KeepMax { expr, key: k } => {
                    arms.push_str(&format!("      {key} = {}\n", rb_expr(unparen(&self.expr(k, local).text))));
                    arms.push_str(&format!("      if {best}.nil? || {key} > {best}\n"));
                    arms.push_str(&format!("        {best} = {key}\n"));
                    arms.push_str(&format!("        {kept} = {}\n      end\n", val(expr, local)));
                }
                // The weight goes on the running total, and the element is taken where the drawn
                // value is below the total for the first time (§15.226). A total past int64 is
                // refused there, before another element is read: the other languages hold it in an
                // int64, and every language answers alike.
                Arm::TakeWeighted { expr, weight } => {
                    let (w, t, c) = (&names.weight, &names.total, &names.cuts);
                    arms.push_str(&format!("      {w} = {}\n", rb_expr(unparen(&self.expr(weight, local).text))));
                    arms.push_str(&format!("      {t} += {w}\n"));
                    arms.push_str(&format!("      raise RuleInputError.new(\"{}\") if {t} > 9223372036854775807\n", crate::door::weights_past(&fold.over)));
                    arms.push_str(&format!("      {c} << {t}\n"));
                    arms.push_str(&format!("      {taken} = {} if {taken}.nil? && {draw} < {t}\n", val(expr, local)));
                }
            }
            arms.push_str("    end\n");
        }
        let step = if alone { arms } else { format!("    unless {done}  # {}\n{}    end\n", fold.verdict, Self::indent_block(&arms, "  ")) };

        // The drawn value of a pick by weight is below the total of the weights, unless they add up
        // to 0 (§15.226).
        let mut fin = String::new();
        if weighted {
            let t = &names.total;
            let what = fold.draw.as_ref().map(|d| crate::door::draw_past(&d.text)).unwrap_or_default();
            fin.push_str(&format!("    raise RuleInputError.new(\"{what}\", {draw}) if {t} > 0 && {draw} >= {t}\n"));
        }
        fin.push_str(&format!("    unless {seq}.empty? || {stopped}\n"));
        fin.push_str(&format!("      {held} = {taken} || {kept} || {}\n", text(&fold.empty)));
        // An `exhausted` that does not read what was held, as Go's (§15.340).
        let ex = text(&fold.exhausted);
        if !clash::has_word(&ex, held) {
            fin.push_str(&format!("      _ = {held}\n"));
        }
        fin.push_str(&format!("      {answer} = {ex}\n    end\n"));
        fin.push_str(&format!("    {} = {answer}\n", self.ident(&out_name)));
        fin.push_str(&self.rb_unread(&out_name));
        (init, step, fin)
    }

    /// The fields marked `unique` (§15.290), in Ruby: a hash for each before the first walk over the
    /// sequence, and in the walk, after the guards of one element, the refusal of a value an element
    /// before it had.
    fn rb_unique(&self, s: usize, sums: &[usize], local: &dyn Fn(&str) -> String) -> (String, String) {
        let seq = &self.f.elements[s].name.text;
        let (mut init, mut step) = (String::new(), String::new());
        for fd in self.unique_fields(s, sums) {
            let seen = self.temp(&format!("{}_seen", self.ident(&fd.name.text)));
            let v = local(&fd.name.text);
            // Where each value was first seen, which the sentence says beside the element it came
            // round again on (§15.324).
            let here = self.here_used();
            init.push_str(&format!("    {seen} = {{}}\n"));
            step.push_str(&format!(
                "    raise RuleInputError.new({}, {v}) if {seen}.key?({v})\n    {seen}[{v}] = {here}\n",
                rb_said(&crate::door::duplicate(seq, &fd.name.text, crate::door::BEFORE, crate::door::HERE), &here, &format!("{seen}[{v}]"))
            ));
        }
        (init, step)
    }

    /// A whole float of an element's field is the integer it is, here as at the inputs (§15.205):
    /// what reads the element next reads a copy that holds the integer. The value is taken in a
    /// local first: steep 1.10 narrows a method call under `is_a?`, and a Float is not what the
    /// signature gives the field, so `elem.amount` after it would read as one and `total +=` of
    /// an Integer would not take it (§15.347).
    fn rb_element_whole(&self, v: &str) -> String {
        let Some((e, field)) = v.rsplit_once('.') else { return String::new() };
        let w = self.temp(&format!("{field}_w"));
        format!("    {w} = {v}\n    {e} = {e}.dup.tap {{ |x| x[:{field}] = {w}.to_i }} if {w}.is_a?(Float) && {w}.finite? && {w} == {w}.floor\n")
    }

    /// The first look at each element of every sequence, in Ruby (§15.328): before any input is
    /// held to its range, each element in turn, and each of its fields in the order the rule
    /// declares them, of its kind — a value of its enum, a truth value, a String (§15.339), an
    /// Integer, and one that int64 holds, as a reader of the wire takes it.
    fn rb_element_kinds(&self) -> String {
        let mut out = String::new();
        for (s, el) in self.f.elements.iter().enumerate() {
            let fields: BTreeMap<String, String> = el.fields.iter().map(|fd| (fd.name.text.clone(), pub_name(&fd.name))).collect();
            let e = self.elem_var(s);
            let local = |n: &str| -> String { fields.get(n).map(|f| format!("{e}.{f}")).unwrap_or_else(|| n.to_string()) };
            let index = self.index_var(s);
            self.walk_begin(s, &index);
            let mut o = String::new();
            let here = self.here();
            for i in self.element_fields(s) {
                let v = local(&i.name.text);
                // The sentences name the field by its place on the element (§15.324).
                let said = || self.said_name(&i.name.text);
                let ty = self.ty_of(&i.name.text);
                // An optional field is looked at as the type it wraps when it has a value (§15.201).
                let optional = matches!(ty, Ty::Opt(_));
                let ty = ty.present().clone();
                let mut o = Guarded::new(&mut o, optional.then(|| (format!("    unless {v}.nil?\n"), "    end\n".to_string())), "  ");
                match &ty {
                    Ty::Bool => o.push_str(&format!(
                        "    raise RuleInputError.new({}, {v}) unless {v} == true || {v} == false\n",
                        rb_said(&not_bool(&said()), &here, "")
                    )),
                    Ty::Str => o.push_str(&format!(
                        "    raise RuleInputError.new({}, {v}) unless {v}.is_a?(String)\n",
                        rb_said(&crate::door::not_string(&said()), &here, "")
                    )),
                    // A field of an enum is held to its values, as the main door holds an input
                    // and the other languages' element doors hold a field (§15.204).
                    Ty::Enum(n) => {
                        let cls = rb_const(&self.enum_names.get(n).cloned().unwrap_or_default());
                        o.push_str(&format!(
                            "    raise RuleInputError.new({}, {v}) unless {cls}::ALL.include?({v})\n",
                            rb_said(&crate::door::not_in_enum(&said(), n), &here, "")
                        ));
                    }
                    Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
                        if ty != Ty::Date {
                            o.push_str(&self.rb_element_whole(&v));
                        }
                        o.push_str(&format!(
                            "    raise RuleInputError.new({}, {v}) unless {v}.is_a?(Integer)\n",
                            rb_said(&crate::door::not_integer(&said()), &here, "")
                        ));
                        if ty != Ty::Date {
                            o.push_str(&format!(
                                "    raise RuleInputError.new({}, {v}) {}\n",
                                rb_said(&crate::door::out_of_range(&said()), &here, ""),
                                rb_out_of(&v, INT64_ENDS).unwrap_or_default()
                            ));
                        }
                    }
                    _ => {}
                }
            }
            let used = self.walk_end();
            if !o.is_empty() {
                let seq = self.held(&el.name);
                if used {
                    out.push_str(&format!("    {seq}.each_with_index do |{e}, {index}|\n"));
                } else {
                    out.push_str(&format!("    {seq}.each do |{e}|\n"));
                }
                out.push_str(&Self::indent_block(&o, "    "));
                out.push_str("    end\n");
            }
        }
        out
    }

    /// The door's look at one element where the walk comes to it (§15.328): each field inside the
    /// range it declares, in the order the rule declares them, on the values the first look found
    /// of their kinds.
    fn rb_element_ranges(&self, s: usize, local: &dyn Fn(&str) -> String) -> String {
        let mut o = String::new();
        let here = self.here();
        for i in self.element_fields(s) {
            let v = local(&i.name.text);
            // The sentences name the field by its place on the element (§15.324). Asked for only
            // where one is written: a walk binds the index only where a sentence reads it.
            let said = || self.said_name(&i.name.text);
            let ty = self.ty_of(&i.name.text);
            // An optional input is guarded as the type it wraps whenever it has a value (§15.201).
            let optional = matches!(ty, Ty::Opt(_));
            let ty = ty.present().clone();
            let mut o = Guarded::new(&mut o, optional.then(|| (format!("    unless {v}.nil?\n"), "    end\n".to_string())), "  ");
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                // The walk reads the element again from the sequence, so a whole float is made the
                // integer it is again.
                if ty != Ty::Date {
                    o.push_str(&self.rb_element_whole(&v));
                }
                // Ruby's integers have no end, so the door holds a field to both ends its type
                // has where its range writes none (§15.295).
                if let Some(test) = rb_out_of(&v, crate::door::ends(self.c, &i.name.text, true)) {
                    o.push_str(&format!(
                        "    raise RuleInputError.new({}, {v}) {test}\n",
                        rb_said(&crate::door::out_of_range(&said()), &here, ""),
                    ));
                }
            }
        }
        o
    }

    fn rb_fn(&self) -> String {
        let fname = pub_name(&self.f.name);
        let outs = &self.f.outputs;
        let mut o = String::new();

        // One element is a row of inputs, so it is a Struct like `Output`.
        for (s, el) in self.f.elements.iter().enumerate() {
            let fs: Vec<String> = el.fields.iter().map(|fd| format!(":{}", pub_name(&fd.name))).collect();
            o.push_str(&format!("  # {}\n", tr!("{} の一件", "one of {}", el.name.text)));
            o.push_str(&format!("  {} = Struct.new({})\n\n", self.element_type(s), fs.join(", ")));
        }

        // Multiple outputs come back as a Struct; one output is the value itself (§8.5).
        if outs.len() > 1 {
            let fs: Vec<String> = outs.iter().map(|od| format!(":{}", pub_name(&od.name))).collect();
            o.push_str(&format!("  Output = Struct.new({})\n\n", fs.join(", ")));
        }

        // The signature, with each parameter's declared type as a comment: this is where
        // the unit lives, since Ruby cannot hold it (§15.20).
        let traced = format!("{fname}_traced");
        o.push_str(&format!("  # {}\n", plain_doc(&self.f.name.text, &self.f.version, &traced)));
        for i in &self.f.inputs {
            o.push_str(&format!(
                "  #   {} : {}\n",
                pub_name(&i.name),
                self.rb_doc_ty(&self.ty_of(&i.name.text)).replace("Integer  # ", "")
            ));
        }
        for od in outs {
            o.push_str(&format!(
                "  # -> {} : {}\n",
                pub_name(&od.name),
                self.rb_doc_ty(&self.ty_of(&od.name.text)).replace("Integer  # ", "")
            ));
        }

        // The public method keeps the plain shape; the branches live in its traced twin,
        // which also returns the rows that matched (§15.33).
        let mut params: Vec<String> = self.f.inputs.iter().map(|i| self.held(&i.name)).collect();
        for el in &self.f.elements {
            params.push(self.held(&el.name));
        }
        o.push_str(&format!(
            "  def self.{}({})\n    {traced}({})[0]\n  end\n\n",
            self.fam(&fname),
            params.join(", "),
            params.join(", ")
        ));
        o.push_str(&format!("  # {}\n", traced_doc(&self.f.name.text, &self.f.version)));
        o.push_str(&format!("  def self.{traced}({})\n", params.join(", ")));
        // A rule that picks by weight runs its walk in a method of its own, which gives back the
        // running totals of the weights too (§15.225).
        let walked = self.weighted_fold().map(|(i, _)| (self.walked_fn("ruby"), self.fold_names(i).cuts));
        if let Some((w, _)) = &walked {
            o.push_str(&format!("    out, trace, _ = {w}({})\n    [out, trace]\n  end\n\n", params.join(", ")));
            o.push_str(&format!("  # {}\n  def self.{w}({})\n", walked_doc(), params.join(", ")));
        }

        // Entry guards (§8.5). An input with two or more things wrong with it is refused for the
        // first of them in the order every door looks (§15.328): each input of its kind, then each
        // element of each sequence, then each input inside its range, then the constraints.
        let local = |n: &str| -> String { self.ident(n) };
        for i in &self.f.inputs {
            let v = self.held(&i.name);
            let ty = self.ty_of(&i.name.text);
            // An optional input is guarded as the type it wraps whenever it has a value (§15.201).
            let optional = matches!(ty, Ty::Opt(_));
            let ty = ty.present().clone();
            let mut o = Guarded::new(&mut o, optional.then(|| (format!("    unless {v}.nil?\n"), "    end\n".to_string())), "  ");
            match &ty {
                // A truth value is `true` or `false` and nothing else (§15.203).
                Ty::Bool => o.push_str(&format!(
                    "    raise RuleInputError.new(\"{}\", {v}) unless {v} == true || {v} == false\n",
                    not_bool(&i.name.text)
                )),
                // A string is a String and nothing else (§15.339): a number given for it is equal
                // to no cell of the table, and fell to another row without a word.
                Ty::Str => o.push_str(&format!(
                    "    raise RuleInputError.new(\"{}\", {v}) unless {v}.is_a?(String)\n",
                    crate::door::not_string(&i.name.text)
                )),
                Ty::Enum(n) => {
                    let cls = rb_const(&self.enum_names.get(n).cloned().unwrap_or_default());
                    o.push_str(&format!(
                        "    raise RuleInputError.new(\"{}\", {v}) unless {cls}::ALL.include?({v})\n",
                        crate::door::not_in_enum(&i.name.text, n)
                    ));
                }
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
                    // `cover?` is true of a Float inside the range, so the kind is checked
                    // first (§15.43). A Float whose value is whole, `1000.0` from a caller's
                    // JSON, is that integer (§15.205).
                    if ty != Ty::Date {
                        o.push_str(&format!("    {v} = {v}.to_i if {v}.is_a?(Float) && {v}.finite? && {v} == {v}.floor\n"));
                    }
                    o.push_str(&format!(
                        "    raise RuleInputError.new(\"{}\", {v}) unless {v}.is_a?(Integer)\n",
                        crate::door::not_integer(&i.name.text)
                    ));
                    // A whole number int64 does not hold, where a reader of the wire refuses it.
                    if ty != Ty::Date {
                        o.push_str(&format!(
                            "    raise RuleInputError.new(\"{}\", {v}) {}\n",
                            crate::door::out_of_range(&i.name.text),
                            rb_out_of(&v, INT64_ENDS).unwrap_or_default()
                        ));
                    }
                }
                _ => {}
            }
        }
        o.push_str(&self.rb_element_kinds());
        for i in &self.f.inputs {
            let v = self.held(&i.name);
            let ty = self.ty_of(&i.name.text);
            let optional = matches!(ty, Ty::Opt(_));
            let ty = ty.present().clone();
            let mut o = Guarded::new(&mut o, optional.then(|| (format!("    unless {v}.nil?\n"), "    end\n".to_string())), "  ");
            match &ty {
                // The values the range of an ordered enum lets in, first to last (§15.210).
                Ty::Enum(n) => {
                    if let Some((lo, hi)) = self.c.enum_ranges.get(&i.name.text).and_then(|vs| Some((vs.first()?, vs.last()?))) {
                        let r = self.rb_rank_fn(n);
                        o.push_str(&format!(
                            "    raise RuleInputError.new(\"{}\", {v}) unless ({r}({})..{r}({})).cover?({r}({v}))\n",
                            crate::door::out_of_range(&i.name.text),
                            self.rb_value(lo, &ty),
                            self.rb_value(hi, &ty)
                        ));
                    }
                }
                // Each end the range writes, and the end of the type where it writes none: an
                // Integer here can be any size (§15.295).
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
                    if let Some(test) = rb_out_of(&v, crate::door::ends(self.c, &i.name.text, true)) {
                        o.push_str(&format!(
                            "    raise RuleInputError.new(\"{}\", {v}) {test}\n",
                            crate::door::out_of_range(&i.name.text),
                        ));
                    }
                }
                _ => {}
            }
        }

        o.push_str(&self.constraint_guards(&local, Lang::Rb, "    ", |m| {
            format!("      raise RuleInputError.new(\"{m}\")\n")
        }));

        let trace = self.temp("trace");
        o.push_str(&format!("    {trace} = [] #: Array[Fired]\n"));

        if self.f.walks() {
            o.push_str(&self.rb_walks(&local, &trace));
        } else {
            o.push_str(&self.rb_items(&local, &trace, Phase::All));
        }

        // Every output: its source, brought to the wire scale, then its rounding once. An
        // optional output takes those steps only when its column has a value, and is `nil` when
        // it has none (§15.249).
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let opt = self.opt_out(oi);
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let res = self.onto_wire(res, os, od);
            let ty = self.ty_of(out_name);
            let body = match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    if res.scale != os && !opt {
                        let raw = self.temp(&raw_base(oi));
                        o.push_str(&format!(
                            "    {raw} = {}  # {}\n",
                            rb_expr(unparen(&res.text)),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("_round_{}({raw}, {}) / {}", mode_fn(m), grid_i, res.scale / os)
                    } else if res.scale != os {
                        format!("_round_{}({}, {}) / {}", mode_fn(m), rb_expr(unparen(&res.text)), grid_i, res.scale / os)
                    } else {
                        format!("_round_{}({}, {})", mode_fn(m), rb_expr(&res.text), grid_i)
                    }
                }
                None => rb_expr(&res.text),
            };
            let v = local(out_name);
            finals.push(if opt && body != v { format!("{v}.nil? ? nil : {body}") } else { body });
        }
        if outs.len() == 1 {
            let cuts = walked.as_ref().map(|(_, c)| format!(", {c}")).unwrap_or_default();
            o.push_str(&format!("    [{}, {trace}{cuts}]\n", finals[0]));
        } else {
            let cuts = walked.as_ref().map(|(_, c)| format!(", {c}")).unwrap_or_default();
            o.push_str(&format!("    [Output.new({}), {trace}{cuts}]\n", finals.join(", ")));
        }
        o.push_str("  end\n");
        o
    }

    fn rb_table(&self, t: &Table, local: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = format!("    # {}\n", self.table_head(t));
        for (ri, row) in t.rows.iter().enumerate() {
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.rb_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            let cond = if conds.is_empty() { "true".into() } else { conds.join(" && ") };
            let kw = if ri == 0 { "if" } else { "elsif" };
            let cells: Vec<String> = row.cells.iter().map(cell_src).chain(row.outs.iter().map(out_src)).collect();
            o.push_str(&format!("    {kw} {cond}  # {}\n", self.row_head(t, ri, &cells.join(" | "))));
            for (oi, oc) in t.outputs.iter().enumerate() {
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => {
                        let ty = self.ty_of(&oc.name.text);
                        self.int_lit(n, &ty, self.scale(&oc.name.text))
                    }
                    Some(OutCell::Lit(l)) => match l {
                        Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                        Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                        Lit::Word(w) => self.rb_value(w, &self.ty_of(&oc.name.text)),
                        Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                        Lit::Str(x) => str_lit(x),
                        _ => "0".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            "true".into()
                        } else if w == crate::kw::FALSE {
                            "false".into()
                        } else if self.is_value(w) {
                            self.rb_value(w, &self.ty_of(&oc.name.text))
                        } else {
                            let v = local(w);
                            match rb_expr(&self.rescaled(w, &oc.name.text, v.clone())) {
                                // A value that may be absent is scaled only when it is there.
                                r if r != v && matches!(self.ty_of(w), Ty::Opt(_)) => format!("{v}.nil? ? nil : {r}"),
                                r => r,
                            }
                        }
                    }
                    Some(OutCell::Nothing) => "nil".into(),
                    None => "0".into(),
                };
                o.push_str(&format!("      {} = {v}\n", self.ident(&oc.name.text)));
            }
            let (tn, rn, lbl) = self.fired_of(t, ri);
            if lbl.is_empty() {
                o.push_str(&format!("      {trace} << Fired.new({tn:?}, {rn})\n"));
            } else {
                o.push_str(&format!("      {trace} << Fired.new({tn:?}, {rn}, {lbl:?})\n"));
            }
        }
        o.push_str(&format!(
            "    else\n      raise RuleContradictionError.new(\"{}\")\n    end\n",
            tr!("到達不能: 完全性は rulec が静的に検査済み", "unreachable: completeness was statically checked by rulec")
        ));
        for oc in &t.outputs {
            o.push_str(&self.rb_unread(&oc.name.text));
        }
        o.push_str(&self.guards(t, local, Lang::Rb, "    ", |name, i, j| {
            format!(
                "      raise RuleContradictionError.new(\"{}\")\n",
                tr!("表 {name}: {i} と {j} が同時に当てはまりました", "table {name}: {i} and {j} matched at the same time")
            )
        }));
        o
    }
}

impl<'a> Gen<'a> {
    /// A Ruby runner that reads JSONL from stdin and prints just the outputs, one record
    /// per line. `json` and `date` are both standard library, so this needs no gem.
    pub fn ruby_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        let m = self.rb_module();
        // One input read off the wire the way every door reads it (§15.204): there at all, of
        // its kind, a date turned into its day number. A refusal is the module's own
        // `RuleInputError`, with the sentence every language says.
        // `said` is what the sentences call the value: the name, or for a field of an element its
        // place, `lines[\u{1}].amount`, which takes the index `i` of the element (§15.324).
        let read = |ty: &Ty, src: &str, name: &str, said: &str| -> String {
            let lit = |x: String| -> String { if names_element(&x) { rb_said(&x, "i", "") } else { format!("{x:?}") } };
            let optional = matches!(ty, Ty::Opt(_));
            let take = format!("_take({src}, {name:?}, {}{})", lit(crate::door::missing(said)), if optional { ", true" } else { "" });
            let one = |v: &str| -> String {
                match ty.present() {
                    Ty::Enum(n) => {
                        let cls = rb_const(&self.enum_names.get(n).cloned().unwrap_or_default());
                        format!("_enum({m}::{cls}::ALL, {v}, {})", lit(crate::door::not_in_enum(said, n)))
                    }
                    Ty::Bool => format!("_bool({v}, {})", lit(crate::door::not_bool(said))),
                    Ty::Date => format!("_date({v}, {})", lit(crate::door::not_date(said))),
                    Ty::Str => format!("_str({v}, {})", lit(crate::door::not_string(said))),
                    _ => format!("_int({v}, {}, {})", lit(crate::door::not_integer(said)), lit(crate::door::out_of_range(said))),
                }
            };
            if optional {
                format!("_maybe({take}) {{ |x| {} }}", one("x"))
            } else {
                one(&take)
            }
        };
        let mut args: Vec<String> = self.f.inputs.iter().map(|i| read(&self.ty_of(&i.name.text), "d", &i.name.text, &i.name.text)).collect();
        // The sequences a walk reads, one element at a time (§15.56, §15.219). A sentence about an
        // element names it by its index `i` (§15.324).
        for (s, el) in self.f.elements.iter().enumerate() {
            let seq = &el.name.text;
            let obj = format!("_obj(e, {})", rb_said(&crate::door::not_object(seq, crate::door::HERE), "i", ""));
            let fields: Vec<String> = el
                .fields
                .iter()
                .map(|fd| read(&self.ty_of(&fd.name.text), &obj, &fd.name.text, &crate::door::element(seq, crate::door::HERE, &fd.name.text)))
                .collect();
            args.push(format!(
                "_seq(_take(d, {seq:?}, {:?}), {:?}).each_with_index.map {{ |e, i| {m}::{}.new({}) }}",
                crate::door::missing(seq),
                crate::door::not_sequence(seq),
                self.element_type(s),
                fields.join(", ")
            ));
        }
        // `--chances` reads a line's inputs as the vectors are read (§15.225).
        let read_args = args.clone();
        // A machine's traces (§15.148): the carried input is the state this language answered
        // to the call before, handed over as the value it is.
        // A rule that reads no input reads nothing out of the record's `in`, which `ruby -w` warns
        // of where it is held unread (§15.330).
        let unread = if self.f.inputs.is_empty() && self.f.elements.is_empty() { "_d" } else { "d" };
        let (mut head, mut body, mut tail) = (String::new(), format!("{unread} = JSON.parse(line)[\"in\"]\n  "), String::new());
        if let (Some((en, _, _)), Some((cin, _)), Some(out)) = (self.machine_consts(), self.carried(), self.carried_out_alias()) {
            let cls = rb_const(&self.enum_names.get(&en).cloned().unwrap_or_default());
            if let Some(k) = self.f.inputs.iter().position(|i| i.name.text == cin) {
                args[k] = format!("(v.key?(\"step\") ? state : {})", args[k]);
            }
            // The values carried along with the state (§15.217): a local each, from the
            // module's constant, read where each trace starts and handed on from the answer.
            let (mut more_head, mut more_meta, mut more_start, mut more_tail) = (String::new(), Vec::<String>::new(), String::new(), String::new());
            for mc in self.more_carried() {
                let local = format!("carried_{}", mc.in_alias);
                if let Some(k) = self.f.inputs.iter().position(|i| i.name.text == mc.input) {
                    args[k] = format!("(v.key?(\"step\") ? {local} : {})", args[k]);
                }
                more_head.push_str(&format!("{local} = {m}::{}\n", mc.upper()));
                more_meta.push(format!("{} => {m}::{}", mc.key(), mc.upper()));
                more_start.push_str(&format!("{local} = {} if v[\"step\"] == \"start\"\n  ", read(&mc.ty, "v[\"carried\"]", &mc.input, &mc.input)));
                more_tail.push_str(&format!("  {local} = r.{} if v.key?(\"step\")\n", mc.out_alias));
            }
            let carried_meta = if more_meta.is_empty() { String::new() } else { format!(", \"carried\" => {{ {} }}", more_meta.join(", ")) };
            head = format!("state = {m}::INITIAL\n{more_head}");
            body = format!(
                "v = JSON.parse(line)\n  \
                 if v.key?(\"machine\")\n    \
                   puts JSON.generate({{ \"initial\" => {m}::INITIAL, \"final\" => {m}::{cls}::ALL.select {{ |s| {m}.final?(s) }}{carried_meta} }})\n    \
                   next\n  \
                 end\n  \
                 d = v[\"in\"]\n  \
                 state = v[\"state\"] if v[\"step\"] == \"start\"\n  {more_start}"
            );
            tail = format!("  state = {} if v.key?(\"step\")\n{more_tail}", self.next_state_of("r", &format!(".{out}")));
        }
        format!(
            "# Code generated by rulec {}. DO NOT EDIT.\n\
             # frozen_string_literal: true\n\n\
             require \"date\"\n\
             require \"json\"\n\
             require_relative \"{alias}\"\n\n\
             {}\
             {}\
             {head}\
             STDIN.each_line do |line|\n  \
               line = line.strip\n  \
               next if line.empty?\n  \
               {body}\
               begin\n    \
                 args = [{}]\n    \
                 r, trace = {m}.{alias}_traced(*args)\n  \
               rescue {m}::RuleInputError => e\n    \
                 _refused(\"input\", e.what)\n    \
                 next\n  \
               rescue {m}::RuleContradictionError => e\n    \
                 _refused(\"contradiction\", e.what)\n    \
                 next\n  \
               end\n  \
               puts {m}.{alias}_record(*args, r, trace)\n\
             {tail}\
             end\n",
            ritsu_emit::header::VERSION,
            RB_WIRE.replace("@M@", &m),
            self.rb_labels_mode() + &self.rb_prereq_mode() + &self.rb_chances_mode(&read_args),
            args.join(", ")
        )
    }

    /// What the Ruby runner answers with `--labels` (§15.211): for each line, an enum's name and
    /// a value's, the display name the module gives.
    fn rb_labels_mode(&self) -> String {
        let enums = self.labeled_enums();
        if enums.is_empty() {
            return String::new();
        }
        let m = self.rb_module();
        let mut arms = String::new();
        for (en, _, _) in &enums {
            // the function of an `enums` file's enum by its module, as the rule's code calls it
            let f = if self.c.owners.contains_key(en) { self.rb_label_fn(en) } else { format!("{m}.{}", self.rb_label_fn(en)) };
            arms.push_str(&format!("    when {} then puts {f}(v)\n", rb_str(en)));
        }
        format!(
            "if ARGV.include?(\"--labels\")\n  STDIN.each_line do |line|\n    en, v = line.chomp.split(\"\\t\", 2)\n    case en\n{arms}    end\n  end\n  exit 0\nend\n\n"
        )
    }
}

/// How the generated Ruby runner reads the wire (§15.204): the readers of `PY_WIRE` in Ruby.
const RB_WIRE: &str = r#"# The calendar of every year (Date::GREGORIAN): by default Date takes the Julian calendar before
# 1582-10-15, where 1500-02-29 is a day and 1582-10-10 is not.
def _ord(s)
  Date.iso8601(s, Date::GREGORIAN).jd - Date.new(1970, 1, 1).jd
end

def _take(d, k, missing, optional = false)
  raise @M@::RuleInputError.new(missing) unless d.is_a?(Hash) && d.key?(k) && (optional || !d[k].nil?)

  d[k]
end

def _int(v, what, past)
  # A whole number written with a point or an exponent, 1000.0 or 1e3, is that integer.
  n = v.is_a?(Float) && v.finite? && v == v.floor ? v.to_i : v
  raise @M@::RuleInputError.new(what, v) unless n.is_a?(Integer)
  # One int64 does not hold is past what a reader of every language takes, there and not later.
  raise @M@::RuleInputError.new(past, v) unless n.between?(-9223372036854775808, 9223372036854775807)

  n
end

def _bool(v, what)
  raise @M@::RuleInputError.new(what, v) unless v == true || v == false

  v
end

def _str(v, what)
  raise @M@::RuleInputError.new(what, v) unless v.is_a?(String)

  v
end

def _date(v, what)
  raise @M@::RuleInputError.new(what, v) unless v.is_a?(String) && v.match?(/\A(?!0000)[0-9]{4}-[0-9]{2}-[0-9]{2}\z/)

  begin
    _ord(v)
  rescue Date::Error
    raise @M@::RuleInputError.new(what, v)
  end
end

def _enum(all, v, what)
  raise @M@::RuleInputError.new(what, v) unless all.include?(v)

  v
end

def _seq(v, what)
  raise @M@::RuleInputError.new(what, v) unless v.is_a?(Array)

  v
end

def _obj(v, what)
  raise @M@::RuleInputError.new(what, v) unless v.is_a?(Hash)

  v
end

def _maybe(v)
  v.nil? ? nil : yield(v)
end

def _refused(kind, what)
  puts JSON.generate({ "refused" => kind, "error" => what })
end

"#;

/// The five modes of §7.3 in Ruby, checked against the same reference cases the other
/// others are checked against. `rulec test` runs it beside the generated module.
pub fn round_tests_ruby(u: &Uses) -> String {
    let mut o = format!(
        "# Code generated by rulec {}. DO NOT EDIT.\n\
         # frozen_string_literal: true\n\
         # {}\n\n\
         module R\n",
        ritsu_emit::header::VERSION,
        tr!(
            "丸めの五つのモード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five rounding modes, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str(round_rb().trim_start_matches('\n'));
    o.push_str(&arith::rb_helpers(u));
    o.push_str("  public_class_method :_round_down, :_round_up, :_round_half, :_round_half_down, :_round_bankers\n");
    if u.any() {
        o.push_str(&format!("  public_class_method {}\n", arith::rb_names(u).join(", ")));
    }
    o.push_str("end\n\nCASES = [\n");
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("  [{:?}, {x}, {g}, {want}],\n", mode_fn(m)));
    }
    o.push_str("].freeze\n\nbad = 0\nCASES.each do |mode, x, g, want|\n  ");
    o.push_str("got = R.send(\"_round_#{mode}\", x, g)\n  next if got == want\n  ");
    o.push_str("puts \"NG #{mode}(#{x}, #{g}) = #{got}, want #{want}\"\n  bad += 1\nend\n");
    let (arith, n) = arith::rb_tests(u);
    o.push_str(&arith);
    o.push_str("exit(1) unless bad.zero?\n");
    let count = if n == 0 { "CASES.size" } else { "CASES.size + ARITH.size" };
    o.push_str(&format!("puts \"{}\"\n", tr!("ok #{{{count}}} 件", "ok #{{{count}}} cases")));
    o
}

impl<'a> Gen<'a> {
    /// The type as the inventory states it for Ruby. There are no brands here (§15.20), so
    /// a number is `Integer` and the unit travels in the entry's own `unit` field.
    fn rb_api_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => rb_const(&self.enum_names.get(n).cloned().unwrap_or_else(|| "String".into())),
            Ty::Bool => "Boolean".into(),
            Ty::Str => "String".into(),
            Ty::Opt(t) => format!("{} | nil", self.rb_api_ty(t)),
            _ => "Integer".into(),
        }
    }
}

impl<'a> Gen<'a> {
    /// A cell as a condition in one language. The one place that maps a `Lang` to its cell
    /// renderer, so that shared code never has to know the set (§15.20).
    fn cell(&self, lang: Lang, cell: &Cell, var: &str, ty: &Ty, scale: i128) -> Option<String> {
        match lang {
            Lang::Py => self.py_cell(cell, var, ty, scale),
            Lang::Go => self.go_cell(cell, var, ty, scale),
            Lang::Ts => self.ts_cell(cell, var, ty, scale),
            Lang::Rs => self.rs_cell(cell, var, ty, scale),
            Lang::Rb => self.rb_cell(cell, var, ty, scale),
            Lang::Sw => self.sw_cell(cell, var, ty, scale),
            Lang::Sql => self.sql_cell(cell, var, ty, scale),
            Lang::Php => self.php_cell(cell, var, ty, scale),
            Lang::Java => self.java_cell(cell, var, ty, scale),
        }
    }
}

impl<'a> Gen<'a> {
    /// The RBS type name for a value. There is no newtype in RBS either, so a unit is not
    /// expressible and every number is `Integer` (§15.23) — but an enum **is**, as the
    /// union of its own values, which closes the set at the type level.
    fn rbs_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => self
                .enum_names
                .get(n)
                .map(|a| snake(a))
                .unwrap_or_else(|| "String".into()),
            Ty::Bool => "bool".into(),
            Ty::Str => "String".into(),
            Ty::Opt(t) => format!("{}?", self.rbs_ty(t)),
            _ => "Integer".into(),
        }
    }

    /// The signature file that ships beside the generated Ruby. `steep` reads it and
    /// refuses a caller that passes the wrong kind, the wrong count, or a string that is
    /// not one of an enum's values. It cannot refuse grams where yen were meant — RBS has
    /// no newtype, and a type alias is the same type (§15.23).
    /// One enum in the signature: the union of its values, its module of constants, and the
    /// functions for its order and its display names when it has them (§15.210, §15.211).
    pub(crate) fn rbs_enum(&self, jp: &str, ascii: &str) -> String {
        let Some(vals) = self.c.enums.get(jp) else { return String::new() };
        let lits: Vec<String> = vals.iter().map(|v| format!("{v:?}")).collect();
        let mut o = format!("  type {} = {}\n\n", snake(ascii), lits.join(" | "));
        o.push_str(&format!("  module {}\n", rb_const(ascii)));
        for v in vals {
            o.push_str(&format!("    {}: {v:?}\n", rb_const(&self.py_member(jp, v))));
        }
        o.push_str(&format!("    ALL: Array[{}]\n  end\n\n", snake(ascii)));
        if self.c.ordered.contains(jp) {
            o.push_str(&format!("  def self.{}: ({}) -> Integer\n", self.rb_rank_fn(jp), snake(ascii)));
        }
        if self.c.labels.contains_key(jp) {
            o.push_str(&format!("  def self.{}: ({}) -> String\n", self.rb_label_fn(jp), snake(ascii)));
        }
        if self.c.ordered.contains(jp) || self.c.labels.contains_key(jp) {
            o.push('\n');
        }
        o
    }

    /// The signatures name a type from inside the rule's module, where an enum's type hides a class
    /// of its name (§15.336).
    pub fn rbs(&self) -> String {
        self.apart_types(clash::Syntax::Ruby, |g| g.rbs_text())
    }

    fn rbs_text(&self) -> String {
        let m = self.rb_module();
        let mut o = self.header("#");
        o.push_str(&format!("\nmodule {m}\n"));

        // Enums, as a union of the values themselves.
        for (jp, ascii) in self.own_enums() {
            o.push_str(&self.rbs_enum(&jp, &ascii));
        }
        // An enum of an `enums` file is that module's: its type and its module by another name
        // (§15.212), and the functions of its order and its display names (§15.319).
        for (module, types) in self.foreign_enums() {
            let owner = ruby_module(&module);
            for (jp, ascii) in types {
                // the file's module's names, should the enum be handed on under another type here
                // (§15.326)
                let owned = self.owner_type(&jp);
                o.push_str(&format!("  type {} = {owner}::{}\n", snake(&ascii), snake(&owned)));
                o.push_str(&format!("  module {} = {owner}::{}\n\n", rb_const(&ascii), rb_const(&owned)));
                let fns = self.handed_fns(&jp, &ascii);
                for (f, rank, _) in &fns {
                    let ret = if *rank { "Integer" } else { "String" };
                    o.push_str(&format!("  def self.{f}: ({}) -> {ret}\n", snake(&ascii)));
                }
                if !fns.is_empty() {
                    o.push('\n');
                }
            }
        }
        for en in self.c.enum_order.iter().filter(|e| self.c.ordered.contains(*e)) {
            let (rank, max, min) = self.enum_helpers(en);
            let t = self.enum_type(en);
            let ty = snake(&t);
            if rank {
                o.push_str(&format!("  private def self._rank_{t}: ({ty}) -> Integer\n"));
            }
            if max {
                o.push_str(&format!("  private def self._max_{t}: ({ty}, {ty}) -> {ty}\n"));
            }
            if min {
                o.push_str(&format!("  private def self._min_{t}: ({ty}, {ty}) -> {ty}\n"));
            }
            if rank || max || min {
                o.push('\n');
            }
        }
        // Which values need which before them (§15.222).
        o.push_str(&self.rbs_prereqs());

        if let Some((en, _, _)) = self.machine_consts() {
            let t = snake(&self.enum_names.get(&en).cloned().unwrap_or_default());
            // Where each value carried along with the state starts (§15.217).
            let more: String = self.more_carried().iter().map(|mc| format!("  {}: {}\n", mc.upper(), self.rbs_ty(&mc.ty))).collect();
            o.push_str(&format!("  INITIAL: {t}\n  FINAL: Array[{t}]\n{more}  def self.final?: ({t}) -> bool\n\n"));
        }

        o.push_str(
            "  class RuleInputError < ArgumentError\n    NO_VALUE: Object\n    attr_reader what: String\n    \
             attr_reader value: untyped\n    def initialize: (String, ?untyped) -> void\n  end\n\n",
        );
        o.push_str(
            "  class RuleContradictionError < RuntimeError\n    attr_reader what: String\n    \
             def initialize: (String) -> void\n  end\n\n",
        );

        for g in &self.f.groups {
            let el = self
                .c
                .groups
                .get(&g.name.text)
                .map(|(ty, _)| self.rbs_ty(&Ty::Enum(ty.clone())))
                .unwrap_or_else(|| "String".into());
            o.push_str(&format!("  GROUP_{}: Array[{el}]\n", self.rb_group(&g.name.text)));
        }
        if !self.f.groups.is_empty() {
            o.push('\n');
        }

        let outs = &self.f.outputs;
        if outs.len() > 1 {
            let tys: Vec<String> = outs.iter().map(|od| self.rbs_ty(&self.ty_of(&od.name.text))).collect();
            o.push_str("  class Output < Struct[untyped]\n");
            for (od, t) in outs.iter().zip(&tys) {
                o.push_str(&format!("    attr_reader {}: {t}\n", pub_name(&od.name)));
            }
            o.push_str(&format!("    def self.new: ({}) -> Output\n  end\n\n", tys.join(", ")));
        }

        // One element is a row of inputs, and the Ruby module makes it a Struct; without
        // this the signature has no name for what the caller passes (§15.56).
        for (s, el) in self.f.elements.iter().enumerate() {
            let ty = self.element_type(s);
            let tys: Vec<String> =
                el.fields.iter().map(|fd| self.rbs_ty(&self.ty_of(&fd.name.text))).collect();
            o.push_str(&format!("  class {ty} < Struct[untyped]\n"));
            for (fd, t) in el.fields.iter().zip(&tys) {
                o.push_str(&format!("    attr_reader {}: {t}\n", pub_name(&fd.name)));
            }
            o.push_str(&format!("    def self.new: ({}) -> {ty}\n  end\n\n", tys.join(", ")));
        }

        o.push_str(
            "  class Fired < Struct[untyped]\n    attr_reader table: String\n    attr_reader row: Integer\n    attr_reader label: String?\n    def self.new: (String, Integer, ?String) -> Fired\n  end\n\n",
        );

        // The rounding helpers are private at run time; declaring them keeps `steep` from
        // warning about a method the module defines and the signature does not mention.
        o.push_str(&format!("  # {}\n", tr!("丸めの補助。実行時には private で、外からは呼べない。", "The rounding helpers, private at run time.")));
        for h in ["_min", "_max", "_round_down", "_round_up", "_round_half", "_round_half_down", "_round_bankers"] {
            o.push_str(&format!("  private def self.{h}: (Integer, Integer) -> Integer\n"));
        }
        // And those of the arithmetic, where the rule calls them (§15.213 to §15.215).
        let uses = self.arith_uses();
        for m in &uses.div {
            o.push_str(&format!("  private def self.{}: (Integer, Integer) -> Integer\n", arith::py_div(*m)));
        }
        if uses.modulo {
            o.push_str("  private def self._mod: (Integer, Integer) -> Integer\n");
        }
        if uses.root {
            o.push_str("  private def self._isqrt: (Integer) -> Integer\n");
        }
        o.push('\n');

        let params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{} {}", self.rbs_ty(&self.ty_of(&i.name.text)), pub_name(&i.name)))
            .chain(self.f.elements.iter().enumerate().map(|(s, el)| format!("Array[{}] {}", self.element_type(s), pub_name(&el.name))))
            .collect();
        let ret = if outs.len() == 1 {
            self.rbs_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        o.push_str(&format!("  def self.{}: ({}) -> {ret}\n", pub_name(&self.f.name), params.join(", ")));
        o.push_str(&format!("  def self.{}_traced: ({}) -> [{ret}, Array[Fired]]\n", pub_name(&self.f.name), params.join(", ")));
        // The walk of a pick by weight, and the method that counts the answers (§15.225).
        o.push_str(&self.rbs_walked());
        o.push_str(&self.rbs_chances());
        let (out, trace, tag) = (self.temp("out"), self.temp("trace"), self.temp("tag"));
        o.push_str(&format!(
            "  def self.{}_record: ({}{ret} {out}, Array[Fired] {trace}, ?String {tag}) -> String\n",
            pub_name(&self.f.name),
            leading_params(&params)
        ));
        o.push_str("  private def self._json_str: (String) -> String\n");
        if civil_needed(self.f, self.c) {
            o.push_str("  private def self._civil: (Integer) -> String\n");
        }
        // The projection (§15.125). The caller's object is `Hash[String, untyped]`: the
        // signature says what the method takes without naming the object's own type, which
        // is the whole of the line this tool draws there.
        if self.projects() {
            let mut from: Vec<String> = self.proj_params().iter().map(|r| format!("Hash[String, untyped] {r}")).collect();
            from.extend(
                self.proj_rest()
                    .iter()
                    .map(|i| format!("{} {}", self.rbs_ty(&self.ty_of(&i.name.text)), pub_name(&i.name))),
            );
            // A sequence taken out of the object is built there, not passed (§15.223).
            from.extend(self.proj_seq_rest().into_iter().map(|s| format!("Array[{}] {}", self.element_type(s), pub_name(&self.f.elements[s].name))));
            o.push_str(&format!(
                "  def self.{}_from: ({}) -> {ret}\n",
                pub_name(&self.f.name),
                from.join(", ")
            ));
            // The helpers it reads the object with (§15.133, §15.234): the walk over a `.proto`
            // contract's JSON form, the day number of a date, and each value held to its kind.
            o.push_str(&self.rbs_from_helpers());
        }
        o.push_str("end\n");
        o
    }
}

/// PascalCase to snake_case, for an RBS type alias — those have to start lowercase.
fn snake(s: &str) -> String {
    if let Some(rest) = s.strip_prefix(clash::MARK) {
        return format!("{}{}", clash::MARK, snake(rest));
    }
    let mut o = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                o.push('_');
            }
            o.push(c.to_ascii_lowercase());
        } else {
            o.push(c);
        }
    }
    o
}

// ---------------------------------------------------------------------------
// Swift (§15.20). The second target after Rust that can hold a unit in the type: a
// single-field struct over `Int64` is laid out as the integer itself, so a compiler that
// refuses `YenExclTax` where `YenInclTax` was meant costs nothing at run time. Integer
// division truncates toward zero, as it does in Go, in Rust and on a JavaScript bigint, so
// the rounding helpers are those transliterated rather than Python's.
//
// Two spellings are Swift's own. Identifiers are lowerCamelCase — the convention its
// linters enforce, and the same reason the Go backend writes `pascal` — and a name that
// lands on a keyword goes in backticks, which is the one shape a rule can declare
// (`alias: in`) that no other target has to escape.

/// The reserved words that have to be written in backticks.
const SWIFT_KEYWORDS: &[&str] = &[
    "Any", "Protocol", "Self", "Type", "as", "associatedtype", "await", "break", "case",
    "catch", "class", "continue", "default", "defer", "deinit", "do", "else", "enum",
    "extension", "fallthrough", "false", "fileprivate", "for", "func", "guard", "if",
    "import", "in", "init", "inout", "internal", "is", "let", "nil", "operator", "private",
    "protocol", "public", "repeat", "rethrows", "return", "self", "static", "struct",
    "subscript", "super", "switch", "throw", "throws", "true", "try", "typealias", "var",
    "where", "while",
];

/// Lower the leading run of capitals: all of it, unless the run is longer than one and the
/// character after it is lowercase, in which case that last capital begins the next word.
/// `rulec import` spells an alias for a word of the sheet so too (`URLPath(urlPath)`, §15.266).
pub(crate) fn lower_lead(w: &str) -> String {
    let cs: Vec<char> = w.chars().collect();
    let run = cs.iter().take_while(|c| c.is_uppercase()).count();
    let keep = if run > 1 && cs.get(run).is_some_and(|c| c.is_lowercase()) { run - 1 } else { run };
    let mut out = String::with_capacity(w.len());
    for (i, c) in cs.iter().enumerate() {
        if i < keep {
            out.extend(c.to_lowercase());
        } else {
            out.push(*c);
        }
    }
    out
}

/// To lowerCamelCase, the way Swift's own guidelines do it: `Basic` → `basic`, `EUR` →
/// `eur`, `URLSession` → `urlSession`, `pay_rate` → `payRate`. A name with no capitals and
/// no underscores comes back unchanged, so an alias already written in Swift's spelling is
/// left alone.
///
/// Leading and trailing underscores survive, and that is not cosmetic: [`Gen::temp`] makes
/// a generated temporary unique by appending one, so a `camel` that dropped it would hand
/// `raw_` and `raw` the same Swift identifier — the collision `temp` exists to prevent.
fn camel(s: &str) -> String {
    if let Some(rest) = s.strip_prefix(clash::MARK) {
        return format!("{}{}", clash::MARK, camel(rest));
    }
    let lead = s.len() - s.trim_start_matches('_').len();
    let end = s.trim_end_matches('_').len();
    if end <= lead {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    out.push_str(&s[..lead]);
    for (i, w) in s[lead..end].split('_').filter(|p| !p.is_empty()).enumerate() {
        if i == 0 {
            out.push_str(&lower_lead(w));
            continue;
        }
        let mut ch = w.chars();
        if let Some(c) = ch.next() {
            out.extend(c.to_uppercase());
            out.push_str(ch.as_str());
        }
    }
    out.push_str(&s[end..]);
    out
}

/// A declared name as Swift writes a value: lowerCamelCase, in backticks when that is one
/// of the language's own words.
fn sw_name(s: &str) -> String {
    let c = camel(s);
    if SWIFT_KEYWORDS.contains(&c.as_str()) {
        format!("`{c}`")
    } else {
        c
    }
}

/// The same name as an **argument label at a call site**, where the rule is the other way
/// around: Swift takes a keyword there as it stands, and warns that the backticks were not
/// needed. `inout` is the one that still has to be escaped, because it is also a parameter
/// modifier and the parser cannot tell the two apart.
fn sw_label(s: &str) -> String {
    let c = camel(s);
    if c == "inout" {
        format!("`{c}`")
    } else {
        c
    }
}

/// The helper for one rounding mode, named as the generated module spells it.
fn sw_round_fn(m: RoundMode) -> String {
    format!("_round{}", pascal(mode_fn(m)))
}

/// Turn the shared expression text into Swift. The same post-processing shape as `go_expr`,
/// `ts_expr` and `rs_expr` — and the `//` substitution is load-bearing twice over here,
/// since in Swift it would otherwise start a comment and swallow the rest of the line.
///
/// `_min` and `_max` stay our own rather than becoming the standard library's `min` and
/// `max`. A rule may declare an input named `min` — `クーポン割引.rule` does — and a
/// parameter shadows a global in Swift, so the call would resolve to the money the caller
/// passed in.
/// The Swift twin of [`go_typed`]: a bare literal would be an `Int`, not an `Int64`.
fn sw_typed(s: &str) -> String {
    if s.parse::<i128>().is_ok() { format!("Int64({s})") } else { s.to_string() }
}

fn sw_expr(s: &str) -> String {
    arith::sw_expr(&truths(s, "true", "false"))
        .replace(" // ", " / ")
        .replace("_round_down(", "_roundDown(")
        .replace("_round_up(", "_roundUp(")
        .replace("_round_half_down(", "_roundHalfDown(")
        .replace("_round_half(", "_roundHalf(")
        .replace("_round_bankers(", "_roundBankers(")
}

/// The error type and the five modes of §7.3. `private` at file scope in Swift is what
/// `fileprivate` is, so the same text serves the generated module — where nothing outside
/// may call these — and `_round_test.swift`, where the cases sit in the same file.
fn round_sw() -> String {
    format!(
        r#"
/// {err}
public enum RuleError: Error, CustomStringConvertible {{
    /// {input}
    case input(what: String, value: Int64?)
    /// {contra}
    case contradiction(what: String)

    public var description: String {{
        switch self {{
        case .input(let what, .some(let v)): return "\(what): \(v)"
        case .input(let what, .none), .contradiction(let what): return what
        }}
    }}
}}

private func _min(_ a: Int64, _ b: Int64) -> Int64 {{
    a < b ? a : b
}}

private func _max(_ a: Int64, _ b: Int64) -> Int64 {{
    a > b ? a : b
}}

/// {down}
private func _roundDown(_ x: Int64, _ g: Int64) -> Int64 {{
    let v = abs(x) / g * g
    return x < 0 ? -v : v
}}

/// {up}
private func _roundUp(_ x: Int64, _ g: Int64) -> Int64 {{
    let a = abs(x)
    let v = a % g == 0 ? a / g * g : (a / g + 1) * g
    return x < 0 ? -v : v
}}

/// {half}
private func _roundHalf(_ x: Int64, _ g: Int64) -> Int64 {{
    let a = abs(x)
    let v = 2 * (a % g) >= g ? (a / g + 1) * g : a / g * g
    return x < 0 ? -v : v
}}

/// {half_down}
private func _roundHalfDown(_ x: Int64, _ g: Int64) -> Int64 {{
    let a = abs(x)
    let v = 2 * (a % g) > g ? (a / g + 1) * g : a / g * g
    return x < 0 ? -v : v
}}

/// {bankers}
private func _roundBankers(_ x: Int64, _ g: Int64) -> Int64 {{
    let a = abs(x)
    var q = a / g
    let r = a % g
    if 2 * r > g || (2 * r == g && q % 2 == 1) {{
        q += 1
    }}
    let v = q * g
    return x < 0 ? -v : v
}}
"#,
        err = tr!("この規則が返しうる誤り。", "Everything this rule can go wrong with."),
        input = tr!("宣言した範囲の外。呼び出し側の契約違反。", "Outside the declared input domain: a contract violation by the caller."),
        contra = tr!("規則そのものの矛盾。呼び出し側の誤りではない。", "A contradiction in the rule itself, not a mistake by the caller."),
        down = tr!("0 へ寄せる。-4.8円 → -4円。", "Toward zero: -4.8 yen -> -4 yen."),
        up = tr!("0 から遠ざける。-4.2円 → -5円。", "Away from zero: -4.2 yen -> -5 yen."),
        half = tr!("半分ちょうどは 0 から遠ざける。", "An exact half goes away from zero."),
        half_down = tr!("半分ちょうどは 0 へ寄せる。", "An exact half goes toward zero."),
        bankers = tr!("半分ちょうどは偶数へ。", "An exact half goes to the even neighbor."),
    )
}

impl<'a> Gen<'a> {
    /// The Swift type for a value. A unit is a struct over `Int64`, which is what the
    /// overflow proof (E108) is stated in — `Int` is the platform's word and only
    /// happens to be 64 bits.
    fn sw_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => self.enum_names.get(n).cloned().unwrap_or_else(|| "String".into()),
            Ty::Bool => "Bool".into(),
            Ty::Str => "String".into(),
            Ty::Number | Ty::Date => "Int64".into(),
            Ty::Opt(t) => format!("{}?", self.sw_ty(t)),
            _ => brand_of(ty),
        }
    }

    /// The identifier a declared name gets, in Swift's spelling.
    fn sw_ident(&self, n: &str) -> String {
        sw_name(&self.ident(n))
    }

    /// A parameter of a Swift function that takes the declared name `n`, of the type `ty`: its
    /// label is the name, and where the value is bound apart from it inside the functions
    /// (§15.336), the parameter takes it under that identifier after the label.
    fn sw_param(&self, n: &Name, ty: &str) -> String {
        let (label, held) = (sw_name(&pub_name(n)), sw_name(&self.held(n)));
        if label == held {
            format!("{label}: {ty}")
        } else {
            format!("{label} {held}: {ty}")
        }
    }

    /// The name of the set a group is declared as. The underscore goes on *before* the
    /// keyword escaping: `_` + `` `internal` `` is not an identifier, and `_internal` needs
    /// no backticks in the first place.
    fn sw_group(&self, n: &str) -> String {
        sw_name(&format!("_{}", self.ident(n)))
    }

    /// A blank assignment for a value nothing downstream reads, the same device the Go
    /// backend needs. The value is still computed, so the generated code and the rule stay
    /// line for line, but Swift warns about a binding that is never read — and the module
    /// says DO NOT EDIT, so nobody can quiet the warning afterwards. W111 has already named
    /// the declaration.
    fn sw_unread(&self, name: &str) -> String {
        if self.is_read(name) {
            String::new()
        } else {
            format!("    _ = {}\n", self.sw_ident(name))
        }
    }

    fn sw_value(&self, v: &str, ty: &Ty) -> String {
        match self.value_name(v, ty) {
            Some((ty, alias)) => format!("{ty}.{}", sw_name(alias)),
            None => format!("{v:?}"),
        }
    }

    /// One enum as Swift: its cases in the order declared, and the order and the display names
    /// as computed properties when it has them (§15.210, §15.211).
    pub(crate) fn sw_enum(&self, jp: &str, ascii: &str) -> String {
        let Some(vals) = self.c.enums.get(jp) else { return String::new() };
        let mut o = format!("public enum {ascii}: String, CaseIterable, Sendable {{\n");
        let names: Vec<String> = vals
            .iter()
            .map(|v| self.value_names.get(&(jp.to_string(), v.clone())).map(|(_, a)| sw_name(a)).unwrap_or_else(|| sw_name(v)))
            .collect();
        for (v, n) in vals.iter().zip(&names) {
            o.push_str(&format!("    case {n} = {v:?}\n"));
        }
        let property = |o: &mut String, doc: String, name: &str, ty: &str, ret: &dyn Fn(usize, &String) -> String| {
            o.push_str(&format!("\n    /// {doc}\n    public var {name}: {ty} {{\n        switch self {{\n"));
            for (i, (v, n)) in vals.iter().zip(&names).enumerate() {
                o.push_str(&format!("        case .{n}: return {}\n", ret(i, v)));
            }
            o.push_str("        }\n    }\n");
        };
        if self.c.ordered.contains(jp) {
            property(&mut o, tr!("宣言の順序の何番目か（0 から）。", "The place in the order declared, counting from 0."), "rank", "Int", &|i, _| i.to_string());
        }
        if let Some(labels) = self.c.labels.get(jp) {
            property(
                &mut o,
                tr!("表示名。表示名の無い値は値の名前。", "The display name: a value without one is shown by its name."),
                "label",
                "String",
                &|i, v| str_lit(&labels.get(i).cloned().flatten().unwrap_or_else(|| v.clone())),
            );
        }
        o.push_str("}\n\n");
        o
    }

    /// The `extension` for an enum an `enums` file the rule reads declares as the same enum
    /// (§15.355): the file's module declares the type, and this one adds each member the rule's
    /// own declaration had and the file's does not — `rank` for an ordered enum, `label` for a
    /// labelled one — so the type is written once and the names the rule's code reads stay.
    /// Written to nothing where the file's declaration holds every member it needs.
    fn sw_shared_ext(&self, jp: &str, ascii: &str, d: &crate::ast::EnumDecl) -> String {
        let rank = self.c.ordered.contains(jp) && !d.ordered;
        let labels = self.c.labels.get(jp).filter(|_| !d.has_labels());
        if !rank && labels.is_none() {
            return String::new();
        }
        let Some(vals) = self.c.enums.get(jp) else { return String::new() };
        let names: Vec<String> = vals
            .iter()
            .map(|v| self.value_names.get(&(jp.to_string(), v.clone())).map(|(_, a)| sw_name(a)).unwrap_or_else(|| sw_name(v)))
            .collect();
        let property = |o: &mut String, doc: String, name: &str, ty: &str, ret: &dyn Fn(usize, &String) -> String| {
            o.push_str(&format!("\n    /// {doc}\n    public var {name}: {ty} {{\n        switch self {{\n"));
            for (i, (v, n)) in vals.iter().zip(&names).enumerate() {
                o.push_str(&format!("        case .{n}: return {}\n", ret(i, v)));
            }
            o.push_str("        }\n    }\n");
        };
        let mut o = format!("/// {}\nextension {ascii} {{\n", tr!("`enums` ファイルが宣言する同じ列挙に足す、この規則の宣言が持っていた要素。", "The members the rule's own declaration had, added to the same enum the `enums` file declares."));
        if rank {
            property(&mut o, tr!("宣言の順序の何番目か（0 から）。", "The place in the order declared, counting from 0."), "rank", "Int", &|i, _| i.to_string());
        }
        if let Some(labels) = labels {
            property(
                &mut o,
                tr!("表示名。表示名の無い値は値の名前。", "The display name: a value without one is shown by its name."),
                "label",
                "String",
                &|i, v| str_lit(&labels.get(i).cloned().flatten().unwrap_or_else(|| v.clone())),
            );
        }
        o.push_str("}\n\n");
        o
    }

    /// The helpers the functions call for the expressions over an ordered enum (§15.210).
    fn sw_enum_helpers(&self) -> String {
        let mut o = String::new();
        for en in self.c.enum_order.iter().filter(|e| self.c.ordered.contains(*e)) {
            let (rank, max, min) = self.enum_helpers(en);
            let t = self.enum_type(en);
            if rank {
                o.push_str(&format!("private func _rank_{t}(_ x: {t}) -> Int {{\n    x.rank\n}}\n\n"));
            }
            if max {
                o.push_str(&format!("private func _max_{t}(_ a: {t}, _ b: {t}) -> {t} {{\n    a.rank >= b.rank ? a : b\n}}\n\n"));
            }
            if min {
                o.push_str(&format!("private func _min_{t}(_ a: {t}, _ b: {t}) -> {t} {{\n    a.rank <= b.rank ? a : b\n}}\n\n"));
            }
        }
        o
    }

    /// Render a cell as a Swift condition. A don't-care yields None (no condition).
    fn sw_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        // An optional value is an `Optional`, which has no `<` and is no `Bool`: the module did
        // not build. A comparison is made on the value inside, and is false of `nil` (§15.201).
        let optional = matches!(ty, Ty::Opt(_));
        // A branded value is a struct around its integer: what a cell compares is the integer.
        let branded = optional && matches!(inner, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate);
        let inside = if branded { format!("{var}?.value") } else { var.to_string() };
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                Lit::Word(w) => self.sw_value(w, ty),
                Lit::Num(n) => self.int_lit(n, inner, col_scale),
                Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => format!("{s:?}"),
            }
        };
        let members = |ls: &Vec<Lit>| -> String {
            let mut out: Vec<String> = Vec::new();
            for l in ls {
                if let Lit::Word(w) = l {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| self.sw_value(m, ty)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            format!("[{}]", out.join(", "))
        };
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => format!("{var} == nil"),
            Cell::Prefix(ps) => ps
                .iter()
                .map(|p| format!("{var}.hasPrefix({})", format!("{p:?}")))
                .collect::<Vec<_>>()
                .join(" || "),
            // A cell naming one group uses the set the module already declares, rather than
            // writing the members out a second time.
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                format!("{}.contains({var})", self.sw_group(w))
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE && optional => format!("{var} == true"),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE && optional => format!("{var} == false"),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("!{var}"),
            Cell::Lit(l) => format!("{inside} == {}", lit(l)),
            Cell::Set(ls) => format!("{}.contains({inside})", members(ls)),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                format!("!{}.contains({var})", self.sw_group(w))
            }
            Cell::Not(ls) => format!("!{}.contains({inside})", members(ls)),
            Cell::Cmp(cs) => {
                let on = if branded { "$0.value" } else if optional { "$0" } else { var };
                let c = cs
                    .iter()
                    .map(|(o, l)| {
                        let op = match o {
                            CmpOp::Le => "<=",
                            CmpOp::Ge => ">=",
                            CmpOp::Lt => "<",
                            CmpOp::Gt => ">",
                        };
                        format!("{on} {op} {}", lit(l))
                    })
                    .collect::<Vec<_>>()
                    .join(" && ");
                if optional { format!("({var}.map {{ {c} }} ?? false)") } else { c }
            }
            // A comparison on an ordered enum, by the places in its order (§15.210).
            Cell::Ranked { ops, .. } => {
                let on = if optional { "$0" } else { var };
                let c = ops
                    .iter()
                    .map(|(o, v)| format!("{on}.rank {} {}.rank", o.word(), self.sw_value(v, ty)))
                    .collect::<Vec<_>>()
                    .join(" && ");
                if optional { format!("({var}.map {{ {c} }} ?? false)") } else { c }
            }
        })
    }

    pub fn swift(&self) -> String {
        self.apart_text(clash::Syntax::Swift, |g| g.sw_module(g.sw_alone()))
    }

    /// The rule's Swift. `alone`: the module declares the enums it reads from `enums` files as
    /// well, and is built without their modules (§15.326).
    fn sw_module(&self, alone: bool) -> String {
        let mut o = self.header("//");
        o.push('\n');

        // Brands. A struct with one stored property is laid out as that property, so the
        // type costs nothing at run time — the same bargain the Rust backend makes.
        let mut brands: BTreeMap<String, String> = BTreeMap::new();
        for v in self.branded().into_iter().chain(self.branded_columns()) {
            // An optional amount is the same brand when it is there: the runner builds it, and
            // a rule whose only amount is optional declared none (§15.201), nor one whose only
            // amount is a column a table writes that may be absent (§15.343).
            let ty = self.ty_of(v).present().clone();
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                brands.insert(brand_of(&ty), format!("{ty}"));
            }
        }
        for (b, doc) in &brands {
            o.push_str(&format!(
                "/// {doc}\n\
                 public struct {b}: Hashable, Comparable, Sendable {{\n    \
                     public var value: Int64\n\n    \
                     public init(_ value: Int64) {{ self.value = value }}\n\n    \
                     public static func < (lhs: {b}, rhs: {b}) -> Bool {{ lhs.value < rhs.value }}\n\
                 }}\n\n"
            ));
        }
        o.push_str(&format!(
            "/// {}\npublic struct Fired: Hashable, Sendable {{\n    public let table: String\n    public let row: Int\n    public let label: String\n\n    \
             public init(table: String, row: Int, label: String = \"\") {{\n        self.table = table\n        self.row = row\n        self.label = label\n    }}\n}}\n\n",
            fired_doc()
        ));

        // Enums. The raw value is the source name, which is also what the wire carries
        // (§10.2), so `rawValue` and `init(rawValue:)` are the whole conversion and no
        // hand-written parser is needed on either side. An enum of an `enums` file is that
        // file's, compiled into the same module (§15.212); where that module and this one would
        // declare one name twice, this one declares the enums it reads itself (§15.326). One it
        // would declare that the file declares as the same enum, the file's module declares,
        // and this one adds what the rule's own declaration had in an `extension` (§15.355).
        let shared = if alone { BTreeMap::new() } else { self.sw_shared_enums() };
        for (jp, ascii) in self.own_enums() {
            match shared.get(&jp) {
                Some(d) => o.push_str(&self.sw_shared_ext(&jp, &ascii, d)),
                None => o.push_str(&self.sw_enum(&jp, &ascii)),
            }
        }
        if alone {
            for (_, types) in self.foreign_enums() {
                for (jp, ascii) in types {
                    o.push_str(&self.sw_enum(&jp, &ascii));
                }
            }
        }
        o.push_str(&self.sw_enum_helpers());
        // Which values need which before them (§15.222).
        o.push_str(&self.sw_prereqs());

        // The machine this function is one step of (§15.148). `final` is a keyword here.
        if let Some((en, init, fins)) = self.machine_consts() {
            let cls = self.enum_names.get(&en).cloned().unwrap_or_default();
            // The values carried along with the state, and where a case starts (§15.217).
            let mut more = String::new();
            for mc in self.more_carried() {
                let v = match mc.kind() {
                    MoreKind::Enum => self.sw_value(&mc.word(), &mc.ty),
                    MoreKind::Bool => mc.bool_word().to_string(),
                    MoreKind::Number => mc.int.to_string(),
                    MoreKind::Branded => format!("{}({})", self.sw_ty(&mc.ty), mc.int),
                };
                more.push_str(&format!(
                    "/// {}\npublic let {}: {} = {v}\n",
                    tr!("持ち越す {} の始まりの値。", "Where the carried {} starts.", mc.input),
                    mc.swift(),
                    self.sw_ty(&mc.ty)
                ));
            }
            o.push_str(&format!(
                "/// {}\npublic let initialState: {cls} = {}\n/// {}\npublic let finalStates: Set<{cls}> = [{}]\n{more}\n\
                 /// {}\npublic func isFinal(_ state: {cls}) -> Bool {{\n    finalStates.contains(state)\n}}\n\n",
                tr!("案件が始まる状態。", "The state a case starts in."),
                self.sw_value(&init, &Ty::Enum(en.clone())),
                tr!("案件が終わる状態。", "The states a case ends in."),
                fins.iter().map(|v| self.sw_value(v, &Ty::Enum(en.clone()))).collect::<Vec<_>>().join(", "),
                tr!("この状態で案件が終わっているか。", "Whether a case in this state has ended.")
            ));
        }

        o.push_str(round_sw().trim_start_matches('\n'));
        o.push_str(&arith::sw_helpers(&arith::Uses::of(self.f)));
        o.push('\n');

        // Groups. A `Set` needs Hashable, which a raw-value enum already is.
        for g in &self.f.groups {
            let ty = self
                .c
                .groups
                .get(&g.name.text)
                .and_then(|(owner, _)| self.enum_names.get(owner).cloned())
                .unwrap_or_else(|| "Int64".into());
            let ms: Vec<String> = g.members.iter().map(|m| self.sw_value(&m.text, &self.group_ty(&g.name.text))).collect();
            o.push_str(&format!(
                "private let {}: Set<{ty}> = [{}]\n",
                self.sw_group(&g.name.text),
                ms.join(", ")
            ));
        }
        if !self.f.groups.is_empty() {
            o.push('\n');
        }

        o.push_str(&self.sw_fn());
        o.push('\n');
        o.push_str(&self.sw_record());
        // The function that counts the answers (§15.225).
        if self.f.has_random() {
            o.push('\n');
            o.push_str(&self.sw_chances());
        }
        o
    }

    /// The rule's items as Swift, at the base indentation (§15.56).
    fn sw_items(&self, local: &dyn Fn(&str) -> String, trace: &str, phase: Phase) -> String {
        let mut o = String::new();
        for (i, it) in self.f.items.iter().enumerate() {
            if !self.in_phase(i, it, phase) {
                continue;
            }
            match it {
                Item::Derived(d) => {
                    o.push_str(&format!(
                        "    let {} = {}  // {}\n",
                        self.sw_ident(&d.name.text),
                        sw_typed(&sw_expr(unparen(&self.expr(&d.expr, local).text))),
                        tr!("導出", "derived value")
                    ));
                    o.push_str(&self.sw_unread(&d.name.text));
                }
                Item::Define(d) => {
                    o.push_str(&format!(
                        "    let {} = {}  // {}\n",
                        self.sw_ident(&d.name.text),
                        sw_typed(&sw_expr(unparen(&self.expr(&d.expr, local).text))),
                        tr!("定義", "definition")
                    ));
                    // The range the rule accepts for it, held where it is worked out (§15.313).
                    let n = self.sw_ident(&d.name.text);
                    if let Some(test) = self.accepted_ends(d).and_then(|ends| or_out_of(&n, ends, "")) {
                        o.push_str(&format!(
                            "    if {test} {{\n        throw RuleError.input(what: {}, value: Int64({n}))\n    }}\n",
                            sw_said(&crate::door::out_of_range(&self.said_name(&d.name.text)), &self.here(), "")
                        ));
                    }
                    o.push_str(&self.sw_unread(&d.name.text));
                }
                Item::Agg(_) | Item::Fold(_) => {}
                Item::Table(t) => {
                    if let Some(t) = self.c.table_at(t) {
                        o.push_str(&self.sw_table(t, local, trace));
                    }
                }
            }
        }
        o
    }

    /// Every walk of a rule that walks sequences, in Swift (§15.56, §15.58, §15.219). The shape is
    /// the one `py_walks` writes.
    fn sw_walks(&self, outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = String::new();
        for (s, el) in self.f.elements.iter().enumerate() {
            if let Some(cap) = self.count_cap_of(s) {
                let seq = sw_name(&self.held(&el.name));
                o.push_str(&format!(
                    "    if {seq}.count > {cap} {{\n        throw RuleError.input(what: \"{}\", value: Int64({seq}.count))\n    }}\n",
                    crate::door::too_many(&el.name.text, cap)
                ));
            }
        }
        for step in self.walk_steps() {
            match step {
                // What a walk does not depend on is worked out once, before it (§15.208).
                WalkStep::Once(ix) => o.push_str(&self.sw_items(outer, trace, Phase::Only(&ix))),
                WalkStep::Walk { seq, body, sums, .. } => o.push_str(&self.sw_walk(seq, &body, &sums, outer, trace)),
            }
        }
        o
    }

    /// One walk over sequence `s`, in Swift (§15.219).
    fn sw_walk(&self, s: usize, body_items: &[usize], sums: &[usize], outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let el = &self.f.elements[s];
        let seq = sw_name(&self.held(&el.name));
        let fields: BTreeMap<String, String> = el.fields.iter().map(|fd| (fd.name.text.clone(), sw_name(&pub_name(&fd.name)))).collect();
        let e = sw_name(&self.elem_var(s));
        // Inside the walk a field is read off the element.
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => {
                    let v = format!("{e}.{f}");
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                        format!("{v}.value")
                    } else {
                        v
                    }
                }
                None => outer(n),
            }
        };
        let mut o = String::new();
        let index = self.index_var(s);
        self.walk_begin(s, &index);
        let mut body = self.sw_element_guards(s, &local);
        let (seen, step) = self.sw_unique(s, sums, &local);
        o.push_str(&seen);
        body.push_str(&step);
        // Past a `stop`, the door alone (§15.328).
        if let Some(i) = self.walk_goes_past_stop(sums) {
            body.push_str(&format!("    if {} {{\n        continue\n    }}\n", sw_name(&self.fold_names(i).done)));
        }
        body.push_str(&self.sw_items(&local, trace, Phase::Only(body_items)));
        let mut after = String::new();
        for &i in sums {
            match &self.f.items[i] {
                Item::Agg(d) if d.kind.extreme() => {
                    let (init, step) = self.sw_extreme(d, &local);
                    o.push_str(&init);
                    body.push_str(&step);
                }
                Item::Agg(d) => {
                    o.push_str(&format!("    var {}: Int64 = 0  // {}\n", self.sw_ident(&d.name.text), self.agg_word(d)));
                    let v = local(&d.column.text);
                    if d.kind == crate::ast::AggKind::Sum {
                        body.push_str(&format!("    {} += {v}  // {}\n", self.sw_ident(&d.name.text), d.name.text));
                    } else {
                        let test = self.sw_test(&v, &d.column.text, &d.value);
                        body.push_str(&format!("    if {test} {{  // {}\n        {} += 1\n    }}\n", d.name.text, self.sw_ident(&d.name.text)));
                    }
                    if let Some((_, cap)) = self.sum_caps().into_iter().find(|(x, _)| x.name.text == d.name.text) {
                        let n = self.sw_ident(&d.name.text);
                        after.push_str(&format!(
                            "    if {n} > {cap} {{\n        throw RuleError.input(what: \"{}\", value: Int64({n}))\n    }}\n",
                            crate::door::out_of_range(&d.name.text)
                        ));
                    }
                    // The bottom of its range, where a walk can come in under it (§15.314).
                    if let Some(floor) = self.walk_floor(d) {
                        let n = self.sw_ident(&d.name.text);
                        after.push_str(&format!(
                            "    if {n} < {floor} {{\n        throw RuleError.input(what: \"{}\", value: Int64({n}))\n    }}\n",
                            crate::door::out_of_range(&d.name.text)
                        ));
                    }
                }
                Item::Fold(fold) => {
                    let (init, step, fin) = self.sw_fold(i, fold, &seq, &local);
                    o.push_str(&init);
                    body.push_str(&step);
                    after.push_str(&fin);
                }
                _ => {}
            }
        }
        // The index of the element is bound where a sentence of the walk says it (§15.324). An
        // element no line of the walk reads is said to be left unread on purpose: swiftc warns of
        // a value never used (§15.340).
        if !clash::reads_word(&body, &e, clash::Syntax::Swift) {
            body.insert_str(0, &format!("    _ = {e}\n"));
        }
        if self.walk_end() {
            o.push_str(&format!("    for ({index}, {e}) in {seq}.enumerated() {{\n"));
        } else {
            o.push_str(&format!("    for {e} in {seq} {{\n"));
        }
        o.push_str(&Self::indent_block(&body, "    "));
        o.push_str("    }\n");
        o.push_str(&after);
        o
    }

    /// A test on a column of one element, in Swift: `count`'s, and the `where` of `max` and `min`.
    fn sw_test(&self, v: &str, col: &str, value: &Option<crate::ast::Name>) -> String {
        let ty = self.ty_of(col);
        match (&ty, value) {
            (Ty::Bool, Some(w)) if w.text == crate::kw::FALSE => format!("!{v}"),
            (Ty::Bool, _) => v.to_string(),
            (_, Some(w)) => format!("{v} == {}", self.sw_value(&w.text, &ty)),
            (_, None) => v.to_string(),
        }
    }

    /// A `max` or a `min`, in Swift (§15.219).
    fn sw_extreme(&self, d: &crate::ast::AggDecl, local: &dyn Fn(&str) -> String) -> (String, String) {
        let n = self.sw_ident(&d.name.text);
        let set = sw_name(&self.temp(&format!("{}Set", self.ident(&d.name.text))));
        let v = local(&d.column.text);
        let ty = self.ty_of(&d.column.text);
        let op = if d.kind == crate::ast::AggKind::Max { ">" } else { "<" };
        let (sw_t, start, better) = match self.extreme_enum(d) {
            Some(en) => {
                let t = self.enum_type(&en);
                let w = self.extreme_empty_word(d).unwrap_or_default();
                (self.sw_ty(&ty), self.sw_value(&w, &ty), format!("_rank_{t}({v}) {op} _rank_{t}({n})"))
            }
            None => ("Int64".to_string(), self.extreme_empty_int(d).unwrap_or_else(|| "0".into()), format!("{v} {op} {n}")),
        };
        let init = format!("    var {n}: {sw_t} = {start}  // {}\n    var {set} = false\n", self.agg_word(d));
        let mut step = String::new();
        let pad = match &d.filter {
            Some((w, want)) => {
                step.push_str(&format!("    if {} {{  // {}\n", self.sw_test(&local(&w.text), &w.text, want), d.name.text));
                "    "
            }
            None => "",
        };
        step.push_str(&format!("{pad}    if !{set} || {better} {{\n{pad}        {n} = {v}\n{pad}        {set} = true\n{pad}    }}\n"));
        if !pad.is_empty() {
            step.push_str("    }\n");
        }
        (init, step)
    }

    /// A fold, in Swift (§15.56).
    fn sw_fold(&self, i: usize, fold: &crate::ast::FoldDecl, seq: &str, local: &dyn Fn(&str) -> String) -> (String, String, String) {
        let names = self.fold_names(i);
        let (answer, stopped, done) = (sw_name(&names.answer), sw_name(&names.stopped), sw_name(&names.done));
        let (taken, kept, best) = (sw_name(&names.taken), sw_name(&names.kept), sw_name(&names.best));
        let (held, key) = (sw_name(&names.held), sw_name(&names.key));
        let taken_at = sw_name(&names.taken_at_camel);
        let alone = self.fold_alone(i);
        // Ends the loop at a `stop`: a fold alone in its walk with no door to keep past it (§15.328).
        let ends = alone && !self.stop_keeps_door(i);
        let with_held = |n: &str| -> String { if n == crate::kw::HELD { held.clone() } else { local(n) } };
        let (out_name, out_ty) = self.fold_target(fold);
        let acc_ty = if out_ty.is_numeric() { "Int64".to_string() } else { self.sw_ty(&out_ty) };
        // An answer is read in the type the fold answers with: a value of its enum is that
        // enum's member (§15.208).
        let val = |x: &Expr, l: &dyn Fn(&str) -> String| -> String {
            match x {
                Expr::Lit(Lit::Word(w), _) if w != crate::kw::TRUE && w != crate::kw::FALSE => self.sw_value(w, &out_ty),
                _ => sw_expr(unparen(&self.expr(x, l).text)),
            }
        };
        let text = |x: &Option<Expr>| -> String { x.as_ref().map(|x| val(x, &with_held)).unwrap_or_else(|| "0".to_string()) };
        let takes = fold.arms.iter().any(|(_, a, _)| matches!(a, Arm::Take { .. } | Arm::TakeWeighted { .. }));
        let keeps = fold.arms.iter().any(|(_, a, _)| matches!(a, Arm::KeepMax { .. }));
        let weighted = fold.weighted();
        let draw = fold.draw.as_ref().map(|d| local(&d.text)).unwrap_or_default();
        let (total, weight, cuts) = (sw_name(&names.total), sw_name(&names.weight), sw_name(&names.cuts));

        // A flag no arm of the fold sets is a `let`: swiftc warns of a `var` never changed (§15.340).
        let set = |stop: fn(&Arm) -> bool| if fold.arms.iter().any(|(_, a, _)| stop(a)) { "var" } else { "let" };
        let mut init = String::new();
        init.push_str(&format!("    var {answer}: {acc_ty} = {}  // {}\n", text(&fold.empty), tr!("要素ゼロ件の答え", "the answer for no elements")));
        init.push_str(&format!("    {} {stopped} = false\n", set(|a| matches!(a, Arm::Stop(Some(_))))));
        if !ends {
            init.push_str(&format!("    {} {done} = false\n", set(|a| matches!(a, Arm::Stop(_)))));
        }
        if takes {
            init.push_str(&format!("    var {taken}: {acc_ty}? = nil\n"));
        }
        // The element a `take_unique` took, which the sentence says beside the one that matched
        // again (§15.324).
        if fold.arms.iter().any(|(_, a, _)| matches!(a, Arm::Take { unique: true, .. })) {
            init.push_str(&format!("    var {taken_at} = 0\n"));
        }
        if weighted {
            init.push_str(&format!("    var {total}: Int64 = 0  // {}\n", tr!("重みの合計", "the running total of the weights")));
            init.push_str(&format!("    var {cuts}: [Int64] = []\n"));
        }
        if keeps {
            init.push_str(&format!("    var {kept}: {acc_ty}? = nil\n    var {best}: Int64? = nil\n"));
        }

        // The verdict the table wrote for this element, and what the walk does about it.
        let mut arms = String::new();
        let v = local(&fold.verdict);
        for (name, arm, _) in &fold.arms {
            arms.push_str(&format!("    if {v} == {} {{  // {}\n", self.sw_value(&name.text, &self.ty_of(&fold.verdict)), name.text));
            match arm {
                Arm::Next => arms.push_str(&format!("        // {}\n", crate::kw::NEXT)),
                Arm::Stop(None) if ends => arms.push_str("        break\n"),
                Arm::Stop(None) => arms.push_str(&format!("        {done} = true\n")),
                Arm::Stop(Some(x)) => {
                    arms.push_str(&format!("        {answer} = {}\n", val(x, local)));
                    if ends {
                        arms.push_str(&format!("        {stopped} = true\n        break\n"));
                    } else {
                        arms.push_str(&format!("        {stopped} = true\n        {done} = true\n"));
                    }
                }
                Arm::Take { expr, unique } => {
                    if *unique {
                        let here = self.here_used();
                        arms.push_str(&format!("        if {taken} != nil {{\n"));
                        arms.push_str(&format!(
                            "            throw RuleError.contradiction(what: {})\n        }}\n",
                            sw_said(&crate::door::fold_contradiction(&fold.verdict, &fold.over, crate::door::BEFORE, crate::door::HERE), &here, &taken_at)
                        ));
                        arms.push_str(&format!("        {taken} = {}\n        {taken_at} = {here}\n", val(expr, local)));
                    } else {
                        arms.push_str(&format!("        if {taken} == nil {{\n"));
                        arms.push_str(&format!("            {taken} = {}\n        }}\n", val(expr, local)));
                    }
                }
                Arm::KeepMax { expr, key: k } => {
                    arms.push_str(&format!("        let {key} = {}\n", sw_expr(unparen(&self.expr(k, local).text))));
                    arms.push_str(&format!("        if {best} == nil || {key} > {best}! {{\n"));
                    arms.push_str(&format!("            {best} = {key}\n"));
                    arms.push_str(&format!("            {kept} = {}\n        }}\n", val(expr, local)));
                }
                // The weight goes on the running total, and the element is taken where the drawn
                // value is below the total for the first time (§15.226). A weight that would take
                // the total past int64 is refused before it goes on, and before another element is
                // read.
                Arm::TakeWeighted { expr, weight: wx } => {
                    arms.push_str(&format!("        let {weight}: Int64 = {}\n", sw_expr(unparen(&self.expr(wx, local).text))));
                    arms.push_str(&format!(
                        "        if {weight} > Int64.max - {total} {{\n            throw RuleError.input(what: \"{}\", value: nil)\n        }}\n",
                        crate::door::weights_past(&fold.over)
                    ));
                    arms.push_str(&format!("        {total} += {weight}\n        {cuts}.append({total})\n"));
                    arms.push_str(&format!("        if {taken} == nil && {draw} < {total} {{\n"));
                    arms.push_str(&format!("            {taken} = {}\n        }}\n", val(expr, local)));
                }
            }
            arms.push_str("    }\n");
        }
        let step = if alone { arms } else { format!("    if !{done} {{  // {}\n{}    }}\n", fold.verdict, Self::indent_block(&arms, "    ")) };

        // The drawn value of a pick by weight is below the total of the weights, unless they add up
        // to 0 (§15.226).
        let mut fin = String::new();
        if weighted {
            let what = fold.draw.as_ref().map(|d| crate::door::draw_past(&d.text)).unwrap_or_default();
            fin.push_str(&format!("    if {total} > 0 && {draw} >= {total} {{\n        throw RuleError.input(what: \"{what}\", value: Int64({draw}))\n    }}\n"));
        }
        // A sequence with nothing in it answers `empty` and never reaches `exhausted`; a walk
        // that ended on `stop with` has its answer already.
        fin.push_str(&format!("    if !{seq}.isEmpty && !{stopped} {{\n"));
        let mut fallback = text(&fold.empty);
        if keeps {
            fallback = format!("({kept} ?? {fallback})");
        }
        if takes {
            fallback = format!("({taken} ?? {fallback})");
        }
        fin.push_str(&format!("        let {held}: {acc_ty} = {fallback}\n"));
        let ex = text(&fold.exhausted);
        if !ex.contains(held.as_str()) {
            fin.push_str(&format!("        _ = {held}\n"));
        }
        fin.push_str(&format!("        {answer} = {ex}\n    }}\n"));
        fin.push_str(&format!("    let {} = {answer}\n", self.sw_ident(&out_name)));
        fin.push_str(&self.sw_unread(&out_name));
        (init, step, fin)
    }

    /// The fields marked `unique` (§15.290), in Swift: a set for each before the first walk over the
    /// sequence, and in the walk, after the guards of one element, the refusal of a value an element
    /// before it had.
    fn sw_unique(&self, s: usize, sums: &[usize], local: &dyn Fn(&str) -> String) -> (String, String) {
        let seq = &self.f.elements[s].name.text;
        let (mut init, mut step) = (String::new(), String::new());
        for fd in self.unique_fields(s, sums) {
            let seen = sw_name(&self.temp(&format!("{}Seen", self.ident(&fd.name.text))));
            let v = local(&fd.name.text);
            let t = match self.ty_of(&fd.name.text) {
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => "Int64".to_string(),
                other => self.sw_ty(&other),
            };
            // Where each value was first seen, which the sentence says beside the element it came
            // round again on (§15.324).
            let here = self.here_used();
            let first = sw_name(&self.temp("first"));
            init.push_str(&format!("    var {seen}: [{t}: Int] = [:]\n"));
            step.push_str(&format!(
                "    if let {first} = {seen}[{v}] {{\n        throw RuleError.input(what: {}, value: nil)\n    }}\n    {seen}[{v}] = {here}\n",
                sw_said(&crate::door::duplicate(seq, &fd.name.text, crate::door::BEFORE, crate::door::HERE), &here, &first)
            ));
        }
        (init, step)
    }

    fn sw_element_guards(&self, s: usize, local: &dyn Fn(&str) -> String) -> String {
        let mut o = String::new();
        let here = self.here();
        for i in self.element_fields(s) {
            let ty = self.ty_of(&i.name.text);
            // An optional input is guarded as the type it wraps whenever it has a value (§15.201).
            let optional = matches!(ty, Ty::Opt(_));
            let ty = ty.present().clone();
            if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                continue;
            }
            let v = local(&i.name.text);
            // A branded value is a struct around its integer, which is what the guard compares.
            let inside = if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) { "__v.value" } else { "__v" };
            let (v, wrap) = if optional {
                (inside.to_string(), Some((format!("    if let __v = {v} {{\n"), "    }\n".to_string())))
            } else {
                (v, None)
            };
            // Each end the range writes, and −(2^63−1) where it writes no lower one: an Int64
            // holds one value below that, and the door refuses it (§15.295).
            let Some(test) = or_out_of(&v, crate::door::ends(self.c, &i.name.text, false), "") else {
                continue;
            };
            let mut o = Guarded::new(&mut o, wrap, "    ");
            // The sentence names the field by its place on the element (§15.324).
            o.push_str(&format!(
                "    if {test} {{\n        throw RuleError.input(what: {}, value: Int64({v}))\n    }}\n",
                sw_said(&crate::door::out_of_range(&self.said_name(&i.name.text)), &here, "")
            ));
        }
        o
    }

    fn sw_fn(&self) -> String {
        let fname = sw_name(&pub_name(&self.f.name));
        let outs = &self.f.outputs;
        let mut o = String::new();

        // One element is a row of inputs, so it is a struct like an `Output`, and needs its
        // initializer declared for the same reason.
        for (s, el) in self.f.elements.iter().enumerate() {
            let fields: Vec<(String, String)> = el
                .fields
                .iter()
                .map(|fd| (sw_name(&pub_name(&fd.name)), self.sw_ty(&self.ty_of(&fd.name.text))))
                .collect();
            o.push_str(&format!("/// {}\npublic struct {}: Hashable, Sendable {{\n", tr!("{} の一件", "one of {}", el.name.text), self.element_type(s)));
            for (n, t) in &fields {
                o.push_str(&format!("    public var {n}: {t}\n"));
            }
            o.push_str(&format!(
                "\n    public init({}) {{\n",
                fields.iter().map(|(n, t)| format!("{n}: {t}")).collect::<Vec<_>>().join(", ")
            ));
            for (n, _) in &fields {
                o.push_str(&format!("        self.{n} = {n}\n"));
            }
            o.push_str("    }\n}\n\n");
        }

        // Multiple outputs come back as a struct. The memberwise initializer Swift writes
        // for a public struct is internal, so a caller in another module would not be able
        // to build one; this one is declared.
        if outs.len() > 1 {
            let fields: Vec<(String, String)> = outs
                .iter()
                .map(|od| (sw_name(&pub_name(&od.name)), self.sw_ty(&self.ty_of(&od.name.text))))
                .collect();
            o.push_str("public struct Output: Hashable, Sendable {\n");
            for (n, t) in &fields {
                o.push_str(&format!("    public var {n}: {t}\n"));
            }
            o.push_str(&format!(
                "\n    public init({}) {{\n",
                fields.iter().map(|(n, t)| format!("{n}: {t}")).collect::<Vec<_>>().join(", ")
            ));
            for (n, _) in &fields {
                o.push_str(&format!("        self.{n} = {n}\n"));
            }
            o.push_str("    }\n}\n\n");
        }

        let ret = if outs.len() == 1 {
            self.sw_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| self.sw_param(&i.name, &self.sw_ty(&self.ty_of(&i.name.text))))
            .collect();

        // The public function keeps the plain signature; the branches live in its traced
        // twin, which also returns the rows that matched (§15.33).
        let mut args: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", sw_label(&pub_name(&i.name)), sw_name(&self.held(&i.name))))
            .collect();
        // A sequence a walk reads is a parameter like the rest (§15.56, §15.219).
        for (s, el) in self.f.elements.iter().enumerate() {
            params.push(self.sw_param(&el.name, &format!("[{}]", self.element_type(s))));
            args.push(format!("{}: {}", sw_label(&pub_name(&el.name)), sw_name(&self.held(&el.name))));
        }
        let traced = format!("{fname}Traced");
        o.push_str(&format!("/// {}\n", plain_doc(&self.f.name.text, &self.f.version, &format!("`{traced}`"))));
        o.push_str(&format!(
            "public func {fname}({}) throws -> {ret} {{\n    try {traced}({}).0\n}}\n\n",
            params.join(", "),
            args.join(", ")
        ));
        o.push_str(&format!("/// {}\n", traced_doc(&self.f.name.text, &self.f.version)));
        o.push_str(&format!(
            "public func {traced}({}) throws -> ({ret}, [Fired]) {{\n",
            params.join(", ")
        ));
        // A rule that picks by weight runs its walk in a function of its own, which gives back the
        // running totals of the weights too (§15.225).
        let walked = self.weighted_fold().map(|(i, _)| (self.walked_fn("swift"), sw_name(&self.fold_names(i).cuts)));
        if let Some((w, _)) = &walked {
            o.push_str(&format!("    let (out, trace, _) = try {w}({})\n    return (out, trace)\n}}\n\n", args.join(", ")));
            o.push_str(&format!("/// {}\nfunc {w}({}) throws -> ({ret}, [Fired], [Int64]) {{\n", walked_doc(), params.join(", ")));
        }

        // A branded input is an Int64 inside; unwrap it once, where it is read.
        let local = |n: &str| -> String {
            match self.f.inputs.iter().find(|i| i.name.text == n) {
                Some(i) => {
                    let v = sw_name(&self.held(&i.name));
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                        format!("{v}.value")
                    } else {
                        v
                    }
                }
                None => self.sw_ident(n),
            }
        };
        // Entry guards. An enum needs none: a value of an enum type is one of its cases by
        // construction, so the check Python, TypeScript, Ruby and Go make at run time is
        // already made by the compiler, exactly as in Rust.
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            // An optional input is guarded as the type it wraps whenever it has a value (§15.201).
            let optional = matches!(ty, Ty::Opt(_));
            let ty = ty.present().clone();
            // An ordered enum held to the values its range lets in, first to last (§15.210).
            if let Some((lo, hi)) = self.c.enum_ranges.get(&i.name.text).and_then(|vs| Some((vs.first()?, vs.last()?))) {
                let v = local(&i.name.text);
                let (v, wrap) = if optional {
                    ("__v".to_string(), Some((format!("    if let __v = {v} {{\n"), "    }\n".to_string())))
                } else {
                    (v, None)
                };
                let mut o = Guarded::new(&mut o, wrap, "    ");
                o.push_str(&format!(
                    "    if {v}.rank < {}.rank || {v}.rank > {}.rank {{\n        throw RuleError.input(what: \"{}\", value: nil)\n    }}\n",
                    self.sw_value(lo, &ty),
                    self.sw_value(hi, &ty),
                    crate::door::out_of_range(&i.name.text)
                ));
                continue;
            }
            if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                continue;
            }
            let v = local(&i.name.text);
            // A branded value is a struct around its integer, which is what the guard compares.
            let inside = if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) { "__v.value" } else { "__v" };
            let (v, wrap) = if optional {
                (inside.to_string(), Some((format!("    if let __v = {v} {{\n"), "    }\n".to_string())))
            } else {
                (v, None)
            };
            // Each end the range writes, and −(2^63−1) where it writes no lower one: an Int64
            // holds one value below that, and the door refuses it (§15.295).
            let Some(test) = or_out_of(&v, crate::door::ends(self.c, &i.name.text, false), "") else {
                continue;
            };
            let mut o = Guarded::new(&mut o, wrap, "    ");
            o.push_str(&format!(
                "    if {test} {{\n        throw RuleError.input(what: \"{}\", value: Int64({v}))\n    }}\n",
                crate::door::out_of_range(&i.name.text)
            ));
        }

        o.push_str(&self.constraint_guards(&local, Lang::Sw, "    ", |m| {
            format!("        throw RuleError.input(what: \"{m}\", value: nil)\n")
        }));

        let trace = self.temp("trace");
        let has_table = self.f.items.iter().any(|i| matches!(i, Item::Table(_)));
        o.push_str(&format!("    {} {trace}: [Fired] = []\n", if has_table { "var" } else { "let" }));

        if self.f.walks() {
            o.push_str(&self.sw_walks(&local, &trace));
        } else {
            o.push_str(&self.sw_items(&local, &trace, Phase::All));
        }

        let wrap = |ty: &Ty, body: String| -> String {
            match ty {
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => format!("{}({body})", self.sw_ty(ty)),
                _ => body,
            }
        };
        // Every output takes the same three steps: its source — the `result` expression for
        // the first output, the binding of its own name otherwise — brought to the wire
        // scale, then its declared rounding applied exactly once (§15.16).
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let ty = self.ty_of(out_name);
            // An optional output is its column's optional, taken through the output's steps only
            // when it is there (§15.249). An enum, a truth value, a string and a date take no step.
            let opt = self.opt_out(oi);
            if opt && !ty.present().is_numeric() {
                finals.push(local(out_name));
                continue;
            }
            // The value inside the optional: the brand's integer, or the integer a `number` is.
            let inside = if matches!(ty.present(), Ty::Number) { "$0" } else { "$0.value" };
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ if opt => Expr2 { text: inside.into(), scale: self.scale(out_name) },
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let res = self.onto_wire(res, os, od);
            let body = match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    if res.scale != os && !opt {
                        let raw = self.temp(&raw_base(oi));
                        o.push_str(&format!(
                            "    let {} = {}  // {}\n",
                            sw_name(&raw),
                            sw_expr(unparen(&res.text)),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("{}({}, {grid_i}) / {}", sw_round_fn(m), sw_name(&raw), res.scale / os)
                    } else if res.scale != os {
                        format!("{}({}, {grid_i}) / {}", sw_round_fn(m), sw_expr(unparen(&res.text)), res.scale / os)
                    } else {
                        format!("{}({}, {grid_i})", sw_round_fn(m), sw_expr(&res.text))
                    }
                }
                None => sw_expr(&res.text),
            };
            if opt {
                finals.push(format!("{}.map {{ {} }}", local(out_name), wrap(ty.present(), body)));
            } else {
                finals.push(wrap(&ty, body));
            }
        }
        if outs.len() == 1 {
            let cuts = walked.as_ref().map(|(_, c)| format!(", {c}")).unwrap_or_default();
            o.push_str(&format!("    return ({}, {trace}{cuts})\n}}\n", finals[0]));
        } else {
            let fields: Vec<String> = outs
                .iter()
                .zip(&finals)
                .map(|(od, body)| format!("{}: {body}", sw_name(&pub_name(&od.name))))
                .collect();
            let cuts = walked.as_ref().map(|(_, c)| format!(", {c}")).unwrap_or_default();
            o.push_str(&format!("    return (Output({}), {trace}{cuts})\n}}\n", fields.join(", ")));
        }
        o
    }

    fn sw_table(&self, t: &Table, local: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = format!("    // {}\n", self.table_head(t));
        // Swift lets a `let` be assigned once on every path; the throwing `else` below is a
        // path that assigns nothing, and the compiler accepts it because it does not return.
        for oc in &t.outputs {
            let ty = self.ty_of(&oc.name.text);
            let decl = match ty {
                Ty::Bool => "Bool".to_string(),
                Ty::Str => "String".to_string(),
                Ty::Enum(_) => self.sw_ty(&ty),
                // An optional column is the optional an optional input is, of the brand
                // (§15.249): an input that may be absent passes on as it stands.
                Ty::Opt(_) => self.sw_ty(&ty),
                _ => "Int64".to_string(),
            };
            o.push_str(&format!("    let {}: {decl}\n", self.sw_ident(&oc.name.text)));
        }
        for (ri, row) in t.rows.iter().enumerate() {
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.sw_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            let cond = if conds.is_empty() { "true".into() } else { conds.join(" && ") };
            let cells: Vec<String> = row.cells.iter().map(cell_src).chain(row.outs.iter().map(out_src)).collect();
            let line = self.row_head(t, ri, &cells.join(" | "));
            let head = if ri == 0 { "    if" } else { "    } else if" };
            o.push_str(&format!("{head} {cond} {{ // {line}\n"));
            for (oi, oc) in t.outputs.iter().enumerate() {
                let col_ty = self.ty_of(&oc.name.text);
                // A value written into an optional column is one of the brand, which Swift makes
                // optional on its own; `none` is `nil` (§15.249).
                let branded = |v: String| -> String {
                    match &col_ty {
                        Ty::Opt(inner) if matches!(inner.as_ref(), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) => {
                            format!("{}({v})", self.sw_ty(inner))
                        }
                        _ => v,
                    }
                };
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => branded(self.int_lit(n, &col_ty, self.scale(&oc.name.text))),
                    Some(OutCell::Lit(l)) => match l {
                        Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                        Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                        Lit::Word(w) => self.sw_value(w, &col_ty),
                        Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                        Lit::Str(x) => str_lit(x),
                        _ => "0".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            "true".into()
                        } else if w == crate::kw::FALSE {
                            "false".into()
                        } else if self.is_value(w) {
                            self.sw_value(w, &col_ty)
                        } else if let (Ty::Opt(inner), Ty::Opt(_)) = (&col_ty, self.ty_of(w)) {
                            // A value that may be absent passes on as the optional it is, and is
                            // scaled only when it is there.
                            let v = local(w);
                            let brand = matches!(inner.as_ref(), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate);
                            let raw = if brand { "$0.value" } else { "$0" };
                            let scaled = self.rescaled(w, &oc.name.text, raw.to_string());
                            if scaled == raw {
                                v
                            } else if brand {
                                format!("{v}.map {{ {}({}) }}", self.sw_ty(inner), sw_expr(&scaled))
                            } else {
                                format!("{v}.map {{ {} }}", sw_expr(&scaled))
                            }
                        } else {
                            branded(sw_expr(&self.rescaled(w, &oc.name.text, local(w))))
                        }
                    }
                    Some(OutCell::Nothing) => "nil".into(),
                    None => "0".into(),
                };
                o.push_str(&format!("        {} = {v}\n", self.sw_ident(&oc.name.text)));
            }
            let (tn, rn, lbl) = self.fired_of(t, ri);
            if lbl.is_empty() {
                o.push_str(&format!("        {trace}.append(Fired(table: {tn:?}, row: {rn}))\n"));
            } else {
                o.push_str(&format!("        {trace}.append(Fired(table: {tn:?}, row: {rn}, label: {lbl:?}))\n"));
            }
        }
        o.push_str(&format!(
            "    }} else {{\n        throw RuleError.contradiction(what: \"{}\")\n    }}\n",
            tr!("到達不能: 完全性は rulec が静的に検査済み", "unreachable: completeness was statically checked by rulec")
        ));
        for oc in &t.outputs {
            o.push_str(&self.sw_unread(&oc.name.text));
        }
        o.push_str(&self.guards(t, local, Lang::Sw, "    ", |name, i, j| {
            format!(
                "        throw RuleError.contradiction(what: \"{}\")\n",
                tr!("表 {name}: {i} と {j} が同時に当てはまりました", "table {name}: {i} and {j} matched at the same time")
            )
        }));
        o
    }

    /// The runner. Foundation's `JSONSerialization` is to Swift what `json` is to Ruby —
    /// part of the toolchain rather than a dependency. On macOS it keeps an integer that fits
    /// in `Int64` exact; on Linux a negative one of 19 digits is an `NSDecimalNumber`, which
    /// `_int` reads through `doubleValue` (§15.295). The answer is written by hand, not serialized,
    /// as the comparison is byte-for-byte and the fields go in the order the rule declares its outputs.
    ///
    /// `@main` needs the runner not to be the module's main file, which is why it is
    /// `{alias}_runner.swift` and not `main.swift`.
    pub fn swift_runner(&self) -> String {
        // the runner is built with the module, whose enums' types it reads by their names (§15.336)
        self.apart_types(clash::Syntax::Swift, |g| g.swift_runner_text())
    }

    fn swift_runner_text(&self) -> String {
        // The module takes its name from the binary, so the call has to be the function's
        // own spelling: a plain `shipping_fee(...)` resolves to the *module* and fails to
        // compile, which is how this was found.
        let fname = sw_name(&pub_name(&self.f.name));
        let d = runner_local("d", &fname);
        // A rule that reads no input reads nothing out of the record's `in`, which Swift warns of
        // where it is held unread (§15.330).
        let d_decl = if self.f.inputs.is_empty() && self.f.elements.is_empty() { "_".to_string() } else { d.clone() };
        let got = runner_local("got", &fname);
        let line = runner_local("line", &fname);
        let root = runner_local("root", &fname);
        let trace = runner_local("trace", &fname);
        let (state, stepping) = (runner_local("state", &fname), runner_local("stepping", &fname));
        // One input read off the wire the way every door reads it (§15.204). Every reader
        // throws the module's own `RuleError.input`, with the sentence every language says; a
        // statement that reads one starts with `try`, which covers the whole of it.
        // `said` is what the sentences call the value: the name, or for a field of an element its
        // place, `lines[\u{1}].amount`, which takes the index `at` of the element (§15.324).
        let read = |ty: &Ty, src: &str, name: &str, said: &str, at: &str| -> String {
            let lit = |x: String| -> String { if names_element(&x) { sw_said(&x, at, "") } else { format!("{x:?}") } };
            let optional = matches!(ty, Ty::Opt(_));
            let take = format!("_take({src}, {name:?}, {}{})", lit(crate::door::missing(said)), if optional { ", true" } else { "" });
            let one = |v: &str| -> String {
                match ty.present() {
                    Ty::Enum(n) => {
                        let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                        format!("_enum({cls}.self, {v}, {})", lit(crate::door::not_in_enum(said, n)))
                    }
                    Ty::Bool => format!("_bool({v}, {})", lit(crate::door::not_bool(said))),
                    Ty::Date => format!("_date({v}, {})", lit(crate::door::not_date(said))),
                    Ty::Str => format!("_str({v}, {})", lit(crate::door::not_string(said))),
                    Ty::Number => format!("_int({v}, {}, {})", lit(crate::door::not_integer(said)), lit(crate::door::out_of_range(said))),
                    other => format!("{}(_int({v}, {}, {}))", self.sw_ty(other), lit(crate::door::not_integer(said)), lit(crate::door::out_of_range(said))),
                }
            };
            if optional {
                // `null` on the wire is `nil` (DESIGN §15.88).
                format!("_maybe({take}) {{ try {} }}", one("$0"))
            } else {
                one(&take)
            }
        };
        let mut args: Vec<String> = Vec::new();
        let mut binds: Vec<String> = Vec::new();
        // `--chances` reads every input of a line plainly, as the vectors are read (§15.225).
        let mut plain: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            let label = sw_label(&pub_name(&i.name));
            let v = read(&ty, &d, &i.name.text, &i.name.text, "");
            let local = runner_local(&format!("a{}", args.len()), &fname);
            plain.push(format!("                let {local} = try {v}\n"));
            // A machine's traces (§15.148): the carried input is the state this language
            // answered to the call before, handed over as the value it is — and so is each
            // value carried along with it (§15.217).
            let more_local = self
                .more_carried()
                .into_iter()
                .find(|mc| mc.input == i.name.text && self.machine_consts().is_some())
                .map(|mc| runner_local(&format!("carried_{}", mc.in_alias), &fname));
            let v = if self.carried().map(|(c, _)| c) == Some(i.name.text.clone()) {
                format!("{stepping} ? {state} : {v}")
            } else if let Some(local) = more_local {
                format!("{stepping} ? {local} : {v}")
            } else {
                v
            };
            binds.push(format!("                let {local} = try {v}\n"));
            args.push(format!("{label}: {local}"));
        }
        // A sequence arrives as an array of objects (§10.2), one per element (§15.219).
        for (s, el) in self.f.elements.iter().enumerate() {
            let seq = &el.name.text;
            let label = sw_label(&pub_name(&el.name));
            let local = runner_local(&format!("a{}", args.len()), &fname);
            let ev = runner_local("e", &fname);
            // A sentence about an element names it by its index (§15.324).
            let at = runner_local("i", &fname);
            let obj = format!("_obj({ev}, {})", sw_said(&crate::door::not_object(seq, crate::door::HERE), &at, ""));
            let inner: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let said = crate::door::element(seq, crate::door::HERE, &fd.name.text);
                    format!("{}: {}", sw_name(&pub_name(&fd.name)), read(&self.ty_of(&fd.name.text), &obj, &fd.name.text, &said, &at))
                })
                .collect();
            binds.push(format!(
                "                let {local} = try _seq(_take({d}, {seq:?}, {:?}), {:?}).enumerated().map {{ {at}, {ev} in try {}({}) }}\n",
                crate::door::missing(seq),
                crate::door::not_sequence(seq),
                self.element_type(s),
                inner.join(", ")
            ));
            plain.push(binds.last().cloned().unwrap_or_default());
            args.push(format!("{label}: {local}"));
        }
        // What the function that counts the answers is passed: the inputs that are not random, and
        // the sequences, by their labels.
        let chances_args: Vec<String> = self
            .f
            .inputs
            .iter()
            .zip(args.iter())
            .filter(|(i, _)| i.random.is_none())
            .map(|(_, a)| a.clone())
            .chain(args[self.f.inputs.len()..].iter().cloned())
            .collect();
        let args = args.join(", ");
        // the record's arguments, before what it is told came out (§15.330)
        let record_args = leading_param(&args);
        // The record function's own parameter names, which are the call's labels.
        let (p_out, p_trace) = (sw_name(&self.temp("out")), sw_name(&self.temp("trace")));
        let (mut head, mut meta, mut tail) = (String::new(), String::new(), String::new());
        if let (Some((en, _, _)), Some(out)) = (self.machine_consts(), self.carried_out_alias()) {
            let cls = self.enum_names.get(&en).cloned().unwrap_or_default();
            // The values carried along with the state (§15.217): a local each, from the
            // module's constant, read where each trace starts and handed on from the answer.
            let (mut more_head, mut more_meta, mut more_start, mut more_tail) = (String::new(), Vec::<String>::new(), String::new(), String::new());
            for mc in self.more_carried() {
                let local = runner_local(&format!("carried_{}", mc.in_alias), &fname);
                more_head.push_str(&format!("        var {local}: {} = {}\n", self.sw_ty(&mc.ty), mc.swift()));
                let key = mc.key().replace('\\', "\\\\").replace('"', "\\\"");
                let shown = match mc.kind() {
                    MoreKind::Enum => format!("\\\"\\({}.rawValue)\\\"", mc.swift()),
                    MoreKind::Branded => format!("\\({}.value)", mc.swift()),
                    _ => format!("\\({})", mc.swift()),
                };
                more_meta.push(format!("{key}:{shown}"));
                more_start.push_str(&format!("                {local} = try {}\n", read(&mc.ty, &format!("{root}[\"carried\"]"), &mc.input, &mc.input, "")));
                more_tail.push_str(&format!("                    {local} = {got}.{}\n", sw_name(&mc.out_alias)));
            }
            let carried_meta = if more_meta.is_empty() { String::new() } else { format!(",\\\"carried\\\":{{{}}}", more_meta.join(",")) };
            head = format!("        var {state}: {cls} = initialState\n{more_head}");
            meta = format!(
                "            if {root}[\"machine\"] != nil {{\n                \
                 let fin = {cls}.allCases.filter {{ isFinal($0) }}.map {{ \"\\\"\\($0.rawValue)\\\"\" }}.joined(separator: \",\")\n                \
                 print(\"{{\\\"initial\\\":\\\"\\(initialState.rawValue)\\\",\\\"final\\\":[\\(fin)]{carried_meta}}}\")\n                \
                 continue\n            }}\n"
            );
            tail = format!("                if {stepping} {{\n                    {state} = {}\n{more_tail}                }}\n", self.next_state_of(&got, &format!(".{}", sw_name(&out))));
            meta.push_str(&format!(
                "            let {stepping} = {root}[\"step\"] != nil\n            \
                 if ({root}[\"step\"] as? String) == \"start\" {{\n                {state} = {cls}(rawValue: {root}[\"state\"] as! String)!\n{more_start}            }}\n"
            ));
        }
        format!(
            r#"// Code generated by rulec {ver}. DO NOT EDIT.
import Foundation

@main
enum Runner {{
    static func main() throws {{
{labels}{head}        while let {line} = readLine(strippingNewline: true) {{
            if {line}.trimmingCharacters(in: .whitespaces).isEmpty {{
                continue
            }}
            let {root} = try JSONSerialization.jsonObject(with: Data({line}.utf8)) as! [String: Any]
{meta}            let {d_decl}: Any? = {root}["in"]
            // One call, or the refusal its door gives, in the place of the record (§15.204).
            do {{
{binds}                let ({got}, {trace}) = try {fname}Traced({args})
                print({fname}Record({record_args}{p_out}: {got}, {p_trace}: {trace}))
{tail}            }} catch RuleError.input(let what, _) {{
                _refused("input", what)
            }} catch RuleError.contradiction(let what) {{
                _refused("contradiction", what)
            }}
        }}
    }}

{wire}}}
"#,
            ver = ritsu_emit::header::VERSION,
            binds = binds.join(""),
            wire = SWIFT_WIRE,
            labels = self.sw_labels_mode() + &self.sw_prereq_mode() + &self.sw_chances_mode(&d, &plain.join(""), &chances_args.join(", "), &args, &p_out, &p_trace),
        )
    }

    /// What the Swift runner answers with `--labels` (§15.211): for each line, an enum's name and
    /// a value's, the display name the module gives.
    fn sw_labels_mode(&self) -> String {
        let enums = self.labeled_enums();
        if enums.is_empty() {
            return String::new();
        }
        let arms: String = enums
            .iter()
            .map(|(en, ty, _)| format!("                case {}: print({ty}(rawValue: v)?.label ?? \"?\")\n", str_lit(en)))
            .collect();
        format!(
            "        if CommandLine.arguments.contains(\"--labels\") {{\n            while let l = readLine(strippingNewline: true) {{\n                let parts = l.split(separator: \"\\t\", maxSplits: 1).map(String.init)\n                guard parts.count == 2 else {{ continue }}\n                let v = parts[1]\n                switch parts[0] {{\n{arms}                default: print(\"?\")\n                }}\n            }}\n            return\n        }}\n"
        )
    }

    /// The type as the inventory states it for Swift.
    fn sw_api_ty(&self, ty: &Ty) -> String {
        self.sw_ty(ty)
    }
}

/// The five modes of §7.3 in Swift, checked against the same reference cases the other five
/// are. A lone file compiled by `swiftc` is the module's main file, so this one is written
/// as top-level code and needs no `@main`.
pub fn round_tests_swift(u: &Uses) -> String {
    let mut o = format!(
        "// Code generated by rulec {}. DO NOT EDIT.\n// {}\nimport Foundation\n",
        ritsu_emit::header::VERSION,
        tr!(
            "丸めの五つのモード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five rounding modes, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str(round_sw().trim_start_matches('\n'));
    o.push_str(&arith::sw_helpers(u));
    o.push_str("\nlet cases: [(String, Int64, Int64, Int64)] = [\n");
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("    (\"{}\", {x}, {g}, {want}),\n", mode_fn(m)));
    }
    o.push_str("]\n\nvar bad = 0\nfor (mode, x, g, want) in cases {\n    let got: Int64\n    switch mode {\n");
    o.push_str("    case \"down\": got = _roundDown(x, g)\n    case \"up\": got = _roundUp(x, g)\n");
    o.push_str("    case \"half\": got = _roundHalf(x, g)\n    case \"half_down\": got = _roundHalfDown(x, g)\n    default: got = _roundBankers(x, g)\n    }\n");
    o.push_str("    if got != want {\n        print(\"NG \\(mode)(\\(x), \\(g)) = \\(got), want \\(want)\")\n        bad += 1\n    }\n}\n");
    let (arith, n) = arith::sw_tests(u);
    o.push_str(&arith);
    o.push_str("if bad > 0 {\n    exit(1)\n}\n");
    let count = if n == 0 { "cases.count" } else { "cases.count + arith.count" };
    o.push_str(&format!("print(\"{}\")\n", tr!("ok \\({count}) 件", "ok \\({count}) cases")));
    o
}


// ---------------------------------------------------------------------------
// The record function (§15.35): one call as one line of the fixtures format.
// ---------------------------------------------------------------------------

/// Howard Hinnant's civil_from_days, as each language spells it. Only emitted when the rule
/// has a date anywhere on its surface: an input, an output, or a field of an element, which the
/// record writes back as a day too.
fn civil_needed(f: &RuleFile, c: &Checked) -> bool {
    let date = |t: Option<Ty>| match t {
        Some(Ty::Date) => true,
        Some(Ty::Opt(t)) => *t == Ty::Date,
        _ => false,
    };
    f.inputs
        .iter()
        .map(|i| &i.name.text)
        .chain(f.outputs.iter().map(|o| &o.name.text))
        .any(|n| matches!(c.ty_of(n), Some(Ty::Date) | Some(Ty::Opt(_))))
        || f.element_fields().any(|fd| date(c.ty_of(&fd.name.text)))
}

impl<'a> Gen<'a> {
    /// The inputs of the rule with the expression the record function reads each from, in
    /// the language's own spelling, and the outputs likewise.
    fn record_fields(&self, input: impl Fn(&str, &Ty) -> String, output: impl Fn(usize, &str, &Ty) -> String) -> (Vec<(String, String, Wire)>, Vec<(String, String, Wire)>) {
        let ins = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                (i.name.text.clone(), input(&self.held(&i.name), &ty), wire_of(&ty))
            })
            .collect();
        let outs = self
            .f
            .outputs
            .iter()
            .enumerate()
            .map(|(k, o)| {
                let ty = self.ty_of(&o.name.text);
                (o.name.text.clone(), output(k, &pub_name(&o.name), &ty), wire_of(&ty))
            })
            .collect();
        (ins, outs)
    }

    // ── Python
    fn py_wire(e: &str, w: &Wire) -> String {
        match w {
            Wire::Enum => format!("_json_str({e}.value)"),
            Wire::Bool => format!("(\"true\" if {e} else \"false\")"),
            Wire::Int | Wire::Brand => format!("f\"{{{e}}}\""),
            Wire::Date => format!("_json_str(_civil({e}))"),
            Wire::Str => format!("_json_str({e})"),
            // A truth value that may be absent is compared with `True` rather than read as a
            // condition: its type is `bool | None` wherever the `is None` above is not followed
            // (§15.208).
            Wire::Opt(w) if matches!(**w, Wire::Bool) => format!("(\"null\" if {e} is None else (\"true\" if {e} is True else \"false\"))"),
            Wire::Opt(w) => format!("(\"null\" if {e} is None else {})", Self::py_wire(e, w)),
        }
    }

    pub fn py_record(&self) -> String {
        let fname = pub_name(&self.f.name);
        let single = self.f.outputs.len() == 1;
        let (out, trace, tag) = (self.temp("out"), self.temp("trace"), self.temp("tag"));
        let (ins, obs) = self.record_fields(
            |a, _| a.to_string(),
            |_, a, _| if single { out.clone() } else { format!("{out}.{a}") },
        );
        let ret = if single { self.py_ty(&self.ty_of(&self.f.outputs[0].name.text)) } else { "Output".into() };
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", pub_name(&i.name), self.py_ty(&self.ty_of(&i.name.text))))
            .collect();
        for (s, el) in self.f.elements.iter().enumerate() {
            params.push(format!("{}: list[{}]", pub_name(&el.name), self.element_type(s)));
        }
        let mut o = String::new();
        o.push_str(
            "def _json_str(s: str) -> str:\n    o = '\"'\n    for ch in s:\n        if ch == '\"' or ch == \"\\\\\":\n            o += \"\\\\\" + ch\n        elif ord(ch) < 32:\n            o += \"\\\\u%04x\" % ord(ch)\n        else:\n            o += ch\n    return o + '\"'\n\n\n",
        );
        if civil_needed(self.f, self.c) {
            o.push_str(
                "def _civil(days: int) -> str:\n    z = days + 719468\n    era = (z if z >= 0 else z - 146096) // 146097\n    doe = z - era * 146097\n    yoe = (doe - doe // 1460 + doe // 36524 - doe // 146096) // 365\n    doy = doe - (365 * yoe + yoe // 4 - yoe // 100)\n    mp = (5 * doy + 2) // 153\n    d = doy - (153 * mp + 2) // 5 + 1\n    m = mp + 3 if mp < 10 else mp - 9\n    y = yoe + era * 400 + (1 if m <= 2 else 0)\n    return f\"{y:04d}-{m:02d}-{d:02d}\"\n\n\n",
            );
        }
        o.push_str(&format!(
            "def {fname}_record({}{out}: {ret}, {trace}: _Trace, {tag}: str = \"\") -> str:\n    \"\"\"{}\"\"\"\n",
            leading_params(&params),
            record_doc()
        ));
        // As the traced function does, where some input is bound apart from its name (§15.336).
        if let Some((held, _)) = self.py_held_params() {
            let mut args: Vec<String> = self.f.inputs.iter().map(|i| pub_name(&i.name)).collect();
            args.extend(self.f.elements.iter().map(|el| pub_name(&el.name)));
            let inner = self.py_inner(&format!("{fname}_record"));
            o.push_str(&format!("    return {inner}({}{out}, {trace}, {tag})\n\n\n", leading_params(&args)));
            o.push_str(&format!("def {inner}({}{out}: {ret}, {trace}: _Trace, {tag}: str = \"\") -> str:\n", leading_params(&held)));
        }
        let (v_ins, v_obs, v_rows, v_head) = (self.temp("ins"), self.temp("obs"), self.temp("rows"), self.temp("head"));
        let field = |(jp, e, w): &(String, String, Wire)| format!("        '\"{jp}\":' + {},\n", Self::py_wire(e, w));
        // The sequence goes in first, and one element is written the way one record's inputs
        // are: the same wire, one level in (§15.56).
        let mut seq_first = String::new();
        for (s, el) in self.f.elements.iter().enumerate() {
            let seq = if s == 0 { self.temp("seq") } else { self.temp(&format!("seq{}", s + 1)) };
            let inner: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    format!(
                        "'\"{}\":' + {}",
                        fd.name.text,
                        Self::py_wire(&format!("e.{}", pub_name(&fd.name)), &wire_of(&ty))
                    )
                })
                .collect();
            o.push_str(&format!(
                "    {seq} = '\"{}\":[' + \",\".join(\"{{\" + {} + \"}}\" for e in {}) + \"]\"\n",
                el.name.text,
                inner.join(" + \",\" + "),
                self.held(&el.name)
            ));
            seq_first.push_str(&format!("        {seq},\n"));
        }
        if seq_first.is_empty() && ins.is_empty() {
            // an empty list has no type mypy can tell
            o.push_str(&format!("    {v_ins}: list[str] = []\n"));
        } else {
            o.push_str(&format!("    {v_ins} = [\n{seq_first}{}    ]\n", ins.iter().map(field).collect::<String>()));
        }
        o.push_str(&format!("    {v_obs} = [\n{}    ]\n", obs.iter().map(field).collect::<String>()));
        o.push_str(&format!(
            "    {v_rows} = ['{{\"table\":' + _json_str(f.table) + ',\"row\":' + f\"{{f.row}}\" + (',\"label\":' + _json_str(f.label) if f.label != \"\" else \"\") + \"}}\" for f in {trace}]\n"
        ));
        o.push_str(&format!("    {v_head} = '{{\"tag\":' + _json_str({tag}) + \",\" if {tag} != \"\" else \"{{\"\n"));
        o.push_str(&format!(
            "    return {v_head} + '\"in\":{{' + \",\".join({v_ins}) + '}},\"observed\":{{' + \",\".join({v_obs}) + '}},\"trace\":[' + \",\".join({v_rows}) + \"]}}\"\n"
        ));
        o
    }

    // ── TypeScript
    fn ts_wire(e: &str, w: &Wire) -> String {
        match w {
            Wire::Enum | Wire::Str => format!("JSON.stringify({e})"),
            Wire::Bool | Wire::Int | Wire::Brand => format!("String({e})"),
            Wire::Date => format!("JSON.stringify(_civil({e}))"),
            Wire::Opt(w) => format!("({e} === null ? \"null\" : {})", Self::ts_wire(e, w)),
        }
    }

    pub fn ts_record(&self) -> String {
        let fname = pub_name(&self.f.name);
        let single = self.f.outputs.len() == 1;
        let (out, trace, tag) = (self.temp("out"), self.temp("trace"), self.temp("tag"));
        let (ins, obs) = self.record_fields(
            |a, _| a.to_string(),
            |_, a, _| if single { out.clone() } else { format!("{out}.{a}") },
        );
        let ret = if single { self.ts_ty(&self.ty_of(&self.f.outputs[0].name.text)) } else { "Output".into() };
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", self.held(&i.name), self.ts_ty(&self.ty_of(&i.name.text))))
            .collect();
        for (s, el) in self.f.elements.iter().enumerate() {
            params.push(format!("{}: readonly {}[]", self.held(&el.name), self.element_type(s)));
        }
        let mut o = String::new();
        if civil_needed(self.f, self.c) {
            o.push_str(
                "function _civil(days: bigint): string {\n  const z = days + 719468n;\n  const era = (z >= 0n ? z : z - 146096n) / 146097n;\n  const doe = z - era * 146097n;\n  const yoe = (doe - doe / 1460n + doe / 36524n - doe / 146096n) / 365n;\n  const doy = doe - (365n * yoe + yoe / 4n - yoe / 100n);\n  const mp = (5n * doy + 2n) / 153n;\n  const d = doy - (153n * mp + 2n) / 5n + 1n;\n  const m = mp < 10n ? mp + 3n : mp - 9n;\n  const y = yoe + era * 400n + (m <= 2n ? 1n : 0n);\n  return String(y).padStart(4, \"0\") + \"-\" + String(m).padStart(2, \"0\") + \"-\" + String(d).padStart(2, \"0\");\n}\n\n",
            );
        }
        o.push_str(&format!("/** {} */\n", record_doc()));
        o.push_str(&format!(
            "export function {fname}_record({}{out}: {ret}, {trace}: Fired[], {tag} = \"\"): string {{\n",
            leading_params(&params)
        ));
        let (v_ins, v_obs, v_rows, v_head) = (self.temp("ins"), self.temp("obs"), self.temp("rows"), self.temp("head"));
        let field = |(jp, e, w): &(String, String, Wire)| format!("    '\"{jp}\":' + {},\n", Self::ts_wire(e, w));
        // The sequence goes in first, written the way one record's inputs are (§15.56).
        let mut seq_first = String::new();
        for el in &self.f.elements {
            let e = self.temp("elem");
            let inner: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    format!(
                        "'\"{}\":' + {}",
                        fd.name.text,
                        Self::ts_wire(&format!("{e}.{}", pub_name(&fd.name)), &wire_of(&ty))
                    )
                })
                .collect();
            seq_first.push_str(&format!(
                "    '\"{}\":[' + {}.map(({e}) => \"{{\" + {} + \"}}\").join(\",\") + \"]\",\n",
                el.name.text,
                self.held(&el.name),
                inner.join(" + \",\" + ")
            ));
        }
        o.push_str(&format!("  const {v_ins} = [\n{seq_first}{}  ].join(\",\");\n", ins.iter().map(field).collect::<String>()));
        o.push_str(&format!("  const {v_obs} = [\n{}  ].join(\",\");\n", obs.iter().map(field).collect::<String>()));
        o.push_str(&format!(
            "  const {v_rows} = {trace}.map((f) => '{{\"table\":' + JSON.stringify(f.table) + ',\"row\":' + String(f.row) + (f.label ? ',\"label\":' + JSON.stringify(f.label) : \"\") + \"}}\").join(\",\");\n"
        ));
        o.push_str(&format!("  const {v_head} = {tag} === \"\" ? \"{{\" : '{{\"tag\":' + JSON.stringify({tag}) + \",\";\n"));
        o.push_str(&format!(
            "  return {v_head} + '\"in\":{{' + {v_ins} + '}},\"observed\":{{' + {v_obs} + '}},\"trace\":[' + {v_rows} + \"]}}\";\n}}\n"
        ));
        o
    }

    // ── Rust
    fn rs_wire(e: &str, w: &Wire) -> String {
        match w {
            Wire::Enum => format!("json_str({e}.as_str())"),
            Wire::Bool | Wire::Int => format!("{e}.to_string()"),
            Wire::Brand => format!("{e}.0.to_string()"),
            Wire::Date => format!("json_str(&civil({e}))"),
            Wire::Str => format!("json_str(&{e})"),
            // A string is not Copy: one that may be absent is borrowed, not taken out of what holds
            // it, which is an element the record reads through a reference (§15.343). The values of
            // the other types are Copy, and are written as they were.
            Wire::Opt(w) if matches!(**w, Wire::Str) => format!("{e}.as_deref().map(json_str).unwrap_or_else(|| \"null\".to_string())"),
            Wire::Opt(w) => format!("{e}.map(|v| {}).unwrap_or_else(|| \"null\".to_string())", Self::rs_wire("v", w)),
        }
    }

    pub fn rs_record(&self) -> String {
        let fname = pub_name(&self.f.name);
        let single = self.f.outputs.len() == 1;
        let (out, trace, tag) = (self.temp("out"), self.temp("trace"), self.temp("tag"));
        let (ins, obs) = self.record_fields(
            |a, _| a.to_string(),
            |_, a, _| if single { out.clone() } else { format!("{out}.{a}") },
        );
        let ret = if single { self.rs_ty(&self.ty_of(&self.f.outputs[0].name.text)) } else { "Output".into() };
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", self.held(&i.name), self.rs_ty(&self.ty_of(&i.name.text))))
            .collect();
        for (s, el) in self.f.elements.iter().enumerate() {
            params.push(format!("{}: &[{}]", self.held(&el.name), self.element_type(s)));
        }
        let mut o = String::new();
        o.push_str(RS_JSON_STR);
        if civil_needed(self.f, self.c) {
            o.push_str(
                "fn civil(days: i64) -> String {\n    let z = days + 719468;\n    let era = if z >= 0 { z } else { z - 146096 } / 146097;\n    let doe = z - era * 146097;\n    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;\n    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);\n    let mp = (5 * doy + 2) / 153;\n    let d = doy - (153 * mp + 2) / 5 + 1;\n    let m = if mp < 10 { mp + 3 } else { mp - 9 };\n    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };\n    format!(\"{y:04}-{m:02}-{d:02}\")\n}\n\n",
            );
        }
        o.push_str(&format!("/// {}\n", record_doc()));
        o.push_str(&format!(
            "pub fn {fname}_record({}{out}: {ret}, {trace}: &[Fired], {tag}: &str) -> String {{\n",
            leading_params(&params)
        ));
        let (v_ins, v_obs, v_rows, v_head) = (self.temp("ins"), self.temp("obs"), self.temp("rows"), self.temp("head"));
        let field = |(jp, e, w): &(String, String, Wire)| format!("        String::from(\"\\\"{jp}\\\":\") + &{},\n", Self::rs_wire(e, w));
        // The sequences go in first, each element written the way one record's inputs are.
        let mut seq_first = String::new();
        for el in &self.f.elements {
            let inner: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    format!(
                        "String::from(\"\\\"{}\\\":\") + &{}",
                        fd.name.text,
                        Self::rs_wire(&format!("e.{}", pub_name(&fd.name)), &wire_of(&ty))
                    )
                })
                .collect();
            seq_first.push_str(&format!(
                "        String::from(\"\\\"{}\\\":[\") + &{}.iter().map(|e| String::from(\"{{\") + &({}) + \"}}\").collect::<Vec<_>>().join(\",\") + \"]\",\n",
                el.name.text,
                self.held(&el.name),
                inner.join(" + \",\" + &")
            ));
        }
        if seq_first.is_empty() && ins.is_empty() {
            // an empty array has no type Rust can tell
            o.push_str(&format!("    let {v_ins} = String::new();\n"));
        } else {
            o.push_str(&format!("    let {v_ins} = [\n{seq_first}{}    ]\n    .join(\",\");\n", ins.iter().map(field).collect::<String>()));
        }
        o.push_str(&format!("    let {v_obs} = [\n{}    ]\n    .join(\",\");\n", obs.iter().map(field).collect::<String>()));
        o.push_str(&format!(
            "    let {v_rows} = {trace}\n        .iter()\n        .map(|f| String::from(\"{{\\\"table\\\":\") + &json_str(f.table) + \",\\\"row\\\":\" + &f.row.to_string() + &(if f.label.is_empty() {{ String::new() }} else {{ String::from(\",\\\"label\\\":\") + &json_str(f.label) }}) + \"}}\")\n        .collect::<Vec<_>>()\n        .join(\",\");\n"
        ));
        o.push_str(&format!(
            "    let {v_head} = if {tag}.is_empty() {{ String::from(\"{{\") }} else {{ String::from(\"{{\\\"tag\\\":\") + &json_str({tag}) + \",\" }};\n"
        ));
        o.push_str(&format!(
            "    {v_head} + \"\\\"in\\\":{{\" + &{v_ins} + \"}},\\\"observed\\\":{{\" + &{v_obs} + \"}},\\\"trace\\\":[\" + &{v_rows} + \"]}}\"\n}}\n"
        ));
        o
    }

    // ── Go
    fn go_wire(e: &str, w: &Wire) -> String {
        match w {
            Wire::Enum => format!("jsonStr({e}.String())"),
            Wire::Bool => format!("fmt.Sprint({e})"),
            Wire::Int | Wire::Brand => format!("fmt.Sprint(int64({e}))"),
            Wire::Date => format!("jsonStr(civil(int64({e})))"),
            Wire::Str => format!("jsonStr({e})"),
            // Handled as a statement by the caller: a pointer has to be tested first.
            Wire::Opt(w) => Self::go_wire(&format!("(*{e})"), w),
        }
    }

    /// The function that writes a field of an element of type `ty` where the type may hold no
    /// value (§15.343): what tells it from the others (the pointer's type and how what it points
    /// at is written), the word its name ends with, the pointer's type, and the value it writes
    /// of `*p`. `None` for a type that always holds one.
    fn go_or_null(&self, ty: &Ty) -> Option<(String, String, String, String)> {
        let Wire::Opt(w) = wire_of(ty) else { return None };
        let gt = self.go_ty(ty);
        let write = Self::go_wire(if matches!(*w, Wire::Enum) { "(*p)" } else { "*p" }, &w);
        let base = match *w {
            Wire::Date => "Date".to_string(),
            _ => {
                let t = gt.trim_start_matches('*');
                t.chars().next().map(|c| c.to_uppercase().collect::<String>() + &t[c.len_utf8()..]).unwrap_or_default()
            }
        };
        Some((format!("{gt} {write}"), base, gt, write))
    }

    pub fn go_record(&self) -> String {
        let fname = pascal(&pub_name(&self.f.name));
        let single = self.f.outputs.len() == 1;
        let (ins, obs) = self.record_fields(
            |a, _| format!("in.{}", pascal(a)),
            |_, a, _| if single { "out".to_string() } else { format!("out.{}", pascal(a)) },
        );
        let ret = if single { self.go_ty(&self.ty_of(&self.f.outputs[0].name.text)) } else { "Output".into() };
        let mut o = String::new();
        o.push_str(
            "func jsonStr(s string) string {\n\tb := []byte{'\"'}\n\tfor _, r := range s {\n\t\tswitch {\n\t\tcase r == '\"' || r == '\\\\':\n\t\t\tb = append(b, '\\\\', byte(r))\n\t\tcase r < 32:\n\t\t\tb = append(b, []byte(fmt.Sprintf(\"\\\\u%04x\", r))...)\n\t\tdefault:\n\t\t\tb = append(b, []byte(string(r))...)\n\t\t}\n\t}\n\treturn string(append(b, '\"'))\n}\n\n",
        );
        o.push_str("func joinComma(xs []string) string {\n\ts := \"\"\n\tfor i, x := range xs {\n\t\tif i > 0 {\n\t\t\ts += \",\"\n\t\t}\n\t\ts += x\n\t}\n\treturn s\n}\n\n");
        if civil_needed(self.f, self.c) {
            o.push_str(
                "func civil(days int64) string {\n\tz := days + 719468\n\tera := z\n\tif z < 0 {\n\t\tera = z - 146096\n\t}\n\tera /= 146097\n\tdoe := z - era*146097\n\tyoe := (doe - doe/1460 + doe/36524 - doe/146096) / 365\n\tdoy := doe - (365*yoe + yoe/4 - yoe/100)\n\tmp := (5*doy + 2) / 153\n\td := doy - (153*mp+2)/5 + 1\n\tm := mp - 9\n\tif mp < 10 {\n\t\tm = mp + 3\n\t}\n\ty := yoe + era*400\n\tif m <= 2 {\n\t\ty++\n\t}\n\treturn fmt.Sprintf(\"%04d-%02d-%02d\", y, m, d)\n}\n\n",
            );
        }
        // A field of an element that may hold no value is a pointer, written by a function of its
        // type: `null` where it is nil, and what it points at otherwise. The record's line calls
        // it by its name, where a function literal of more than one statement would be one gofmt
        // writes on lines of its own (§15.343). Two types the rule spells alike take a number.
        let mut or_null: Vec<(String, String)> = Vec::new();
        for fd in self.f.elements.iter().flat_map(|el| el.fields.iter()) {
            let Some((key, base, gt, write)) = self.go_or_null(&self.ty_of(&fd.name.text)) else { continue };
            if or_null.iter().any(|(k, _)| *k == key) {
                continue;
            }
            let base = format!("jsonOrNull{base}");
            let (mut name, mut k) = (base.clone(), 1);
            while or_null.iter().any(|(_, m)| *m == name) {
                k += 1;
                name = format!("{base}{k}");
            }
            o.push_str(&format!("func {name}(p {gt}) string {{\n\tif p == nil {{\n\t\treturn \"null\"\n\t}}\n\treturn {write}\n}}\n\n"));
            or_null.push((key, name));
        }
        o.push_str(&format!("// {fname}Record {}\n", record_doc()));
        o.push_str(&format!("func {fname}Record(in Input, out {ret}, trace []Fired, tag string) string {{\n"));
        // The sequence goes in first, and one element is written the way one record's inputs
        // are: the same wire, one level in (§15.56).
        let mut seq_first: Vec<String> = Vec::new();
        for (s, el) in self.f.elements.iter().enumerate() {
            let seq = if s == 0 { self.temp("seq") } else { self.temp(&format!("seq{}", s + 1)) };
            let g = pascal(&pub_name(&el.name));
            let inner: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    let at = format!("e.{}", pascal(&pub_name(&fd.name)));
                    // An optional field is a pointer, written by the function of its type above. It
                    // was read through with no test once, and a nil one panicked (§15.201).
                    let value = match self.go_or_null(&ty) {
                        Some((key, ..)) => format!("{}({at})", or_null.iter().find(|(k, _)| *k == key).map(|(_, n)| n.as_str()).unwrap_or_default()),
                        None => Self::go_wire(&at, &wire_of(&ty)),
                    };
                    format!("\"\\\"{}\\\":\" + {}", fd.name.text, value)
                })
                .collect();
            o.push_str(&format!("\t{seq} := \"\\\"{}\\\":[\"\n", el.name.text));
            o.push_str(&format!("\tfor i, e := range in.{g} {{\n\t\tif i > 0 {{\n\t\t\t{seq} += \",\"\n\t\t}}\n"));
            o.push_str(&format!("\t\t{seq} += \"{{\" + {} + \"}}\"\n\t}}\n", inner.join(" + \",\" + ")));
            o.push_str(&format!("\t{seq} += \"]\"\n"));
            seq_first.push(seq);
        }
        let mut list = |name: &str, fields: &[(String, String, Wire)]| {
            o.push_str(&format!("\t{name} := []string{{}}\n"));
            if name == "ins" {
                for seq in &seq_first {
                    o.push_str(&format!("\tins = append(ins, {seq})\n"));
                }
            }
            for (jp, e, w) in fields {
                match w {
                    Wire::Opt(_) => o.push_str(&format!(
                        "\tif {e} == nil {{\n\t\t{name} = append({name}, \"\\\"{jp}\\\":null\")\n\t}} else {{\n\t\t{name} = append({name}, fmt.Sprintf(\"\\\"{jp}\\\":%s\", {}))\n\t}}\n",
                        Self::go_wire(e, w)
                    )),
                    _ => o.push_str(&format!("\t{name} = append({name}, fmt.Sprintf(\"\\\"{jp}\\\":%s\", {}))\n", Self::go_wire(e, w))),
                }
            }
        };
        list("ins", &ins);
        list("obs", &obs);
        o.push_str("\trows := []string{}\n\tfor _, f := range trace {\n\t\tlbl := \"\"\n\t\tif f.Label != \"\" {\n\t\t\tlbl = fmt.Sprintf(\",\\\"label\\\":%s\", jsonStr(f.Label))\n\t\t}\n\t\trows = append(rows, fmt.Sprintf(\"{\\\"table\\\":%s,\\\"row\\\":%d%s}\", jsonStr(f.Table), f.Row, lbl))\n\t}\n");
        o.push_str("\thead := \"{\"\n\tif tag != \"\" {\n\t\thead = fmt.Sprintf(\"{\\\"tag\\\":%s,\", jsonStr(tag))\n\t}\n");
        o.push_str("\treturn fmt.Sprintf(\"%s\\\"in\\\":{%s},\\\"observed\\\":{%s},\\\"trace\\\":[%s]}\", head, joinComma(ins), joinComma(obs), joinComma(rows))\n}\n");
        o
    }

    // ── Ruby
    fn rb_wire(e: &str, w: &Wire) -> String {
        match w {
            Wire::Enum | Wire::Str => format!("_json_str({e})"),
            Wire::Bool | Wire::Int | Wire::Brand => e.to_string(),
            Wire::Date => format!("_json_str(_civil({e}))"),
            Wire::Opt(w) => format!("({e}.nil? ? \"null\" : {})", Self::rb_wire(e, w)),
        }
    }

    pub fn rb_record(&self) -> String {
        let fname = pub_name(&self.f.name);
        let single = self.f.outputs.len() == 1;
        let (out, trace, tag) = (self.temp("out"), self.temp("trace"), self.temp("tag"));
        let (ins, obs) = self.record_fields(
            |a, _| a.to_string(),
            |_, a, _| if single { out.clone() } else { format!("{out}.{a}") },
        );
        let mut params: Vec<String> = self.f.inputs.iter().map(|i| self.held(&i.name)).collect();
        for el in &self.f.elements {
            params.push(self.held(&el.name));
        }
        let mut o = String::new();
        o.push_str(
            "  def self._json_str(s)\n    o = +\"\\\"\"\n    s.each_char do |ch|\n      if ch == \"\\\"\" || ch == \"\\\\\"\n        o << \"\\\\\" << ch\n      elsif ch.ord < 32\n        o << format(\"\\\\u%04x\", ch.ord)\n      else\n        o << ch\n      end\n    end\n    o << \"\\\"\"\n  end\n\n",
        );
        if civil_needed(self.f, self.c) {
            o.push_str(
                "  def self._civil(days)\n    z = days + 719468\n    era = (z >= 0 ? z : z - 146096) / 146097\n    doe = z - era * 146097\n    yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365\n    doy = doe - (365 * yoe + yoe / 4 - yoe / 100)\n    mp = (5 * doy + 2) / 153\n    d = doy - (153 * mp + 2) / 5 + 1\n    m = mp < 10 ? mp + 3 : mp - 9\n    y = yoe + era * 400 + (m <= 2 ? 1 : 0)\n    format(\"%04d-%02d-%02d\", y, m, d)\n  end\n\n",
            );
        }
        o.push_str(&format!("  # {}\n", record_doc()));
        o.push_str(&format!(
            "  def self.{fname}_record({}{out}, {trace}, {tag} = \"\")\n",
            leading_params(&params)
        ));
        let (v_ins, v_obs, v_rows, v_head) = (self.temp("ins"), self.temp("obs"), self.temp("rows"), self.temp("head"));
        let field = |(jp, e, w): &(String, String, Wire)| format!("      \"\\\"{jp}\\\":#{{{}}}\",\n", Self::rb_wire(e, w));
        // The sequences go in first (§15.56).
        let mut seq_first = String::new();
        for el in &self.f.elements {
            let inner: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    format!(
                        "\\\"{}\\\":#{{{}}}",
                        fd.name.text,
                        Self::rb_wire(&format!("e.{}", pub_name(&fd.name)), &wire_of(&ty))
                    )
                })
                .collect();
            seq_first.push_str(&format!(
                "      \"\\\"{}\\\":[\" + {}.map {{ |e| \"{{{}}}\" }}.join(\",\") + \"]\",\n",
                el.name.text,
                self.held(&el.name),
                inner.join(",")
            ));
        }
        o.push_str(&format!("    {v_ins} = [\n{seq_first}{}    ].join(\",\")\n", ins.iter().map(field).collect::<String>()));
        o.push_str(&format!("    {v_obs} = [\n{}    ].join(\",\")\n", obs.iter().map(field).collect::<String>()));
        o.push_str(&format!(
            "    {v_rows} = {trace}.map {{ |f| lbl = f.label.to_s; \"{{\\\"table\\\":#{{_json_str(f.table)}},\\\"row\\\":#{{f.row}}#{{lbl.empty? ? \"\" : \",\\\"label\\\":#{{_json_str(lbl)}}\"}}}}\" }}.join(\",\")\n"
        ));
        o.push_str(&format!("    {v_head} = {tag}.empty? ? \"{{\" : \"{{\\\"tag\\\":#{{_json_str({tag})}},\"\n"));
        o.push_str(&format!(
            "    \"#{{{v_head}}}\\\"in\\\":{{#{{{v_ins}}}}},\\\"observed\\\":{{#{{{v_obs}}}}},\\\"trace\\\":[#{{{v_rows}}}]}}\"\n  end\n"
        ));
        o
    }

    // ── Swift
    fn sw_wire(e: &str, w: &Wire) -> String {
        match w {
            Wire::Enum => format!("_jsonStr({e}.rawValue)"),
            Wire::Bool | Wire::Int => format!("String({e})"),
            Wire::Brand => format!("String({e}.value)"),
            Wire::Date => format!("_jsonStr(_civil({e}))"),
            Wire::Str => format!("_jsonStr({e})"),
            Wire::Opt(w) => format!("({e}.map {{ {} }} ?? \"null\")", Self::sw_wire("$0", w)),
        }
    }

    pub fn sw_record(&self) -> String {
        let fname = sw_name(&pub_name(&self.f.name));
        let single = self.f.outputs.len() == 1;
        let (out, trace, tag) = (self.temp("out"), self.temp("trace"), self.temp("tag"));
        let (ins, obs) = self.record_fields(
            |a, _| sw_name(a),
            |_, a, _| if single { sw_name(&out) } else { format!("{}.{}", sw_name(&out), sw_name(a)) },
        );
        let ret = if single { self.sw_ty(&self.ty_of(&self.f.outputs[0].name.text)) } else { "Output".into() };
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| self.sw_param(&i.name, &self.sw_ty(&self.ty_of(&i.name.text))))
            .collect();
        for (s, el) in self.f.elements.iter().enumerate() {
            params.push(self.sw_param(&el.name, &format!("[{}]", self.element_type(s))));
        }
        let mut o = String::new();
        o.push_str(
            "func _jsonStr(_ s: String) -> String {\n    var o = \"\\\"\"\n    for u in s.unicodeScalars {\n        if u == \"\\\"\" {\n            o += \"\\\\\\\"\"\n        } else if u == \"\\\\\" {\n            o += \"\\\\\\\\\"\n        } else if u.value < 32 {\n            let h = String(u.value, radix: 16)\n            o += \"\\\\u\" + String(repeating: \"0\", count: 4 - h.count) + h\n        } else {\n            o.unicodeScalars.append(u)\n        }\n    }\n    return o + \"\\\"\"\n}\n\n",
        );
        if civil_needed(self.f, self.c) {
            o.push_str(
                "func _pad(_ v: Int64, _ w: Int) -> String {\n    let s = String(v)\n    return s.count >= w ? s : String(repeating: \"0\", count: w - s.count) + s\n}\n\nfunc _civil(_ days: Int64) -> String {\n    let z = days + 719468\n    let era = (z >= 0 ? z : z - 146096) / 146097\n    let doe = z - era * 146097\n    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365\n    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100)\n    let mp = (5 * doy + 2) / 153\n    let d = doy - (153 * mp + 2) / 5 + 1\n    let m = mp < 10 ? mp + 3 : mp - 9\n    let y = yoe + era * 400 + (m <= 2 ? 1 : 0)\n    return _pad(y, 4) + \"-\" + _pad(m, 2) + \"-\" + _pad(d, 2)\n}\n\n",
            );
        }
        o.push_str(&format!("/// {}\n", record_doc()));
        o.push_str(&format!(
            "public func {fname}Record({}{}: {ret}, {}: [Fired], {}: String = \"\") -> String {{\n",
            leading_params(&params),
            sw_name(&out),
            sw_name(&trace),
            sw_name(&tag)
        ));
        let (v_ins, v_obs, v_rows, v_head) = (self.temp("ins"), self.temp("obs"), self.temp("rows"), self.temp("head"));
        let field = |(jp, e, w): &(String, String, Wire)| format!("        \"\\\"{jp}\\\":\\({})\",\n", Self::sw_wire(e, w));
        // The sequence goes in first, and one element is written the way one record's inputs
        // are: the same wire, one level in (§15.56).
        let mut seq_first = String::new();
        for (s, el) in self.f.elements.iter().enumerate() {
            let seq = sw_name(&if s == 0 { self.temp("seq") } else { self.temp(&format!("seq{}", s + 1)) });
            let inner: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    format!(
                        "\\\"{}\\\":\\({})",
                        fd.name.text,
                        Self::sw_wire(&format!("$0.{}", sw_name(&pub_name(&fd.name))), &wire_of(&ty))
                    )
                })
                .collect();
            o.push_str(&format!(
                "    let {seq} = \"\\\"{}\\\":[\" + {}.map {{ \"{{{}}}\" }}.joined(separator: \",\") + \"]\"\n",
                el.name.text,
                sw_name(&self.held(&el.name)),
                inner.join(",")
            ));
            seq_first.push_str(&format!("        {seq},\n"));
        }
        o.push_str(&format!("    let {v_ins} = [\n{seq_first}{}    ].joined(separator: \",\")\n", ins.iter().map(field).collect::<String>()));
        o.push_str(&format!("    let {v_obs} = [\n{}    ].joined(separator: \",\")\n", obs.iter().map(field).collect::<String>()));
        o.push_str(&format!(
            "    let {v_rows} = {}.map {{ f in\n        let l = f.label.isEmpty ? \"\" : \",\\\"label\\\":\" + _jsonStr(f.label)\n        return \"{{\\\"table\\\":\\(_jsonStr(f.table)),\\\"row\\\":\\(f.row)\\(l)}}\"\n    }}.joined(separator: \",\")\n",
            sw_name(&trace)
        ));
        o.push_str(&format!(
            "    let {v_head} = {}.isEmpty ? \"{{\" : \"{{\\\"tag\\\":\\(_jsonStr({})),\"\n",
            sw_name(&tag),
            sw_name(&tag)
        ));
        o.push_str(&format!(
            "    return \"\\({v_head})\\\"in\\\":{{\\({v_ins})}},\\\"observed\\\":{{\\({v_obs})}},\\\"trace\\\":[\\({v_rows})]}}\"\n}}\n"
        ));
        o
    }
}


// ---------------------------------------------------------------------------
// The ends a door holds a number or a date to (§15.295), as each language tests them.
// ---------------------------------------------------------------------------

/// What a Python door refuses a value past its ends with (`crate::door::ends`): `not lo <= v <= hi`
/// where there are both, as it always read, and the one comparison where there is one. `None`
/// where there is nothing to compare. `v` is the integer as it travels.
pub(crate) fn py_out_of(v: &str, (lo, hi): (Option<i128>, Option<i128>)) -> Option<String> {
    match (lo, hi) {
        (Some(l), Some(h)) => Some(format!("not {l} <= {v} <= {h}")),
        (Some(l), None) => Some(format!("{v} < {l}")),
        (None, Some(h)) => Some(format!("{v} > {h}")),
        (None, None) => None,
    }
}

/// The ends of int64, which a reader of the wire holds a whole number to as it takes it (§15.328):
/// a number past them is refused in the first look, before any value is held to its range.
pub(crate) const INT64_ENDS: (Option<i128>, Option<i128>) = (Some(i64::MIN as i128), Some(i64::MAX as i128));

/// The test of the first look for a number of a language whose integers go past int64, in Python.
pub(crate) fn py_past_int64(v: &str) -> String {
    py_out_of(v, INT64_ENDS).unwrap_or_default()
}

/// The same test where a language writes `||` between two comparisons and may put a suffix on an
/// integer literal: `n` for TypeScript's `bigint`, `L` for Java's `long`, nothing for Go, Rust,
/// Swift and PHP.
pub(crate) fn or_out_of(v: &str, (lo, hi): (Option<i128>, Option<i128>), suffix: &str) -> Option<String> {
    match (lo, hi) {
        (Some(l), Some(h)) => Some(format!("{v} < {l}{suffix} || {v} > {h}{suffix}")),
        (Some(l), None) => Some(format!("{v} < {l}{suffix}")),
        (None, Some(h)) => Some(format!("{v} > {h}{suffix}")),
        (None, None) => None,
    }
}

/// The same for the end of a Ruby `raise … ` line: `unless (lo..hi).cover?(v)` where there are both
/// ends, as it always read, and `if` with the one comparison where there is one.
fn rb_out_of(v: &str, (lo, hi): (Option<i128>, Option<i128>)) -> Option<String> {
    match (lo, hi) {
        (Some(l), Some(h)) => Some(format!("unless ({l}..{h}).cover?({v})")),
        (Some(l), None) => Some(format!("if {v} < {l}")),
        (None, Some(h)) => Some(format!("if {v} > {h}")),
        (None, None) => None,
    }
}

// ---------------------------------------------------------------------------
// JavaScript (§15.36): the TypeScript with its types taken off.
// ---------------------------------------------------------------------------

/// The generated TypeScript is erasable syntax by design (§8.3), so the JavaScript target
/// is the same file with the annotations removed: the same branches, the same helpers, held
/// to the same vectors by `rulec test`. A transform rather than a second emitter, because
/// two emitters of the same code drift apart; the agreement check is what keeps the
/// transform honest.
pub fn strip_types(ts: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut in_interface = false;
    for line in ts.lines() {
        if in_interface {
            if line == "}" {
                in_interface = false;
            }
            continue;
        }
        let t = line.trim_start();
        // A type alias, exported or not: the projection function declares two of its own
        // for the caller's object, which JavaScript neither needs nor can parse (§15.125).
        if t.starts_with("export type ") || t.starts_with("import type ") || t.starts_with("type ") {
            continue;
        }
        if t.starts_with("export interface ") {
            in_interface = true;
            continue;
        }
        if is_field_decl(t) {
            continue;
        }
        // Comment lines carry prose, and prose says "as" too. An import's `as` is a
        // binding, not a cast, and stays, as a re-export's does (`export { sizeLabel as
        // size2Label } from`, §15.326).
        if t.starts_with("//") || t.starts_with("/*") || t.starts_with("* ") || t == "*/" || t.starts_with("import ") || t.starts_with("export {") {
            out.push(line.to_string());
            continue;
        }
        let mut l = line.replace(" as const;", ";");
        l = strip_signature(&l);
        l = strip_declaration(&l);
        l = strip_casts(&l);
        out.push(l);
    }
    out.join("\n") + "\n"
}

/// `readonly what: string;` — a class property declared and not assigned. TypeScript needs
/// the line; JavaScript gets the property from the constructor, so the line is dropped
/// rather than rewritten. An object literal's `k: v,` ends in a comma and is left alone.
fn is_field_decl(t: &str) -> bool {
    let Some(rest) = t.strip_suffix(';') else { return false };
    let rest = rest.strip_prefix("readonly ").unwrap_or(rest);
    let Some((name, ty)) = rest.split_once(": ") else { return false };
    !name.is_empty()
        && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        && !ty.is_empty()
        && !ty.contains('=')
        && !ty.contains('(')
}

/// `function f(a: T, b: U): R {` → `function f(a, b) {`. Also a class constructor.
fn strip_signature(l: &str) -> String {
    let t = l.trim_start();
    if !(t.contains("function ") || t.starts_with("constructor(")) {
        return l.to_string();
    }
    let Some(open) = l.find('(') else { return l.to_string() };
    let Some(close) = l.rfind(')') else { return l.to_string() };
    if close < open {
        return l.to_string();
    }
    let params: Vec<String> = split_params(&l[open + 1..close])
        .into_iter()
        .filter(|p| !p.is_empty())
        .map(|p| match p.find(": ") {
            // A default value outlives its annotation: `value: unknown = NO_VALUE` keeps
            // the `= NO_VALUE`, or JavaScript would see a different sentinel (§15.95).
            Some(i) => match p[i..].find(" = ") {
                Some(j) => format!("{}{}", &p[..i], &p[i + j..]),
                None => p[..i].to_string(),
            },
            None => p.to_string(),
        })
        .collect();
    let mut rest = &l[close + 1..];
    if let Some(r) = rest.strip_prefix(": ") {
        // The return type runs up to the block's brace.
        rest = match r.rfind(" {") {
            Some(i) => &r[i..],
            None => "",
        };
    }
    format!("{}({}){}", &l[..open], params.join(", "), rest)
}

/// `let x: T;`, `const x: T = …` → without the annotation. Destructuring has none.
/// A parameter list, cut at the commas that separate parameters — not at the ones inside a
/// type. `Record<string, string>` is one type and not two parameters.
fn split_params(list: &str) -> Vec<&str> {
    let (mut depth, mut start) = (0i32, 0usize);
    let mut out = Vec::new();
    for (i, c) in list.char_indices() {
        match c {
            '<' | '(' | '[' | '{' => depth += 1,
            '>' | ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                out.push(list[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(list[start..].trim());
    out
}

fn strip_declaration(l: &str) -> String {
    let t0 = l.trim_start();
    let indent = &l[..l.len() - t0.len()];
    // `export const INITIAL: State = …` (§15.148) is a declaration like any other.
    let (export, t) = match t0.strip_prefix("export ") {
        Some(r) => ("export ", r),
        None => ("", t0),
    };
    let Some(rest) = t.strip_prefix("let ").or_else(|| t.strip_prefix("const ")) else {
        return l.to_string();
    };
    let kw = if t.starts_with("let ") { "let " } else { "const " };
    let kw = format!("{export}{kw}");
    let name_end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(rest.len());
    if name_end == 0 {
        return l.to_string();
    }
    let after = &rest[name_end..];
    let Some(a) = after.strip_prefix(": ") else { return l.to_string() };
    // The annotation ends at the assignment or at the semicolon, whichever comes first.
    let end = match (a.find(" = "), a.find(';')) {
        (Some(i), Some(j)) => i.min(j),
        (Some(i), None) => i,
        (None, Some(j)) => j,
        (None, None) => return l.to_string(),
    };
    format!("{indent}{kw}{}{}", &rest[..name_end], &a[end..])
}

/// ` as YenInclTax`, ` as number`, ` as Record<string, unknown>` → gone.
fn strip_casts(l: &str) -> String {
    let mut o = String::new();
    let mut rest = l;
    while let Some(i) = rest.find(" as ") {
        o.push_str(&rest[..i]);
        let after = &rest[i + 4..];
        let mut n = after.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(after.len());
        if n == 0 {
            // Not a cast (nothing that looks like a type follows); keep the text.
            o.push_str(" as ");
            rest = after;
            continue;
        }
        if after[n..].starts_with('<') {
            if let Some(k) = after[n..].find('>') {
                n += k + 1;
            }
        }
        // An array type (`as unknown[]`) is one more pair of brackets on the end of the name.
        while after[n..].starts_with("[]") {
            n += 2;
        }
        // A type that may be absent (`as Tier | null`, §15.342).
        while after[n..].starts_with(" | null") {
            n += " | null".len();
        }
        rest = &after[n..];
    }
    o.push_str(rest);
    o
}

impl<'a> Gen<'a> {
    /// The JavaScript module: the TypeScript one without its types (§15.36).
    pub fn javascript(&self) -> String {
        // An `enums` file's module is imported as the `.mjs` beside this one (§15.212).
        strip_types(&self.typescript()).replace(".ts\";", ".mjs\";")
    }

    /// The runner, likewise. The import points at the `.mjs` beside it.
    pub fn js_runner(&self) -> String {
        strip_types(&self.ts_runner()).replace(".ts\";", ".mjs\";")
    }
}

pub fn round_tests_javascript(u: &Uses) -> String {
    strip_types(&round_tests_typescript(u))
}

// The projection function (§15.125): the caller's object in, the rule's answer out.
// ---------------------------------------------------------------------------
//
// The glue between a caller's domain object and the rule's flat inputs is otherwise written
// by hand, once per input per language, and nothing checks it. Here it is generated, and the
// paths it walks were held to the contract by `check` before anything was written.
//
// **Five of the twelve targets get it, and the line is the caller's object.** Python,
// TypeScript, JavaScript, Ruby and PHP hold it as a plain map, so the projection needs no
// name for it. Go, Swift, Java and Rust hold it as a type, and the only ways to name that
// type are to generate it — which is the domain object model §15-6 rules out — or to follow
// the caller's own, which §15.63 and §15.73 both rejected. SQL takes a relation of flat
// columns, NumPy takes columns, and the Wasm ABI takes one JSON object of the rule's own
// inputs; none of the three has an object to project from.

/// One input and where the caller's object holds it.
pub(crate) struct Proj<'x> {
    /// The input, as this rule's own name for it.
    pub(crate) jp: String,
    pub(crate) ty: Ty,
    /// The shape's alias, which is the parameter the projection takes.
    pub(crate) root: String,
    pub(crate) path: Vec<String>,
    pub(crate) kind: &'x crate::ast::ProjKind,
    /// How the path is read when the shape is a `.proto`: its JSON form (§15.133).
    pub(crate) proto: Option<crate::projection::ProtoPath>,
    /// The path as the rule writes it, which a refusal names (§15.234): `order.shipping.zone`,
    /// `order.lines` for a walk, `order.lines[1].amount_usd` for a field of an element, the index
    /// left open (`door::HERE`) for the generated code to put in (§15.324).
    pub(crate) shown: String,
    /// What the contract carries at the field a `where` tests (§15.234).
    pub(crate) test_kind: Option<crate::projection::At>,
}

/// A sequence the projection function builds out of the caller's object (§15.223): which of the
/// rule's sequences it is, the collection it is taken from, and each field of an element read
/// from one element — a [`Proj`] whose root is the name the language's loop gives the element.
pub(crate) struct SeqProj<'x> {
    pub(crate) s: usize,
    /// The shape's alias, which is the parameter the projection takes.
    pub(crate) root: String,
    pub(crate) path: Vec<String>,
    /// How the collection is read when the shape is a `.proto`: its JSON form (§15.133).
    pub(crate) proto: Option<crate::projection::ProtoPath>,
    pub(crate) fields: Vec<Proj<'x>>,
    /// The collection's path as the rule writes it: `order.lines` (§15.234).
    pub(crate) shown: String,
}

impl<'a> Gen<'a> {
    /// How each path is read from a `.proto` shape, by the name it is read into.
    fn proto_reads(&self) -> std::collections::HashMap<String, crate::projection::ProtoPath> {
        if self.f.shapes.iter().any(|d| d.source == crate::ast::EnumSource::Proto) {
            crate::projection::proto_paths(self.f, &self.path)
        } else {
            Default::default()
        }
    }

    /// The parameter a shape is passed as: its public name.
    fn shape_param(&self, root: &str) -> String {
        self.f.shapes.iter().find(|d| d.name.text == root).map(pub_name_of_shape).unwrap_or_else(|| root.to_string())
    }

    /// The projections of this rule, in the order the inputs are declared.
    pub(crate) fn projections(&self) -> Vec<Proj<'_>> {
        let reads = self.proto_reads();
        // What a `where` reads is held to what the contract carries there (§15.234).
        let kinds = if self.f.inputs.iter().any(|i| i.from.as_ref().is_some_and(|p| p.kind.test().is_some())) {
            crate::projection::where_kinds(self.f, &self.path)
        } else {
            Default::default()
        };
        self.f
            .inputs
            .iter()
            .filter_map(|i| {
                let p = i.from.as_ref()?;
                Some(Proj {
                    jp: i.name.text.clone(),
                    ty: self.ty_of(&i.name.text),
                    root: self.shape_param(&p.root.text),
                    path: p.path.iter().map(|n| n.text.clone()).collect(),
                    kind: &p.kind,
                    proto: reads.get(&i.name.text).cloned(),
                    shown: std::iter::once(p.root.text.clone()).chain(p.path.iter().map(|n| n.text.clone())).collect::<Vec<_>>().join("."),
                    test_kind: kinds.get(&i.name.text).cloned(),
                })
            })
            .collect()
    }

    /// The sequences taken from the caller's object (§15.223), in the order they are declared,
    /// with their fields read from one element called `elem`.
    pub(crate) fn seq_projections(&self, elem: &str) -> Vec<SeqProj<'_>> {
        let reads = self.proto_reads();
        self.f
            .elements
            .iter()
            .enumerate()
            .filter_map(|(s, el)| {
                let sp = el.from.as_ref()?;
                let shown = std::iter::once(sp.root.text.clone()).chain(sp.path_steps()).collect::<Vec<_>>().join(".");
                let fields = el
                    .fields
                    .iter()
                    .filter_map(|fd| {
                        let p = fd.from.as_ref()?;
                        Some(Proj {
                            jp: fd.name.text.clone(),
                            ty: self.ty_of(&fd.name.text),
                            root: elem.to_string(),
                            path: p.steps(),
                            kind: &p.kind,
                            proto: reads.get(&fd.name.text).cloned(),
                            // The index of the element is left open for the generated code to put
                            // in (§15.324): `order.lines[1].quantity`.
                            shown: format!("{shown}[{}].{}", crate::door::HERE, p.steps().join(".")),
                            test_kind: None,
                        })
                    })
                    .collect();
                Some(SeqProj { s, root: self.shape_param(&sp.root.text), path: sp.path_steps(), proto: reads.get(&el.name.text).cloned(), fields, shown })
            })
            .collect()
    }

    /// The parameters the projection function takes: one per shape some input or sequence is
    /// projected from, in the order the shapes are declared. A rule with no projection has none,
    /// and no projection function is written.
    pub(crate) fn proj_params(&self) -> Vec<String> {
        let roots = |p: &Option<crate::ast::Projection>, d: &crate::ast::ShapeDecl| p.as_ref().is_some_and(|p| p.root.text == d.name.text);
        self.f
            .shapes
            .iter()
            .filter(|d| self.f.inputs.iter().any(|i| roots(&i.from, d)) || self.f.elements.iter().any(|e| roots(&e.from, d)))
            // under the identifier it is bound under apart from its name, where it has one (§15.336)
            .map(|d| self.binds.get(&d.name.text).cloned().unwrap_or_else(|| pub_name_of_shape(d)))
            .collect()
    }

    /// The sequences the caller still passes as they are, by their index: the ones no `from`
    /// takes out of the object (§15.223).
    pub(crate) fn proj_seq_rest(&self) -> Vec<usize> {
        self.f.elements.iter().enumerate().filter(|(_, el)| el.from.is_none()).map(|(s, _)| s).collect()
    }

    /// Whether this rule projects at all. Every emitter asks before writing anything.
    pub fn projects(&self) -> bool {
        self.f.inputs.iter().any(|i| i.from.is_some()) || self.f.elements.iter().any(|e| e.from.is_some())
    }

    /// What `max` or `min` gives when no element passes, as the integer the rule takes: a number
    /// at the input's wire scale, a date as its day number (§15.223).
    pub(crate) fn proj_empty(&self, p: &Proj) -> String {
        let v = match (p.kind.empty(), p.ty.present()) {
            (Some((Lit::Num(n), _)), t) => crate::types::lit_value_in_pub(n, t).map(|v| crate::types::wire_int(v, self.c.wire_scale(&p.jp))),
            (Some((Lit::Date(y, m, d), _)), _) => {
                let r = crate::types::date_ord(*y, *m, *d);
                Some(r.num / r.den)
            }
            _ => None,
        };
        v.unwrap_or(0).to_string()
    }

    /// The name of the helper that picks the largest or the smallest value (§15.223).
    pub(crate) fn extreme_helper(kind: &crate::ast::ProjKind) -> &'static str {
        if matches!(kind, crate::ast::ProjKind::Max(..)) { "_largest" } else { "_smallest" }
    }

    /// The docstring every language puts on the projection function, in one place so the
    /// five say the same thing.
    pub(crate) fn proj_doc(&self) -> String {
        // A rule that takes a sequence out of the object says so (§15.223); every other keeps the
        // sentence it always had.
        if self.f.elements.iter().any(|e| e.from.is_some()) {
            return tr!(
                "呼び出し側のオブジェクトから入力と並びを取り出して、この規則を呼びます。パスは宣言した契約に照らして検査済みです。",
                "Reads the inputs and the sequences out of the caller's object and calls this rule. The paths were held to the declared contract before this was written."
            );
        }
        tr!(
            "呼び出し側のオブジェクトから入力を取り出して、この規則を呼びます。パスは宣言した契約に照らして検査済みです。",
            "Reads the inputs out of the caller's object and calls this rule. The paths were held to the declared contract before this was written."
        )
    }
}

/// A shape's public name: its ASCII alias where it has one, its own name otherwise (§1.3).
fn pub_name_of_shape(d: &crate::ast::ShapeDecl) -> String {
    d.name.ascii.clone().unwrap_or_else(|| d.name.text.clone())
}

/// A literal of a `where` test, as the raw value the contract carries. A contract has no
/// units, which is why `check` refuses a united literal here: comparing a scaled number with
/// a raw one is the one thing this must not do quietly.
fn raw_lit(l: &Lit, tru: &str, fls: &str) -> String {
    match l {
        Lit::Str(s) => format!("{s:?}"),
        Lit::Word(w) if w == crate::kw::TRUE => tru.to_string(),
        Lit::Word(w) if w == crate::kw::FALSE => fls.to_string(),
        Lit::Word(w) => format!("{w:?}"),
        Lit::Num(n) => n.raw.clone(),
        // A date travels as `YYYY-MM-DD`, and in that form the strings order as the dates do.
        Lit::Date(y, m, d) => format!("\"{y:04}-{m:02}-{d:02}\""),
    }
}

/// Whether every number a `where` cell names is whole, so that a BigInt can be compared with
/// each of them for equality as well as for order.
fn cell_whole(c: &Cell) -> bool {
    let whole = |l: &Lit| match l {
        Lit::Num(n) => n.raw.chars().all(|ch| ch.is_ascii_digit() || ch == '-'),
        _ => true,
    };
    match c {
        Cell::Lit(l) => whole(l),
        Cell::Set(ls) | Cell::Not(ls) => ls.iter().all(whole),
        Cell::Cmp(ops) => ops.iter().all(|(_, l)| whole(l)),
        _ => true,
    }
}

/// What a `.proto` field reads as when protojson leaves it out, in one language's spelling
/// (§15.133).
pub(crate) fn pb_zero(z: &crate::projection::Zero, fls: &str, quote: &dyn Fn(&str) -> String) -> String {
    match z {
        crate::projection::Zero::Int | crate::projection::Zero::Frac => "0".to_string(),
        crate::projection::Zero::Str => quote(""),
        crate::projection::Zero::Bool => fls.to_string(),
        crate::projection::Zero::List => "[]".to_string(),
        crate::projection::Zero::Enum(v) => quote(v),
    }
}

/// How one language spells the pieces a `where` test is made of. What differs between the
/// five is equality, conjunction, the two booleans, and whether a boolean is compared or
/// simply read.
pub(crate) struct Syn<'s> {
    pub is: &'s str,
    pub isnt: &'s str,
    pub and: &'s str,
    pub or: &'s str,
    pub tru: &'s str,
    /// How a language spells "not", when comparing with a boolean literal is a lint there
    /// rather than an expression. Python's `== True` is E712; every other target writes the
    /// comparison out.
    pub not: Option<&'s str>,
}

/// A `where` test on one element, in the spelling one language asks for, with the literals spelled
/// by the caller: a JavaScript BigInt is compared with `100n`, not `100` (§15.133), and a date with
/// its day number (§15.234).
pub(crate) fn elem_cond_with(cell: &Cell, at: &str, y: &Syn, lit: &dyn Fn(&Lit) -> String) -> String {
    let (is, isnt, and, or, tru) = (y.is, y.isnt, y.and, y.or, y.tru);
    // A boolean field tested against `true` or `false` is the field itself, or its negation,
    // where the language would rather have it that way.
    let boolean = |l: &Lit, neg: bool| -> Option<String> {
        let not = y.not?;
        let Lit::Word(w) = l else { return None };
        let yes = match w.as_str() {
            _ if w == crate::kw::TRUE => true,
            _ if w == crate::kw::FALSE => false,
            _ => return None,
        };
        Some(if yes == !neg { at.to_string() } else { format!("{not}{at}") })
    };
    match cell {
        Cell::DontCare => tru.to_string(),
        Cell::Lit(l) => boolean(l, false).unwrap_or_else(|| format!("{at} {is} {}", lit(l))),
        Cell::Set(ls) => ls
            .iter()
            .map(|l| boolean(l, false).unwrap_or_else(|| format!("{at} {is} {}", lit(l))))
            .collect::<Vec<_>>()
            .join(&format!(" {or} ")),
        Cell::Not(ls) => ls
            .iter()
            .map(|l| boolean(l, true).unwrap_or_else(|| format!("{at} {isnt} {}", lit(l))))
            .collect::<Vec<_>>()
            .join(&format!(" {and} ")),
        Cell::Cmp(ops) => ops
            .iter()
            .map(|(o, l)| format!("{at} {} {}", o.word(), lit(l)))
            .collect::<Vec<_>>()
            .join(&format!(" {and} ")),
        // `starts_with` and `none` do not reach here: `check` takes only the tests a
        // contract's own kinds can answer.
        _ => tru.to_string(),
    }
}

impl<'a> Gen<'a> {
    /// The two defaults a `.proto` read passes to `_proto`, in one language's spelling: what the
    /// value reads as when it is missing, and when a message on the way to it is. An optional
    /// input reads as none where the contract gives presence — an unset message, an `optional`
    /// field — and as the default where it does not, because a proto3 field without presence
    /// that is missing *is* its default (§15.133).
    fn proto_defaults(&self, p: &Proj, pp: &crate::projection::ProtoPath, lit: &dyn Fn(&crate::projection::Zero) -> String, none: &str) -> (String, String) {
        let opt = matches!(p.ty, Ty::Opt(_));
        let zero = lit(&pp.zero);
        match (matches!(p.kind, crate::ast::ProjKind::Field), opt) {
            (true, true) => (if pp.leaf_optional { none.to_string() } else { zero }, none.to_string()),
            (true, false) => (zero.clone(), zero),
            // A walk: a missing collection, or one behind a missing message, has no elements.
            (false, _) => (zero.clone(), zero),
        }
    }

    /// The inputs a projection does not reach, which the projection function takes as it
    /// is: a rule may take some of its inputs from the caller's object and the rest from
    /// the caller.
    pub(crate) fn proj_rest(&self) -> Vec<&crate::ast::VarDecl> {
        self.f.inputs.iter().filter(|i| i.from.is_none()).collect()
    }

    /// The parameters of the Python projection function, with their types: the objects, the
    /// inputs no `from` reaches, and the sequences no `from` takes out of an object (§15.223).
    fn py_from_params(&self) -> Vec<String> {
        let mut params: Vec<String> = self.proj_params().iter().map(|r| format!("{r}: dict[str, Any]")).collect();
        params.extend(self.proj_rest().iter().map(|i| format!("{}: {}", pub_name(&i.name), self.py_ty(&self.ty_of(&i.name.text)))));
        for s in self.proj_seq_rest() {
            params.push(format!("{}: list[{}]", pub_name(&self.f.elements[s].name), self.element_type(s)));
        }
        params
    }

    /// The public name of a field of an element, by the rule's name for it.
    fn elem_field_alias(&self, s: usize, jp: &str) -> String {
        self.f.elements[s].fields.iter().find(|fd| fd.name.text == jp).map(|fd| pub_name(&fd.name)).unwrap_or_else(|| jp.to_string())
    }

    /// The return type the projection function repeats, so its signature reads like the
    /// rule's own.
    fn py_ret(&self) -> String {
        match self.f.outputs.len() {
            1 => self.py_ty(&self.ty_of(&self.f.outputs[0].name.text)),
            _ => "Output".into(),
        }
    }
}

impl<'a> Gen<'a> {
    /// The parameters of the TypeScript projection function, with their types, in the order
    /// `py_from_params` gives them (§15.223).
    fn ts_from_params(&self) -> Vec<String> {
        let mut params: Vec<String> = self.proj_params().iter().map(|r| format!("{r}: _Obj")).collect();
        params.extend(self.proj_rest().iter().map(|i| format!("{}: {}", self.held(&i.name), self.ts_ty(&self.ty_of(&i.name.text)))));
        for s in self.proj_seq_rest() {
            params.push(format!("{}: {}[]", self.held(&self.f.elements[s].name), self.element_type(s)));
        }
        params
    }

    /// The parameters with no types, as JavaScript and Ruby write them.
    fn plain_from_params(&self) -> Vec<String> {
        let mut params: Vec<String> = self.proj_params();
        params.extend(self.proj_rest().iter().map(|i| self.held(&i.name)));
        params.extend(self.proj_seq_rest().into_iter().map(|s| self.held(&self.f.elements[s].name)));
        params
    }
}

/// `rulec gen`: §8.4, the generated files are committed to git, and `--check` in CI verifies that
/// they match a fresh generation. What it prints goes to `out` and `err`, and the answer is its exit
/// code. A program that holds rulec as a library and needs a rule's generated code calls this, as
/// the command does (dandori's tests build their workflows against it, ritsu's DESIGN 3.3).
/// The languages a package of ritsu's is written in (ritsu's DESIGN 9.3), as `ritsu gen --target`
/// names them.
pub const PACKAGE_TARGETS: [&str; 4] = ["typescript", "python", "go", "rust"];

/// The head of a rule's module of Rust in a package of ritsu's (ritsu's DESIGN 9.3): the lints of
/// `rulec gen`'s module, and those a module of a library crate beside the other rules' meets. The
/// helpers every module holds (the five ways of rounding, `min_i64`) are not each called by
/// every rule; the shapes of the helpers and of the tables are clippy's lints the same in every
/// rule, written for the proof of §8.3 to read, so the package's `rules.rs` allows clippy's lints on
/// each rule's module. The lints of the names' letters are the crate's, which a module's own allowing
/// does not reach (`uncommon_codepoints`): the package's `lib.rs` allows them.
const RS_PACKAGE_ALLOW: &str = "\n#![allow(non_snake_case, unused_parens, dead_code)]\n\n";

/// The module of a rule in TypeScript, Python, Go or Rust as a package of ritsu's holds it (ritsu's
/// DESIGN 9.3): the file `rulec gen` writes as the rule's library (not its runner, MCP server, page,
/// `.proto` or vectors), at its path under the package's `rules/`, its head naming the rule's file
/// as `shown` (its path from the project's root). The rule is read from `src`, which was read from
/// `path`; nothing is made from a rule that does not pass check, and the error says so.
pub fn package_module(src: &str, path: &str, shown: &str, target: &str) -> Result<(String, String), String> {
    package_module_in(src, path, shown, target, "")
}

/// [`package_module`], for a package whose Go import path is `go_module`: a rule's module reads
/// the types of an `enums` file from that file's module beside it in the package's `rules/`, and
/// an `enums` file is that module (§15.212).
pub fn package_module_in(src: &str, path: &str, shown: &str, target: &str, go_module: &str) -> Result<(String, String), String> {
    package_parts(src, path, shown, target, go_module).map(|(rel, body, _)| (rel, body))
}

/// [`package_module_in`], with the units whose types the module takes from the package's `units`
/// module (§15.248), for the package to declare them there: money and the quantities, as the rule
/// writes them. An `enums` file takes none.
pub fn package_parts(src: &str, path: &str, shown: &str, target: &str, go_module: &str) -> Result<(String, String, Vec<ritsu_units::Unit>), String> {
    package_parts_at(src, path, shown, target, go_module, ritsu_emit::package::Place::here())
}

/// [`package_parts`], the module put at `place` in the package (ritsu's DESIGN 17.8): a rule of a
/// dependency goes under `deps/<name>/`, and reads the package's `units` and the modules of the
/// `enums` files it reads from where they are. The path it returns is under the rules' directory of
/// its module, as for the module at hand.
pub fn package_parts_at(src: &str, path: &str, shown: &str, target: &str, go_module: &str, place: ritsu_emit::package::Place) -> Result<(String, String, Vec<ritsu_units::Unit>), String> {
    let rep = crate::report(src, path);
    if crate::has_error(&rep.diags) {
        return Err(tr!("`{path}` は検査を通っていないので生成しません", "`{path}` does not pass check, so nothing is generated"));
    }
    let (f, c) = crate::prepare(src, path).map_err(|_| tr!("`{path}` は検査を通っていないので生成しません", "`{path}` does not pass check, so nothing is generated"))?;
    package_parts_of(&f, &c, src, path, shown, target, go_module, place)
}

/// [`package_parts_at`], of a rule already checked and prepared (`f`, `c`, of the text `src` at
/// `path`): what `ritsu gen` writes from the check the ports kept, so that the rule is checked once
/// in the run (ritsu's DESIGN 4.15).
#[allow(clippy::too_many_arguments)]
pub fn package_parts_of(f: &RuleFile, c: &Checked, src: &str, path: &str, shown: &str, target: &str, go_module: &str, place: ritsu_emit::package::Place) -> Result<(String, String, Vec<ritsu_units::Unit>), String> {
    let g = Gen::new(f, c, src, path).shown(shown).in_package(go_module).at(place);
    if f.kind == FileKind::Enums {
        let (rel, body) = g.enums_module_for(target).ok_or_else(|| format!("a package is written in {}, not in {target}", PACKAGE_TARGETS.join(", ")))?;
        return Ok((rel, body, Vec::new()));
    }
    let alias = f.name.ascii.clone().unwrap_or_else(|| f.name.text.clone());
    // each enum under the type it takes in the language (§15.326)
    let here = g.for_target(target);
    let g = here.as_ref().unwrap_or(&g);
    let (rel, body) = match target {
        "typescript" => (format!("{alias}.ts"), g.typescript()),
        "python" => (format!("{alias}.py"), g.python()),
        "go" => (format!("{}/{alias}.go", crate::backend::go_package(&alias)), g.go()),
        "rust" => (format!("{alias}.rs"), g.rust()),
        _ => return Err(format!("a package is written in {}, not in {target}", PACKAGE_TARGETS.join(", "))),
    };
    Ok((rel, body, g.package_units()))
}

/// The `units` module of a package of ritsu's (§15.248, ritsu's DESIGN 9.3), at its path from the
/// package's directory: one brand of each unit its rules and its books take amounts in, as §8.2
/// brands an amount (a branded bigint, `NewType`, a defined int64 type). The modules of the rules
/// and the clients of the books name these, so that the type checker keeps an amount of one unit
/// from where another is taken: a rule's output given to a book's call, or to another rule. A unit's
/// brand is named by the spelling the table gives the unit first (`円` for `JPY`, `cm3` for `mL`); a
/// spelling the package also writes it in is an alias of that brand, the same type under the name
/// its files give it. None when nothing in the package takes such an amount, or for a language a
/// package is not written in.
pub fn units_module(target: &str, units: &[ritsu_units::Unit]) -> Option<(String, String)> {
    units_module_in(target, units, &BTreeMap::new())
}

/// [`units_module`], for a package written as a library (ritsu's DESIGN 17.16). The brand of a unit
/// named in `shared` is the brand of the library that holds the unit: the module reads it from that
/// library's `units` module (the path in `shared`, `@acme/pricing/units`, `acme_pricing.units`,
/// `example.com/acme/pricing-go/units`) and hands it on under the same name, so that an amount the
/// library's code gives is an amount this package's code takes, in Python and Go as in TypeScript.
/// The brand's name is the same in every package, the unit as the table spells it first. The other
/// spellings stay aliases here.
///
/// Rust's `units` is a module of the package's crate: a brand is a newtype of an i64 that converts
/// into an i64 (`From`), as the clients of the books read it, and another spelling is the brand
/// under another name (`pub use`, which names its constructor too). A package of Rust is no
/// library (ritsu's DESIGN 17.16), so `shared` is not read for it.
pub fn units_module_in(target: &str, units: &[ritsu_units::Unit], shared: &BTreeMap<String, String>) -> Option<(String, String)> {
    // each brand by its name: the unit as the table spells it first, and its other names
    let mut brands: BTreeMap<String, (String, BTreeMap<String, String>)> = BTreeMap::new();
    for u in units {
        let first = u.first_spelling();
        let (Some(name), Some(base)) = (u.type_name(), first.type_name()) else { continue };
        let entry = brands.entry(base.clone()).or_insert_with(|| (first.to_string(), BTreeMap::new()));
        if name != base {
            entry.1.insert(name, u.to_string());
        }
    }
    if brands.is_empty() {
        return None;
    }
    use ritsu_emit::header::{Comment, generated};
    let head = |c: Comment| c.line(&generated("ritsu"));
    let what = "The types of the units the package's rules and books take amounts in, one type of each unit (ritsu gen). The modules of the rules and the clients of the books name these types, so that the type checker keeps an amount of one unit from where another unit is taken.";
    Some(match target {
        "typescript" => {
            let mut o = format!("{}// {what}\n\n", head(Comment::Slashes));
            let mut from: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
            for base in brands.keys() {
                if let Some(path) = shared.get(base) {
                    from.entry(path.as_str()).or_default().push(base.as_str());
                }
            }
            for (path, names) in &from {
                o.push_str(&format!("import type {{ {} }} from \"{path}\";\n", names.join(", ")));
            }
            if !from.is_empty() {
                o.push('\n');
            }
            for (base, (shown, others)) in &brands {
                match shared.get(base) {
                    Some(_) => o.push_str(&format!("export type {{ {base} }}; // {shown}\n")),
                    None => o.push_str(&format!("export type {base} = bigint & {{ readonly __rulec: \"{base}\" }}; // {shown}\n")),
                }
                for (name, written) in others {
                    o.push_str(&format!("export type {name} = {base}; // {written}, the same unit\n"));
                }
            }
            ("units.ts".to_string(), o)
        }
        "python" => {
            let mut o = format!("{}\"\"\"{what}\"\"\"\n\nfrom __future__ import annotations\n\n", head(Comment::Hash));
            let own = brands.keys().any(|base| !shared.contains_key(base));
            if own {
                o.push_str("from typing import NewType\n");
            }
            if own && brands.keys().any(|base| shared.contains_key(base)) {
                o.push('\n');
            }
            for (base, (shown, _)) in &brands {
                if let Some(path) = shared.get(base) {
                    // `X as X`, an explicit re-export: the package's modules name it here
                    o.push_str(&format!("from {path} import {base} as {base}  # {shown}\n"));
                }
            }
            if brands.iter().any(|(base, (_, others))| !shared.contains_key(base) || !others.is_empty()) {
                o.push('\n');
            }
            for (base, (shown, others)) in &brands {
                if !shared.contains_key(base) {
                    o.push_str(&format!("{base} = NewType(\"{base}\", int)  # {shown}\n"));
                }
                for (name, written) in others {
                    o.push_str(&format!("{name} = {base}  # {written}, the same unit\n"));
                }
            }
            ("units.py".to_string(), o)
        }
        "go" => {
            let mut o = format!(
                "{}\n// Package units is the types of the units the package's rules and books take amounts in, one type of each\n// unit (ritsu gen). The packages of the rules and the clients of the books name these types, so that the\n// compiler keeps an amount of one unit from where another unit is taken.\npackage units\n",
                head(Comment::Slashes)
            );
            // each library's units package, by a name of its own: `pricinggounits` for
            // `example.com/acme/pricing-go/units`
            let mut named: BTreeMap<&str, String> = BTreeMap::new();
            for path in shared.iter().filter(|(base, _)| brands.contains_key(*base)).map(|(_, p)| p.as_str()) {
                if named.contains_key(path) {
                    continue;
                }
                let segments: Vec<&str> = path.trim_end_matches("/units").split('/').collect();
                let last = match segments.as_slice() {
                    [.., before, v] if v.len() > 1 && v.starts_with('v') && v[1..].bytes().all(|b| b.is_ascii_digit()) => before,
                    [.., last] => last,
                    [] => "",
                };
                let mut stem: String = last.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
                if stem.is_empty() || stem.starts_with(|c: char| c.is_ascii_digit()) {
                    stem = format!("lib{stem}");
                }
                let mut name = format!("{stem}units");
                let mut n = 2;
                while named.values().any(|v| *v == name) {
                    name = format!("{stem}units{n}");
                    n += 1;
                }
                named.insert(path, name);
            }
            match named.len() {
                0 => {}
                1 => {
                    let (path, name) = named.iter().next().expect("one");
                    o.push_str(&format!("\nimport {name} \"{path}\"\n"));
                }
                _ => {
                    o.push_str("\nimport (\n");
                    for (path, name) in &named {
                        o.push_str(&format!("\t{name} \"{path}\"\n"));
                    }
                    o.push_str(")\n");
                }
            }
            for (base, (shown, others)) in &brands {
                match shared.get(base) {
                    // an alias: the same type as the library's, so an amount passes between the two
                    Some(path) => o.push_str(&format!("\n// {base} is an amount of {shown}, the type of {path}.\ntype {base} = {}.{base}\n", named[path.as_str()])),
                    // a defined type, as a rule's own brand is: the compiler rejects YenInclTax + YenExclTax
                    None => o.push_str(&format!("\n// {base} is an amount of {shown}.\ntype {base} int64\n")),
                }
                for (name, written) in others {
                    o.push_str(&format!("\n// {name} is an amount of {written}, the same unit as {base}.\ntype {name} = {base}\n"));
                }
            }
            ("units/units.go".to_string(), o)
        }
        "rust" => {
            let mut o = format!(
                "{}//! The types of the units the package's rules and books take amounts in, one type of each unit (ritsu\n//! gen). The modules of the rules, the clients of the books and the code of kaname name these types,\n//! so that the compiler keeps an amount of one unit from where another unit is taken.\n",
                head(Comment::Slashes)
            );
            for (base, (shown, others)) in &brands {
                o.push_str(&format!(
                    "\n/// An amount of {shown}.\n#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub struct {base}(pub i64);\n\nimpl From<{base}> for i64 {{\n    fn from(v: {base}) -> i64 {{\n        v.0\n    }}\n}}\n"
                ));
                for (name, written) in others {
                    o.push_str(&format!("\n/// An amount of {written}, the same unit as {base}.\npub use {base} as {name};\n"));
                }
            }
            ("units.rs".to_string(), o)
        }
        _ => return None,
    })
}

impl<'a> Gen<'a> {
    /// What an `enums` file is generated as (§15.212): in each language that names a type
    /// across files, one module of the types and nothing else — the enums, the order of the
    /// ordered ones, the display names. The modules of the rules that read from it import the
    /// types from it. Not SQL, which holds a value as its name; not NumPy, whose plan carries
    /// the values; not Wasm, whose crate is one rule's alone and writes the types into itself.
    pub fn enums_module_files(&self) -> Vec<(String, String)> {
        let alias = pub_name(&self.f.name);
        let pkg = crate::backend::go_package(&alias);
        // The generator of a language where an enum's type of the file hides a builtin the module
        // reads, which reaches the builtin past it (§15.336).
        let (ts, rs, sw) = (self.for_target("typescript"), self.for_target("rust"), self.for_target("swift"));
        let (ts, rs, sw) = (ts.as_ref().unwrap_or(self), rs.as_ref().unwrap_or(self), sw.as_ref().unwrap_or(self));
        vec![
            (format!("python/{alias}.py"), self.py_enums_module()),
            (format!("typescript/{alias}.ts"), ts.ts_enums_module()),
            (format!("javascript/{alias}.mjs"), strip_types(&ts.ts_enums_module())),
            (format!("rust/{alias}.rs"), rs.rs_enums_module()),
            (format!("ruby/{alias}.rb"), self.rb_enums_module()),
            (format!("ruby/sig/{alias}.rbs"), self.rbs_enums_module()),
            (format!("php/{alias}.php"), self.php_enums_module()),
            (format!("go/{pkg}/{alias}.go"), self.go_enums_module()),
            (format!("go/{pkg}/go.mod"), format!("module {pkg}\n\ngo {}\n", crate::backend::GO_MIN)),
            (format!("swift/{alias}.swift"), sw.sw_enums_module()),
            (format!("java/{}.java", crate::codegen::java_class(&alias)), self.java_enums_module()),
        ]
    }

    /// The module of an `enums` file for one language of a package of ritsu's (ritsu's DESIGN
    /// 9.3): Python, TypeScript, Go or Rust.
    pub fn enums_module_for(&self, target: &str) -> Option<(String, String)> {
        let alias = pub_name(&self.f.name);
        let lang = self.for_target(target);
        let g = lang.as_ref().unwrap_or(self);
        Some(match target {
            "python" => (format!("{alias}.py"), g.py_enums_module()),
            "typescript" => (format!("{alias}.ts"), g.ts_enums_module()),
            "go" => (format!("{}/{alias}.go", crate::backend::go_package(&alias)), g.go_enums_module()),
            "rust" => (format!("{alias}.rs"), g.rs_enums_module_as(true)),
            _ => return None,
        })
    }

    // The functions of the `prerequisites` blocks of the file go into its module too (§15.222).

    fn py_enums_module(&self) -> String {
        let mut o = self.header("#");
        o.push_str("\nfrom __future__ import annotations\n\nimport enum\n\n\n");
        o.push_str(&self.py_enums());
        o.push_str(&self.py_prereqs());
        pep8_blanks(&o)
    }

    /// The module of an `enums` file in TypeScript, with a builtin an enum's type hides there read
    /// past it (`globalThis.Object`, §15.336).
    fn ts_enums_module(&self) -> String {
        self.apart_types(clash::Syntax::Script, |g| g.ts_enums_module_text())
    }

    fn ts_enums_module_text(&self) -> String {
        let mut o = self.header("//");
        o.push('\n');
        o.push_str(&self.ts_enums());
        o.push_str(&self.ts_prereqs());
        o
    }

    fn rs_enums_module(&self) -> String {
        self.rs_enums_module_as(false)
    }

    /// The same, as `rulec gen` writes it or (`package`) as a module of a package's crate, whose
    /// root allows the lints of the names' letters.
    fn rs_enums_module_as(&self, package: bool) -> String {
        self.apart_types(clash::Syntax::Rust, |g| g.rs_enums_module_text(package))
    }

    /// The module of [`Gen::rs_enums_module_as`] as it is written where no enum's type hides a
    /// builtin it reads; where one does, the module reaches it by its path (`::std::vec::Vec`).
    fn rs_enums_module_text(&self, package: bool) -> String {
        let mut o = self.header("//");
        // As the rule's module does: a module of the runner's crate where a rule reads the file.
        o.push_str(if package { "\n#![allow(non_snake_case, dead_code)]\n\n" } else { "\n#![allow(non_snake_case, uncommon_codepoints, unused_attributes, dead_code, clippy::all)]\n\n" });
        for (jp, ascii) in self.own_enums() {
            o.push_str(&self.rs_enum(&jp, &ascii));
        }
        o.push_str(&self.rs_prereqs());
        o
    }

    fn go_enums_module(&self) -> String {
        let alias = pub_name(&self.f.name);
        let mut o = self.header("//");
        o.push_str(&format!("\npackage {}\n\n", crate::backend::go_package(&alias)));
        for (jp, ascii) in self.own_enums() {
            o.push_str(&self.go_enum(&jp, &ascii));
        }
        o.push_str(&self.go_prereqs());
        align(o.trim_end_matches('\n')) + "\n"
    }

    fn rb_enums_module(&self) -> String {
        let mut o = self.header("#");
        o.insert_str(0, "# frozen_string_literal: true\n");
        o.push_str(&format!("module {}\n", self.rb_module()));
        for (jp, ascii) in self.own_enums() {
            o.push_str(&self.rb_enum(&jp, &ascii));
        }
        o.push_str(&self.rb_prereqs());
        let o = o.trim_end_matches('\n').to_string();
        format!("{o}\nend\n")
    }

    fn rbs_enums_module(&self) -> String {
        let mut o = self.header("#");
        o.push_str(&format!("\nmodule {}\n", self.rb_module()));
        for (jp, ascii) in self.own_enums() {
            o.push_str(&self.rbs_enum(&jp, &ascii));
        }
        o.push_str(&self.rbs_prereqs());
        let o = o.trim_end_matches('\n').to_string();
        format!("{o}\nend\n")
    }

    fn php_enums_module(&self) -> String {
        let mut o = String::from("<?php\n\n");
        o.push_str(&self.header("//"));
        o.push_str("\ndeclare(strict_types=1);\n\n");
        o.push_str(&format!("namespace {};\n\n", self.php_ns()));
        for (jp, ascii) in self.own_enums() {
            o.push_str(&self.php_enum(&jp, &ascii));
        }
        o.push_str(&self.php_prereqs());
        o.trim_end_matches('\n').to_string() + "\n"
    }

    /// The module of an `enums` file in Swift, with a type an enum's type hides there read through
    /// the module `Swift` (`public enum String: Swift.String`, §15.336).
    fn sw_enums_module(&self) -> String {
        self.apart_types(clash::Syntax::Swift, |g| g.sw_enums_module_text())
    }

    fn sw_enums_module_text(&self) -> String {
        let mut o = self.header("//");
        o.push('\n');
        for (jp, ascii) in self.own_enums() {
            o.push_str(&self.sw_enum(&jp, &ascii));
        }
        o.push_str(&self.sw_prereqs());
        o.trim_end_matches('\n').to_string() + "\n"
    }

    /// An `enums` file's class in Java: where an enum's type of the file has the name of a class of
    /// the JDK the class reads by its simple name, the class reads that one by its full name
    /// (§15.336, [`Gen::java_apart`]).
    fn java_enums_module(&self) -> String {
        self.java_apart(self.java_hidden(true), |g| g.java_enums_module_text())
    }

    /// The class of [`Gen::java_enums_module`] as it is written where no name meets a class of the
    /// JDK.
    fn java_enums_module_text(&self) -> String {
        let cls = self.java_class();
        let mut o = ritsu_emit::header::for_unicode_comment(&self.header("//"));
        o.push_str(&format!(
            "\n/** {} */\npublic final class {cls} {{\n    private {cls}() {{\n    }}\n\n",
            tr!("enums {} v{} の列挙", "The enums of enums {} v{}", self.f.name.text, self.f.version)
        ));
        for (jp, ascii) in self.own_enums() {
            o.push_str(&self.java_enum_decl(&jp, &ascii));
        }
        o.push_str(&self.java_prereqs());
        o.trim_end_matches('\n').to_string() + "\n}\n"
    }
}

/// The modules of the `enums` files a rule reads its types from (§15.212), as `rulec gen` writes
/// them beside the rule's own: each file read and checked again, its modules in every language.
/// `Err` says which file did not read.
fn owner_modules(f: &RuleFile) -> Result<Vec<(String, Vec<(String, String)>)>, String> {
    let mut out = Vec::new();
    for p in crate::enums::owner_paths(f) {
        let ef = crate::enums::read_enums_file(&p).map_err(|_| crate::i18n::said(|| tr!("`{p}` が check を通りません", "`{p}` does not pass check")))?;
        let g = Gen::new(&ef.file, &ef.checked, &ef.src, &ef.path);
        out.push((p.clone(), g.enums_module_files()));
    }
    Ok(out)
}

/// The JavaScript module `js` with the module of an `enums` file (`code`, written as
/// `./<module>.mjs`) put inside it, for a page (§15.319): the module's text in a function that
/// answers the names it exports, bound once before the first line that reads from it; then
/// `import { A, B } from "./<module>.mjs";` takes A and B from that answer, and `export { a, b }
/// from "./<module>.mjs";` takes them and exports them as the module's own.
fn module_inside(js: &str, module: &str, code: &str) -> String {
    let from = format!(" from \"./{module}.mjs\";");
    let mut exported: Vec<String> = Vec::new();
    let mut body = String::new();
    for line in code.lines() {
        let decl = ["export const ", "export function ", "export class "].into_iter().find(|k| line.starts_with(k));
        match decl {
            Some(k) => {
                exported.push(line[k.len()..].chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$').collect());
                body.push_str(&line["export ".len()..]);
            }
            None => body.push_str(line),
        }
        body.push('\n');
    }
    let answer = format!("_{module}_module");
    let mut bound: Vec<String> = Vec::new();
    let mut out = String::new();
    for line in js.lines() {
        let Some(head) = line.strip_suffix(&from) else {
            out.push_str(line);
            out.push('\n');
            continue;
        };
        let (export, list) = match (head.strip_prefix("import "), head.strip_prefix("export ")) {
            (Some(l), _) => (false, l),
            (_, Some(l)) => (true, l),
            _ => (false, head),
        };
        let names: Vec<&str> = list.trim().trim_start_matches('{').trim_end_matches('}').split(',').map(str::trim).filter(|n| !n.is_empty()).collect();
        if bound.is_empty() {
            out.push_str(&format!("const {answer} = (() => {{\n{body}return {{ {} }};\n}})();\n", exported.join(", ")));
        }
        // `X as Y` is the module's X by the name Y here; a line that hands it on exports X as Y
        let mut taken: Vec<String> = Vec::new();
        for n in &names {
            let (theirs, ours) = match n.split_once(" as ") {
                Some((x, y)) if !export => (x.trim(), y.trim()),
                Some((x, _)) => (x.trim(), x.trim()),
                None => (*n, *n),
            };
            if !bound.iter().any(|b| b == ours) {
                bound.push(ours.to_string());
                taken.push(if theirs == ours { ours.to_string() } else { format!("{theirs}: {ours}") });
            }
        }
        if !taken.is_empty() {
            out.push_str(&format!("const {{ {} }} = {answer};\n", taken.join(", ")));
        }
        if export {
            out.push_str(&format!("export {{ {} }};\n", names.join(", ")));
        }
    }
    out
}

/// Why the module `gen` writes cannot hold two contracts at one path when one of them is a
/// dependency's (ritsu's DESIGN 17.6): buf reads the module in one run of protoc, which reads one
/// file by one name.
fn contracts_clash(rel: &str, a: &std::path::Path, b: &std::path::Path) -> String {
    let (a, b) = (a.display(), b.display());
    tr!(
        "`{a}` と `{b}` は中身の違うファイルですが、どちらも生成するモジュールの `proto/{rel}` になり、`{rel}` として import されます。protoc は一つの名前を一つのファイルとして読むので、生成しません",
        "`{a}` and `{b}` are two different files, and both would be `proto/{rel}` of the module generated, imported as `{rel}`; protoc reads one name as one file, so nothing is generated"
    )
}

/// Why the module `gen` writes cannot hold one file of a dependency at two paths (ritsu's DESIGN
/// 17.6): two contracts reach it by two names, and protoc reads the two copies as two files that
/// declare the same types.
fn contracts_two_names(file: &std::path::Path, a: &str, b: &str) -> String {
    let file = file.display();
    tr!(
        "`{file}` は、`{a}` と `{b}` の二つの名前で import され、生成するモジュールの `proto/{a}` と `proto/{b}` の二つになります。protoc は二つを別のファイルとして読み、同じ型を二度宣言するので、生成しません",
        "`{file}` is imported by two names, `{a}` and `{b}`, and would be both `proto/{a}` and `proto/{b}` of the module generated; protoc reads the two as two files that declare the same types, so nothing is generated"
    )
}

/// Why the module `gen` writes cannot hold a contract at the path of a rule's own `.proto`: the copy
/// would be written over the `.proto`, or the `.proto` over the copy, and a `.proto` copied from a
/// generated one to be the contract it implements is what `implements` is for (§15.274).
fn contract_over_own(rel: &str, contract: &std::path::Path, rule: &str) -> String {
    let contract = contract.display();
    tr!(
        "`{contract}` のコピー先の `proto/{rel}` は、`{rule}` から生成する `.proto` と同じパスで、二つは中身が違うので、生成しません。生成した `.proto` をコピーして契約にしたのなら、`{rule}` に `implements proto \"<ファイル>\" <サービス>` を書いてください。規則の `.proto` は生成せず、契約をそのままコピーします",
        "`{contract}` would be copied to `proto/{rel}`, the path of the `.proto` generated from `{rule}`, and the two differ, so nothing is generated. If the contract is a copy of a generated `.proto`, write `implements proto \"<file>\" <Service>` in `{rule}`: its own `.proto` is then not generated, and the contract is copied as it stands"
    )
}

/// A rule of a run of `gen`, read: its text and what `prepare` made of it, once its check passed; or
/// what the check found, which stops the run there; or nothing, when the file does not read.
enum RuleRead {
    Unread(std::io::Error),
    Fails(String, Vec<crate::diag::Diag>),
    Ready(String, Box<(RuleFile, Checked)>),
}

/// The rule at `path` read and checked as `gen` reads it before it writes anything of it.
fn read_rule(path: &str) -> RuleRead {
    let src = match ritsu_base::fs::read_to_string(path) {
        Ok(src) => src,
        Err(e) => return RuleRead::Unread(e),
    };
    // Nothing is generated from a rule that does not pass (README, AGENTS.md, the generate page).
    // `prepare` only gets as far as the types, so the checks the tables themselves carry —
    // completeness, overlap, dead rows, overflow and the examples — are run here, the way `doc`
    // runs them (§1.6).
    let rep = crate::report(&src, path);
    if crate::has_error(&rep.diags) {
        return RuleRead::Fails(src, rep.diags);
    }
    match crate::prepare(&src, path) {
        Ok(fc) => RuleRead::Ready(src, Box::new(fc)),
        Err(ds) => RuleRead::Fails(src, ds),
    }
}

/// What stops a run of `gen` in a module of ritsu's for a dependency's file (ritsu's DESIGN 17.6):
/// two files with different text at one path of the module of the rule's service, one of them a
/// dependency's, and a file of a dependency's reached by two names, in one rule's contracts or across
/// the rules of the run; the sentence the run stops with (exit 2). Made sure of over every rule that
/// passes before any rule's files are written, as `ritsu gen` makes sure of what a module stops before
/// any target is written, so that a stop met at the second rule does not leave the first rule's files
/// behind. A rule that does not read or pass is left to the run, which stops at it as it did; so is a
/// clash of two files of the module's own (exit 1), which no dependency brings.
fn dependencies_hold(files: &[&str], read: &[RuleRead]) -> Option<String> {
    let mut contracts: std::collections::BTreeMap<String, (String, std::path::PathBuf)> = std::collections::BTreeMap::new();
    let mut dependency_files: std::collections::BTreeMap<std::path::PathBuf, String> = std::collections::BTreeMap::new();
    // Each rule's own `.proto`, by its path in the module, with its text and the rule (§15.274).
    let mut own: std::collections::BTreeMap<String, (String, String)> = std::collections::BTreeMap::new();
    for (path, r) in files.iter().zip(read) {
        let RuleRead::Ready(src, fc) = r else { continue };
        let (f, c) = &**fc;
        if f.kind == FileKind::Enums {
            continue;
        }
        let g = crate::codegen::Gen::new(f, c, src, path);
        if f.implements.is_none() {
            let (rel, body) = (g.proto_path(), g.proto());
            if let Some((_, disk)) = contracts.get(&rel).filter(|(prev, disk)| *prev != body && ritsu_base::modules::under_dependency(disk)) {
                return Some(contract_over_own(&rel, disk, path));
            }
            own.insert(rel, (body, path.to_string()));
        }
        let found = g.proto_contracts();
        if let Some((rel, a, b)) = &found.clash {
            return Some(contracts_clash(rel, a, b));
        }
        if let Some((file, a, b)) = &found.two_names {
            return Some(contracts_two_names(file, a, b));
        }
        for ((rel, body), disk) in found.files.into_iter().zip(found.from) {
            if let Some((_, rule)) = own.get(&rel).filter(|(prev, _)| *prev != body)
                && ritsu_base::modules::under_dependency(&disk)
            {
                return Some(contract_over_own(&rel, &disk, rule));
            }
            match contracts.get(&rel) {
                Some((prev, prev_disk)) if *prev != body => {
                    if ritsu_base::modules::under_dependency(prev_disk) || ritsu_base::modules::under_dependency(&disk) {
                        return Some(contracts_clash(&rel, prev_disk, &disk));
                    }
                    continue;
                }
                Some(_) => {}
                None => {
                    contracts.insert(rel.clone(), (body, disk.clone()));
                }
            }
            if ritsu_base::modules::under_dependency(&disk) {
                let key = connect::same_file_key(&disk);
                match dependency_files.get(&key) {
                    Some(prev) if *prev != rel => {
                        let mut names = [prev.clone(), rel.clone()];
                        names.sort();
                        return Some(contracts_two_names(&disk, &names[0], &names[1]));
                    }
                    Some(_) => {}
                    None => {
                        dependency_files.insert(key, rel);
                    }
                }
            }
        }
    }
    None
}

/// `rulec gen` (§15.277): the library of each target `asked` gives and the extras it asks for,
/// written under `out_dir`, or under `--check` held to what is there; the exit code.
pub fn generate(files: &[&str], out_dir: &str, asked: &crate::targets::Asked, check_only: bool, json: bool, out: &mut dyn std::io::Write, err: &mut dyn std::io::Write) -> u8 {
    use crate::targets::{PROTO, Part};
    // Every path below is `{out_dir}/{rel}`, so the slashes `--out` ends with are dropped first:
    // `--out generated/` is `--out generated`, and no path is said as `generated//python/…`, as it
    // was up to 0.40.0 (§15.277). `--out /` keeps its one slash, at the head of each path.
    let out_dir = out_dir.trim_end_matches('/');
    // In a module of ritsu's, every rule is read first, and what a dependency's file stops is made
    // sure of before anything is written (ritsu's DESIGN 17.6); the loop below takes each rule as it
    // was read. Outside every module, each rule is read where the loop comes to it, as before (M14).
    let in_a_module = files.iter().any(|p| {
        let dir = std::path::Path::new(p).parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(std::path::Path::new("."));
        ritsu_base::modules::module_of(dir).is_some()
    });
    let mut early: Vec<Option<RuleRead>> = Vec::new();
    if in_a_module {
        let read: Vec<RuleRead> = files.iter().map(|p| crate::i18n::said(|| read_rule(p))).collect();
        // What it makes sure of is the module of the `.proto` files, which `--target proto` writes.
        if asked.target(PROTO)
            && let Some(stop) = crate::i18n::said(|| dependencies_hold(files, &read))
        {
            let _ = writeln!(err, "{}", crate::i18n::said(|| ritsu_base::cli::refusal(&stop, crate::i18n::base())));
            return 2;
        }
        early = read.into_iter().map(Some).collect();
    }
    let mut st = Emitted::default();
    // The files of the extras this run is not asked for, of the targets it is given, each with its
    // part: one found where it would be written is named as left there (§15.277).
    let mut unasked: Vec<(String, Part)> = Vec::new();
    // The contracts copied into the module so far, the rule each came with and the file it was
    // copied from: two rules may import the same one, but not two different files under one path.
    let mut contracts: std::collections::BTreeMap<String, (String, String, std::path::PathBuf)> = std::collections::BTreeMap::new();
    // The files of the dependencies copied so far, each by the name it was copied under: one file is
    // not copied under two names (ritsu's DESIGN 17.6).
    let mut dependency_files: std::collections::BTreeMap<std::path::PathBuf, String> = std::collections::BTreeMap::new();
    // The rules' own `.proto` files written so far, with their text and the rule: a contract is not
    // copied over one, nor one written over a contract (§15.274).
    let mut own_protos: std::collections::BTreeMap<String, (String, String)> = std::collections::BTreeMap::new();
    // Where each rule that implements a contract's service had its own `.proto` before, and the
    // rule: an earlier run may have left the file there (§15.275).
    let mut before_contract: Vec<(String, String)> = Vec::new();
    // What the module needs from the BSR, over every rule of the run (§15.161).
    let mut module_deps = crate::codegen::BufDeps::default();
    // The modules of the `enums` files written so far, and the file each came from: two rules
    // may read the same one, but two files may not make the same module (§15.212).
    let mut enum_modules: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    // One file reached by two spellings of its path (`rules/x/../shared/catalog.rule`) is one file.
    let same_file = |a: &str, b: &str| a == b || matches!((std::fs::canonicalize(a), std::fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y);
    let mut put_modules = |from: &str, files: Vec<(String, String)>, err: &mut dyn std::io::Write| -> Option<Vec<(String, String)>> {
        let mut fresh = Vec::new();
        for (rel, body) in files {
            let p = format!("{out_dir}/{rel}");
            match enum_modules.get(&p) {
                Some(prev) if !same_file(prev, from) => {
                    let _ = writeln!(err, "{}", crate::i18n::said(|| ritsu_base::cli::refusal(&tr!(
                        "`{prev}` と `{from}` が、同じ `{rel}` を作ります。どちらかの名前を変えてください",
                        "`{prev}` and `{from}` both make `{rel}`; rename one of them"
                    ), crate::i18n::base())));
                    return None;
                }
                Some(_) => {}
                None => {
                    enum_modules.insert(p.clone(), from.to_string());
                    fresh.push((p, body));
                }
            }
        }
        Some(fresh)
    };
    for (k, path) in files.iter().enumerate() {
        let (src, f, c) = match early.get_mut(k).and_then(Option::take).unwrap_or_else(|| crate::i18n::said(|| read_rule(path))) {
            // what stops the run, as every command of ritsu says it (ritsu's DESIGN 6.2, item 10)
            RuleRead::Unread(e) => return crate::i18n::refuse(err, &ritsu_base::cli::cannot_read(path, &e)),
            RuleRead::Fails(src, ds) => {
                let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
                let _ = write!(out, "{}", crate::i18n::said(|| crate::findings_text(&ds, &lines)));
                let _ = writeln!(err, "{}", crate::i18n::said(|| ritsu_base::cli::refusal(&tr!("`{path}` は検査を通っていないので生成しません", "`{path}` does not pass check, so nothing is generated"), crate::i18n::base())));
                return 1;
            }
            RuleRead::Ready(src, fc) => {
                let (f, c) = *fc;
                (src, f, c)
            }
        };
        // A file of enums is its modules and nothing else: no function, so no vectors either
        // (§15.212).
        if f.kind == FileKind::Enums {
            let g = crate::codegen::Gen::new(&f, &c, &src, path);
            let Some(mut targets) = put_modules(path, asked.split(g.enums_module_files(), Part::Library).0, err) else { return 1 };
            // Its `prerequisites` blocks are functions of its modules (§15.222): a runner beside
            // each, and what they have to answer.
            if let Some((input, expected)) = g.prereq_input() {
                let alias = pub_name(&f.name);
                let (runners, not) = asked.split(g.enums_runner_files(), Part::Runner);
                for (rel, body) in runners {
                    targets.push((format!("{out_dir}/{rel}"), body));
                }
                unasked.extend(not.into_iter().map(|rel| (format!("{out_dir}/{rel}"), Part::Runner)));
                if asked.extra("vectors") {
                    targets.push((format!("{out_dir}/vectors/{alias}.prereq.tsv"), input));
                    targets.push((format!("{out_dir}/vectors/{alias}.prereq.expected"), expected));
                }
                if !json {
                    for b in crate::backend::ALL.iter().filter(|b| asked.target(b.id) && (!b.prereqs || b.id == "numpy")) {
                        let _ = writeln!(out, "{}", crate::i18n::said(|| if b.prereqs { prereq_not_here(b.name) } else { prereq_skipped(b.name) }));
                    }
                }
            }
            if let Some(code) = emit(targets, check_only, json, &mut st, out, err) {
                return code;
            }
            continue;
        }
        let g = crate::codegen::Gen::new(&f, &c, &src, path).in_python_package(asked.python_package());
        let alias = f.name.ascii.clone().unwrap_or_else(|| f.name.text.clone());
        let pkg = crate::backend::go_package(&alias);
        // The modules of the `enums` files it reads its types from, beside its own, whether or
        // not those files were named on the command line (§15.212).
        let owners = match owner_modules(&f) {
            Ok(v) => v,
            Err(e) => {
                let _ = writeln!(err, "{}", crate::i18n::said(|| ritsu_base::cli::refusal(&e, crate::i18n::base())));
                return 1;
            }
        };
        for (from, files) in owners {
            let Some(targets) = put_modules(&from, asked.split(files, Part::Library).0, err) else { return 1 };
            if let Some(code) = emit(targets, check_only, json, &mut st, out, err) {
                return code;
            }
        }
        // Every backend says which files it writes (src/backend.rs). The list used to be
        // here, spelled out per language, which is one of the seven places a new target had
        // to be added by hand.
        let mut targets: Vec<(String, String)> = Vec::new();
        for b in crate::backend::ALL.iter().filter(|b| asked.target(b.id)) {
            // A walk is written only by the backends that can write one (§15.56). A file that
            // cannot run is worse than a missing one, so the rest are named and skipped.
            if !f.elements.is_empty() && !b.folds {
                if !json {
                    let _ = writeln!(out,
                        "{}",
                        crate::i18n::said(|| tr!(
                            "{}: この言語には並びをたどる規則を生成しません",
                            "{}: a rule that walks a sequence is not generated for this target",
                            b.name
                        ))
                    );
                }
                continue;
            }
            // The same rule for a table whose column is a `string` (§15.101).
            let text_column = f.items.iter().any(|it| match it {
                crate::ast::Item::Table(t) => t
                    .inputs
                    .iter()
                    .any(|(n, _)| matches!(c.ty_of(n), Some(crate::types::Ty::Str))),
                _ => false,
            });
            if !b.texts && text_column {
                if !json {
                    let _ = writeln!(out,
                        "{}",
                        crate::i18n::said(|| tr!(
                            "{}: この言語には文字列の列を持つ表を生成しません",
                            "{}: a table with a column of strings is not generated for this target",
                            b.name
                        ))
                    );
                }
                continue;
            }
            // The functions of `prerequisites` where the language has no room for them (§15.222):
            // the rule is generated, and the reader told what it lacks.
            if !b.prereqs && !f.prerequisites.is_empty() && !json {
                let _ = writeln!(out, "{}", crate::i18n::said(|| prereq_skipped(b.name)));
            }
            // The function that reads a rule's inputs out of the caller's object, where the
            // language has no object to read them from (§15.125, §15.223): the rule is generated,
            // and the reader told which function it lacks.
            if !b.projects && g.projects() && !json {
                let _ = writeln!(out, "{}", crate::i18n::said(|| proj_skipped(b.name, b.id, &alias)));
            }
            // The same for the function that counts the answers over the random inputs (§15.225).
            if !b.chances && f.has_random() && !json {
                let _ = writeln!(out, "{}", crate::i18n::said(|| chances_skipped(b.name, b.id, &alias)));
            }
            // Its library, and the extras asked for (§15.277), each enum under the type it takes
            // in the language (§15.326).
            let here = g.for_target(b.id);
            for (rel, body, part) in (b.files)(here.as_ref().unwrap_or(&g), &alias, &pkg) {
                if asked.wants(b.id, part) {
                    targets.push((format!("{out_dir}/{rel}"), body));
                } else {
                    unasked.push((format!("{out_dir}/{rel}"), part));
                }
            }
        }
        // The target `proto` (§15.277): the rule's `.proto`, or the contracts it implements and
        // imports from, as the module of buf they land in.
        if asked.target(PROTO) {
            // The wire of the rule as a service (§15.112). It is one file for every language,
            // not one per backend, so it is pushed here rather than from the registry: the
            // `.proto` is the contract, and a contract does not come in twelve copies. A rule that
            // implements a contract's service has none of its own: the contract is the `.proto`,
            // copied below (§15.274).
            if f.implements.is_none() {
                let (p, body) = (format!("{out_dir}/proto/{}", g.proto_path()), g.proto());
                if let Some((_, _, disk)) = contracts.get(&p).filter(|(prev, _, _)| *prev != body) {
                    let _ = writeln!(err, "{}", crate::i18n::said(|| ritsu_base::cli::refusal(&contract_over_own(&g.proto_path(), disk, path), crate::i18n::base())));
                    return if ritsu_base::modules::under_dependency(disk) { 2 } else { 1 };
                }
                own_protos.insert(p.clone(), (body.clone(), path.to_string()));
                targets.push((p, body));
            } else {
                before_contract.push((format!("{out_dir}/proto/{}", g.proto_path()), path.to_string()));
            }
            // The contracts that `.proto` imports an enum from, so that the module builds as it
            // stands (§15.160): each the file `check` held the rule to, copied byte for byte to
            // the path the import names, and the contract the rule implements. What they import
            // from the BSR goes into the module's `buf.yaml` and `buf.lock`, written once below.
            let found = g.proto_contracts();
            if let Some((rel, a, b)) = &found.clash {
                let _ = writeln!(err, "{}", crate::i18n::said(|| ritsu_base::cli::refusal(&contracts_clash(rel, a, b), crate::i18n::base())));
                return 2;
            }
            if let Some((file, a, b)) = &found.two_names {
                let _ = writeln!(err, "{}", crate::i18n::said(|| ritsu_base::cli::refusal(&contracts_two_names(file, a, b), crate::i18n::base())));
                return 2;
            }
            if let Err(e) = crate::i18n::said(|| module_deps.merge(found.deps)) {
                let _ = writeln!(err, "{}", crate::i18n::said(|| ritsu_base::cli::refusal(&e, crate::i18n::base())));
                return 1;
            }
            for ((rel, body), disk) in found.files.into_iter().zip(found.from) {
                let p = format!("{out_dir}/proto/{rel}");
                // a contract that would be copied over a rule's own `.proto`, or under it
                if let Some((_, rule)) = own_protos.get(&p).filter(|(prev, _)| *prev != body) {
                    let _ = writeln!(err, "{}", crate::i18n::said(|| ritsu_base::cli::refusal(&contract_over_own(&rel, &disk, rule), crate::i18n::base())));
                    return if ritsu_base::modules::under_dependency(&disk) { 2 } else { 1 };
                }
                if let Some((_, from, prev_disk)) = contracts.get(&p).filter(|(prev, _, _)| *prev != body) {
                    // in a module of ritsu's, with a dependency's file among the two, the run stops as
                    // it does for one rule's (ritsu's DESIGN 17.6)
                    if ritsu_base::modules::under_dependency(prev_disk) || ritsu_base::modules::under_dependency(&disk) {
                        let _ = writeln!(err, "{}", crate::i18n::said(|| ritsu_base::cli::refusal(&contracts_clash(&rel, prev_disk, &disk), crate::i18n::base())));
                        return 2;
                    }
                    let _ = writeln!(err,
                        "{}",
                        crate::i18n::said(|| ritsu_base::cli::refusal(&tr!(
                            "`{from}` と `{path}` が、違う内容の契約を同じ `proto/{rel}` として取り込んでいます",
                            "`{from}` and `{path}` import two different contracts as the same `proto/{rel}`"
                        ), crate::i18n::base()))
                    );
                    return 1;
                }
                // one file of a dependency's that another rule copied under another name
                if ritsu_base::modules::under_dependency(&disk) {
                    let key = connect::same_file_key(&disk);
                    match dependency_files.get(&key) {
                        Some(prev) if *prev != rel => {
                            let mut names = [prev.clone(), rel.clone()];
                            names.sort();
                            let _ = writeln!(err, "{}", crate::i18n::said(|| ritsu_base::cli::refusal(&contracts_two_names(&disk, &names[0], &names[1]), crate::i18n::base())));
                            return 2;
                        }
                        Some(_) => {}
                        None => {
                            dependency_files.insert(key, rel.clone());
                        }
                    }
                }
                contracts.insert(p.clone(), (body.clone(), path.to_string(), disk));
                targets.push((p, body));
            }
        }
        // Also emit the vectors and the expected values, for `--extra vectors` (§15.277). Of the
        // three uses in §9.3, the cross-language agreement test and the golden files run on these.
        if asked.extra("vectors") {
            let suite = crate::vectors::suite(&f, &c);
            let vs = &suite.vectors;
            // One line a vector, each ended; a suite with no answered case — every input found is
            // one the rule refuses — is a file with no line, not one empty line (§15.314).
            let lines = |of: &dyn Fn(&crate::vectors::Vector) -> String| -> String { vs.iter().map(|v| of(v) + "\n").collect() };
            let vec_body: String = lines(&|v| crate::vectors::to_json(&f, &c, v));
            let exp_body: String = lines(&|v| crate::vectors::expected_json(&f, &c, v));
            targets.push((format!("{out_dir}/vectors/{alias}.jsonl"), vec_body));
            targets.push((format!("{out_dir}/vectors/{alias}.expected.jsonl"), exp_body));
            // The modules of the `enums` files the rule reads, for a language that builds them
            // together with it (§15.212): `rulec test` reads this beside the vectors. None for a
            // rule whose Swift declares those enums itself (§15.326).
            let owners: Vec<String> = g.foreign_enums().into_iter().map(|(m, _)| m).collect();
            if !owners.is_empty() && !g.sw_alone() {
                targets.push((format!("{out_dir}/vectors/{alias}.owners.json"), crate::json::strs(&owners) + "\n"));
            }
            // The display names every language has to give back, value by value (§15.211).
            if let Some((input, expected)) = g.labels_input() {
                targets.push((format!("{out_dir}/vectors/{alias}.labels.tsv"), input));
                targets.push((format!("{out_dir}/vectors/{alias}.labels.expected"), expected));
            }
            // What the functions of each `prerequisites` block have to answer (§15.222).
            if let Some((input, expected)) = g.prereq_input() {
                targets.push((format!("{out_dir}/vectors/{alias}.prereq.tsv"), input));
                targets.push((format!("{out_dir}/vectors/{alias}.prereq.expected"), expected));
            }
            // What the function that counts the answers has to give, line by line (§15.225).
            if let Some((input, expected)) = crate::chances::vector_files(&f, &c, vs) {
                targets.push((format!("{out_dir}/vectors/{alias}.chances.jsonl"), input));
                targets.push((format!("{out_dir}/vectors/{alias}.chances.expected"), expected));
            }
            // The cases the reference evaluator refuses: the walks it has no answer for, and an
            // input for every reason the door has (§15.204). They have no expected record —
            // refusing is the expectation — so they get a file of their own, and `rulec test` holds
            // every generated language to refusing each of them with the reference's sentence
            // (§15.56).
            let refused: Vec<String> = crate::vectors::refused_lines(&f, &c, &suite);
            if !refused.is_empty() {
                targets.push((format!("{out_dir}/vectors/{alias}.refused.jsonl"), refused.join("\n") + "\n"));
            }
            // The sequences of calls a machine is played through (§15.148): each runner passes
            // the state its own language answered to the next call, and prints its constants
            // first, so the traces hold the hand-over and `INITIAL` / the final states as well.
            if let Some(t) = crate::vectors::machine_traces(&f, &c) {
                targets.push((format!("{out_dir}/vectors/{alias}.traces.jsonl"), crate::vectors::traces_json(&f, &c, &t)));
                targets.push((
                    format!("{out_dir}/vectors/{alias}.traces.expected.jsonl"),
                    crate::vectors::traces_expected_json(&f, &c, &t),
                ));
            }
        }
        if let Some(code) = emit(targets, check_only, json, &mut st, out, err) {
            return code;
        }
    }
    // The directory the `.proto` files land in is a buf module, configured the way
    // connect-python's own documentation configures one, so that `buf lint` and `buf generate`
    // work in it as it stands. Its configuration belongs to the directory, not to a rule, and
    // is written once: the BSR modules the contracts of every rule depend on, and their pins
    // when the `buf.lock` files beside the contracts give every one (§15.161).
    for n in &module_deps.notes {
        let _ = writeln!(err, "{}", crate::i18n::said(|| tr!("注意: {n}", "warning: {n}")));
    }
    // It is the target `proto`'s; `buf.gen.yaml`, which builds the stubs of the Python service and
    // nothing else, is the Connect service's (§15.277).
    let mut module = Vec::new();
    if asked.target(PROTO) {
        module.push((format!("{out_dir}/proto/buf.yaml"), crate::codegen::buf_yaml(&module_deps)));
        let stubs = format!("{out_dir}/proto/buf.gen.yaml");
        if asked.wants(PROTO, Part::Connect) {
            module.push((stubs, crate::codegen::buf_gen_yaml()));
        } else {
            unasked.push((stubs, Part::Connect));
        }
        if let Some(lock) = crate::codegen::buf_lock(&module_deps) {
            module.push((format!("{out_dir}/proto/buf.lock"), lock));
        }
    }
    if let Some(code) = emit(module, check_only, json, &mut st, out, err) {
        return code;
    }
    // The `.proto` a rule had before it implemented a contract's service, still in the output
    // directory. `gen` deletes nothing there, so it says the file is left and may go, with or
    // without `--check`, and the exit code stays what it is (§15.275). A path this run writes is
    // none (the copy of a generated `.proto` taken as the contract lands on it), and only a file
    // whose first line is rulec's head is one: a file someone put there is not named.
    for (p, rule) in &before_contract {
        if st.planned.contains(p) || st.left.contains(p) || !generated_by_rulec(p) {
            continue;
        }
        st.left.push(p.clone());
        if !json {
            let _ = writeln!(out, "{}", crate::i18n::said(|| left_behind(p, rule)));
        }
    }
    // The files of the extras this run is not asked for, found where it would have written them:
    // an earlier run asked for them, or was 0.40.0's, which wrote every one (§15.277). They are said
    // as that `.proto` is, the extras that write them named once, and the exit code stays what it is.
    let mut extras_left: Vec<&'static str> = Vec::new();
    for (p, part) in &unasked {
        if st.planned.contains(p) || st.left.contains(p) || !extra_by_rulec(p) {
            continue;
        }
        st.left.push(p.clone());
        extras_left.extend(asked.not_asked(*part));
        if !json {
            let _ = writeln!(out, "{}", crate::i18n::said(|| tr!("もう生成しないファイルが残っています: {p}", "generated no more, and still there: {p}")));
        }
    }
    if !json && !extras_left.is_empty() {
        let _ = writeln!(out, "{}", crate::i18n::said(|| crate::targets::left_note(&extras_left)));
    }
    if json {
        let _ = writeln!(out,
            "{}",
            crate::json::Obj::new()
                .raw("written", crate::json::strs(&st.written))
                .raw("stale", crate::json::strs(&st.stale))
                .raw("missing", crate::json::strs(&st.missing))
                .raw("left", crate::json::strs(&st.left))
                .finish()
        );
    }
    st.dirty
}

/// Whether the file at `p` is one rulec wrote: its first line is the head of every file rulec
/// generates (`// Code generated by rulec 0.38.0. DO NOT EDIT.`), of whichever version.
fn generated_by_rulec(p: &str) -> bool {
    let Ok(text) = ritsu_base::fs::read_to_string(p) else { return false };
    text.lines().next().and_then(|l| l.strip_prefix("// ")).is_some_and(|l| l.starts_with("Code generated by rulec ") && l.ends_with("DO NOT EDIT."))
}

/// Whether the file at `p` is a file of an extra rulec wrote (§15.277): a line of it is rulec's
/// head, of whichever version, in the comment of the file's language. It is the first line of most;
/// PHP's comes after `<?php`, and the page carries it in its script. The `.proto` of §15.275 is
/// held to its first line, as [`generated_by_rulec`] reads it.
fn extra_by_rulec(p: &str) -> bool {
    let Ok(text) = ritsu_base::fs::read_to_string(p) else { return false };
    text.lines().any(|l| {
        ["// ", "# ", "-- "].iter().filter_map(|c| l.trim().strip_prefix(c)).any(|l| l.starts_with("Code generated by rulec ") && l.ends_with("DO NOT EDIT."))
    })
}

/// What `gen` says of the `.proto` it wrote for a rule before the rule implemented a contract's
/// service, found still there (§15.275): it is generated no more, and may be deleted.
fn left_behind(p: &str, rule: &str) -> String {
    tr!(
        "もう生成しないファイルが残っています: {p}\n注記: `{rule}` は契約のサービスを実装する（`implements`）ので、gen は規則の `.proto` をもう生成しません。gen は出力先のファイルを消さないので、このファイルは消してかまいません",
        "generated no more, and still there: {p}\nnote: `{rule}` implements a contract's service (`implements`), so gen writes no `.proto` of the rule's own; gen deletes nothing in the output directory, and this file can be deleted"
    )
}


/// What `rulec gen` says of NumPy for an `enums` file's `prerequisites` (§15.222): the file has no
/// plan there, and the plan of a rule carries its own blocks.
fn prereq_not_here(name: &str) -> String {
    tr!(
        "{name}: `enums` のファイルの前提のつながりは、この言語には生成しません。列挙の値は規則の計画が持ち、前提は規則に書いた塊だけを持ちます",
        "{name}: an `enums` file's `prerequisites` are not generated for this target: a rule's plan holds the values of its enums, and the blocks the rule itself writes"
    )
}

/// Why a target writes no function that reads the inputs out of the caller's object (§15.125,
/// §15.223), by its id: what `rulec gen` says, and what `rulec api` lists under
/// `projection.without`.
pub(crate) fn proj_why(id: &str) -> String {
    match id {
        "sql" => tr!("平たい列の関係を受け取るので、取り出す相手のオブジェクトがありません", "it takes a relation of flat columns, and there is no object to read from"),
        "numpy" => tr!("列を受け取るので、取り出す相手のオブジェクトがありません", "it takes columns, and there is no object to read from"),
        "wasm" => tr!(
            "入口は規則自身の入力の JSON を一つ受け取るので、取り出す相手のオブジェクトがありません",
            "its one entry takes a JSON object of the rule's own inputs, and there is no object to read from"
        ),
        _ => tr!(
            "呼び出し側のオブジェクトを型で持つ言語で、その型を生成するか、呼び出し側の型に合わせて書くことになります",
            "it holds the caller's object as a type, which would have to be generated or followed"
        ),
    }
}

/// Why a target writes no function that counts the answers over the random inputs (§15.225), by
/// its id: what `rulec gen` says, and what `rulec api` lists under `chances.without`.
pub(crate) fn chances_why(id: &str) -> String {
    match id {
        "sql" => tr!(
            "点を回す問い合わせにすると、問い合わせの枝と規則の行が 1:1 で対応しなくなります",
            "a query that walked the points would give up the one-to-one of its branches and the rule's rows"
        ),
        "numpy" => tr!("並びをたどる規則は、この言語には生成しません", "a rule that walks a sequence is not generated for this target"),
        _ => tr!("入口が一つで、二つ目の関数を置く場所がありません", "it has one entry, and no room for a second function"),
    }
}

/// What `rulec gen` says of a target that gets no function that counts the answers (§15.225).
fn chances_skipped(name: &str, id: &str, alias: &str) -> String {
    let why = chances_why(id);
    tr!(
        "{name}: この言語には、答えの確率を数える関数（`{alias}_chances`）を生成しません。{why}",
        "{name}: the function that counts the chances of the answers (`{alias}_chances`) is not generated for this target: {why}"
    )
}

/// What `rulec gen` says of a target that gets no function reading the inputs out of the caller's
/// object (§15.125, §15.223).
fn proj_skipped(name: &str, id: &str, alias: &str) -> String {
    let why = proj_why(id);
    tr!(
        "{name}: この言語には、呼び出し側のオブジェクトから入力を取り出す関数（`{alias}_from`）を生成しません。{why}",
        "{name}: the function that reads the inputs out of the caller's object (`{alias}_from`) is not generated for this target: {why}"
    )
}

/// What `rulec gen` says of a target that gets no functions for a `prerequisites` block (§15.222).
fn prereq_skipped(name: &str) -> String {
    tr!(
        "{name}: この言語には前提のつながり（`prerequisites`）の関数を生成しません。入口が一つで、二つ目の関数を置く場所がありません",
        "{name}: the functions of `prerequisites` are not generated for this target: it has one entry, and no room for a second function"
    )
}

/// What `generate` wrote, found stale or found missing, and its exit code so far; every path it
/// was to write, written or not; and the files of an earlier run it found still there (§15.275).
#[derive(Default)]
struct Emitted {
    dirty: u8,
    written: Vec<String>,
    stale: Vec<String>,
    missing: Vec<String>,
    planned: std::collections::BTreeSet<String>,
    left: Vec<String>,
}

/// Writes what changed, or under `--check` says what is stale. `Some` is the exit code of a write
/// that failed.
fn emit(targets: Vec<(String, String)>, check_only: bool, json: bool, st: &mut Emitted, out: &mut dyn std::io::Write, err: &mut dyn std::io::Write) -> Option<u8> {
    for (p, body) in targets {
        st.planned.insert(p.clone());
        let existing = ritsu_base::fs::read_to_string(&p).ok();
        if existing.as_deref() == Some(body.as_str()) {
            continue;
        }
        if check_only {
            if existing.is_none() { st.missing.push(p.clone()) } else { st.stale.push(p.clone()) }
            if !json {
                let _ = writeln!(out, "{}", crate::i18n::said(|| tr!("生成物が古いか手で編集されています: {p}", "generated file is stale or hand-edited: {p}")));
            }
            st.dirty = 1;
            continue;
        }
        // what stops the run, as every command of ritsu says it (ritsu's DESIGN 6.2, item 10)
        if let Some(dir) = std::path::Path::new(&p).parent()
            && let Err(e) = ritsu_base::fs::create_dir_all(dir)
        {
            return Some(crate::i18n::refuse(err, &ritsu_base::cli::cannot_create(&dir.to_string_lossy(), &e)));
        }
        if let Err(e) = ritsu_base::fs::write(&p, &body) {
            return Some(crate::i18n::refuse(err, &ritsu_base::cli::cannot_write(&p, &e)));
        }
        st.written.push(p.clone());
        if !json {
            let _ = writeln!(out, "{}", crate::i18n::said(|| tr!("生成しました: {p}", "generated: {p}")));
        }
    }
    None
}
