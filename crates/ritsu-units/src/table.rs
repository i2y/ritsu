//! The closed table of units (DESIGN 5.1): every currency and every unit a value may carry, each
//! with the dimension it belongs to and its exact factor to that dimension's base unit. It is
//! rulec's table (`money_unit`, `unit_info`, `unit_offset` and `CURRENCIES` of its `types.rs`),
//! moved here so that every language reads the same one.
//!
//! The table is closed on purpose. With any identifier allowed, `100lbs` would pass as an
//! amount in a currency called "lbs" (rulec's §15.18).

use crate::Rat;
use crate::unit::Dim;

/// The currencies a value may be written in, besides `円`, as ISO 4217 codes. Each also has a
/// hundredth, spelled by appending `c` — `USD` and `USDc` — which is the relation `円` has to
/// `銭`, and `mass[kg]` to `mass[g]`.
pub const CURRENCIES: &[&str] = &[
    "USD", "EUR", "GBP", "CHF", "CAD", "AUD", "NZD", "CNY", "HKD", "SGD", "KRW", "INR",
    "TWD", "THB", "SEK", "NOK", "DKK", "PLN", "CZK", "HUF", "TRY", "BRL", "MXN", "ZAR",
    "AED", "SAR", "ILS", "PHP", "IDR", "MYR", "VND",
];

/// The dimensions of the quantities, with the words rulec spells them by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Mass,
    Length,
    Area,
    Volume,
    Duration,
    Temperature,
    Sound,
    Rate,
}

impl Kind {
    fn dim(self) -> Dim {
        match self {
            Kind::Mass => Dim::Mass,
            Kind::Length => Dim::Length,
            Kind::Area => Dim::Area,
            Kind::Volume => Dim::Volume,
            Kind::Duration => Dim::Duration,
            Kind::Temperature => Dim::Temperature,
            Kind::Sound => Dim::Sound,
            Kind::Rate => Dim::Rate,
        }
    }
}

/// Every unit that is not a currency: its spelling, its dimension, and its factor to the
/// dimension's base unit as numerator and denominator.
///
/// The imperial factors are exact rationals (a pound is 453.59237 g on the nose), so they cost
/// nothing in precision. What they do cost is scale: `1lb` in a column that counts grams is not a
/// whole number of grams and is refused, which is right — a pound is not writable there.
const QUANTITIES: &[(&str, Kind, i128, i128)] = &[
    ("mg", Kind::Mass, 1, 1000),
    ("g", Kind::Mass, 1, 1),
    ("kg", Kind::Mass, 1000, 1),
    ("t", Kind::Mass, 1_000_000, 1),
    ("oz", Kind::Mass, 28_349_523_125, 1_000_000_000),
    ("lb", Kind::Mass, 45_359_237, 100_000),
    ("mm", Kind::Length, 1, 10),
    ("cm", Kind::Length, 1, 1),
    ("m", Kind::Length, 100, 1),
    ("km", Kind::Length, 100_000, 1),
    ("in", Kind::Length, 254, 100),
    ("ft", Kind::Length, 3048, 100),
    ("yd", Kind::Length, 9144, 100),
    ("mi", Kind::Length, 1_609_344, 10),
    // Area, in square metres. It is a dimension of its own: two lengths do not multiply into one
    // of these (rulec's §15.83). 坪 is exact — 一間 is 六尺 and a 尺 is 10/33 m, so a 坪 is
    // (20/11)² = 400/121 m².
    ("mm2", Kind::Area, 1, 1_000_000),
    ("cm2", Kind::Area, 1, 10_000),
    ("m2", Kind::Area, 1, 1),
    ("a", Kind::Area, 100, 1),
    ("ha", Kind::Area, 10_000, 1),
    ("km2", Kind::Area, 1_000_000, 1),
    ("坪", Kind::Area, 400, 121),
    ("in2", Kind::Area, 64_516, 100_000_000),
    ("ft2", Kind::Area, 9_290_304, 100_000_000),
    ("yd2", Kind::Area, 83_612_736, 100_000_000),
    ("mi2", Kind::Area, 2_589_988_110_336, 1_000_000),
    ("ac", Kind::Area, 40_468_564_224, 10_000_000),
    // Volume, in litres. `cm3` and `mL` are the same size and so are `m3` and `kL`; both spellings
    // are kept because a water tariff writes m³ and a fire code writes L. **No gallon**: the US
    // one is 3.785411784 L and the imperial one 4.54609 L, and a unit that means two different
    // sizes is the one thing this table must not hold.
    ("mm3", Kind::Volume, 1, 1_000_000),
    ("cm3", Kind::Volume, 1, 1000),
    ("mL", Kind::Volume, 1, 1000),
    ("L", Kind::Volume, 1, 1),
    ("m3", Kind::Volume, 1000, 1),
    ("kL", Kind::Volume, 1000, 1),
    // Time, in seconds: the span a rule compares, not a calendar day (a `date` has no
    // arithmetic). A minute is `min` because `m` is the metre.
    ("ms", Kind::Duration, 1, 1000),
    ("s", Kind::Duration, 1, 1),
    ("min", Kind::Duration, 60, 1),
    ("h", Kind::Duration, 3600, 1),
    ("d", Kind::Duration, 86_400, 1),
    ("w", Kind::Duration, 604_800, 1),
    // Ordered, not arithmetic (rulec's §15.84). A ℃ is a scale with a displaced zero, so ℉
    // carries an offset as well as a factor — see `offset`. A decibel is a logarithm and has one
    // spelling, so there is nothing to convert it to.
    ("℃", Kind::Temperature, 1, 1),
    ("℉", Kind::Temperature, 5, 9),
    ("dB", Kind::Sound, 1, 1),
    ("%", Kind::Rate, 1, 100),
];

