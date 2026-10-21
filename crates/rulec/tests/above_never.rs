//! The boxes a certificate states below a table that writes only some of an enum's values (§15.253).
//!
//! The check reads every box of a table without the coordinates of a column decided above whose
//! value no row of the deciding table writes: no input brings the column there. The certificate
//! states those boxes, so a row whose cell takes such a value too — `-`, `>=silver`, `not: basic`,
//! `<=gold`, a list of values — states a box narrower than the one its cell describes. Both
//! re-checkers rebuild each box from its cell and hold the stated box to it, so they refused every
//! such certificate, though the rule passed `check`. They now leave out of the rebuilt box the
//! coordinates the table's `never` facts name, each fact earned back from the rows of the table
//! that decides the column; and the certificate states every such coordinate, not only the ones a
//! leaf of the cover rested on, and writes the box that fixes one as a single leaf on the tables
//! above, however many axes it leaves open.
//!
//! The materials are in `tests/above_never`, English first, with a Japanese version beside each.
//! Each lifts a member's tier to silver or gold in a table above, and reads the lifted tier below:
//! the repro (`plain_lift.rule`), each kind of cell, and two rows that meet only at platinum
//! (`lift_cells.rule`), the lifted tier as the first of two axes (`lift_and_area.rule`), a flag the
//! table above only sets and a `first` table (`flag_first.rule`), an `apply` of a rule whose rows
//! take basic and platinum (`lifted_credit.rule`, of `tier_credit.rule`), and a tier that may be
//! absent (`optional_lift.rule`). Every one of them passed `check` before, and both re-checkers
//! refused its certificate.

use ritsu_testkit::{Need, ready};
use rulec::json::Json;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The rules that read a column decided above, the English rule first and its Japanese version
/// beside it.
const MATERIALS: [(&str, &str); 6] = [
    ("plain_lift", "段階の引き上げ"),
    ("lift_cells", "引き上げた段階のセル"),
    ("lift_and_area", "引き上げた段階と地域"),
    ("flag_first", "対象と段階の値引き"),
    ("lifted_credit", "引き上げた段階の値引き"),
    ("optional_lift", "値が無いことのある段階"),
];

/// The rule `lifted_credit.rule` applies, and its Japanese version. No table is above its table.
const CALLEES: [(&str, &str); 1] = [("tier_credit", "段階ごとの値引き")];

