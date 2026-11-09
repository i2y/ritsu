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
    for v in ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG", "SEKISHO_LANG", "KANAME_LANG"] {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

const TOOLS: [&str; 10] = ["rulec", "dandori", "koyomi", "chobo", "geas", "yuen", "sakai", "sekisho", "kaname", "ritsu"];

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

/// Hold each file's `check` in the JSON of `ritsu check <path> --root <path>` to what its language
/// writes of it: the object of the language's own `check --format json` (geas's `--json`) without
/// `file`, `root` and `diagnostics` — on each `.req`, the one of yuen's requirements; on a map,
/// sakai's object of the map — and null for a rule, a `.proto` and a context that is no map. How
/// many files have an object.
fn checks_hold_to_the_languages(dir: &Path, path: &str) -> usize {
    let (code, out, err) = ritsu_in(dir, &["check", path, "--root", path, "--format", "json"]);
    assert!(code < 2, "{out}{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}{err}"));
    // every object the command writes, one after another
    let written = |args: &[&str]| -> Vec<serde_json::Value> {
        let (_, out, err) = ritsu_in(dir, args);
        serde_json::Deserializer::from_str(&out).into_iter::<serde_json::Value>().map(|x| x.unwrap_or_else(|e| panic!("{args:?}: {e}: {out}{err}"))).collect()
    };
    let without = |mut o: serde_json::Value| {
        if let Some(m) = o.as_object_mut() {
            for k in ["file", "root", "diagnostics"] {
                m.remove(k);
            }
        }
        o
    };
    let mut objects = 0;
    for f in v["files"].as_array().unwrap() {
        let (tool, rel) = (f["tool"].as_str().unwrap(), f["file"].as_str().unwrap());
        let shown = if path == "." { rel.to_string() } else { format!("{path}/{rel}") };
        let want = match tool {
            "rulec" | "proto" => serde_json::Value::Null,
            "chobo" => without(written(&["chobo", "check", &shown, "--format", "json"])[0]["files"][0].clone()),
            "geas" => without(written(&["geas", "check", &shown, "--json"])[0].clone()),
            "kaname" | "sekisho" => without(written(&[tool, "check", &shown, "--format", "json", "--root", path])[0].clone()),
            "yuen" => without(written(&["yuen", "check", path, "--root", path, "--format", "json"])[0].clone()),
            "sakai" => written(&["sakai", "check", path, "--root", path, "--format", "json"]).into_iter().find(|m| m["file"] == rel).map_or(serde_json::Value::Null, without),
            t => without(written(&[t, "check", &shown, "--format", "json"])[0].clone()),
        };
        assert_eq!(f["check"], want, "the check of {rel} ({tool}) in the JSON of ritsu check {path}");
        assert_eq!(f.as_object().unwrap().keys().last().map(String::as_str), Some("check"), "{f}");
        objects += usize::from(want.is_object());
    }
    objects
}

fn golden(path: &str, got: &str, failures: &mut Vec<String>) {
    if let Err(e) = ritsu_testkit::golden::check(&here().join(path), got) {
        failures.push(e);
    }
}

/// The test project with Japanese names (sakai's Japanese example), and its twin with English names.
/// The golden files are named by the project's directory.
const SHOP: &str = "tests/projects/通販";
const SHOP_EN: &str = "tests/projects/shop";

/// The test project (sakai's example): every language's check passes, and the text, the Japanese
/// and the JSON are held to their golden files.
#[test]
fn the_shop_project() {
    let mut failures = Vec::new();
    for lang in ["en", "ja"] {
        let (code, out, err) = ritsu_in(&here(), &["check", SHOP, "--root", SHOP, "--lang", lang]);
        assert_eq!(code, 0, "{out}{err}");
        golden(&format!("tests/golden/check/通販.{lang}.txt"), &out, &mut failures);
    }
    let (code, out, _) = ritsu_in(&here(), &["check", SHOP, "--root", SHOP, "--format", "json"]);
    assert_eq!(code, 0);
    golden("tests/golden/check/通販.json", &out, &mut failures);
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
        let head = if lang == "ja" { "エラー[rulec E032]: " } else { "error[rulec E032]: " };
        assert!(out.contains(head) && out.contains("[sakai E105]: billing/rules/請求の要否.rule:5:1: "), "{out}");
        golden(&format!("tests/golden/check/通販-returned.{lang}.txt"), &out, &mut failures);
    }
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--format", "json"]);
    assert_eq!(code, 1);
    golden("tests/golden/check/通販-returned.json", &out, &mut failures);
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

