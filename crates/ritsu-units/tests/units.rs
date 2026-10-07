//! The table of units, pinned spelling by spelling, and what a unit answers (PLAN D.1): the same
//! unit under two spellings, an exact conversion, one that does not land on a whole number, ℉'s
//! offset, and the dimensions that are ordered only.

use ritsu_units::table::{self, CURRENCIES};
use ritsu_units::{Dim, Problem, Rat, Tax, Unit};

fn r(n: i128, d: i128) -> Rat {
    Rat::new(n, d)
}

/// Every spelling of the table, with its dimension and factor. A change to the table is a change
/// to every language that reads units, so it is written out here in full.
#[test]
fn every_spelling_has_its_dimension_and_factor() {
    let want: Vec<(&str, Dim, Rat)> = vec![
        ("mg", Dim::Mass, r(1, 1000)),
        ("g", Dim::Mass, r(1, 1)),
        ("kg", Dim::Mass, r(1000, 1)),
        ("t", Dim::Mass, r(1_000_000, 1)),
        ("oz", Dim::Mass, r(28_349_523_125, 1_000_000_000)),
        ("lb", Dim::Mass, r(45_359_237, 100_000)),
        ("mm", Dim::Length, r(1, 10)),
        ("cm", Dim::Length, r(1, 1)),
        ("m", Dim::Length, r(100, 1)),
        ("km", Dim::Length, r(100_000, 1)),
        ("in", Dim::Length, r(254, 100)),
        ("ft", Dim::Length, r(3048, 100)),
        ("yd", Dim::Length, r(9144, 100)),
        ("mi", Dim::Length, r(1_609_344, 10)),
        ("mm2", Dim::Area, r(1, 1_000_000)),
        ("cm2", Dim::Area, r(1, 10_000)),
        ("m2", Dim::Area, r(1, 1)),
        ("a", Dim::Area, r(100, 1)),
        ("ha", Dim::Area, r(10_000, 1)),
        ("km2", Dim::Area, r(1_000_000, 1)),
        ("坪", Dim::Area, r(400, 121)),
        ("in2", Dim::Area, r(64_516, 100_000_000)),
        ("ft2", Dim::Area, r(9_290_304, 100_000_000)),
        ("yd2", Dim::Area, r(83_612_736, 100_000_000)),
        ("mi2", Dim::Area, r(2_589_988_110_336, 1_000_000)),
        ("ac", Dim::Area, r(40_468_564_224, 10_000_000)),
        ("mm3", Dim::Volume, r(1, 1_000_000)),
        ("cm3", Dim::Volume, r(1, 1000)),
        ("mL", Dim::Volume, r(1, 1000)),
        ("L", Dim::Volume, r(1, 1)),
        ("m3", Dim::Volume, r(1000, 1)),
        ("kL", Dim::Volume, r(1000, 1)),
        ("ms", Dim::Duration, r(1, 1000)),
        ("s", Dim::Duration, r(1, 1)),
        ("min", Dim::Duration, r(60, 1)),
        ("h", Dim::Duration, r(3600, 1)),
        ("d", Dim::Duration, r(86_400, 1)),
        ("w", Dim::Duration, r(604_800, 1)),
        ("℃", Dim::Temperature, r(1, 1)),
        ("℉", Dim::Temperature, r(5, 9)),
        ("dB", Dim::Sound, r(1, 1)),
        ("%", Dim::Rate, r(1, 100)),
        ("円", Dim::Money("円".into()), r(1, 1)),
        ("JPY", Dim::Money("円".into()), r(1, 1)),
        ("銭", Dim::Money("円".into()), r(1, 100)),
    ];
    let mut want_all: Vec<(String, Dim, Rat)> = want.into_iter().map(|(s, d, f)| (s.to_string(), d, f)).collect();
    for c in CURRENCIES {
        want_all.push((c.to_string(), Dim::Money(c.to_string()), r(1, 1)));
        want_all.push((format!("{c}c"), Dim::Money(c.to_string()), r(1, 100)));
    }
    let spelled = table::spellings();
    assert_eq!(spelled, want_all.iter().map(|(s, ..)| s.clone()).collect::<Vec<_>>(), "the table's spellings, in order");
    for (s, d, f) in &want_all {
        assert_eq!(table::unit(s), Some((d.clone(), *f)), "{s}");
    }
    assert_eq!(CURRENCIES.len(), 31);
    for nope in ["lbs", "gal", "gallon", "yen", "usd", "USDcc", "kg2", "", "c"] {
        assert_eq!(table::unit(nope), None, "{nope:?} is not in the table");
    }
}

