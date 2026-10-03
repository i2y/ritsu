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
            r#"{"row":3,"at":[1,1],"values":{"全条件一致数":1,"会社名一致数":1},"at_values":["1","1"],"extra_values":[]}"#,
            r#"{"row":3,"at":[2,2],"values":{"全条件一致数":500,"会社名一致数":3},"at_values":["500","3"],"extra_values":[]}"#)),
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

/// `#print axioms` on the theorems every claim rests on shows only the three Lean itself
/// stands on — `propext`, `Classical.choice`, `Quot.sound`. A text search for `sorry` cannot
/// see a gap a tactic leaves, nor an `axiom` declared somewhere it does not look; this asks
/// Lean, which can.
#[test]
fn 定理が立つ公理は三つだけ() {
    if checker().is_none() {
        eprintln!("skip: proofs/ が build されていない");
        return;
    }
    let lake = std::env::var("HOME").map(|h| PathBuf::from(h).join(".elan/bin/lake")).ok().filter(|p| p.exists());
    let Some(lake) = lake.or_else(|| {
        Command::new("sh").args(["-c", "command -v lake"]).output().ok().filter(|o| o.status.success()).map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim().to_string()))
    }) else {
        eprintln!("skip: lake が無い");
        return;
    };
    const THEOREMS: &[&str] = &[
        "RulecCert.Certified.complete",
        "RulecCert.Certified.reached",
        "RulecCert.Certified.notExcluded",
        "RulecCert.Certified.disjoint",
        "RulecCert.Certified.disjointAsked",
        "RulecCert.Certified.unique",
        "RulecCert.farkas_sound",
        "RulecCert.not_asked_of_farkas",
        "RulecCert.included_sound",
        "RulecCert.admits_iff",
        "RulecCert.mem_boxOf_cmp_iff",
        "RulecCert.mem_boxOf_in_iff",
        "RulecCert.mem_boxOf_notIn_iff",
        "RulecCert.eval_type_of_typeOf",
        "RulecCert.eval_mem_interval",
        "RulecCert.runTotal_exact",
    ];
    let dir = std::env::temp_dir().join(format!("rulec-axioms-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("Axioms.lean");
    let mut src = String::from("import RulecCert\n");
    for t in THEOREMS {
        src.push_str(&format!("#print axioms {t}\n"));
    }
    std::fs::write(&file, src).unwrap();
    let o = Command::new(&lake)
        .current_dir(root().join("proofs"))
        .args(["env", "lean", &file.to_string_lossy()])
        .output()
        .expect("lake を起動できない");
    let said = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(o.status.success(), "{said}");
    let mut seen = 0;
    for line in said.lines() {
        if line.contains("does not depend on any axioms") {
            seen += 1;
        } else if let Some(rest) = line.split("depends on axioms: [").nth(1) {
            seen += 1;
            for ax in rest.trim_end_matches(']').split(',').map(str::trim) {
                assert!(
                    ["propext", "Classical.choice", "Quot.sound"].contains(&ax),
                    "{line}: 三つのほかに `{ax}` に立っている"
                );
            }
        }
    }
    assert_eq!(seen, THEOREMS.len(), "`#print axioms` の答えが揃っていない:\n{said}");
}

/// The same two rules through the proved checks: a box of the cover and a pair of rows the
/// linear model rules out are proved by `not_asked_of_farkas`, and a changed multiplier is
/// refused (§15.141).
#[test]
fn 線形のモデルの乗数は証明付きの検査器でも確かめられる() {
    let Some(bin) = checker() else {
        eprintln!("skip: proofs/ が build されていない");
        return;
    };
    let rules = [
        ("gap", "rule 連なる制約(chain) v1\n\ninputs\n  a(a) : money[円]  range >=0円 <=100円\n  b(b) : money[円]  range >=0円 <=100円\n  x(x) : money[円]  range >=0円 <=100円\n\nconstraint a <= b\nconstraint b <= x\n\noutputs\n  y(y) : bool\n\ntable 表(t)\npolicy unique\n| a      | x     | -> y  |\n| <=50円 | -     | true  |\n| >50円  | >50円 | false |\n"),
        ("overlap", "rule 連なる制約(chain) v1\n\ninputs\n  a(a) : money[円]  range >=0円 <=100円\n  b(b) : money[円]  range >=0円 <=100円\n  x(x) : money[円]  range >=0円 <=100円\n\nconstraint a <= b\nconstraint b <= x\n\noutputs\n  y(y) : bool\n\ntable 表(t)\npolicy unique\n| a      | x      | -> y  |\n| >50円  | -      | true  |\n| -      | <50円  | false |\n| <=50円 | >=50円 | false |\n"),
    ];
    let dir = std::env::temp_dir().join(format!("rulec-lean-chain-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (tag, src) in rules {
        let p = dir.join(format!("{tag}.rule"));
        std::fs::write(&p, src).unwrap();
        let (c, cert) = rulec(&["certificate", p.to_str().unwrap()]);
        assert_eq!(c, 0, "{cert}");
        let (code, said) = lean(&bin, &cert, Some(p.to_str().unwrap()));
        assert_eq!(code, 0, "{tag}: そのままの証明書が通らない:\n{said}");
        assert!(said.contains("OK: every claim"), "{tag}: {said}");
        if tag == "overlap" {
            assert!(said.contains("(1 by the linear model)"), "{said}");
        }
        let forged = cert.replacen(r#"{"fact":6,"y":"#, r#"{"fact":5,"y":"#, 1);
        assert_ne!(forged, cert, "{tag}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = lean(&bin, &forged, None);
        assert_eq!(code, 1, "{tag}: 別の事実を指した乗数が通ってしまった:\n{said}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Ways to lie about a contract, on the certificate of 速達の見積 (§15.142): each with
/// whether it has to fail, or be said out loud as not shown.
fn contract_lies(cert: &str) -> Vec<(&'static str, String, bool)> {
    let p = r#"{"farkas":[{"atom":9,"part":0,"y":"1"},{"door":true,"y":"1"}]}"#;
    let door = format!(r#"{{"range":"申告額","hi":false,"proofs":[{p},{p}]}},"#);
    assert!(cert.contains(&door), "証明書の形が変わっていて、偽れない:\n{cert}");
    let at = cert.find(r#""sha256":""#).unwrap() + r#""sha256":""#.len();
    let mut digest = cert.to_string();
    digest.replace_range(at..at + 1, if &cert[at..at + 1] == "0" { "1" } else { "0" });
    vec![
        ("乗数を変える", cert.replacen(p, &p.replacen(r#""door":true,"y":"1""#, r#""door":true,"y":"2""#, 1), 1), true),
        ("契約を広く読む", cert.replacen(r#"{"num":{"申告額":"-1"},"k":"0","rel":"le"}"#, r#"{"num":{"申告額":"-1"},"k":"-5","rel":"le"}"#, 1), true),
        ("別の条件を指す", cert.replacen(r#"{"atom":9,"part":0"#, r#"{"atom":10,"part":0"#, 1), true),
        ("場合を一つ欠く", cert.replacen(&door, &format!(r#"{{"range":"申告額","hi":false,"proofs":[{p}]}},"#), 1), true),
        ("食い違わない場合を食い違うと言う", cert.replacen(&door, &format!(r#"{{"range":"申告額","hi":false,"proofs":[{{"clash":"速達"}},{p}]}},"#), 1), true),
        ("値を渡す入力を値の一覧から外す", cert.replacen(r#""vars":{"num":["申告額","補償額","重さ"]"#, r#""vars":{"num":["補償額","重さ"]"#, 1), true),
        ("別の契約の本文", digest, true),
        ("入口の条件を一つ落とす", cert.replacen(&door, "", 1), false),
        ("証明を空にする", cert.replacen(&door, r#"{"range":"申告額","hi":false,"proofs":null},"#, 1), false),
    ]
}

/// A case whose truth values cannot both hold, added by hand with the proof that says so:
/// rulec leaves such cases out when it opens a condition, so no certificate of its own
/// carries one, and this is how the re-checker's reading of that proof is held to account.
fn with_a_clashing_case(cert: &str) -> String {
    let last = r#"{"num":{"補償額":"1"},"k":"-300000","rel":"le"}],"cases":["#;
    assert!(cert.contains(last), "証明書の形が変わっていて、場合を足せない:\n{cert}");
    cert.replacen(last, r#"{"num":{"補償額":"1"},"k":"-300000","rel":"le"},{"bool":"速達","value":true}],"cases":[[0,15],"#, 1)
        .replace(r#""proofs":["#, r#""proofs":[{"clash":"速達"},"#)
}

/// The same through the proved checks: what the door asks is proved to hold in every case
/// the contract lets through, by `included_sound`, and the same lies are refused (§15.142).
#[test]
fn 契約の関係は証明付きの検査器でも確かめられる() {
    let Some(bin) = checker() else {
        eprintln!("skip: proofs/ が build されていない");
        return;
    };
    let rule = "tests/corpus/速達の見積.rule";
    let (c, cert) = rulec(&["certificate", rule]);
    assert_eq!(c, 0, "{cert}");
    let (code, said) = lean(&bin, &cert, Some(rule));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("contract 見積: 7 of 7 things the door asks hold in all 2 cases"), "{said}");
    assert!(said.contains("the digest of contract 見積 is tests/corpus/contracts/quote.proto's"), "{said}");
    assert!(said.contains("OK: every claim"), "{said}");
    for (what, forged, fails) in contract_lies(&cert) {
        assert_ne!(forged, cert, "{what}: 偽れていない");
        let (code, said) = lean(&bin, &forged, Some(rule));
        if fails {
            assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった:\n{said}");
            assert!(said.contains("FAILED"), "{what}: 何が悪いか言っていない:\n{said}");
        } else {
            assert_eq!(code, 0, "{what}: {said}");
            assert!(
                said.contains("stated rather than proved: contract 見積: the low end of 申告額's range is not shown to hold"),
                "{what}: 示していない条件を言っていない:\n{said}"
            );
        }
    }
    let (code, said) = lean(&bin, &with_a_clashing_case(&cert), Some(rule));
    assert_eq!(code, 0, "食い違う場合の証明が通らない:\n{said}");
    assert!(said.contains("hold in all 3 cases"), "{said}");

    let rule = "tests/corpus/出荷の送料.rule";
    let (c, cert) = rulec(&["certificate", rule]);
    assert_eq!(c, 0, "{cert}");
    assert_eq!(lean(&bin, &cert, Some(rule)).0, 0);
    let wider = cert.replacen(r#""values":["honshu","hokkaido","okinawa"],"in":true"#, r#""values":["honshu","hokkaido","okinawa","kyushu"],"in":true"#, 1);
    assert_ne!(wider, cert);
    let (code, said) = lean(&bin, &wider, Some(rule));
    assert_eq!(code, 1, "列挙にない値を通す契約が通ってしまった:\n{said}");
}


/// The same set through the proved checks: the box is `boxOf (.inVals …)`, which
/// `mem_boxOf_in_iff` ties to the values, and a widened one is refused (§15.143).
#[test]
fn 数の集合の箱は証明付きの検査器でも組み直される() {
    let Some(bin) = checker() else {
        eprintln!("skip: proofs/ が build されていない");
        return;
    };
    let dir = std::env::temp_dir().join(format!("rulec-lean-set-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("pieces.rule");
    std::fs::write(&p, "rule 個数の割引(pieces) v1\n\ninputs\n  個数(n) : number  range >=1 <=500\n\noutputs\n  割引(off) : money[円]  round down(1円)\n\ntable 割引表(t)\npolicy first\n| 個数         | -> 割引 |\n| 100, 200     | 500円   |\n| not: 300, 400 | 100円   |\n| -            | 0円     |\n").unwrap();
    let (c, cert) = rulec(&["certificate", p.to_str().unwrap()]);
    assert_eq!(c, 0, "{cert}");
    let (code, said) = lean(&bin, &cert, Some(p.to_str().unwrap()));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("OK: every claim"), "{said}");
    let at = cert.find(r#""cell":"in""#).unwrap();
    let acc = cert[at..].find(r#""accepts":[["#).unwrap() + at + r#""accepts":[["#.len();
    let mut wide = cert.clone();
    wide.insert_str(acc, "0,");
    let (code, said) = lean(&bin, &wide, None);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(code, 1, "広げた箱が通ってしまった:\n{said}");
    assert!(said.contains("is not the one its cell describes"), "{said}");
}

/// A certificate of a shape other than the one the proofs are about is refused, not read
/// (§15.156). `tests/cert.rs` holds `tools/recheck.py` to the same.
#[test]
fn 知らない形式の版の証明書は証明付きの検査器でも断る() {
    let Some(bin) = checker() else {
        eprintln!("skip: proofs/ が build されていない");
        return;
    };
    let (c, cert) = rulec(&["certificate", "tests/corpus/健康保険料.rule"]);
    assert_eq!(c, 0, "{cert}");
    let later = cert.replacen(r#"{"v":1,"#, r#"{"v":2,"#, 1);
    assert_ne!(later, cert, "証明書が形式の版を名乗らない");
    let (code, said) = lean(&bin, &later, None);
    assert_eq!(code, 1, "版 2 の証明書を読んでしまった\n{said}");
    assert!(said.contains("format version"), "{said}");
    let unmarked = cert.replacen(r#"{"v":1,"#, "{", 1);
    assert_eq!(lean(&bin, &unmarked, None).0, 0, "v の無い証明書を版 1 として読まない");
}
