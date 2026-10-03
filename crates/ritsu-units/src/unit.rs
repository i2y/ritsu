//! A unit (DESIGN 5.1): its dimension, the spelling it was written with, whether an amount is
//! with tax or without, and the step one integer on the wire counts.
//!
//! The spelling is rulec's — `money[円, incl_tax]`, `mass[kg]`, `rate[step 0.1%]`, `number` —
//! and dandori writes its types the same way. A unit keeps the spelling it was written with
//! (`JPY` stays `JPY`), because what a tool prints must not change with how it reads; whether two
//! units are the same is [`Unit::same`], which reads the table: `money[JPY, incl_tax]` and
//! `money[円, incl_tax]` are the same unit, `mass[kg]` and `mass[g]` are not.

use crate::Rat;
use crate::table;
use ritsu_base::text::Text;
use ritsu_base::tr;

/// What a quantity is a quantity of. A currency is a dimension of its own, so two currencies are
/// as unrelated as grams and centimetres.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Dim {
    Mass,
    Length,
    Area,
    Volume,
    Duration,
    /// Ordered, with no arithmetic.
    Temperature,
    /// Ordered, with no arithmetic.
    Sound,
    /// An amount of one currency, named as the table names it: `円`, or an ISO 4217 code.
    Money(String),
    /// A fraction with no unit: 12% is 3/25.
    Rate,
    /// A whole number with no unit: a count, a number of days, a score (rulec's `number`).
    Number,
    /// A count of things that has a name and nothing else (chobo's `unit 個`): the same only as
    /// itself, so that seats are not counted as items.
    Count(String),
}

/// The dimensions a quantity's type names with a word of its own: `mass[kg]`, `length[cm]`.
const QUANTITY_WORDS: [(&str, Dim); 7] = [
    ("mass", Dim::Mass),
    ("length", Dim::Length),
    ("area", Dim::Area),
    ("volume", Dim::Volume),
    ("duration", Dim::Duration),
    ("temperature", Dim::Temperature),
    ("sound", Dim::Sound),
];

impl Dim {
    /// The word a type of this dimension starts with: `mass`, `money`, `rate`, `number`; a
    /// count's name.
    pub fn word(&self) -> &str {
        match self {
            Dim::Mass => "mass",
            Dim::Length => "length",
            Dim::Area => "area",
            Dim::Volume => "volume",
            Dim::Duration => "duration",
            Dim::Temperature => "temperature",
            Dim::Sound => "sound",
            Dim::Money(_) => "money",
            Dim::Rate => "rate",
            Dim::Number => "number",
            Dim::Count(n) => n,
        }
    }

    /// The dimension of a quantity's word (`mass`), for the seven that have one.
    pub fn of_word(w: &str) -> Option<Dim> {
        QUANTITY_WORDS.iter().find(|(x, _)| *x == w).map(|(_, d)| d.clone())
    }

    /// The words of the seven quantities, in the table's order.
    pub fn quantity_words() -> [&'static str; 7] {
        QUANTITY_WORDS.map(|(w, _)| w)
    }

    /// One string for the dimension, the way rulec keys it: `money/円` for a currency (each its
    /// own), `count/席` for a count, else its word.
    pub fn key(&self) -> String {
        match self {
            Dim::Money(c) => format!("money/{c}"),
            Dim::Count(n) => format!("count/{n}"),
            d => d.word().to_string(),
        }
    }

    /// Whether the dimension is ordered but has no arithmetic (rulec's §15.84): a temperature is
    /// a scale with a displaced zero, and a decibel is a logarithm.
    pub fn compares_only(&self) -> bool {
        matches!(self, Dim::Temperature | Dim::Sound)
    }
}

/// Whether an amount of money is with tax or without. Two amounts that differ in this are not the
/// same amount, and no conversion turns one into the other (it would take a tax rate).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tax {
    Incl,
    Excl,
}

impl Tax {
    pub fn word(self) -> &'static str {
        match self {
            Tax::Incl => "incl_tax",
            Tax::Excl => "excl_tax",
        }
    }

    pub fn parse(w: &str) -> Option<Tax> {
        match w {
            "incl_tax" => Some(Tax::Incl),
            "excl_tax" => Some(Tax::Excl),
            _ => None,
        }
    }
}