#[test]
fn jpy_is_another_spelling_of_yen() {
    let jpy = Unit::parse("money[JPY, incl_tax]").unwrap();
    let yen = Unit::parse("money[円, incl_tax]").unwrap();
    assert!(jpy.same(&yen));
    assert_eq!(jpy.to_string(), "money[JPY, incl_tax]", "the spelling as written is kept");
    assert_eq!(jpy.canonical().to_string(), "money[円, incl_tax]");
    assert_eq!(jpy.convert(Rat::int(500), &yen), Some(Rat::int(500)));
    // the tax is part of the unit: with tax and without are not the same amount
    let excl = Unit::parse("money[円, excl_tax]").unwrap();
    let bare = Unit::parse("money[円]").unwrap();
    assert!(!yen.same(&excl));
    assert!(!yen.same(&bare));
    assert_eq!(yen.convert(Rat::int(500), &excl), None);
    // 銭 is a hundredth of 円
    let sen = Unit::money("銭", Some(Tax::Incl)).unwrap();
    assert!(!sen.same(&yen));
    assert_eq!(yen.whole(Rat::int(5), &sen), Some(Rat::int(500)));
    assert_eq!(sen.whole(Rat::int(150), &yen), None, "150銭 is no whole number of yen");
}

#[test]
fn a_currency_and_its_hundredth_convert_and_two_currencies_never_do() {
    let usd = Unit::money("USD", None).unwrap();
    let cents = Unit::money("USDc", None).unwrap();
    assert_eq!(usd.dim, Dim::Money("USD".into()));
    assert_eq!(cents.dim, Dim::Money("USD".into()));
    assert_eq!(usd.whole(Rat::int(12), &cents), Some(Rat::int(1200)));
    assert_eq!(cents.convert(Rat::int(1250), &usd), Some(r(25, 2)));
    assert_eq!(cents.whole(Rat::int(1250), &usd), None);
    let eur = Unit::money("EUR", None).unwrap();
    assert_eq!(usd.convert(Rat::int(1), &eur), None, "there is no exchange rate");
    let yen = Unit::money("円", None).unwrap();
    assert_eq!(usd.convert(Rat::int(1), &yen), None);
    assert_eq!(Unit::money("XYZ", None), None);
}

#[test]
fn a_conversion_that_is_no_whole_number_is_refused() {
    let g = Unit::quantity("mass", "g").unwrap();
    let kg = Unit::quantity("mass", "kg").unwrap();
    let lb = Unit::quantity("mass", "lb").unwrap();
    assert_eq!(kg.whole(r(3, 2), &g), Some(Rat::int(1500)), "1.5kg is 1500g");
    assert_eq!(lb.convert(Rat::int(1), &g), Some(r(45_359_237, 100_000)));
    assert_eq!(lb.whole(Rat::int(1), &g), None, "1lb is no whole number of grams");
    assert_eq!(lb.whole(Rat::int(100_000), &g), Some(Rat::int(45_359_237)));
    assert!(!kg.same(&g), "the same dimension in another unit is another unit");
    let cm = Unit::quantity("length", "cm").unwrap();
    assert_eq!(g.convert(Rat::int(1), &cm), None, "grams are not centimetres");
    assert_eq!(Unit::quantity("mass", "cm"), None);
    assert_eq!(Unit::quantity("weight", "g"), None);
    // 坪 is exactly 400/121 m2
    let tsubo = Unit::quantity("area", "坪").unwrap();
    let m2 = Unit::quantity("area", "m2").unwrap();
    assert_eq!(tsubo.convert(Rat::int(121), &m2), Some(Rat::int(400)));
}

