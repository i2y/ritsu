//! `rulec strength` (§15.288): which changes to a table anything notices.
//!
//! The changes are made to the source and handed to the same check, so what is held here is the
//! three kinds of change, who is said to notice each, that the other table really passes, and that
//! the answer is the same twice.

use ritsu_testkit::TempDir;
use rulec::json::Json;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_in(lang: &str, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", lang).current_dir(root()).args(args).output().expect("rulec does not start");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn rulec(args: &[&str]) -> (i32, String, String) {
    run_in("en", args)
}

fn json_of(file: &str, more: &[&str]) -> Json {
    let mut args = vec!["strength", file, "--format", "json"];
    args.extend_from_slice(more);
    let (c, out, err) = rulec(&args);
    assert_eq!(c, 0, "{file}: {err}");
    rulec::json::parse(out.trim()).unwrap_or_else(|e| panic!("{file}: {e}\n{out}"))
}

fn num(j: &Json, k: &str) -> i64 {
    j.get(k).and_then(|v| v.as_int()).map(|n| n as i64).unwrap_or_else(|| panic!("no number `{k}`"))
}

fn items<'a>(j: &'a Json, k: &str) -> &'a [Json] {
    j.get(k).and_then(|v| v.as_arr()).unwrap_or_else(|| panic!("no list `{k}`"))
}

fn text<'a>(j: &'a Json, k: &str) -> &'a str {
    j.get(k).and_then(|v| v.as_str()).unwrap_or("")
}

/// `(row, what it is about, from, to)`, for finding one change among many.
fn says(j: &Json, list: &str, kind: &str) -> Vec<(i64, String, String, String)> {
    items(j, list)
        .iter()
        .filter(|i| text(i, "kind") == kind)
        .map(|i| {
            let about = if kind == "boundary" { text(i, "column") } else { text(i, "output") };
            (num(i, "row"), about.to_string(), text(i, "from").to_string(), text(i, "to").to_string())
        })
        .collect()
}

fn has(list: &[(i64, String, String, String)], row: i64, about: &str, from: &str, to: &str) -> bool {
    list.iter().any(|(r, a, f, t)| *r == row && a == about && f == from && t == to)
}

const DOOR: &str = "tests/strength/door.rule";
const FARE: &str = "tests/strength/fare.rule";

#[test]
fn three_kinds_of_change_are_made() {
    let j = json_of(DOOR, &[]);
    let every = |kind| ["unnoticed", "noticed", "same"].iter().flat_map(|l| says(&j, l, kind)).collect::<Vec<_>>();
    let outputs = every("output");
    assert!(has(&outputs, 2, "next_state", "open", "closed"), "an enum to its other values");
    assert!(has(&outputs, 2, "next_state", "open", "state"), "an enum to the input that holds it");
    assert!(has(&outputs, 2, "moved", "true", "false"), "a boolean to its other value");
    assert!(has(&outputs, 5, "fee", "2USD", "3USD"), "a literal one step up");
    assert!(has(&outputs, 5, "fee", "2USD", "1USD"), "a literal one step down");
    assert!(has(&outputs, 5, "fee", "2USD", "0USD"), "another expression of the column");
    assert!(!outputs.iter().any(|(_, a, _, t)| a == "fee" && t == "-1USD"), "a value the output's range does not hold is not tried");
    let rows = every("row");
    assert_eq!(rows.len(), 7, "one row out at a time: {rows:?}");
    let moves = every("boundary");
    assert!(has(&moves, 1, "force", "<5, >=5", "<4, >=4"), "{moves:?}");
    assert!(has(&moves, 1, "force", "<5, >=5", "<6, >=6"), "{moves:?}");
}

#[test]
fn a_boundary_moves_in_every_row_and_stays_on_its_column() {
    let d = FARE;
    let j = json_of(d, &[]);
    let every = ["unnoticed", "noticed", "same"].iter().flat_map(|l| says(&j, l, "boundary")).collect::<Vec<_>>();
    // Two clauses write the one boundary of a column between them.
    assert!(has(&every, 1, "age", "<13, >=13", "<12, >=12"), "{every:?}");
    assert!(has(&every, 1, "age", "<13, >=13", "<14, >=14"), "{every:?}");
    // `<=` and `>=` on a date write one boundary, a day apart.
    assert!(has(&every, 1, "ride", "<=2026-06-30, >=2026-07-01", "<=2026-06-29, >=2026-06-30"), "{every:?}");
    assert!(has(&every, 1, "ride", "<=2026-06-30, >=2026-07-01", "<=2026-07-01, >=2026-07-02"), "{every:?}");
    // An ordered enum's boundary moves along its order, and not off its end.
    assert!(has(&every, 1, "member", "<gold, >=gold", "<silver, >=silver"), "{every:?}");
    assert_eq!(every.iter().filter(|(_, a, _, _)| a == "member").count(), 1, "{every:?}");
}

