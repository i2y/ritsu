//! The certificate, re-checked by the Lean program that carries the proofs (§15.97).
//!
//! `proofs/` states the meaning of a rule, the checks a certificate has to pass, and the
//! theorems that say a `true` from each check settles the matching claim. `Main.lean` runs
//! exactly those check functions, so what it prints is the proofs applied to one document.
//! These tests hold the pair together: the corpus has to pass here as it does under
//! `tools/recheck.py` in `tests/cert.rs`, and a certificate that has been tampered with has
//! to fail in both. Nothing here builds Lean: CI does that, and without the binary the
//! tests say they skipped rather than passing quietly.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn checker() -> Option<PathBuf> {
    let p = root().join("proofs/.lake/build/bin/rulec-recheck");
    p.exists().then_some(p)
}

fn rulec(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

/// Feed one certificate to the Lean program, with the rule it is about.
fn lean(bin: &PathBuf, cert: &str, rule: Option<&str>) -> (i32, String) {
    use std::io::Write;
    use std::process::Stdio;
    let mut cmd = Command::new(bin);
    cmd.current_dir(root());
    if let Some(r) = rule {
        cmd.args(["--rule", r]);
    }
    let mut p = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Lean の検査器を起動できない");
    p.stdin.as_mut().unwrap().write_all(cert.as_bytes()).unwrap();
    let o = p.wait_with_output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr),
    )
}

/// Every rule in the corpus passes, held to its own text.
#[test]
fn コーパスは証明付きの検査器を通る() {
    let Some(bin) = checker() else {
        eprintln!("skip: proofs/ が build されていない（lake build で作る）");
        return;
    };
    let dir = root().join("tests/corpus");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rule"))
        .collect();
    files.sort();
    assert!(files.len() >= 30, "コーパスが減っている: {}", files.len());
    let (mut tables, mut cells) = (0usize, 0usize);
    for f in &files {
        let rel = format!("tests/corpus/{}", f.file_name().unwrap().to_string_lossy());
        let (c, cert) = rulec(&["certificate", &rel]);
        assert_eq!(c, 0, "{rel}: 証明書が出ない");
        let (code, said) = lean(&bin, &cert, Some(&rel));
        assert_eq!(code, 0, "{rel} が Lean の検査器を通らない:\n{said}");
        assert!(said.contains("the digest is"), "{rel}: ファイルに突き合わせていない:\n{said}");
        tables += said.matches(" rows — ").count();
        cells += said
            .split("and ")
            .nth(1)
            .and_then(|s| s.split(' ').next())
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
    }
    assert!(tables >= 40, "表の数が減っている: {tables}");
    assert!(cells >= 700, "ファイルから読み戻したセルが減っている: {cells}");
}

