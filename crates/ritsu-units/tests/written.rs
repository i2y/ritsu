//! A number as the languages write it (`ritsu_units::written`, ritsu's DESIGN 6.2), for a message to
//! say: the digits grouped from five on, a whole number on the wire in its unit (a rate as the
//! percentage its steps come to), a book's amount with its unit's decimal places, a range with each
//! of its four shapes of ends, and a unit with a Japanese name as the file spells it.

use ritsu_units::Unit;
use ritsu_units::written::{self, End};

fn unit(s: &str) -> Unit {
    Unit::parse(s).unwrap_or_else(|e| panic!("{s}: {:?}", e.text()))
}

const MAX: i128 = i64::MAX as i128;

#[test]
fn a_whole_number_is_grouped_from_five_digits() {
    for (n, want) in [
        (0, "0"),
        (7, "7"),
        (5000, "5000"),
        (-9999, "-9999"),
        (10_000, "10_000"),
        (-12_345, "-12_345"),
        (100_000_000, "100_000_000"),
        (MAX, "9_223_372_036_854_775_807"),
        (-MAX, "-9_223_372_036_854_775_807"),
    ] {
        assert_eq!(written::digits(n), want, "{n}");
    }
}

#[test]
fn a_value_on_the_wire_is_written_in_its_unit() {
    let (jpy, usdc, g, celsius) = (unit("money[JPY]"), unit("money[USDc]"), unit("mass[g]"), unit("temperature[℃]"));
    for (n, u, want) in [
        (10_000_000, Some(&jpy), "10_000_000JPY"),
        (0, Some(&jpy), "0JPY"),
        (1, Some(&jpy), "1JPY"),
        (MAX, Some(&jpy), "9_223_372_036_854_775_807JPY"),
        (-48, Some(&usdc), "-48USDc"),
        (1250, Some(&usdc), "1250USDc"),
        (30_000, Some(&g), "30_000g"),
        (-40, Some(&celsius), "-40℃"),
    ] {
        assert_eq!(written::value(n, u), want, "{n}");
    }
    // a number, a count and a unit nothing says: the digits alone
    assert_eq!(written::value(5000, Some(&Unit::number())), "5000");
    assert_eq!(written::value(20_000, Some(&Unit::number())), "20_000");
    assert_eq!(written::value(-20, Some(&Unit::count("seat"))), "-20");
    assert_eq!(written::value(12_345, None), "12_345");
}

#[test]
fn a_rate_is_the_percentage_its_steps_come_to() {
    for (n, u, want) in [
        (5, "rate[step 1%]", "5%"),
        (5, "rate[step 0.1%]", "0.5%"),
        (-5, "rate[step 0.1%]", "-0.5%"),
        (1250, "rate[step 0.01%]", "12.5%"),
        (5, "rate[step 0.001%]", "0.005%"),
        (0, "rate[step 0.1%]", "0%"),
        (1_000_000, "rate[step 0.01%]", "10_000%"),
        // with no step, one integer counts the whole rate
        (1, "rate", "100%"),
        (-2, "rate", "-200%"),
    ] {
        assert_eq!(written::value(n, Some(&unit(u))), want, "{n} {u}");
    }
}

/// A book's amount with no decimal places, with two and with three; a count is digits only.
#[test]
fn a_books_amount_is_written_with_its_decimal_places() {
    let (usdc, yen) = (unit("money[USDc]"), unit("money[円]"));
    for (n, places, name, u, want) in [
        (1000, 0, "円", &yen, "1000円"),
        (100_000, 0, "円", &yen, "100_000円"),
        (-3, 0, "円", &yen, "-3円"),
        (1250, 2, "USD", &usdc, "12.50USD"),
        (-48, 2, "USD", &usdc, "-0.48USD"),
        (0, 2, "USD", &usdc, "0.00USD"),
        (123_456_750, 2, "USD", &usdc, "1_234_567.50USD"),
        (MAX, 2, "USD", &usdc, "92_233_720_368_547_758.07USD"),
    ] {
        assert_eq!(written::amount(n, places, name, u), want, "{n}");
    }
    assert_eq!(written::amount(1234, 3, "BHD", &Unit::count("BHD")), "1.234");
    assert_eq!(written::amount(-12_345_678, 3, "BHD", &Unit::count("BHD")), "-12_345.678");
    assert_eq!(written::amount(-20, 0, "seat", &Unit::count("seat")), "-20");
}

/// Both ends, one end, an end it stops short of, and none.
#[test]
fn a_range_is_written_with_its_ends() {
    let jpy = unit("money[JPY]");
    assert_eq!(written::between(Some(1), Some(100_000), Some(&jpy)), ">=1JPY <=100_000JPY");
    assert_eq!(written::between(Some(0), None, Some(&unit("rate[step 1%]"))), ">=0%");
    assert_eq!(written::between(None, Some(10_000), Some(&Unit::number())), "<=10_000");
    assert_eq!(written::between(None, None, Some(&jpy)), "");
    let show = |n| written::value(n, Some(&jpy));
    assert_eq!(written::range(Some(End::Open(0)), Some(End::Open(100)), show), ">0JPY <100JPY");
    assert_eq!(written::range(Some(End::Closed(-5)), Some(End::Open(10_000)), show), ">=-5JPY <10_000JPY");
    assert_eq!(written::range(None, Some(End::Open(0)), show), "<0JPY");
    assert_eq!(written::range(Some(End::Open(-1)), None, show), ">-1JPY");
}

/// A unit with a Japanese name is the one the file writes: `円` where the rule says `円`, and the
/// same digits as in English.
#[test]
fn a_unit_with_a_japanese_name_is_written_as_the_file_spells_it() {
    assert_eq!(written::value(1_500_000, Some(&unit("money[円]"))), "1_500_000円");
    assert_eq!(written::value(1_500_000, Some(&unit("money[JPY]"))), "1_500_000JPY");
    assert_eq!(written::value(50, Some(&unit("money[銭]"))), "50銭");
    assert_eq!(written::value(12, Some(&unit("area[坪]"))), "12坪");
    assert_eq!(written::between(Some(1), Some(100_000), Some(&unit("money[円, incl_tax]"))), ">=1円 <=100_000円");
}