#[test]
fn a_rate_and_an_optional_output_get_what_their_types_hold() {
    let d = FARE;
    let j = json_of(d, &[]);
    let every = ["unnoticed", "noticed", "same"].iter().flat_map(|l| says(&j, l, "output")).collect::<Vec<_>>();
    assert!(has(&every, 1, "share", "50%", "49%") && has(&every, 1, "share", "50%", "100%"), "{every:?}");
    assert!(!every.iter().any(|(_, a, f, t)| a == "share" && f == "100%" && t == "101%"), "over the range: {every:?}");
    assert!(has(&every, 1, "booking", "3USD", "none") && has(&every, 3, "booking", "none", "3USD"), "{every:?}");
}

#[test]
fn a_point_of_the_column_keeps_its_boundary_where_it_is() {
    let (_t, dir) = scratch("point");
    let f = dir.join("point.rule");
    std::fs::write(
        &f,
        "rule point v1\ndescription \"A column with a point in it\"\n\ninputs\n  n : number  range >=0 <=9\n\noutputs\n  a : number  round down(1)\n\ntable t\npolicy unique\n| n   | -> a |\n| <3  | 1    |\n| 3   | 2    |\n| >=4 <7 | 3    |\n| >=7 | 4    |\n",
    )
    .unwrap();
    let j = json_of(f.to_str().unwrap(), &[]);
    let every = ["unnoticed", "noticed", "same"].iter().flat_map(|l| says(&j, l, "boundary")).collect::<Vec<_>>();
    assert!(every.iter().all(|(_, _, f, _)| f.contains('7')), "only the boundary with no point against it moves: {every:?}");
    assert!(has(&every, 3, "n", "<7, >=7", "<6, >=6"), "{every:?}");
    assert!(has(&every, 3, "n", "<7, >=7", "<4, >=4"), "to the next boundary, with the point left out of it: {every:?}");
    assert!(!has(&every, 3, "n", "<7, >=7", "<3, >=3"), "not across the point: {every:?}");
}

#[test]
fn the_edge_of_a_range_is_not_a_boundary() {
    let (_t, dir) = scratch("edge");
    let f = dir.join("edge.rule");
    std::fs::write(
        &f,
        "rule edge v1\ndescription \"A column written out to both edges of its range\"\n\ninputs\n  n : number  range >=0 <=9\n\noutputs\n  a : number  round down(1)\n\ntable t\npolicy unique\n| n       | -> a |\n| >=0 <5  | 1    |\n| >=5 <=9 | 2    |\n",
    )
    .unwrap();
    let j = json_of(f.to_str().unwrap(), &[]);
    let every = ["unnoticed", "noticed", "same"].iter().flat_map(|l| says(&j, l, "boundary")).collect::<Vec<_>>();
    assert!(!every.is_empty() && every.iter().all(|(_, _, f, _)| f == "<5, >=5"), "only the boundary between the two rows moves: {every:?}");
}

#[test]
fn four_classes_and_the_same_answers() {
    let j = json_of(DOOR, &[]);
    assert_eq!(num(&j, "changes"), num(&j, "claims") + num(&j, "tests") + num(&j, "warnings") + num(&j, "nothing"));
    assert_eq!(items(&j, "unnoticed").len() as i64, num(&j, "nothing"));
    assert_eq!(items(&j, "same").len() as i64, num(&j, "same_answers"));
    assert_eq!(items(&j, "noticed").len() as i64, num(&j, "claims") + num(&j, "tests") + num(&j, "warnings"));

    let noticed = |row: i64, out: &str, to: &str| -> (String, Vec<String>) {
        let i = items(&j, "noticed").iter().find(|i| num(i, "row") == row && text(i, "output") == out && text(i, "to") == to).unwrap_or_else(|| panic!("row {row} {out} -> {to}"));
        (text(i, "class").to_string(), items(i, "by").iter().filter_map(|c| c.as_str().map(str::to_string)).collect())
    };
    // The claim stops it whatever the tests say.
    let (class, by) = noticed(7, "next_state", "locked");
    assert_eq!((class.as_str(), by.contains(&"E126".to_string())), ("claims", true), "{by:?}");
    // Only the scenario does.
    let (class, by) = noticed(3, "moved", "false");
    assert_eq!((class.as_str(), by), ("tests", vec!["E107".to_string()]));
    // Nobody does, and the call that shows it comes with it.
    let nobody = says(&j, "unnoticed", "output");
    assert!(has(&nobody, 4, "next_state", "state", "open"), "{nobody:?}");
    // The row of one state, naming the state: the same rule written another way.
    let same = says(&j, "same", "output");
    assert!(has(&same, 1, "next_state", "state", "closed") && has(&same, 7, "next_state", "state", "open"), "{same:?}");

    // A new warning alone: what no longer reaches a state of the machine.
    let j = json_of("tests/strength/alarm.rule", &[]);
    let warned: Vec<&Json> = items(&j, "noticed").iter().filter(|i| text(i, "class") == "warnings").collect();
    assert!(!warned.is_empty() && num(&j, "warnings") == warned.len() as i64);
    assert!(warned.iter().all(|i| items(i, "by").iter().any(|c| c.as_str() == Some("W125"))));
}

