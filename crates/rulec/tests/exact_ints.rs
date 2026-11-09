//! The integers a runner and a server read (§15.297): every integer a line writes reaches the door
//! as it was written. TypeScript and JavaScript read a line with `JSON.parse`, whose float64 holds
//! every integer only up to 2^53, so their runners, their MCP servers and Wasm's runner read an
//! integer past it from the digits written and write a bigint back as its digits. Swift's runner
//! reads the `NSDecimalNumber` that Foundation on Linux gives for a negative integer of 19 digits
//! from its digits, and holds a UInt64 past Int64 to be no integer, as the other languages that
//! hold an int64 do, where it kept the low 64 bits.

use ritsu_testkit::{Need, TempDir, ready, skip};
use std::path::Path;
use std::process::Command;

/// A boundary one past 2^53, which a float64 reads as 2^53, and an upper end at 10^16, whose one
/// above a float64 reads as the end itself.
const BIG_EDGE: &str = "rule big_edge v1\n\
     description \"A fee with a boundary one past 2^53\"\n\n\
     inputs\n  amount : money[JPY]  range >=0JPY <=10000000000000000JPY\n\n\
     outputs\n  fee : money[JPY]  round down(1JPY)\n\n\
     table fees\npolicy unique\n\
     | amount                | -> fee : money[JPY] |\n\
     | <9007199254740993JPY  | 100JPY              |\n\
     | >=9007199254740993JPY | 0JPY                |\n\n\
     examples\n| amount              | -> fee |\n| 5JPY                | 100JPY |\n| 9007199254740993JPY | 0JPY   |\n";

/// A boundary at a negative number of 19 digits, and a lower end one above the least int64: both
/// are integers Foundation on Linux gives as an `NSDecimalNumber`.
const NEG_EDGE: &str = "rule neg_edge v1\n\
     description \"A fee with a boundary at a negative number of 19 digits\"\n\n\
     inputs\n  amount : money[JPY]  range >=-9223372036854775807JPY <=0JPY\n\n\
     outputs\n  fee : money[JPY]  round down(1JPY)\n\n\
     table fees\npolicy unique\n\
     | amount                    | -> fee : money[JPY] |\n\
     | <-1000000000000000001JPY  | 100JPY              |\n\
     | >=-1000000000000000001JPY | 0JPY                |\n\n\
     examples\n| amount                  | -> fee |\n| -5JPY                   | 0JPY   |\n| -2000000000000000000JPY | 100JPY |\n";

/// The standing of a seller (tests/machine.rs) with a grace period of 2^53+3 days, and 2^53+1 days
/// carried from the start: a trace hands a number past 2^53 from one call to the next, and the
/// line that asks for the machine's constants answers with one.
const BIG_STANDING: &str = "rule big_standing v1

enum standing = fresh | dispute | grace | probation | good | trusted
enum event = first_sale | warn | review | promote | revoke | night

inputs
  standing  : standing
  event     : event
  days_left : number  range >=0 <=9007199254740995

outputs
  next_standing  : standing
  next_days_left : number  round down(1)

derive one_less : number = days_left - 1  range >=-1 <=9007199254740994

table step
policy unique
| standing                        | event      | days_left | -> next_standing | next_days_left   |
| fresh                           | first_sale | -         | dispute          | days_left        |
| not: fresh                      | first_sale | -         | standing         | days_left        |
| dispute                         | warn       | -         | grace            | 9007199254740995 |
| not: dispute                    | warn       | -         | standing         | days_left        |
| dispute, grace                  | review     | -         | probation        | 9007199254740995 |
| fresh, probation, good, trusted | review     | -         | standing         | days_left        |
| not: trusted                    | promote    | -         | trusted          | days_left        |
| trusted                         | promote    | -         | standing         | days_left        |
| fresh, dispute                  | revoke     | -         | standing         | days_left        |
| grace, probation, good          | revoke     | -         | dispute          | 9007199254740995 |
| trusted                         | revoke     | -         | probation        | 9007199254740995 |
| fresh, dispute, good, trusted   | night      | -         | standing         | days_left        |
| grace                           | night      | 1         | dispute          | 0                |
| grace                           | night      | >=2       | grace            | one_less         |
| grace                           | night      | 0         | grace            | one_less         |
| probation                       | night      | <=1       | good             | 0                |
| probation                       | night      | >=2       | probation        | one_less         |

machine seller over step
  carry   standing -> next_standing
  carry   days_left -> next_days_left
  initial fresh
  initial days_left 9007199254740993
  never   fresh after dispute, grace, probation, good, trusted
  always  days_left >=1 <=9007199254740995 when grace, probation
";

/// No range: a number past 64 bits is the one the readers of the int64 languages meet.
const NO_RANGE: &str = "rule no_range v1\n\
     description \"A fee from an amount with no range\"\n\n\
     inputs\n  amount : money[JPY]\n\n\
     outputs\n  fee : money[JPY]  round down(1JPY)\n\n\
     table fees\npolicy unique\n\
     | amount    | -> fee : money[JPY] |\n\
     | <1000JPY  | 100JPY              |\n\
     | >=1000JPY | 0JPY                |\n\n\
     examples\n| amount  | -> fee |\n| 5JPY    | 100JPY |\n| 1000JPY | 0JPY   |\n";

