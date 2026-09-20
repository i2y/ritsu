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
    ] {
        assert_ne!(forged, cert, "{what}: 証明書の形が変わっていて、偽れていない");
        let (code, said) = lean(&bin, &forged, None);
        assert_eq!(code, 1, "{what}: 偽った証明書が通ってしまった\n{said}");
        assert!(said.contains("FAILED"), "{what}: 何が悪いか言っていない\n{said}");
    }
}

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