fn rel(stem: &str) -> String {
    format!("tests/above_never/{stem}.rule")
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

/// The re-checkers this machine has, each with what it says of a box it rebuilt without the values
/// the tables above never write.
fn checkers() -> Vec<(Vec<String>, &'static str)> {
    let mut out = Vec::new();
    if ready(Need::Python, have_python, "python3 is not there") {
        out.push((vec!["python3".to_string(), root().join("tools/recheck.py").to_string_lossy().into_owned()], "without the values the tables above never write"));
    }
    if let Some(bin) = lean_checker() {
        out.push((vec![bin.to_string_lossy().into_owned()], "less the values no table above writes"));
    }
    out
}

fn tables(cert: &str) -> Vec<Json> {
    let j = rulec::json::parse(cert.trim()).unwrap();
    j.get("tables").and_then(Json::as_arr).unwrap().to_vec()
}

fn ints(j: &Json) -> Vec<i128> {
    j.as_arr().unwrap().iter().map(|x| x.as_int().unwrap()).collect()
}

/// A table's `never` facts, as (axis, coordinate).
fn nevers(t: &Json) -> Vec<(i128, i128)> {
    t.get("above").and_then(|a| a.get("never")).and_then(Json::as_arr).unwrap().iter().map(|f| (f.get("axis").unwrap().as_int().unwrap(), f.get("coord").unwrap().as_int().unwrap())).collect()
}

/// Each row's box, axis by axis.
fn boxes(t: &Json) -> Vec<Vec<Vec<i128>>> {
    t.get("rows").and_then(Json::as_arr).unwrap().iter().map(|r| r.get("accepts").unwrap().as_arr().unwrap().iter().map(ints).collect()).collect()
}

/// The cover with the rows and the names left out: the shape of the tree and the kind of each leaf.
fn shape(j: &Json) -> String {
    match j.get("split") {
        Some(kids) => format!("[{}]", kids.as_arr().unwrap().iter().map(shape).collect::<Vec<_>>().join(" ")),
        None => match j {
            Json::Obj(kv) => kv.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>().join("+"),
            _ => "?".to_string(),
        },
    }
}

/// Every material, English and Japanese, passes `check`, and its certificate passes both
/// re-checkers held to the rule's file: a box narrowed by a value no table above writes is read
/// back from its cell with that value left out. The rules that read a column decided above each
/// have such a box.
#[test]
fn every_material_passes_check_and_both_rechecks() {
    let checkers = checkers();
    let stems: Vec<(&str, bool)> = MATERIALS.iter().chain(&CALLEES).flat_map(|(en, ja)| [*en, *ja]).map(|s| (s, !CALLEES.iter().any(|(en, ja)| *en == s || *ja == s))).collect();
    for (stem, narrowed) in stems {
        let (code, said) = rulec(&["check", &rel(stem)]);
        assert_eq!(code, 0, "{stem} does not pass check:\n{said}");
        assert!(!said.contains("warning["), "{stem}: {said}");
        let cert = certificate(stem);
        for (program, told) in &checkers {
            let (code, said) = recheck(program, &cert, stem);
            assert_eq!(code, 0, "{stem} is refused by {}:\n{said}", program.last().unwrap());
            assert_eq!(said.contains(*told), narrowed, "{stem}: {}:\n{said}", program.last().unwrap());
        }
    }
}

/// Where the column decided above is the first of two axes, the boxes that fix it at basic or at
/// platinum are each one leaf on the tables above, and the certificate states both facts. Before,
/// the cover asked each point under them, and stated no fact at all.
#[test]
fn every_value_no_table_above_writes_is_stated() {
    for stem in ["lift_and_area", "引き上げた段階と地域"] {
        let ts = tables(&certificate(stem));
        let t = &ts[1];
        let first = t.get("axes").and_then(Json::as_arr).unwrap()[0].get("kind").and_then(Json::as_str).unwrap().to_string();
        assert_eq!(first, "upstream", "{stem}: the column decided above is not the first axis");
        assert_eq!(nevers(t), vec![(0, 0), (0, 3)], "{stem}");
        assert_eq!(shape(t.get("cover").unwrap()), "[upstream [row row row row row] [row row row row row] upstream]", "{stem}");
        assert_eq!(boxes(t), vec![vec![vec![1, 2], vec![0]], vec![vec![1, 2], vec![1, 2, 3, 4]]], "{stem}");
    }
}

/// The certificates of the English rule and its Japanese version say the same thing on every
/// table: the facts, the boxes, and the cover's shape.
#[test]
fn the_japanese_versions_state_the_same_boxes_and_facts() {
    for (en, ja) in MATERIALS.iter().chain(&CALLEES) {
        let (a, b) = (tables(&certificate(en)), tables(&certificate(ja)));
        assert_eq!(a.len(), b.len(), "{en} and {ja}");
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(nevers(x), nevers(y), "{en} and {ja}");
            assert_eq!(boxes(x), boxes(y), "{en} and {ja}");
            assert_eq!(shape(x.get("cover").unwrap()), shape(y.get("cover").unwrap()), "{en} and {ja}");
        }
    }
}

/// A box may leave out a coordinate only where a fact names it. Gold left out of the box of
/// `>=silver` — a value the table above writes — and the box with a fact taken away are each a
/// box narrower than its cell, and both re-checkers refuse them, as they refuse the box the cell
/// describes with platinum kept in it. A value the table above writes, called one it never
/// writes and left out of the box, is refused too.
#[test]
fn a_box_narrower_than_its_cells_is_refused() {
    let checkers = checkers();
    let cert = certificate("plain_lift");
    const BOX: &str = r#""accepts":[[1,2]]"#;
    const FACTS: &str = r#""never":[{"axis":0,"coord":0},{"axis":0,"coord":3}]"#;
    assert!(cert.matches(BOX).count() == 1 && cert.contains(FACTS), "the certificate changed its shape, and the lies below would tell nothing:\n{cert}");
    let lies = [
        ("gold left out of the box", cert.replace(BOX, r#""accepts":[[1]]"#)),
        ("the fact for platinum taken away", cert.replace(FACTS, r#""never":[{"axis":0,"coord":0}]"#)),
        ("platinum kept in the box", cert.replace(BOX, r#""accepts":[[1,2,3]]"#)),
        ("silver called a value the table above never writes", cert.replace(FACTS, r#""never":[{"axis":0,"coord":0},{"axis":0,"coord":1},{"axis":0,"coord":3}]"#).replace(BOX, r#""accepts":[[2]]"#)),
    ];
    for (program, _) in &checkers {
        let lean = !program[0].ends_with("python3");
        for (i, (what, forged)) in lies.iter().enumerate() {
            let (code, said) = recheck(program, forged, "plain_lift");
            assert_eq!(code, 1, "{what}: {} let it through:\n{said}", program.last().unwrap());
            let why = match (lean, i) {
                (false, 0 | 2) => "is not what its cell `>= silver` describes once the values the tables above never write are left out",
                (false, 1) => "is not what its cell `>= silver` describes",
                (true, 0..=2) => "the box it states is not the one its cell describes",
                (false, _) => "FAILED",
                (true, _) => "does write lift = silver, so it is not a value the tables above never reach",
            };
            assert!(said.contains(why), "{what}: {}:\n{said}", program.last().unwrap());
        }
    }
}