#[test]
fn an_unnoticed_change_says_a_call_and_how_to_get_there() {
    let j = json_of(DOOR, &[]);
    let one = items(&j, "unnoticed").iter().find(|i| num(i, "row") == 6 && text(i, "output") == "next_state" && text(i, "to") == "closed").expect("row 6");
    let call = one.get("call").expect("call");
    assert!(call.get("inputs").is_some() && call.get("outputs").is_some());
    let calls = items(one, "calls");
    assert_eq!(calls.len(), 2, "bolt, then the call that parts them");
    let last = calls.last().unwrap();
    assert_eq!(text(last.get("old").unwrap(), "next_state"), "locked");
    assert_eq!(text(last.get("new").unwrap(), "next_state"), "closed");

    // Outside any machine there is one call, and no sequence.
    let d = FARE;
    let j = json_of(d, &[]);
    let one = items(&j, "unnoticed").first().expect("something goes unnoticed");
    assert!(one.get("call").and_then(|c| c.get("inputs")).is_some());
    assert!(matches!(one.get("calls"), Some(Json::Null)));
}

#[test]
fn text_says_the_conclusion_first() {
    let (c, out, _) = rulec(&["strength", DOOR]);
    assert_eq!(c, 0);
    let j = json_of(DOOR, &[]);
    let first = out.lines().next().unwrap();
    assert_eq!(first, format!("{DOOR}: {} of {} changes to the rule are noticed by nothing.", num(&j, "nothing"), num(&j, "changes")));
    assert!(out.contains("Noticed by nothing:") && out.contains("Calls (from closed):"), "{out}");
    assert!(out.contains("next_state state -> open | locked"), "one place on one line: {out}");
    assert!(out.contains("--other other.rule writes it."), "{out}");
    assert!(out.contains("at least") && out.contains("not said to be the most different"), "{out}");

    let (c, out, _) = rulec(&["strength", "tests/corpus/retry_attempts.rule"]);
    assert_eq!(c, 0);
    assert!(out.lines().next().unwrap().ends_with("every one of 24 changes to the rule is noticed."), "{out}");
    assert!(!out.contains("Another table"), "{out}");

    let (c, out, _) = run_in("ja", &["strength", DOOR]);
    assert_eq!(c, 0);
    assert!(out.lines().next().unwrap().contains("誰も気づきません"), "{out}");
    assert!(out.contains("少なくとも"), "{out}");
}

#[test]
fn at_most_is_the_gate() {
    let j = json_of(DOOR, &[]);
    let n = num(&j, "nothing").to_string();
    let less = (num(&j, "nothing") - 1).to_string();
    assert_eq!(rulec(&["strength", DOOR, "--at-most", &n]).0, 0);
    assert_eq!(rulec(&["strength", DOOR, "--at-most", &less]).0, 1);
    assert_eq!(rulec(&["strength", DOOR]).0, 0, "with no gate it is a report");
    assert_eq!(rulec(&["strength", DOOR, "--at-most", "many"]).0, 2);
}

