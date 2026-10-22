//! The types of kaname (DESIGN 3.1), as the checks give them to expressions and declarations.
//!
//! A number's type is rulec's: a dimension and a unit with its tax (ritsu-units' [`Unit`]), and the
//! step every value of it is a whole multiple of ([`NumTy::step`], rulec's lattice of §15.190):
//! 1 for an amount in whole units, the declared step of a rate, `0.0349` for an amount in whole
//! cents times `3.49%`. A value is rounded where it goes when its step is a whole multiple of the
//! step of the place (a whole unit; a rate's declared step), and unrounded otherwise (DESIGN 3.2):
//! the product of an amount and a rate, a quotient by a value. A constant (a literal, or literals
//! put together) carries its value, and takes the unit and the tax of what it meets (`15lb` beside
//! a `mass[oz]` is 240 ounces).
//!
//! An enum, a record and a transfer are told apart by where they are declared (DESIGN 3.5, 6.3):
//! the file, by its path on the disk, and the name. Two modules that read one book have one type of
//! each of its transfers. An enum whose values have fields is a sum (DESIGN 23.1): its type is the
//! enum's, and each of its values has a type of its own, the record of the value's fields
//! ([`RecId::Variant`]), whose values go where the enum's go.

use ritsu_units::{Dim, Rat, Unit};
use std::path::PathBuf;

/// A numeric type: `number`, a rate, money, a quantity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NumTy {
    /// The dimension, the unit as written and the tax. A rate's unit holds no step: what its
    /// values land on is [`NumTy::step`].
    pub unit: Unit,
    /// The step every value is a whole multiple of, counted in `unit`: zero for a value that is
    /// always zero, None where it is not known (a quotient by a value).
    pub step: Option<Rat>,
    /// The value, for a constant: a literal, or literals put together. Its unit and tax follow
    /// what it meets.
    pub konst: Option<Rat>,
    /// A quotient of two values of one unit (DESIGN 3.2): a `number` or a rate, as the rounding or
    /// the place it goes to says.
    pub ratio: bool,
}

impl NumTy {
    /// A value of a declared type: whole in its unit, or on its step.
    pub fn of(unit: Unit) -> NumTy {
        let step = unit.step.unwrap_or(Rat::int(1));
        NumTy { unit: Unit { step: None, ..unit }, step: Some(step), konst: None, ratio: false }
    }

    /// A number with no unit, whole.
    pub fn number() -> NumTy {
        NumTy::of(Unit::number())
    }

    pub fn is_rate(&self) -> bool {
        self.unit.dim == Dim::Rate
    }

    pub fn is_number(&self) -> bool {
        self.unit.dim == Dim::Number
    }

    /// Money or a quantity: a value that carries a unit.
    pub fn has_unit(&self) -> bool {
        !matches!(self.unit.dim, Dim::Rate | Dim::Number)
    }
}

/// Where a declared enum, record or function lives: a module, or a rule (the file on the disk).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Owner {
    Module(PathBuf),
    Rule(PathBuf),
}

/// An enum, by where it is declared and its name; and, for a generic enum (DESIGN 26.1), the types its
/// type parameters take in this type (`Outcome[number]`), in their order.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EnumId {
    pub owner: Owner,
    pub name: String,
    pub args: Vec<Ty>,
}

impl EnumId {
    /// An enum that takes no type arguments.
    pub fn plain(owner: Owner, name: impl Into<String>) -> EnumId {
        EnumId { owner, name: name.into(), args: Vec::new() }
    }

