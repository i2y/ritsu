//! The certificate, re-checked by the Lean program that carries the proofs (§15.97).
//!
//! `RulecCert`, in ritsu's `proofs/` at the root of the repository (ritsu's DESIGN 11.4), states
//! the meaning of a rule, the checks a certificate has to pass, and the theorems that say a `true`
//! from each check settles the matching claim. `RulecMain.lean` there runs exactly those check
//! functions, so what it prints is the proofs applied to one document.
//! These tests hold the pair together: the corpus has to pass here as it does under
//! `tools/recheck.py` in `tests/cert.rs`, and a certificate that has been tampered with has
//! to fail in both. Nothing here builds Lean: CI does that, and without the binary the
//! tests say they skipped rather than passing quietly.

use std::path::PathBuf;
use std::process::Command;
use ritsu_testkit::{Need, TempDir, ready, skip};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// ritsu's Lean package, which holds `RulecCert` beside the other libraries of the Lean layer.
fn proofs() -> PathBuf {
    root().join("../../proofs")
}

/// The Lean program, when the level lets the tests run it (ritsu's DESIGN 10.2) and it has been
/// built (`lake build` in ritsu's `proofs/`); a SKIP line says which it is not.
fn checker() -> Option<PathBuf> {
    let p = proofs().join(".lake/build/bin/rulec-recheck");
    ready(Need::Lean, || p.exists(), "ritsu の proofs/ が build されていない（そこで lake build で作る）").then_some(p)
}

fn rulec(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "ja")
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
        ("型を偽る", cert.replace(r#""name":"合算率","of":"derive","type":"rate""#, r#""name":"合算率","of":"derive","type":"money[円]""#)),
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
        return;
    };
    let tmp = TempDir::new("lean-con");
    let dir = tmp.path().to_path_buf();
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
        return;
    };
    let (c, cert) = rulec(&["certificate", "tests/corpus/印紙税.rule"]);
    assert_eq!(c, 0, "{cert}");
    let (code, said) = lean(&bin, &cert, Some("tests/corpus/送料.rule"));
    assert_eq!(code, 1, "別のファイルを指しても通ってしまった:\n{said}");
    assert!(said.contains("another text"), "{said}");
}

/// Nothing in the development is left open. A `sorry` anywhere would make every theorem
/// above it worth nothing, and it is the one thing a reader cannot see from the outside. The
/// package holds every library of ritsu's Lean layer, and every file of it is read: a theorem
/// may stand on another library's (`RitsuCross` stands on `RulecCert`).
#[test]
fn 証明に穴が無い() {
    let dir = proofs();
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
        return;
    }
    let lake = std::env::var("HOME").map(|h| PathBuf::from(h).join(".elan/bin/lake")).ok().filter(|p| p.exists());
    let Some(lake) = lake.or_else(|| {
        Command::new("sh").args(["-c", "command -v lake"]).output().ok().filter(|o| o.status.success()).map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim().to_string()))
    }) else {
        skip("lake が無い");
        return;
    };
    const THEOREMS: &[&str] = &[
        "RulecCert.Certified.complete",
        "RulecCert.Certified.reached",
        "RulecCert.Certified.notExcluded",
        "RulecCert.Certified.disjoint",
        "RulecCert.Certified.disjointAsked",
        "RulecCert.Certified.unique",
        "RulecCert.inBox_dropNever",
        "RulecCert.Certified.cells_complete",
        "RulecCert.Certified.cells_unique",
        "RulecCert.Certified.cells_disjointAsked",
        "RulecCert.Certified.cells_reached",
        "RulecCert.Certified.cells_notExcluded",
        "RulecCert.farkas_sound",
        "RulecCert.not_asked_of_farkas",
        "RulecCert.not_asked_of_derived",
        "RulecCert.not_asked_of_aboveRuledOut",
        "RulecCert.not_asked_of_truth",
        "RulecCert.not_asked_of_abovePart",
        "RulecCert.asked_of_witnessOk",
        "RulecCert.eval_mem_hull",
        "RulecCert.eval_mem_stated",
        "RulecCert.included_sound",
        "RulecCert.admits_iff",
        "RulecCert.mem_boxOf_cmp_iff",
        "RulecCert.mem_boxOf_in_iff",
        "RulecCert.mem_boxOf_notIn_iff",
        "RulecCert.mem_boxOf_absent_iff",
        "RulecCert.eval_type_of_typeOf",
        "RulecCert.eval_mem_interval",
        "RulecCert.runTotal_exact",
        "RulecCert.cut_of_box",
        "RulecCert.written_mem_hull",
        "RulecCert.enumAxis_coords",
        "RulecCert.Carried.Model.simulates",
        "RulecCert.Carried.Sys.reaches_mem",
        "RulecCert.Carried.Sys.final_stays",
        "RulecCert.Carried.Sys.never_after",
        "RulecCert.Carried.Sys.once_below_two",
        "RulecCert.Carried.Model.always_holds",
        "RulecCert.Carried.Model.range_holds",
        "RulecCert.Carried.Model.start_inhabited",
        "RulecCert.extreme_mem_stated",
        "RulecCert.foldAnswer_mem_stated",
        "RulecCert.rounds_onGrid",
        "RulecCert.evalExact_sound",
        "RulecCert.Prereq.no_cycle",
        "RulecCert.Prereq.reached_holds",
        "RulecCert.Prereq.never_held",
        "RulecCert.Prereq.order_along",
        "RulecCert.Carried.Model.call_of_fires",
        "RulecCert.Carried.Model.fires_of_callOk",
        "RulecCert.Carried.Model.finishes_of_pathOk",
        "RulecCert.Carried.Model.reachable_finishes",
        "RulecCert.axisTilesB_cover",
        "RulecCert.axisTilesB_unique",
        "RulecCert.extreme_on_axis",
        "RulecCert.foldAnswer_on_axis",
        "RulecCert.Certified.answers_at",
        "RulecCert.openValue_mem_stated",
        "RulecCert.Chances.countP_cart_flatMap",
        "RulecCert.Chances.chain_flatMap",
        "RulecCert.Chances.tiles_flatMap",
        "RulecCert.Chances.mem_grid_num",
        "RulecCert.Chances.holds_iff_inRun",
        "RulecCert.Chances.nearOf_sound",
        "RulecCert.Chances.cellOf_sound",
        "RulecCert.Chances.CCell.inside_iff",
        "RulecCert.Chances.FAx.cover",
        "RulecCert.Chances.Claim.box_const",
        "RulecCert.Chances.Claim.count_share",
        "RulecCert.Chances.Claim.holds",
        "RulecCert.Chances.envExact_sound",
        "RulecCert.Chances.RunClaim.pointHit_sound",
        "RulecCert.Chances.RunClaim.holds",
        "RulecCert.Chances.WeightedClaim.pointHit_sound",
        "RulecCert.Chances.WeightedClaim.holds",
    ];
    let tmp = TempDir::new("axioms");
    let dir = tmp.path().to_path_buf();
    let file = dir.join("Axioms.lean");
    let mut src = String::from("import RulecCert\n");
    for t in THEOREMS {
        src.push_str(&format!("#print axioms {t}\n"));
    }
    std::fs::write(&file, src).unwrap();
    let o = Command::new(&lake)
        .current_dir(proofs())
        .args(["env", "lean", &file.to_string_lossy()])
        .output()
        .expect("lake を起動できない");
    let said = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
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
        return;
    };
    let rules = [
        ("gap", "rule 連なる制約(chain) v1\n\ninputs\n  a(a) : money[円]  range >=0円 <=100円\n  b(b) : money[円]  range >=0円 <=100円\n  x(x) : money[円]  range >=0円 <=100円\n\nconstraint a <= b\nconstraint b <= x\n\noutputs\n  y(y) : bool\n\ntable 表(t)\npolicy unique\n| a      | x     | -> y  |\n| <=50円 | -     | true  |\n| >50円  | >50円 | false |\n"),
        ("overlap", "rule 連なる制約(chain) v1\n\ninputs\n  a(a) : money[円]  range >=0円 <=100円\n  b(b) : money[円]  range >=0円 <=100円\n  x(x) : money[円]  range >=0円 <=100円\n\nconstraint a <= b\nconstraint b <= x\n\noutputs\n  y(y) : bool\n\ntable 表(t)\npolicy unique\n| a      | x      | -> y  |\n| >50円  | -      | true  |\n| -      | <50円  | false |\n| <=50円 | >=50円 | false |\n"),
    ];
    let tmp = TempDir::new("lean-chain");
    let dir = tmp.path().to_path_buf();
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
}

/// A commission of a tenth of the amount, in two rows that meet at 1GBP.
const TENTHS: &str = "rule commission_band v1\n\ninputs\n  amount : money[GBP]  range >=1GBP <=100GBP\n\noutputs\n  reviewed : bool\n\nderive commission : money[GBP] = amount * 10%  range >=0GBP <=10GBP\n\ntable review\npolicy unique\n| commission | -> reviewed : bool |\n| <=1GBP     | false              |\n| >1GBP      | true               |\n";

