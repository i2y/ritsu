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
//! each of its transfers.

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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EnumId {
    pub owner: Owner,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RecId {
    /// A record a module declares.
    Module { file: PathBuf, name: String },
    /// The outputs of a rule (`charges.outputs`).
    Outputs(PathBuf),
    /// An element of a list a rule walks (`shipping.lines`).
    Element(PathBuf, String),
    /// An element of what `running_totals` answers: the item, and the weight (DESIGN 3.6).
    Running(Box<Ty>, Box<Ty>),
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
    Opt(Box<Ty>),
    Transfer(TransferId),
    /// The literal `none`, before it meets a `T?`.
    None,
    /// The literal `[]`, before it meets a list.
    Empty,
    /// What a check has said is wrong already: it agrees with everything, so that one mistake is
    /// said once.
    Error,
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