    /// The same declaration, whatever its type arguments.
    pub fn same_decl(&self, o: &EnumId) -> bool {
        self.owner == o.owner && self.name == o.name
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RecId {
    /// A record a module declares, and the types its type parameters take in it (DESIGN 26.1).
    Module { file: PathBuf, name: String, args: Vec<Ty> },
    /// The type of a value of an enum a module declares whose values have fields (`Order.placed`,
    /// DESIGN 23.2): the record of the value's fields, a value of which is a value of the enum. By the
    /// module's file, the enum and the value, as they are declared, and the enum's type arguments.
    Variant { file: PathBuf, name: String, value: String, args: Vec<Ty> },
    /// The outputs of a rule (`charges.outputs`).
    Outputs(PathBuf),
    /// An element of a list a rule walks (`shipping.lines`).
    Element(PathBuf, String),
    /// An element of what `running_totals` answers: the item, and the weight (DESIGN 3.6).
    Running(Box<Ty>, Box<Ty>),
    /// An element of what `d.entries()` answers, and what `d.filter` gives its function: the key and
    /// the value (`entry[K, V]`, DESIGN 24.3).
    Entry(Box<Ty>, Box<Ty>),
    /// An element of what `xs.indexed()` answers: the index from 0, and the item (`indexed[T]`).
    Indexed(Box<Ty>),
}

impl RecId {
    /// The enum the type of a value of a sum is of (`Order.placed` is of `Order`); None for a record.
    pub fn sum(&self) -> Option<EnumId> {
        match self {
            RecId::Variant { file, name, args, .. } => Some(EnumId { owner: Owner::Module(file.clone()), name: name.clone(), args: args.clone() }),
            _ => None,
        }
    }

    /// A record of a module that takes no type arguments.
    pub fn record(file: PathBuf, name: impl Into<String>) -> RecId {
        RecId::Module { file, name: name.into(), args: Vec::new() }
    }

    /// The type of the value `value` of the enum `id` (its type arguments too).
    pub fn variant(id: &EnumId, value: impl Into<String>) -> Option<RecId> {
        match &id.owner {
            Owner::Module(file) => Some(RecId::Variant { file: file.clone(), name: id.name.clone(), value: value.into(), args: id.args.clone() }),
            Owner::Rule(_) => None,
        }
    }
}

/// An operation of a transfer of a book: `ledger.capture.do`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TransferId {
    pub book: PathBuf,
    pub transfer: String,
    pub op: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ty {
    Num(NumTy),
    Bool,
    Str,
    Date,
    Enum(EnumId),
    Record(RecId),
    List(Box<Ty>),
    /// `dict[K, V]` (DESIGN 24.2): its keys of a type that can be a key ([`is_key`]).
    Dict(Box<Ty>, Box<Ty>),
    /// `set[T]`: its members of a type that can be a key.
    Set(Box<Ty>),
    Opt(Box<Ty>),
    Transfer(TransferId),
    /// The literal `none`, before it meets a `T?`.
    None,
    /// The literal `[]`, before it meets a list.
    Empty,
    /// The literal `[:]`, before it meets a dict.
    EmptyDict,
    /// A type parameter of the generic function, record or enum it is written in (DESIGN 26.1), by its
    /// declared name: a value of it is passed on, held and compared with `=`, and nothing more.
    Param(String),
    /// `result[T, E]` (DESIGN 26.3): `ok` with a value of `T`, or `err` with a value of `E`.
    Result(Box<Ty>, Box<Ty>),
    /// A function type, `(T, Adjustment) -> T` (DESIGN 27.1): the types of the parameters, the type of
    /// the answer, and the effect written on it, which a function given there keeps within (E402). A
    /// value of it is passed, named by a `let` and called, and goes nowhere else (E113).
    Fn(Vec<Ty>, Box<Ty>, Option<FnEffect>),
    /// What a check has said is wrong already: it agrees with everything, so that one mistake is
    /// said once.
    Error,
}

/// The effect written on a function type (`(T) -> U total`, `(T) -> U fails range`, DESIGN 27.1): the
/// kinds of failure it may have, in their order, and whether it may not stop. `total` has neither.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FnEffect {
    pub fails: Vec<crate::ast::FailKind>,
    pub diverges: bool,
}

impl FnEffect {
    /// The effect as written on a function's head or a function type.
    pub fn of(e: &crate::ast::Effect) -> FnEffect {
        let mut fails: Vec<crate::ast::FailKind> = e.fails.iter().map(|(k, _)| *k).collect();
        fails.sort();
        fails.dedup();
        FnEffect { fails, diverges: e.diverges }
    }

    /// As kaname writes it after a type: `total`, `fails range, overflow`, `diverges`.
    pub fn shown(&self) -> String {
        let kinds = self.fails.iter().map(|k| k.word()).collect::<Vec<_>>().join(", ");
        match (kinds.is_empty(), self.diverges) {
            (true, false) => "total".into(),
            (true, true) => "diverges".into(),
            (false, false) => format!("fails {kinds}"),
            (false, true) => format!("fails {kinds} diverges"),
        }
    }
}

impl Ty {
    pub fn is_error(&self) -> bool {
        matches!(self, Ty::Error)
    }

    /// The type of a value that is there: the type itself, or the one an optional wraps.
    pub fn present(&self) -> &Ty {
        match self {
            Ty::Opt(t) => t,
            t => t,
        }
    }

