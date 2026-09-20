//! `rulec import xlsx` (§15.50): the workbook the business sends, read as it is.
//!
//! What is tested is the reading — the container, the strings, and the three places where a
//! spreadsheet is not a CSV (a date is a number, a percentage is a hundredth, a unit lives
//! in the number format) — and the property that matters most: **the same table read from
//! an xlsx and from a CSV drafts the same rule**. The drafting itself is `tests/import.rs`.
//!
//! The fixtures are written by `tests/xlsxbuild.py` (python3, standard library only), so
//! there is no binary in the repository and the ZIP entries come out of a real deflate.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn run(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rulec-xlsx-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Write a workbook from the spec and return its path.
fn book(d: &PathBuf, name: &str, spec: &str) -> String {
    let out = d.join(name);
    let spec = spec.replace("OUT", out.to_str().unwrap());
    let o = Command::new("python3")
        .arg(root().join("tests/xlsxbuild.py"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut c| {
            use std::io::Write;
            c.stdin.take().unwrap().write_all(spec.as_bytes())?;
            c.wait_with_output()
        })
        .expect("python3 を起動できない");
    assert!(o.status.success(), "xlsx を作れない: {}", String::from_utf8_lossy(&o.stderr));
    out.to_str().unwrap().to_string()
}

/// The workbook the other tests read: a sheet of prose first, then the table, and the table
/// itself starting at C3 rather than at A1 — which is where a table actually sits.
const FEE: &str = r#"{"out":"OUT","compress":"COMP","sheets":[
 {"name":"説明","rows":[[["s","このシートは説明です"]]]},
 {"name":"運賃表","first_row":3,"first_col":3,"rows":[
  [["s","あて先"],["s","サイズ"],["s","改定日"],["s","割引率"],["s","運賃"]],
  [["s","近畿圏"],["inline","S60"],["date","2026-04-01"],["pct","0.05"],["yen","990"]],
  [["s","近畿圏"],["inline","S80"],["jdate","2026-04-01"],["pct","0.183"],["yen","1310"]],
  [["s","遠隔地"],["inline","S60"],["date","2026-04-01"],["pct","0.05"],["yen","880"]],
  [["s","遠隔地"],["inline","S80"],["date","2026-04-01"],["pct","0.183"],["yen","1200"]]]}]}"#;