/// The JSON of `ritsu check` holds, for each file, what its language's check writes of it (DESIGN
/// 8.3): in the test project (a book, flows, date files and a map) and its copy with errors, a
/// project of yuen's (two `.req` with one object), two of ritsu's with gates and modules of kaname
/// (one with files that fail), sekisho's example with its dates, kaname's payout with its books,
/// and the spec of geas's calc, whose claims run a program (Python).
#[test]
fn each_files_check_in_the_json_is_what_its_language_writes() {
    assert_eq!(checks_hold_to_the_languages(&here(), SHOP), 7);
    assert_eq!(checks_hold_to_the_languages(&here(), SHOP_EN), 7);
    let t = TempDir::new("returned-json");
    ritsu_testkit::tmp::copy_dir(&here().join(SHOP_EN), t.path());
    let order = t.path().join("proto/shop/ordering/v1/order.proto");
    let s = std::fs::read_to_string(&order).unwrap();
    std::fs::write(&order, s.replacen("  ORDER_STATUS_CANCELLED = 4;\n", "  ORDER_STATUS_CANCELLED = 4;\n  ORDER_STATUS_RETURNED = 5;\n", 1)).unwrap();
    assert_eq!(checks_hold_to_the_languages(t.path(), "."), 7);
    assert_eq!(checks_hold_to_the_languages(&here().join("../yuen"), "tests/fixtures/rulec"), 2);
    assert_eq!(checks_hold_to_the_languages(&here(), "tests/projects/cold_chain"), 6);
    assert_eq!(checks_hold_to_the_languages(&here(), "tests/projects/regions"), 12);
    assert_eq!(checks_hold_to_the_languages(&here().join("../sekisho"), "examples/refunds"), 6);
    let p = TempDir::new("payout-json");
    ritsu_testkit::tmp::copy_dir(&here().join("../kaname/examples/payout"), p.path());
    assert_eq!(checks_hold_to_the_languages(p.path(), "."), 4);
    let (_, out, _) = ritsu_in(p.path(), &["check", ".", "--root", ".", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let kaname = v["files"].as_array().unwrap().iter().find(|f| f["file"] == "payout.kn").unwrap();
    assert_eq!(kaname["check"]["summary"], "1 function, total", "{kaname}");
    assert_eq!(kaname["check"]["functions"][0]["effect"]["fails"], serde_json::json!([]), "{kaname}");
    if ritsu_testkit::ready(ritsu_testkit::Need::Python, || ritsu_testkit::tools::on_path("python3").is_some(), "python3 is not on the PATH; the claims of geas's calc are not run") {
        let g = TempDir::new("calc-json");
        ritsu_testkit::tmp::copy_dir(&here().join("../geas/examples/calc"), g.path());
        assert_eq!(checks_hold_to_the_languages(g.path(), "."), 1);
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
    assert!(err.starts_with("エラー: `--format` に `yaml` は使えません"), "{err}");
    let t = TempDir::new("unreadable");
    std::fs::write(t.path().join("bad.rule"), [0xff, 0xfe, 0x00]).unwrap();
    std::fs::write(t.path().join("ok.cal"), "dates ok v1\n").unwrap();
    let (code, out, _) = ritsu_in(t.path(), &["check", "."]);
    assert_eq!(code, 2, "{out}");
    assert!(out.starts_with("error: cannot read `bad.rule`\n"), "{out}");
    assert!(out.contains("1 not checked"), "{out}");
}

/// `--lean` with no Lean to run (kaname's DESIGN 38.3): `lake` neither on the `PATH` nor in
/// `~/.elan/bin` stops `ritsu check --lean` and `ritsu kaname check --lean` with exit 2 and what to
/// install, before any file is checked.
#[test]
fn check_lean_without_lean_exits_2() {
    let t = TempDir::new("lean-missing");
    ritsu_testkit::tmp::copy_dir(&here().join("../kaname/examples/refund_split"), t.path());
    let home = TempDir::new("lean-missing-home");
    let said = |args: &[&str]| {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
        c.current_dir(t.path()).args(args).env("PATH", "/usr/bin:/bin").env("HOME", home.path()).env_remove("RITSU_LANG").env_remove("KANAME_LANG");
        let o = c.output().expect("could not run ritsu");
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
    };
    let missing = format!(
        "error: --lean needs Lean: `lake` is neither on PATH nor in ~/.elan/bin. Install elan, the Lean version manager; the toolchain the proofs use ({}) is installed on the first run\n",
        kaname::r#gen::lean::TOOLCHAIN
    );
    assert_eq!(said(&["check", ".", "--lean"]), (2, String::new(), missing.clone()));
    assert_eq!(said(&["kaname", "check", "refund_split.kn", "--lean"]), (2, String::new(), missing));
    let (code, out, err) = said(&["check", ".", "--lean", "--lang", "ja"]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert!(err.starts_with("エラー: --lean には Lean が要ります。`lake` が PATH にも ~/.elan/bin にもありません。"), "{err}");
}

/// The JSON gives each property of a module its answer (kaname's DESIGN 36, 38.3): undecided, with
/// why, when nothing shows it (the proof beside it waits for `--lean`; no proof is there); the
/// counterexample, as the function's input, when a vector breaks it.
#[test]
fn check_json_gives_each_property_its_outcome() {
    let t = TempDir::new("properties-json");
    ritsu_testkit::tmp::copy_dir(&here().join("../kaname/examples/refund_split"), t.path());
    let properties = |dir: &Path, file: &str| -> serde_json::Value {
        let (_, out, err) = ritsu_in(dir, &["check", ".", "--root", ".", "--format", "json"]);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}{err}"));
        v["files"].as_array().unwrap().iter().find(|f| f["file"] == file).unwrap_or_else(|| panic!("no {file}: {v}"))["check"]["properties"].clone()
    };
    let p = properties(t.path(), "refund_split.kn");
    let undecided = |reason: &str| {
        serde_json::json!([
            {"name": "shares_add_up", "line": 18, "outcome": "undecided", "reason": reason, "code": "W603", "vectors": 12, "held": 12, "input": null},
            {"name": "no_share_is_negative", "line": 22, "outcome": "undecided", "reason": reason, "code": "W603", "vectors": 12, "held": 12, "input": null},
            {"name": "no_share_is_more_than_its_left", "line": 26, "outcome": "undecided", "reason": reason, "code": "W603", "vectors": 12, "held": 6, "input": null},
        ])
    };
    assert_eq!(p, undecided("lean-not-run"));
    std::fs::remove_file(t.path().join("refund_split.lean")).unwrap();
    assert_eq!(properties(t.path(), "refund_split.kn"), undecided("no-proof"));
    let f = TempDir::new("properties-json-false");
    std::fs::copy(here().join("../kaname/tests/properties/shares.kn"), f.path().join("shares.kn")).unwrap();
    assert_eq!(
        properties(f.path(), "shares.kn"),
        serde_json::json!([{"name": "every_line_gets_a_share", "line": 12, "outcome": "example", "reason": null, "code": "E603", "vectors": 18, "held": 12, "input": {"lines": [{"sku": "a", "left": 500000}, {"sku": "a", "left": 500000}], "refund": 1}}])
    );
}

/// `--budget` caps what the checks that walk a space may walk, one number for each language that
/// has such a check: the nodes rulec's check of one rule visits (E109) and the combinations
/// sekisho walks for one action (E307), as `ritsu rulec check --budget` and `ritsu sekisho check
/// --budget` take it. A budget that is no positive whole number is refused.
#[test]
fn the_budget_goes_to_each_language_that_walks() {
    let (code, out, _) = ritsu_in(&here(), &["check", "--help"]);
    assert_eq!(code, 0);
    assert!(out.contains("[--budget <n>]") && out.contains("ritsu check rules/ --budget 200000000"), "{out}");
    // the smallest rule of the E109 entry in rulec's ledger: one node is not enough to prove it
    let t = TempDir::new("budget");
    std::fs::write(t.path().join("pick.rule"), "rule pick v1\n\nenum k = a | b | c\n\ninputs\n  x : k\n\noutputs\n  r : bool\n\ntable j\npolicy unique\n| x | -> r : bool |\n| a | true        |\n| b | false       |\n| c | true        |\n").unwrap();
    let (code, out, _) = ritsu_in(t.path(), &["check", "."]);
    assert_eq!(code, 0, "{out}");
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--budget", "1"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("error[rulec E109]"), "{out}");
    let refunds = here().join("../sekisho/examples/refunds");
    let (code, out, _) = ritsu_in(&refunds, &["check", "refunds.gate", "--budget", "20"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("error[sekisho E307]") && out.contains("more than the budget of 20"), "{out}");
    let (code, out, _) = ritsu_in(&refunds, &["check", "refunds.gate"]);
    assert_eq!(code, 0, "{out}");
    for v in ["0", "-3", "many"] {
        let (code, out, err) = ritsu_in(&here(), &["check", SHOP, "--budget", v]);
        assert_eq!((code, out.as_str(), err), (2, "", format!("error: `--budget {v}` is not a positive integer\n")));
    }
    let (code, _, err) = ritsu_in(&here(), &["check", SHOP, "--budget", "0", "--lang", "ja"]);
    assert_eq!((code, err.as_str()), (2, "エラー: `--budget 0` は正の整数ではありません\n"));
}

// ── The English twins: the same project with its names in English ──

/// The test project with English names: every language's check passes, and the text, the Japanese
/// and the JSON are held to their golden files.
#[test]
fn the_shop_project_in_english() {
    let mut failures = Vec::new();
    for lang in ["en", "ja"] {
        let (code, out, err) = ritsu_in(&here(), &["check", SHOP_EN, "--root", SHOP_EN, "--lang", lang]);
        assert_eq!(code, 0, "{out}{err}");
        golden(&format!("tests/golden/check/shop.{lang}.txt"), &out, &mut failures);
    }
    let (code, out, _) = ritsu_in(&here(), &["check", SHOP_EN, "--root", SHOP_EN, "--format", "json"]);
    assert_eq!(code, 0);
    golden("tests/golden/check/shop.json", &out, &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // the two projects are the same project: the same files in the same languages
    let count = |dir: &str| {
        let (_, out, _) = ritsu_in(&here(), &["check", dir, "--root", dir]);
        out.lines().last().unwrap_or("").to_string()
    };
    assert_eq!(count(SHOP), count(SHOP_EN));
}

/// The twin of `a_project_with_errors`: ordering adds a value to the order status that billing's
/// rule takes in; rulec and sakai say so, each headline with its tool, and the exit is 1.
#[test]
fn a_project_with_errors_in_english() {
    let t = TempDir::new("returned-en");
    ritsu_testkit::tmp::copy_dir(&here().join(SHOP_EN), t.path());
    let order = t.path().join("proto/shop/ordering/v1/order.proto");
    let s = std::fs::read_to_string(&order).unwrap();
    std::fs::write(&order, s.replacen("  ORDER_STATUS_CANCELLED = 4;\n", "  ORDER_STATUS_CANCELLED = 4;\n  ORDER_STATUS_RETURNED = 5;\n", 1)).unwrap();
    let mut failures = Vec::new();
    for lang in ["en", "ja"] {
        let (code, out, err) = ritsu_in(t.path(), &["check", ".", "--lang", lang]);
        assert_eq!(code, 1, "{out}{err}");
        let head = if lang == "ja" { "エラー[rulec E032]: " } else { "error[rulec E032]: " };
        assert!(out.contains(head) && out.contains("[sakai E105]: billing/rules/billing_need.rule:5:1: "), "{out}");
        golden(&format!("tests/golden/check/shop-returned.{lang}.txt"), &out, &mut failures);
    }
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--format", "json"]);
    assert_eq!(code, 1);
    golden("tests/golden/check/shop-returned.json", &out, &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let d = &v["diagnostics"];
    assert_eq!(d.as_array().unwrap().len(), 2, "{d}");
    assert_eq!(d[0].as_object().unwrap().keys().take(3).collect::<Vec<_>>(), ["tool", "v", "severity"]);
    assert_eq!((d[0]["tool"].as_str(), d[0]["code"].as_str(), d[0]["file"].as_str()), (Some("rulec"), Some("E032"), Some("billing/rules/billing_need.rule")));
    assert_eq!((d[1]["tool"].as_str(), d[1]["code"].as_str(), d[1]["file"].as_str()), (Some("sakai"), Some("E105"), Some("billing/rules/billing_need.rule")));
    assert_eq!(v["ok"], false);
    let bad: Vec<&str> = v["files"].as_array().unwrap().iter().filter(|f| f["ok"] == false).map(|f| f["file"].as_str().unwrap()).collect();
    assert_eq!(bad, ["billing/rules/billing_need.rule"]);
}

/// The twin of `each_file_as_its_language_says_it`, on the English project and its copy with errors.
#[test]
fn each_file_as_its_language_says_it_in_english() {
    holds_to_the_languages(&here(), SHOP_EN, "en");
    holds_to_the_languages(&here(), SHOP_EN, "ja");
    let t = TempDir::new("returned-en");
    ritsu_testkit::tmp::copy_dir(&here().join(SHOP_EN), t.path());
    let order = t.path().join("proto/shop/ordering/v1/order.proto");
    let s = std::fs::read_to_string(&order).unwrap();
    std::fs::write(&order, s.replacen("  ORDER_STATUS_CANCELLED = 4;\n", "  ORDER_STATUS_CANCELLED = 4;\n  ORDER_STATUS_RETURNED = 5;\n", 1)).unwrap();
    holds_to_the_languages(t.path(), ".", "en");
    holds_to_the_languages(t.path(), ".", "ja");
}

/// A project whose rules are added to by contributions (rulec's §15.227–§15.231): `ritsu check`
/// merges the project's contributions into the rules they reach, which is what rulec says of the
/// project's rule files given to it in one run, and the flow is checked against the rule merged.
/// Without the file that gives the new kind of product its markup, the open table of the price has
/// no row for it, the error says which contribution added the kind, and the flow cannot read the
/// rule.
#[test]
fn a_projects_contributions_are_merged_into_the_rules_they_reach() {
    let dir = here().join("tests/projects/product_line");
    let (code, out, err) = ritsu_in(&dir, &["check", "--lang", "en"]);
    assert_eq!(code, 0, "{out}{err}");
    let rules = ["rules/contrib/garden_kind.rule", "rules/contrib/garden_price.rule", "rules/product_price.rule", "rules/shared/product_kinds.rule"];
    let mut args = vec!["rulec", "check"];
    args.extend(rules);
    args.extend(["--lang", "en"]);
    let (_, said, _) = ritsu_in(&dir, &args);
    assert_eq!(said, "ok rules/contrib/garden_kind.rule (contributes to rules/shared/product_kinds.rule)\nok rules/contrib/garden_price.rule (contributes to rules/product_price.rule)\nok rules/product_price.rule (2 contributions: rules/contrib/garden_price.rule, rules/contrib/garden_kind.rule)\nok rules/shared/product_kinds.rule (1 contribution: rules/contrib/garden_kind.rule)\n");
    assert_eq!(without_summary(&out), format!("{said}pricing.flow: ok\n"));
    let t = TempDir::new("product-line");
    ritsu_testkit::tmp::copy_dir(&dir, t.path());
    std::fs::remove_file(t.path().join("rules/contrib/garden_price.rule")).unwrap();
    let (code, out, _) = ritsu_in(t.path(), &["check", "--lang", "en"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("error[rulec E101]: Completeness gap: some input matches no row\n  --> rules/product_price.rule:13 table markup_of"), "{out}");
    assert!(out.contains("garden is a value garden_kind adds (rules/contrib/garden_kind.rule)."), "{out}");
    assert!(out.contains("error[dandori E005]: pricing.flow:7:10: could not read the rule `rules/product_price.rule`"), "{out}");
    // A markup written for toys rather than for the new kind: the overlap, the gap and the
    // contribution's example are said once, for the contribution, and the rule says they were.
    let t = TempDir::new("product-line-toys");
    ritsu_testkit::tmp::copy_dir(&dir, t.path());
    let f = t.path().join("rules/contrib/garden_price.rule");
    std::fs::write(&f, std::fs::read_to_string(&f).unwrap().replace("  when item_kind garden", "  when item_kind toys")).unwrap();
    let (code, out, _) = ritsu_in(t.path(), &["check", "--lang", "en"]);
    assert_eq!(code, 1, "{out}");
    let (_, said, _) = ritsu_in(t.path(), &args);
    assert!(without_tools(&out).starts_with(&said), "ritsu check says what rulec says of the files in one run:\n{out}\n---\n{said}");
    assert!(said.contains("note rules/product_price.rule: its errors are said above\n"), "{said}");
    assert_eq!(said.matches("error[E105]").count(), 1, "{said}");
}

/// The last line counts each finding with the files that came out as the file it is about did
/// (rulec's §15.242): a warning of a file that passes is counted with the files that pass whether or
/// not another file fails, as it is when every file passes. Before, the warnings of a file that
/// passed were counted with the files that failed when one did, `1 fail (1 error, 2 warnings)`, and
/// with the ones that passed when none did, `all pass (2 warnings)`.
#[test]
fn a_warning_of_a_file_that_passes_is_counted_with_the_files_that_pass() {
    let t = TempDir::new("summary-counts");
    let d = t.path();
    std::fs::write(d.join("ritsu.toml"), "").unwrap();
    // two warnings: an input no table reads, and a value of an enum no row names
    std::fs::write(
        d.join("warned.rule"),
        "rule warned v1\ndescription \"A rule with two warnings\"\n\nenum k = x | y | z\n\ninputs\n  n : number  range >=0 <=9\n  w : k\n\noutputs\n  fee : number  round down(1)\n\n\
         table t\npolicy unique\n| n   | -> fee : number |\n| <=4 | 1               |\n| >=5 | 2               |\n",
    )
    .unwrap();
    let broken = "rule broken v1\ndescription \"A rule with a gap\"\n\ninputs\n  n : number  range >=0 <=9\n\noutputs\n  fee : number  round down(1)\n\n\
                  table t\npolicy unique\n| n   | -> fee : number |\n| <=4 | 1               |\n";
    std::fs::write(d.join("broken.rule"), broken).unwrap();
    let last = |out: &str| out.lines().last().unwrap_or_default().to_string();
    let (code, out, _) = ritsu_in(d, &["check", ".", "--lang", "en"]);
    assert_eq!(code, 1, "{out}");
    assert!(last(&out).contains(": 1 fail (1 error), 1 pass (2 warnings); "), "{out}");
    let (_, out, _) = ritsu_in(d, &["check", ".", "--lang", "ja"]);
    assert!(last(&out).contains("。検査を通らないもの 1 個（エラー 1 件）、通るもの 1 個（警告 2 件）。"), "{out}");
    std::fs::remove_file(d.join("broken.rule")).unwrap();
    let (code, out, _) = ritsu_in(d, &["check", ".", "--lang", "en"]);
    assert_eq!(code, 0, "{out}");
    assert!(last(&out).contains(": all pass (2 warnings); "), "{out}");
}

/// A project with modules of kaname (kaname's PLAN B.6): `ritsu check` checks each `.kn` after the
/// rules and the books it calls, with them joined, and says of it what `ritsu kaname check` says;
/// a mistake in a module fails the project with kaname's word in its headline.
#[test]
fn the_modules_of_kaname_are_checked_as_kaname_checks_them() {
    let t = TempDir::new("kaname-project");
    ritsu_testkit::tmp::copy_dir(&here().join("../kaname/examples/payout"), t.path());
    let out = holds_to_the_languages(t.path(), ".", "en");
    // each module's two amounts and the refusals of its two operations are borders between the
    // languages (X4), shown
    assert!(out.ends_with("ritsu check: 6 files (rulec 2, chobo 2, kaname 2): all pass; borders between the languages: 8 checked, 0 undecided\n"), "{out}");
    let out = holds_to_the_languages(t.path(), ".", "ja");
    assert!(out.contains("ok payout.ja.kn: 関数 1 個、total\n"), "{out}");
    // the modules come after the rules and the books
    let (_, out, _) = ritsu_in(t.path(), &["check", ".", "--root", "."]);
    assert!(out.find("ok payout.kn").unwrap() > out.find("books/ledger.book: ok").unwrap(), "{out}");
    // a fee counted in dollars given to a book that counts cents
    let kn = t.path().join("payout.kn");
    let src = std::fs::read_to_string(&kn).unwrap();
    std::fs::write(&kn, src.replace("seller: payment.seller, fee: fee)", "seller: payment.seller, fee: fee in USD)")).unwrap();
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--root", "."]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("error[kaname E102]: payout.kn:22:"), "{out}");
    assert!(out.ends_with("ritsu check: 6 files (rulec 2, chobo 2, kaname 2): 1 fail (1 error); borders between the languages: 4 checked, 0 undecided\n"), "{out}");
    holds_to_the_languages(t.path(), ".", "en");
    let (_, out, _) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let d = &v["diagnostics"][0];
    assert_eq!((d["tool"].as_str(), d["code"].as_str(), d["file"].as_str()), (Some("kaname"), Some("E102"), Some("payout.kn")));
    let files: Vec<(&str, bool)> = v["files"].as_array().unwrap().iter().filter(|f| f["tool"] == "kaname").map(|f| (f["file"].as_str().unwrap(), f["ok"].as_bool().unwrap())).collect();
    assert_eq!(files, [("payout.ja.kn", true), ("payout.kn", false)]);
}

/// A rule's diagnostic keeps the request it shows, so that what reads the JSON of the whole project
/// gets the same example as what reads `rulec check` (the rule's §15.282).
#[test]
fn a_rules_example_request_stays_in_the_json() {
    let t = TempDir::new("request");
    std::fs::write(
        t.path().join("orders.schema.json"),
        r##"{"$defs":{"Order":{"type":"object","required":["lines"],"additionalProperties":false,"properties":{"lines":{"type":"array","items":{"type":"object","required":["category"],"additionalProperties":false,"properties":{"category":{"enum":["tool","part","service"]}}}}}}}}"##,
    )
    .unwrap();
    std::fs::write(
        t.path().join("tool_fee.rule"),
        r##"rule tool_fee v1
description "Whether an order holds a tool, and whether it holds nothing but parts"

shape order = jsonschema "orders.schema.json" "#/$defs/Order"

inputs
  has_tool  : bool  from any order.lines where category = "tool"
  all_parts : bool  from all order.lines where category = "part"

never has_tool = false, all_parts = false

outputs
  fee : money[USD]  round up(1USD)

table fee_of
policy unique
| has_tool | all_parts | -> fee : money[USD] |
| true     | true      | 5USD                |
| true     | false     | 3USD                |
| false    | true      | 0USD                |

examples
| has_tool | all_parts | -> fee |
| true     | false     | 3USD   |
| false    | true      | 0USD   |
"##,
    )
    .unwrap();
    let (code, out, err) = ritsu_in(t.path(), &["check", ".", "--format", "json"]);
    assert_eq!(code, 1, "{out}{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let d = v["diagnostics"].as_array().unwrap().iter().find(|d| d["code"] == "E142").unwrap_or_else(|| panic!("{out}"));
    assert_eq!(d["tool"].as_str(), Some("rulec"), "{out}");
    let c = &d["request"]["collections"][0];
    assert_eq!(c["path"].as_str(), Some("order.lines"), "{out}");
    let e = &c["elements"][0];
    assert_eq!(e["fields"]["category"].as_str(), Some("service"), "{out}");
}

/// `--no-claims`: a `.geas` file is read and nothing it names is run, no journal is written, and it
/// still counts as a file of the project. What can be said without a run stops the check all the
/// same: a line that does not parse, and a call its contract does not have.
#[test]
fn no_claims_reads_the_claim_files_and_runs_nothing() {
    let t = TempDir::new("check-no-claims");
    let mark = t.path().join("ran");
    t.write("leave_a_mark.sh", format!("touch '{}'\n", mark.display()));
    t.write("api.yaml", "openapi: 3.0.3\ninfo: {title: leave, version: \"1\"}\npaths:\n  /leave:\n    get:\n      responses:\n        \"200\": {description: ok}\n");
    let claim = "target mark {\n  run \"sh leave_a_mark.sh\"\n}\n\nclaim \"it leaves a mark\" {\n  when mark.run()\n  then exit is 0\n}\n";
    t.write("mark.geas", claim);
    t.write("ritsu.mod", "module example.com/acme/leave\nversion 1.0.0\ndescription \"Leave\"\n");

    let (code, out, err) = ritsu_in(t.path(), &["check", ".", "--no-claims"]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    assert!(out.contains("1 claim · not run\n"), "{out}");
    assert!(out.contains("ritsu check: 1 file (geas 1): all pass"), "{out}");
    assert!(!mark.exists(), "a claim ran:\n{out}");
    assert!(!t.path().join(".geas").exists(), "a journal was written");

    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--no-claims", "--lang", "ja"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("主張 1 件 · 走らせていません\n"), "{out}");

    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--no-claims", "--format", "json"]);
    assert_eq!(code, 0, "{out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let file = &v["files"].as_array().unwrap()[0];
    assert_eq!((file["file"].as_str(), file["ok"].as_bool(), file["check"]["claims_not_run"].as_i64()), (Some("mark.geas"), Some(true), Some(1)), "{out}");
    assert!(!mark.exists(), "a claim ran:\n{out}");

    // a call the contract does not have, before anything runs
    let held = "target api {\n  serve \"sh leave_a_mark.sh {port}\"\n  port auto\n  contract \"api.yaml\"\n}\n\nclaim \"it answers\" {\n  when api.get(\"/nothing\")\n  then status is 200\n}\n";
    t.write("mark.geas", held);
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--no-claims"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("E016"), "{out}");
    assert!(!mark.exists(), "a claim ran:\n{out}");

    // a line that does not parse
    t.write("mark.geas", "claim \"it never ends\" {\n");
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--no-claims"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("error[geas E"), "{out}");

    // and without the flag it does run, which is what the mark above is there to tell
    t.write("mark.geas", claim);
    let (code, out, _) = ritsu_in(t.path(), &["check", "."]);
    assert_eq!(code, 0, "{out}");
    assert!(mark.exists(), "the claim did not run:\n{out}");
}