#[test]
fn the_other_table_passes_and_can_be_compared() {
    let (_t, dir) = scratch("other");
    let at = dir.join("other.rule");
    let at = at.to_str().unwrap();
    let (c, out, err) = rulec(&["strength", DOOR, "--other", at]);
    assert_eq!(c, 0, "{err}");
    assert!(out.contains(at), "{out}");
    let (src, other) = (std::fs::read_to_string(root().join(DOOR)).unwrap(), std::fs::read_to_string(at).unwrap());
    assert_ne!(src, other);
    assert!(other.contains("# an open door stays open") && other.contains("# A bolted door"), "comments stay");
    // The order and the shape of the file stay: only table rows differ.
    let (a, b): (Vec<&str>, Vec<&str>) = (src.lines().collect(), other.lines().collect());
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert!(x == y || x.starts_with('|'), "{x}\n{y}");
    }
    let (c, _, err) = rulec(&["check", at]);
    assert_eq!(c, 0, "{err}");
    let (c, out, _) = rulec(&["diff", DOOR, at]);
    assert_eq!(c, 1, "the two answer differently: {out}");

    let j = json_of(DOOR, &["--other", at]);
    let o = j.get("other").unwrap();
    assert_eq!(text(o, "written"), at);
    assert!(num(o, "cells") > 0 && num(o, "cells") <= num(o, "of"), "{}", rulec::json::unparse(o));
    assert_eq!(num(o, "of"), 21);
    // The cells it counts are the cells that differ.
    let differing: usize = a.iter().zip(&b).filter(|(x, y)| x != y).map(|(x, y)| x.split('|').zip(y.split('|')).skip(4).filter(|(p, q)| p.trim() != q.trim()).count()).sum();
    assert_eq!(differing as i64, num(o, "cells"));
}

#[test]
fn the_other_table_is_not_written_over_the_file_nor_where_it_cannot_read_what_it_reads() {
    let (c, _, err) = rulec(&["strength", DOOR, "--other", DOOR]);
    assert_eq!(c, 2, "{err}");
    // Where nothing can be kept, nothing is written, and it says so.
    let (_t, dir) = scratch("none");
    let at = dir.join("other.rule");
    let (c, _, err) = rulec(&["strength", "tests/corpus/retry_attempts.rule", "--other", at.to_str().unwrap()]);
    assert_eq!(c, 0, "{err}");
    assert!(err.contains("nothing is written") && !at.exists(), "{err}");
    // A rule that applies another reads it from its own directory.
    let (_t, dir) = scratch("away");
    let at = dir.join("other.rule");
    let f = "tests/apply_absent/absent_tiers_read.rule";
    let (c, _, err) = rulec(&["strength", f, "--other", at.to_str().unwrap()]);
    assert_eq!(c, 2, "{err}");
    assert!(err.contains("does not pass check") && err.contains("E044"), "{err}");
    assert!(!at.exists());
}

#[test]
fn claims_only_takes_the_tests_out() {
    let (_t, dir) = scratch("claims");
    let at = dir.join("bare.rule");
    let at = at.to_str().unwrap();
    let all = json_of(DOOR, &[]);
    let j = json_of(DOOR, &["--claims-only", "--other", at]);
    assert!(j.get("claims_only").and_then(|v| v.as_bool()).unwrap_or(false));
    assert_eq!(num(&j, "tests"), 0);
    assert_eq!(num(&j, "changes"), num(&j, "claims") + num(&j, "warnings") + num(&j, "nothing"));
    assert!(num(&j, "nothing") > num(&all, "nothing"), "what only a test noticed nobody does now");
    let bare = std::fs::read_to_string(at).unwrap();
    assert!(!bare.contains("scenario") && !bare.contains("examples"), "{bare}");
    assert!(!bare.contains("# A bolted door"), "a comment above a block goes with it: {bare}");
    assert!(bare.contains("# an open door stays open"), "{bare}");
    assert_eq!(rulec(&["check", at]).0, 0);
    let (_, out, _) = rulec(&["strength", DOOR, "--claims-only"]);
    assert!(out.lines().next().unwrap().contains("scenarios and examples taken out"), "{out}");
    assert!(!out.contains("only by a scenario"), "{out}");
    assert!(out.contains("--claims-only --other other.rule"), "{out}");
}