#[test]
fn シートは表になり_日付と率と単位が読める() {
    if !have("python3") {
        eprintln!("skip: python3 が無い");
        return;
    }
    let d = dir("fee");
    let x = book(&d, "運賃.xlsx", &FEE.replace("COMP", "deflate"));
    let (c, out, e) = run(&["import", "xlsx", &x, "--sheet", "運賃表", "--lang", "ja"]);
    assert_eq!(c, 0, "{e}");
    // The strings, with the reading Excel keeps beside them left out.
    assert!(out.contains("enum あて先の値(c1_kind) = 近畿圏(v1) | 遠隔地(v2)"), "{out}");
    assert!(!out.contains("ヨミ"), "rPh（ふりがな）を値に混ぜてはいけない: {out}");
    // A serial number under a date format is a date, in both spellings of the format.
    assert!(out.contains("  改定日(c3) : date  range >=2026-04-01 <=2026-04-01"), "{out}");
    // A percentage is stored as its fraction; 0.183 is 18.3%, exactly.
    assert!(out.contains("| 近畿圏 | S80 | 2026-04-01 | 18.3% | 1310円 |"), "{out}");
    assert!(out.contains("  割引率(c4) : rate[step 0.1%]  range >=5% <=18.3%"), "{out}");
    // The unit is in the number format (`#,##0"円"`), not in the cell.
    assert!(out.contains("  運賃(o1) : money[円, incl_tax]"), "{out}");
    // Four rows over 2 × 2 × 1 × 2 values is not complete, but it parses and formats.
    let rule = d.join("運賃.rule");
    std::fs::write(&rule, &out).unwrap();
    let (c, o, e) = run(&["fmt", rule.to_str().unwrap()]);
    assert_eq!(c, 0, "{o}{e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn xlsxとcsvは同じ下書きになる() {
    if !have("python3") {
        eprintln!("skip: python3 が無い");
        return;
    }
    let d = dir("same");
    let x = book(&d, "t.xlsx", &FEE.replace("COMP", "deflate"));
    let (_, from_xlsx, _) = run(&["import", "xlsx", &x, "--sheet", "運賃表", "--lang", "ja"]);
    // The same grid, written out as a CSV by hand: the reader is what differs, and nothing
    // below it may.
    let csv = d.join("t.csv");
    std::fs::write(
        &csv,
        "あて先,サイズ,改定日,割引率,運賃\n\
         近畿圏,S60,2026-04-01,5%,990円\n\
         近畿圏,S80,2026-04-01,18.3%,1310円\n\
         遠隔地,S60,2026-04-01,5%,880円\n\
         遠隔地,S80,2026-04-01,18.3%,1200円\n",
    )
    .unwrap();
    let (_, from_csv, _) = run(&["import", "csv", csv.to_str().unwrap(), "--lang", "ja"]);
    // Only the name of the source differs, and the draft is supposed to say which it was.
    let norm = |s: String| s.replace("t.xlsx（シート 運賃表）", "SOURCE").replace("t.csv", "SOURCE");
    assert_eq!(norm(from_xlsx), norm(from_csv), "xlsx と csv で下書きが違う");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn 無圧縮のzipも読める() {
    if !have("python3") {
        eprintln!("skip: python3 が無い");
        return;
    }
    let d = dir("stored");
    let a = book(&d, "d.xlsx", &FEE.replace("COMP", "deflate"));
    let b = book(&d, "s.xlsx", &FEE.replace("COMP", "stored"));
    let (c1, one, _) = run(&["import", "xlsx", &a, "--sheet", "運賃表", "--name", "運賃"]);
    let (c2, two, _) = run(&["import", "xlsx", &b, "--sheet", "運賃表", "--name", "運賃"]);
    assert_eq!((c1, c2), (0, 0));
    assert_eq!(one.replace("d.xlsx", "X"), two.replace("s.xlsx", "X"));
    let _ = std::fs::remove_dir_all(&d);
}

/// A sheet long enough that deflate uses dynamic Huffman codes and back-references that
/// reach across the window — the part of the reader a five-row fixture never exercises.
#[test]
fn 長いシートも読める() {
    if !have("python3") {
        eprintln!("skip: python3 が無い");
        return;
    }
    let d = dir("long");
    let mut rows = String::from(r#"[[["s","区分"],["s","下限"],["s","運賃"]]"#);
    for i in 0..400 {
        rows.push_str(&format!(r#",[["s","区分{}"],["n","{}"],["yen","{}"]]"#, i % 7, i * 10, 800 + i));
    }
    rows.push(']');
    let spec = format!(r#"{{"out":"OUT","compress":"deflate","sheets":[{{"name":"表","rows":{rows}}}]}}"#);
    let x = book(&d, "long.xlsx", &spec);
    let (c, out, e) = run(&["import", "xlsx", &x]);
    assert_eq!(c, 0, "{e}");
    let data = out
        .lines()
        .filter(|l| {
            l.strip_prefix("| 区分").is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit()))
        })
        .count();
    assert_eq!(data, 400, "行が落ちている");
    assert!(out.contains("| 区分0 | 3990 | 1199円 |"), "最後の行まで読めていない: {out}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn シートの選び方() {
    if !have("python3") {
        eprintln!("skip: python3 が無い");
        return;
    }
    let d = dir("sheets");
    let x = book(&d, "b.xlsx", &FEE.replace("COMP", "deflate"));
    // Without --sheet it is the first one, which here is the sheet of prose: one column, so
    // it is not a table, and the refusal says so rather than drafting nonsense.
    let (c, _, e) = run(&["import", "xlsx", &x]);
    assert_eq!(c, 2, "{e}");
    // A name that is not there lists the ones that are.
    let (c, _, e) = run(&["import", "xlsx", &x, "--sheet", "料金"]);
    assert_eq!(c, 2);
    assert!(e.contains("説明") && e.contains("運賃表"), "あるシートを挙げること: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn 一九〇四年のブックも読める() {
    if !have("python3") {
        eprintln!("skip: python3 が無い");
        return;
    }
    let d = dir("1904");
    let spec = r#"{"out":"OUT","compress":"deflate","date1904":true,"sheets":[{"name":"日付","rows":[
      [["s","区分"],["s","日"]],
      [["s","A"],["date1904","2026-04-01"]],
      [["s","B"],["date1904","2026-05-01"]]]}]}"#;
    let x = book(&d, "d1904.xlsx", spec);
    let (c, out, e) = run(&["import", "xlsx", &x]);
    assert_eq!(c, 0, "{e}");
    assert!(out.contains("| B | 2026-05-01 |"), "1904 年起点の通し番号: {out}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn xlsxでないファイルは断る() {
    let d = dir("bad");
    let p = d.join("not.xlsx");
    std::fs::write(&p, b"this is not a zip at all").unwrap();
    let (c, _, e) = run(&["import", "xlsx", p.to_str().unwrap()]);
    assert_eq!(c, 2);
    assert!(e.contains("ZIP") || e.contains("zip"), "{e}");
    // A ZIP that is not a workbook.
    let z = d.join("plain.zip");
    if have("python3") {
        let o = Command::new("python3")
            .args([
                "-c",
                &format!(
                    "import zipfile; zipfile.ZipFile(r'{}','w').writestr('a.txt','hello')",
                    z.display()
                ),
            ])
            .output()
            .unwrap();
        assert!(o.status.success());
        let (c, _, e) = run(&["import", "xlsx", z.to_str().unwrap()]);
        assert_eq!(c, 2);
        assert!(e.contains("workbook.xml"), "{e}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// The fixtures above are written by this repository; a workbook written by a library that
/// people actually use is the other half of the evidence. Skipped where `uv` is not there.
#[test]
fn 実物の道具が書いたブックも読める() {
    if !have("uv") {
        eprintln!("skip: uv が無い（openpyxl を一時的に呼べない）");
        return;
    }
    let d = dir("openpyxl");
    let py = d.join("make.py");
    std::fs::write(
        &py,
        r#"
from openpyxl import Workbook
import datetime, sys
wb = Workbook()
ws = wb.active
ws.title = "運賃表"
ws.append(["あて先", "改定日", "割引率", "運賃"])
ws.append(["近畿圏", datetime.date(2026, 4, 1), 0.05, 990])
ws.append(["遠隔地", datetime.date(2026, 4, 1), 0.183, 1200])
for row in ws.iter_rows(min_row=2, min_col=2, max_col=2):
    for c in row:
        c.number_format = "yyyy/mm/dd"
for row in ws.iter_rows(min_row=2, min_col=3, max_col=3):
    for c in row:
        c.number_format = "0.0%"
for row in ws.iter_rows(min_row=2, min_col=4, max_col=4):
    for c in row:
        c.number_format = '#,##0"円"'
wb.save(sys.argv[1])
"#,
    )
    .unwrap();
    let x = d.join("openpyxl.xlsx");
    let o = Command::new("uv")
        .args(["run", "--quiet", "--with", "openpyxl", "python"])
        .arg(&py)
        .arg(&x)
        .output()
        .expect("uv を起動できない");
    if !o.status.success() {
        eprintln!("skip: openpyxl を取れない: {}", String::from_utf8_lossy(&o.stderr));
        return;
    }
    let (c, out, e) = run(&["import", "xlsx", x.to_str().unwrap(), "--lang", "ja"]);
    assert_eq!(c, 0, "{e}");
    assert!(out.contains("| 遠隔地 | 2026-04-01 | 18.3% | 1200円 |"), "{out}");
    let _ = std::fs::remove_dir_all(&d);
}

/// A workbook's fragments are its sheets, in the order the workbook lists them, so that
/// `@料金表 表2` names the second sheet however much prose the first one holds (§15.82).
#[test]
fn ブックの引用箇所はシートで_順番はシートの順番() {
    if !have("python3") {
        eprintln!("skip: python3 が無い");
        return;
    }
    let d = dir("fragments");
    let x = book(&d, "運賃.xlsx", &FEE.replace("COMP", "deflate"));
    let p = std::path::PathBuf::from(&x);
    let bytes = std::fs::read(&p).unwrap();
    let ts = rulec::extract::tables(&p, &bytes).unwrap();
    assert_eq!(ts.len(), 2, "a sheet is a table, prose and all");
    assert_eq!(ts[0], vec![vec!["このシートは説明です"]]);
    assert_eq!(ts[1][0], vec!["あて先", "サイズ", "改定日", "割引率", "運賃"]);
    assert_eq!(ts[1][2], vec!["近畿圏", "S80", "2026-04-01", "18.3%", "1310円"]);
    let _ = std::fs::remove_dir_all(&d);
}