/// The step an axis is cut at has to be one its column's values sit on (§15.190), here as under
/// `tools/recheck.py`: an axis of tenths of a pound cut every whole pound is refused, and a
/// certificate with no `scales` rests the step on its own word and is said to.
#[test]
fn 軸の刻みは証明付きの検査器でも値の載る刻みに合う() {
    let Some(bin) = checker() else {
        return;
    };
    let tmp = TempDir::new("lean-tenths");
    let p = tmp.path().join("tenths.rule");
    std::fs::write(&p, TENTHS).unwrap();
    let (c, cert) = rulec(&["certificate", p.to_str().unwrap()]);
    assert_eq!(c, 0, "{cert}");
    assert!(cert.contains(r#""step":"1/10""#), "{cert}");
    let (code, said) = lean(&bin, &cert, Some(p.to_str().unwrap()));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("OK: every claim"), "{said}");

    let forged = cert.replacen(r#""step":"1/10""#, r#""step":"1""#, 1);
    let (code, said) = lean(&bin, &forged, None);
    assert_eq!(code, 1, "整数の刻みで切った軸が通ってしまった:\n{said}");
    assert!(said.contains("its values sit on a step of 1/10"), "{said}");

    let at = cert.find(r#""scales":{"#).expect("証明書に scales が無い");
    let end = at + cert[at..].find("},").unwrap() + 2;
    let old = format!("{}{}", &cert[..at], &cert[end..]);
    let (code, said) = lean(&bin, &old, Some(p.to_str().unwrap()));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("which no scale here bears out"), "{said}");
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
        return;
    };
    let tmp = TempDir::new("lean-set");
    let dir = tmp.path().to_path_buf();
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
    assert_eq!(code, 1, "広げた箱が通ってしまった:\n{said}");
    assert!(said.contains("is not the one its cell describes"), "{said}");
}

/// A certificate of a shape other than the one the proofs are about is refused, not read
/// (§15.156). `tests/cert.rs` holds `tools/recheck.py` to the same.
#[test]
fn 知らない形式の版の証明書は証明付きの検査器でも受け付けない() {
    let Some(bin) = checker() else {
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

/// The certificate of a rule in `tests/sieve` (DESIGN §15.195).
fn sieve_cert(stem: &str) -> String {
    let (c, cert) = rulec(&["certificate", &format!("tests/sieve/{stem}.rule")]);
    assert_eq!(c, 0, "{cert}");
    cert
}

/// The same leaves through the proved checks (§15.195): a coordinate of a `define` past its reach,
/// which `not_asked_of_derived` settles with the reach worked out from the define's expression,
/// and a value a table above writes only in rows that do not fire, which
/// `not_asked_of_aboveRuledOut` settles with the rows read from that table. The same lies as under
/// `tools/recheck.py` are refused.
#[test]
fn 定義と上の表の行の葉は証明付きの検査器でも確かめられる() {
    let Some(bin) = checker() else {
        return;
    };
    let define = sieve_cert("E101_past_a_defines_reach");
    let rel = "tests/sieve/E101_past_a_defines_reach.rule";
    let (code, said) = lean(&bin, &define, Some(rel));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("OK: every claim"), "{said}");
    let lies = [
        ("届く座標に葉を置く", define.replacen(r#"{"split":[{"row":1},"#, r#"{"split":[{"define_axis":0},"#, 1)),
        ("取りうる値を狭く書く", define.replacen(r#""share":["0","1"]"#, r#""share":["0","1/2"]"#, 1).replacen(r#"{"row":2},{"row":2},"#, r#"{"row":2},{"define_axis":0},"#, 1)),
    ];
    for (what, forged) in lies {
        assert_ne!(forged, define, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = lean(&bin, &forged, Some(rel));
        assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった\n{said}");
        assert!(said.contains("FAILED"), "{what}: {said}");
    }

    let above = sieve_cert("E101_past_what_a_table_above_allows");
    let rel = "tests/sieve/E101_past_what_a_table_above_allows.rule";
    let (code, said) = lean(&bin, &above, Some(rel));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("OK: every claim"), "{said}");
    let leaf = r#"{"above_rows":{"axis":0,"column":"refund_size","value":"small","rows":[{"row":1,"farkas":["#;
    let end = r#"{"cond":{"name":"amount","hi":true,"at":"50","open":false},"y":"1"}"#;
    assert!(above.contains(leaf) && above.contains(end), "{above}");
    let lies = [
        ("行の理由を落とす", above.replacen(leaf, &leaf.replace(r#"{"row":1,"#, r#"{"row":2,"#), 1)),
        ("乗数を変える", above.replacen(end, &end.replace(r#""y":"1""#, r#""y":"2""#), 1)),
        ("行の端を動かす", above.replacen(end, &end.replace(r#""at":"50""#, r#""at":"60""#), 1)),
        ("ほかの行も同じ値を書く", above.replacen(r#""produces":["large"]"#, r#""produces":["small"]"#, 1)),
        ("点の値を行の外に置く", above.replacen(r#""at_values":[null,"1"],"extra_values":["1","0"]"#, r#""at_values":[null,"1"],"extra_values":["60","59"]"#, 1)),
    ];
    for (what, forged) in lies {
        assert_ne!(forged, above, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = lean(&bin, &forged, Some(rel));
        assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった\n{said}");
        assert!(said.contains("FAILED"), "{what}: {said}");
    }
}

/// The equation of a `define` of a number whose expression is linear, as a fact of the linear
/// model (§15.209), through the proved checks: the box the rows leave open is closed by
/// `not_asked_of_farkas` with the equation read again from the define's expression (`linOf`), and
/// the same lies as under `tools/recheck.py` are refused.
#[test]
fn 定義の式の事実は証明付きの検査器でも確かめられる() {
    let Some(bin) = checker() else {
        return;
    };
    let rel = "tests/sieve/E101_closed_by_a_defines_equation.rule";
    let cert = sieve_cert("E101_closed_by_a_defines_equation");
    let (code, said) = lean(&bin, &cert, Some(rel));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("OK: every claim"), "{said}");
    let facts = r#""facts":[{"define":"excess","le":true},{"define":"excess","le":false}"#;
    let lies = [
        ("等式の向きを入れ替える", cert.replacen(facts, r#""facts":[{"define":"excess","le":false},{"define":"excess","le":true}"#, 1)),
        ("別の定義の等式を指す", cert.replacen(facts, r#""facts":[{"define":"added","le":true},{"define":"excess","le":false}"#, 1)),
        ("定義の等式を導出の等式と呼ぶ", cert.replacen(facts, r#""facts":[{"derive":"excess","le":true},{"define":"excess","le":false}"#, 1)),
    ];
    for (what, forged) in lies {
        assert_ne!(forged, cert, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = lean(&bin, &forged, Some(rel));
        assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった\n{said}");
        assert!(said.contains("FAILED"), "{what}: {said}");
    }
}

/// The kind of every column is worked out from `inputs`, `walks` and `values`, and held to what
/// the certificate calls it (§15.196).
#[test]
fn 列の種類は証明付きの検査器でも求め直される() {
    let Some(bin) = checker() else {
        return;
    };
    let (c, cert) = rulec(&["certificate", "tests/corpus/rating_grade.rule"]);
    assert_eq!(c, 0, "{cert}");
    let axis = r#""column":"ratio","kind":"define""#;
    assert!(cert.contains(axis), "{cert}");
    assert_eq!(lean(&bin, &cert, None).0, 0, "そのままの証明書が通らない");
    for kind in ["input", "upstream", "derived"] {
        let (code, said) = lean(&bin, &cert.replacen(axis, &axis.replace("define", kind), 1), None);
        assert_eq!(code, 1, "{kind}: 種類を偽った証明書が通ってしまった\n{said}");
        assert!(said.contains("ratio is a column of kind `define`"), "{kind}: {said}");
    }
    let (c, cert) = rulec(&["certificate", "tests/corpus/送料.rule"]);
    assert_eq!(c, 0, "{cert}");
    let inputs = r#""inputs":["届け先","重量","注文金額","会員"]"#;
    assert!(cert.contains(inputs), "{cert}");
    let (code, said) = lean(&bin, &cert.replacen(inputs, r#""inputs":["届け先","重量","注文金額"]"#, 1), None);
    assert_eq!(code, 1, "入力を落とした証明書が通ってしまった\n{said}");
    assert!(said.contains("会員 is a column of kind `upstream`"), "{said}");
}

/// The range of a column a table writes numbers into is the hull of what its rows write, worked out
/// here; the stated range is held to it, and with the file each answer cell is read back (§15.196).
#[test]
fn 上の表が書く数の範囲は証明付きの検査器でも行から計算し直される() {
    let Some(bin) = checker() else {
        return;
    };
    let rel = "tests/apply_fixtures/規程.rule";
    let (c, cert) = rulec(&["certificate", rel]);
    assert_eq!(c, 0, "{cert}");
    let range = r#""率":["1/5","1"]"#;
    let cell = r#"{"num":"50%","value":"1/2","type":"rate"}"#;
    assert!(cert.contains(range) && cert.contains(cell), "{cert}");
    let (code, said) = lean(&bin, &cert, Some(rel));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("1 columns a table writes numbers into hold every value their rows write"), "{said}");
    let (code, said) = lean(&bin, &cert.replacen(range, r#""率":["1/5","4/5"]"#, 1), None);
    assert_eq!(code, 1, "狭く言った範囲が通ってしまった\n{said}");
    assert!(said.contains("its rows write a value outside the range the certificate states for it"), "{said}");
    let forged = cert.replacen(cell, r#"{"num":"50%","value":"3/5","type":"rate"}"#, 1);
    assert_eq!(lean(&bin, &forged, None).0, 0, "ファイル無しでは区別できない書き換えのはず");
    let (code, said) = lean(&bin, &forged, Some(rel));
    assert_eq!(code, 1, "セルの値を書き換えた証明書が通ってしまった\n{said}");
    assert!(said.contains("the file writes `50%`, and the certificate reads it as another number"), "{said}");
    let (code, said) = lean(&bin, &cert.replacen(cell, r#"{"num":"60%","value":"3/5","type":"rate"}"#, 1), Some(rel));
    assert_eq!(code, 1, "セルの字を書き換えた証明書が通ってしまった\n{said}");
    assert!(said.contains("the file does not say `60%` there"), "{said}");
}

/// The other truth value of a boolean `define` that always takes one is a box no input reaches,
/// worked out again from the comparison (§15.196, `not_asked_of_truth`).
#[test]
fn 真偽の定義がとらない値の葉は証明付きの検査器でも計算し直される() {
    let Some(bin) = checker() else {
        return;
    };
    let cert = sieve_cert("E101_a_define_that_is_always_true");
    let cover = r#""cover":{"split":[{"row":1},{"truth_axis":0}]}"#;
    assert!(cert.contains(cover), "{cert}");
    assert_eq!(lean(&bin, &cert, None).0, 0, "そのままの証明書が通らない");
    for (what, forged) in [
        ("葉を true の側に置く", cert.replacen(cover, r#""cover":{"split":[{"truth_axis":0},{"row":1}]}"#, 1)),
        ("比べる日を動かす", cert.replacen(r#""value":"20908""#, r#""value":"20000""#, 1)),
        ("入力の範囲を延ばす", cert.replacen(r#""made":["19814","20818"]"#, r#""made":["19814","21000"]"#, 1)),
    ] {
        assert_ne!(forged, cert, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = lean(&bin, &forged, None);
        assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった\n{said}");
        assert!(said.contains("the cover does not tile the space"), "{what}: {said}");
    }
}

/// Two rows the rows of a table above part: the pair is proved apart on the points the rule is asked
/// about (§15.196, `not_asked_of_abovePart`), and three ways of lying about it are refused.
#[test]
fn 上の表の行で分かれる対は証明付きの検査器でも確かめられる() {
    let Some(bin) = checker() else {
        return;
    };
    let cert = sieve_cert("E105_rows_that_meet_where_a_table_above_allows_nothing");
    let pair = r#"{"a":1,"b":2,"axis":0,"at":[{"coord":0,"above_rows":{"axis":0,"column":"refund_size","value":"small","rows":[{"row":1,"farkas":["#;
    assert!(cert.contains(pair), "{cert}");
    let (code, said) = lean(&bin, &cert, None);
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("no two rows meet"), "{said}");
    let end = r#"{"coord":{"axis":1,"hi":false,"at":"60","open":true},"y":"1/10"}"#;
    for (what, forged) in [
        ("別の値の理由を書く", cert.replacen(pair, &pair.replace(r#""at":[{"coord":0,"#, r#""at":[{"coord":1,"#), 1)),
        ("乗数を変える", cert.replacen(end, &end.replace(r#""y":"1/10""#, r#""y":"1/5""#), 1)),
        ("別の対に付ける", cert.replacen(pair, &pair.replace(r#""b":2"#, r#""b":3"#), 1)),
    ] {
        assert_ne!(forged, cert, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = lean(&bin, &forged, None);
        assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった\n{said}");
        assert!(said.contains("two rows are neither proved apart nor named as undecided"), "{what}: {said}");
    }
}

/// A constraint between two dates, read by the day numbers of its two sides (§15.198): the point
/// that reaches a row keeps it, and a leaf that says it rules a box out is held to the box's ends.
/// The two lies `tests/cert.rs` tells are refused here too.
#[test]
fn 日付どうしの制約は証明付きの検査器でも到達点と葉で確かめられる() {
    let Some(bin) = checker() else {
        return;
    };
    for (path, check_in, check_out) in [
        ("tests/date_constraint/hotel_stay.rule", "check_in", "check_out"),
        ("tests/date_constraint/宿泊料金.rule", "チェックイン日", "チェックアウト日"),
    ] {
        let (c, cert) = rulec(&["certificate", path]);
        assert_eq!(c, 0, "{cert}");
        assert_eq!(lean(&bin, &cert, Some(path)).0, 0, "{path}: そのままの証明書が通らない");
        let point = format!(r#"{{"row":3,"at":[3,3],"values":{{"{check_in}":"2026-07-01","{check_out}":"2026-07-01"}},"at_values":["20635","20635"],"extra_values":[]}}"#);
        let moved = format!(r#"{{"row":3,"at":[4,4],"values":{{"{check_in}":"2026-07-10","{check_out}":"2026-07-05"}},"at_values":["20644","20639"],"extra_values":[]}}"#);
        let leaf = r#"{"split":[{"constraint":0},{"constraint":0},{"constraint":0},{"row":3}"#;
        assert!(cert.contains(&point) && cert.contains(leaf), "{path}: 証明書の形が変わっていて、偽れない:\n{cert}");
        for (what, forged, why) in [
            ("到達点のチェックアウト日をチェックイン日より前にする", cert.replacen(&point, &moved, 1), "a row has no point, or the values behind its point do not hold"),
            (
                "同じ日に始まり終わる箱を制約で落とす",
                cert.replacen(leaf, r#"{"split":[{"constraint":0},{"constraint":0},{"constraint":0},{"constraint":0}"#, 1),
                "the cover does not tile the space",
            ),
        ] {
            let (code, said) = lean(&bin, &forged, Some(path));
            assert_eq!(code, 1, "{path}: {what}: 偽った証明書が通ってしまった\n{said}");
            assert!(said.contains(why), "{path}: {what}: {said}");
        }
    }
}

/// Every `.rule` under the workspace's crates and site, by its path from the workspace's root.
fn tree_rules() -> Vec<String> {
    fn walk(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() {
                if !matches!(name.as_str(), "target" | "node_modules" | ".venv" | ".lake" | ".git") {
                    walk(&p, base, out);
                }
            } else if name.ends_with(".rule") {
                out.push(p.strip_prefix(base).unwrap().to_string_lossy().into_owned());
            }
        }
    }
    let base = root().join("../..").canonicalize().unwrap();
    let mut out = Vec::new();
    for d in ["crates", "website"] {
        walk(&base.join(d), &base, &mut out);
    }
    out.sort();
    out
}

/// Every rule in the tree that states a certificate passes here too, and under `tools/recheck.py`,
/// held to its own file (§15.196): the cells of a rule that applies another, and of a mutant, are
/// read back as well, and every numeric output with a range is held to what it answers once rounded
/// to its grid, as many by one program as by the other (§15.269).
#[test]
fn 木の規則の証明書は証明付きの検査器でもファイルを添えて通る() {
    let Some(bin) = checker() else {
        return;
    };
    use std::io::Write;
    use std::process::Stdio;
    let base = root().join("../..").canonicalize().unwrap();
    let (mut held, mut failures) = (0usize, Vec::new());
    let (mut outputs, mut python) = ((0usize, 0usize), 0usize);
    for rel in tree_rules() {
        let o = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(&base).args(["certificate", &rel]).output().unwrap();
        if !o.status.success() {
            continue;
        }
        // The certificate of a contribution is the rule it adds to, merged with it (§15.231).
        let rel = held_to(&base, &rel);
        let mut p = Command::new(&bin)
            .current_dir(&base)
            .args(["--rule", &rel])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Lean の検査器を起動できない");
        p.stdin.as_mut().unwrap().write_all(&o.stdout).unwrap();
        let q = p.wait_with_output().unwrap();
        let said = String::from_utf8_lossy(&q.stdout).into_owned();
        if !q.status.success() {
            failures.push(format!("{rel}:\n{said}"));
        }
        outputs.0 += outputs_held(&said);
        // `tools/recheck.py` too, which holds every output's range to what it answers once rounded
        // as the Lean program does (§15.269).
        let cert = String::from_utf8_lossy(&o.stdout).into_owned();
        if let Some((code, said)) = python_at(&base, &cert, &rel) {
            if code != 0 {
                failures.push(format!("{rel} (tools/recheck.py):\n{said}"));
            }
            outputs.1 += outputs_held(&said);
            python += 1;
        }
        held += 1;
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(held >= 190, "証明書の出る規則が減っている: {held}");
    // Every numeric output that has a range in the certificate is held to it, by both programs alike.
    assert!(outputs.0 >= 500, "出力の範囲を確かめた数が減っている: {}", outputs.0);
    if python > 0 {
        assert_eq!(outputs.0, outputs.1, "二つの再検査が確かめた出力の範囲の数が違う");
    }
}

/// How many output ranges a re-checker says it held to what the output answers once rounded.
fn outputs_held(said: &str) -> usize {
    said.lines()
        .filter_map(|l| l.trim().strip_prefix("outputs: "))
        .filter_map(|l| l.split_whitespace().next()?.parse::<usize>().ok())
        .sum()
}

/// [`python`] run from the workspace's root, as the tree's rules are named from there.
fn python_at(base: &std::path::Path, cert: &str, rule: &str) -> Option<(i32, String)> {
    use std::io::Write;
    use std::process::Stdio;
    let have = Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
    if !ready(Need::Python, || have, "python3 が無いので tools/recheck.py は回していない") {
        return None;
    }
    let mut p = Command::new("python3")
        .current_dir(base)
        .args([root().join("tools/recheck.py").to_str().unwrap(), "--rule", rule])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("python3 を起動できない");
    p.stdin.as_mut().unwrap().write_all(cert.as_bytes()).unwrap();
    let o = p.wait_with_output().unwrap();
    Some((o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr)))
}

/// What an output answers is what decides it put on the grid its line rounds to (rulec's §15.269):
/// a fee of the amount itself, 0JPY to 1,005JPY, rounded half up to ten yen answers up to 1,010JPY.
/// The range the certificate states for the output under `output_ranges` — written on its line, or
/// what a `result`, a `define` or a table decides put out to the grid — is held to that by both
/// re-checkers (`output_mem_stated`, `written_output_mem_stated`), while `ranges` keeps what decides
/// it: one that holds what decides the output and not its rounding, or less, is refused, with the
/// same words from each. So is a range with no grid — the output's taken out of `rounds`, or `rounds`
/// taken out whole — since what it bounds is the answer on that grid. A certificate from before both
/// leaves the claim stated rather than proved.
#[test]
fn 出力の範囲は丸めた答えまで二つの再検査で確かめられる() {
    let Some(bin) = checker() else {
        return;
    };
    let tmp = TempDir::new("lean-rounds");
    let dir = tmp.path().to_path_buf();
    // Each rule, and the high end `ranges` states for `fee`: a range written on the line, or what the
    // `result` comes to (§15.268), is already on the grid; a `define` and a table keep what decides it.
    let rules = [
        ("written", "rule coarse v1\n\ninputs\n  amount : money[JPY]  range >=0JPY <=1005JPY\n\noutputs\n  fee : money[JPY]  round half_up(10JPY)  range >=0JPY <=1010JPY\n\nresult fee = amount\n", "1010"),
        ("result", "rule coarse v1\n\ninputs\n  amount : money[JPY]  range >=0JPY <=1005JPY\n\noutputs\n  fee : money[JPY]  round half_up(10JPY)\n\nresult fee = amount\n", "1010"),
        ("define", "rule coarse v1\n\ninputs\n  amount : money[JPY]  range >=0JPY <=1005JPY\n\noutputs\n  fee : money[JPY]  round half_up(10JPY)\n\ndefine fee : money[JPY] = amount\n", "1005"),
        ("table", "rule coarse v1\n\ninputs\n  amount : money[JPY]  range >=0JPY <=1005JPY\n  waived : bool\n\noutputs\n  fee : money[JPY]  round half_up(10JPY)\n\ndefine charged : money[JPY] = amount\n\ntable fees\npolicy unique\n| waived | -> fee  |\n| false  | charged |\n| true   | 0JPY    |\n", "1005"),
    ];
    for (what, src, decided) in rules {
        let p = dir.join(format!("{what}.rule"));
        std::fs::write(&p, src).unwrap();
        let rule = p.to_str().unwrap();
        let (c, cert) = rulec(&["certificate", rule]);
        assert_eq!(c, 0, "{what}: {cert}");
        let stated = r#""output_ranges":{"fee":["0","1010"]}"#;
        assert!(cert.contains(stated), "{what}: 出力の範囲が丸めた答えまで届かない\n{cert}");
        assert!(cert.contains(&format!(r#""fee":["0","{decided}"]"#)), "{what}: ranges が出力を決めるものの範囲でない\n{cert}");
        assert!(cert.contains(r#""rounds":{"fee":"10"}"#), "{what}: 証明書が出力の刻みを言わない\n{cert}");
        let (code, said) = lean(&bin, &cert, Some(rule));
        assert_eq!(code, 0, "{what}: {said}");
        assert!(said.contains("outputs: 1 ranges hold what their output answers"), "{what}: {said}");
        if let Some((code, said)) = python(&cert, rule) {
            assert_eq!(code, 0, "{what}: {said}");
            assert!(said.contains("outputs: 1 ranges hold what their output answers"), "{what}: {said}");
        }
        // Narrowed to what decides the output, and below it: refused by both, with the same words.
        for hi in ["1005", "1000"] {
            let why = format!("fee: what decides it runs from 0 to 1005; rounded to 10, it answers from 0 to 1010, and the certificate states the range [0, {hi}] for it");
            let forged = cert.replacen(stated, &format!(r#""output_ranges":{{"fee":["0","{hi}"]}}"#), 1);
            assert_ne!(forged, cert);
            let (code, said) = lean(&bin, &forged, Some(rule));
            assert_eq!(code, 1, "{what}: 狭めた出力の範囲が Lean で通ってしまった\n{said}");
            assert!(said.contains(&why), "{what}: {said}");
            if let Some((code, said)) = python(&forged, rule) {
                assert_eq!(code, 1, "{what}: 狭めた出力の範囲が tools/recheck.py で通ってしまった\n{said}");
                assert!(said.contains(&why), "{what}: {said}");
            }
        }
        // A range narrowed below the answer with no grid to round to: the output's grid taken out of
        // `rounds`, or `rounds` taken out whole. Refused by both, with the same words, rather than left
        // unread.
        let why = "fee: the certificate states a range for it but not the grid it rounds to";
        let narrowed = r#""output_ranges":{"fee":["0","1000"]}"#;
        let forms = [
            ("fee の刻みを外した", cert.replacen(&format!(r#""rounds":{{"fee":"10"}},{stated}"#), &format!(r#""rounds":{{}},{narrowed}"#), 1)),
            ("rounds を丸ごと外した", cert.replacen(&format!(r#","rounds":{{"fee":"10"}},{stated}"#), &format!(",{narrowed}"), 1)),
        ];
        for (form, forged) in forms {
            assert_ne!(forged, cert, "{what}: {form}");
            let (code, said) = lean(&bin, &forged, Some(rule));
            assert_eq!(code, 1, "{what}: 刻みの無い出力の範囲が Lean で通ってしまった（{form}）\n{said}");
            assert!(said.contains(why), "{what}: {form}: {said}");
            if let Some((code, said)) = python(&forged, rule) {
                assert_eq!(code, 1, "{what}: 刻みの無い出力の範囲が tools/recheck.py で通ってしまった（{form}）\n{said}");
                assert!(said.contains(why), "{what}: {form}: {said}");
            }
        }
        // Without the grids and the ranges they put out, the claim is the certificate's word, and the
        // run says so.
        let old = cert.replacen(&format!(r#","rounds":{{"fee":"10"}},{stated}"#), "", 1);
        assert_ne!(old, cert);
        let (code, said) = lean(&bin, &old, Some(rule));
        assert_eq!(code, 0, "{what}: {said}");
        assert!(said.contains("the certificate does not say what grid its outputs round to") && !said.contains("OK: every claim"), "{what}: {said}");
        if let Some((code, said)) = python(&old, rule) {
            assert_eq!(code, 0, "{what}: {said}");
            assert!(said.contains("the certificate does not say what grid its outputs round to"), "{what}: {said}");
        }
    }
}

/// The range an output answers in is stated beside `ranges`, not in it (rulec's §15.269), because the
/// other claims read `ranges` as what decides each name. A sum of up to 1,050 cents that the rule
/// rounds down to a dollar answers up to 1,100 cents, and a table cut on the sum runs to 1,050 cents;
/// a `define` read by one written above it bounds that one by its own range. Both certificates pass
/// both re-checkers, with the output's range held. A range written on the line of a walk is the walk's
/// too, and nothing tighter is stated: off the grid, the run says so rather than refusing it.
#[test]
fn 出力の範囲を丸めの刻みへ広げても証明書のほかの主張は変わらない() {
    let Some(bin) = checker() else {
        return;
    };
    let tmp = TempDir::new("lean-rounds-apart");
    let dir = tmp.path().to_path_buf();
    let walk = |total: &str, sum: &str| {
        format!(
            "rule walk_grid v1\n\ninputs\n  cap : money[USDc]  range >=0USDc <=1000USDc  contract_only\n\nelements items\n  w : money[USDc]  range >=0USDc <=150USDc\n\noutputs\n  total : money[USDc]  round down(100USDc){total}\n  big   : bool\n\nsum total over items of w  range >=0USDc <={sum}USDc\n\ntable big_of\npolicy unique\n| total     | -> big : bool |\n| <=500USDc | false         |\n| >500USDc  | true          |\n"
        )
    };
    let read_first = "rule order_dep v1\n\ninputs\n  amount : money[USDc]  range >=1USDc <=1000USDc\n\noutputs\n  total : money[USDc]  round half_up(1USDc)\n  fee   : money[USDc]  round half_up(1USDc)\n\ndefine total : money[USDc] = fee + amount\n\ndefine fee : money[USDc] = amount × 3.49% + 49USDc\n";
    let cases = [
        ("walk", walk("", "1050"), r#""total":["0","1050"]"#, r#""output_ranges":{"total":["0","1100"]}"#),
        ("read_first", read_first.to_string(), r#""fee":["490349/10000","839/10"]"#, r#""output_ranges":{"fee":["49","84"]}"#),
    ];
    for (what, src, decided, stated) in cases {
        let p = dir.join(format!("{what}.rule"));
        std::fs::write(&p, &src).unwrap();
        let rule = p.to_str().unwrap();
        let (c, cert) = rulec(&["certificate", rule]);
        assert_eq!(c, 0, "{what}: {cert}");
        assert!(cert.contains(decided) && cert.contains(stated), "{what}: {cert}");
        let (code, said) = lean(&bin, &cert, Some(rule));
        assert_eq!(code, 0, "{what}: {said}");
        assert!(said.contains("outputs: 1 ranges hold what their output answers") && said.contains("OK: every claim"), "{what}: {said}");
        if let Some((code, said)) = python(&cert, rule) {
            assert_eq!(code, 0, "{what}: {said}");
            assert!(said.contains("outputs: 1 ranges hold what their output answers") && said.contains("every claim this program states was proved"), "{what}: {said}");
        }
    }
    // A range written on the walk's output line, off the grid: `check` holds it to what the sum comes
    // to (E140), and the certificate has nothing tighter for the re-checkers.
    let p = dir.join("walk_written.rule");
    std::fs::write(&p, walk("  range >=0USDc <=1050USDc", "1000")).unwrap();
    let rule = p.to_str().unwrap();
    let (c, cert) = rulec(&["certificate", rule]);
    assert_eq!(c, 0, "{cert}");
    let why = "what total answers once rounded: the range it declares, [0, 1050], does not sit on the grid it rounds to, 100";
    let (code, said) = lean(&bin, &cert, Some(rule));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains(why) && !said.contains("OK: every claim"), "{said}");
    if let Some((code, said)) = python(&cert, rule) {
        assert_eq!(code, 0, "{said}");
        assert!(said.contains(why), "{said}");
    }
}

/// What a row writes where its own cell narrows it (`inside`, rulec's §15.213) is worked out again from
/// the row's cell (`cut_of_box`, `written_mem_hull`, §15.232): nothing about it is left stated, and
/// the three ways of lying about it that `tests/arith.rs` holds `tools/recheck.py` to are refused here.
#[test]
fn 行のセルが狭める範囲は証明付きの検査器でもセルから組み直される() {
    let Some(bin) = checker() else {
        return;
    };
    let rel = "tests/corpus/average_unit_price_split.rule";
    let (c, cert) = rulec(&["certificate", rel]);
    assert_eq!(c, 0, "{cert}");
    let inside = r#""inside":["1","1000"]"#;
    assert!(cert.contains(inside), "{cert}");
    let (code, said) = lean(&bin, &cert, Some(rel));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("1 rows inside what their own cell lets through"), "{said}");
    assert!(said.contains("OK: every claim this program states was proved"), "{said}");
    for (what, forged, why) in [
        (
            "狭く言った inside",
            cert.replacen(inside, r#""inside":["2","1000"]"#, 1),
            "row 2's cell lets units take [1, 1000], and the certificate says the row writes it inside [2, 1000]",
        ),
        ("狭く言った列の範囲", cert.replacen(r#""per":["1","1000"]"#, r#""per":["2","1000"]"#, 1), "per: its rows write a value outside the range the certificate states for it"),
        ("行の inside を消す", cert.replacen(&format!(",{inside}"), "", 1), "per: its rows write a value outside the range the certificate states for it"),
    ] {
        assert_ne!(forged, cert, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = lean(&bin, &forged, Some(rel));
        assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった\n{said}");
        assert!(said.contains(why), "{what}: {said}");
    }
}

/// A discount by tier, for an input whose `range` lets in two of the four tiers.
const RANGED_TIER: &str = "rule member_discount v1\ndescription \"A discount by the member's tier, for the tiers from silver to gold\"\n\nenum tier = basic < silver < gold < platinum\n\ninputs\n  member : tier  range >=silver <=gold\n\noutputs\n  discount : rate  round down(1%)\n\ntable discount_of\npolicy unique\n| member | -> discount |\n| silver | 5%          |\n| >=gold | 10%         |\n";

/// The axis of an input of an enum is the enum's values, or those its `range` lets in (rulec's
/// §15.210, `enumAxis_coords`, §15.232): checked by the proved checker too, so neither an
/// `enum_ranges` the axis does not bear out nor an axis that leaves a value off passes.
#[test]
fn 列挙の入力の軸は証明付きの検査器でも列挙の値に合う() {
    let Some(bin) = checker() else {
        return;
    };
    let tmp = TempDir::new("lean-enum-axis");
    let p = tmp.path().join("member_discount.rule");
    std::fs::write(&p, RANGED_TIER).unwrap();
    let rel = p.to_str().unwrap();
    let (c, cert) = rulec(&["certificate", rel]);
    assert_eq!(c, 0, "{cert}");
    let ranges = r#""enum_ranges":{"member":["silver","gold"]}"#;
    assert!(cert.contains(ranges), "{cert}");
    let (code, said) = lean(&bin, &cert, Some(rel));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("1 axes of an enum's input run over the values it takes"), "{said}");
    assert!(said.contains("OK: every claim this program states was proved"), "{said}");
    for (what, forged) in [
        ("範囲に値を足す", cert.replacen(ranges, r#""enum_ranges":{"member":["silver","gold","platinum"]}"#, 1)),
        ("範囲を消す", cert.replacen(&format!(",{ranges}"), "", 1)),
    ] {
        assert_ne!(forged, cert, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = lean(&bin, &forged, Some(rel));
        assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった\n{said}");
        assert!(said.contains("member's axis is not the values its enum lets in"), "{what}: {said}");
    }
    // An enum with no range: the axis is every value, in the enum's order.
    let rel = "tests/corpus/grace_period.rule";
    let (c, cert) = rulec(&["certificate", rel]);
    assert_eq!(c, 0, "{cert}");
    let axis = r#""coords":["payment_missed","day_passed","payment_received"]"#;
    assert!(cert.contains(axis), "{cert}");
    let (code, said) = lean(&bin, &cert.replacen(axis, r#""coords":["day_passed","payment_missed","payment_received"]"#, 1), Some(rel));
    assert_eq!(code, 1, "並べ替えた軸が通ってしまった\n{said}");
    assert!(said.contains("event's axis is not the values its enum lets in"), "{said}");
}

/// The values a machine carries besides its state (rulec's §15.217, §15.232): the places are closed
/// under every call the check counts from them (`Carried.Model.simulates`, `Carried.Sys.reaches_mem`),
/// and the claims laid there are proved — only that a case can still finish stays stated. Every way of
/// breaking a claim in one place is refused, and a certificate that does not say what grid the
/// carried output rounds to is named as stated rather than passed.
#[test]
fn 持ち越す値の主張は証明付きの検査器で証明される() {
    let Some(bin) = checker() else {
        return;
    };
    for (rel, line) in [
        ("tests/corpus/grace_period.rule", "carried values over step: 9 places a case can be, no call leaves a final state, `never` line 1, `always` line 1, `always` line 2, next_days_left within its range"),
        ("tests/corpus/retry_attempts.rule", "carried values over step: 7 places a case can be, no call leaves a final state, `never` line 1, `always` line 1, `always` line 2, next_attempts within its range"),
        ("tests/corpus/one_time_perk.rule", "carried values over step: 5 places a case can be, no call leaves a final state, `once` line 1 (trial_granted), `always` line 1"),
        ("tests/corpus/猶予の残り日数.rule", "carried values over 遷移: 9 places a case can be"),
        ("tests/corpus/再試行の回数.rule", "carried values over 試行: 7 places a case can be"),
        ("tests/corpus/一度だけの特典.rule", "carried values over 申し出の扱い: 5 places a case can be"),
    ] {
        let (c, cert) = rulec(&["certificate", rel]);
        assert_eq!(c, 0, "{rel}: {cert}");
        let (code, said) = lean(&bin, &cert, Some(rel));
        assert_eq!(code, 0, "{rel}: {said}");
        assert!(said.contains(line), "{rel}: {said}");
        assert!(!said.contains("besides its state"), "{rel}: まだ述べただけと言っている\n{said}");
        assert!(!said.contains("is not laid on the rows") || said.contains("that a case can still finish is not laid on the rows"), "{rel}: {said}");
    }
    let rel = "tests/corpus/grace_period.rule";
    let (c, cert) = rulec(&["certificate", rel]);
    assert_eq!(c, 0, "{cert}");
    for (what, forged, why) in [
        ("場所を一つ落とした", cert.replace("[1,[2]],[1,[1]],[2,[0]]]", "[1,[1]],[2,[0]]]"), "leads outside the places"),
        ("行 1 が書く値", cert.replacen(r#"{"row":1,"to":7,"#, r#"{"row":1,"to":6,"#, 1), "what row 1 is said to write into next_days_left is not what the table says"),
        (
            "always の境を狭めた",
            cert.replacen(r#"{"op":"<=","value":"7"}]}}],"holds":true"#, r#"{"op":"<=","value":"6"}]}}],"holds":true"#, 1),
            "the `always` line 1 does not hold at every place it names",
        ),
        ("始まりの場所", cert.replacen(r#""start":[0]"#, r#""start":[1]"#, 1), "the places leave out where a case starts"),
        ("範囲", cert.replacen(r#""range":["0","7"]"#, r#""range":["0","8"]"#, 1), "the range of days_left is not the one it declares"),
        ("丸めの刻み", cert.replacen(r#""round":"1""#, r#""round":"2""#, 1), "leads outside the places"),
        ("座標の刻み", cert.replacen(r#""step":"1","round""#, r#""step":"2","round""#, 1), "do not run from one end of its range to the other on its step"),
        ("never の組を一つ落とした", cert.replacen("[1,[7],false],", "", 1), "the `never` set of line 1 is not closed"),
        ("範囲の外の場所", cert.replacen(r#""places":["#, r#""places":[[1,[-1]],"#, 1), "next_days_left is said to stay within its range, and a place leaves it"),
        ("始まりの座標を空にする", cert.replacen(r#""start":[0]"#, r#""start":[]"#, 1), "where a case starts is not one coordinate of each carried value"),
        ("座標の無い場所", cert.replacen(r#""places":["#, r#""places":[[0,[]],"#, 1), "a place does not give one coordinate of each carried value"),
        ("行の値のファイルの字", cert.replacen(r#""expr":"one_less","source":{"line":25,"col":62,"len":8,"text":"one_less"}"#, r#""expr":"one_less","source":{"line":25,"col":62,"len":8,"text":"days_lef"}"#, 1), "the file has"),
    ] {
        assert_ne!(forged, cert, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = lean(&bin, &forged, Some(rel));
        assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった\n{said}");
        assert!(said.contains(why), "{what}: {said}");
    }
    // A certificate from before `round` was written cannot say where a rounded value lands.
    let (code, said) = lean(&bin, &cert.replacen(r#","round":"1""#, "", 1), Some(rel));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("the certificate does not say what grid next_days_left rounds to"), "{said}");
    let rel = "tests/corpus/one_time_perk.rule";
    let (c, cert) = rulec(&["certificate", rel]);
    assert_eq!(c, 0, "{cert}");
    let (code, said) = lean(&bin, &cert.replacen("[1,[1],1]", "[1,[1],0]", 1), Some(rel));
    assert_eq!(code, 1, "数えた呼び出しを数えない once の組が通ってしまった\n{said}");
    assert!(said.contains("the `once trial_granted` set is not closed, or counts two"), "{said}");
}

/// Feed one certificate to `tools/recheck.py`, with the rule it is about, when python3 is here.
fn python(cert: &str, rule: &str) -> Option<(i32, String)> {
    use std::io::Write;
    use std::process::Stdio;
    let have = Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
    if !ready(Need::Python, || have, "python3 が無いので tools/recheck.py は回していない") {
        return None;
    }
    let mut p = Command::new("python3")
        .current_dir(root())
        .args(["tools/recheck.py", "--rule", rule])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("python3 を起動できない");
    p.stdin.as_mut().unwrap().write_all(cert.as_bytes()).unwrap();
    let o = p.wait_with_output().unwrap();
    Some((o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr)))
}

/// Each forgery has to be refused by both re-checkers, the Lean one saying `why`.
fn both_refuse(bin: &PathBuf, cert: &str, rule: &str, forged: &[(&str, String, &str)]) {
    for (what, f, why) in forged {
        assert_ne!(f, cert, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = lean(bin, f, Some(rule));
        assert_eq!(code, 1, "{what}: 偽った証明書が Lean で通ってしまった\n{said}");
        assert!(said.contains(why), "{what}: {said}");
        if let Some((code, said)) = python(f, rule) {
            assert_eq!(code, 1, "{what}: 偽った証明書が tools/recheck.py で通ってしまった\n{said}");
        }
    }
}

/// A block of prerequisites ordered by a table of numbers (rulec's §15.222, §15.233): the answers are
/// the numbers the rows write (`writes`), not words they produce.
const TIERS: &str = "rule tech_tiers v1\ndescription \"The tier of each technology, and what a player must have researched before it\"\n\nenum tech = farming | pottery | writing | wheel | math\n\ninputs\n  tech : tech\n\noutputs\n  tier : number  round down(1)\n\ntable tier_of\npolicy unique\n| tech           | -> tier |\n| farming        | 1       |\n| pottery, wheel | 2       |\n| writing        | 3       |\n| math           | 4       |\n\nprerequisites research over tech\n  start   farming\n  pottery requires farming\n  wheel   requires farming\n  writing requires pottery or wheel\n  math    requires writing, wheel\n  ordered by tier_of\n\nexamples\n| tech    | -> tier |\n| farming | 1       |\n| pottery | 2       |\n";

/// Which values need which before them (rulec's §15.222, §15.233): no cycle (`Prereq.no_cycle`),
/// every value but those no case can take reached from `start` (`Prereq.reached_holds`,
/// `Prereq.never_held`), and the order of the table the block is `ordered by` kept along its edges
/// (`Prereq.order_along`) — proved, so nothing about a block is left stated; and each claim broken in
/// one place is refused by the Lean checker and by `tools/recheck.py` alike.
#[test]
fn 前提のつながりは証明付きの検査器で証明される() {
    let Some(bin) = checker() else {
        return;
    };
    for (rel, line) in [
        ("tests/corpus/course_catalog.rule", "prerequisites enrollment over course: 14 values and 15 edges with no cycle, 13 reached from start and 1 no case can take, and level_of keeps its order along 15 edges"),
        ("tests/corpus/科目の履修.rule", "prerequisites 履修 over 科目: 14 values and 15 edges with no cycle, 13 reached from start and 1 no case can take, and 年次表 keeps its order along 15 edges"),
        ("tests/corpus/shared/certifications.rule", "prerequisites exam_path over certification: 7 values and 8 edges with no cycle, 7 reached from start and 0 no case can take"),
        ("tests/corpus/shared/資格.rule", "prerequisites 受験の前提 over 資格: 7 values"),
    ] {
        let (c, cert) = rulec(&["certificate", rel]);
        assert_eq!(c, 0, "{rel}: {cert}");
        let (code, said) = lean(&bin, &cert, Some(rel));
        assert_eq!(code, 0, "{rel}: {said}");
        assert!(said.contains(line), "{rel}: {said}");
        assert!(said.contains("OK: every claim this program states was proved"), "{rel}: {said}");
    }
    let rel = "tests/corpus/course_catalog.rule";
    let (_, cert) = rulec(&["certificate", rel]);
    both_refuse(&bin, &cert, rel, &[
        ("値を二か所に書く", cert.replacen(r#""start":["intro_programming","discrete_math","calculus"]"#, r#""start":["intro_programming","discrete_math","calculus","statistics"]"#, 1), "the values of course are not each said once"),
        ("前提より前に来る順", cert.replacen(r#""topological":["intro_programming","#, r#""topological":["statistics","intro_programming","#, 1), "`topological` is not the values of course, each once"),
        ("前提の輪", cert.replacen(r#""statistics":[["calculus"]]"#, r#""statistics":[["machine_learning"]]"#, 1), "`topological` puts a value before one it needs"),
        ("前提を持つ前にたどり着く順", cert.replacen(r#""reached":["intro_programming","#, r#""reached":["capstone","intro_programming","#, 1), "`reached` and `never` are not the values of course, each once"),
        ("たどり着く順を入れ替える", cert.replacen(r#""reached":["intro_programming","discrete_math","calculus","statistics","linear_algebra","data_structures","databases","algorithms""#, r#""reached":["intro_programming","discrete_math","calculus","statistics","linear_algebra","algorithms","databases","data_structures""#, 1), "`reached` names a value before a value of each group it needs"),
        ("表が書かない答え", cert.replacen(r#""capstone":"fourth""#, r#""capstone":"first""#, 1), "the answers of level_of are not the ones its rows write"),
        ("表の行が書く語", cert.replacen(r#""accepts":[[0,1,2]],"produces":["first"]"#, r#""accepts":[[0,1,2]],"produces":["second"]"#, 1), "level_of: row 1, answer 1: the file writes `first` there, and the certificate reads it as `second`"),
    ]);
    // Ordered by a table of numbers: the answers are what the rows write.
    let tmp = TempDir::new("lean-tiers");
    let p = tmp.path().join("tech_tiers.rule");
    std::fs::write(&p, TIERS).unwrap();
    let rel = p.to_str().unwrap();
    let (c, cert) = rulec(&["certificate", rel]);
    assert_eq!(c, 0, "{cert}");
    assert!(cert.contains(r#""pottery":"2""#), "{cert}");
    let line = "prerequisites research over tech: 5 values and 6 edges with no cycle, 5 reached from start and 0 no case can take, and tier_of keeps its order along 6 edges";
    let (code, said) = lean(&bin, &cert, Some(rel));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains(line), "{said}");
    if let Some((code, said)) = python(&cert, rel) {
        assert_eq!(code, 0, "{said}");
        assert!(said.contains(line), "{said}");
    }
    both_refuse(&bin, &cert, rel, &[
        ("数の答えを動かす", cert.replacen(r#""pottery":"2""#, r#""pottery":"1""#, 1), "the answers of tier_of are not the ones its rows write"),
        ("表の行が書く数", cert.replacen(r#""value":"4","type":"number"},"source":{"line":18"#, r#""value":"1","type":"number"},"source":{"line":18"#, 1), "tier_of: row 4, -> tier: the file writes `4`, and the certificate reads it as another number"),
    ]);
}

/// A loan paid off in instalments, settled once nothing is left: a table under `first` that reads a
/// number the rule computes (`left`), from an input no column reads (`amount`).
const LOAN: &str = "rule loan_payoff v1\ndescription \"A loan paid off in instalments of up to three, settled once nothing is left\"\n\nenum status = open | settled\nenum act = pay | wait\n\ninputs\n  status  : status\n  act     : act\n  balance : number  range >=0 <=6\n  amount  : number  range >=1 <=3\n\noutputs\n  next_status  : status\n  next_balance : number  round down(1)\n\ndefine left : number = balance - min(balance, amount)\n\ntable step\npolicy first\n| status  | act  | left | -> next_status | next_balance |\n| open    | pay  | 0    | settled        | 0            |\n| open    | pay  | -    | status         | left         |\n| open    | wait | -    | status         | balance      |\n| settled | -    | -    | status         | balance      |\n\nmachine loan over step\n  carry   status -> next_status\n  carry   balance -> next_balance\n  initial open\n  initial balance 6\n  final   settled\n\nexamples\n| status | act | balance | amount | -> next_status | next_balance |\n| open   | pay | 6       | 3      | open           | 3            |\n| open   | pay | 3       | 3      | settled        | 0            |\n";

/// A stock halved on each tick and rounded down: from 5 the half is 2.5, which the modes round apart,
/// so no path is laid and the claim stays stated rather than resting on one mode.
const HALVING: &str = "rule halving v1\ndescription \"A stock halved on each tick, rounded down, until nothing is left\"\n\nenum phase = running | stopped\n\ninputs\n  phase : phase\n  units : number  range >=0 <=10\n\noutputs\n  next_phase : phase\n  next_units : number  round down(1)\n\ndefine half : number = units * 50%\n\ntable step\npolicy unique\n| phase   | units | -> next_phase | next_units |\n| running | >=2   | phase         | half       |\n| running | <=1   | stopped       | 0          |\n| stopped | -     | phase         | units      |\n\nmachine stock over step\n  carry   phase -> next_phase\n  carry   units -> next_units\n  initial running\n  initial units 10\n  final   stopped\n\nexamples\n| phase   | units | -> next_phase | next_units |\n| running | 10    | running       | 5          |\n";

/// That a case can still finish, for a machine that carries more than its state (rulec's §15.217,
/// §15.233): where every carried number was walked one value at a time, the certificate gives a path
/// of calls that really happen from every place not in a final state, and the Lean checker runs each
/// call (`Carried.Model.fires_of_callOk`, `Carried.Model.reachable_finishes`). The four rules of the
/// corpus that have a final state prove it; the mutant walked in intervals still names it as stated,
/// as `check` itself leaves it open (W127). A path broken in one place is refused by both checkers.
#[test]
fn 持ち越す値のある機械が終われることは証明付きの検査器で証明される() {
    let Some(bin) = checker() else {
        return;
    };
    for rel in ["tests/corpus/grace_period.rule", "tests/corpus/retry_attempts.rule", "tests/corpus/猶予の残り日数.rule", "tests/corpus/再試行の回数.rule"] {
        let (c, cert) = rulec(&["certificate", rel]);
        assert_eq!(c, 0, "{rel}: {cert}");
        assert!(cert.contains(r#""finish":[{"place":"#), "{rel}: 終わりまでの呼び出しの並びが無い");
        let (code, said) = lean(&bin, &cert, Some(rel));
        assert_eq!(code, 0, "{rel}: {said}");
        assert!(said.contains("every place a case can be can still finish"), "{rel}: {said}");
        assert!(said.contains("OK: every claim this program states was proved"), "{rel}: {said}");
        if let Some((code, said)) = python(&cert, rel) {
            assert_eq!(code, 0, "{rel}: {said}");
            assert!(said.contains("every claim this program states was proved"), "{rel}: {said}");
        }
    }
    let rel = "tests/mutants/m_w127.rule";
    let (c, cert) = rulec(&["certificate", rel]);
    assert_eq!(c, 0, "{cert}");
    assert!(!cert.contains(r#""finish":[{"place":"#), "{cert}");
    let (code, said) = lean(&bin, &cert, Some(rel));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("that a case can still finish is not laid on the rows (the machine carries more than its state)"), "{said}");
    // A computed column under `first`, and an input no column reads.
    let tmp = TempDir::new("lean-finish");
    let p = tmp.path().join("loan.rule");
    std::fs::write(&p, LOAN).unwrap();
    let loan = p.to_str().unwrap().to_string();
    let (c, cert) = rulec(&["certificate", &loan]);
    assert_eq!(c, 0, "{cert}");
    let (code, said) = lean(&bin, &cert, Some(&loan));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("8 places a case can be, no call leaves a final state, next_balance within its range, every place a case can be can still finish"), "{said}");
    if let Some((code, said)) = python(&cert, &loan) {
        assert_eq!(code, 0, "{said}");
    }
    both_refuse(&bin, &cert, &loan, &[
        ("範囲の外の入力", cert.replacen(r#"{"row":1,"at":[0,0,1],"inputs":{"balance":"1","amount":"1"}}"#, r#"{"row":1,"at":[0,0,1],"inputs":{"balance":"1","amount":"4"}}"#, 1), "a place has no path of calls to a final state that checks"),
        ("前の行が取る点", cert.replacen(r#"{"row":1,"at":[0,0,1],"inputs":{"balance":"1","amount":"1"}}"#, r#"{"row":2,"at":[0,0,1],"inputs":{"balance":"1","amount":"1"}}"#, 1), "a place has no path of calls to a final state that checks"),
    ]);
    // A path that would rest on how a mode rounds 2.5 is not laid.
    let p = tmp.path().join("halving.rule");
    std::fs::write(&p, HALVING).unwrap();
    let halving = p.to_str().unwrap().to_string();
    let (c, cert) = rulec(&["certificate", &halving]);
    assert_eq!(c, 0, "{cert}");
    assert!(!cert.contains(r#""finish":[{"place":"#), "{cert}");
    let (code, said) = lean(&bin, &cert, Some(&halving));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("that a case can still finish is not laid on the rows (the machine carries more than its state)"), "{said}");
    let rel = "tests/corpus/grace_period.rule";
    let (_, cert) = rulec(&["certificate", rel]);
    both_refuse(&bin, &cert, rel, &[
        ("並びの呼び出しの行", cert.replacen(r#"{"row":1,"at":[0,0,0],"inputs":{"days_left":"0"}}"#, r#"{"row":2,"at":[0,0,0],"inputs":{"days_left":"0"}}"#, 1), "a place has no path of calls to a final state that checks"),
        ("呼び出しの数", cert.replacen(r#"{"place":[1,[7]],"path":[{"row":5,"at":[1,1,4],"inputs":{"days_left":"7"}}"#, r#"{"place":[1,[7]],"path":[{"row":5,"at":[1,1,4],"inputs":{"days_left":"6"}}"#, 1), "a place has no path of calls to a final state that checks"),
        ("座標の外の値", cert.replacen(r#"{"place":[1,[7]],"path":[{"row":5,"at":[1,1,4],"#, r#"{"place":[1,[7]],"path":[{"row":5,"at":[1,1,3],"#, 1), "a place has no path of calls to a final state that checks"),
        ("終わりに着かない並び", cert.replacen(r#"{"place":[1,[1]],"path":[{"row":6,"at":[1,1,1],"inputs":{"days_left":"1"}}]}"#, r#"{"place":[1,[1]],"path":[]}"#, 1), "a place has no path of calls to a final state that checks"),
        ("場所の並びを落とす", cert.replacen(r#",{"place":[1,[1]],"path":[{"row":6,"at":[1,1,1],"inputs":{"days_left":"1"}}]}"#, "", 1), "a place has no path of calls to a final state that checks"),
    ]);
}

/// Where a case starts, what a `once` line reads on each row, a move, and the words a row writes are
/// read back out of the file (rulec's §15.233): each `initial` line, and each answer cell in its own
/// place on the row's line. Pointed elsewhere — at the same text in another cell, or another value —
/// each is refused by both checkers.
#[test]
fn 始まりとonceの値と行の答えはファイルから読み戻される() {
    let Some(bin) = checker() else {
        return;
    };
    let rel = "tests/corpus/one_time_perk.rule";
    let (c, cert) = rulec(&["certificate", rel]);
    assert_eq!(c, 0, "{cert}");
    let (code, said) = lean(&bin, &cert, Some(rel));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("OK: every claim this program states was proved"), "{said}");
    both_refuse(&bin, &cert, rel, &[
        ("始まりの行", cert.replacen(r#""initial_source":{"line":34,"col":10,"len":5,"text":"guest"}"#, r#""initial_source":{"line":20,"col":2,"len":5,"text":"guest"}"#, 1), "machine: where a case starts is not read from an `initial` line"),
        ("始まりの状態", cert.replacen(r#""initial":0,"#, r#""initial":1,"#, 1), "machine: the `initial` line does not name member there"),
        ("onceが読む値", cert.replacen(r#"{"row":4,"value":true,"#, r#"{"row":4,"value":false,"#, 1), "machine: row 4 writes `true` into trial_granted, and the certificate reads another value for the `once` line"),
        ("持ち越す値の始まり", cert.replacen(r#""initial_source":{"line":35,"col":22,"len":5,"text":"false"}"#, r#""initial_source":{"line":20,"col":76,"len":5,"text":"false"}"#, 1), "carried values: where trial_taken starts is not read from its `initial` line"),
    ]);
    let rel = "tests/corpus/order_lifecycle.rule";
    let (c, cert) = rulec(&["certificate", rel]);
    assert_eq!(c, 0, "{cert}");
    both_refuse(&bin, &cert, rel, &[
        ("onceの値を別の行のセルで読む", cert.replacen(r#"{"row":1,"value":0,"source":{"line":20,"col":50,"len":4,"text":"0JPY"}}"#, r#"{"row":1,"value":0,"source":{"line":21,"col":50,"len":4,"text":"0JPY"}}"#, 1), "machine: the value `once refund` reads on row 1 is not read from the row's own cell of refund"),
        ("行き先を別の行のセルで読む", cert.replacen(r#"{"row":3,"stay":true,"source":{"line":22,"col":34,"len":5,"text":"state"}}"#, r#"{"row":3,"stay":true,"source":{"line":25,"col":34,"len":5,"text":"state"}}"#, 1), "machine: row 3's move is not read from the row's own cell of next_state"),
        ("行が書く語", cert.replacen(r#""produces":["paid",null,"true"]"#, r#""produces":["paid",null,"false"]"#, 1), "step: row 1, answer 3: the file writes `true` there, and the certificate reads it as `false`"),
    ]);
}

/// The claims the fourth stage added, proved (rulec's §15.243): the claims of `chances` counted box by
/// box over the rows (`Chances.Claim.holds`), run point by point (`Chances.RunClaim.holds`) and walked by
/// weight (`Chances.WeightedClaim.holds`), the range of an open value (`openValue_mem_stated`), and every
/// contribution held to the digest of its file. Each one broken in one place is refused by the Lean
/// checker and by `tools/recheck.py` alike.
#[test]
fn 第4段の主張は証明付きの検査器で証明される() {
    let Some(bin) = checker() else {
        return;
    };
    for (rel, line) in [
        ("tests/corpus/prize_lottery.rule", "chances: 6 of 6 claims proved — 8 classes counted box by box over the rows, 0 points run one by one"),
        ("tests/corpus/くじの景品.rule", "chances: 6 of 6 claims proved — 8 classes counted box by box over the rows, 0 points run one by one"),
        ("tests/corpus/reward_box.rule", "chances: 6 of 6 claims proved — 450 classes counted box by box over the rows, 0 points run one by one"),
        ("tests/corpus/invoice_sampling.rule", "chances: 6 of 6 claims proved — 40 classes counted box by box over the rows, 0 points run one by one"),
        ("tests/corpus/contested_roll.rule", "chances: 5 of 5 claims proved — 0 classes counted box by box over the rows, 25000 points run one by one"),
        ("tests/corpus/case_routing.rule", "chances: 7 of 7 claims proved — 0 classes counted box by box over the rows, 0 points run one by one, 70000 draws walked by weight"),
        ("tests/corpus/案件の振り分け.rule", "chances: 7 of 7 claims proved — 0 classes counted box by box over the rows, 0 points run one by one, 70000 draws walked by weight"),
    ] {
        let (c, cert) = rulec(&["certificate", rel]);
        assert_eq!(c, 0, "{rel}: {cert}");
        let (code, said) = lean(&bin, &cert, Some(rel));
        assert_eq!(code, 0, "{rel}: {said}");
        assert!(said.contains(line), "{rel}: {said}");
        assert!(said.contains("OK: every claim this program states was proved"), "{rel}: {said}");
    }
    // A claim the check left undecided is the one thing named.
    let (c, cert) = rulec(&["certificate", "tests/mutants/m_w129.rule"]);
    assert_eq!(c, 0, "{cert}");
    let (code, said) = lean(&bin, &cert, Some("tests/mutants/m_w129.rule"));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("OK, and one thing is stated rather than proved: chances 1 row 5: undecided within the budget (W129)."), "{said}");

    let rel = "tests/corpus/prize_lottery.rule";
    let (_, cert) = rulec(&["certificate", rel]);
    both_refuse(&bin, &cert, rel, &[
        ("区画の点の数", cert.replacen(r#""points":"48""#, r#""points":"49""#, 1), "a box holds 48 points"),
        ("引く軸の敷き詰めの隙間", cert.replacen(r#"{"point":"1"},"#, "", 1), "do not tile what their columns take"),
        ("区分の当たりの数", cert.replacen(r#""points":"1000","hits":"1""#, r#""points":"1000","hits":"2""#, 1), "a class's boxes add up to 1 of 1000, and the certificate says 2 of 1000"),
        ("主張の分数", cert.replacen(r#""chance":{"is":"1/1000"}"#, r#""chance":{"is":"1/999"}"#, 1), "does not pass the chance it claims"),
        ("区画の答え", cert.replacen(r#""outputs":{"prize":"first"},"hit":true"#, r#""outputs":{"prize":"second"},"hit":true"#, 1), "the rows answer prize = first in a box"),
        ("主張のセルの場所", cert.replacen(r#""chance":{"line":26,"col":24,"len":4,"text":"0.1%"}"#, r#""chance":{"line":27,"col":24,"len":4,"text":"0.1%"}"#, 1), "chances 1 row 1: its cells are not the cells of the line it is written on"),
    ]);
    // An axis that is not random cut coarser than the values its column takes: the claim would be
    // counted over fewer values than there are.
    let rel = "tests/corpus/invoice_sampling.rule";
    let (_, cert) = rulec(&["certificate", rel]);
    both_refuse(&bin, &cert, rel, &[(
        "引かない軸の刻み",
        cert.replacen(r#""column":"amount","random":false,"type":"money[JPY]","step":"1""#, r#""column":"amount","random":false,"type":"money[JPY]","step":"100000""#, 1),
        "chances: amount is cut every 100000, and its values sit on a step of 1",
    )]);
    let rel = "tests/corpus/contested_roll.rule";
    let (_, cert) = rulec(&["certificate", rel]);
    both_refuse(&bin, &cert, rel, &[
        ("一つずつ走らせた点の数", cert.replacen(r#""run":"200""#, r#""run":"199""#, 1), "200 points are run here, and the certificate says 199"),
        ("一つずつ走らせた確率", cert.replacen(r#""low":"3/5""#, r#""low":"2/5""#, 1), "the chances run here come to 3/5–3/5, and the certificate says 2/5–3/5"),
    ]);
    let rel = "tests/corpus/case_routing.rule";
    let (_, cert) = rulec(&["certificate", rel]);
    both_refuse(&bin, &cert, rel, &[
        ("並びの要素のセル", cert.replacen(r#"{"value":"50","source":{"line":35,"col":26,"len":2,"text":"50"}}"#, r#"{"value":"60","source":{"line":35,"col":26,"len":2,"text":"50"}}"#, 1), "the file writes `50`, and the certificate reads another value"),
        ("重みで選ぶ主張の分数", cert.replacen(r#""chance":{"is":"1/2"}"#, r#""chance":{"is":"1/3"}"#, 1), "does not pass the chance it claims"),
    ]);

    // The range of an open value, and the files of the contributions merged into the rule.
    let (c, cert) = rulec(&["certificate", "tests/corpus/contrib/order_discount/regional_campaign.rule"]);
    assert_eq!(c, 0, "{cert}");
    let rel = "tests/corpus/order_discount.rule";
    let (code, said) = lean(&bin, &cert, Some(rel));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("open values: 3 hold what their 6 terms come to together inside the range they declare, 3 of the terms from contributions"), "{said}");
    assert!(said.contains("the digests of the 1 contributions merged into it are their files'"), "{said}");
    assert!(said.contains("OK: every claim this program states was proved"), "{said}");
    let at = cert.find("\"contributions\":").expect("no contributions");
    let (head, tail) = cert.split_at(at);
    both_refuse(&bin, &cert, rel, &[
        ("合わせた値の区間", cert.replacen(r#""stackable":["0","100"]"#, r#""stackable":["0","10"]"#, 1), "stackable: its terms together come to [0, 15], outside the range [0, 10] it declares"),
        ("寄与の行のセル", cert.replacen(r#""text":"7USD""#, r#""text":"8USD""#, 1), "the file does not say"),
        ("寄与のファイルのハッシュ", format!("{head}{}", tail.replacen("614de68c", "000de68c", 1)), "contribution regional_campaign is read from another text than"),
    ]);
}

/// A rule whose open table a contribution adds a clause to that writes a word (§15.231, §15.243): the
/// table is written in two places, so the certificate names the cell each row writes its word in, and
/// both re-checkers read every word back out of its own file.
const CARRIER: &str = "rule carrier_pick(carrier_pick) v1\ndescription \"Which carrier takes a parcel\"\n\nenum carrier = post | truck | drone\n\ninputs\n  weight(weight) : number  range >=0 <=30\n\noutputs\n  carrier(carrier) : carrier\n\ntable by_weight open\npolicy unique\n| weight | -> carrier |\n| <=5    | post       |\n| >5     | truck      |\n\nexamples\n| weight | -> carrier |\n| 2      | post       |\n| 9      | truck      |\n";
const DRONES: &str = "contribution drones(drones) v1\ndescription \"Light parcels near the depot go by drone\"\nto \"../carrier.rule\" signature:a340e3857fd84c35\n\ninputs\n  near(near) : bool\n\nclause drone_light -> carrier\n  when near true and weight <=2\n  then drone\n  overrides by_weight\n\nexamples\n| weight | near | -> carrier |\n| 1      | true | drone      |\n";

#[test]
fn 寄与の行が書く語は寄与のファイルから読み戻される() {
    let Some(bin) = checker() else {
        return;
    };
    let t = TempDir::new("contributed-words");
    let d = t.path();
    std::fs::create_dir_all(d.join("contrib")).unwrap();
    std::fs::write(d.join("carrier.rule"), CARRIER).unwrap();
    std::fs::write(d.join("contrib/drones.rule"), DRONES).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(d).args(["certificate", "contrib/drones.rule"]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let cert = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(cert.contains(r#""answers":[{"line":10,"col":7,"len":5,"text":"drone"}]"#), "{cert}");
    let run = |cert: &str| -> (i32, String, Option<(i32, String)>) {
        use std::io::Write;
        use std::process::Stdio;
        let mut p = Command::new(&bin).current_dir(d).args(["--rule", "carrier.rule"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        p.stdin.as_mut().unwrap().write_all(cert.as_bytes()).unwrap();
        let o = p.wait_with_output().unwrap();
        let py = Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false).then(|| {
            let p = d.join("c.json");
            std::fs::write(&p, cert).unwrap();
            let o = Command::new("python3").current_dir(d).arg(root().join("tools/recheck.py")).arg(&p).args(["--rule", "carrier.rule"]).output().unwrap();
            (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
        });
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), py)
    };
    let (code, said, py) = run(&cert);
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("7 cells are read back out of it and the 1 file of its contributions"), "{said}");
    if let Some((code, said)) = py {
        assert_eq!(code, 0, "{said}");
        assert!(said.contains("7 cells read back from carrier.rule and the 1 file of its contributions"), "{said}");
    }
    let forged = cert.replacen(r#""produces":["drone"]"#, r#""produces":["truck"]"#, 1);
    assert_ne!(forged, cert);
    let (code, said, py) = run(&forged);
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("answer 1: the file writes `drone` there, and the certificate reads it as `truck`"), "{said}");
    if let Some((code, said)) = py {
        assert_eq!(code, 1, "{said}");
    }
}

/// A table that reads an open value (rulec's §15.227): its axis runs over the range the rule declares for
/// the value, as an input's does, and `opens` holds that range to what the terms come to together.
const OPEN_BAND: &str = "rule site_band(site_band) v1\ndescription \"The band of a storage site, from the extra capacity its equipment adds\"\n\ninputs\n  has_shelving(has_shelving) : bool\n  has_racking(has_racking)   : bool\n\noutputs\n  band(band) : number  round down(1)\n\ntable from_shelving\npolicy unique\n| has_shelving | -> shelving_extra : number |\n| true         | 10                         |\n| false        | 0                          |\n\ntable from_racking\npolicy unique\n| has_racking | -> racking_extra : number |\n| true        | 20                        |\n| false       | 0                         |\n\nopen extra : number  combine sum  range >=0 <=100\n  shelving_extra\n  racking_extra\n\ntable band_of\npolicy unique\n| extra | -> band |\n| <=20  | 1       |\n| >20   | 2       |\n\nexamples\n| has_shelving | has_racking | -> band |\n| true         | true        | 2       |\n| false        | false       | 1       |\n";

/// Both re-checkers tile the axis of an open value over the range the rule declares for it — the Lean
/// one's reason is `axisTilesB_cover` with `openValue_mem_stated` — and refuse an axis that stops short
/// of that range.
#[test]
fn 開いた値を読む表の軸は宣言した範囲を敷き詰める() {
    let Some(bin) = checker() else {
        return;
    };
    let t = TempDir::new("open-axis");
    let rule = t.write("site_band.rule", OPEN_BAND);
    let rule = rule.to_string_lossy().into_owned();
    let (c, cert) = rulec(&["certificate", &rule]);
    assert_eq!(c, 0, "{cert}");
    assert!(cert.contains(r#""column":"extra","kind":"open""#), "{cert}");
    let (code, said) = lean(&bin, &cert, Some(&rule));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("open values: 1 hold what their 2 terms come to together inside the range they declare"), "{said}");
    assert!(said.contains("OK: every claim this program states was proved"), "{said}");
    if let Some((code, said)) = python(&cert, &rule) {
        assert_eq!(code, 0, "{said}");
    }
    both_refuse(&bin, &cert, &rule, &[(
        "宣言した範囲に届かない軸",
        cert.replacen(r#""extra":["0","100"]"#, r#""extra":["0","200"]"#, 1),
        "band_of: extra does not run the whole of the range the rule declares",
    )]);
}

/// The file a rule's certificate is held to: the rule itself, or for a contribution the rule it comes
/// down to, which `rulec certificate` writes the certificate of (§15.231). From `base`.
fn held_to(base: &std::path::Path, rel: &str) -> String {
    let abs = base.join(rel).to_string_lossy().into_owned();
    match rulec::link::target_of(&abs) {
        Some((_, root)) if rulec::link::is_contribution_file(&abs) => std::path::Path::new(&root).strip_prefix(base).map(|p| p.to_string_lossy().into_owned()).unwrap_or(root),
        _ => rel.to_string(),
    }
}
