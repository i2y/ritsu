//! `--format json` on every command that reports something (docs/formats.md).
//!
//! What is pinned is the **shape**, not the numbers: the keys that have to be there, that
//! every line reads back as JSON, and that the keys do not move with `--lang`. An agent
//! drives rulec by these keys, so a key that disappears is a broken contract even when the
//! prose beside it improved.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

fn obj(line: &str) -> rulec::json::Json {
    rulec::json::parse(line.trim())
        .unwrap_or_else(|e| panic!("JSON として読めない: {e}\n{line}"))
}

/// Every non-empty line must be one complete JSON object.
fn objects(out: &str) -> Vec<rulec::json::Json> {
    out.lines().filter(|l| !l.trim().is_empty()).map(obj).collect()
}

fn keys(j: &rulec::json::Json, want: &[&str], what: &str) {
    for k in want {
        assert!(j.get(k).is_some(), "{what}: {k} が無い\n{j:?}");
    }
}

const TMP: &str = "rulec-formats";

fn gen_dir(tag: &str, rule: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("{TMP}-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = dir.to_string_lossy().to_string();
    let (c, _) = run(&["gen", rule, "--out", &out]);
    assert_eq!(c, 0);
    dir
}

#[test]
fn fmtは整形されていないファイルを名指しする() {
    let (c, out) = run(&["fmt", "--check", "tests/mutants/m_e102.rule", "--format", "json"]);
    assert_eq!(c, 1);
    let js = objects(&out);
    assert_eq!(js.len(), 1, "一件のはず");
    keys(&js[0], &["unformatted", "formatted"], "fmt");
    let rulec::json::Json::Arr(un) = js[0].get("unformatted").unwrap() else { panic!() };
    assert_eq!(un.len(), 1);
    assert!(un[0].as_str().unwrap().ends_with("m_e102.rule"));
    // A clean file names nothing, and still says so.
    let (c, out) = run(&["fmt", "--check", "tests/corpus/送料.rule", "--format", "json"]);
    assert_eq!(c, 0);
    let js = objects(&out);
    let rulec::json::Json::Arr(un) = js[0].get("unformatted").unwrap() else { panic!() };
    assert!(un.is_empty(), "{out}");
}

#[test]
fn genは古い生成物と欠けた生成物を分ける() {
    let dir = std::env::temp_dir().join(format!("{TMP}-gen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out_dir = dir.to_string_lossy().to_string();
    let rule = "tests/corpus/送料.rule";

    // Nothing generated yet: everything is missing, nothing is stale.
    let (c, out) = run(&["gen", rule, "--out", &out_dir, "--check", "--format", "json"]);
    assert_eq!(c, 1);
    let j = obj(&out);
    keys(&j, &["written", "stale", "missing"], "gen");
    let arr = |j: &rulec::json::Json, k: &str| -> usize {
        match j.get(k).unwrap() {
            rulec::json::Json::Arr(a) => a.len(),
            _ => panic!("{k} が配列でない"),
        }
    };
    assert!(arr(&j, "missing") > 0 && arr(&j, "stale") == 0 && arr(&j, "written") == 0, "{out}");

    // Generate, then edit one by hand: that one is stale, nothing is missing.
    let (c, out) = run(&["gen", rule, "--out", &out_dir, "--format", "json"]);
    assert_eq!(c, 0);
    assert!(arr(&obj(&out), "written") > 0);
    let py = dir.join("python").join("shipping_fee.py");
    let before = std::fs::read_to_string(&py).unwrap();
    std::fs::write(&py, format!("{before} ")).unwrap();
    let (c, out) = run(&["gen", rule, "--out", &out_dir, "--check", "--format", "json"]);
    assert_eq!(c, 1);
    let j = obj(&out);
    assert_eq!(arr(&j, "stale"), 1, "{out}");
    assert_eq!(arr(&j, "missing"), 0, "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn coverageは四基準を名前つきで出す() {
    let (c, out) = run(&["coverage", "tests/corpus/送料.rule", "--format", "json"]);
    assert_eq!(c, 0);
    let js = objects(&out);
    assert_eq!(js.len(), 1, "ファイル一つにつき一行");
    keys(&js[0], &["file", "vectors", "criteria"], "coverage");
    let rulec::json::Json::Arr(cs) = js[0].get("criteria").unwrap() else { panic!() };
    let names: Vec<&str> = cs.iter().map(|c| c.get("name").unwrap().as_str().unwrap()).collect();
    assert_eq!(names, ["row", "boundary_pair", "shadow_pair", "rounding_tie"], "基準の名前は英語固定");
    for c in cs {
        keys(c, &["name", "satisfied", "total", "missing"], "criterion");
    }
    // Two files, two lines.
    let (_, out) =
        run(&["coverage", "tests/corpus/送料.rule", "tests/corpus/期間区分.rule", "--format", "json"]);
    assert_eq!(objects(&out).len(), 2);
}

#[test]
fn testは言語ごとの結果と最初の食い違いを出す() {
    let dir = gen_dir("run", "tests/corpus/期間区分.rule");
    let (c, out) = run(&["test", dir.to_str().unwrap(), "--format", "json"]);
    assert_eq!(c, 0, "{out}");
    let j = obj(&out);
    keys(&j, &["results", "skipped"], "test");
    let rulec::json::Json::Arr(rs) = j.get("results").unwrap() else { panic!() };
    assert!(!rs.is_empty(), "{out}");
    for r in rs {
        keys(r, &["rule", "lang", "via", "vectors", "ok", "ran", "first_diff", "error"], "result");
        let via = r.get("via").unwrap().as_str().unwrap();
        assert!(via == "runner" || via == "mcp", "via が安定していない: {via}");
        let lang = r.get("lang").unwrap().as_str().unwrap();
        // The stable ids are src/backend.rs's; a language added there without a name here
        // used to fail for the wrong reason.
        assert!(
            rulec::backend::ids().contains(&lang),
            "言語の名前が安定していない: {lang}"
        );
        assert_eq!(r.get("ok").unwrap(), &rulec::json::Json::Bool(true));
        assert_eq!(r.get("ran").unwrap(), &rulec::json::Json::Bool(true));
        assert_eq!(r.get("first_diff").unwrap(), &rulec::json::Json::Null);
        assert_eq!(r.get("error").unwrap(), &rulec::json::Json::Null);
    }
    // The rounding helpers are reported under a stable id, not under a translated name.
    let rules: Vec<&str> = rs.iter().map(|r| r.get("rule").unwrap().as_str().unwrap()).collect();
    assert!(rules.contains(&"_round"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fixtures_lintは問題を種類つきで数える() {
    let dir = std::env::temp_dir().join(format!("{TMP}-fx-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Two records: one clean, one whose destination is not a prefecture.
    let good = r#"{"tag":"a","in":{"届け先":"東京都","重量":1000,"注文金額":5000,"会員":"一般"},"observed":{"送料":600}}"#;
    let bad = r#"{"tag":"b","in":{"届け先":"江戸","重量":1000,"注文金額":5000,"会員":"一般"},"observed":{"送料":600}}"#;
    let p = dir.join("r.jsonl");
    std::fs::write(&p, format!("{good}\n{bad}\n")).unwrap();
    let (c, out) = run(&[
        "fixtures",
        "lint",
        p.to_str().unwrap(),
        "tests/corpus/送料.rule",
        "--format",
        "json",
        "--lang",
        "en",
    ]);
    assert_eq!(c, 1, "{out}");
    let j = obj(&out);
    keys(&j, &["file", "records", "observed", "filled", "dropped", "problems"], "lint");
    let rulec::json::Json::Arr(ps) = j.get("problems").unwrap() else { panic!() };
    assert_eq!(ps.len(), 1, "{out}");
    keys(&ps[0], &["kind", "field", "count", "example", "what", "hint"], "problem");
    assert_eq!(ps[0].get("kind").unwrap().as_str(), Some("bad_input"));
    assert_eq!(ps[0].get("field").unwrap().as_str(), Some("届け先"));
    keys(ps[0].get("example").unwrap(), &["line", "tag"], "example");
    // The kind is the stable half; `what` is the prose half.
    let (_, ja) = run(&[
        "fixtures", "lint", p.to_str().unwrap(), "tests/corpus/送料.rule",
        "--format", "json", "--lang", "ja",
    ]);
    let jj = obj(&ja);
    let rulec::json::Json::Arr(ps2) = jj.get("problems").unwrap() else { panic!() };
    assert_eq!(ps[0].get("kind"), ps2[0].get("kind"), "kind が言語で変わっている");
    assert_ne!(ps[0].get("what"), ps2[0].get("what"), "what は文面なので変わるはず");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn verifyは一致率とクラスタを構造で出す() {
    let Some(dir) = adapter_dir() else { return };
    let (_, out) = verify_json(&dir, "1");
    let j = obj(&out);
    keys(
        &j,
        &["compared", "matched", "rate", "counterpart", "unanswered", "clusters", "excluded", "filled"],
        "verify",
    );
    assert_eq!(j.get("counterpart").unwrap().as_str(), Some("legacy@fake-1"));
    let rulec::json::Json::Arr(cs) = j.get("clusters").unwrap() else { panic!() };
    assert!(!cs.is_empty(), "{out}");
    for c in cs {
        keys(c, &["rows", "count", "delta", "witness", "suspect_rounding"], "cluster");
        let rulec::json::Json::Arr(rows) = c.get("rows").unwrap() else { panic!() };
        for r in rows {
            keys(r, &["table", "row"], "row");
        }
        keys(c.get("witness").unwrap(), &["in", "ours", "theirs"], "witness");
        // The delta is keyed by output name, so several outputs can move independently.
        let rulec::json::Json::Obj(d) = c.get("delta").unwrap() else { panic!() };
        for v in d.values() {
            keys(v, &["min", "max", "uniform", "total"], "delta");
        }
    }
    // A seeded difference smaller than the output grid is tagged, and the tag is a boolean.
    let (_, out2) = verify_json(&dir, "2");
    let rulec::json::Json::Arr(cs2) = obj(&out2).get("clusters").unwrap().clone() else { panic!() };
    assert!(
        cs2.iter().all(|c| c.get("suspect_rounding") == Some(&rulec::json::Json::Bool(true))),
        "{out2}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The same fake legacy implementation `tests/verify.rs` uses: the generated Python, with an
/// optional defect. Returns None when python3 is not installed.
fn adapter_dir() -> Option<PathBuf> {
    let ok = Command::new("python3")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !ok {
        return None;
    }
    let dir = std::env::temp_dir().join(format!("{TMP}-vf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = dir.to_string_lossy().to_string();
    let (c, _) = run(&["gen", "tests/corpus/ゆうパック運賃.rule", "--out", &out]);
    assert_eq!(c, 0);
    let src = std::fs::read_to_string(root().join("tests/verify.rs")).unwrap();
    let body = src.split("const ADAPTER: &str = r#\"").nth(1).unwrap().split("\"#;").next().unwrap();
    std::fs::write(dir.join("adapter.py"), body).unwrap();
    Some(dir)
}

fn verify_json(dir: &PathBuf, bug: &str) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(["verify", "tests/corpus/ゆうパック運賃.rule", "--format", "json", "--adapter", "python3"])
        .arg(dir.join("adapter.py"))
        .env("BUG", bug)
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

#[test]
fn 鍵は言語で変わらない() {
    // One sweep over every command that has `--format json`: the set of keys is the contract,
    // so it must be identical in both languages, top level and one level down.
    let dir = gen_dir("keys", "tests/corpus/期間区分.rule");
    let d = dir.to_string_lossy().to_string();
    let cases: Vec<Vec<&str>> = vec![
        vec!["check", "tests/mutants/m_e101.rule", "--format", "json"],
        vec!["fmt", "--check", "tests/mutants/m_e102.rule", "--format", "json"],
        vec!["gen", "tests/corpus/送料.rule", "--out", &d, "--check", "--format", "json"],
        vec!["coverage", "tests/corpus/送料.rule", "--format", "json"],
        vec!["test", &d, "--format", "json"],
        vec!["explain", "--all", "--format", "json"],
    ];
    for case in &cases {
        let mut ja: Vec<&str> = case.clone();
        ja.extend(["--lang", "ja"]);
        let mut en: Vec<&str> = case.clone();
        en.extend(["--lang", "en"]);
        let (_, a) = run(&ja);
        let (_, b) = run(&en);
        let shape = |s: &str| -> Vec<Vec<String>> {
            objects(s)
                .iter()
                .map(|j| match j {
                    rulec::json::Json::Obj(m) => m.keys().cloned().collect(),
                    _ => Vec::new(),
                })
                .collect()
        };
        assert_eq!(shape(&a), shape(&b), "鍵が言語で変わる: rulec {}", case.join(" "));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// docs/formats.md is the definition these tests are written against; it must name every
/// command that has the flag.
#[test]
fn 形式の文書が全コマンドを載せている() {
    let doc = std::fs::read_to_string(root().join("docs/formats.md")).unwrap();
    for cmd in ["check", "explain", "fmt", "gen", "coverage", "test", "verify", "replay", "diff", "fixtures lint"] {
        assert!(doc.contains(&format!("## `{cmd}`")) || doc.contains(cmd), "docs/formats.md に {cmd} が無い");
    }
}

fn have(cmd: &str) -> bool {
    ["--version", "version"]
        .iter()
        .any(|a| Command::new(cmd).arg(a).output().map(|o| o.status.success()).unwrap_or(false))
}

/// A run that never happened must not be reported as a disagreement.
///
/// Both used to print under `disagrees with the reference evaluator`, which tells a reader the
/// comparison ran and came out badly when it never ran at all — a missing toolchain, a compile
/// error, a process that died. `ran` is the field to branch on, and the heading splits with it.
#[test]
fn 走らなかった実行は食い違いとして報告されない() {
    if !have("python3") {
        eprintln!("注意: python3 が無いので飛ばした");
        return;
    }
    let dir = gen_dir("broken", "tests/corpus/期間区分.rule");
    // Break the module the runner imports, so the run cannot produce a line to compare.
    let module = dir.join("python").join("period.py");
    std::fs::write(&module, "this is not python\n").unwrap();

    let (c, out) = run(&["test", dir.to_str().unwrap()]);
    assert_eq!(c, 1, "{out}");
    assert!(out.contains("could not be run") || out.contains("走らせられません"), "{out}");
    assert!(
        !out.contains("disagrees with the reference evaluator") && !out.contains("食い違います"),
        "走らなかった実行が食い違い扱いのまま: {out}"
    );

    let (_, out) = run(&["test", dir.to_str().unwrap(), "--format", "json"]);
    let j = obj(&out);
    let rulec::json::Json::Arr(rs) = j.get("results").unwrap() else { panic!() };
    let py = rs.iter().find(|r| r.get("lang").unwrap().as_str().unwrap() == "python").unwrap();
    assert_eq!(py.get("ok").unwrap(), &rulec::json::Json::Bool(false));
    assert_eq!(py.get("ran").unwrap(), &rulec::json::Json::Bool(false));
    assert_eq!(py.get("first_diff").unwrap(), &rulec::json::Json::Null);
    assert!(py.get("error").unwrap().as_str().is_some(), "prose が error に無い: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A language skipped for a missing toolchain narrows what the run proved, so the summary says
/// so and `--require-all` refuses to call it green.
///
/// Without that, CI passes on one language out of six while the claim being made is that the
/// reference evaluator and **every** generated language agree.
#[test]
fn 飛ばした言語があると要求時に落ちる() {
    if !have("python3") {
        eprintln!("注意: python3 が無いので飛ばした");
        return;
    }
    let dir = gen_dir("skipped", "tests/corpus/期間区分.rule");
    // A PATH holding python3 and nothing else, so the other five are skipped.
    let bin = dir.join("_bin");
    std::fs::create_dir_all(&bin).unwrap();
    let py = String::from_utf8_lossy(
        &Command::new("sh").args(["-c", "command -v python3"]).output().unwrap().stdout,
    )
    .trim()
    .to_string();
    std::os::unix::fs::symlink(&py, bin.join("python3")).unwrap();

    let run_with_path = |extra: &[&str]| -> (i32, String) {
        let mut args = vec!["test", dir.to_str().unwrap()];
        args.extend_from_slice(extra);
        let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
            .current_dir(root())
            .args(&args)
            .env("PATH", &bin)
            .output()
            .expect("rulec を起動できない");
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
    };

    let (c, out) = run_with_path(&[]);
    assert_eq!(c, 0, "既定は寛容なまま: {out}");
    assert!(
        out.contains("languages skipped") || out.contains("言語を飛ばしました"),
        "要約が範囲を言っていない: {out}"
    );

    let (c, out) = run_with_path(&["--require-all"]);
    assert_eq!(c, 1, "--require-all が飛ばしを見逃した: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}
