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

/// Every unit that is not a currency: its spelling, its dimension, its factor to the dimension's
/// base unit as numerator and denominator, and the name generated code gives its type
/// ([`type_name`]).
///
/// The imperial factors are exact rationals (a pound is 453.59237 g on the nose), so they cost
/// nothing in precision. What they do cost is scale: `1lb` in a column that counts grams is not a
/// whole number of grams and is refused, which is right — a pound is not writable there.
const QUANTITIES: &[(&str, Kind, i128, i128, &str)] = &[
    ("mg", Kind::Mass, 1, 1000, "Milligram"),
    ("g", Kind::Mass, 1, 1, "Gram"),
    ("kg", Kind::Mass, 1000, 1, "Kilogram"),
    ("t", Kind::Mass, 1_000_000, 1, "Tonne"),
    ("oz", Kind::Mass, 28_349_523_125, 1_000_000_000, "Ounce"),
    ("lb", Kind::Mass, 45_359_237, 100_000, "Pound"),
    ("mm", Kind::Length, 1, 10, "Millimeter"),
    ("cm", Kind::Length, 1, 1, "Cm"),
    ("m", Kind::Length, 100, 1, "Meter"),
    ("km", Kind::Length, 100_000, 1, "Kilometer"),
    ("in", Kind::Length, 254, 100, "Inch"),
    ("ft", Kind::Length, 3048, 100, "Foot"),
    ("yd", Kind::Length, 9144, 100, "Yard"),
    ("mi", Kind::Length, 1_609_344, 10, "Mile"),
    // Area, in square metres. It is a dimension of its own: two lengths do not multiply into one
    // of these (rulec's §15.83). 坪 is exact — 一間 is 六尺 and a 尺 is 10/33 m, so a 坪 is
    // (20/11)² = 400/121 m².
    ("mm2", Kind::Area, 1, 1_000_000, "SquareMillimeter"),
    ("cm2", Kind::Area, 1, 10_000, "SquareCentimeter"),
    ("m2", Kind::Area, 1, 1, "SquareMeter"),
    ("a", Kind::Area, 100, 1, "Are"),
    ("ha", Kind::Area, 10_000, 1, "Hectare"),
    ("km2", Kind::Area, 1_000_000, 1, "SquareKilometer"),
    ("坪", Kind::Area, 400, 121, "Tsubo"),
    ("in2", Kind::Area, 64_516, 100_000_000, "SquareInch"),
    ("ft2", Kind::Area, 9_290_304, 100_000_000, "SquareFoot"),
    ("yd2", Kind::Area, 83_612_736, 100_000_000, "SquareYard"),
    ("mi2", Kind::Area, 2_589_988_110_336, 1_000_000, "SquareMile"),
    ("ac", Kind::Area, 40_468_564_224, 10_000_000, "Acre"),
    // Volume, in litres. `cm3` and `mL` are the same size and so are `m3` and `kL`; both spellings
    // are kept because a water tariff writes m³ and a fire code writes L. **No gallon**: the US
    // one is 3.785411784 L and the imperial one 4.54609 L, and a unit that means two different
    // sizes is the one thing this table must not hold.
    ("mm3", Kind::Volume, 1, 1_000_000, "CubicMillimeter"),
    ("cm3", Kind::Volume, 1, 1000, "CubicCentimeter"),
    ("mL", Kind::Volume, 1, 1000, "Milliliter"),
    ("L", Kind::Volume, 1, 1, "Liter"),
    ("m3", Kind::Volume, 1000, 1, "CubicMeter"),
    ("kL", Kind::Volume, 1000, 1, "Kiloliter"),
    // Time, in seconds: the span a rule compares, not a calendar day (a `date` has no
    // arithmetic). A minute is `min` because `m` is the metre.
    ("ms", Kind::Duration, 1, 1000, "Millisecond"),
    ("s", Kind::Duration, 1, 1, "Second"),
    ("min", Kind::Duration, 60, 1, "Minute"),
    ("h", Kind::Duration, 3600, 1, "Hour"),
    ("d", Kind::Duration, 86_400, 1, "Day"),
    ("w", Kind::Duration, 604_800, 1, "Week"),
    // Ordered, not arithmetic (rulec's §15.84). A ℃ is a scale with a displaced zero, so ℉
    // carries an offset as well as a factor — see `offset`. A decibel is a logarithm and has one
    // spelling, so there is nothing to convert it to.
    ("℃", Kind::Temperature, 1, 1, "Celsius"),
    ("℉", Kind::Temperature, 5, 9, "Fahrenheit"),
    ("dB", Kind::Sound, 1, 1, "Decibel"),
    ("%", Kind::Rate, 1, 100, "Rate"),
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
    QUANTITIES.iter().find(|(s, ..)| *s == u).map(|(_, k, n, d, _)| (k.dim(), Rat::new(*n, *d)))
}

/// The name generated code gives the type of an amount written in `u` (ritsu's DESIGN 9.3): the
/// English name of a quantity's unit (`kg` is `Kilogram`, `坪` is `Tsubo`), and a currency by its
/// spelling, `円` as `Yen` and `銭` as `Sen` (`JPY`, `USD` and `USDc` as they are). ASCII, so that
/// every language takes it as an identifier. It is the naming of rulec's brands (rulec's §8.2),
/// kept here so that rulec and a package of ritsu's name a unit's type alike. A spelling the table
/// does not have is its own name.
pub fn type_name(u: &str) -> String {
    match u {
        "円" => "Yen".into(),
        "銭" => "Sen".into(),
        _ => QUANTITIES.iter().find(|(s, ..)| *s == u).map(|(.., name)| name.to_string()).unwrap_or_else(|| u.to_string()),
    }
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
