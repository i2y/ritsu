//! The facts a certificate states about a table above whose rows pass a name on (§15.261).
//!
//! Where a table above writes only some of a column's values, the check leaves the others out of
//! every box below, and the certificate states each of them as a `never` fact, which both
//! re-checkers earn back from the rows of the table above (§15.253). A row that passes a name on
//! (`| >=gold | member |`) writes what the name holds, and the certificate had no way to say what
//! that is: it stated no fact, and both re-checkers refused the boxes and the cover the check had
//! built on one, though the rule passed `check`. A row that passes on a column of its own table
//! writes what that column holds where the row fires, which is inside the row's box on that
//! column's axis. The certificate now says which column such a row passes on (`passes`), and the
//! re-checkers read the values it writes off the row's own box, with the answer cell read back out
//! of the file as the column's name. A row that passes on a name of anything else can write any
//! value of its type, so no fact rests on it, and no box is narrowed by one.
//!
//! The materials are in `tests/above_names`, English first, with a Japanese version beside each:
//! the repro (`passed_member.rule`), the lifted tier as the first of two axes below with a row whose
//! box leaves a value out (`passed_member_and_area.rule`), a table in the middle that passes on what
//! the table above decides (`passed_lift_on.rule`), a flag passed on (`passed_flag.rule`), a tier
//! that may be absent (`passed_optional.rule`), and a name the table above has no column for, which
//! narrows nothing (`passed_other_name.rule`). The first five passed `check` before, and both
//! re-checkers refused each certificate.

use ritsu_testkit::{Need, ready};
use rulec::json::Json;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The materials, the English rule first and its Japanese version beside it, with the facts each
/// table states (`(axis, coordinate)`) and the rows that pass on a column (`(row, passes)`).
#[allow(clippy::type_complexity)]
const MATERIALS: &[(&str, &str, &[(&[(i128, i128)], &[(i128, &[Option<i128>])])])] = &[
    ("passed_member", "会員を渡す引き上げ", &[(&[], &[(3, &[Some(0)])]), (&[(0, 0)], &[])]),
    ("passed_member_and_area", "会員を渡す引き上げと地域", &[(&[], &[(3, &[Some(0)])]), (&[(0, 0)], &[])]),
    ("passed_lift_on", "引き上げ先を渡す", &[(&[], &[]), (&[(0, 0), (0, 3)], &[(2, &[Some(0)])]), (&[(0, 0)], &[])]),
    ("passed_flag", "印を渡す", &[(&[], &[]), (&[], &[(1, &[Some(0)])]), (&[(0, 1)], &[])]),
    ("passed_optional", "値の無いこともある会員を渡す", &[(&[], &[(3, &[Some(0)])]), (&[(0, 0), (0, 1)], &[])]),
    ("passed_other_name", "表の外の名前を渡す", &[(&[], &[]), (&[], &[])]),
];

fn rel(stem: &str) -> String {
    format!("tests/above_names/{stem}.rule")
}

fn rulec(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").current_dir(root()).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
}