/// A unit: the dimension, the spelling, the tax, and the step.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Unit {
    pub dim: Dim,
    /// The unit as written (`JPY`, `円`, `kg`); empty for a rate and a number, the name for a
    /// count.
    pub unit: String,
    /// For money: with tax or without, when the type says.
    pub tax: Option<Tax>,
    /// What one integer on the wire counts, as a fraction of the unit: a rate's step
    /// (`rate[step 0.1%]` is 1/1000). None where an integer counts whole units.
    pub step: Option<Rat>,
}

/// Why a spelling is not a unit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// Not of the shape `<word>[…]`, `rate` or `number`.
    Shape(String),
    /// The word before `[` is not a dimension.
    Dimension(String),
    /// A unit the table does not have.
    Unknown(String),
    /// A unit of another dimension than the type's (`mass[cm]`).
    OtherDimension { unit: String, dim: String },
    /// After the currency, a word that is not `incl_tax` or `excl_tax`.
    Tax(String),
    /// A rate's step that is not a positive percentage (`rate[step 0.1%]`).
    Step(String),
}

impl Problem {
    pub fn text(&self) -> Text {
        match self {
            Problem::Shape(s) => tr!("`{s}` は単位の書き方ではありません（`money[円, incl_tax]`、`mass[kg]`、`rate[step 1%]`、`number` のように書きます）", "`{s}` is not how a unit is written (write `money[円, incl_tax]`, `mass[kg]`, `rate[step 1%]` or `number`)"),
            Problem::Dimension(d) => tr!("`{d}` は次元ではありません", "`{d}` is not a dimension"),
            Problem::Unknown(u) => tr!("`{u}` は単位ではありません", "`{u}` is not a unit"),
            Problem::OtherDimension { unit, dim } => tr!("`{unit}` は {dim} の単位ではありません", "`{unit}` is not a unit of {dim}"),
            Problem::Tax(t) => tr!("`{t}` は税込（incl_tax）か税抜（excl_tax）ではありません", "`{t}` is neither incl_tax nor excl_tax"),
            Problem::Step(s) => tr!("刻み `{s}` は正の百分率ではありません", "the step `{s}` is not a positive percentage"),
        }
    }
}

impl Unit {
    /// An amount of money in a currency's unit (`JPY`, `円`, `銭`, `USDc`); None when the table has
    /// no such currency.
    pub fn money(unit: &str, tax: Option<Tax>) -> Option<Unit> {
        let (cur, _) = table::money(unit)?;
        Some(Unit { dim: Dim::Money(cur), unit: unit.to_string(), tax, step: None })
    }

    /// A quantity of a dimension (`mass`) in one of its units (`kg`); None when the word is not a
    /// dimension or the unit is not one of its.
    pub fn quantity(dim: &str, unit: &str) -> Option<Unit> {
        let d = Dim::of_word(dim)?;
        let (ud, _) = table::unit(unit)?;
        (ud == d).then(|| Unit { dim: d, unit: unit.to_string(), tax: None, step: None })
    }

    /// A rate, with the step its integer counts.
    pub fn rate(step: Option<Rat>) -> Unit {
        Unit { dim: Dim::Rate, unit: String::new(), tax: None, step }
    }

    /// A whole number with no unit.
    pub fn number() -> Unit {
        Unit { dim: Dim::Number, unit: String::new(), tax: None, step: None }
    }

    /// A count of things with a name and nothing else.
    pub fn count(name: &str) -> Unit {
        Unit { dim: Dim::Count(name.to_string()), unit: name.to_string(), tax: None, step: None }
    }

    /// Read a unit as rulec spells it: `money[円]`, `money[JPY, incl_tax]` (with or without the
    /// space), `mass[kg]`, `rate`, `rate[step 0.1%]`, `number`.
    pub fn parse(s: &str) -> Result<Unit, Problem> {
        let t = s.trim();
        match t {
            "number" => return Ok(Unit::number()),
            "rate" => return Ok(Unit::rate(None)),
            _ => {}
        }
        let shape = || Problem::Shape(s.to_string());
        let (word, rest) = t.split_once('[').ok_or_else(shape)?;
        let inner = rest.strip_suffix(']').ok_or_else(shape)?;
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        match word.trim() {
            "money" => {
                let (cur, tax) = match parts.as_slice() {
                    [c] => (*c, None),
                    [c, t] => (*c, Some(Tax::parse(t).ok_or_else(|| Problem::Tax(t.to_string()))?)),
                    _ => return Err(shape()),
                };
                Unit::money(cur, tax).ok_or_else(|| Problem::Unknown(cur.to_string()))
            }
            "rate" => {
                let [p] = parts.as_slice() else { return Err(shape()) };
                let step = p.strip_prefix("step").map(str::trim).ok_or_else(shape)?;
                Ok(Unit::rate(Some(percent(step).ok_or_else(|| Problem::Step(step.to_string()))?)))
            }
            w => {
                let d = Dim::of_word(w).ok_or_else(|| Problem::Dimension(w.to_string()))?;
                let [u] = parts.as_slice() else { return Err(shape()) };
                match table::unit(u) {
                    None => Err(Problem::Unknown(u.to_string())),
                    Some((ud, _)) if ud != d => Err(Problem::OtherDimension { unit: u.to_string(), dim: w.to_string() }),
                    Some(_) => Ok(Unit { dim: d, unit: u.to_string(), tax: None, step: None }),
                }
            }
        }
    }