    pub fn num(&self) -> Option<&NumTy> {
        match self {
            Ty::Num(n) => Some(n),
            _ => None,
        }
    }

    /// Whether a function type is in the type, at any depth: what holds a function value (DESIGN 27.1).
    pub fn has_fn(&self) -> bool {
        match self {
            Ty::Fn(..) => true,
            Ty::List(x) | Ty::Set(x) | Ty::Opt(x) => x.has_fn(),
            Ty::Dict(a, b) | Ty::Result(a, b) => a.has_fn() || b.has_fn(),
            Ty::Enum(id) => id.args.iter().any(Ty::has_fn),
            Ty::Record(r) => match r {
                RecId::Module { args, .. } | RecId::Variant { args, .. } => args.iter().any(Ty::has_fn),
                RecId::Running(a, b) | RecId::Entry(a, b) => a.has_fn() || b.has_fn(),
                RecId::Indexed(x) => x.has_fn(),
                RecId::Outputs(_) | RecId::Element(..) => false,
            },
            _ => false,
        }
    }

    /// Whether a type parameter is in the type.
    pub fn has_param(&self) -> bool {
        match self {
            Ty::Param(_) => true,
            Ty::List(x) | Ty::Set(x) | Ty::Opt(x) => x.has_param(),
            Ty::Dict(a, b) | Ty::Result(a, b) => a.has_param() || b.has_param(),
            Ty::Fn(ps, r, _) => ps.iter().any(Ty::has_param) || r.has_param(),
            Ty::Enum(id) => id.args.iter().any(Ty::has_param),
            Ty::Record(r) => match r {
                RecId::Module { args, .. } | RecId::Variant { args, .. } => args.iter().any(Ty::has_param),
                RecId::Running(a, b) | RecId::Entry(a, b) => a.has_param() || b.has_param(),
                RecId::Indexed(x) => x.has_param(),
                RecId::Outputs(_) | RecId::Element(..) => false,
            },
            _ => false,
        }
    }
}

/// The type with each type parameter named in `params` put to the type at the same place in `args`
/// (DESIGN 26.2): the fields of `Page[Invoice]` are those of `Page` with `T` put to `Invoice`. Every
/// parameter is put at once, so a parameter of the same name in an argument is not put again.
pub fn subst(t: &Ty, params: &[String], args: &[Ty]) -> Ty {
    if params.is_empty() || !t.has_param() {
        return t.clone();
    }
    let s = |x: &Ty| subst(x, params, args);
    let b = |x: &Ty| Box::new(subst(x, params, args));
    match t {
        Ty::Param(n) => match params.iter().position(|p| p == n) {
            Some(i) => args.get(i).cloned().unwrap_or(Ty::Error),
            None => t.clone(),
        },
        Ty::List(x) => Ty::List(b(x)),
        Ty::Set(x) => Ty::Set(b(x)),
        Ty::Opt(x) => Ty::Opt(b(x)),
        Ty::Dict(k, v) => Ty::Dict(b(k), b(v)),
        Ty::Result(a, e) => Ty::Result(b(a), b(e)),
        Ty::Fn(ps, r, eff) => Ty::Fn(ps.iter().map(s).collect(), b(r), eff.clone()),
        Ty::Enum(id) => Ty::Enum(EnumId { args: id.args.iter().map(s).collect(), ..id.clone() }),
        Ty::Record(r) => Ty::Record(match r {
            RecId::Module { file, name, args: a } => RecId::Module { file: file.clone(), name: name.clone(), args: a.iter().map(s).collect() },
            RecId::Variant { file, name, value, args: a } => RecId::Variant { file: file.clone(), name: name.clone(), value: value.clone(), args: a.iter().map(s).collect() },
            RecId::Running(a, w) => RecId::Running(b(a), b(w)),
            RecId::Entry(k, v) => RecId::Entry(b(k), b(v)),
            RecId::Indexed(x) => RecId::Indexed(b(x)),
            other => other.clone(),
        }),
        other => other.clone(),
    }
}

/// A function type as kaname writes it (DESIGN 27.1): `(Line, number) -> money[USDc] total`, from the
/// types of its parameters and its answer as written.
pub fn fn_text(params: &[String], ret: &str, eff: Option<&FnEffect>) -> String {
    let eff = eff.map(|e| format!(" {}", e.shown())).unwrap_or_default();
    format!("({}) -> {ret}{eff}", params.join(", "))
}

/// The same, a slot's type put so; its range and its count are kept.
pub fn subst_slot(s: &Slot, params: &[String], args: &[Ty]) -> Slot {
    Slot { ty: subst(&s.ty, params, args), ..s.clone() }
}

/// The ends of a declared range or count, both in, in the unit of what it holds (a date as its day
/// number); None at an open end.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Range {
    pub min: Option<Rat>,
    pub max: Option<Rat>,
}

