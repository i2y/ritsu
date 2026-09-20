//! The certificate (DESIGN §15.96) and the program that re-checks it.
//!
//! `rulec certificate` states all five things `check` proves, in evidence small enough to
//! hand over: the tree that tiles the input space, the axis on which each pair of rows of a
//! `unique` table parts, a point that reaches every row, the interval every computed value
//! is forced into, and the type it keeps. `tools/recheck.py` holds the certificate to those
//! claims and shares no code with the tool — so what is tested here is the pair. The corpus
//! has to pass, and a certificate that has been tampered with has to fail: a re-checker
//! that accepts anything proves nothing.

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rulec(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(root()).args(args).output().expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

fn have_python() -> bool {
    Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Feed one certificate to the re-checker. Returns (exit code, what it said).
fn recheck(cert: &str) -> (i32, String) {
    use std::io::Write;
    let mut p = Command::new("python3")
        .current_dir(root())
        .arg("tools/recheck.py")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("python3 を起動できない");
    p.stdin.as_mut().unwrap().write_all(cert.as_bytes()).unwrap();
    let o = p.wait_with_output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr),
    )
}

/// Every rule in the corpus states a certificate, and every certificate holds.
#[test]
fn コーパスの証明書は再検査を通る() {
    if !have_python() {
        eprintln!("skip: python3 が無い");
        return;
    }
    let dir = root().join("tests/corpus");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rule"))
        .collect();
    files.sort();
    assert!(files.len() >= 30, "コーパスが減っている: {}", files.len());
    let mut tables = 0usize;
    for f in &files {
        let rel = format!("tests/corpus/{}", f.file_name().unwrap().to_string_lossy());
        let (c, cert) = rulec(&["certificate", &rel]);
        assert_eq!(c, 0, "{rel}: 証明書が出ない");
        tables += cert.matches("\"table\":").count();
        let (code, said) = recheck(&cert);
        assert_eq!(code, 0, "{rel} の証明書が再検査を通らない:\n{said}");
    }
    assert!(tables >= 40, "表の数が減っている: {tables}");
}