#[test]
fn fahrenheit_is_a_line_and_not_a_scaling() {
    let c = Unit::quantity("temperature", "℃").unwrap();
    let f = Unit::quantity("temperature", "℉").unwrap();
    assert_eq!(f.convert(Rat::int(41), &c), Some(Rat::int(5)));
    assert_eq!(f.convert(Rat::int(-40), &c), Some(Rat::int(-40)));
    assert_eq!(f.convert(Rat::int(212), &c), Some(Rat::int(100)));
    assert_eq!(c.convert(Rat::int(100), &f), Some(Rat::int(212)));
    assert_eq!(c.convert(Rat::int(37), &f), Some(r(493, 5)));
    assert_eq!(c.whole(Rat::int(37), &f), None, "98.6℉ is no whole number");
    assert_eq!(table::offset("℉"), r(-160, 9));
    for s in table::spellings().iter().filter(|s| s.as_str() != "℉") {
        assert_eq!(table::offset(s), Rat::zero(), "{s} is a scaling");
    }
}

#[test]
fn only_temperature_and_sound_are_ordered_and_nothing_more() {
    let only: Vec<&str> = Dim::quantity_words().into_iter().filter(|w| Dim::of_word(w).unwrap().compares_only()).collect();
    assert_eq!(only, ["temperature", "sound"]);
    for d in [Dim::Money("円".into()), Dim::Rate, Dim::Number, Dim::Count("個".into())] {
        assert!(!d.compares_only(), "{d:?}");
    }
    assert!(Unit::quantity("sound", "dB").unwrap().compares_only());
    assert!(!Unit::quantity("duration", "h").unwrap().compares_only());
}

#[test]
fn a_unit_reads_back_as_it_is_written() {
    for s in ["money[円]", "money[円, incl_tax]", "money[JPY, excl_tax]", "money[USDc]", "mass[kg]", "length[mi]", "area[坪]", "volume[mL]", "duration[min]", "temperature[℉]", "sound[dB]", "rate", "rate[step 1%]", "rate[step 0.1%]", "rate[step 0.01%]", "rate[step 2.5%]", "number"] {
        let u = Unit::parse(s).unwrap_or_else(|e| panic!("{s}: {e:?}"));
        assert_eq!(u.to_string(), s);
        assert_eq!(Unit::parse(&u.to_string()).unwrap(), u);
    }
    // the space after the comma is not part of the unit
    assert_eq!(Unit::parse("money[円,incl_tax]").unwrap().to_string(), "money[円, incl_tax]");
    assert_eq!(Unit::parse("rate[step 0.1%]").unwrap().step, Some(r(1, 1000)));
    assert_eq!(Unit::rate(Some(r(1, 100))).to_string(), "rate[step 1%]");
    assert!(Unit::rate(Some(r(1, 100))).same(&Unit::parse("rate[step 1%]").unwrap()));
    assert!(!Unit::rate(Some(r(1, 100))).same(&Unit::rate(Some(r(1, 1000)))), "a rate counted in another step is another unit");
    assert_eq!(Unit::count("席").to_string(), "席");
    assert!(!Unit::count("席").same(&Unit::count("個")));
    assert!(Unit::count("席").same(&Unit::count("席")));
}

