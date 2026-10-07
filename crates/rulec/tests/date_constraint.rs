//! A `constraint` between two dates (§15.198): a hotel stay's `check_in <= check_out`.
//!
//! Wherever rulec puts an input together itself — the vector suite, the input E101 hands over,
//! the answers of the port's `outputs_over`, the examples it holds to the constraints (E019), the
//! calls of a state machine, the cells `diff` compares — it keeps only what the constraints let
//! through, and its port refuses at `eval` what the generated code refuses at its door. Two dates
//! compare there by their day numbers, the way that door compares them. Those places used to read
//! only numbers, so every pair of dates got through: the vectors of a stay handed the generated
//! code check-outs before the check-in, and `rulec test` said every language disagreed.
//!
//! The materials are in `tests/date_constraint/`: `hotel_stay.rule` and `hotel_booking.rule`, with
//! their Japanese versions `宿泊料金.rule` and `宿泊の予約.rule` beside them. The variants below are
//! written English first, each with its Japanese version.

use ritsu_ports::{Answer, Found, Precondition, RuleError, Rules, Value, Values};
use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, ready, skip};
use rulec::json::Json;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A material, with the names it gives the two dates and the enum output.
struct Stay {
    path: &'static str,
    check_in: &'static str,
    check_out: &'static str,
    band: &'static str,
}

const STAYS: [Stay; 2] = [
    Stay { path: "tests/date_constraint/hotel_stay.rule", check_in: "check_in", check_out: "check_out", band: "band" },
    Stay { path: "tests/date_constraint/宿泊料金.rule", check_in: "チェックイン日", check_out: "チェックアウト日", band: "帯" },
];

/// The state machines, the same way.
const BOOKINGS: [Stay; 2] = [
    Stay { path: "tests/date_constraint/hotel_booking.rule", check_in: "check_in", check_out: "check_out", band: "" },
    Stay { path: "tests/date_constraint/宿泊の予約.rule", check_in: "チェックイン日", check_out: "チェックアウト日", band: "" },
];

/// `rulec`, answering in English: the notes this file reads are the English ones.
fn rulec(args: &[&str]) -> (i32, String, String) {
    rulec_in(None, args)
}