#[test]
fn the_json_has_its_keys() {
    let j = json_of(DOOR, &[]);
    let keys = |v: &Json| -> Vec<String> { rulec::json::members(v).unwrap().keys().map(|s| s.to_string()).collect() };
    let want: Vec<String> = ["changes", "claims", "claims_only", "file", "nothing", "noticed", "other", "same", "same_answers", "tests", "unnoticed", "warnings"].iter().map(|s| s.to_string()).collect();
    assert_eq!(keys(&j), want);
    let want: Vec<String> = ["call", "calls", "clause", "column", "from", "kind", "output", "row", "table", "to", "when"].iter().map(|s| s.to_string()).collect();
    assert_eq!(keys(&items(&j, "unnoticed")[0]), want);
    let mut with: Vec<String> = want.clone();
    with.extend(["by".to_string(), "class".to_string()]);
    with.sort();
    assert_eq!(keys(&items(&j, "noticed")[0]), with);
    assert_eq!(keys(j.get("other").unwrap()), ["boundaries", "cells", "of", "rows", "written"].map(String::from).to_vec());
    assert_eq!(text(&j, "file"), DOOR);
    let kinds: BTreeMap<&str, usize> = items(&j, "noticed").iter().fold(BTreeMap::new(), |mut m, i| {
        *m.entry(text(i, "kind")).or_default() += 1;
        m
    });
    assert!(kinds.contains_key("output") && kinds.contains_key("row"), "{kinds:?}");
    // The keys do not move with the language.
    let (_, ja, _) = run_in("ja", &["strength", DOOR, "--format", "json"]);
    assert_eq!(keys(&rulec::json::parse(ja.trim()).unwrap()), keys(&j));
}

#[test]
fn the_order_is_tables_rows_columns_and_a_run_is_the_same_twice() {
    for f in [DOOR, "tests/date_constraint/hotel_booking.rule", "tests/strength/alarm.rule"] {
        let a = rulec(&["strength", f, "--format", "json"]);
        assert_eq!(a, rulec(&["strength", f, "--format", "json"]), "{f}");
        assert_eq!(rulec(&["strength", f]), rulec(&["strength", f]), "{f}");
    }
    let j = json_of(DOOR, &[]);
    let rows: Vec<i64> = items(&j, "unnoticed").iter().map(|i| num(i, "row")).collect();
    let mut sorted = rows.clone();
    sorted.sort();
    assert_eq!(rows, sorted);
}

#[test]
fn a_file_that_applies_or_imports_is_checked_where_it_is() {
    // The rows of what it applies are that file's: only its own are changed.
    let f = "tests/apply_absent/absent_tiers_read.rule";
    let j = json_of(f, &[]);
    let src = std::fs::read_to_string(root().join(f)).unwrap();
    for l in ["unnoticed", "noticed", "same"] {
        for i in items(&j, l) {
            let table = text(i, "table");
            assert!(src.contains(&format!("table {table}")) || src.contains(&format!("clause {table}")), "{table} is not this file's");
        }
    }
    assert!(num(&j, "changes") > 0);
    // A file with no table of its own has nothing to change.
    let (c, out, _) = rulec(&["strength", "tests/apply_fixtures/適用.rule"]);
    assert_eq!(c, 0);
    assert!(out.contains("there is nothing to change"), "{out}");

    let f = "tests/corpus/shipping_fee_proviso.rule";
    let j = json_of(f, &[]);
    assert!(num(&j, "changes") > 0);
    // A clause goes whole, its `overrides` line with it.
    let gone = ["unnoticed", "noticed"].iter().flat_map(|l| items(&j, l)).find(|i| text(i, "table") == "free" && text(i, "kind") == "row").expect("the clause taken out");
    assert!(gone.get("clause").and_then(|v| v.as_bool()).unwrap_or(false));
    assert!(!items(gone, "by").iter().any(|c| c.as_str() == Some("E005")), "the file still reads with it gone");
}

#[test]
fn a_contribution_is_changed_in_its_own_scope() {
    let j = json_of("tests/merging/override/contrib/grace_policy.rule", &[]);
    assert!(num(&j, "tests") >= 1, "its example notices its own cell");
    assert!(items(&j, "noticed").iter().any(|i| text(i, "output") == "decay" && items(i, "by").iter().any(|c| c.as_str() == Some("E107"))));
}

#[test]
fn a_file_in_japanese_is_cut_where_its_cells_are() {
    let (_t, dir) = scratch("ja");
    let at = dir.join("other.ja.rule");
    let at = at.to_str().unwrap();
    let f = "tests/strength/door.ja.rule";
    let j = json_of(f, &["--other", at]);
    let en = json_of(DOOR, &[]);
    for k in ["changes", "claims", "tests", "warnings", "nothing", "same_answers"] {
        assert_eq!(num(&j, k), num(&en, k), "{k}: the version in Japanese is the same table");
    }
    let every = says(&j, "noticed", "output");
    assert!(has(&every, 5, "料金", "200円", "201円"), "{every:?}");
    assert_eq!(rulec(&["check", at]).0, 0);
    assert!(std::fs::read_to_string(at).unwrap().contains("# 開いた扉は開いたまま"));
}

