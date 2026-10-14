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
    let (c, said) = rulec(&["gen", "tests/corpus/quota_request.rule", "--out", &out]);
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
        ("tests/corpus/daily_proration.rule", r#""interval":["0","1107143"]"#, r#""interval":["0","1000000"]"#),
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