/// A money unit: the currency it belongs to, and its factor to that currency's main unit.
///
/// **Two currencies never convert into one another.** There is no exchange rate in this table
/// and there must not be one, so the currency is the dimension rather than a brand inside a
/// single "money" dimension. Adding dollars to yen is a mistake of the same kind as adding
/// grams to yen. `JPY` is another spelling of `円`, and `銭` is its hundredth.
pub fn money(u: &str) -> Option<(String, Rat)> {
    match u {
        "円" | "JPY" => Some(("円".into(), Rat::int(1))),
        "銭" => Some(("円".into(), Rat::new(1, 100))),
        _ if CURRENCIES.contains(&u) => Some((u.to_string(), Rat::int(1))),
        _ => {
            let base = u.strip_suffix('c')?;
            CURRENCIES.contains(&base).then(|| (base.to_string(), Rat::new(1, 100)))
        }
    }
}

/// A unit's dimension, and its factor to that dimension's base unit; None for a spelling the
/// table does not have.
pub fn unit(u: &str) -> Option<(Dim, Rat)> {
    if let Some((cur, f)) = money(u) {
        return Some((Dim::Money(cur), f));
    }
    QUANTITIES.iter().find(|(s, ..)| *s == u).map(|(_, k, n, d)| (k.dim(), Rat::new(*n, *d)))
}

/// What a unit adds after its factor, to reach its dimension's base unit: `base = v × f + o`.
/// Every unit but ℉ has a zero where its dimension has one, so this is 0 for all of them and a
/// conversion reduces to a scaling.
///
/// ℉ is the one entry that is not a scaling: 41℉ is exactly 5℃, and −40 is the same in both. It
/// is exact — 5/9 and −160/9 are rationals — and it is safe **because a temperature has no
/// arithmetic** ([`Dim::compares_only`]): the offset would be wrong on a difference (a rise of
/// 9℉ is a rise of 5℃, not of −27.2℃), and the languages form no difference of temperatures.
pub fn offset(u: &str) -> Rat {
    match u {
        "℉" => Rat::new(-160, 9),
        _ => Rat::zero(),
    }
}

/// Every spelling the table has, quantities first in the table's order, then `円`, `JPY` and
/// `銭`, then each ISO 4217 code and its hundredth.
pub fn spellings() -> Vec<String> {
    let mut out: Vec<String> = QUANTITIES.iter().map(|(s, ..)| s.to_string()).collect();
    out.extend(["円", "JPY", "銭"].map(String::from));
    for c in CURRENCIES {
        out.push(c.to_string());
        out.push(format!("{c}c"));
    }
    out
}
