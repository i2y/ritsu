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

/// A pair the elimination decides is proved in the certificate: the multipliers travel with
/// it, and the re-checker adds them up (§15.141). A pair it cannot decide — W114 — is stated
/// as undecided, not as proved.
#[test]
fn 消去で決めた対は乗数つきで書かれ_決められない対は未決と書かれる() {
    let (c, cert) = rulec(&["certificate", "tests/corpus/クーポン併用.rule"]);
    assert_eq!(c, 0, "{cert}");
    assert!(cert.contains(r#""refuted":[{"a":1,"b":2,"farkas":"#), "消去で決めた対が乗数つきで出ていない:\n{cert}");
    assert!(cert.contains(r#""undecided":[]"#), "{cert}");
    if have_python() {
        let (code, said) = recheck(&cert);
        assert_eq!(code, 0, "{said}");
        assert!(said.contains("2 pairs disjoint + 1 apart on the linear model"), "{said}");
    }
    let (c, cert) = rulec(&["certificate", "tests/mutants/m_w114.rule"]);
    assert_eq!(c, 0, "{cert}");
    assert!(cert.contains(r#""undecided":[{"a":1,"b":2}]"#), "W114 の対が未決として出ていない:\n{cert}");
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
        // The last row's point dropped, so row 23 is left with none.
        ("行の証人を落とす", {
            let at = cert.find(r#",{"row":23,"at":[0,1,23]"#).expect("行 23 の点が無い");
            let end = cert[at..].find("}]").map(|i| at + i + 1).expect("点の終わりが無い");
            format!("{}{}", &cert[..at], &cert[end..])
        }),
        // A row whose point sits outside its own box.
        ("証人を箱の外へ", cert.replace(r#""row":3,"at":[0,0,2]"#, r#""row":3,"at":[0,0,0]"#)),
    ] {
        assert_ne!(forged, cert, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = recheck(&forged);
        assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった\n{said}");
        assert!(said.contains("FAILED"), "{what}: 何が悪いか言っていない\n{said}");
    }
}

/// The leaf that rests on the tables above, and the four ways of lying about it.
///
/// `Cover::ByUpstream` used to be the one leaf neither re-checker looked at: it said "the
/// tables above cannot produce this box" and was counted among the things stated rather
/// than proved, which made it the one place a forged certificate could hide a real
/// completeness gap. Since §15.115 the reason is carried on the table — a value no table
/// above ever writes, or two coordinates whose spans on a shared input do not meet — and
/// the reason is **recomputed here** from the rows of the tables that decide the columns.
///
/// So this test is the other way round from the one it replaces. The honest certificate is
/// proved rather than stated, and each of the four lies is refused.
#[test]
fn 上流由来の葉は_事実から組み直される() {
    if !have_python() {
        eprintln!("skip: python3 が無い");
        return;
    }
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
    let (code, said) = recheck(&cert);
    assert_eq!(code, 0, "そのままの証明書が通らない:\n{said}");
    assert!(
        said.contains("impossible for the tables above"),
        "上流由来の葉が、証明された側に数えられていない:\n{said}"
    );
    assert!(
        !said.contains("rest on a table above"),
        "まだ「述べただけ」の側に残っている:\n{said}"
    );

    for (what, forged) in forgeries {
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

/// The certificate quotes the file: every cell is read back out of the `.rule` text at the
/// byte span the certificate names, and the box a row states is the one its own cell
/// describes. A quote that has been edited, a span that has been moved, and a row number
/// used twice all have to fail (§15.97).
#[test]
fn 証明書はファイルを引用する() {
    if !have_python() {
        eprintln!("skip: python3 が無い");
        return;
    }
    let (c, cert) = rulec(&["certificate", "tests/corpus/印紙税.rule"]);
    assert_eq!(c, 0, "{cert}");
    let run = |text: &str| {
        use std::io::Write;
        let mut p = Command::new("python3")
            .current_dir(root())
            .args(["tools/recheck.py", "--rule", "tests/corpus/印紙税.rule"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("python3 を起動できない");
        p.stdin.as_mut().unwrap().write_all(text.as_bytes()).unwrap();
        let o = p.wait_with_output().unwrap();
        (
            o.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr),
        )
    };
    let (code, said) = run(&cert);
    assert_eq!(code, 0, "そのままの証明書が通らない:\n{said}");
    assert!(said.contains("cells read back from it"), "セルを読み戻していない:\n{said}");

    for (what, forged) in [
        // The quote says something the file does not.
        ("引用を書き換える", cert.replace(r#""len":8,"text":"<1万円""#, r#""len":8,"text":"<2万円""#)),
        // The span is moved to another place on the line.
        ("引用の場所をずらす", cert.replace(r#""line":19,"col":30,"len":8"#, r#""line":19,"col":31,"len":8"#)),
        // Two rows with one number leaves the pair between them unexamined.
        ("行番号をぶつける", cert.replacen(r#"{"row":3,"label""#, r#"{"row":2,"label""#, 1)),
        // A row in the middle said to be written in another table, which would then be
        // written among this one's rows.
        ("出どころを偽る", {
            let pat = r#""origin":"税額""#;
            let at = cert.match_indices(pat).nth(11).map(|(i, _)| i).expect("行が足りない");
            let mut f = cert.clone();
            f.replace_range(at..at + pat.len(), r#""origin":"別の表""#);
            f
        }),
    ] {
        assert_ne!(forged, cert, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = run(&forged);
        assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった\n{said}");
        assert!(said.contains("FAILED"), "{what}: 何が悪いか言っていない\n{said}");
    }
}

/// The point that reaches a row has to be one the sieve admits, and the certificate carries
/// the values behind it so that can be checked at all.
#[test]
fn 到達の点は篩を通る() {
    if !have_python() {
        eprintln!("skip: python3 が無い");
        return;
    }
    let dir = std::env::temp_dir().join(format!("rulec-cert-sieve-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("con.rule");
    std::fs::write(&p, BY_CONSTRAINT).unwrap();
    let (c, cert) = rulec(&["certificate", p.to_str().unwrap()]);
    assert_eq!(c, 0, "{cert}");
    assert!(cert.contains(r#""at_values""#), "点の値が出ていない:\n{cert}");
    assert_eq!(recheck(&cert).0, 0, "そのままの証明書が通らない");

    // The point for row 3 moved into the open coordinate on both axes — where every value
    // is one the axis really takes — and given values the `constraint` forbids. Only the
    // constraint can catch this; the coordinates admit both numbers.
    let forged = cert.replace(
        r#"{"row":3,"at":[1,1],"values":{"全条件一致数":1,"会社名一致数":1},"at_values":["1","1"],"extra_values":[]}"#,
        r#"{"row":3,"at":[2,2],"values":{"全条件一致数":500,"会社名一致数":3},"at_values":["500","3"],"extra_values":[]}"#,
    );
    assert_ne!(forged, cert, "証明書の形が変わっていて、偽れていない");
    let (code, said) = recheck(&forged);
    assert_eq!(code, 1, "制約を破る点が通ってしまった:\n{said}");
    assert!(said.contains("does not hold at"), "制約ではなく別の検査が落としている:\n{said}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The span discipline (§15.99). A cell is read back from **its own place** in the file:
/// a row is one line, its cells are that line's own cells, and a row that says it has
/// none is one an `apply` brought in. Each of these was a way to pass a rule with a gap.
#[test]
fn 引用は行と桁に縛られる() {
    if !have_python() {
        eprintln!("skip: python3 が無い");
        return;
    }
    let (c, cert) = rulec(&["certificate", "tests/corpus/送料.rule"]);
    assert_eq!(c, 0, "{cert}");
    let run = |text: &str| {
        use std::io::Write;
        let mut p = Command::new("python3")
            .current_dir(root())
            .args(["tools/recheck.py", "--rule", "tests/corpus/送料.rule"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("python3 を起動できない");
        p.stdin.as_mut().unwrap().write_all(text.as_bytes()).unwrap();
        let o = p.wait_with_output().unwrap();
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
    };
    assert_eq!(run(&cert).0, 0, "そのままの証明書が通らない");

    for (what, forged) in [
        // A span of no length reads back as the empty string, which parses as a don't-care.
        ("長さ 0 の引用", cert.replacen(r#"{"line":25,"col":19,"len":6,"text":">2000g"}"#, r#"{"line":25,"col":19,"len":0,"text":""}"#, 1)),
        // A row that says it is written nowhere is one an `apply` brought in.
        ("行ごと無かったことにする", cert.replacen(r#""source":[{"line":25,"col":19,"len":6,"text":">2000g"},{"line":25,"col":2,"len":9,"text":"遠隔地"}]"#, r#""source":null"#, 1)),
        // A span pointed at another row's identical text.
        ("別の行の同じ字を指す", cert.replacen(r#"{"line":26,"col":19,"len":7,"text":"<=2000g"}"#, r#"{"line":24,"col":19,"len":7,"text":"<=2000g"}"#, 1)),
    ] {
        assert_ne!(forged, cert, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = run(&forged);
        assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった\n{said}");
        assert!(said.contains("FAILED"), "{what}: 何が悪いか言っていない\n{said}");
    }
}

/// The universe itself. A coordinate quietly removed leaves a gap under it that nothing
/// covers, and a cell literal read as a number the axis has no boundary for is the
/// certificate reading the file as something it does not say (§15.99).
#[test]
fn 軸は宣言範囲を敷き詰める() {
    if !have_python() {
        eprintln!("skip: python3 が無い");
        return;
    }
    let (c, cert) = rulec(&["certificate", "tests/corpus/印紙税.rule"]);
    assert_eq!(c, 0, "{cert}");
    assert_eq!(recheck(&cert).0, 0, "そのままの証明書が通らない");

    for (what, forged) in [
        // A boundary the cell does not write.
        ("境目でない数に読む", cert.replacen(r#"{"op":"<","value":"10000"}"#, r#"{"op":"<","value":"50000"}"#, 1)),
        // The grid the coordinates sit on, without which a gap looks like two neighbours.
        ("刻みを隠す", cert.replacen(r#""step":"1""#, r#""step":null"#, 1)),
        // A cover that is not stated is not a cover.
        ("覆いを述べない", cert.replacen(r#""cover":{"split""#, r#""cover":null,"x":{"split""#, 1)),
    ] {
        assert_ne!(forged, cert, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = recheck(&forged);
        assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった\n{said}");
        assert!(said.contains("FAILED"), "{what}: 何が悪いか言っていない\n{said}");
    }
}

/// What the certificate does **not** cover is said out loud, because a certificate that
/// looks complete is worse than one that names its edges (§15.47).
#[test]
fn 証明書は自分の届く先を言う() {
    let doc = std::fs::read_to_string(root().join("docs/formats.md")).unwrap();
    let cut = doc.find("## `certificate`").expect("formats.md に certificate の節が無い");
    let sect = &doc[cut..doc[cut + 4..].find("\n## ").map(|i| cut + 4 + i).unwrap_or(doc.len())];
    let low = sect.to_lowercase();
    for want in ["completeness", "int64", "recheck.py", "undecided", "does not pass `check`", "units", "--rule", "`contracts`", "`enums`", "included_sound"] {
        assert!(low.contains(want), "formats.md の certificate の節が `{want}` を言っていない");
    }
}

/// The two rules of a constraint chain through an input no table has a column for: one
/// complete only with the chain, one unique only with it (§15.141).
const CHAIN_GAP: &str = "rule 連なる制約(chain) v1\n\ninputs\n  a(a) : money[円]  range >=0円 <=100円\n  b(b) : money[円]  range >=0円 <=100円\n  x(x) : money[円]  range >=0円 <=100円\n\nconstraint a <= b\nconstraint b <= x\n\noutputs\n  y(y) : bool\n\ntable 表(t)\npolicy unique\n| a      | x     | -> y  |\n| <=50円 | -     | true  |\n| >50円  | >50円 | false |\n";
const CHAIN_OVERLAP: &str = "rule 連なる制約(chain) v1\n\ninputs\n  a(a) : money[円]  range >=0円 <=100円\n  b(b) : money[円]  range >=0円 <=100円\n  x(x) : money[円]  range >=0円 <=100円\n\nconstraint a <= b\nconstraint b <= x\n\noutputs\n  y(y) : bool\n\ntable 表(t)\npolicy unique\n| a      | x      | -> y  |\n| >50円  | -      | true  |\n| -      | <50円  | false |\n| <=50円 | >=50円 | false |\n";

fn chain_cert(tag: &str, src: &str) -> String {
    let dir = std::env::temp_dir().join(format!("rulec-cert-chain-{tag}-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("chain.rule");
    std::fs::write(&p, src).unwrap();
    let (c, cert) = rulec(&["certificate", p.to_str().unwrap()]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(c, 0, "{cert}");
    cert
}

/// The multipliers of the linear model travel in the certificate, and the re-checker adds
/// them up — for a box of the cover no row takes, and for a pair the axes do not part. A
/// multiplier changed is a sum that no longer cancels, and the certificate fails (§15.141).
#[test]
fn 線形のモデルの乗数は再検査で足し算される() {
    if !have_python() {
        eprintln!("skip: python3 が無い");
        return;
    }
    let gap = chain_cert("gap", CHAIN_GAP);
    assert!(gap.contains(r#"{"farkas":["#), "被覆の葉に乗数が無い:\n{gap}");
    assert!(gap.contains(r#""extra":["b"]"#), "列でない名前がモデルに無い:\n{gap}");
    let (code, said) = recheck(&gap);
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("impossible by the linear model"), "{said}");
    let forged = gap.replacen(r#"{"fact":6,"y":"#, r#"{"fact":5,"y":"#, 1);
    assert_ne!(forged, gap, "証明書の形が変わっていて、偽れていない");
    let (code, said) = recheck(&forged);
    assert_eq!(code, 1, "別の事実を指した乗数が通ってしまった:\n{said}");

    let overlap = chain_cert("overlap", CHAIN_OVERLAP);
    assert!(overlap.contains(r#""refuted":[{"a":1,"b":2,"farkas":"#), "消去で離れた対が出ていない:\n{overlap}");
    let (code, said) = recheck(&overlap);
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("1 apart on the linear model"), "{said}");
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

/// Feed one certificate to the re-checker with the rule it is about.
fn recheck_rule(cert: &str, rule: &str) -> (i32, String) {
    use std::io::Write;
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
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr),
    )
}

/// What a contract lets through is held to the rule's door (§15.142). The door is built
/// again from the rule, each case of the contract has to keep each thing it asks, and the
/// contract has to be the text the certificate names. A lie fails; a door the certificate
/// leaves unproved is said out loud rather than passed.
#[test]
fn 契約の関係は再検査で確かめられる() {
    if !have_python() {
        eprintln!("skip: python3 が無い");
        return;
    }
    let rule = "tests/corpus/速達の見積.rule";
    let (c, cert) = rulec(&["certificate", rule]);
    assert_eq!(c, 0, "{cert}");
    let (code, said) = recheck_rule(&cert, rule);
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("contract 見積: 7 of 7 things the door asks hold in all 2 cases"), "{said}");
    assert!(said.contains("the digest of contract 見積 is tests/corpus/contracts/quote.proto's"), "{said}");
    for (what, forged, fails) in contract_lies(&cert) {
        assert_ne!(forged, cert, "{what}: 偽れていない");
        let (code, said) = recheck_rule(&forged, rule);
        if fails {
            assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった:\n{said}");
            assert!(said.contains("FAILED"), "{what}: 何が悪いか言っていない:\n{said}");
        } else {
            assert_eq!(code, 0, "{what}: {said}");
            assert!(
                said.contains("contract 見積: the low end of 申告額's range is not shown to hold"),
                "{what}: 示していない条件を言っていない:\n{said}"
            );
        }
    }
    let (code, said) = recheck_rule(&with_a_clashing_case(&cert), rule);
    assert_eq!(code, 0, "食い違う場合の証明が通らない:\n{said}");
    assert!(said.contains("hold in all 3 cases"), "{said}");

    // An enum input: the strings a case lets it be have to be the enum's.
    let rule = "tests/corpus/出荷の送料.rule";
    let (c, cert) = rulec(&["certificate", rule]);
    assert_eq!(c, 0, "{cert}");
    assert!(cert.contains(r#"{"member":"あて先","proofs":[{"within":true},{"within":true}]}"#), "{cert}");
    assert_eq!(recheck_rule(&cert, rule).0, 0);
    let wider = cert.replacen(r#""values":["honshu","hokkaido","okinawa"],"in":true"#, r#""values":["honshu","hokkaido","okinawa","kyushu"],"in":true"#, 1);
    assert_ne!(wider, cert);
    let (code, said) = recheck_rule(&wider, rule);
    assert_eq!(code, 1, "列挙にない値を通す契約が通ってしまった:\n{said}");
}


/// A set on a column of numbers travels as its values (`{"cell":"in"}`), and the re-checker
/// builds the box from them: a box widened past the cell is refused (§15.143).
#[test]
fn 数の集合の箱は値から組み直される() {
    if !have_python() {
        eprintln!("skip: python3 が無い");
        return;
    }
    let dir = std::env::temp_dir().join(format!("rulec-cert-set-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("pieces.rule");
    std::fs::write(&p, "rule 個数の割引(pieces) v1\n\ninputs\n  個数(n) : number  range >=1 <=500\n\noutputs\n  割引(off) : money[円]  round down(1円)\n\ntable 割引表(t)\npolicy first\n| 個数         | -> 割引 |\n| 100, 200     | 500円   |\n| not: 300, 400 | 100円   |\n| -            | 0円     |\n").unwrap();
    let (c, cert) = rulec(&["certificate", p.to_str().unwrap()]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(c, 0, "{cert}");
    assert!(cert.contains(r#"{"cell":"in","values":["100","200"]}"#), "{cert}");
    assert!(cert.contains(r#"{"cell":"not_in","values":["300","400"]}"#), "{cert}");
    let (code, said) = recheck(&cert);
    assert_eq!(code, 0, "{said}");
    let at = cert.find(r#""cell":"in""#).unwrap();
    let acc = cert[at..].find(r#""accepts":[["#).unwrap() + at + r#""accepts":[["#.len();
    let mut wide = cert.clone();
    wide.insert_str(acc, "0,");
    let (code, said) = recheck(&wide);
    assert_eq!(code, 1, "広げた箱が通ってしまった:\n{said}");
}

/// The certificate says which shape it is, and a re-checker refuses one it was not written for
/// rather than checking something else (§15.156). An absent `v` is the first shape, as it is
/// for every output of rulec.
#[test]
fn 知らない形式の版の証明書は断る() {
    if !have_python() {
        eprintln!("skip: python3 が無い");
        return;
    }
    let (c, cert) = rulec(&["certificate", "tests/corpus/印紙税.rule"]);
    assert_eq!(c, 0, "{cert}");
    assert!(cert.starts_with(r#"{"v":1,"#), "証明書が形式の版を名乗らない: {}", &cert[..60.min(cert.len())]);
    let later = cert.replacen(r#"{"v":1,"#, r#"{"v":2,"#, 1);
    let (code, said) = recheck(&later);
    assert_eq!(code, 2, "版 2 の証明書を読んでしまった\n{said}");
    assert!(said.contains("format version"), "{said}");
    let unmarked = cert.replacen(r#"{"v":1,"#, "{", 1);
    assert_eq!(recheck(&unmarked).0, 0, "v の無い証明書を版 1 として読まない");
}