fn rulec(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").current_dir(dir).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The rules written to `dir`, and `rulec gen` of them into `out` for the targets and extras named.
fn generate(dir: &Path, out: &Path, rules: &[(&str, &str)], targets: &[&str], extras: &[&str]) {
    let mut args: Vec<String> = vec!["gen".into()];
    for (alias, text) in rules {
        std::fs::write(dir.join(format!("{alias}.rule")), text).unwrap();
        args.push(format!("{alias}.rule"));
    }
    args.extend(["--out".into(), out.to_string_lossy().into_owned()]);
    for t in targets {
        args.extend(["--target".into(), t.to_string()]);
    }
    for x in extras {
        args.extend(["--extra".into(), x.to_string()]);
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (c, said, e) = rulec(dir, &args);
    assert_eq!(c, 0, "{said}{e}");
}

/// `rulec test --format json` over `out`: every result that ran agreed, and the runs, as
/// `rule lang/via`. A language whose toolchain is missing is a SKIP line, as `rulec test` says it.
fn agree(out: &Path) -> Vec<String> {
    let tmp = TempDir::new("exact-ints-tmp");
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
        assert_eq!(r.get("ok").and_then(|v| v.as_bool()), Some(true), "{r:?}\n{said}");
        let of = |k: &str| r.get(k).and_then(|v| v.as_str()).unwrap_or_default().to_string();
        ran.push(format!("{} {}/{}", of("rule"), of("lang"), of("via")));
    }
    ran
}

/// Every runner and server answers as the reference evaluator does at a boundary one past 2^53, at a
/// negative boundary of 19 digits, and along the traces of a machine that carries a number past
/// 2^53, held by `rulec test`: TypeScript and JavaScript read 2^53+1 as 2^53 and answered with the
/// row below it, their MCP servers answered for the one above an upper end of 10^16, Wasm's runner
/// handed the next call 2^53+4 for 2^53+3, and Swift's on Linux read −(10^18+1) as −10^18.
#[test]
fn every_runner_and_server_hands_the_door_the_integer_a_line_writes() {
    if !ready(Need::Node, || Command::new("node").arg("--version").output().is_ok_and(|o| o.status.success()), "node is not here") {
        return;
    }
    let t = TempDir::new("exact-ints-langs");
    let out = t.path().join("out");
    generate(
        t.path(),
        &out,
        &[("big_edge", BIG_EDGE), ("neg_edge", NEG_EDGE), ("big_standing", BIG_STANDING)],
        &["python", "numpy", "typescript", "javascript", "rust", "ruby", "php", "go", "swift", "java", "sql", "wasm"],
        &["runner", "mcp", "vectors"],
    );
    let vectors = std::fs::read_to_string(out.join("vectors/big_edge.jsonl")).unwrap();
    assert!(vectors.contains("\"amount\":9007199254740993"), "{vectors}");
    let refused = std::fs::read_to_string(out.join("vectors/big_edge.refused.jsonl")).unwrap();
    assert!(refused.contains("\"amount\":10000000000000001"), "{refused}");
    let traces = std::fs::read_to_string(out.join("vectors/big_standing.traces.expected.jsonl")).unwrap();
    assert!(traces.contains("\"next_days_left\":9007199254740995"), "{traces}");
    let ran = agree(&out);
    for run in [
        "big_edge typescript/runner",
        "big_edge javascript/runner",
        "big_edge typescript/mcp",
        "big_edge javascript/mcp-http",
        "neg_edge typescript/runner",
        "big_standing typescript/runner",
        "big_standing javascript/runner",
    ] {
        assert!(ran.iter().any(|x| x == run), "{run} did not run: {ran:?}");
    }
    println!("exact integers: {} runs agreed: {}", ran.len(), ran.join(", "));
}

/// A number past 64 bits is no integer to the languages that hold an int64 — Rust, Go, Swift, Java,
/// PHP and Wasm — and each refuses it with that sentence. Swift's runner kept the low 64 bits of a
/// UInt64 and answered for 2^64−1 as −1.
#[test]
fn the_int64_languages_refuse_a_number_past_64_bits_as_no_integer() {
    if !ready(Need::Rustc, || Command::new("rustc").arg("--version").output().is_ok_and(|o| o.status.success()), "rustc is not here") {
        return;
    }
    let t = TempDir::new("exact-ints-wide");
    let out = t.path().join("out");
    generate(t.path(), &out, &[("no_range", NO_RANGE)], &["rust", "go", "swift", "java", "php", "wasm"], &["runner", "vectors"]);
    let past = [
        r#"{"in":{"amount":9223372036854775808},"refused":"input","error":"amount is not an integer","why":"2^63"}"#,
        r#"{"in":{"amount":18446744073709551615},"refused":"input","error":"amount is not an integer","why":"2^64-1, the most a UInt64 holds"}"#,
        r#"{"in":{"amount":18446744073709551616},"refused":"input","error":"amount is not an integer","why":"2^64"}"#,
    ];
    std::fs::write(out.join("vectors/no_range.refused.jsonl"), past.join("\n") + "\n").unwrap();
    let ran = agree(&out);
    assert!(ran.iter().any(|x| x.starts_with("no_range ")), "nothing ran: {ran:?}");
    println!("past 64 bits: {} runs agreed: {}", ran.len(), ran.join(", "));
}
