//! kaname's rounding (DESIGN 3.3, 16.3): the five ways, by the integer procedure the generated code
//! repeats, give rulec's answers (`RoundTo::round_to`, rulec's own) on every value of a grid of
//! fractions of both signs, for every step of a rounding the design names and the steps of the
//! currencies.

use kaname::num::{Rat, Rounding, round};
use rulec::num::{RoundMode, RoundTo};

fn rulec_way(r: Rounding) -> RoundMode {
    match r {
        Rounding::Up => RoundMode::Up,
        Rounding::Down => RoundMode::Down,
        Rounding::HalfUp => RoundMode::Half,
        Rounding::HalfDown => RoundMode::HalfDown,
        Rounding::HalfEven => RoundMode::Bankers,
    }
}

#[test]
fn every_way_of_rounding_is_rulecs() {
    // DESIGN 3.3's prototype: 1, 10, 1/2, 1/100, 3, 5/2; and steps a currency or a quantity takes
    // (a hundred cents, a tenth, five yen, a thousandth)
    let steps = [Rat::int(1), Rat::int(10), Rat::new(1, 2), Rat::new(1, 100), Rat::int(3), Rat::new(5, 2), Rat::int(100), Rat::new(1, 10), Rat::int(5), Rat::new(1, 1000)];
    let mut compared = 0u64;
    let mut differ = Vec::new();
    for n in -300i128..=300 {
        for d in 1i128..=12 {
            let x = Rat::new(n, d);
            for g in steps {
                for way in Rounding::ALL {
                    let ours = round(x, way, g).expect("small values fit");
                    let theirs = x.round_to(rulec_way(way), g);
                    compared += 1;
                    if ours != theirs {
                        differ.push(format!("{x} {} {g}: kaname {ours}, rulec {theirs}", way.word()));
                    }
                }
            }
        }
    }
    eprintln!("rounding: {compared} compared with rulec's round_to, {} differ", differ.len());
    assert!(differ.is_empty(), "{}", differ.iter().take(20).cloned().collect::<Vec<_>>().join("\n"));
    assert_eq!(compared, 601 * 12 * 10 * 5);
}

#[test]
fn a_rounding_that_leaves_int64_is_overflow() {
    let big = Rat::int(i64::MAX as i128);
    assert_eq!(round(big, Rounding::Up, Rat::int(1)), Ok(big));
    assert_eq!(round(Rat::new(i64::MAX as i128 * 2 - 1, 2), Rounding::Up, Rat::int(1)), Ok(big));
    assert_eq!(round(big, Rounding::Up, Rat::int(10)), Err(kaname::num::Arith::Overflow));
}