#[test]
fn what_is_not_a_unit_says_why() {
    let cases: Vec<(&str, Problem)> = vec![
        ("money[円, foo]", Problem::Tax("foo".into())),
        ("money[yen]", Problem::Unknown("yen".into())),
        ("mass[cm]", Problem::OtherDimension { unit: "cm".into(), dim: "mass".into() }),
        ("mass[lbs]", Problem::Unknown("lbs".into())),
        ("weight[g]", Problem::Dimension("weight".into())),
        ("rate[step 0%]", Problem::Step("0%".into())),
        ("rate[step 1]", Problem::Step("1".into())),
        ("money[円, incl_tax, x]", Problem::Shape("money[円, incl_tax, x]".into())),
        ("kg", Problem::Shape("kg".into())),
        ("mass[kg", Problem::Shape("mass[kg".into())),
    ];
    for (s, want) in cases {
        assert_eq!(Unit::parse(s), Err(want.clone()), "{s}");
        let t = want.text();
        assert!(!t.ja.is_empty() && !t.en.is_empty());
    }
}

#[test]
fn rationals_are_exact_and_say_when_they_do_not_fit() {
    assert_eq!(r(2, 4), r(1, 2));
    assert_eq!(r(1, -2), r(-1, 2));
    assert_eq!(r(1, 3).add(r(1, 6)), r(1, 2));
    assert_eq!(r(1, 2).sub(r(3, 4)), r(-1, 4));
    assert_eq!(r(2, 3).mul(r(3, 4)), r(1, 2));
    assert_eq!(r(1, 2).div(r(1, 4)), Rat::int(2));
    assert!(Rat::int(30).on_grid(Rat::int(10)) && !Rat::int(35).on_grid(Rat::int(10)));
    assert_eq!(r(1, 3).cmp_to(r(1, 2)), std::cmp::Ordering::Less);
    // decimal where the denominator allows it, else a fraction
    assert_eq!(r(3, 2).to_string(), "1.5");
    assert_eq!(r(1, 200).to_string(), "0.005");
    assert_eq!(r(1, 3).to_string(), "1/3");
    assert_eq!(Rat::int(-7).to_string(), "-7");
    // the checked arithmetic refuses what does not fit in 128 bits
    let big = Rat::int(i128::MAX);
    assert_eq!(big.checked_add(Rat::int(1)), None);
    assert_eq!(big.checked_mul(Rat::int(2)), None);
    assert_eq!(Rat::int(6).checked_div(Rat::int(4)), Some(r(3, 2)));
    assert_eq!(Rat::checked_new(1, 0), None);
    assert_eq!(r(1, 3).checked_cmp(r(1, 2)), Some(std::cmp::Ordering::Less));
}

/// The sign is written before the magnitude. It used to come from the whole part, and the whole
/// part of a value between -1 and 0 is 0: -1/2 was written `0.5`, -1/20 `0.05`.
#[test]
fn a_value_between_minus_one_and_zero_keeps_its_sign() {
    assert_eq!(r(-1, 2).to_string(), "-0.5");
    assert_eq!(r(-1, 20).to_string(), "-0.05");
    assert_eq!(r(-1, 200).to_string(), "-0.005");
    assert_eq!(r(-3, 10).to_string(), "-0.3");
    // a fraction keeps the sign it always had
    assert_eq!(r(-1, 3).to_string(), "-1/3");
    assert_eq!(r(-4, 3).to_string(), "-4/3");
    // zero, positive values and values at or below -1 are written as before
    assert_eq!(Rat::zero().to_string(), "0");
    assert_eq!(r(1, 2).to_string(), "0.5");
    assert_eq!(r(5, 4).to_string(), "1.25");
    assert_eq!(Rat::int(-1).to_string(), "-1");
    assert_eq!(r(-13, 10).to_string(), "-1.3");
    assert_eq!(r(-2005, 1000).to_string(), "-2.005");
    // a rate written as its percentage, the way rulec writes `-0.5%`
    assert_eq!(format!("{}%", r(-1, 200).mul(Rat::int(100))), "-0.5%");
    // a decimal too long for 128 bits is written as the fraction rather than overflowing
    let tiny = Rat::new(1, 1i128 << 126);
    assert_eq!(tiny.to_string(), format!("1/{}", 1i128 << 126));
    assert_eq!(Rat::new(-1, 1i128 << 126).to_string(), format!("-1/{}", 1i128 << 126));
}