/// A certificate that has been tampered with has to fail here too. Each of these is a claim
/// with a theorem behind it, so a `true` that should not be one is the thing to catch.
#[test]
fn 偽った証明書は証明付きの検査器でも落ちる() {
    let Some(bin) = checker() else {
        eprintln!("skip: proofs/ が build されていない");
        return;
    };
    let (c, cert) = rulec(&["certificate", "tests/corpus/健康保険料.rule"]);
    assert_eq!(c, 0, "{cert}");
    assert_eq!(lean(&bin, &cert, None).0, 0, "そのままの証明書が通らない");

    for (what, forged) in [
        // A cover leaf that names a row which does not take the box (E101).
        ("覆う行を偽る", cert.replace(r#""cover":{"split":[{"row":1},{"row":1}"#, r#""cover":{"split":[{"row":1},{"row":9}"#)),
        // A split with a child removed: the children no longer tile the axis (E101).
        ("枝を一つ落とす", cert.replace(r#""cover":{"split":[{"row":1},"#, r#""cover":{"split":["#)),
        // An interval narrower than the expression really reaches (E108).
        ("区間を狭く言う", cert.replace(r#""interval":["0","173750"]"#, r#""interval":["0","2"]"#)),
        // A type that does not follow from the expression (E103).
        ("型を偽る", cert.replace(r#""name":"合算率","type":"rate""#, r#""name":"合算率","type":"money[円]""#)),
        // A box widened without touching the cell it was read from.
        ("箱を広げる", cert.replacen(r#""accepts":[[0,1]]"#, r#""accepts":[[0,1,2]]"#, 1)),
        // A coordinate removed from an axis: the gap under it is then covered by nothing.
        ("軸から座標を抜く", cert.replacen(r#""step":"1","prefixes":null,"bounds":[["0","0"],["0","63000"],"#, r#""step":"1","prefixes":null,"bounds":[["0","0"],"#, 1)
            .replacen(r#""coords":["0円","1円","63000円"]"#, r#""coords":["0円","63000円"]"#, 1)),
    ] {
        assert_ne!(forged, cert, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = lean(&bin, &forged, None);
        assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった\n{said}");
        assert!(said.contains("FAILED"), "{what}: 何が悪いか言っていない\n{said}");
    }

    // The box check has to be the one that catches a widened box, not a neighbour.
    let forged = cert.replacen(r#""accepts":[[0,1]]"#, r#""accepts":[[0,1,2]]"#, 1);
    let (_, said) = lean(&bin, &forged, None);
    assert!(
        said.contains("is not the one its cell describes"),
        "箱を広げる: セルから組み直す検査が落としていない:\n{said}"
    );
}

/// The leaf the proved checker used to be deliberately not about.
///
/// `Certified.coverChecks` used to carry `!C.cover.leansOnUpstream` as a premise: the
/// theorems said nothing about a cover resting on a table above. Since §15.115 the facts
/// those tables give are part of the sieve, the premise is gone, and such a box is settled
/// by `boxRuledOut` like any other — so what the theorems cover now includes it. The facts
/// themselves are recomputed from the rows of the tables that decide the columns, which is
/// what the four lies below are refused by.
#[test]
fn 上流由来の葉は_定理の中に入った() {
    let Some(bin) = checker() else {
        eprintln!("skip: proofs/ が build されていない");
        return;
    };
    let rel = "tests/corpus/二つの区分.rule";
    let (c, cert) = rulec(&["certificate", rel]);
    assert_eq!(c, 0, "{cert}");
    const FACT: &str = r#""a":{"axis":0,"coord":0},"b":{"axis":1,"coord":1},"input":"重量","spans":[["0","2"],["11","30"]]"#;
    assert!(cert.contains(FACT), "証明書の形が変わっています:\n{cert}");

    let forgeries: Vec<(&str, String)> = vec![
        // The leaf with no fact under it. Before §15.115 this was the way through.
        ("葉だけ書いて事実を消す", cert.replace(FACT, "")),
        // The pair moved onto two values that really do arrive together.
        (
            "起こる組を離れていると言う",
            cert.replace(
                FACT,
                r#""a":{"axis":0,"coord":1},"b":{"axis":1,"coord":0},"input":"重量","spans":[["3","30"],["0","10"]]"#,
            ),
        ),
        // The written span narrowed until two spans that meet look apart.
        (
            "範囲を狭く書いて離す",
            cert.replace(
                FACT,
                r#""a":{"axis":0,"coord":1},"b":{"axis":1,"coord":0},"input":"重量","spans":[["11","30"],["0","2"]]"#,
            ),
        ),
        // A value the table above does write, called one it never writes.
        (
            "never をでっち上げる",
            cert.replace(r#""above":{"never":[],"apart":[{"# , r#""above":{"never":[{"axis":0,"coord":0}],"apart":[{"#),
        ),
    ];
    let (code, said) = lean(&bin, &cert, Some(rel));
    assert_eq!(code, 0, "そのままの証明書が通らない:\n{said}");
    assert!(!said.contains("FAILED"), "正直な証明書が落ちている:\n{said}");
    assert!(
        !said.contains("rests on a table above"),
        "まだ「述べただけ」として数えられている:\n{said}"
    );

    for (what, forged) in forgeries {
        assert_ne!(forged, cert, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = lean(&bin, &forged, None);
        assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった\n{said}");
        assert!(said.contains("FAILED"), "{what}: 何が悪いか言っていない\n{said}");
    }
}

/// A share (§15.102). Its interval rests on a `constraint`, not on the declared ranges:
/// without one, `floor(T × C ÷ S)` is bounded by the product of two ranges and not by the
/// amount. So the re-checker has to refuse a certificate that keeps the tight interval and
/// drops the line that earns it — and it has to accept the honest one through a chain of
/// two constraints, which is how the corpus rule states it.
#[test]
fn 配分の区間は制約に乗っている() {
    let Some(bin) = checker() else {
        eprintln!("skip: proofs/ が build されていない");
        return;
    };
    let rel = "tests/corpus/比例配分.rule";
    let (c, cert) = rulec(&["certificate", rel]);
    assert_eq!(c, 0, "{cert}");
    assert!(cert.contains(r#""call":"allocate""#), "配分の式が出ていない");
    let (code, said) = lean(&bin, &cert, Some(rel));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("values: 4 typed, 4 held to int64"), "配分の値が検査されていない:
{said}");

    for (what, forged) in [
        // The guarantees dropped: the interval no longer follows from anything.
        ("制約を消す", cert.replace(
            r#""constraints":[{"left":"直前までの定価","op":"<=","right":"ここまでの定価"},{"left":"ここまでの定価","op":"<=","right":"定価合計"}]"#,
            r#""constraints":[]"#)),
        // The chain broken in the middle: 直前までの定価 <= 定価合計 no longer follows.
        ("鎖を切る", cert.replace(
            r#"{"left":"直前までの定価","op":"<=","right":"ここまでの定価"}"#,
            r#"{"left":"直前までの定価","op":"<=","right":"直前までの定価"}"#)),
        // A share claimed to reach less than it does.
        ("配分の区間を狭く言う", cert.replacen(r#""interval":["0","1000000"]"#, r#""interval":["0","2"]"#, 1)),
    ] {
        assert_ne!(forged, cert, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = lean(&bin, &forged, Some(rel));
        assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった\n{said}");
        assert!(said.contains("FAILED"), "{what}: 何が悪いか言っていない\n{said}");
    }
}

/// The leaves that say "no input reaches here" (§15.55, §15.98). The corpus has none, so
/// without this the whole sieve — `boxRuledOut`, `pointRuledOut`, `completions` and the
/// theorem over them — would never run in CI.
#[test]
fn 制約で閉じた葉も検査される() {
    let Some(bin) = checker() else {
        eprintln!("skip: proofs/ が build されていない");
        return;
    };
    let dir = std::env::temp_dir().join(format!("rulec-lean-con-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("con.rule");
    std::fs::write(&p, BY_CONSTRAINT).unwrap();
    let (c, cert) = rulec(&["certificate", p.to_str().unwrap()]);
    assert_eq!(c, 0, "{cert}");
    assert!(cert.contains(r#"{"constraint":0}"#), "制約で閉じた葉が出ていない:\n{cert}");
    let (code, said) = lean(&bin, &cert, Some(p.to_str().unwrap()));
    assert_eq!(code, 0, "{said}");

    for (what, forged) in [
        // The constraint turned round: the box it ruled out is then reachable.
        ("制約の向きを変える", cert.replace(r#""op":"<=""#, r#""op":">=""#)),
        // The point for row 3 moved into the open coordinate on both axes and given
        // values the constraint forbids. Only the constraint can catch this.
        ("制約を破る値を渡す", cert.replace(
            r#"{"row":3,"at":[1,1],"values":{"全条件一致数":1,"会社名一致数":1},"at_values":["1","1"]}"#,
            r#"{"row":3,"at":[2,2],"values":{"全条件一致数":500,"会社名一致数":3},"at_values":["500","3"]}"#)),
    ] {
        assert_ne!(forged, cert, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = lean(&bin, &forged, Some(p.to_str().unwrap()));
        assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった\n{said}");
        assert!(said.contains("FAILED"), "{what}: 何が悪いか言っていない\n{said}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A rule whose completeness rests on a `constraint` (§15.55), as `tests/cert.rs` has it.
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

/// The digest ties the certificate to one text, and the Lean program computes it itself.
#[test]
fn 証明付きの検査器は別のファイルを拒む() {
    let Some(bin) = checker() else {
        eprintln!("skip: proofs/ が build されていない");
        return;
    };
    let (c, cert) = rulec(&["certificate", "tests/corpus/印紙税.rule"]);
    assert_eq!(c, 0, "{cert}");
    let (code, said) = lean(&bin, &cert, Some("tests/corpus/送料.rule"));
    assert_eq!(code, 1, "別のファイルを指しても通ってしまった:\n{said}");
    assert!(said.contains("another text"), "{said}");
}

/// Nothing in the development is left open. A `sorry` anywhere would make every theorem
/// above it worth nothing, and it is the one thing a reader cannot see from the outside.
#[test]
fn 証明に穴が無い() {
    let dir = root().join("proofs");
    let mut files: Vec<PathBuf> = Vec::new();
    let mut stack = vec![dir.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() && p.file_name().is_some_and(|n| n != ".lake") {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "lean") {
                files.push(p);
            }
        }
    }
    assert!(files.len() >= 8, "proofs/ の .lean が少ない: {}", files.len());
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap();
        for bad in ["sorry", "axiom ", "@[implemented_by", "native_decide"] {
            assert!(
                !text.contains(bad),
                "{}: `{bad}` がある。証明が開いたままになる",
                f.display()
            );
        }
    }
}
