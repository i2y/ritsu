//! `ritsu check` (DESIGN 8.1, 8.3, 8.4; PLAN E.2): every file of a project checked by its
//! language's own `check`, in the order the languages are checked, the text with the tool's word in
//! each headline, the JSON one object, and the exit code. What it says of each file is what the
//! language's command says of it, word for word, which the test holds it to by running each
//! language's command on the same files.

use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::process::Command;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// `ritsu`, run in `dir`, with no language asked of the environment.
fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).args(args);
    for v in ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG"] {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

const TOOLS: [&str; 8] = ["rulec", "dandori", "koyomi", "chobo", "geas", "yuen", "sakai", "ritsu"];

/// The text without the tools' words in the headlines: `[rulec E032]` is `[E032]` again.
fn without_tools(s: &str) -> String {
    let mut out = s.to_string();
    for t in TOOLS {
        for c in ["E", "W", "N"] {
            out = out.replace(&format!("[{t} {c}"), &format!("[{c}"));
        }
    }
    out
}

/// The text without its last line, the summary.
fn without_summary(s: &str) -> &str {
    let body = s.strip_suffix('\n').unwrap_or(s);
    match body.rfind('\n') {
        Some(at) if body[at + 1..].starts_with("ritsu check: ") => &s[..at + 1],
        _ if body.starts_with("ritsu check: ") => "",
        _ => s,
    }
}

/// What each language's own command prints for the project, unit by unit, in the order `ritsu
/// check` checks them: `ritsu <language> check` on each file, and on the path given for yuen and
/// sakai, which check the project as one. The order is the one the JSON lists the files in.
fn languages_say(dir: &Path, path: &str, lang: &str) -> String {
    let (_, out, err) = ritsu_in(dir, &["check", path, "--root", path, "--format", "json", "--lang", lang]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}{err}"));
    let mut said = String::new();
    let mut whole_done = Vec::new();
    for f in v["files"].as_array().unwrap() {
        let (tool, rel) = (f["tool"].as_str().unwrap(), f["file"].as_str().unwrap());
        let shown = if path == "." { rel.to_string() } else { format!("{path}/{rel}") };
        let args: Vec<&str> = match tool {
            "proto" => continue,
            "yuen" | "sakai" if whole_done.contains(&tool) => continue,
            "yuen" | "sakai" => {
                whole_done.push(tool);
                vec![tool, "check", path, "--root", path, "--lang", lang]
            }
            t => vec![t, "check", &shown, "--lang", lang],
        };
        let (_, out, err) = ritsu_in(dir, &args);
        said.push_str(&out);
        said.push_str(&err);
    }
    said
}

/// Hold `ritsu check <path> --root <path>` to what the languages say, file by file: the same
/// text but for the tools' words, every headline with its tool's word, and the summary last.
fn holds_to_the_languages(dir: &Path, path: &str, lang: &str) -> String {
    let (code, out, err) = ritsu_in(dir, &["check", path, "--root", path, "--lang", lang]);
    assert!(err.is_empty(), "{err}");
    assert!(code < 2, "{out}");
    let body = without_summary(&out);
    assert_eq!(without_tools(body), languages_say(dir, path, lang), "ritsu check of {path} ({lang}) says what the languages say");
    for l in body.lines() {
        for head in ["error[", "warning[", "note[", "エラー[", "警告[", "備考["] {
            if let Some(rest) = l.trim_start().strip_prefix(head)
                && rest.starts_with(['E', 'W', 'N'])
            {
                panic!("a headline without its tool's word: {l}");
            }
        }
    }
    out
}

fn golden(path: &str, got: &str, failures: &mut Vec<String>) {
    if let Err(e) = ritsu_testkit::golden::check(&here().join(path), got) {
        failures.push(e);
    }
}

const SHOP: &str = "tests/projects/通販";

