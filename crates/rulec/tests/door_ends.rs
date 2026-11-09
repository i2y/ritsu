//! The ends a door holds a number to (§15.295): each end its range writes, and on a side it writes
//! none, the end of what its type holds, ±(2^63−1). The reference evaluator and the door of every
//! generated language refuse the same values with the same sentence, held by `rulec test`; the
//! vectors carry the one below a written lower end, the one above a written upper end, and for a
//! number whose range writes no lower end, the least int64. A value past 2^63−1 reaches the door only
//! in the languages whose integers have no end, and they refuse it too. A date is held to the day its
//! range writes, and its reader holds it to the years 0001 to 9999.

use ritsu_testkit::{Need, TempDir, ready, skip};
use std::path::Path;
use std::process::Command;

/// A lower end only, which the table leans on: the rows start at 0JPY.
const HALF_LOW: &str = "rule half_low v1\n\
     description \"A fee whose amount has a lower end only\"\n\n\
     inputs\n  amount : money[JPY]  range >=0JPY\n\n\
     outputs\n  fee : money[JPY]  round down(1JPY)\n\n\
     table fees\npolicy unique\n\
     | amount          | -> fee : money[JPY] |\n\
     | >=0JPY <1000JPY | 100JPY              |\n\
     | >=1000JPY       | 0JPY                |\n\n\
     examples\n| amount  | -> fee |\n| 0JPY    | 100JPY |\n| 1000JPY | 0JPY   |\n";

/// An upper end only.
const HALF_HIGH: &str = "rule half_high v1\n\
     description \"A fee whose amount has an upper end only\"\n\n\
     inputs\n  amount : money[JPY]  range <=100000JPY\n\n\
     outputs\n  fee : money[JPY]  round down(1JPY)\n\n\
     table fees\npolicy unique\n\
     | amount                | -> fee : money[JPY] |\n\
     | <1000JPY              | 100JPY              |\n\
     | >=1000JPY <=100000JPY | 0JPY                |\n\n\
     examples\n| amount  | -> fee |\n| -5JPY   | 100JPY |\n| 1000JPY | 0JPY   |\n";

/// No range at all.
const NO_RANGE: &str = "rule no_range v1\n\
     description \"A fee whose amount has no range\"\n\n\
     inputs\n  amount : money[JPY]\n\n\
     outputs\n  fee : money[JPY]  round down(1JPY)\n\n\
     table fees\npolicy unique\n\
     | amount    | -> fee : money[JPY] |\n\
     | <1000JPY  | 100JPY              |\n\
     | >=1000JPY | 0JPY                |\n\n\
     examples\n| amount  | -> fee |\n| -5JPY   | 100JPY |\n| 1000JPY | 0JPY   |\n";

/// An optional number with no range: the door holds it to its type whenever it has a value.
const OPT_NONE: &str = "rule opt_none v1\n\
     description \"Says whether a bonus is big, from a number that may be missing and has no range\"\n\n\
     inputs\n  bonus : number?\n\n\
     outputs\n  big : bool\n\n\
     table pick\npolicy unique\n\
     | bonus | -> big : bool |\n\
     | none  | false         |\n\
     | <5    | false         |\n\
     | >=5   | true          |\n\n\
     examples\n| bonus | -> big |\n| none  | false  |\n| -3    | false  |\n| 7     | true   |\n";

/// A rate whose range writes a lower end only: its upper end is the most steps an int64 holds.
const RATE_HALF: &str = "rule rate_half v1\n\
     description \"A flag from a rate that has a lower end only\"\n\n\
     inputs\n  r : rate[step 0.1%]  range >=0%\n\n\
     outputs\n  high : bool\n\n\
     table pick\npolicy unique\n\
     | r    | -> high : bool |\n\
     | <5%  | false          |\n\
     | >=5% | true           |\n\n\
     examples\n| r  | -> high |\n| 0% | false   |\n| 5% | true    |\n";