/// A place a value is put in (DESIGN 0.5): a parameter, a field, a function's result, an input
/// of a rule or a dates file, an argument of a transfer. Its type, and what was declared of it:
/// the range of the numbers or the dates it holds (of the elements, for a list of them) and the
/// count of a list.
#[derive(Clone, Debug, PartialEq)]
pub struct Slot {
    pub ty: Ty,
    pub range: Option<Range>,
    pub count: Option<Range>,
}

impl Slot {
    pub fn of(ty: Ty) -> Slot {
        Slot { ty, range: None, count: None }
    }
}

// ── The whole of a type ──────────────────────────────────────────────────────

/// The least value of int64 and the greatest: what a number, an amount or a quantity whose range is
/// not written takes, counted in its unit (DESIGN 22.1).
pub const INT_MIN: i128 = i64::MIN as i128;
pub const INT_MAX: i128 = i64::MAX as i128;

/// The range a value of `t` takes where none is written (DESIGN 22.1): the whole of int64, in its
/// unit, for a number, an amount and a quantity; every day from 0001-01-01 to 9999-12-31 for a date.
/// None for a rate, whose step is its range's grid (a rate with a step takes 0% to 100% where a
/// parameter reaches it, DESIGN 3.9), and for what holds no number.
pub fn whole_range(t: &Ty) -> Option<Range> {
    match t {
        Ty::Num(n) if !n.is_rate() => Some(Range { min: Some(Rat::int(INT_MIN)), max: Some(Rat::int(INT_MAX)) }),
        Ty::Date => Some(Range { min: Some(Rat::int(crate::date::MIN as i128)), max: Some(Rat::int(crate::date::MAX as i128)) }),
        _ => None,
    }
}

/// Whether values of `t` can be keys (DESIGN 24.1): of a dict, of a set, of an order. A number,
/// an amount, a quantity, a rate, a date, a string, a bool, a value of an enum whose values have no
/// fields (a rule's too). What a key is held to besides (a number rounded to its step) is the check's.
pub fn is_key(t: &Ty, sum: impl Fn(&EnumId) -> bool) -> bool {
    match t {
        Ty::Num(_) | Ty::Date | Ty::Str | Ty::Bool => true,
        Ty::Enum(id) => !sum(id),
        Ty::Error => true,
        _ => false,
    }
}

/// The count a list takes where none is written: 0 to 2⁶³ − 1 elements (DESIGN 22.1).
pub fn whole_count() -> Range {
    Range { min: Some(Rat::zero()), max: Some(Rat::int(INT_MAX)) }
}

/// A range as a place holds a value to it where a parameter reaches the value (DESIGN 22.1): the
/// ends written, and the type's where an end is not written; None where the type has no whole (a rate,
/// what holds no number) and nothing is written.
pub fn filled(written: Option<&Range>, whole: Option<Range>) -> Option<Range> {
    match (written, whole) {
        (Some(r), Some(w)) => Some(Range { min: r.min.or(w.min), max: r.max.or(w.max) }),
        (Some(r), None) => Some(r.clone()),
        (None, w) => w,
    }
}

// ── Steps ────────────────────────────────────────────────────────────────────

fn gcd(mut x: i128, mut y: i128) -> i128 {
    while y != 0 {
        let t = x % y;
        x = y;
        y = t;
    }
    x.abs()
}

/// The largest step two positive steps are both whole multiples of: a sixth and a fourth share a
/// twelfth (rulec's `common_step`). None where the arithmetic does not fit.
pub fn common_step(a: Rat, b: Rat) -> Option<Rat> {
    let n = gcd(a.num.checked_mul(b.den)?.checked_abs()?, b.num.checked_mul(a.den)?.checked_abs()?);
    if n == 0 {
        return None;
    }
    Rat::checked_new(n, a.den.checked_mul(b.den)?)
}

fn abs(r: Rat) -> Rat {
    if r.num < 0 { Rat { num: -r.num, den: r.den } } else { r }
}