/// The test project (sakai's example): every language's check passes, and the text, the Japanese
/// and the JSON are held to their golden files.
#[test]
fn the_shop_project() {
    let mut failures = Vec::new();
    for lang in ["en", "ja"] {
        let (code, out, err) = ritsu_in(&here(), &["check", SHOP, "--root", SHOP, "--lang", lang]);
        assert_eq!(code, 0, "{out}{err}");
        golden(&format!("tests/golden/check/shop.{lang}.txt"), &out, &mut failures);
    }
    let (code, out, _) = ritsu_in(&here(), &["check", SHOP, "--root", SHOP, "--format", "json"]);
    assert_eq!(code, 0);
    golden("tests/golden/check/shop.json", &out, &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A copy of the project where ordering adds a value to the order status that billing's rule
/// takes in: rulec says the rule's enum does not agree with the contract, and sakai that the rule
/// does not pass rulec's check. Each headline carries its tool; the exit is 1.
#[test]
fn a_project_with_errors() {
    let t = TempDir::new("returned");
    ritsu_testkit::tmp::copy_dir(&here().join(SHOP), t.path());
    let order = t.path().join("proto/shop/ordering/v1/order.proto");
    let s = std::fs::read_to_string(&order).unwrap();
    std::fs::write(&order, s.replacen("  ORDER_STATUS_CANCELLED = 4;\n", "  ORDER_STATUS_CANCELLED = 4;\n  ORDER_STATUS_RETURNED = 5;\n", 1)).unwrap();
    let mut failures = Vec::new();
    for lang in ["en", "ja"] {
        let (code, out, err) = ritsu_in(t.path(), &["check", ".", "--lang", lang]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("error[rulec E032]: ") && out.contains("[sakai E105]: billing/rules/請求の要否.rule:5:1: "), "{out}");
        golden(&format!("tests/golden/check/shop-returned.{lang}.txt"), &out, &mut failures);
    }
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--format", "json"]);
    assert_eq!(code, 1);
    golden("tests/golden/check/shop-returned.json", &out, &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // each diagnostic is its language's own object, with the tool first and the file from the root
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let d = &v["diagnostics"];
    assert_eq!(d.as_array().unwrap().len(), 2, "{d}");
    assert_eq!(d[0].as_object().unwrap().keys().take(3).collect::<Vec<_>>(), ["tool", "v", "severity"]);
    assert_eq!((d[0]["tool"].as_str(), d[0]["code"].as_str(), d[0]["file"].as_str()), (Some("rulec"), Some("E032"), Some("billing/rules/請求の要否.rule")));
    assert_eq!((d[1]["tool"].as_str(), d[1]["code"].as_str(), d[1]["file"].as_str()), (Some("sakai"), Some("E105"), Some("billing/rules/請求の要否.rule")));
    assert_eq!(v["ok"], false);
    let bad: Vec<&str> = v["files"].as_array().unwrap().iter().filter(|f| f["ok"] == false).map(|f| f["file"].as_str().unwrap()).collect();
    assert_eq!(bad, ["billing/rules/請求の要否.rule"]);
}

/// What `ritsu check` says of each file is what the file's language says, word for word: the test
/// project and the copy with errors, in English and in Japanese, a project of yuen's with its
/// rules, and a spec of geas whose claims run a program (Python).
#[test]
fn each_file_as_its_language_says_it() {
    holds_to_the_languages(&here(), SHOP, "en");
    holds_to_the_languages(&here(), SHOP, "ja");
    let t = TempDir::new("returned");
    ritsu_testkit::tmp::copy_dir(&here().join(SHOP), t.path());
    let order = t.path().join("proto/shop/ordering/v1/order.proto");
    let s = std::fs::read_to_string(&order).unwrap();
    std::fs::write(&order, s.replacen("  ORDER_STATUS_CANCELLED = 4;\n", "  ORDER_STATUS_CANCELLED = 4;\n  ORDER_STATUS_RETURNED = 5;\n", 1)).unwrap();
    holds_to_the_languages(t.path(), ".", "en");
    holds_to_the_languages(t.path(), ".", "ja");
    let yuen = here().join("../yuen");
    let out = holds_to_the_languages(&yuen, "tests/fixtures/rulec", "en");
    assert!(out.ends_with("ritsu check: 4 files (rulec 2, yuen 2): all pass; borders between the languages: 0 checked, 0 undecided\n"), "{out}");
    if ritsu_testkit::ready(ritsu_testkit::Need::Python, || ritsu_testkit::tools::on_path("python3").is_some(), "python3 is not on the PATH; the claims of geas's calc are not run") {
        let g = TempDir::new("calc");
        ritsu_testkit::tmp::copy_dir(&here().join("../geas/examples/calc"), g.path());
        let out = holds_to_the_languages(g.path(), ".", "en");
        assert!(out.contains("4 claims · 4 ok · 0 failed") && out.ends_with("(geas 1): all pass; borders between the languages: 0 checked, 0 undecided\n"), "{out}");
    }
}

/// What stops `ritsu check` with exit 2: a path that is not there, a file of no language, a flag
/// it does not take, a value outside a flag's set, and a file a language cannot read.
#[test]
fn what_stops_ritsu_check() {
    let (code, out, err) = ritsu_in(&here(), &["check", "tests/projects/無い"]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert_eq!(err, "error: `tests/projects/無い` is not there\n");
    let (code, _, err) = ritsu_in(&here(), &["check", "Cargo.toml"]);
    assert_eq!(code, 2);
    assert!(err.contains("`Cargo.toml` is a file of none of ritsu's languages"), "{err}");
    let (code, _, err) = ritsu_in(&here(), &["check", SHOP, "--fast"]);
    assert_eq!((code, err.as_str()), (2, "error: unknown flag `--fast`; run `ritsu check --help`\n"));
    let (code, _, err) = ritsu_in(&here(), &["check", SHOP, "--format", "yaml", "--lang", "ja"]);
    assert_eq!(code, 2);
    assert!(err.starts_with("エラー: `--format yaml` は知らない値です"), "{err}");
    let t = TempDir::new("unreadable");
    std::fs::write(t.path().join("bad.rule"), [0xff, 0xfe, 0x00]).unwrap();
    std::fs::write(t.path().join("ok.cal"), "dates ok v1\n").unwrap();
    let (code, out, _) = ritsu_in(t.path(), &["check", "."]);
    assert_eq!(code, 2, "{out}");
    assert!(out.starts_with("error: cannot read `bad.rule`\n"), "{out}");
    assert!(out.contains("1 not checked"), "{out}");
}