/// `rulec`, with `tmp` as its TMPDIR when there is one: `rulec test` asks swiftc for its version,
/// and swiftc leaves an empty directory in its TMPDIR almost every time.
fn rulec_in(tmp: Option<&Path>, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rulec"));
    c.env("RULEC_LANG", "en").current_dir(root()).args(args);
    if let Some(t) = tmp {
        c.env("TMPDIR", t);
    }
    let o = c.output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn json(line: &str) -> Json {
    rulec::json::parse(line).unwrap_or_else(|e| panic!("{e}: {line}"))
}

/// The two dates of a vector's input, as written (`YYYY-MM-DD`, which sorts as the dates do).
fn dates_in(j: &Json, s: &Stay) -> (String, String) {
    let get = |k: &str| j.get("in").and_then(|i| i.get(k)).and_then(|v| v.as_str()).unwrap_or_else(|| panic!("no {k} in {j:?}")).to_string();
    (get(s.check_in), get(s.check_out))
}

/// A date the port gives, from the values it hands over.
fn date_of(vs: &Values, name: &str) -> String {
    match vs.iter().find(|(n, _)| n == name) {
        Some((_, Value::Date(d))) => d.clone(),
        other => panic!("{name} is not a date here: {other:?} in {vs:?}"),
    }
}

/// Every vector keeps the constraint, and the suite still meets every criterion of coverage with
/// none of the inputs the constraint rules out.
#[test]
fn every_vector_keeps_the_constraint() {
    for s in STAYS.iter().chain(BOOKINGS.iter()) {
        let (code, out, e) = rulec(&["vectors", s.path]);
        assert_eq!(code, 0, "{}: {e}", s.path);
        let mut n = 0;
        for line in out.lines().filter(|l| !l.trim().is_empty()) {
            let (a, b) = dates_in(&json(line), s);
            assert!(a <= b, "{}: a vector breaks `{} <= {}`: {line}", s.path, s.check_in, s.check_out);
            n += 1;
        }
        assert!(n > 0, "{}: no vectors", s.path);
        let (code, out, e) = rulec(&["coverage", s.path]);
        assert_eq!(code, 0, "{}: {out}{e}", s.path);
        let criteria: Vec<&str> = out.lines().filter(|l| l.contains(" / ")).collect();
        assert!(criteria.len() >= 5 && criteria.iter().all(|l| l.trim_end().ends_with("satisfied")), "{}: {out}", s.path);
    }
}

/// A day use read off the two dates: a comparison of two names, which the vectors step on at the
/// tie and on both sides of it (§9.1). The constraint leaves only the tie and one side.
const DAY_USE: &str = r#"rule day_use v1
description "A stay that leaves on the day it arrives is a day use"

inputs
  check_in  : date  range >=2026-01-01 <=2026-12-31
  check_out : date  range >=2026-01-01 <=2026-12-31

constraint check_in <= check_out

outputs
  rate : money[USD]  round half_up(1USD)

define day_use : bool = check_out <= check_in

table by_stay
policy unique
| day_use | check_in     | -> rate : money[USD] |
| true    | -            | 60USD                |
| false   | <=2026-06-30 | 120USD               |
| false   | >=2026-07-01 | 180USD               |
"#;

/// `DAY_USE` in Japanese.
const DAY_USE_JA: &str = r#"rule デイユース(day_use_ja) v1
description "着いた日に発つ宿泊はデイユース。day_use の日本語の版"

inputs
  チェックイン日(check_in)    : date  range >=2026-01-01 <=2026-12-31
  チェックアウト日(check_out) : date  range >=2026-01-01 <=2026-12-31

constraint チェックイン日 <= チェックアウト日

outputs
  料金(rate) : money[USD]  round half_up(1USD)

define デイユース(day_use) : bool = チェックアウト日 <= チェックイン日

table 宿泊別(by_stay)
policy unique
| デイユース | チェックイン日 | -> 料金 : money[USD] |
| true       | -              | 60USD                |
| false      | <=2026-06-30   | 120USD               |
| false      | >=2026-07-01   | 180USD               |
"#;

/// The tie of two dates is stepped on from the side the constraint leaves: the vectors keep it,
/// and a day use, which only the two dates being the same makes, is among them.
#[test]
fn the_tie_of_two_dates_keeps_the_constraint() {
    let tmp = TempDir::new("date-constraint-tie");
    for (name, src, s) in [
        ("day_use.rule", DAY_USE, Stay { path: "", check_in: "check_in", check_out: "check_out", band: "" }),
        ("デイユース.rule", DAY_USE_JA, Stay { path: "", check_in: "チェックイン日", check_out: "チェックアウト日", band: "" }),
    ] {
        let p = tmp.path().join(name);
        std::fs::write(&p, src).unwrap();
        let p = p.to_string_lossy().into_owned();
        let (code, out, e) = rulec(&["vectors", &p]);
        assert_eq!(code, 0, "{name}: {e}");
        let mut day_use = 0;
        for line in out.lines().filter(|l| !l.trim().is_empty()) {
            let j = json(line);
            let (a, b) = dates_in(&j, &s);
            assert!(a <= b, "{name}: a vector breaks the constraint: {line}");
            if a == b {
                day_use += 1;
            }
        }
        assert!(day_use > 0, "{name}: no vector stands on the tie\n{out}");
    }
}

/// The examples of a rule are cases it is claimed to answer, so one that breaks the constraint
/// between the two dates is E019, as one that breaks a constraint between two numbers is. The
/// example here gives the answer the evaluator would give, so nothing else stops it.
#[test]
fn an_example_that_breaks_the_constraint_is_e019() {
    let tmp = TempDir::new("date-constraint-e019");
    for s in &STAYS {
        let src = read(s.path);
        let mut lines: Vec<String> = src.lines().map(String::from).collect();
        let at = lines.iter().rposition(|l| l.starts_with("| 2026-12-30")).expect("the last example");
        lines[at] = lines[at].replacen("2026-12-30", "@", 1).replacen("2026-12-31", "2026-12-30", 1).replacen('@', "2026-12-31", 1);
        let p = tmp.path().join(Path::new(s.path).file_name().unwrap());
        std::fs::write(&p, lines.join("\n") + "\n").unwrap();
        let (code, out, _) = rulec(&["check", &p.to_string_lossy(), "--format", "json"]);
        assert_eq!(code, 1, "{}: {out}", s.path);
        let codes: Vec<String> = out.lines().filter(|l| l.starts_with('{')).filter_map(|l| json(l).get("code").and_then(|c| c.as_str()).map(String::from)).collect();
        assert_eq!(codes, vec!["E019".to_string()], "{}: {out}", s.path);
        assert!(out.contains(&format!("`{} <= {}` does not hold for this input", s.check_in, s.check_out)), "{}: {out}", s.path);
    }
}

/// E101 on a column the rule computes hands over the input behind the gap (§15.194), and that input
/// keeps the constraint: day use is offered only until the end of June here, so the gap is a day use
/// from July, and the input that makes it has the two dates the same.
#[test]
fn the_input_behind_an_e101_keeps_the_constraint() {
    let tmp = TempDir::new("date-constraint-e101");
    for (name, src, check_in, check_out) in [
        ("day_use.rule", DAY_USE.replace("| true    | -            | 60USD                |", "| true    | <=2026-06-30 | 60USD                |"), "check_in", "check_out"),
        (
            "デイユース.rule",
            DAY_USE_JA.replace("| true       | -              | 60USD                |", "| true       | <=2026-06-30   | 60USD                |"),
            "チェックイン日",
            "チェックアウト日",
        ),
    ] {
        assert!(src.contains("<=2026-06-30 | 60USD") || src.contains("<=2026-06-30   | 60USD"), "{name}: the row did not move");
        let p = tmp.path().join(name);
        std::fs::write(&p, &src).unwrap();
        let (code, out, _) = rulec(&["check", &p.to_string_lossy(), "--format", "json"]);
        assert_eq!(code, 1, "{name}: {out}");
        let e101 = out.lines().filter(|l| l.starts_with('{')).map(json).find(|j| j.get("code").and_then(|c| c.as_str()) == Some("E101")).unwrap_or_else(|| panic!("{name}: no E101\n{out}"));
        let notes: Vec<String> = e101.get("notes").and_then(|n| n.as_arr()).unwrap_or_default().iter().filter_map(|n| n.as_str().map(String::from)).collect();
        let behind = notes.iter().find_map(|n| n.strip_prefix("An input producing this example: ")).unwrap_or_else(|| panic!("{name}: no input behind the gap\n{out}"));
        let value = |k: &str| -> String {
            behind.split(", ").find_map(|kv| kv.strip_prefix(&format!("{k} = "))).unwrap_or_else(|| panic!("{name}: no {k} in `{behind}`")).to_string()
        };
        assert!(value(check_in) <= value(check_out), "{name}: the input behind the gap breaks the constraint: {behind}");
    }
}

/// The inputs a question to `outputs_over` holds to ranges: each by name, with its two ends.
type Held = Vec<(String, Option<i128>, Option<i128>)>;

/// The port's `outputs_over` answers each value with an input that comes to it, from the vectors
/// over the ranges; every one keeps the constraint and lies inside the ranges. Held to the turn of
/// June, the two dates' ranges cross: the low end of the check-out's is before the check-in's, so
/// the first input of the vectors over them breaks the constraint.
#[test]
fn outputs_over_answers_with_inputs_that_keep_the_constraint() {
    let e = rulec::ports::Engine::new();
    // day numbers: 2026-06-20, 06-25, 07-01, 07-02, 07-05 and 08-31
    let (jun20, jun25, jul1, jul2, jul5, aug31) = (20624, 20629, 20635, 20636, 20639, 20696);
    for s in &STAYS {
        let cases: [(Held, usize); 3] = [
            (vec![], 3),
            (vec![(s.check_in.to_string(), Some(jul1), Some(aug31))], 2),
            (vec![(s.check_in.to_string(), Some(jun25), Some(jul5)), (s.check_out.to_string(), Some(jun20), Some(jul2))], 3),
        ];
        for (held, count) in cases {
            let found = e.outputs_over(Path::new(s.path), s.band, &held).unwrap_or_else(|x| panic!("{}: {x:?}", s.path));
            let Found::Value(values) = found else { panic!("{}: {found:?}", s.path) };
            assert_eq!(values.len(), count, "{}: {held:?}: {values:?}", s.path);
            let day = |d: &str| -> i128 {
                let (y, m, dd) = (d[0..4].parse().unwrap(), d[5..7].parse().unwrap(), d[8..10].parse().unwrap());
                let r = rulec::types::date_ord(y, m, dd);
                r.num / r.den
            };
            for (v, inputs) in &values {
                let (a, b) = (date_of(inputs, s.check_in), date_of(inputs, s.check_out));
                assert!(a <= b, "{}: {v:?} is answered with an input that breaks the constraint: {inputs:?}", s.path);
                for (name, lo, hi) in &held {
                    let x = day(&date_of(inputs, name));
                    assert!(lo.is_none_or(|l| x >= l) && hi.is_none_or(|h| x <= h), "{}: {name} = {x} is outside {lo:?}..{hi:?}", s.path);
                }
            }
        }
    }
}

/// rulec's port refuses at `eval` what the generated code refuses at its door: a check-out before
/// the check-in, and a date outside its range. A date of koyomi's days that is not one of them is
/// refused too (§15.174).
#[test]
fn the_port_refuses_what_the_door_refuses() {
    let e = rulec::ports::Engine::new();
    for s in &STAYS {
        let at = |a: &str, b: &str| -> Values { vec![(s.check_in.to_string(), Value::Date(a.into())), (s.check_out.to_string(), Value::Date(b.into()))] };
        let p = Path::new(s.path);
        let ok = e.eval(p, &at("2026-08-01", "2026-08-03")).unwrap_or_else(|x| panic!("{}: {x:?}", s.path));
        assert!(ok.iter().any(|(_, v)| *v == Value::Int(180)), "{}: {ok:?}", s.path);
        match e.eval(p, &at("2026-08-03", "2026-08-01")) {
            Err(RuleError::Input(t)) => assert_eq!(t.en, format!("`{} <= {}` does not hold", s.check_in, s.check_out), "{}", s.path),
            other => panic!("{}: a check-out before the check-in is answered: {other:?}", s.path),
        }
        match e.eval(p, &at("2025-12-31", "2026-01-02")) {
            Err(RuleError::Input(t)) => assert_eq!(t.en, format!("the input `{}` is outside its range", s.check_in), "{}", s.path),
            other => panic!("{}: a date before its range is answered: {other:?}", s.path),
        }
        match e.eval(p, &at("2026-12-30", "2027-01-01")) {
            Err(RuleError::Input(t)) => assert_eq!(t.en, format!("the input `{}` is outside its range", s.check_out), "{}", s.path),
            other => panic!("{}: a date after its range is answered: {other:?}", s.path),
        }
    }
    let e = rulec::ports::Engine::with_dates(Arc::new(koyomi::ports::Engine));
    let p = Path::new("tests/days/settlement.rule");
    let on = |d: &str| -> Values { vec![("pay_day".to_string(), Value::Date(d.into()))] };
    assert!(e.eval(p, &on("2026-02-10")).is_ok(), "a payment day is refused");
    match e.eval(p, &on("2026-02-11")) {
        Err(RuleError::Input(t)) => assert_eq!(t.en, "the input `pay_day` is not a day payment of payment_terms.cal comes to"),
        other => panic!("a day that is not a payment day is answered: {other:?}"),
    }
    match e.eval(p, &on("2026-01-05")) {
        Err(RuleError::Input(t)) => assert_eq!(t.en, "the input `pay_day` is outside its range"),
        other => panic!("a day before the first payment day is answered: {other:?}"),
    }
}

/// A precondition between two dates that the ranges a caller gives can break comes with the corner
/// that breaks it, written as dates, as the port writes every other date.
#[test]
fn a_broken_relation_between_two_dates_is_shown_at_dates() {
    let e = rulec::ports::Engine::new();
    for s in &STAYS {
        let p = Path::new(s.path);
        let relation = |ranges: &[(String, Option<i128>, Option<i128>)]| -> Answer<Values> {
            let all = e.preconditions_hold(p, ranges, None).unwrap_or_else(|x| panic!("{}: {x:?}", s.path));
            all.into_iter().find(|(q, _)| matches!(q, Precondition::Relation { .. })).map(|(_, a)| a).expect("the constraint is a precondition")
        };
        // check-in in the summer, check-out from 2026-06-01 to 2026-07-16
        let broken = relation(&[(s.check_in.into(), Some(20635), Some(20696)), (s.check_out.into(), Some(20605), Some(20650))]);
        assert_eq!(
            broken,
            Answer::Fails(vec![(s.check_in.to_string(), Value::Date("2026-08-31".into())), (s.check_out.to_string(), Value::Date("2026-06-01".into()))]),
            "{}",
            s.path
        );
        // check-in until the end of June, check-out from July
        assert_eq!(relation(&[(s.check_in.into(), None, Some(20634)), (s.check_out.into(), Some(20635), None)]), Answer::Holds, "{}", s.path);
    }
}

/// Two versions of a stay, the second charging more for one row; the first row takes any check-out,
/// so the cells under the second cross the constraint's line.
const STAY_PAIR: &str = r#"rule stay_pair v1
description "A stay charged by the dates it begins and ends on"

inputs
  check_in  : date  range >=2026-01-01 <=2026-12-31
  check_out : date  range >=2026-01-01 <=2026-12-31

constraint check_in <= check_out

outputs
  rate : money[USD]  round half_up(1USD)

table rate_of
policy unique
| check_in     | check_out    | -> rate : money[USD] |
| <=2026-07-14 | -            | 100USD               |
| >=2026-07-15 | <=2026-07-20 | 200USD               |
| >=2026-07-15 | >=2026-07-21 | 300USD               |
"#;

/// `STAY_PAIR` in Japanese.
const STAY_PAIR_JA: &str = r#"rule 宿泊の組(stay_pair_ja) v1
description "宿泊の始まる日と終わる日で決まる料金。stay_pair の日本語の版"

inputs
  チェックイン日(check_in)    : date  range >=2026-01-01 <=2026-12-31
  チェックアウト日(check_out) : date  range >=2026-01-01 <=2026-12-31

constraint チェックイン日 <= チェックアウト日

outputs
  料金(rate) : money[USD]  round half_up(1USD)

table 料金表(rate_of)
policy unique
| チェックイン日 | チェックアウト日 | -> 料金 : money[USD] |
| <=2026-07-14   | -                | 100USD               |
| >=2026-07-15   | <=2026-07-20     | 200USD               |
| >=2026-07-15   | >=2026-07-21     | 300USD               |
"#;

/// Two derives that share inputs and a coupon issued and used on two dates: a cell under the
/// derives is solved by elimination (§15.129), and the dates it places come back as dates.
const COUPON_WINDOW: &str = r#"rule coupon_window v1
description "A coupon issued and used on two dates, and two derives that share inputs"

enum stack_verdict = no default | yes

inputs
  total  : money[JPY,incl_tax]  range >=0JPY <=1_000_000JPY
  disc_a : money[JPY,incl_tax]  range >=0JPY <=100_000JPY
  disc_b : money[JPY,incl_tax]  range >=0JPY <=100_000JPY
  issued : date  range >=2026-01-01 <=2026-12-31
  used   : date  range >=2026-01-01 <=2026-12-31

constraint issued <= used

outputs
  verdict : stack_verdict

derive rest_a : money[JPY,incl_tax] = total - disc_a           range >=-100_000JPY <=1_000_000JPY
derive rest_b : money[JPY,incl_tax] = total - disc_a - disc_b  range >=-200_000JPY <=1_000_000JPY

table decide
policy unique
| rest_a    | rest_b    | used         | -> verdict : stack_verdict |
| <=1000JPY | -         | -            | no                         |
| -         | >=3980JPY | <=2026-06-30 | yes                        |
| -         | >=3980JPY | >=2026-07-01 | no                         |
| >1000JPY  | <3980JPY  | -            | no                         |
"#;

/// `COUPON_WINDOW` in Japanese.
const COUPON_WINDOW_JA: &str = r#"rule クーポンの期間(coupon_window_ja) v1
description "二つの日付に発行して使うクーポンと、入力を共有する二つの導出。coupon_window の日本語の版"

enum 判定(stack_verdict) = 対象外(no) default | 対象(yes)

inputs
  合計(total)   : money[JPY,incl_tax]  range >=0JPY <=1_000_000JPY
  割引A(disc_a) : money[JPY,incl_tax]  range >=0JPY <=100_000JPY
  割引B(disc_b) : money[JPY,incl_tax]  range >=0JPY <=100_000JPY
  発行日(issued) : date  range >=2026-01-01 <=2026-12-31
  使用日(used)   : date  range >=2026-01-01 <=2026-12-31

constraint 発行日 <= 使用日

outputs
  判定結果(verdict) : 判定

derive 残高A(rest_a) : money[JPY,incl_tax] = 合計 - 割引A          range >=-100_000JPY <=1_000_000JPY
derive 残高B(rest_b) : money[JPY,incl_tax] = 合計 - 割引A - 割引B  range >=-200_000JPY <=1_000_000JPY

table 判定表(decide)
policy unique
| 残高A     | 残高B     | 使用日       | -> 判定結果 : 判定 |
| <=1000JPY | -         | -            | 対象外             |
| -         | >=3980JPY | <=2026-06-30 | 対象               |
| -         | >=3980JPY | >=2026-07-01 | 対象外             |
| >1000JPY  | <3980JPY  | -            | 対象外             |
"#;

/// `rulec diff` compares the two versions only where an input can arrive: every example it gives
/// keeps the constraint and writes a date as a date, no cell is left without an input or a proof
/// that none arrives, and so it can say the versions answer alike outside the region it reports.
#[test]
fn diff_compares_only_where_the_constraint_lets_an_input_arrive() {
    let tmp = TempDir::new("date-constraint-diff");
    let cases: [(&str, String, String, &str, &str); 4] = [
        ("stay_pair", STAY_PAIR.to_string(), STAY_PAIR.replace("| 200USD               |", "| 250USD               |").replace(" v1\n", " v2\n"), "check_in", "check_out"),
        ("宿泊の組", STAY_PAIR_JA.to_string(), STAY_PAIR_JA.replace("| 200USD               |", "| 250USD               |").replace(" v1\n", " v2\n"), "チェックイン日", "チェックアウト日"),
        (
            "coupon_window",
            COUPON_WINDOW.to_string(),
            COUPON_WINDOW.replace("| >1000JPY  | <3980JPY  | -            | no                         |", "| >1000JPY  | <3980JPY  | -            | yes                        |").replace(" v1\n", " v2\n"),
            "issued",
            "used",
        ),
        (
            "クーポンの期間",
            COUPON_WINDOW_JA.to_string(),
            COUPON_WINDOW_JA.replace("| >1000JPY  | <3980JPY  | -            | 対象外             |", "| >1000JPY  | <3980JPY  | -            | 対象               |").replace(" v1\n", " v2\n"),
            "発行日",
            "使用日",
        ),
    ];
    for (name, old, new, first, second) in cases {
        assert_ne!(old, new, "{name}: the second version is the first");
        let (a, b) = (tmp.path().join(format!("{name}_v1.rule")), tmp.path().join(format!("{name}_v2.rule")));
        std::fs::write(&a, &old).unwrap();
        std::fs::write(&b, &new).unwrap();
        let (code, out, e) = rulec(&["diff", &a.to_string_lossy(), &b.to_string_lossy(), "--format", "json"]);
        assert_eq!(code, 1, "{name}: the versions differ\n{out}{e}");
        let j = json(out.lines().next().unwrap_or_default());
        let int = |k: &str| j.get(k).and_then(|v| v.as_int()).unwrap_or(-1);
        assert!(int("differing") > 0, "{name}: {out}");
        assert_eq!((int("unrealized"), int("unsettled")), (0, 0), "{name}: a cell is left with no input and no proof\n{out}");
        assert_eq!(j.get("total").and_then(|v| v.as_bool()), Some(true), "{name}: nothing is said outside the region\n{out}");
        let changes = j.get("changes").and_then(|c| c.as_arr()).unwrap_or_default();
        assert!(!changes.is_empty(), "{name}: {out}");
        for ch in changes {
            let w = ch.get("witness").and_then(|w| w.as_arr()).unwrap_or_default();
            let at = |k: &str| -> String {
                w.iter().find(|x| x.get("input").and_then(|i| i.as_str()) == Some(k)).and_then(|x| x.get("value")).and_then(|v| v.as_str()).unwrap_or_else(|| panic!("{name}: no {k} in the example")).to_string()
            };
            let (x, y) = (at(first), at(second));
            assert!(x.len() == 10 && x.as_bytes()[4] == b'-' && y.len() == 10 && y.as_bytes()[4] == b'-', "{name}: a date is not written as one: {x}, {y}");
            assert!(x <= y, "{name}: the example breaks the constraint: {x}, {y}");
        }
    }
}

/// The state machine's scenarios are cases too: a call that breaks the constraint is E019. And
/// the calls it is walked over keep the constraint, so the claims it can settle are not left open
/// by cells no input was built for. What it leaves open — whether every state can still get to a
/// final one, with the two dates held — the page for people says is open, not broken.
#[test]
fn a_machine_holds_its_calls_to_the_constraint() {
    let tmp = TempDir::new("date-constraint-machine");
    for s in &BOOKINGS {
        let (code, out, e) = rulec(&["check", s.path]);
        assert_eq!(code, 0, "{}: {out}{e}", s.path);
        assert!(!out.contains("no input was built for"), "{}: a call of the machine is left unsettled\n{out}", s.path);
        assert!(out.contains("warning[W127]") && !out.contains("E125"), "{}: {out}", s.path);
        let (code, page, e) = rulec(&["doc", s.path]);
        assert_eq!(code, 0, "{}: {e}", s.path);
        assert!(
            page.contains("Whether a case can get to a final state from every state it reaches could not be settled (W127).") && !page.contains("(E125)"),
            "{}: the page says more than check settled\n{page}",
            s.path
        );
        let src = read(s.path);
        // the June cancellation, with the stay ending the day before it begins
        let broken = src.replacen("2026-06-28 ", "2026-07-03 ", 1);
        assert_ne!(broken, src, "{}: the scenario did not move", s.path);
        let p = tmp.path().join(Path::new(s.path).file_name().unwrap());
        std::fs::write(&p, &broken).unwrap();
        let (code, out, _) = rulec(&["check", &p.to_string_lossy(), "--format", "json"]);
        assert_eq!(code, 1, "{}: {out}", s.path);
        let e019 = out.lines().filter(|l| l.starts_with('{')).map(json).find(|j| j.get("code").and_then(|c| c.as_str()) == Some("E019"));
        assert!(e019.is_some(), "{}: no E019\n{out}", s.path);
        assert!(out.contains("breaks a constraint"), "{}: {out}", s.path);
    }
}

/// Every language the code is generated in answers every vector of the stay and plays every call
/// of the booking: the inputs are ones the generated door lets through, so none is refused there.
#[test]
fn the_generated_code_agrees_with_the_vectors() {
    if !ready(Need::Python, || have("python3"), "python3 is not here") {
        return;
    }
    for s in STAYS.iter().chain(BOOKINGS.iter()) {
        let tmp = TempDir::new("date-constraint-gen");
        let out = tmp.path().join("out");
        let (code, said, e) = rulec(&["gen", s.path, "--out", &out.to_string_lossy()]);
        assert_eq!(code, 0, "{}: {said}{e}", s.path);
        let (_, said, e) = rulec_in(Some(&tmpdir_in(&out)), &["test", &out.to_string_lossy(), "--format", "json"]);
        let j = json(said.lines().next().unwrap_or_else(|| panic!("{}: no result\n{e}", s.path)));
        for why in j.get("skipped").and_then(|v| v.as_arr()).unwrap_or_default() {
            skip(&format!("rulec test: {}", why.as_str().unwrap_or_default()));
        }
        let alias = read(s.path).lines().next().and_then(|l| l.split_whitespace().nth(1)).map(|n| n.split_once('(').map(|(_, a)| a.trim_end_matches(')').to_string()).unwrap_or(n.to_string())).unwrap_or_default();
        let mut ran = 0;
        for r in j.get("results").and_then(|v| v.as_arr()).unwrap_or_default() {
            if r.get("ran").and_then(|v| v.as_bool()) != Some(true) {
                continue;
            }
            assert_eq!(r.get("ok").and_then(|v| v.as_bool()), Some(true), "{}: {r:?}\n{said}", s.path);
            if r.get("rule").and_then(|v| v.as_str()) == Some(alias.as_str()) {
                ran += 1;
            }
        }
        assert!(ran >= 1, "{}: no language ran `{alias}`\n{said}", s.path);
    }
}