    /// The same unit in the table's own spelling: `JPY` is `円`.
    pub fn canonical(&self) -> Unit {
        match &self.dim {
            Dim::Money(cur) if self.unit == "JPY" => Unit { unit: cur.clone(), ..self.clone() },
            _ => self.clone(),
        }
    }

    /// The factor to the dimension's base unit: 1000 for `kg` (in grams), 1/100 for `銭` (in yen).
    pub fn factor(&self) -> Rat {
        match &self.dim {
            Dim::Rate | Dim::Number | Dim::Count(_) => Rat::int(1),
            _ => table::unit(&self.unit).map(|(_, f)| f).unwrap_or(Rat::int(1)),
        }
    }

    /// What the unit adds after its factor to reach the base unit (only ℉ adds anything).
    pub fn offset(&self) -> Rat {
        table::offset(&self.unit)
    }

    /// Whether the two are the same unit: the same dimension, the same size (`JPY` and `円`), the
    /// same tax, and the same step.
    pub fn same(&self, o: &Unit) -> bool {
        self.dim == o.dim && self.factor() == o.factor() && self.offset() == o.offset() && self.tax == o.tax && self.step == o.step
    }

    /// `v` counted in this unit, counted in `to`, exactly; None when the two are of different
    /// dimensions or taxes, which no conversion joins.
    pub fn convert(&self, v: Rat, to: &Unit) -> Option<Rat> {
        if self.dim != to.dim || self.tax != to.tax {
            return None;
        }
        let base = v.mul(self.factor()).add(self.offset());
        Some(base.sub(to.offset()).div(to.factor()))
    }

    /// [`Unit::convert`], when the value lands on a whole number of `to`: `1.5kg` is 1500 in
    /// grams, and `1lb` is no whole number of grams, so None.
    pub fn whole(&self, v: Rat, to: &Unit) -> Option<Rat> {
        self.convert(v, to).filter(|r| r.is_int())
    }

    /// Whether values of the unit are ordered but have no arithmetic.
    pub fn compares_only(&self) -> bool {
        self.dim.compares_only()
    }
}

/// `0.1%` as the fraction it is (1/1000); None for what is not a positive percentage.
fn percent(s: &str) -> Option<Rat> {
    let n = s.strip_suffix('%')?.trim();
    let (whole, frac) = n.split_once('.').unwrap_or((n, ""));
    if (whole.is_empty() && frac.is_empty()) || !whole.chars().chain(frac.chars()).all(|c| c.is_ascii_digit()) || frac.len() > 18 {
        return None;
    }
    let digits: i128 = format!("{whole}{frac}").parse().ok()?;
    let r = Rat::checked_new(digits, 10i128.checked_pow(frac.len() as u32)?.checked_mul(100)?)?;
    (r.num > 0).then_some(r)
}

impl std::fmt::Display for Unit {
    /// rulec's spelling, with the unit as written: `money[JPY, incl_tax]`, `mass[kg]`,
    /// `rate[step 0.1%]`, `number`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.dim, self.tax, self.step) {
            (Dim::Money(_), Some(t), _) => write!(f, "money[{}, {}]", self.unit, t.word()),
            (Dim::Money(_), None, _) => write!(f, "money[{}]", self.unit),
            (Dim::Rate, _, Some(step)) => write!(f, "rate[step {}%]", step.mul(Rat::int(100))),
            (Dim::Rate, _, None) => write!(f, "rate"),
            (Dim::Number, ..) => write!(f, "number"),
            (Dim::Count(n), ..) => write!(f, "{n}"),
            (d, ..) => write!(f, "{}[{}]", d.word(), self.unit),
        }
    }
}