/// A field of an element whose range writes a lower end only, summed.
const WALK_HALF: &str = "rule walk_half v1\n\
     description \"A fee from the total of lines whose amounts have a lower end only\"\n\n\
     inputs\n  member : bool\n\n\
     elements lines\n  amount : money[JPY]  range >=0JPY\n\n\
     sum total over lines of amount  range >=0JPY <=1000000JPY\n\n\
     outputs\n  fee : money[JPY]  round up(1JPY)\n\n\
     table fee_table\npolicy unique\n\
     | total     | member | -> fee : money[JPY] |\n\
     | >=5000JPY | -      | 0JPY                |\n\
     | <5000JPY  | true   | 0JPY                |\n\
     | <5000JPY  | false  | 500JPY              |\n\n\
     examples\n| lines | member | -> fee |\n| two   | false  | 0JPY   |\n| one   | false  | 500JPY |\n| one   | true   | 0JPY   |\n\n\
     sequence two\n| amount  |\n| 3000JPY |\n| 2500JPY |\n\n\
     sequence one\n| amount  |\n| 1200JPY |\n";

/// A field of an element with no range, read by a table and counted.
const WALK_NONE: &str = "rule walk_none v1\n\
     description \"A flag from the count of lines whose amounts have no range\"\n\n\
     elements lines\n  amount : money[JPY]\n\n\
     outputs\n  busy : bool\n\n\
     table big_line_by_amount\npolicy unique\n\
     | amount    | -> big_line : bool |\n\
     | >=1000JPY | true               |\n\
     | <1000JPY  | false              |\n\n\
     count big_lines over lines where big_line  range >=0 <=20\n\n\
     table busy_by\npolicy unique\n\
     | big_lines | -> busy : bool |\n\
     | >=2       | true           |\n\
     | <2        | false          |\n\n\
     examples\n| lines | -> busy |\n| two   | true    |\n| one   | false   |\n\n\
     sequence two\n| amount  |\n| 3000JPY |\n| 2500JPY |\n\n\
     sequence one\n| amount   |\n| -1200JPY |\n";

/// Both ends written at the ends of the type: the one above 2^63−1 is no case, as no int64 holds it.
const AT_THE_ENDS: &str = "rule at_the_ends v1\n\
     description \"A fee whose amount is written to the ends of its type\"\n\n\
     inputs\n  amount : money[JPY]  range >=-9223372036854775807JPY <=9223372036854775807JPY\n\n\
     outputs\n  fee : money[JPY]  round down(1JPY)\n\n\
     table fees\npolicy unique\n\
     | amount    | -> fee : money[JPY] |\n\
     | <1000JPY  | 100JPY              |\n\
     | >=1000JPY | 0JPY                |\n\n\
     examples\n| amount  | -> fee |\n| -5JPY   | 100JPY |\n| 1000JPY | 0JPY   |\n";

/// A date whose range writes a lower end only: the door holds it to that day.
const DATE_HALF: &str = "rule date_half v1\n\
     description \"A flag from a date that has a lower end only\"\n\n\
     inputs\n  on : date  range >=2026-01-01\n\n\
     outputs\n  late : bool\n\n\
     table pick\npolicy unique\n\
     | on           | -> late : bool |\n\
     | <2026-06-01  | false          |\n\
     | >=2026-06-01 | true           |\n\n\
     examples\n| on         | -> late |\n| 2026-01-01 | false   |\n| 2026-06-01 | true    |\n";

const RULES: [(&str, &str); 9] = [
    ("half_low", HALF_LOW),
    ("half_high", HALF_HIGH),
    ("no_range", NO_RANGE),
    ("opt_none", OPT_NONE),
    ("rate_half", RATE_HALF),
    ("walk_half", WALK_HALF),
    ("walk_none", WALK_NONE),
    ("at_the_ends", AT_THE_ENDS),
    ("date_half", DATE_HALF),
];

