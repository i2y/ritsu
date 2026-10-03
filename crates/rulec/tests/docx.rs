//! Reading a `.docx` (§15.82, third stage): the tables of a Word document, out of the file a
//! word processor writes rather than out of a hand-made XML string.
//!
//! The fixtures are written by `tests/docxbuild.py` (python3, standard library only), so the
//! container really is a ZIP and the markup really carries the `w:` prefix. What the unit
//! tests beside the reader cannot show — that the whole loop works on a Word document — is
//! shown here: a rule cites `表1` of a `.docx`, and `fetch`, `pin` and `check` carry it.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().is_ok()
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rulec-docx-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Write a document from the spec and return its path.
fn docx(d: &PathBuf, name: &str, spec: &str) -> PathBuf {
    let out = d.join(name);
    let spec = spec.replace("OUT", out.to_str().unwrap());
    let o = Command::new("python3")
        .arg(root().join("tests/docxbuild.py"))
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
    assert!(o.status.success(), "docx を作れない: {}", String::from_utf8_lossy(&o.stderr));
    out
}

fn rulec(d: &PathBuf, args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(d).args(args).output().expect("rulec を起動できない");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr),
    )
}

/// A document of the shape a tariff really has: prose, then a table whose header spans two
/// columns and whose first column is merged down the rows it applies to.
const TARIFF: &str = r#"{"out":"OUT","blocks":[
 {"kind":"p","text":"配送料金表（2026年4月1日改定）"},
 {"kind":"table","rows":[
  [{"text":"あて先"},{"text":"運賃","span":2}],
  ["あて先","S60","S80"],
  [{"text":"近畿","vmerge":"restart"},"990円","1210円"],
  [{"text":"","vmerge":"continue"},"1050円","1270円"],
  ["関東","880円","1100円"]]},
 {"kind":"p","text":"※ 離島は別に定める。"},
 {"kind":"table","rows":[["区分","割増"],["離島","500円"]]}]}"#;

#[test]
fn ワードの表を読む() {
    if !have("python3") {
        eprintln!("skip: python3 が無い");
        return;
    }
    let d = dir("read");
    let p = docx(&d, "料金表.docx", TARIFF);
    let bytes = std::fs::read(&p).unwrap();
    let ts = rulec::extract::tables(&p, &bytes).unwrap();
    assert_eq!(ts.len(), 2, "本文の段落は表ではない");
    // The spanning header leaves the column it took empty; nothing is filled in.
    assert_eq!(ts[0][0], vec!["あて先", "運賃", ""]);
    assert_eq!(ts[0][1], vec!["あて先", "S60", "S80"]);
    assert_eq!(ts[0][2], vec!["近畿", "990円", "1210円"]);
    // The continuation of a vertical merge is empty, as the document has it.
    assert_eq!(ts[0][3], vec!["", "1050円", "1270円"]);
    assert_eq!(ts[1], vec![vec!["区分", "割増"], vec!["離島", "500円"]]);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn ワードの表を引いて写しにする() {
    if !have("python3") {
        eprintln!("skip: python3 が無い");
        return;
    }
    let d = dir("cite");
    docx(&d, "料金表.docx", TARIFF);
    let rule = "rule t(t) v1\n\nsource 料金表 = file \"料金表.docx\"\n\nenum あて先(dest) = 近畿(kinki) | 関東(kanto)\n\n\
                inputs\n  あて先(dest) : あて先\n\noutputs\n  運賃(fee) : money[円]  round up(10円)\n\n\
                table 運賃表(fee_table)  @料金表 表2\npolicy unique\n| あて先 | -> 運賃 |\n| 近畿 | 500円 |\n| 関東 | 500円 |\n";
    std::fs::write(d.join("a.rule"), rule).unwrap();
    let (c, out) = rulec(&d, &["source", "fetch", "a.rule"]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains("表2"), "{out}");
    let tsv = std::fs::read_to_string(d.join("料金表.docx.fragments/表2.tsv")).unwrap();
    assert_eq!(tsv, "区分\t割増\n離島\t500円\n", "the second table of the document, as it stands");
    let (c, out) = rulec(&d, &["source", "pin", "a.rule"]);
    assert_eq!(c, 0, "{out}");
    let (c, out) = rulec(&d, &["check", "a.rule"]);
    assert_eq!(c, 0, "{out}");

    // And the amounts are held to it, as they are for every other format.
    let after = std::fs::read_to_string(d.join("a.rule")).unwrap();
    let typo = after.replace("| 近畿 | 500円 |", "| 近畿 | 600円 |");
    std::fs::write(d.join("a.rule"), typo).unwrap();
    let (c, out) = rulec(&d, &["check", "a.rule"]);
    assert_eq!(c, 1, "{out}");
    assert!(out.contains("E116"), "{out}");
    let _ = std::fs::remove_dir_all(&d);
}