fn certificate(stem: &str) -> String {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(root()).args(["certificate", &rel(stem)]).output().expect("cannot start rulec");
    assert!(o.status.success(), "{stem}: no certificate:\n{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap()
}

fn have_python() -> bool {
    Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// The Lean program, when the level lets the tests run it and `lake build` in ritsu's `proofs/`
/// has made it; a SKIP line says which it is not.
fn lean_checker() -> Option<PathBuf> {
    let p = root().join("../../proofs/.lake/build/bin/rulec-recheck");
    ready(Need::Lean, || p.exists(), "ritsu's proofs/ is not built (lake build there makes it)").then_some(p)
}

/// Feed a certificate to a re-checker, held to the rule's own file. Returns (exit code, what it said).
fn recheck(program: &[String], cert: &str, stem: &str) -> (i32, String) {
    use std::io::Write;
    let mut p = Command::new(&program[0])
        .args(&program[1..])
        .args(["--rule", &rel(stem)])
        .current_dir(root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("cannot start the re-checker");
    p.stdin.as_mut().unwrap().write_all(cert.as_bytes()).unwrap();
    let o = p.wait_with_output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
}

/// The re-checkers this machine has.
fn checkers() -> Vec<Vec<String>> {
    let mut out = Vec::new();
    if ready(Need::Python, have_python, "python3 is not there") {
        out.push(vec!["python3".to_string(), root().join("tools/recheck.py").to_string_lossy().into_owned()]);
    }
    if let Some(bin) = lean_checker() {
        out.push(vec![bin.to_string_lossy().into_owned()]);
    }
    out
}

fn tables(cert: &str) -> Vec<Json> {
    let j = rulec::json::parse(cert.trim()).unwrap();
    j.get("tables").and_then(Json::as_arr).unwrap().to_vec()
}

/// A table's `never` facts, as (axis, coordinate).
fn nevers(t: &Json) -> Vec<(i128, i128)> {
    t.get("above").and_then(|a| a.get("never")).and_then(Json::as_arr).unwrap().iter().map(|f| (f.get("axis").unwrap().as_int().unwrap(), f.get("coord").unwrap().as_int().unwrap())).collect()
}

/// The rows that pass on a column of their own table, each with what it passes on into each column.
fn passing(t: &Json) -> Vec<(i128, Vec<Option<i128>>)> {
    t.get("rows")
        .and_then(Json::as_arr)
        .unwrap()
        .iter()
        .filter_map(|r| r.get("passes").map(|p| (r.get("row").unwrap().as_int().unwrap(), p.as_arr().unwrap().iter().map(Json::as_int).collect())))
        .collect()
}

/// Each row's box, axis by axis.
fn boxes(t: &Json) -> Vec<Vec<Vec<i128>>> {
    t.get("rows").and_then(Json::as_arr).unwrap().iter().map(|r| r.get("accepts").unwrap().as_arr().unwrap().iter().map(|a| a.as_arr().unwrap().iter().map(|x| x.as_int().unwrap()).collect()).collect()).collect()
}

/// Every material, English and Japanese, passes `check` with nothing to say, and its certificate
/// states the facts and the rows that pass on a column the table above says, and passes both
/// re-checkers held to the rule's file.
#[test]
fn every_material_states_its_facts_and_passes_both_rechecks() {
    let checkers = checkers();
    for (en, ja, want) in MATERIALS {
        for stem in [en, ja] {
            let (code, said) = rulec(&["check", &rel(stem)]);
            assert_eq!(code, 0, "{stem} does not pass check:\n{said}");
            assert!(!said.contains("warning["), "{stem}: {said}");
            let cert = certificate(stem);
            let ts = tables(&cert);
            assert_eq!(ts.len(), want.len(), "{stem}");
            for (t, (facts, rows)) in ts.iter().zip(want.iter()) {
                assert_eq!(nevers(t), facts.to_vec(), "{stem}: the facts of {:?}", t.get("table"));
                let rows: Vec<(i128, Vec<Option<i128>>)> = rows.iter().map(|(r, p)| (*r, p.to_vec())).collect();
                assert_eq!(passing(t), rows, "{stem}: the rows that pass on a column in {:?}", t.get("table"));
            }
            for program in &checkers {
                let (code, said) = recheck(program, &cert, stem);
                assert_eq!(code, 0, "{stem} is refused by {}:\n{said}", program.last().unwrap());
            }
        }
    }
}

/// The certificates of the English rule and its Japanese version state the same boxes, facts and
/// rows that pass on a column, table by table.
#[test]
fn the_japanese_versions_state_the_same_boxes_and_facts() {
    for (en, ja, _) in MATERIALS {
        let (a, b) = (tables(&certificate(en)), tables(&certificate(ja)));
        assert_eq!(a.len(), b.len(), "{en} and {ja}");
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(nevers(x), nevers(y), "{en} and {ja}");
            assert_eq!(boxes(x), boxes(y), "{en} and {ja}");
            assert_eq!(passing(x), passing(y), "{en} and {ja}");
        }
    }
}

/// A name the table above has no column for can be any value of its type, so nothing rests on it:
/// a table below that leaves basic out stops at E101, as it did before.
#[test]
fn a_name_the_table_above_has_no_column_for_narrows_nothing() {
    let src = std::fs::read_to_string(root().join(rel("passed_other_name"))).unwrap();
    let cut = src.replace("| <=silver | 3USD      |\n| >=gold   | 5USD      |", "| >=silver | 3USD      |");
    assert_ne!(cut, src, "the material changed its shape");
    let codes: Vec<&str> = rulec::i18n::with(rulec::i18n::Lang::En, || rulec::check_source(&cut, &rel("passed_other_name"))).iter().map(|d| d.code).collect();
    assert!(codes.contains(&"E101"), "{codes:?}");
}

/// A box may leave out only a value the rows above never write. A fact that the row passing the
/// member on never writes platinum, which its box takes, is refused; so is the row's `passes` taken
/// away, which leaves what it writes unread; so is a box narrower than its cell less the facts; and so
/// is a row said to pass on a column its answer cell does not name. A row said to write the name it
/// passes on as a word — the file does write that word there — is refused too, though the facts,
/// the box and the cover are made to fit: `member` is no value of tier. So is a word a name of the
/// rule is spelled as, which a cell reads as the name.
#[test]
fn a_box_narrowed_by_a_value_a_row_above_passes_on_is_refused() {
    let checkers = checkers();
    let cert = certificate("passed_member");
    const PASSES: &str = r#""passes":[0]"#;
    const FACTS: &str = r#""never":[{"axis":0,"coord":0}]"#;
    const BOX: &str = r#""accepts":[[1,2,3]]"#;
    assert!(cert.matches(PASSES).count() == 1 && cert.matches(FACTS).count() == 1 && cert.matches(BOX).count() == 1, "the certificate changed its shape, and the lies below would tell nothing:\n{cert}");
    let area = certificate("passed_member_and_area");
    assert!(area.matches(PASSES).count() == 1, "the certificate changed its shape:\n{area}");
    const ROW: &str = r#""produces":[null],"passes":[0]"#;
    const COVER: &str = r#""cover":{"split":[{"upstream":"lift = basic"},{"row":1},{"row":1},{"row":1}]}"#;
    const TYPES: &str = r#""types":{"#;
    assert!(cert.matches(ROW).count() == 1 && cert.matches(COVER).count() == 1 && cert.matches(TYPES).count() == 1, "the certificate changed its shape:\n{cert}");
    let a_word = cert
        .replace(ROW, r#""produces":["member"]"#)
        .replace(FACTS, r#""never":[{"axis":0,"coord":0},{"axis":0,"coord":3}]"#)
        .replace(BOX, r#""accepts":[[1,2]]"#)
        .replace(COVER, r#""cover":{"split":[{"upstream":"lift = basic"},{"row":1},{"row":1},{"upstream":"lift = platinum"}]}"#);
    let a_name = cert.replace(TYPES, r#""types":{"gold":"tier","#);
    // (what, the forged certificate, the rule it is held to, what Python says, what Lean says)
    let lies = [
        (
            "platinum called a value the row passing the member on never writes",
            cert.replace(FACTS, r#""never":[{"axis":0,"coord":0},{"axis":0,"coord":3}]"#).replace(BOX, r#""accepts":[[1,2]]"#),
            "passed_member",
            "row 3 of lift_by_member passes on member, which can be platinum there",
            "a row of lift_by_member passes on member, which can be platinum there",
        ),
        ("the row's passes taken away", cert.replace(&format!(",{PASSES}"), ""), "passed_member", "row 3 of lift_by_member writes something this program cannot read", "a row of lift_by_member writes something this program cannot read"),
        ("gold left out of the box", cert.replace(BOX, r#""accepts":[[1,3]]"#), "passed_member", "row 1's box on lift is not what its cell `>= silver` describes", "the box it states is not the one its cell describes"),
        ("the row said to pass on the area", area.replace(PASSES, r#""passes":[1]"#), "passed_member_and_area", "the file writes `member` there, and the certificate reads it as `area`", "the file writes `member` there, and the certificate reads it as `area`"),
        ("the name the row passes on said to be a word it writes", a_word, "passed_member", "row 3 is said to write `member` into lift, which is no value of tier", "row 3 is said to write `member` into lift, which is no value of tier"),
        ("gold said to be a name of the rule", a_name, "passed_member", "row 2 is said to write the word `gold` into lift, and `gold` is a name of the rule", "row 2 is said to write the word `gold` into lift, and `gold` is a name of the rule"),
    ];
    for program in &checkers {
        let lean = !program[0].ends_with("python3");
        for (what, forged, stem, py, ln) in &lies {
            let (code, said) = recheck(program, forged, stem);
            assert_eq!(code, 1, "{what}: {} let it through:\n{said}", program.last().unwrap());
            let why = if lean { ln } else { py };
            assert!(said.contains(why), "{what}: {}:\n{said}", program.last().unwrap());
        }
    }
}

/// The hole `check_words` closes, run end to end. The rule below lifts a member from gold up to
/// their own tier and pays only silver and gold, so `check` stops at E101: a platinum member is
/// lifted to platinum, and no row pays it. A certificate for it, forged from the one of
/// `passed_member.rule`, calls the row that passes the member on a row that writes the word
/// `member`, says platinum is a value no row above writes, and leaves it out of the box and the
/// cover. The file does write `member` there, so the cells read back; before, both re-checkers took
/// the certificate, claims and all. Now both refuse it: `member` is no value of tier.
#[test]
fn a_gap_hidden_behind_a_name_said_to_be_a_word_is_refused() {
    let checkers = checkers();
    let src = std::fs::read_to_string(root().join(rel("passed_member"))).unwrap();
    let gap = src
        .replace("| lift     | -> credit |\n| >=silver | 5USD      |", "| lift         | -> credit |\n| silver, gold | 5USD      |")
        .replace("| platinum | 5USD      |", "| gold     | 5USD      |");
    assert_ne!(gap, src, "the material changed its shape");
    let t = ritsu_testkit::TempDir::new("above-names-gap");
    let path = t.path().join("gap.rule");
    std::fs::write(&path, &gap).unwrap();
    let codes: Vec<&str> = rulec::i18n::with(rulec::i18n::Lang::En, || rulec::check_source(&gap, &path.to_string_lossy())).iter().map(|d| d.code).collect();
    assert_eq!(codes, ["E101"], "the rule with a gap is not the one this test means");
    let cert = certificate("passed_member");
    let line = gap.lines().find(|l| l.starts_with("| silver, gold |")).unwrap();
    let (cell, answer) = (line.find("silver, gold").unwrap(), line.find("5USD").unwrap());
    let parts: [(&str, String); 7] = [
        (r#""produces":[null],"passes":[0]"#, r#""produces":["member"]"#.to_string()),
        (r#""cells":[">= silver"],"tests":[{"cell":"rank","tests":[{"op":">=","value":"silver"}]}]"#, r#""cells":["silver, gold"],"tests":[{"cell":"is","words":["silver","gold"]}]"#.to_string()),
        (r#""source":[{"line":22,"col":2,"len":8,"text":">=silver"}]"#, format!(r#""source":[{{"line":22,"col":{cell},"len":12,"text":"silver, gold"}}]"#)),
        (r#""source":{"line":22,"col":13,"len":4,"text":"5USD"}"#, format!(r#""source":{{"line":22,"col":{answer},"len":4,"text":"5USD"}}"#)),
        (r#""accepts":[[1,2,3]]"#, r#""accepts":[[1,2]]"#.to_string()),
        (r#""never":[{"axis":0,"coord":0}]"#, r#""never":[{"axis":0,"coord":0},{"axis":0,"coord":3}]"#.to_string()),
        (r#""cover":{"split":[{"upstream":"lift = basic"},{"row":1},{"row":1},{"row":1}]}"#, r#""cover":{"split":[{"upstream":"lift = basic"},{"row":1},{"row":1},{"upstream":"lift = platinum"}]}"#.to_string()),
    ];
    let mut forged = cert.clone();
    for (from, to) in &parts {
        assert_eq!(forged.matches(from).count(), 1, "the certificate changed its shape, and the lie below would tell nothing: {from}\n{cert}");
        forged = forged.replace(from, to);
    }
    let digest = rulec::sha256::hex(gap.as_bytes());
    let was = rulec::json::parse(cert.trim()).unwrap().get("source_sha256").and_then(Json::as_str).unwrap().to_string();
    let forged = forged.replace(&was, &digest);
    for program in &checkers {
        use std::io::Write;
        let mut p = Command::new(&program[0])
            .args(&program[1..])
            .args(["--rule", "gap.rule"])
            .current_dir(t.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("cannot start the re-checker");
        p.stdin.as_mut().unwrap().write_all(forged.as_bytes()).unwrap();
        let o = p.wait_with_output().unwrap();
        let said = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
        assert_eq!(o.status.code(), Some(1), "{} took a certificate of a rule with a gap:\n{said}", program.last().unwrap());
        assert!(said.contains("row 3 is said to write `member` into lift, which is no value of tier"), "{}:\n{said}", program.last().unwrap());
        assert!(said.contains("8 cells"), "the forged certificate does not quote the file, and the test tells nothing:\n{said}");
    }
}