/// The step of a sum of two values on these steps: the one both share; zero is what a sum leaves
/// out.
pub fn share(a: Option<Rat>, b: Option<Rat>) -> Option<Rat> {
    let (a, b) = (a?, b?);
    if a.num == 0 {
        Some(abs(b))
    } else if b.num == 0 {
        Some(abs(a))
    } else {
        common_step(abs(a), abs(b))
    }
}

/// The step of a product.
pub fn times(a: Option<Rat>, b: Option<Rat>) -> Option<Rat> {
    abs(a?).checked_mul(abs(b?))
}

/// Whether a value on step `got` is on the step `want`: every multiple of `got` is a multiple of
/// `want`.
pub fn on_step(got: Option<Rat>, want: Rat) -> bool {
    match got {
        None => false,
        Some(g) if g.num == 0 => true,
        Some(g) => want.num != 0 && g.checked_div(want).is_some_and(|q| q.is_int()),
    }
}

/// The step a value of `unit` lands on when it is a whole number of it: a rate's declared step, or
/// one unit.
pub fn unit_step(unit: &Unit) -> Rat {
    unit.step.unwrap_or(Rat::int(1))
}

/// Whether two units are the same unit (DESIGN 3.2): the same dimension, size and tax. A rate's
/// step is not part of it (the step is the value's, [`NumTy::step`]).
pub fn same_unit(a: &Unit, b: &Unit) -> bool {
    Unit { step: None, ..a.clone() }.same(&Unit { step: None, ..b.clone() })
}

/// `v` counted in `from`, counted in `to`; None across dimensions or taxes.
pub fn convert(v: Rat, from: &Unit, to: &Unit) -> Option<Rat> {
    Unit { step: None, tax: None, ..from.clone() }.convert(v, &Unit { step: None, tax: None, ..to.clone() })
}

/// The step of values read from `from` in `to`, given `step`, their step there times the factor,
/// and `konst`, a constant's value read there. Between two units whose zeros differ (℃ and ℉, DESIGN
/// 3.2), the shift between the zeros moves every value: a whole number of ℃ is 32℉ plus a multiple
/// of 9/5℉, so the step is the one the two share, 1/5℉; a constant is on its own value, as a literal
/// is. Between any other two units, `step`.
pub fn step_in(step: Option<Rat>, konst: Option<Rat>, from: &Unit, to: &Unit) -> Option<Rat> {
    if from.offset() == to.offset() {
        return step;
    }
    match konst {
        Some(x) => Some(abs(x)),
        None => share(step, convert(Rat::zero(), from, to)),
    }
}

/// The factor between two units of one dimension: a value counted in `from` is this many of `to`
/// per one of `from` (100 from dollars to cents).
pub fn factor(from: &Unit, to: &Unit) -> Option<Rat> {
    let one = convert(Rat::int(1), from, to)?;
    let zero = convert(Rat::zero(), from, to)?;
    Some(one.sub(zero))
}

/// A unit as kaname writes it in a message: `money[USDc]`, `money[GBP, incl_tax]`, `mass[oz]`,
/// `rate`, `number`.
pub fn unit_text(u: &Unit) -> String {
    Unit { step: None, ..u.clone() }.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_of_sums_and_products() {
        assert_eq!(share(Some(Rat::new(1, 6)), Some(Rat::new(1, 4))), Some(Rat::new(1, 12)));
        assert_eq!(share(Some(Rat::zero()), Some(Rat::int(3))), Some(Rat::int(3)));
        assert_eq!(times(Some(Rat::int(1)), Some(Rat::new(349, 10_000))), Some(Rat::new(349, 10_000)));
        assert!(on_step(Some(Rat::int(100)), Rat::int(1)));
        assert!(!on_step(Some(Rat::new(1, 2)), Rat::int(1)));
        assert!(on_step(Some(Rat::new(1, 100)), Rat::new(1, 10_000)));
        assert!(!on_step(None, Rat::int(1)));
    }

    #[test]
    fn units_convert_within_a_dimension() {
        let (usd, usdc) = (Unit::money("USD", None).unwrap(), Unit::money("USDc", None).unwrap());
        assert_eq!(factor(&usd, &usdc), Some(Rat::int(100)));
        let (lb, oz) = (Unit::quantity("mass", "lb").unwrap(), Unit::quantity("mass", "oz").unwrap());
        assert_eq!(convert(Rat::int(15), &lb, &oz), Some(Rat::int(240)));
        assert!(!same_unit(&usd, &usdc));
        assert!(same_unit(&Unit::money("JPY", None).unwrap(), &Unit::money("円", None).unwrap()));
    }
}