/// The check of a rule whose range is a koyomi date needs the port of dates (§15.174), on whichever
/// thread it runs. `ritsu rulec` hands it over as this does.
#[test]
fn the_port_of_dates_reaches_every_thread() {
    let rel = "tests/days/settlement.rule";
    let (c, _, err) = rulec(&["strength", rel]);
    assert_eq!(c, 2, "no koyomi is joined: {err}");
    assert!(err.contains("E129"), "{err}");

    let path = root().join(rel);
    let path = path.to_str().unwrap();
    let src = std::fs::read_to_string(path).unwrap();
    let port: rulec::days::Port = std::sync::Arc::new(koyomi::ports::Engine);
    let opts = rulec::strength::Options { budget: rulec::region::DEFAULT_BUDGET, claims_only: false, other: None };
    let Ok(s) = rulec::days::with(Some(port), || rulec::strength::measure(&src, path, &opts)) else { panic!("{rel} does not pass with koyomi joined") };
    assert!(s.changes() > 0);
    assert!(s.results.iter().all(|(_, o)| !o.by.contains(&"E129")), "a thread checked with no port");
    assert!(s.results.iter().any(|(_, o)| !o.by.is_empty()) || s.nothing() > 0);
}

#[test]
fn a_file_with_an_error_is_refused() {
    let (c, _, err) = rulec(&["strength", "tests/mutants/m_e101.rule"]);
    assert_eq!(c, 2);
    assert!(err.contains("E101") && err.contains("fix it first"), "{err}");
    let (c, _, err) = run_in("ja", &["strength", "tests/mutants/m_e101.rule"]);
    assert_eq!(c, 2);
    assert!(err.contains("直してから"), "{err}");
    // A budget the file does not fit in is the file's error too.
    let (c, _, err) = rulec(&["strength", DOOR, "--budget", "1"]);
    assert_eq!(c, 2);
    assert!(err.contains("E109"), "{err}");
}

#[test]
fn bad_arguments_are_refused() {
    for args in [
        vec!["strength"],
        vec!["strength", DOOR, "--budget", "0"],
        vec!["strength", DOOR, "--budget", "x"],
        vec!["strength", DOOR, "--format", "xml"],
        vec!["strength", DOOR, "--no-such-flag"],
        vec!["strength", "tests/no/such/file.rule"],
        vec!["strength", DOOR, "tests/strength/alarm.rule"],
        vec!["strength", "tests/strength"],
    ] {
        let (c, _, err) = rulec(&args);
        assert_eq!(c, 2, "{args:?}: {err}");
    }
    // An `enums` file decides nothing.
    let f = enums_file();
    let (c, _, err) = rulec(&["strength", &f]);
    assert_eq!(c, 2, "{err}");
    assert!(err.contains("enums"), "{err}");
}

#[test]
fn the_reference_shows_what_the_command_prints() {
    let doc = std::fs::read_to_string(root().join("docs/reference.md")).unwrap();
    let (_, out, _) = rulec(&["strength", DOOR]);
    let shown = format!("$ rulec strength {DOOR}\n{out}");
    assert!(doc.contains(&shown), "docs/reference.md is behind:\n{out}");
}

/// The five rules the first count was taken on: the changes it made and who noticed them are all
/// here, and these are this command's own counts of the file.
#[test]
fn the_counts_of_the_five_rules_stay() {
    let rows = [
        ("tests/corpus/order_lifecycle.rule", (93, 35, 43, 0, 15, 5)),
        ("tests/corpus/one_time_perk.rule", (61, 35, 11, 0, 15, 8)),
        ("tests/corpus/retry_attempts.rule", (24, 21, 3, 0, 0, 3)),
    ];
    for (f, (n, cl, te, wa, no, sa)) in rows {
        let j = json_of(f, &[]);
        assert_eq!((num(&j, "changes"), num(&j, "claims"), num(&j, "tests"), num(&j, "warnings"), num(&j, "nothing"), num(&j, "same_answers")), (n, cl, te, wa, no, sa), "{f}");
    }
}

fn scratch(tag: &str) -> (TempDir, PathBuf) {
    let t = TempDir::new(&format!("strength-{tag}"));
    let d = t.path().to_path_buf();
    (t, d)
}

fn enums_file() -> String {
    "tests/names/enum_name_is_an_alias.rule".to_string()
}