/// The claim W114 could not settle is stated as undecided, not as proved.
#[test]
fn 決められなかった対は未決と書かれる() {
    let (c, cert) = rulec(&["certificate", "tests/corpus/クーポン併用.rule"]);
    assert_eq!(c, 0, "{cert}");
    assert!(cert.contains(r#""undecided":[{"a":1,"b":2}]"#), "W114 の対が未決として出ていない:\n{cert}");
    if have_python() {
        let (code, said) = recheck(&cert);
        assert_eq!(code, 0, "{said}");
        assert!(said.contains("2 pairs disjoint"), "{said}");
    }
}

/// A certificate that has been tampered with has to fail, four ways. Without this the
/// re-checker could be a program that prints ok.
#[test]
fn 偽った証明書は落ちる() {
    if !have_python() {
        eprintln!("skip: python3 が無い");
        return;
    }
    let (c, cert) = rulec(&["certificate", "tests/corpus/印紙税.rule"]);
    assert_eq!(c, 0, "{cert}");
    let (code, _) = recheck(&cert);
    assert_eq!(code, 0, "そのままの証明書が通らない");

    for (what, forged) in [
        // A pair that is neither proved apart nor listed as undecided.
        ("対を落とす", cert.replace(r#",{"a":22,"b":23,"axis":2}"#, "")),
        // A pair said to part on an axis where both rows take every coordinate.
        ("交わる軸を名指す", cert.replace(r#"{"a":1,"b":2,"axis":0}"#, r#"{"a":1,"b":2,"axis":1}"#)),
        // The last row's point given to the row above it, so row 23 is left with none.
        ("行の証人を落とす", cert.replace(r#"{"row":23,"at":[0,1,23]"#, r#"{"row":22,"at":[0,1,23]"#)),
        // A row whose point sits outside its own box.
        ("証人を箱の外へ", cert.replace(r#""row":3,"at":[0,0,2]"#, r#""row":3,"at":[0,0,0]"#)),
    ] {
        assert_ne!(forged, cert, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = recheck(&forged);
        assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった\n{said}");
        assert!(said.contains("FAILED"), "{what}: 何が悪いか言っていない\n{said}");
    }
}

/// The digest ties a certificate to one text. Pointed at another file, it has to refuse.
#[test]
fn 証明書はどのファイルのものかを言う() {
    if !have_python() {
        eprintln!("skip: python3 が無い");
        return;
    }
    let dir = std::env::temp_dir().join(format!("rulec-cert-sha-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("c.json");
    let (c, cert) = rulec(&["certificate", "tests/corpus/印紙税.rule"]);
    assert_eq!(c, 0, "{cert}");
    std::fs::write(&p, &cert).unwrap();
    let run = |rule: &str| {
        let o = Command::new("python3")
            .current_dir(root())
            .args(["tools/recheck.py", "--rule", rule, p.to_str().unwrap()])
            .output()
            .expect("python3 を起動できない");
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
    };
    let (code, said) = run("tests/corpus/印紙税.rule");
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("digest"), "{said}");
    let (code, said) = run("tests/corpus/送料.rule");
    assert_eq!(code, 1, "別のファイルを指しても通ってしまった:\n{said}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The other half of the tampering: the cover and the int64 claim, on a rule that has both
/// a deep cover and values with an interval.
#[test]
fn 偽った覆いとint64も落ちる() {
    if !have_python() {
        eprintln!("skip: python3 が無い");
        return;
    }
    let (c, cert) = rulec(&["certificate", "tests/corpus/健康保険料.rule"]);
    assert_eq!(c, 0, "{cert}");
    assert_eq!(recheck(&cert).0, 0, "そのままの証明書が通らない");

    for (what, forged) in [
        // A cover leaf that names a row which does not take the box.
        ("覆う行を偽る", cert.replace(r#""cover":{"split":[{"row":1},{"row":1}"#, r#""cover":{"split":[{"row":1},{"row":9}"#)),
        // A split with a child removed: the children no longer tile the axis.
        ("枝を一つ落とす", cert.replace(r#""cover":{"split":[{"row":1},"#, r#""cover":{"split":["#)),
        // An interval narrower than the expression really reaches.
        ("区間を狭く言う", cert.replace(r#""interval":["0","173750"]"#, r#""interval":["0","2"]"#)),
        // The stored integer said to fit where it does not.
        ("刻みを偽る", cert.replace(r#""scale":20000"#, r#""scale":200000000000000"#)),
        // A box widened without touching the cell it was read from.
        ("箱を広げる", cert.replacen(r#""accepts":[[0,1]]"#, r#""accepts":[[0,1,2]]"#, 1)),
        // A type that does not follow from the expression.
        ("型を偽る", cert.replace(r#""name":"合算率","type":"rate""#, r#""name":"合算率","type":"money[円]""#)),
    ] {
        assert_ne!(forged, cert, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = recheck(&forged);
        assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった\n{said}");
        assert!(said.contains("FAILED"), "{what}: 何が悪いか言っていない\n{said}");
    }
}

/// A rule whose completeness rests on a `constraint`: without it the table has a gap, and
/// with it the gap is a box no input reaches (§15.55). The cover has to say so, and the
/// re-checker has to redo that step rather than take it.
const BY_CONSTRAINT: &str = r#"rule 納入先判定(dest_of) v1
description "絞った件数から、自動で確定してよいかを決める"

enum 判定(verdict) = 自動確定(auto) | 候補複数(many) | 該当なし(none)

inputs
  会社名一致数(name_hits) : number  range >=0 <=999
  全条件一致数(all_hits)  : number  range >=0 <=999

constraint 全条件一致数 <= 会社名一致数

outputs
  結果(verdict) : 判定

table 判定表(verdict_of)
policy unique
| 全条件一致数 | 会社名一致数 | -> 結果(verdict) : 判定 |
| 0            | 0            | 該当なし                |
| 0            | >=1          | 候補複数                |
| >=1          | >=1          | 自動確定                |

examples
| 全条件一致数 | 会社名一致数 | -> 結果 |
| 1            | 3            | 自動確定 |
"#;

#[test]
fn 制約で閉じた穴は証明書に出て_再検査される() {
    if !have_python() {
        eprintln!("skip: python3 が無い");
        return;
    }
    let dir = std::env::temp_dir().join(format!("rulec-cert-con-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("con.rule");
    std::fs::write(&p, BY_CONSTRAINT).unwrap();
    let (c, cert) = rulec(&["certificate", p.to_str().unwrap()]);
    assert_eq!(c, 0, "{cert}");
    assert!(cert.contains(r#"{"constraint":0}"#), "制約で閉じた葉が出ていない:
{cert}");
    let (code, said) = recheck(&cert);
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("impossible by a constraint"), "再検査が制約の葉を数えていない:
{said}");

    // The same certificate with the constraint's sense turned round: the box it rules out
    // is then reachable, and the re-checker has to refuse the leaf.
    let forged = cert.replace(r#""op":"<=""#, r#""op":">=""#);
    assert_ne!(forged, cert);
    let (code, said) = recheck(&forged);
    assert_eq!(code, 1, "向きを変えた制約が通ってしまった:
{said}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// What the certificate does **not** cover is said out loud, because a certificate that
/// looks complete is worse than one that names its edges (§15.47).
#[test]
fn 証明書は自分の届く先を言う() {
    let doc = std::fs::read_to_string(root().join("docs/formats.md")).unwrap();
    let cut = doc.find("## `certificate`").expect("formats.md に certificate の節が無い");
    let sect = &doc[cut..doc[cut + 4..].find("\n## ").map(|i| cut + 4 + i).unwrap_or(doc.len())];
    let low = sect.to_lowercase();
    for want in ["completeness", "int64", "recheck.py", "undecided", "does not pass `check`", "units", "--rule"] {
        assert!(low.contains(want), "formats.md の certificate の節が `{want}` を言っていない");
    }
}
