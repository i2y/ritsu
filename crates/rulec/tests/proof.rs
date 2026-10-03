//! The proof harnesses beside the generated Rust (DESIGN §15.95).
//!
//! `gen` writes them for every rule; a model checker reads them. What is held here is the
//! wiring, not the arithmetic: that the file is written and says which harnesses are in it,
//! that `rulec api` names the same ones, that `--proofs` is what asks for the pass, and —
//! when `kani` is on the machine — that the pass really runs and comes back ok. The claim
//! itself, over the whole corpus, is the CI step and `experiments/kani/`.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rulec-proof-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(root()).args(args).output().expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

fn have(cmd: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {cmd} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// The rule W114 is written for: two derived values that share an input, so the checker
/// cannot decide whether rows 1 and 2 can both match and leaves a guard behind (§6.2).
const RULE: &str = "tests/corpus/クーポン併用.rule";

fn rows(json: &str) -> Vec<rulec::json::Json> {
    let j = rulec::json::parse(json.lines().next().expect("結果が無い")).unwrap();
    let rulec::json::Json::Arr(rs) = j.get("results").unwrap() else { panic!("{json}") };
    rs.clone()
}

/// The file is written, and what `rulec api` says is in it is what is in it.
#[test]
fn 証明のハーネスは生成され_apiの名前と一致する() {
    let out = dir("gen");
    let (c, said) = run(&["gen", RULE, "--out", out.to_str().unwrap()]);
    assert_eq!(c, 0, "{said}");
    let f = out.join("rust").join("coupon_stack_proof.rs");
    let body = std::fs::read_to_string(&f).expect("証明のファイルが無い");

    let (c, api) = run(&["api", RULE]);
    assert_eq!(c, 0, "{api}");
    let j = rulec::json::parse(api.trim()).unwrap();
    let rust = j.get("rust").expect("api に rust が無い");
    assert_eq!(rust.get("proof").and_then(|v| v.as_str()), Some("coupon_stack_proof.rs"), "{api}");
    let rulec::json::Json::Arr(hs) = rust.get("harnesses").expect("api に harnesses が無い") else {
        panic!("{api}")
    };
    assert!(!hs.is_empty(), "ハーネスが一つも名指しされていない: {api}");
    for h in hs {
        let name = h.as_str().unwrap();
        assert!(body.contains(&format!("fn {name}(")), "`api` が言う {name} がファイルに無い");
    }
    // Everything is behind the checker's own cfg, so an ordinary build never sees it.
    assert!(body.contains("#[cfg(kani)]"), "cfg(kani) の外に出ている");
    // The W114 guard is the reason this rule is here: the harness says it never fires.
    assert!(body.contains("is_ok()"), "答えが返ることを言っていない");
    let _ = std::fs::remove_dir_all(&out);
}

/// `rustc` compiles the file to nothing: the harnesses cost a reader of the generated code
/// nothing, and cost a build nothing.
#[test]
fn 証明のファイルは普通のビルドでは空である() {
    if !have("rustc") {
        eprintln!("skip: rustc が無い");
        return;
    }
    let out = dir("rustc");
    let (c, said) = run(&["gen", RULE, "--out", out.to_str().unwrap()]);
    assert_eq!(c, 0, "{said}");
    let o = Command::new("rustc")
        .current_dir(out.join("rust"))
        .args(["--edition", "2021", "--crate-type", "lib", "coupon_stack_proof.rs", "-o", "proof.rlib"])
        .output()
        .expect("rustc を起動できない");
    assert!(o.status.success(), "rustc が通らない:\n{}", String::from_utf8_lossy(&o.stderr));
    let _ = std::fs::remove_dir_all(&out);
}

/// The pass is asked for, not assumed: it is minutes where the vectors are milliseconds
/// (§15.95). Without the flag the run says nothing about proofs at all.
#[test]
fn フラグが無ければ証明は走らない() {
    let out = dir("noflag");
    let (c, said) = run(&["gen", RULE, "--out", out.to_str().unwrap()]);
    assert_eq!(c, 0, "{said}");
    let (_, said) = run(&["test", out.to_str().unwrap(), "--format", "json"]);
    assert!(
        rows(&said).iter().all(|r| r.get("via").and_then(|v| v.as_str()) != Some("proof")),
        "`--proofs` が無いのに証明が走った: {said}"
    );
    assert!(!said.contains("kani"), "`--proofs` が無いのに kani の話をしている: {said}");
    let _ = std::fs::remove_dir_all(&out);
}

/// With the flag and the checker, the pass runs and the generated Rust holds.
#[test]
fn フラグを付ければ証明が走る() {
    if !have("kani") || !have("rustc") {
        eprintln!("skip: kani が無い");
        return;
    }
    let out = dir("flag");
    let (c, said) = run(&["gen", RULE, "--out", out.to_str().unwrap()]);
    assert_eq!(c, 0, "{said}");
    let (code, said) = run(&["test", out.to_str().unwrap(), "--proofs", "--format", "json"]);
    assert_eq!(code, 0, "{said}");
    let p: Vec<rulec::json::Json> =
        rows(&said).into_iter().filter(|r| r.get("via").and_then(|v| v.as_str()) == Some("proof")).collect();
    assert_eq!(p.len(), 1, "証明は一つだけ走るはず: {said}");
    assert_eq!(p[0].get("lang").and_then(|v| v.as_str()), Some("rust"), "{said}");
    assert_eq!(p[0].get("ok"), Some(&rulec::json::Json::Bool(true)), "{said}");
    // `vectors` carries the number of harnesses for this pass, and there is more than one.
    assert!(p[0].get("vectors").and_then(|v| v.as_int()).unwrap_or(0) >= 2, "{said}");
    let _ = std::fs::remove_dir_all(&out);
}
