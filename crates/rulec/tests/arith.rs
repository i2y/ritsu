//! Arithmetic (§15.213 to §15.216): a division by a value inside a rounding, the remainder `mod`,
//! the whole square root `isqrt`, and a `constraint` that adds inputs up on one side.
//!
//! What is held here is the line each of them draws through the tool. The check passes only what
//! it can prove and says why it stops (E132, E139, E115 in its new places), with an input that
//! shows it; the reference evaluator answers with exact arithmetic; the sieve reads a sum; the
//! vectors keep to it and the refused ones break it; the certificate says enough for both
//! re-checkers to work every claim out again, and a lie in it is caught. The twelve generated
//! languages are held to the evaluator over the corpus rules by `tests/threeway.rs`, and the
//! helpers they share by the unit vectors `rulec test` runs.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use ritsu_testkit::{Need, TempDir, ready};
use rulec::eval::Val;
use rulec::num::Rat;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rulec(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .current_dir(root())
        .args(args)
        .output()
        .expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

fn have_python() -> bool {
    Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// The codes `check` gives a rule, in English, sorted.
fn codes(src: &str) -> Vec<&'static str> {
    rulec::i18n::set(rulec::i18n::Lang::En);
    let mut v: Vec<&'static str> = rulec::check_source(src, "arith.rule").iter().map(|d| d.code).collect();
    v.sort();
    v
}

/// The one diagnostic with `code`, rendered as `check` prints it.
fn said(src: &str, code: &str) -> String {
    rulec::i18n::set(rulec::i18n::Lang::En);
    let lines: Vec<String> = src.lines().map(String::from).collect();
    let ds = rulec::check_source(src, "arith.rule");
    let d = ds.iter().find(|d| d.code == code).unwrap_or_else(|| panic!("no {code}: {:?}", ds.iter().map(|d| d.code).collect::<Vec<_>>()));
    rulec::diag::render(d, &lines)
}

/// What the reference evaluator answers for whole-number inputs.
fn answer(src: &str, inputs: &[(&str, i128)]) -> Option<Val> {
    let (f, c) = rulec::prepare(src, "arith.rule").unwrap_or_else(|d| panic!("{:?}", d.iter().map(|x| x.code).collect::<Vec<_>>()));
    let ins: HashMap<String, Val> = inputs.iter().map(|(n, v)| (n.to_string(), Val::Num(Rat::int(*v)))).collect();
    rulec::eval::run(&f, &c, ins).0
}

fn num(v: i128) -> Option<Val> {
    Some(Val::Num(Rat::int(v)))
}

/// Feed a certificate to `tools/recheck.py`.
fn recheck(cert: &str) -> (i32, String) {
    use std::io::Write;
    let mut p = Command::new("python3")
        .current_dir(root())
        .arg("tools/recheck.py")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("cannot start python3");
    p.stdin.as_mut().unwrap().write_all(cert.as_bytes()).unwrap();
    let o = p.wait_with_output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
}

/// The Lean program, when it has been built (`lake build` in ritsu's `proofs/`).
fn checker() -> Option<PathBuf> {
    let p = root().join("../../proofs/.lake/build/bin/rulec-recheck");
    ready(Need::Lean, || p.exists(), "ritsu's proofs/ is not built (lake build there makes it)").then_some(p)
}

fn lean(bin: &PathBuf, cert: &str, rule: &str) -> (i32, String) {
    use std::io::Write;
    let mut p = Command::new(bin)
        .current_dir(root())
        .args(["--rule", rule])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("cannot start the Lean checker");
    p.stdin.as_mut().unwrap().write_all(cert.as_bytes()).unwrap();
    let o = p.wait_with_output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
}

/// A rule of one `define` over two numbers, with a rounding mode and the divisor's range filled in.
fn quotient_rule(mode: &str, divisor_range: &str) -> String {
    format!(
        "rule quotient v1\n\ninputs\n  n : number  range >=-100 <=100\n  d : number  range {divisor_range}\n\noutputs\n  q : number  round down(1)\n\ndefine x : number = {mode}(n / d, 1)\n\nresult q = x\n"
    )
}

// ── Division by a value inside a rounding (§15.213) ──────────────────────────────────────────

/// The quotient is the exact rational, put on the grid the way its mode says: away from zero or
/// toward it, and at exactly half by the mode's own rule, the same on both sides of zero. A
/// negative divisor is as good as a positive one; what is refused is a range that reaches zero.
#[test]
fn a_quotient_is_exact_and_put_on_the_grid_by_its_mode() {
    // (n, d, up, down, half_up, half_even, half_down)
    let cases: &[(i128, i128, i128, i128, i128, i128, i128)] = &[
        (7, 2, 4, 3, 4, 4, 3),
        (-7, 2, -4, -3, -4, -4, -3),
        (5, 2, 3, 2, 3, 2, 2),
        (-5, 2, -3, -2, -3, -2, -2),
        (6, 3, 2, 2, 2, 2, 2),
        (1, 3, 1, 0, 0, 0, 0),
        (2, 3, 1, 0, 1, 1, 1),
        (-1, 3, -1, 0, 0, 0, 0),
        (100, 7, 15, 14, 14, 14, 14),
    ];
    let modes = ["up", "down", "half_up", "half_even", "half_down"];
    for (mi, mode) in modes.iter().enumerate() {
        let pos = quotient_rule(mode, ">=1 <=7");
        let neg = quotient_rule(mode, ">=-7 <=-1");
        for &(n, d, a, b, c, e, f) in cases {
            let want = [a, b, c, e, f][mi];
            assert_eq!(answer(&pos, &[("n", n), ("d", d)]), num(want), "{mode}({n} / {d})");
            // n / d is (-n) / (-d): the same quotient over a negative divisor.
            assert_eq!(answer(&neg, &[("n", -n), ("d", -d)]), num(want), "{mode}({} / {})", -n, -d);
        }
    }
}

/// A divisor whose range reaches zero is E132, with an input that makes it zero; the three ways
/// out the hint names each pass.
#[test]
fn a_divisor_that_can_be_zero_is_e132_with_an_input_that_makes_it_so() {
    let base = "rule average_price v1\n\ninputs\n  total : money[USD]  range >=0USD <=1_000_000USD\n  units : number      range >=0 <=10_000\n\noutputs\n  average : money[USD]  round half_up(1USD)\n\ndefine avg : money[USD] = half_up(total / units, 1USD)\n\nresult average = avg\n";
    assert_eq!(codes(base), ["E132"]);
    let text = said(base, "E132");
    assert!(text.contains("The divisor units can be zero"), "{text}");
    assert!(text.contains("it ranges from 0 to 10000, zero included"), "{text}");
    assert!(text.contains("An input that makes the divisor zero: units = 0"), "{text}");
    // The way out with `max`, and the narrower range.
    assert!(codes(&base.replace("total / units", "total / max(units, 1)")).is_empty());
    assert!(codes(&base.replace("range >=0 <=10_000", "range >=1 <=10_000")).is_empty());
    // A divisor that is a sum is zero only where both are: the input says both.
    let share = "rule win_share v1\n\ninputs\n  attack  : number  range >=0 <=100\n  defense : number  range >=0 <=100\n\noutputs\n  share : number  round down(1)\n\ndefine permille : number = half_even(attack * 1000 / (attack + defense), 1)\n\nresult share = permille\n";
    let text = said(share, "E132");
    assert!(text.contains("An input that makes the divisor zero: attack = 0, defense = 0"), "{text}");
    assert!(codes(&share.replace("range >=0 <=100\n  defense", "range >=1 <=100\n  defense")).is_empty());
    // Below zero all the way is as good as above it.
    assert!(codes(&base.replace("range >=0 <=10_000", "range >=-10_000 <=-1")).is_empty());
}

/// The table version of the way out: a table that picks one where the count is zero. The row that
/// writes `units` writes it only where its own cell keeps it at one or more, so the column the
/// division reads is never zero, and the certificate says so on that row.
#[test]
fn a_table_that_picks_the_divisor_keeps_it_off_zero() {
    let src = std::fs::read_to_string(root().join("tests/corpus/average_unit_price_split.rule")).unwrap();
    assert!(codes(&src).is_empty());
    let (c, cert) = rulec(&["certificate", "tests/corpus/average_unit_price_split.rule"]);
    assert_eq!(c, 0, "{cert}");
    assert!(cert.contains(r#""per":["1","1000"]"#), "{cert}");
    let inside = r#""inside":["1","1000"]"#;
    assert!(cert.contains(inside), "{cert}");
    // Without the row's own cell the column reaches zero, and the division stops.
    let wide = src.replace("| >=1   | units           |", "| -     | units           |").replace("policy unique\n| units | -> per", "policy first\n| units | -> per");
    assert_ne!(wide, src);
    assert_eq!(codes(&wide), ["E132"]);
    if ready(Need::Python, have_python, "no python3") {
        let (code, out) = recheck(&cert);
        assert_eq!(code, 0, "{out}");
        for (what, forged, why) in [
            ("a narrower inside", cert.replacen(inside, r#""inside":["2","1000"]"#, 1), "row 2's cell lets units take [1, 1000]"),
            ("a narrower column", cert.replacen(r#""per":["1","1000"]"#, r#""per":["2","1000"]"#, 1), "its rows write values from 1 to 1000"),
            ("the row's word dropped", cert.replacen(&format!(",{inside}"), "", 1), "its rows write values from 0 to 1000"),
        ] {
            assert_ne!(forged, cert, "{what}");
            let (code, out) = recheck(&forged);
            assert_eq!(code, 1, "{what}: a forged certificate passed\n{out}");
            assert!(out.contains(why), "{what}: {out}");
        }
    }
    if let Some(bin) = checker() {
        // The proved checker works the row's cut out of its own cell too (`cut_of_box`, §15.232):
        // the column's range is proved, and the same three lies are refused.
        let rel = "tests/corpus/average_unit_price_split.rule";
        let (code, out) = lean(&bin, &cert, rel);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("OK: every claim this program states was proved"), "{out}");
        for (what, forged, why) in [
            ("a narrower inside", cert.replacen(inside, r#""inside":["2","1000"]"#, 1), "row 2's cell lets units take [1, 1000]"),
            ("a narrower column", cert.replacen(r#""per":["1","1000"]"#, r#""per":["2","1000"]"#, 1), "per: its rows write a value outside the range"),
            ("the row's word dropped", cert.replacen(&format!(",{inside}"), "", 1), "per: its rows write a value outside the range"),
        ] {
            let (code, out) = lean(&bin, &forged, rel);
            assert_eq!(code, 1, "{what}: a forged certificate passed\n{out}");
            assert!(out.contains(why), "{what}: {out}");
        }
    }
}

/// A value divides only as the outermost operation of a rounding's first argument. Anywhere else
/// the divisor is a constant, as it always was (E115), and the message says where a value can go.
#[test]
fn a_division_by_a_value_anywhere_else_is_still_e115() {
    let rule = |expr: &str| {
        format!(
            "rule proration v1\n\ninputs\n  fee  : money[USD]  range >=0USD <=10_000USD\n  used : number      range >=0 <=31\n  days : number      range >=28 <=31\n\noutputs\n  charge : money[USD]  round down(1USD)\n\ndefine x : money[USD] = {expr}\n\nresult charge = x\n"
        )
    };
    assert!(codes(&rule("down(fee * used / days, 1USD)")).is_empty());
    for e in ["down(fee * (used / days), 1USD)", "down(fee / used / days, 1USD)", "down(fee, 1USD) * used / days"] {
        assert_eq!(codes(&rule(e)), ["E115"], "{e}");
    }
    let text = said(&rule("down(fee * (used / days), 1USD)"), "E115");
    assert!(text.contains("a value can be divided by only as the outermost operation of a rounding's argument"), "{text}");
    // A literal divisor is the constant division it always was (`days` is then read by nothing).
    assert_eq!(codes(&rule("down(fee * used / 30, 1USD)")), ["W111"]);
}

/// The quotient's type follows §2.3's division, and the grid says which it is: an amount over an
/// amount is a ratio, a number or a rate as its grid says; an amount over a number is an amount.
#[test]
fn the_grid_says_the_type_of_a_quotient() {
    let rule = |divisor: &str, ty: &str, grid: &str, expr: &str| {
        format!("rule ratio v1\n\ninputs\n  a : money[USD]  range >=0USD <=1000USD\n  {divisor}\n\noutputs\n  r : {ty}  round down({grid})\n\ndefine x : {ty} = {expr}\n\nresult r = x\n")
    };
    let amount = "b : money[USD]  range >=1USD <=1000USD";
    let count = "b : number  range >=1 <=10";
    assert!(codes(&rule(amount, "number", "1", "half_up(a * 100 / b, 1)")).is_empty());
    assert!(codes(&rule(amount, "rate[step 0.1%]", "0.1%", "half_up(a / b, 0.1%)")).is_empty());
    assert!(codes(&rule(count, "money[USD]", "1USD", "half_up(a / b, 1USD)")).is_empty());
    // An amount over an amount on a grid of dollars: the quotient is a ratio, the grid an amount.
    assert_eq!(codes(&rule(amount, "money[USD]", "1USD", "half_up(a / b, 1USD)")), ["E103"]);
}

/// The generated code divides one integer by another; both have to fit int64.
#[test]
fn a_divided_rounding_has_to_fit_int64() {
    let src = "rule big v1\n\ninputs\n  a : number  range >=0 <=9_000_000_000_000_000_000\n  b : number  range >=1 <=10\n\noutputs\n  r : number  round down(1)\n\ndefine x : number = down(a * 10 / b, 1)\n\nresult r = x\n";
    assert!(codes(src).contains(&"E108"), "{:?}", codes(src));
}

// ── An end a range does not write (§15.294) ───────────────────────────────────────────────────

/// A rule over `a`, whose range is `range`, and `b`, from 0 to 100, with `s` its `expr`.
fn ended(range: &str, expr: &str) -> String {
    format!("rule edges v1\n\ninputs\n  a : number{range}\n  b : number  range >=0 <=100\n\noutputs\n  r : number  round down(1)\n\ndefine s : number = {expr}\n\nresult r = s\n")
}

/// An end a range does not write is read as the end of what the type holds, 2^63 − 1 steps either
/// way: a value that can pass int64 from there is E108, which names the input, and one that cannot
/// passes, as it did before. `check` used to pass both, while both re-checkers refused the
/// certificate of either, which stated no interval for the value.
#[test]
fn an_end_a_range_does_not_write_is_the_end_of_its_type() {
    for (range, expr) in [("  range >=0", "a + b"), ("  range <=0", "a - b"), ("", "a + b"), ("", "a * 2 - b")] {
        let src = ended(range, expr);
        assert_eq!(codes(&src), ["E108"], "{range} {expr}");
        let said = said(&src, "E108");
        assert!(said.contains("Cannot prove that s fits in int64"), "{said}");
        assert!(said.contains("`a` has no upper or no lower bound"), "{said}");
        assert!(said.contains("give `a` the end its range does not write"), "{said}");
    }
    // What follows from a value E108 stopped is not stopped a second time: `r` reads `s`.
    assert_eq!(codes(&ended("  range >=0", "a + b")).len(), 1);
    for (range, expr) in [("  range >=0", "a - b"), ("  range <=0", "a + b"), ("", "max(a, b)"), ("", "max(a, 0) - b"), ("", "mod(a, 7) + b")] {
        let src = ended(range, expr);
        assert!(codes(&src).is_empty(), "{range} {expr}: {:?}", codes(&src));
    }
}

/// The certificate states the interval with the end of the type in it, and both re-checkers work
/// out the same and hold it to int64. A narrower interval, a smaller stored integer, or none at all
/// is refused by both.
#[test]
fn the_certificate_states_the_end_of_the_type() {
    if !have_python() {
        return;
    }
    let file = std::env::temp_dir().join(format!("rulec-edges-{}.rule", std::process::id()));
    std::fs::write(&file, ended("  range >=0", "a - b")).unwrap();
    let (code, cert) = rulec(&["certificate", file.to_str().unwrap()]);
    assert_eq!(code, 0, "{cert}");
    let stated = r#""interval":["-100","9223372036854775807"],"scale":1,"stored_max":"9223372036854775807""#;
    assert!(cert.contains(&format!(r#""name":"s","of":"define","type":"number","expr":{{"op":"-","l":{{"name":"a"}},"r":{{"name":"b"}}}},{stated}"#)), "{cert}");
    // The range keeps the end the rule writes.
    assert!(cert.contains(r#""ranges":{"a":["0",null],"b":["0","100"]}"#), "{cert}");
    let (code, out) = recheck(&cert);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("int64: 2 values fit"), "{out}");
    let lean_bin = checker();
    if let Some(bin) = &lean_bin {
        let (code, out) = lean(bin, &cert, file.to_str().unwrap());
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("2 held to int64"), "{out}");
    }
    for (what, forged) in [
        ("narrower", r#""interval":["-100","1000"],"scale":1,"stored_max":"9223372036854775807""#),
        ("a smaller stored integer", r#""interval":["-100","9223372036854775807"],"scale":1,"stored_max":"1000""#),
        ("none", r#""interval":null,"scale":null,"stored_max":null"#),
    ] {
        let forged = cert.replacen(stated, forged, 1);
        assert_ne!(forged, cert, "{what}");
        let (code, out) = recheck(&forged);
        assert_ne!(code, 0, "{what}: {out}");
        assert!(out.contains("FAILED int64: s:"), "{what}: {out}");
        if let Some(bin) = &lean_bin {
            let (code, out) = lean(bin, &forged, file.to_str().unwrap());
            assert_ne!(code, 0, "{what}: {out}");
        }
    }
    std::fs::remove_file(&file).ok();
}

/// A table writes `a`, with no upper bound, or 0 into `v`, and `s` reads `v` with `expr`; `fee` runs
/// from 0 to 500.
fn column(expr: &str) -> String {
    format!(
        "rule column v1\n\ninputs\n  a   : number  range >=0\n  k   : bool\n  fee : number  range >=0 <=500\n\noutputs\n  r : number  round down(1)\n\ntable t\npolicy unique\n| k     | -> v : number |\n| true  | a             |\n| false | 0             |\n\ndefine s : number = {expr}\n\nresult r = s\n"
    )
}

/// A column a table writes numbers into is read in what its rows write, each name in it read as the
/// int64 claim reads it: `v` is never below zero, so `v - fee` fits, and `v + fee` does not. The hint
/// names `a`, whose range has an end to write, not the column, which has no range of its own. The
/// column was read at both ends of its type, which stopped `v - fee` and named `v`.
#[test]
fn a_column_a_table_writes_is_read_in_what_its_rows_write() {
    assert!(codes(&column("v - fee")).is_empty(), "{:?}", codes(&column("v - fee")));
    let src = column("v + fee");
    assert_eq!(codes(&src), ["E108"]);
    let said = said(&src, "E108");
    assert!(said.contains("`a` has no upper or no lower bound") && said.contains("give `a` the end"), "{said}");
    if !have_python() {
        return;
    }
    let file = std::env::temp_dir().join(format!("rulec-column-{}.rule", std::process::id()));
    std::fs::write(&file, column("v - fee")).unwrap();
    let (code, cert) = rulec(&["certificate", file.to_str().unwrap()]);
    assert_eq!(code, 0, "{cert}");
    assert!(cert.contains(r#""interval":["-500","9223372036854775807"],"scale":1,"stored_max":"9223372036854775807""#), "{cert}");
    let (code, out) = recheck(&cert);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("int64: 2 values fit"), "{out}");
    if let Some(bin) = checker() {
        let (code, out) = lean(&bin, &cert, file.to_str().unwrap());
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("2 held to int64"), "{out}");
    }
    std::fs::remove_file(&file).ok();
}

/// A value above the line that decides an output it reads (a second walk reads what the first
/// answers) is held to int64 once every item is read. `check` passed `charge × 10^18` above the
/// `sum` of `charge` without a word, and both re-checkers refused its certificate; an output not
/// decided yet is not read at the ends of its type either, which stopped `charge - cost`.
#[test]
fn a_value_above_the_output_it_reads_is_held_to_int64() {
    let src = std::fs::read_to_string(root().join("tests/stages/parcel_charge.rule")).unwrap();
    let above = |lines: &str| {
        let out = src.replace("define cost : number = weight * factor\n", &format!("define cost : number = weight * factor\n{lines}"));
        assert_ne!(out, src);
        out
    };
    assert!(codes(&above("define rest : number = charge - cost\n")).is_empty());
    for (lines, name) in [
        ("define rest : number = charge * 1000000000000000000\n", "rest"),
        // One that reads a value that waited for the output finds it worked out.
        ("define rest : number = charge - cost\ndefine more : number = rest * 1000000000000000\n", "more"),
    ] {
        let src = above(lines);
        assert_eq!(codes(&src), ["E108"], "{name}");
        let said = said(&src, "E108");
        assert!(said.contains(&format!("Cannot prove that {name} fits in int64")) && said.contains("The reachable interval is"), "{said}");
    }
}

/// A range that writes an end past what its type holds, 2^63 − 1 steps either way, is E108, on an
/// input or a field of an element, read in a table or not: the entry of the generated code refuses
/// anything past that end, so no call can pass what the range promises. The ends themselves pass.
#[test]
fn a_range_past_what_its_type_holds_is_e108() {
    let input = |decl: &str| format!("rule big v1\n\ninputs\n  amount : {decl}\n\noutputs\n  fee : number  round down(1)\n\ntable fees\npolicy unique\n| amount | -> fee |\n| -      | 1      |\n");
    for decl in [
        "money[JPY]  range <=9223372036854775808JPY",
        "money[JPY]  range >=-9223372036854775808JPY",
        "rate[step 0.01%]  range >=0% <=100000000000000000%",
    ] {
        let src = input(decl);
        assert_eq!(codes(&src), ["E108"], "{decl}");
        let said = said(&src, "E108");
        assert!(said.contains("An end of the range of amount is past what its type holds") && said.contains("2^63 − 1 steps either way"), "{said}");
    }
    assert!(codes(&input("money[JPY]  range >=-9223372036854775807JPY <=9223372036854775807JPY")).is_empty());
    let element = |hi: &str| format!("rule big v1\n\nelements items\n  w : number  range >=0 <={hi}\n\noutputs\n  n : number  round down(1)\n\nsum n over items of w  range >=0 <=100\n");
    assert_eq!(codes(&element("9223372036854775808")), ["E108"]);
    assert!(said(&element("9223372036854775808"), "E108").contains("An end of the range of w is past what its type holds"));
    assert!(codes(&element("100")).is_empty());
}

// ── The remainder (§15.214) ──────────────────────────────────────────────────────────────────

/// `mod` is the remainder from zero up to one less than the divisor, whatever the sign.
#[test]
fn mod_leaves_zero_up_to_one_less_than_the_divisor() {
    let src = "rule turn v1\n\ninputs\n  x : number  range >=-100 <=100\n\noutputs\n  r : number  round down(1)\n\ndefine t : number = mod(x, 3)\n\nresult r = t\n";
    assert!(codes(src).is_empty());
    for (x, want) in [(-7, 2), (-3, 0), (-1, 2), (0, 0), (1, 1), (7, 1), (100, 1)] {
        assert_eq!(answer(src, &[("x", x)]), num(want), "mod({x}, 3)");
    }
}

/// The remainder's range: inside one period the argument's interval moved down, otherwise the
/// whole of zero to one less than the divisor. A table over the first needs rows for what it
/// reaches and no more; over the second, for every remainder.
#[test]
fn the_range_of_a_remainder_is_what_the_check_reads() {
    let rule = |range: &str| {
        format!("rule tail v1\n\nenum band = low | high\n\ninputs\n  x : number  range {range}\n\noutputs\n  b : band\n\ndefine t : number = mod(x, 10)\n\ntable bands\npolicy unique\n| t       | -> b : band |\n| >=3 <=5 | low         |\n| >=6 <=7 | high        |\n")
    };
    // 13 to 17 leaves 3 to 7.
    assert!(codes(&rule(">=13 <=17")).is_empty());
    // 13 to 23 passes a multiple of ten, so the check reads every remainder from 0 to 9.
    assert_eq!(codes(&rule(">=13 <=23")), ["E101"]);
}

/// The divisor of `mod` is a positive whole constant, and the argument a whole number.
#[test]
fn mod_takes_a_whole_number_and_a_positive_constant() {
    let rule = |decl: &str, expr: &str| {
        format!("rule turn v1\n\ninputs\n  x : number  range >=0 <=100\n  {decl}\n\noutputs\n  r : number  round down(1)\n\ndefine t : number = {expr}\n\nresult r = t\n")
    };
    let n = "n : number  range >=1 <=10";
    assert_eq!(codes(&rule(n, "mod(x, n)")), ["E115"]);
    for k in ["0", "-3"] {
        assert_eq!(codes(&rule(n, &format!("mod(x + n, {k})"))), ["E115"], "mod(x + n, {k})");
    }
    // A bare decimal is no number at all, and says so as well.
    assert_eq!(codes(&rule(n, "mod(x + n, 2.5)")), ["E103", "E115"]);
    let m = "m : money[USD]  range >=0USD <=100USD";
    assert_eq!(codes(&rule(m, "mod(m + m, 3) + x")), ["E103"]);
    assert!(codes(&rule(n, "mod(x * 2 + n, 7)")).is_empty());
}

// ── The whole square root (§15.215) ──────────────────────────────────────────────────────────

/// `isqrt` is the largest whole number whose square does not pass the argument.
#[test]
fn isqrt_is_the_largest_whole_root() {
    let src = "rule root v1\n\ninputs\n  x : number  range >=0 <=1_000_000_000_000\n\noutputs\n  r : number  round down(1)\n\ndefine s : number = isqrt(x)\n\nresult r = s\n";
    assert!(codes(src).is_empty());
    for (x, want) in [(0, 0), (1, 1), (2, 1), (3, 1), (4, 2), (15, 3), (16, 4), (17, 4), (99, 9), (100, 10), (999_999_999_999, 999_999), (1_000_000_000_000, 1_000_000)] {
        assert_eq!(answer(src, &[("x", x)]), num(want), "isqrt({x})");
    }
}

/// An argument whose range goes below zero is E139, with an input that takes it there.
#[test]
fn isqrt_of_what_can_go_below_zero_is_e139() {
    let src = "rule root v1\n\ninputs\n  a : number  range >=0 <=100\n  b : number  range >=0 <=100\n\noutputs\n  r : number  round down(1)\n\ndefine s : number = isqrt(a - b)\n\nresult r = s\n";
    assert_eq!(codes(src), ["E139"]);
    let text = said(src, "E139");
    assert!(text.contains("An input that makes the argument negative: a = 0, b = 1"), "{text}");
    assert!(codes(&src.replace("isqrt(a - b)", "isqrt(max(a - b, 0))")).is_empty());
    let money = "rule root v1\n\ninputs\n  a : money[USD]  range >=0USD <=100USD\n\noutputs\n  r : number  round down(1)\n\ndefine s : number = isqrt(a)\n\nresult r = s\n";
    assert_eq!(codes(money), ["E103"]);
}

// ── The ends a divisor and an `isqrt` read (§15.299) ─────────────────────────────────────────

/// An average over the units, the range of the units written as `range` and the divisor as
/// `divisor`.
fn averaged(range: &str, divisor: &str) -> String {
    format!(
        "rule average_price v1\n\ninputs\n  total : money[USD]  range >=0USD <=1_000_000USD\n  units : number{range}\n\noutputs\n  average : money[USD]  round half_up(1USD)\n\ndefine avg : money[USD] = half_up(total / {divisor}, 1USD)\n\nresult average = avg\n"
    )
}

/// A divisor reads an end its range does not write as the end of what the type holds, as E108
/// does (§15.294): the entry of the generated code holds every input inside it. `max(units, 1)`
/// passes with an end of the units unwritten, or both, where 0.50.0 said it read a value with no
/// range and suggested `max(max(units, 1), 1)`, which stopped on the same line. `units` alone is
/// still E132, and the message says where the end it gives comes from and names an input that
/// makes the divisor zero, as it does where both ends are written.
#[test]
fn a_divisor_reads_an_end_it_does_not_write_as_the_end_of_its_type() {
    for range in ["  range >=0", "  range <=1000", ""] {
        let kept = averaged(range, "max(units, 1)");
        assert!(codes(&kept).is_empty(), "{range}: {:?}", codes(&kept));
        let src = averaged(range, "units");
        assert_eq!(codes(&src), ["E132"], "{range}");
        let text = said(&src, "E132");
        assert!(text.contains("An input that makes the divisor zero: units = 0"), "{text}");
        assert!(text.contains("zero included"), "{text}");
        assert!(text.contains("9223372036854775807"), "{text}");
        assert!(text.contains("`units` has no upper or no lower bound"), "{text}");
        assert!(text.contains("divide by `max(units, 1)`"), "{text}");
    }
    // Above zero with no upper end: the range from one is enough.
    assert!(codes(&averaged("  range >=1", "units")).is_empty());
}

/// The hint does not wrap a divisor in a second `max`. A floor below one is raised; a `max` that
/// already keeps the divisor above zero is stopped by something else, a value with no interval,
/// and the hint names no `max`. The floor is written in the unit of the divisor, for
/// `max(price, 1)` is E103 where `price` is an amount, and `max(r, 1)` where `r` is a rate.
#[test]
fn the_hint_of_e132_raises_a_floor_rather_than_wrapping_it() {
    let text = said(&averaged("  range >=0 <=1000", "max(units, 0)"), "E132");
    assert!(text.contains("divide by `max(units, 1)`"), "{text}");
    // A `define` another error has stopped has no interval, and a `max` of it with one is never zero.
    let stopped = "rule cascade v1\n\ninputs\n  total : money[USD]  range >=0USD <=1_000_000USD\n  a : number  range >=0 <=100\n  b : number  range >=1 <=100\n\noutputs\n  r : money[USD]  round half_up(1USD)\n\ndefine x : number = a / b + 1\n\ndefine q : money[USD] = half_up(total / max(x, 1), 1USD)\n\nresult r = q\n";
    assert_eq!(codes(stopped), ["E115", "E132"]);
    let text = said(stopped, "E132");
    assert!(text.contains("it reads a value with no range"), "{text}");
    assert!(!text.contains("divide by `max("), "{text}");
    let amount = "rule per_price v1\n\ninputs\n  total : money[USD]  range >=0USD <=1_000_000USD\n  price : money[USD]  range >=0USD <=10_000USD\n\noutputs\n  n : number  round down(1)\n\ndefine q : number = half_up(total / price, 1)\n\nresult n = q\n";
    assert!(said(amount, "E132").contains("divide by `max(price, 1USD)`"));
    assert!(said(&amount.replace("total / price", "total / max(price, 0USD)"), "E132").contains("divide by `max(price, 1USD)`"));
    assert!(codes(&amount.replace("total / price", "total / max(price, 1USD)")).is_empty());
    let rate = "rule base_of v1\n\ninputs\n  amount : money[USD]  range >=0USD <=1_000_000USD\n  r : rate[step 1%]  range >=0% <=100%\n\noutputs\n  base : money[USD]  round down(1USD)\n\ndefine b : money[USD] = half_up(amount / r, 1USD)\n\nresult base = b\n";
    assert!(said(rate, "E132").contains("divide by `max(r, 1%)`"));
    assert!(codes(&rate.replace("amount / r", "amount / max(r, 1%)")).is_empty());
}

/// `isqrt` reads the ends the same way: `isqrt(max(x, 0))` passes where `x` has no range, and an
/// argument that an unwritten end takes below zero is E139, which says where that end comes from
/// and names an input below zero. The hint raises a floor below zero rather than wrapping it.
#[test]
fn an_isqrt_reads_an_end_it_does_not_write_as_the_end_of_its_type() {
    let root = |range: &str, arg: &str| {
        format!("rule root v1\n\ninputs\n  x : number{range}\n\noutputs\n  r : number  round down(1)\n\ndefine s : number = isqrt({arg})\n\nresult r = s\n")
    };
    for range in ["", "  range <=100"] {
        assert!(codes(&root(range, "max(x, 0)")).is_empty(), "{range}");
        let src = root(range, "x");
        assert_eq!(codes(&src), ["E139"], "{range}");
        let text = said(&src, "E139");
        assert!(text.contains("An input that makes the argument negative: x = -1"), "{text}");
        assert!(text.contains("below zero included"), "{text}");
        assert!(text.contains("`x` has no upper or no lower bound"), "{text}");
        assert!(text.contains("pass `max(x, 0)`"), "{text}");
    }
    assert!(codes(&root("  range >=0", "x")).is_empty());
    assert!(said(&root("  range <=100", "max(x, -5)"), "E139").contains("pass `max(x, 0)`"));
}

/// The E108 of a division by a value and of an `isqrt` names an input whose range leaves an end
/// unwritten, as the E108 of a value does, rather than the ends of int64 no one wrote. Past ±2^80
/// it says so, rather than give an interval held there.
#[test]
fn the_e108_of_a_division_and_an_isqrt_names_the_input_with_no_end() {
    // The dividend, brought to the step of a grid of 0.01%.
    let dividend = "rule rate_of v1\n\ninputs\n  a : number  range >=0\n  b : number  range >=1 <=10\n\noutputs\n  r : rate  round half_up(0.01%)\n\ndefine q : rate = half_up(a / b, 0.01%)\n\nresult r = q\n";
    let text = said(dividend, "E108");
    assert!(text.contains("Cannot prove that the dividend in `half_up` fits in int64"), "{text}");
    assert!(text.contains("`a` has no upper or no lower bound"), "{text}");
    assert!(text.contains("brought to one step with the other before the division, can pass int64"), "{text}");
    assert!(text.contains("give `a` the end its range does not write"), "{text}");
    // The divisor, brought to the finer step of the dividend: a hundred times a count with no upper
    // end passes int64.
    let divisor = "rule share_of v1\n\ninputs\n  a : rate[step 0.01%]  range >=0% <=100%\n  n : number  range >=1\n\noutputs\n  r : rate  round half_up(1%)\n\ndefine q : rate = half_up(a / n, 1%)\n\nresult r = q\n";
    let text = said(divisor, "E108");
    assert!(text.contains("Cannot prove that the divisor in `half_up` fits in int64"), "{text}");
    assert!(text.contains("give `n` the end its range does not write"), "{text}");
    assert!(codes(&divisor.replace("range >=1\n", "range >=1 <=1000\n")).is_empty());
    let root = |range: &str, arg: &str| {
        format!("rule root v1\n\ninputs\n  x : number{range}\n\noutputs\n  r : number  round down(1)\n\ndefine s : number = isqrt({arg})\n\nresult r = s\n")
    };
    let square = root("  range >=0", "x * x");
    assert_eq!(codes(&square), ["E108"]);
    let text = said(&square, "E108");
    assert!(text.contains("Cannot prove that the argument of `isqrt` fits in int64"), "{text}");
    assert!(text.contains("give `x` the end its range does not write"), "{text}");
    let text = said(&root("  range >=0 <=1_000_000_000_000_000", "x * x * x"), "E108");
    assert!(text.contains("It passes ±2^80, which fits int64 on no step."), "{text}");
}

// ── Every check reads an end a range does not write as E108 does (§15.299) ───────────────────

/// An open value over `a`, whose range is `range`, and `b`, from 0 to 10, combined by `combine` from
/// `empty`; `x` reads it at a size that passes int64 if the value leaves the range it declares.
fn opened(range: &str, combine: &str, empty: &str) -> String {
    format!(
        "rule best_offer v1\n\ninputs\n  a : money[USD]{range}\n  b : money[USD]  range >=0USD <=10USD\n\noutputs\n  r : money[USD]  round down(1USD)\n\nopen best : money[USD]  combine {combine}  empty {empty}  range >=0USD <=100USD\n  a\n  b\n\ndefine x : money[USD] = best * 90000000000000000\n\nresult r = x\n"
    )
}

/// The terms of an open value read an end their ranges do not write as the end of the type: `a`
/// from 0 with no upper end takes `best` past 100USD, E131, with an input one step past the end the
/// value declares. 0.50.0 passed it, and `x`, read inside 0USD..100USD, passed int64 while the code
/// could multiply any amount `a` carries. A term with no lower end under a `max` from 50USD changes
/// nothing, W132.
#[test]
fn an_open_value_reads_an_end_it_does_not_write_as_the_end_of_its_type() {
    let src = opened("  range >=0USD", "max", "0USD");
    assert_eq!(codes(&src), ["E131"]);
    let text = said(&src, "E131");
    assert!(text.contains("An input that takes it out of the range: a = 101"), "{text}");
    assert!(text.contains("a: 0USD .. 9_223_372_036_854_775_807USD (this rule)"), "{text}");
    assert!(text.contains("`a` has no upper or no lower bound"), "{text}");
    assert!(text.contains("Hint: give `a` the end its range does not write"), "{text}");
    assert!(codes(&opened("  range >=0USD <=100USD", "max", "0USD")).is_empty());
    let low = opened("  range <=10USD", "min", "50USD");
    assert_eq!(codes(&low), ["E131"]);
    assert!(said(&low, "E131").contains("An input that takes it out of the range: a = -1"), "{}", said(&low, "E131"));
    let idle = opened("  range <=10USD", "max", "50USD");
    assert_eq!(codes(&idle), ["W132", "W132"]);
    let text = said(&idle, "W132");
    assert!(text.contains("The term a of the open value best changes nothing"), "{text}");
    assert!(text.contains("The term takes -9_223_372_036_854_775_807USD .. 10USD."), "{text}");
}

/// An open value whose own range writes one end is held to that end, the other being the end of the
/// type, which E108 holds it to: a `combine min` with a term down to −100USD leaves `range >=0USD`,
/// E131, where 0.50.0 checked only a range with both ends and passed it. A `combine max` of terms
/// from 0USD to 10USD stays inside, and both re-checkers hold the end its certificate states and
/// refuse a forged one; they refused the certificate of 0.50.0, which states no upper end.
#[test]
fn an_open_value_is_held_to_the_end_its_range_writes() {
    let one = |range: &str, combine: &str, empty: &str| opened(range, combine, empty).replacen("range >=0USD <=100USD", "range >=0USD", 1).replacen(" * 90000000000000000", "", 1);
    let src = one("  range >=-100USD <=0USD", "min", "5USD");
    assert_eq!(codes(&src), ["E131"]);
    let text = said(&src, "E131");
    assert!(text.contains("range: 0USD .. 9_223_372_036_854_775_807USD"), "{text}");
    assert!(text.contains("The end the range of best does not write is read as the end of what the type holds"), "{text}");
    assert!(text.contains("An input that takes it out of the range: a = -"), "{text}");
    let held = one("  range >=0USD <=10USD", "max", "0USD");
    assert!(codes(&held).is_empty(), "{:?}", codes(&held));
    if !have_python() {
        return;
    }
    let file = std::env::temp_dir().join(format!("rulec-open-end-{}.rule", std::process::id()));
    std::fs::write(&file, &held).unwrap();
    let (code, cert) = rulec(&["certificate", file.to_str().unwrap()]);
    assert_eq!(code, 0, "{cert}");
    let stated = r#""best":["0",null]"#;
    assert!(cert.contains(stated), "{cert}");
    let (code, out) = recheck(&cert);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("open values: 1 hold"), "{out}");
    let lean_bin = checker();
    if let Some(bin) = &lean_bin {
        let (code, out) = lean(bin, &cert, file.to_str().unwrap());
        assert_eq!(code, 0, "{out}");
    }
    let forged = cert.replacen(stated, r#""best":["5",null]"#, 1);
    let (code, out) = recheck(&forged);
    assert_ne!(code, 0, "{out}");
    assert!(out.contains("outside the range [5, 10] it declares"), "{out}");
    if let Some(bin) = &lean_bin {
        let (code, out) = lean(bin, &forged, file.to_str().unwrap());
        assert_ne!(code, 0, "{out}");
    }
    std::fs::remove_file(&file).ok();
}

/// A `derive` that reads `a`, from 0 with no upper end, through `expr`, with the range `range`; a
/// table over it picks a band.
fn derived(expr: &str, range: &str) -> String {
    format!(
        "rule fee_band v1\n\nenum band = low | high\n\ninputs\n  a : number  range >=0\n\noutputs\n  b : band\n\nderive m : number = {expr}  {range}\n\ntable bands\npolicy unique\n| m    | -> b |\n| <=50 | low  |\n| >50  | high |\n"
    )
}

/// The range of a `derive` is held to what it reaches with the end of the type in it: `min(a, 1000)`
/// reaches 1000, which `range >=0 <=100` leaves out. 0.50.0 held the range to nothing. The message
/// names `a`, and a value that passes int64 from there is E108's alone.
#[test]
fn a_derived_range_reads_an_end_it_does_not_write_as_the_end_of_its_type() {
    let src = derived("min(a, 1000)", "range >=0 <=100");
    assert_eq!(codes(&src), ["E112"]);
    let text = said(&src, "E112");
    assert!(text.contains("the reachable interval is >=0 <=1000"), "{text}");
    assert!(text.contains("`a` has no upper or no lower bound"), "{text}");
    assert!(text.contains("Hint: widen the range, or give `a` the end its range does not write"), "{text}");
    assert!(codes(&derived("min(a, 100)", "range >=0 <=100")).is_empty());
    assert_eq!(codes(&derived("a + 1", "range >=0 <=100")), ["E108"]);
}

/// A table over a `define` reads it in what it reaches: `min(a, 100)` is never above 100, and a
/// truth that is never false needs no row for false. 0.50.0 read `m` and `ok` over the whole of
/// what their types hold, and asked for rows no input reaches (E101).
#[test]
fn a_table_reads_a_define_in_what_it_reaches_with_the_end_of_the_type() {
    let band = "rule band_of v1\n\nenum band = low | high\n\ninputs\n  a : number  range >=0\n\noutputs\n  b : band\n\ndefine m : number = min(a, 100)\n\ntable bands\npolicy unique\n| m         | -> b |\n| <=50      | low  |\n| >50 <=100 | high |\n";
    assert!(codes(band).is_empty(), "{:?}", codes(band));
    let truth = "rule always_true v1\n\ninputs\n  a : number  range >=0\n\noutputs\n  n : number  round down(1)\n\ndefine ok : bool = a >= 0\n\ntable t\npolicy unique\n| ok   | -> n |\n| true | 1    |\n";
    assert!(codes(truth).is_empty(), "{:?}", codes(truth));
}

/// A rule whose `derive m` is `expr` with `range` over `a` in `a_range`; a table over `m` with
/// `rows` picks a band.
fn banded(a_range: &str, expr: &str, range: &str, rows: &str) -> String {
    format!(
        "rule fee_band v1\n\nenum band = low | high\n\ninputs\n  a : number  {a_range}\n\noutputs\n  b : band\n\nderive m : number = {expr}  {range}\n\ntable bands\npolicy unique\n| m         | -> b   |\n{rows}"
    )
}

const UP_TO_100: &str = "| <=50      | low    |\n| >50 <=100 | high   |\n";

/// A table over a `derive` reads it in what it reaches, as a table over a `define` does: an interval
/// coordinate leaves its ends out, so `>100` lies outside what `min(a, 100)` comes to however wide
/// the derive's range is written, and `<10` outside what `max(a, 10)` comes to. 0.52.0 read the
/// derive's coordinates with their ends in, and the linear model, which holds the reach of a derive
/// only beside an equation it reads, left them in: it asked for a row at `m = 101` (E101). A row
/// that asks only past 100 is E102 and says what `m` comes to, and `min(a, 1000)` still leaves 101
/// to no row. Both re-checkers hold the certificate, whose leaf past 100 says the derive does not
/// reach there, and refuse one that says so of a box it reaches.
#[test]
fn a_table_reads_a_derive_in_what_it_reaches_with_its_open_ends_left_out() {
    for a_range in ["range >=0", "range >=0 <=5000"] {
        for range in ["range >=0 <=100", "range >=0 <=1000"] {
            let src = banded(a_range, "min(a, 100)", range, UP_TO_100);
            assert!(codes(&src).is_empty(), "{a_range}, {range}: {:?}", codes(&src));
        }
    }
    let from_10 = banded("range >=0 <=5000", "max(a, 10)", "range >=10 <=5000", "| >=10 <=50 | low    |\n| >50       | high   |\n");
    assert!(codes(&from_10).is_empty(), "{:?}", codes(&from_10));
    let past = banded("range >=0", "min(a, 1000)", "range >=0 <=1000", UP_TO_100);
    assert_eq!(codes(&past), ["E101"]);
    let text = said(&past, "E101");
    assert!(text.contains("An input that matches no row: m = 101"), "{text}");
    let over = banded("range >=0 <=5000", "min(a, 100)", "range >=0 <=1000", &format!("{UP_TO_100}| >100      | over   |\n"))
        .replace("enum band = low | high\n", "enum band = low | high | over\n");
    assert_eq!(codes(&over), ["E102"]);
    let text = said(&over, "E102");
    assert!(text.contains("Over the ranges of the inputs, `m` can only come to >=0 <=100."), "{text}");
    if !have_python() {
        return;
    }
    let src = banded("range >=0 <=5000", "min(a, 100)", "range >=0 <=1000", UP_TO_100);
    let file = std::env::temp_dir().join(format!("rulec-derive-reach-{}.rule", std::process::id()));
    std::fs::write(&file, &src).unwrap();
    let (code, cert) = rulec(&["certificate", file.to_str().unwrap()]);
    assert_eq!(code, 0, "{cert}");
    let cover = r#""cover":{"split":[{"row":1},{"row":1},{"row":2},{"row":2},{"derived_axis":0}]}"#;
    assert!(cert.contains(cover), "{cert}");
    let (code, out) = recheck(&cert);
    assert_eq!(code, 0, "{out}");
    let lean_bin = checker();
    if let Some(bin) = &lean_bin {
        let (code, out) = lean(bin, &cert, file.to_str().unwrap());
        assert_eq!(code, 0, "{out}");
    }
    // The same leaf on the coordinate between 50 and 100, which the derive reaches.
    let forged = cert.replacen(cover, r#""cover":{"split":[{"row":1},{"row":1},{"derived_axis":0},{"row":2},{"derived_axis":0}]}"#, 1);
    let (code, out) = recheck(&forged);
    assert_ne!(code, 0, "{out}");
    assert!(out.contains("m can reach this box, so it is not impossible"), "{out}");
    if let Some(bin) = &lean_bin {
        let (code, out) = lean(bin, &forged, file.to_str().unwrap());
        assert_ne!(code, 0, "{out}");
    }
    std::fs::remove_file(&file).ok();
}

/// Lines over a quantity from 0 with no upper end; the first heavy line's quantity is the answer of
/// a fold into `r`, and `tail` is written after it.
fn heavy(tail: &str) -> String {
    format!(
        "rule first_heavy v1\n\nenum line_pick = heavy | light\n\nelements lines\n  q : number  range >=0\n\noutputs\n  r : number  round down(1)  range >=0\n\ntable pick_line\npolicy unique\n| q    | -> pick : line_pick |\n| >=10 | heavy               |\n| <10  | light               |\n\nfold pick over lines -> r\n  heavy     -> take_first q\n  light     -> next\n  empty     -> 0\n  exhausted -> held\n{tail}"
    )
}

/// The answer of a fold is the hull of what its arms answer, an end no range writes read as the end
/// of the type: a range from 0 on `r` holds, the certificate lists it, and both re-checkers work it
/// out and hold it. A range that leaves out what the arms answer is refused by both. Twice the
/// answer passes int64, E108, where 0.50.0 passed it and both re-checkers refused its certificate.
#[test]
fn the_answer_of_a_fold_reads_an_end_it_does_not_write_as_the_end_of_its_type() {
    assert!(codes(&heavy("")).is_empty(), "{:?}", codes(&heavy("")));
    let twice = heavy("").replace("  r : number  round down(1)  range >=0\n", "  r : number  round down(1)\n  s : number  round down(1)\n") + "\ndefine s : number = r * 2\n";
    assert_eq!(codes(&twice), ["E108"]);
    if !have_python() {
        return;
    }
    let file = std::env::temp_dir().join(format!("rulec-fold-ends-{}.rule", std::process::id()));
    std::fs::write(&file, heavy("")).unwrap();
    let (code, cert) = rulec(&["certificate", file.to_str().unwrap()]);
    assert_eq!(code, 0, "{cert}");
    let stated = r#""ranges":{"q":["0",null],"r":["0",null]}"#;
    assert!(cert.contains(stated), "{cert}");
    let (code, out) = recheck(&cert);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("1 of them with an end read at the end of its type"), "{out}");
    let lean_bin = checker();
    if let Some(bin) = &lean_bin {
        let (code, out) = lean(bin, &cert, file.to_str().unwrap());
        assert_eq!(code, 0, "{out}");
    }
    let forged = cert.replacen(stated, r#""ranges":{"q":["0",null],"r":["0","100"]}"#, 1);
    assert_ne!(forged, cert);
    let (code, out) = recheck(&forged);
    assert_ne!(code, 0, "{out}");
    if let Some(bin) = &lean_bin {
        let (code, out) = lean(bin, &forged, file.to_str().unwrap());
        assert_ne!(code, 0, "{out}");
    }
    std::fs::remove_file(&file).ok();
}

// ── A constraint with a sum on one side (§15.216) ────────────────────────────────────────────

const QUOTA: &str = "rule quota v1\n\nenum headroom = used_up | low | ample\n\ninputs\n  used      : number  range >=0 <=1000\n  requested : number  range >=1 <=1000\n  quota     : number  range >=1 <=1000\n\nconstraint used + requested <= quota\n\noutputs\n  status : headroom\n\nderive left : number = quota - used - requested  range >=-1999 <=999\n\ntable remaining\npolicy unique\n| left     | -> status : headroom |\n| 0        | used_up              |\n| >=1 <=99 | low                  |\n| >=100    | ample                |\n";

/// One side of a constraint adds inputs up, written either way round. Both sides at once, a
/// constant in the sum, and something other than an input are each refused.
#[test]
fn a_constraint_adds_inputs_up_on_one_side() {
    assert!(codes(QUOTA).is_empty());
    let line = "constraint used + requested <= quota";
    assert!(codes(&QUOTA.replace(line, "constraint quota >= used + requested")).is_empty());
    assert_eq!(codes(&QUOTA.replace(line, "constraint used + requested <= quota + used")), ["E017"]);
    assert_eq!(codes(&QUOTA.replace(line, "constraint used + 1 <= quota")), ["E017"]);
    let text = said(&QUOTA.replace(line, "constraint used + requested <= quota + used"), "E017");
    assert!(text.contains("One side may add inputs up with `+`"), "{text}");
    assert_eq!(codes(&QUOTA.replace(line, "constraint used + left <= quota")), ["E018"]);
}

/// The sieve reads a sum: the table has no row for what is left below zero, and needs none. The
/// same table without the line has a gap, and its input breaks the sum.
#[test]
fn the_sieve_reads_a_sum() {
    let without = QUOTA.replace("constraint used + requested <= quota\n\n", "");
    assert_ne!(without, QUOTA);
    rulec::i18n::set(rulec::i18n::Lang::En);
    let ds = rulec::check_source(&without, "quota.rule");
    let d = ds.iter().find(|d| d.code == "E101").expect("no E101 without the constraint");
    // "An input producing this example: quota = 1, requested = 1, used = 1"
    let note = d.notes.iter().find_map(|n| n.strip_prefix("An input producing this example: ")).unwrap_or_else(|| panic!("{:?}", d.notes));
    let get = |name: &str| -> i64 {
        note.split(", ").find_map(|p| p.strip_prefix(&format!("{name} = "))).and_then(|v| v.parse().ok()).unwrap_or_else(|| panic!("{name}: {note}"))
    };
    assert!(get("used") + get("requested") > get("quota"), "the gap is an input the sum rules out: {note}");
}

/// An example that breaks a sum is an error, not a case.
#[test]
fn an_example_that_breaks_a_sum_is_e019() {
    let src = format!("{QUOTA}\nexamples\n| used | requested | quota | -> status |\n| 400  | 700       | 1000  | used_up   |\n");
    assert_eq!(codes(&src), ["E019"]);
}

/// The vectors keep to the sum, and the refused ones the generated code is held to break it
/// inside every range: those are the inputs the door turns away in every language.
#[test]
fn the_vectors_keep_to_a_sum_and_the_refused_ones_break_it() {
    let tmp = TempDir::new("arith-quota");
    let dir = tmp.path().to_path_buf();
    let out = dir.to_string_lossy().to_string();
    let (c, said) = rulec(&rulec::targets::up_to_0_40(&["gen", "tests/corpus/quota_request.rule", "--out", &out]));
    assert_eq!(c, 0, "{said}");
    let read = |name: &str| -> Vec<rulec::json::Json> {
        std::fs::read_to_string(dir.join("vectors").join(name))
            .unwrap()
            .lines()
            .map(|l| rulec::json::parse(l).unwrap())
            .collect()
    };
    let n = |j: &rulec::json::Json, k: &str| j.get("in").and_then(|i| i.get(k)).and_then(|v| v.as_int()).unwrap_or_else(|| panic!("{k}"));
    let kept = read("quota_request.jsonl");
    assert!(!kept.is_empty());
    for v in &kept {
        assert!(n(v, "used") + n(v, "requested") <= n(v, "quota"), "a vector breaks the sum");
    }
    // A refused case can leave an input out or send it as a string; the ones of interest send
    // three whole numbers.
    let whole = |j: &rulec::json::Json, k: &str| j.get("in").and_then(|i| i.get(k)).and_then(|v| v.as_int());
    let refused = read("quota_request.refused.jsonl");
    let breaks = refused.iter().filter(|j| {
        match (whole(j, "used"), whole(j, "requested"), whole(j, "quota")) {
            (Some(u), Some(r), Some(q)) => (0..=1000).contains(&u) && (1..=1000).contains(&r) && (1..=1000).contains(&q) && u + r > q,
            _ => false,
        }
    });
    assert!(breaks.count() > 0, "no refused vector breaks the sum inside the ranges");
}

/// Without the line, the strength of one side would only have its range checked (W111); with it,
/// the input takes part in what the rule says can happen.
#[test]
fn a_sum_constraint_puts_an_input_to_use() {
    let src = std::fs::read_to_string(root().join("tests/corpus/contested_roll.rule")).unwrap();
    assert!(codes(&src).is_empty());
    let line = "constraint roll < challenger_strength + holder_strength\n";
    assert!(src.contains(line));
    // The claims of `chances` rest on the line as well (§15.224), so they go with it.
    let at = src.find("\n# The chance of each side").expect("the claims are gone");
    let to = at + src[at..].find("\n\nexamples").expect("the examples are gone");
    let without = format!("{}{}", &src[..at], &src[to..]).replace(line, "");
    assert_eq!(codes(&without), ["W111"]);
}

/// A contract the inputs are read from (§15.140) has to promise the sum the rule takes for granted
/// (E123). One that relates only two of the three fields lets a request through that breaks it, and
/// the message names every field the sum adds, with the CEL that would promise it.
#[test]
fn a_contract_has_to_promise_the_sum() {
    let tmp = TempDir::new("arith-contract");
    let dir = tmp.path().to_path_buf();
    let proto = |cel: &str| {
        format!("syntax = \"proto3\";\n\npackage quota.v1;\n\nimport \"buf/validate/validate.proto\";\n\nmessage QuotaRequest {{\n  option (buf.validate.message).cel = {{\n    id: \"within_quota\",\n    message: \"what was used and what is asked come to more than the quota\",\n    expression: \"{cel}\"\n  }};\n\n  int64 used = 1 [(buf.validate.field).int64 = {{gte: 0, lte: 1000}}];\n  int64 requested = 2 [(buf.validate.field).int64 = {{gte: 1, lte: 1000}}];\n  int64 quota = 3 [(buf.validate.field).int64 = {{gte: 1, lte: 1000}}];\n}}\n")
    };
    let rule = QUOTA
        .replace("enum headroom = used_up | low | ample\n", "enum headroom = used_up | low | ample\n\nshape request = proto \"quota.proto\" quota.v1.QuotaRequest\n")
        .replace("used      : number  range >=0 <=1000", "used      : number  range >=0 <=1000  from request.used")
        .replace("requested : number  range >=1 <=1000", "requested : number  range >=1 <=1000  from request.requested")
        .replace("quota     : number  range >=1 <=1000", "quota     : number  range >=1 <=1000  from request.quota");
    assert_eq!(rule.matches("from request.").count(), 3);
    let path = dir.join("quota.rule");
    std::fs::write(&path, &rule).unwrap();
    let check = || rulec(&["check", path.to_str().unwrap()]);
    std::fs::write(dir.join("quota.proto"), proto("this.used + this.requested <= this.quota")).unwrap();
    let (c, out) = check();
    assert_eq!(c, 0, "{out}");
    std::fs::write(dir.join("quota.proto"), proto("this.used <= this.quota")).unwrap();
    let (c, out) = check();
    assert_eq!(c, 1, "{out}");
    assert!(out.contains("error[E123]: The contract lets `request.used`, `request.requested` and `request.quota` through together where `constraint used + requested <= quota` does not hold"), "{out}");
    assert!(out.contains(r#"expression: "this.used + this.requested <= this.quota""#), "{out}");
}

/// What a caller has to keep to is said to it: `rulec api` carries the sum, a side as a list.
#[test]
fn the_api_says_the_sum() {
    let (c, out) = rulec(&["api", "tests/corpus/quota_request.rule"]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains(r#""preconditions":[{"kind":"constraint","left":["used","requested"],"op":"<=","right":"quota"}]"#), "{out}");
}

// ── W128, decided by the hole a row leaves (§15.213) ─────────────────────────────────────────

/// A row the completeness check needs is reached: taking it out opens a hole, and the hole's
/// input is the row's. Here the column is a remainder, which the search for a row's input cannot
/// run backwards; the hole the completeness check finds without the row hands one over, and the
/// vectors cover the row with it.
#[test]
fn a_row_completeness_needs_is_reached_by_the_hole_it_leaves() {
    let src = "rule slot_pick v1\n\nenum slot = early | lucky | late\n\ninputs\n  ticket : number  range >=0 <=10_000\n\noutputs\n  pick : slot\n\ndefine tail : number = mod(ticket * 3 + 1, 1000)\n\ntable slots\npolicy unique\n| tail  | -> pick : slot |\n| <=776 | early          |\n| 777   | lucky          |\n| >=778 | late           |\n";
    assert!(codes(src).is_empty(), "{:?}", codes(src));
    rulec::i18n::set(rulec::i18n::Lang::En);
    let (f, c) = rulec::prepare(src, "slot.rule").unwrap();
    let vs = rulec::vectors::generate(&f, &c);
    assert!(vs.iter().any(|v| v.trace.iter().any(|t| t == "table slots row 2")), "no vector reaches row 2");
    let a = rulec::coverage::audit(&f, &c, "slot.rule", &vs, &[]);
    assert_eq!(a.tally[rulec::coverage::ROW], (3, 3), "every row is reached");
    // The row that only looks needed stays undecided: in m_w114 the hole without row 3 is the
    // elimination's, over the rationals, and no whole number is in it.
    let w114 = std::fs::read_to_string(root().join("tests/mutants/m_w114.rule")).unwrap();
    assert!(codes(&w114).contains(&"W128"));
}

// ── The certificate (§15.213 to §15.216) ─────────────────────────────────────────────────────

/// The new shapes travel in the certificate — a divided rounding and a remainder and a root as
/// expressions, a sum as a side of a constraint — and both re-checkers work each claim out again
/// from them. A lie about any of them is caught.
#[test]
fn both_rechecks_work_the_new_shapes_out_again() {
    let lies: &[(&str, &str, &str)] = &[
        ("tests/corpus/average_unit_price.rule", r#""interval":["0","1000000"]"#, r#""interval":["0","999999"]"#),
        // `down` to the yen: 31,000,000 / 28 is 1,107,142.857…, which it cuts to 1,107,142 (§15.303).
        ("tests/corpus/daily_proration.rule", r#""interval":["0","1107142"]"#, r#""interval":["0","1000000"]"#),
        ("tests/corpus/duty_rotation.rule", r#""interval":["0","2"]"#, r#""interval":["0","1"]"#),
        ("tests/corpus/economic_order_quantity.rule", r#""interval":["0","447213"]"#, r#""interval":["0","447212"]"#),
    ];
    let python = ready(Need::Python, have_python, "no python3");
    let bin = checker();
    for (rel, honest, forged) in lies {
        let (c, cert) = rulec(&["certificate", rel]);
        assert_eq!(c, 0, "{cert}");
        assert!(cert.contains(honest), "{rel}: {cert}");
        let lie = cert.replacen(honest, forged, 1);
        if python {
            assert_eq!(recheck(&cert).0, 0, "{rel}: {}", recheck(&cert).1);
            let (code, out) = recheck(&lie);
            assert_eq!(code, 1, "{rel}: a narrowed interval passed\n{out}");
        }
        if let Some(bin) = &bin {
            let (code, out) = lean(bin, &cert, rel);
            assert_eq!(code, 0, "{rel}: {out}");
            assert!(out.contains("OK: every claim"), "{rel}: {out}");
            let (code, out) = lean(bin, &lie, rel);
            assert_eq!(code, 1, "{rel}: a narrowed interval passed\n{out}");
        }
    }
    // The sum is what rules the negative remainder out: with it dropped, the leaf that rests on
    // it adds up to nothing.
    let (c, cert) = rulec(&["certificate", "tests/corpus/quota_request.rule"]);
    assert_eq!(c, 0, "{cert}");
    let sum = r#"{"left":["used","requested"],"op":"<=","right":"quota"}"#;
    assert_eq!(cert.matches(sum).count(), 2, "{cert}");
    let lie = cert.replace(sum, r#"{"left":["used"],"op":"<=","right":"quota"}"#);
    if python {
        assert_eq!(recheck(&cert).0, 0, "{}", recheck(&cert).1);
        let (code, out) = recheck(&lie);
        assert_eq!(code, 1, "a sum with an input dropped passed\n{out}");
    }
    if let Some(bin) = &bin {
        let (code, out) = lean(bin, &cert, "tests/corpus/quota_request.rule");
        assert_eq!(code, 0, "{out}");
        let (code, out) = lean(bin, &lie, "tests/corpus/quota_request.rule");
        assert_eq!(code, 1, "a sum with an input dropped passed\n{out}");
    }
}

/// A rule of one `define s` over `a` (with the range given) and `b` (0 to 100), read by `result r`.
fn on_the_way(range: &str, ty: &str, expr: &str) -> String {
    format!(
        "rule steps v1\n\ninputs\n  a : {ty}  {range}\n  b : number  range >=0 <=100\n\noutputs\n  r : number  round down(1)\n\ndefine s : number = {expr}\n\nresult r = s + b\n"
    )
}

/// Every integer the generated code holds on the way to a value is held to int64, not only the value
/// (§15.296): the product inside a `min` whose answer is never above 10, a value brought to the step
/// of its rounding grid before it is rounded, a sum that passes int64 before the difference brings it
/// back, a product inside a rounding. E108 marks the deepest such integer and says why the code holds
/// it at that step. Each of these passed `check`, and Go multiplied in int64.
#[test]
fn every_integer_on_the_way_is_held_to_int64() {
    let closed = "range >=0 <=1000";
    for (expr, part) in [
        ("min(a * 1000000000000000000, 10)", "a * 1000000000000000000"),
        ("half_down(a * 1000000000000000000 / 1000000000000000000, 1)", "a * 1000000000000000000"),
        ("a + 9223372036854775000 - 9223372036854775000", "a + 9223372036854775000"),
        ("down(a * 10000000000000000 * 10%, 1)", "a * 10000000000000000"),
    ] {
        let src = on_the_way(closed, "number", expr);
        assert_eq!(codes(&src), ["E108"], "{expr}");
        let said = said(&src, "E108");
        assert!(said.contains("Cannot prove that s fits in int64"), "{expr}: {said}");
        assert!(said.contains(&format!("the value {part} on the way")), "{expr}: {said}");
        assert!(said.contains("so the stored integer reaches"), "{expr}: {said}");
    }
    // The two sides of a `min` are brought to the step they share before they are compared: `a` in
    // whole percents is held in thousandths of a percent.
    let sides = "rule sides v1\n\ninputs\n  a : rate[step 1%]  range >=0% <=100_000_000_000_000_000%\n\noutputs\n  r : rate  round down(0.001%)\n\ndefine s : rate = min(a, 0.001%)\n\nresult r = s\n";
    assert_eq!(codes(sides), ["E108"]);
    let text = said(sides, "E108");
    assert!(text.contains("the value a on the way") && text.contains("brought to the step they share, 1/100000"), "{text}");
    // What the code holds on the way fits: nothing is said.
    for expr in ["min(a, 10)", "down(a * 10%, 1)", "half_down(a * 1000 / 1000, 1)", "max(a - 5, 0) + 1"] {
        let src = on_the_way(closed, "number", expr);
        assert!(codes(&src).is_empty(), "{expr}: {:?}", codes(&src));
    }
}

/// A truth value has no interval of its own, and the two sides it compares are brought to one step
/// (§15.296): `a > 0.001%` over a rate in whole percents multiplies `a` by 1000 before it compares,
/// and with no upper bound on `a` that passes int64. The hint names `a`.
#[test]
fn the_sides_of_a_comparison_are_held_where_they_meet() {
    let rule = |range: &str| {
        format!(
            "rule sides v1\n\ninputs\n  a : rate[step 1%]  {range}\n\noutputs\n  r : number  round down(1)\n\ndefine big : bool = a > 0.001%\n\ntable t\npolicy unique\n| big   | -> r |\n| true  | 1    |\n| false | 0    |\n"
        )
    };
    let src = rule("range >=0%");
    assert_eq!(codes(&src), ["E108"]);
    let said = said(&src, "E108");
    assert!(said.contains("Cannot prove that big fits in int64"), "{said}");
    assert!(said.contains("brought to the step they share, 1/100000"), "{said}");
    assert!(said.contains("`a` has no upper or no lower bound") && said.contains("give `a` the end its range does not write"), "{said}");
    assert!(codes(&rule("range >=0% <=100%")).is_empty());
}

/// What the code holds once a value is worked out (§15.296): a value a row names is brought to the
/// column's step, and an output to the step it travels at and its rounding grid share, where it is
/// rounded. `r = a` over a rate in whole percents, into a column or an output in finer steps,
/// passed with no upper bound on `a`, and Go multiplied it in int64.
#[test]
fn a_column_and_an_output_are_held_where_the_code_holds_them() {
    let column = |range: &str| {
        format!(
            "rule col v1\n\ninputs\n  a : rate[step 1%]  {range}\n  k : bool\n\noutputs\n  r : rate  round down(0.001%)\n\ntable t\npolicy unique\n| k     | -> r   |\n| true  | a      |\n| false | 0.001% |\n"
        )
    };
    let src = column("range >=0%");
    assert_eq!(codes(&src), ["E108"]);
    let text = said(&src, "E108");
    assert!(text.contains("Cannot prove that r fits in int64"), "{text}");
    assert!(text.contains("What a table writes into r is held on the column's step of 1/100000"), "{text}");
    assert!(text.contains("`a` has no upper or no lower bound"), "{text}");
    assert!(codes(&column("range >=0% <=100%")).is_empty());

    let output = |range: &str| {
        format!("rule out v1\n\ninputs\n  a : rate[step 1%]  {range}\n\noutputs\n  r : rate[step 0.01%]  round down(0.01%)\n\nresult r = a\n")
    };
    let src = output("range >=0%");
    assert_eq!(codes(&src), ["E108"]);
    let text = said(&src, "E108");
    assert!(text.contains("brings r to the step of 1/10000, which divides both the step it travels at and its rounding grid"), "{text}");
    assert!(text.contains("`a` has no upper or no lower bound"), "{text}");
    assert!(codes(&output("range >=0% <=100%")).is_empty());

    // A value that fits on its own step passes int64 once it is brought to the output's finer one.
    let finer = output("range >=0% <=100_000_000_000_000_000%");
    assert_eq!(codes(&finer), ["E108"]);
    assert!(said(&finer, "E108").contains("so the stored integer reaches 10000000000000000000"), "{}", said(&finer, "E108"));
}

/// `half_up`, `half_down` and `half_even` compare twice the remainder with the grid, and `down` and
/// `up` do not (§15.296): a grid past half of int64 is E108 under the first three only. The
/// certificate states the mode of each output's rounding, and both re-checkers read it.
#[test]
fn twice_the_grid_is_held_where_the_rounding_halves() {
    let rule = |mode: &str| {
        format!("rule wide_grid v1\n\ninputs\n  a : number  range >=0 <=100\n\noutputs\n  r : number  round {mode}(5_000_000_000_000_000_000)\n\nresult r = a\n")
    };
    for mode in ["half_up", "half_down", "half_even"] {
        let src = rule(mode);
        assert_eq!(codes(&src), ["E108"], "{mode}");
        let text = said(&src, "E108");
        let why = format!("A rounding by `{mode}` compares twice the remainder with the grid, and twice the grid, counted on the step of 1/1, passes int64.");
        assert!(text.contains(&why), "{mode}: {text}");
    }
    for mode in ["down", "up"] {
        assert!(codes(&rule(mode)).is_empty(), "{mode}: {:?}", codes(&rule(mode)));
    }
    if !have_python() {
        return;
    }
    let file = std::env::temp_dir().join(format!("rulec-wide-grid-{}.rule", std::process::id()));
    std::fs::write(&file, rule("down")).unwrap();
    let (code, cert) = rulec(&["certificate", file.to_str().unwrap()]);
    assert_eq!(code, 0, "{cert}");
    let mode = r#""mode":"down""#;
    assert!(cert.contains(mode), "{cert}");
    let (code, out) = recheck(&cert);
    assert_eq!(code, 0, "{out}");
    let bin = checker();
    if let Some(bin) = &bin {
        let (code, out) = lean(bin, &cert, file.to_str().unwrap());
        assert_eq!(code, 0, "{out}");
    }
    for (what, to) in [("a mode that halves", r#""mode":"half_up""#), ("a mode no rounding has", r#""mode":"sideways""#)] {
        let forged = cert.replacen(mode, to, 1);
        assert_ne!(forged, cert, "{what}");
        let (code, out) = recheck(&forged);
        assert_ne!(code, 0, "{what}: {out}");
        if let Some(bin) = &bin {
            let (code, out) = lean(bin, &forged, file.to_str().unwrap());
            assert_ne!(code, 0, "{what}: {out}");
        }
    }
    std::fs::remove_file(&file).ok();
}

/// What a rounding puts on the grid runs between the two ends of what decides it, each rounded the
/// way the mode rounds it (§15.296). A tenth of an amount with no range is held on the step of 1/10 at
/// the end of the type: `down` takes no value away from zero, so the code holds it, and `up` and the
/// three halves take the end a step past int64. Both re-checkers round the ends by the mode the
/// certificate states.
#[test]
fn a_rounding_is_held_the_way_its_mode_rounds() {
    let rule = |mode: &str| {
        format!("rule tenth v1\n\ninputs\n  amount : money[JPY]\n\noutputs\n  fee : money[JPY]  round {mode}(1JPY)\n\nresult fee = amount ÷ 10\n\nexamples\n| amount  | -> fee |\n| 1000JPY | 100JPY |\n")
    };
    assert!(codes(&rule("down")).is_empty(), "{:?}", codes(&rule("down")));
    for mode in ["up", "half_up", "half_down", "half_even"] {
        let src = rule(mode);
        assert_eq!(codes(&src), ["E108"], "{mode}");
        let text = said(&src, "E108");
        assert!(text.contains("The generated code brings fee to the step of 1/10") && text.contains("`amount` has no upper or no lower bound"), "{mode}: {text}");
    }
    if !have_python() {
        return;
    }
    let file = std::env::temp_dir().join(format!("rulec-tenth-{}.rule", std::process::id()));
    std::fs::write(&file, rule("down")).unwrap();
    let (code, cert) = rulec(&["certificate", file.to_str().unwrap()]);
    assert_eq!(code, 0, "{cert}");
    let held = r#"{"name":"fee","of":"output","interval":["-9223372036854775807/10","9223372036854775807/10"],"scale":10,"stored_max":"9223372036854775807","mode":"down"}"#;
    assert!(cert.contains(held), "{cert}");
    let (code, out) = recheck(&cert);
    assert_eq!(code, 0, "{out}");
    let bin = checker();
    if let Some(bin) = &bin {
        let (code, out) = lean(bin, &cert, file.to_str().unwrap());
        assert_eq!(code, 0, "{out}");
    }
    let forged = cert.replacen(r#""mode":"down""#, r#""mode":"up""#, 1);
    assert_ne!(forged, cert);
    let (code, out) = recheck(&forged);
    assert_ne!(code, 0, "{out}");
    if let Some(bin) = &bin {
        let (code, out) = lean(bin, &forged, file.to_str().unwrap());
        assert_ne!(code, 0, "{out}");
    }
    std::fs::remove_file(&file).ok();
}

/// Three values with no upper bound multiplied pass 128 bits. The arithmetic of the interval wrapped
/// in a release build, and stopped `check` with a panic in a debug one; it is held at ±2^80 now, and
/// E108 says the value passes that (§15.296).
#[test]
fn a_product_past_128_bits_is_e108_and_no_panic() {
    let src = "rule mul3 v1\n\ninputs\n  a : number  range >=0\n  b : number  range >=0\n  c : number  range >=0\n\noutputs\n  r : number  round down(1)\n\nresult r = a * b * c\n";
    assert_eq!(codes(src), ["E108"]);
    let wide = "rule mul3 v1\n\ninputs\n  a : number  range >=0 <=1000000000000000\n  b : number  range >=0 <=1000000000000000\n  c : number  range >=0 <=1000000000000000\n\noutputs\n  r : number  round down(1)\n\nresult r = a * b * c\n";
    assert_eq!(codes(wide), ["E108"]);
    assert!(said(wide, "E108").contains("The reachable interval passes ±2^80, which fits int64 on no step."), "{}", said(wide, "E108"));
}

/// What a fold works out for an element is held to int64 like a value (§15.296).
#[test]
fn what_a_fold_takes_is_held_to_int64() {
    let fold = |k: &str| {
        format!(
            "rule picks v1\n\nenum pick = take | skip\n\nelements items\n  price : number  range >=0 <=500\n\noutputs\n  best : number  round down(1)\n\ntable choose\npolicy unique\n| price | -> p : pick |\n| <=100 | take        |\n| >100  | skip        |\n\nfold p over items -> best\n  take      -> take_first price * {k}\n  skip      -> next\n  empty     -> 0\n  exhausted -> held\n"
        )
    };
    let src = fold("1000000000000000000");
    assert_eq!(codes(&src), ["E108"], "{:?}", codes(&src));
    assert!(said(&src, "E108").contains("Cannot prove that best fits in int64"), "{}", said(&src, "E108"));
    assert!(codes(&fold("1000")).is_empty(), "{:?}", codes(&fold("1000")));
}

/// The certificate states every integer on the way to each value (`steps`), what the code holds once
/// a value is worked out (`held`) and what each fold works out (`fold_values`), and both re-checkers
/// work each out again: a narrower interval, a smaller stored integer, a step left out, a held value
/// left out, and a scale that lets an integer past int64 are refused by both (§15.296).
#[test]
fn the_certificate_states_what_the_code_holds() {
    if !have_python() {
        return;
    }
    let src = "rule holds v1\n\ninputs\n  a : number  range >=0 <=1000\n  p : rate[step 1%]  range >=0% <=100%\n  k : bool\n\noutputs\n  r : number  round down(1)\n  q : rate[step 0.01%]  round down(0.01%)\n\ndefine s : number = min(a * 1000, 5000) + a\n\ntable t\npolicy unique\n| k     | -> q   |\n| true  | p      |\n| false | 0.5%   |\n\nresult r = s\n";
    let file = std::env::temp_dir().join(format!("rulec-held-{}.rule", std::process::id()));
    std::fs::write(&file, src).unwrap();
    let (code, cert) = rulec(&["certificate", file.to_str().unwrap()]);
    assert_eq!(code, 0, "{cert}");
    let step = r#"{"at":[0,0,0],"scale":1,"interval":["0","1000"],"stored_max":"1000"}"#;
    assert!(cert.contains(step), "{cert}");
    let held_q = r#"{"name":"q","of":"column","interval":["0","1"],"scale":10000,"stored_max":"10000"}"#;
    assert!(cert.contains(held_q), "{cert}");
    let held_r = r#"{"name":"r","of":"output","interval":["0","6000"],"scale":1,"stored_max":"6000","mode":"down"}"#;
    assert!(cert.contains(held_r), "{cert}");
    let (code, out) = recheck(&cert);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("int64: 2 values fit, and 8 integers on the way to them"), "{out}");
    assert!(out.contains("held: 1 columns and 2 outputs fit int64 where the code holds them"), "{out}");
    let bin = checker();
    if let Some(bin) = &bin {
        let (code, out) = lean(bin, &cert, file.to_str().unwrap());
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("values: 2 typed, 2 held to int64, and 8 integers on the way to them"), "{out}");
        assert!(out.contains("held: 1 columns and 2 outputs fit int64 where the code holds them"), "{out}");
    }
    for (what, from, to) in [
        ("a narrower step", step, r#"{"at":[0,0,0],"scale":1,"interval":["0","10"],"stored_max":"1000"}"#),
        ("a smaller stored integer on the way", step, r#"{"at":[0,0,0],"scale":1,"interval":["0","1000"],"stored_max":"10"}"#),
        ("a step left out", &format!("{step},"), ""),
        ("a step past int64", step, r#"{"at":[0,0,0],"scale":100000000000000000,"interval":["0","1000"],"stored_max":"100000000000000000000"}"#),
        ("a column left out", &format!("{held_q},"), ""),
        ("a narrower column", held_q, r#"{"name":"q","of":"column","interval":["0","0.5"],"scale":10000,"stored_max":"10000"}"#),
        ("a smaller stored output", held_r, r#"{"name":"r","of":"output","interval":["0","6000"],"scale":1,"stored_max":"60","mode":"down"}"#),
    ] {
        let forged = cert.replacen(from, to, 1);
        assert_ne!(forged, cert, "{what}");
        let (code, out) = recheck(&forged);
        assert_ne!(code, 0, "{what}: {out}");
        if let Some(bin) = &bin {
            let (code, out) = lean(bin, &forged, file.to_str().unwrap());
            assert_ne!(code, 0, "{what}: {out}");
        }
    }
    std::fs::remove_file(&file).ok();
}