fn write_rules(dir: &Path) {
    for (alias, text) in RULES {
        std::fs::write(dir.join(format!("{alias}.rule")), text).unwrap();
    }
}

fn rulec(dir: &Path, lang: &str, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", lang).current_dir(dir).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn generate(dir: &Path, out: &Path, targets: &[&str], extras: &[&str]) {
    let o = out.to_string_lossy().into_owned();
    let mut args: Vec<String> = vec!["gen".into()];
    args.extend(RULES.iter().map(|(alias, _)| format!("{alias}.rule")));
    args.extend(["--out".into(), o]);
    for t in targets {
        args.extend(["--target".into(), t.to_string()]);
    }
    for x in extras {
        args.extend(["--extra".into(), x.to_string()]);
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (c, said, e) = rulec(dir, "en", &args);
    assert_eq!(c, 0, "{said}{e}");
}

/// The runs that cannot hand the door the value a vector writes. The runners of TypeScript and
/// JavaScript read a line with `JSON.parse`, which makes a number a float64 before it is a bigint,
/// and Swift's reads it with Foundation's `JSONSerialization`, which on Linux gives
/// -9223372036854775807 as a Double (on macOS it stays an Int64): so the lower end of
/// `at_the_ends` reaches the door as −2^63. It is the reading, not the door, and it was so before
/// the ends (§15.295, left as it is).
fn read_as_float64(rule: &str, lang: &str) -> bool {
    rule == "at_the_ends" && (matches!(lang, "typescript" | "javascript") || (lang == "swift" && cfg!(target_os = "linux")))
}

/// `rulec test --format json` over `out`: every result that ran agreed, and the languages that ran.
/// A language whose toolchain is missing is a SKIP line, as `rulec test` says it.
fn agree(out: &Path) -> Vec<String> {
    let tmp = TempDir::new("door-ends-tmp");
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .env("TMPDIR", tmp.path())
        .args(["test", &out.to_string_lossy(), "--format", "json"])
        .output()
        .expect("cannot start rulec");
    let said = String::from_utf8_lossy(&o.stdout).into_owned();
    let j = rulec::json::parse(said.lines().next().unwrap_or_else(|| panic!("no result: {}", String::from_utf8_lossy(&o.stderr)))).unwrap_or_else(|x| panic!("{x}: {said}"));
    for why in j.get("skipped").and_then(|v| v.as_arr()).unwrap_or_default() {
        skip(&format!("rulec test: {}", why.as_str().unwrap_or_default()));
    }
    let mut ran = Vec::new();
    for r in j.get("results").and_then(|v| v.as_arr()).unwrap_or_default() {
        if r.get("ran").and_then(|v| v.as_bool()) != Some(true) {
            continue;
        }
        let of = |k: &str| r.get(k).and_then(|v| v.as_str()).unwrap_or_default().to_string();
        if read_as_float64(&of("rule"), &of("lang")) {
            continue;
        }
        assert_eq!(r.get("ok").and_then(|v| v.as_bool()), Some(true), "{r:?}\n{said}");
        ran.push(format!(
            "{} {}/{}",
            r.get("rule").and_then(|v| v.as_str()).unwrap_or_default(),
            r.get("lang").and_then(|v| v.as_str()).unwrap_or_default(),
            r.get("via").and_then(|v| v.as_str()).unwrap_or_default()
        ));
    }
    ran
}

/// The reference evaluator refuses a value past the end a range writes, and past ±(2^63−1) where
/// it writes none, and answers at those ends. The same in Japanese.
#[test]
fn the_reference_door_holds_a_number_to_its_written_end_and_the_end_of_its_type() {
    let t = TempDir::new("door-ends-run");
    write_rules(t.path());
    let refused = "refused (input): amount is out of range";
    for (file, given, said) in [
        ("half_low.rule", "amount=-1", refused),
        ("half_low.rule", "amount=0", "fee = 100JPY"),
        ("half_low.rule", "amount=9223372036854775807", "fee = 0JPY"),
        ("half_low.rule", "amount=9223372036854775808", refused),
        ("half_high.rule", "amount=100001", refused),
        ("half_high.rule", "amount=100000", "fee = 0JPY"),
        ("half_high.rule", "amount=-9223372036854775807", "fee = 100JPY"),
        ("half_high.rule", "amount=-9223372036854775808", refused),
        ("no_range.rule", "amount=-9223372036854775808", refused),
        ("no_range.rule", "amount=-9223372036854775807", "fee = 100JPY"),
        ("no_range.rule", "amount=9223372036854775807", "fee = 0JPY"),
        ("no_range.rule", "amount=9223372036854775808", refused),
        // A rate is given as its count of steps, as it travels.
        ("rate_half.rule", "r=-1", "refused (input): r is out of range"),
        ("date_half.rule", "on=2025-12-31", "refused (input): on is out of range"),
        ("date_half.rule", "on=2026-01-01", "late = false"),
    ] {
        let (c, out, e) = rulec(t.path(), "en", &["run", file, given]);
        assert_eq!(c, 0, "{file} {given}: {out}{e}");
        assert_eq!(out.trim(), said, "{file} {given}");
    }
    let (_, out, e) = rulec(t.path(), "ja", &["run", "no_range.rule", "amount=-9223372036854775808"]);
    assert!(out.contains("amount が範囲の外です"), "{out}{e}");
}

/// The vectors carry the one past each end the door holds: below a written lower end, above a
/// written upper end, and the least int64 below a number whose range writes no lower end, at an
/// input and at a field of an element. No case goes past 2^63−1, which no int64 holds. A date has
/// the day before the one its range writes, and no case at the end of a type.
#[test]
fn the_vectors_carry_the_one_past_each_end_the_door_holds() {
    let t = TempDir::new("door-ends-vectors");
    write_rules(t.path());
    let out = t.path().join("out");
    generate(t.path(), &out, &["python"], &["vectors"]);
    let refused = |alias: &str| std::fs::read_to_string(out.join("vectors").join(format!("{alias}.refused.jsonl"))).unwrap();
    let least = r#"{"in":{"amount":-9223372036854775808},"refused":"input","error":"amount is out of range","why":"amount one below what its type holds"}"#;

    let r = refused("half_low");
    assert!(r.contains(r#"{"in":{"amount":-1},"refused":"input","error":"amount is out of range","why":"amount one below its range"}"#), "{r}");
    assert!(!r.contains("what its type holds"), "{r}");

    let r = refused("half_high");
    assert!(r.contains(least), "{r}");
    assert!(r.contains(r#"{"in":{"amount":100001},"refused":"input","error":"amount is out of range","why":"amount one above its range"}"#), "{r}");

    let r = refused("no_range");
    assert!(r.contains(least), "{r}");
    assert!(!r.contains(":9223372036854775808"), "{r}");

    let r = refused("opt_none");
    assert!(r.contains(r#"{"in":{"bonus":-9223372036854775808},"refused":"input","error":"bonus is out of range","why":"bonus one below what its type holds"}"#), "{r}");

    let r = refused("rate_half");
    assert!(r.contains(r#"{"in":{"r":-1},"refused":"input","error":"r is out of range","why":"r one below its range"}"#), "{r}");

    let r = refused("walk_half");
    assert!(r.contains(r#""error":"amount is out of range","why":"amount one below its range, in an element of lines""#), "{r}");
    let r = refused("walk_none");
    assert!(r.contains(r#"{"in":{"lines":[{"amount":-9223372036854775808}]},"refused":"input","error":"amount is out of range","why":"amount one below what its type holds, in an element of lines"}"#), "{r}");

    // Written at the ends of the type: the one below is the least int64, and the one above 2^63−1,
    // which the vectors once carried as 2^63, is no case.
    let r = refused("at_the_ends");
    assert!(r.contains(least), "{r}");
    assert!(!r.contains(":9223372036854775808") && !r.contains("one above its range"), "{r}");
    // The number that is not whole stays nearer zero than 2^52, where a float64 still holds the half.
    assert!(r.contains(r#"{"in":{"amount":-4503599627370495.5},"refused":"input","error":"amount is not an integer","why":"amount not a whole number"}"#), "{r}");

    let r = refused("date_half");
    assert!(r.contains(r#"{"in":{"on":"2025-12-31"},"refused":"input","error":"on is out of range","why":"on the day before its range"}"#), "{r}");
    assert!(!r.contains("what its type holds"), "{r}");
}

/// What each language writes at its door. The languages whose integers have no end hold a number to
/// both ends; the languages that hold it in an int64 test the lower end alone, which refuses the
/// least int64 and nothing else where the range writes none, and write no test of 2^63−1, the most
/// an int64 holds. A range that writes one end is tested on that side. NumPy's plan carries both
/// ends, which its runtime tests before the cast to int64.
#[test]
fn each_language_writes_the_ends_its_integers_need() {
    let t = TempDir::new("door-ends-text");
    write_rules(t.path());
    let out = t.path().join("out");
    generate(t.path(), &out, &["python", "numpy", "typescript", "javascript", "rust", "ruby", "php", "go", "swift", "java", "sql"], &[]);
    let read = |p: &str| std::fs::read_to_string(out.join(p)).unwrap_or_else(|e| panic!("{p}: {e}"));
    for (file, has) in [
        ("python/no_range.py", "    if not -9223372036854775807 <= amount <= 9223372036854775807:\n"),
        ("python/half_low.py", "    if not 0 <= amount <= 9223372036854775807:\n"),
        ("typescript/no_range.ts", "  if (amount < -9223372036854775807n || amount > 9223372036854775807n) {\n"),
        ("typescript/half_low.ts", "  if (amount < 0n || amount > 9223372036854775807n) {\n"),
        ("javascript/no_range.mjs", "  if (amount < -9223372036854775807n || amount > 9223372036854775807n) {\n"),
        ("ruby/no_range.rb", "unless (-9223372036854775807..9223372036854775807).cover?(amount)\n"),
        ("ruby/half_low.rb", "unless (0..9223372036854775807).cover?(amount)\n"),
        ("numpy/no_range.json", r#""min":-9223372036854775807,"max":9223372036854775807"#),
        ("numpy/half_low.json", r#""min":0,"max":9223372036854775807"#),
        ("rust/no_range.rs", "    if amount.0 < -9223372036854775807 {\n"),
        ("rust/half_low.rs", "    if amount.0 < 0 {\n"),
        ("go/norange/no_range.go", "\tif int64(in.Amount) < -9223372036854775807 {\n"),
        ("go/halflow/half_low.go", "\tif int64(in.Amount) < 0 {\n"),
        ("go/halflow/half_low.go", "Amount JPY // amount range 0..\n"),
        ("go/halfhigh/half_high.go", "Amount JPY // amount range ..100000\n"),
        ("java/NoRange.java", "        if (amount < -9223372036854775807L) {\n"),
        ("java/HalfLow.java", "        if (amount < 0L) {\n"),
        ("php/no_range.php", "    if ($amount < -9223372036854775807) {\n"),
        ("php/half_low.php", "    if ($amount < 0) {\n"),
        ("swift/no_range.swift", "    if amount.value < -9223372036854775807 {\n"),
        ("swift/half_low.swift", "    if amount.value < 0 {\n"),
        ("sql/no_range.sql", r#"WHEN "amount" < -9223372036854775807 THEN 'amount is out of range'"#),
        ("sql/half_low.sql", r#"WHEN "amount" < 0 THEN 'amount is out of range'"#),
        ("sql/half_high.sql", r#"WHEN "amount" < -9223372036854775807 OR "amount" > 100000 THEN 'amount is out of range'"#),
        // A date's day number, on the side its range writes.
        ("python/date_half.py", "    if on < 20454:\n"),
        ("typescript/date_half.ts", "  if (on < 20454n) {\n"),
        ("rust/date_half.rs", "    if on < 20454 {\n"),
        ("go/datehalf/date_half.go", "\tif int64(in.On) < 20454 {\n"),
        ("go/datehalf/date_half.go", "On int64 // on range 20454..\n"),
        ("java/DateHalf.java", "        if (on < 20454L) {\n"),
        ("numpy/date_half.json", r#""range":"on is out of range"},"min":20454}]"#),
    ] {
        let text = read(file);
        let squeezed = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let want = has.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(squeezed.contains(&want), "{file}: missing `{}`:\n{text}", has.trim());
    }
    // An int64 holds nothing above 2^63−1, so no language that holds one tests it.
    for file in ["rust/no_range.rs", "go/norange/no_range.go", "java/NoRange.java", "php/no_range.php", "swift/no_range.swift", "sql/no_range.sql"] {
        let text = read(file);
        assert!(!text.contains("> 9223372036854775807"), "{file}:\n{text}");
    }
}

/// Every language refuses what the reference evaluator refuses past each end — the vectors' one
/// below a written lower end, one above a written upper end, and the least int64 where no lower end
/// is written — with its sentence, held by `rulec test`, PostgreSQL's function among them when a
/// server answers. SQL and NumPy do not walk a sequence and are not generated for the two rules
/// with elements.
#[test]
fn every_language_refuses_past_each_end_as_the_reference_door_does() {
    if !ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not here") {
        return;
    }
    let t = TempDir::new("door-ends-langs");
    write_rules(t.path());
    let out = t.path().join("out");
    generate(
        t.path(),
        &out,
        &["python", "numpy", "typescript", "javascript", "rust", "ruby", "php", "go", "swift", "java", "sql", "wasm"],
        &["runner", "vectors"],
    );
    let ran = agree(&out);
    for alias in ["half_low", "no_range", "walk_none", "date_half"] {
        assert!(ran.iter().any(|x| x.starts_with(&format!("{alias} python/"))), "{alias}: Python did not run: {ran:?}");
    }
    println!("door ends: {} runs agreed: {}", ran.len(), ran.join(", "));
}

/// A number past 2^63−1, or below −(2^63−1) by more than one, reaches the door only where integers
/// have no end — Python, NumPy, TypeScript, JavaScript and Ruby — and each refuses it as out of
/// range, as the reference evaluator does. NumPy tests it before the cast to int64, which would
/// have wrapped 2^63 to the least int64 and answered for it.
#[test]
fn the_languages_whose_integers_have_no_end_refuse_a_number_past_its_type() {
    if !ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not here") {
        return;
    }
    let t = TempDir::new("door-ends-wide");
    write_rules(t.path());
    let out = t.path().join("out");
    generate(t.path(), &out, &["python", "numpy", "typescript", "javascript", "ruby"], &["runner", "vectors"]);
    let past = [
        r#"{"in":{"amount":9223372036854775808},"refused":"input","error":"amount is out of range","why":"2^63"}"#,
        r#"{"in":{"amount":18446744073709551616},"refused":"input","error":"amount is out of range","why":"2^64"}"#,
        r#"{"in":{"amount":-9223372036854775809},"refused":"input","error":"amount is out of range","why":"one below the least int64"}"#,
    ];
    for alias in ["no_range", "half_low"] {
        std::fs::write(out.join("vectors").join(format!("{alias}.refused.jsonl")), past.join("\n") + "\n").unwrap();
        let (c, said, e) = rulec(t.path(), "en", &["run", &format!("{alias}.rule"), "amount=9223372036854775808"]);
        assert_eq!((c, said.trim()), (0, "refused (input): amount is out of range"), "{e}");
    }
    let ran = agree(&out);
    assert!(ran.iter().any(|x| x.starts_with("no_range python/")), "Python did not run: {ran:?}");
    println!("past the type: {} runs agreed: {}", ran.len(), ran.join(", "));
}
