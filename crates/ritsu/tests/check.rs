//! `ritsu check` (DESIGN 8.1, 8.3, 8.4; PLAN E.2): every file of a project checked by its
//! language's own `check`, in the order the languages are checked, the text with the tool's word in
//! each headline, the JSON one object, and the exit code. What it says of each file is what the
//! language's command says of it, word for word, which the test holds it to by running each
//! language's command on the same files, but for the lines that read a file that does not pass:
//! those are said once, under that file, after the rest (DESIGN 8.3).

mod common;

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
    for v in common::LANG_VARS {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

const TOOLS: [&str; 11] = ["rulec", "dandori", "koyomi", "chobo", "geas", "yuen", "sakai", "sekisho", "kaname", "ringi", "ritsu"];

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

/// The text without its blocks of the lines that wait for a file that does not pass (DESIGN 8.3),
/// and those lines: a block is a line that names the files waited for, then the lines, indented.
fn without_waiting(s: &str) -> (String, Vec<&str>) {
    let mut kept = String::new();
    let mut waiting = Vec::new();
    let mut inside = false;
    for l in s.split_inclusive('\n') {
        let t = l.trim_end_matches('\n');
        if (t.starts_with("waiting for ") && t.ends_with(" to pass:")) || t.ends_with("が通るのを待っています:") {
            inside = true;
        } else if inside && t.starts_with("  ") {
            waiting.push(t.trim_start());
        } else {
            inside = false;
            kept.push_str(l);
        }
    }
    (kept, waiting)
}

/// A line that waits for a file that does not pass, as the JSON of `ritsu check` gives it: the
/// tool, the file from the root, the line and the code.
struct Waits {
    tool: String,
    file: String,
    line: u64,
    code: String,
}

/// What a language's command printed, without the diagnostics `waits` names: each from its
/// headline (rulec's, with the line under it that gives the place) to the end of what goes with
/// it (the indented lines after it; for rulec, its empty line). yuen's line after its findings
/// counts the errors and warnings left, and goes when no error is left, as `ritsu check` writes it.
fn without_waits(out: &str, tool: &str, waits: &[&Waits], path: &str) -> String {
    if waits.is_empty() {
        return out.to_string();
    }
    let lines: Vec<&str> = out.split_inclusive('\n').collect();
    let at = |i: usize, w: &Waits| {
        let shown = if path == "." { w.file.clone() } else { format!("{path}/{}", w.file) };
        let head = ["error[", "エラー["].iter().any(|h| lines[i].starts_with(&format!("{h}{}]: ", w.code)));
        if tool == "rulec" {
            let place = format!("--> {shown}:{}", w.line);
            head && lines.get(i + 1).is_some_and(|n| n.trim() == place || n.trim().starts_with(&format!("{place} ")))
        } else {
            head && lines[i].contains(&format!("]: {shown}:{}:", w.line))
        }
    };
    let mut kept: Vec<&str> = Vec::new();
    let (mut i, mut gone) = (0, 0);
    while i < lines.len() {
        if !waits.iter().any(|w| at(i, w)) {
            kept.push(lines[i]);
            i += 1;
            continue;
        }
        gone += 1;
        i += 1;
        if tool == "rulec" {
            while i < lines.len() && lines[i] != "\n" {
                i += 1;
            }
            i += 1;
        } else {
            while i < lines.len() && lines[i].starts_with(' ') {
                i += 1;
            }
        }
    }
    if tool == "yuen" && gone > 0 {
        let last = kept.pop().unwrap_or_default();
        let label = last.split(": ").next().unwrap_or_default();
        let count = |heads: [&str; 2]| kept.iter().filter(|l| heads.iter().any(|h| l.starts_with(h))).count();
        let (e, w) = (count(["error[", "エラー["]), count(["warning[", "警告["]));
        let ja = last.contains(": エラー ");
        let line = match (e, w, ja) {
            (0, _, _) => String::new(),
            (e, 0, true) => format!("{label}: エラー {e} 件\n"),
            (e, w, true) => format!("{label}: エラー {e} 件、警告 {w} 件\n"),
            (e, w, false) => {
                let n = |k: usize, one: &str| if k == 1 { format!("1 {one}") } else { format!("{k} {one}s") };
                if w == 0 { format!("{label}: {}\n", n(e, "error")) } else { format!("{label}: {}, {}\n", n(e, "error"), n(w, "warning")) }
            }
        };
        return kept.concat() + &line;
    }
    kept.concat()
}

/// What each language's own command prints for the project, unit by unit, in the order `ritsu
/// check` checks them: `ritsu <language> check` on each file, and on the path given for yuen and
/// sakai, which check the project as one. The order is the one the JSON lists the files in. The
/// diagnostics `ritsu check` says are lines that wait for a file that does not pass are left out
/// ([`without_waits`]); how many there are comes second.
fn languages_say(dir: &Path, path: &str, lang: &str) -> (String, usize) {
    let (_, out, err) = ritsu_in(dir, &["check", path, "--root", path, "--format", "json", "--lang", lang]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}{err}"));
    let waits: Vec<Waits> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d.get("waits_for").is_some())
        .map(|d| Waits { tool: d["tool"].as_str().unwrap().into(), file: d["file"].as_str().unwrap().into(), line: d["line"].as_u64().unwrap(), code: d["code"].as_str().unwrap().into() })
        .collect();
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
        // dandori writes its diagnostics to stderr, the others to stdout
        let theirs: Vec<&Waits> = waits.iter().filter(|w| w.tool == tool && (matches!(tool, "yuen" | "sakai") || w.file == rel)).collect();
        said.push_str(&without_waits(&format!("{out}{err}"), tool, &theirs, path));
    }
    (said, waits.len())
}

/// Hold `ritsu check <path> --root <path>` to what the languages say, file by file: the same
/// text but for the tools' words and the lines that wait for a file that does not pass, which are
/// said once each, after the rest; every headline with its tool's word, and the summary last.
fn holds_to_the_languages(dir: &Path, path: &str, lang: &str) -> String {
    let (code, out, err) = ritsu_in(dir, &["check", path, "--root", path, "--lang", lang]);
    assert!(err.is_empty(), "{err}");
    assert!(code < 2, "{out}");
    let body = without_summary(&out);
    let (said, waiting) = without_waiting(body);
    let (theirs, waits) = languages_say(dir, path, lang);
    assert_eq!(waiting.len(), waits, "ritsu check of {path} ({lang}) says each line that waits once:\n{out}");
    assert_eq!(without_tools(&said), theirs, "ritsu check of {path} ({lang}) says what the languages say");
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
/// takes in: rulec says the rule's enum does not agree with the contract, and sakai's line that
/// the rule does not pass rulec's check waits for it, said under the rule after the rest (DESIGN
/// 8.3). The headline carries its tool; the exit is 1.
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
        assert!(out.contains(head) && out.contains("\n  billing/rules/請求の要否.rule:5: [sakai E105] enum "), "{out}");
        assert!(!out.contains("[sakai E105]: "), "{out}");
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
    // sakai's line reads the rule and waits for it; rulec's is no line that waits
    assert_eq!((d[1]["reads"].as_str(), &d[1]["waits_for"]), (Some("billing/rules/請求の要否.rule"), &serde_json::json!(["billing/rules/請求の要否.rule"])));
    assert!(d[0].get("reads").is_none() && d[0].get("waits_for").is_none(), "{d}");
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
    assert!(out.starts_with("error: `bad.rule` is not UTF-8\n"), "{out}");
    assert!(out.contains("1 not checked"), "{out}");
}

/// A directory with no `.cal` file (DESIGN 6.2, item 10): `koyomi check` and `gen` given it stop
/// with exit 2 and say so, through `ritsu koyomi` as well, while `ritsu check`, which reads the
/// files of every language under it, checks the rule there and passes.
#[test]
fn a_directory_without_a_cal_file_stops_koyomi_and_passes_ritsu_check() {
    let t = TempDir::new("no-cal");
    std::fs::create_dir_all(t.path().join("rules")).unwrap();
    let rule = "rule refund_limit v1\n\ninputs\n  left : money[USDc]  range >=0USDc <=10000000USDc\n\noutputs\n  band : number  round down(1)\n\ntable band_of\npolicy unique\n| left       | -> band : number |\n| <=1000USDc | 0                |\n| >1000USDc  | 2                |\n";
    std::fs::write(t.path().join("rules/refund_limit.rule"), rule).unwrap();
    let (code, out, err) = ritsu_in(t.path(), &["check", "rules"]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    assert!(out.starts_with("ok rules/refund_limit.rule\n"), "{out}");
    for args in [&["koyomi", "check", "rules"][..], &["koyomi", "gen", "rules", "--target", "go", "--out", "out"][..]] {
        for (lang, said) in [("en", "error: there is no .cal file under `rules`\n"), ("ja", "エラー: `rules` の下に .cal のファイルがありません\n")] {
            let mut with_lang = args.to_vec();
            with_lang.extend(["--lang", lang]);
            let (code, out, err) = ritsu_in(t.path(), &with_lang);
            assert_eq!((code, out.as_str(), err.as_str()), (2, "", said), "{with_lang:?}");
        }
    }
    assert!(!t.path().join("out").exists());
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
/// rule takes in; rulec says so with its tool in the headline, sakai's line waits for the rule,
/// and the exit is 1.
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
        assert!(out.contains(head) && out.contains("\n  billing/rules/billing_need.rule:5: [sakai E105] enum "), "{out}");
        assert!(!out.contains("[sakai E105]: "), "{out}");
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
    assert_eq!((d[1]["reads"].as_str(), &d[1]["waits_for"]), (Some("billing/rules/billing_need.rule"), &serde_json::json!(["billing/rules/billing_need.rule"])));
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
/// no row for it, the error says which contribution added the kind, and the flow's line that reads
/// the rule waits for it (DESIGN 8.3).
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
    assert!(out.contains("\nwaiting for rules/product_price.rule (rulec E101 at line 13) to pass:\n  pricing.flow:7: [dandori E005] use rule price from \"rules/product_price.rule\"\n"), "{out}");
    assert!(out.ends_with(": 1 fail (1 error), 1 waiting; borders between the languages: 0 checked, 0 undecided\n"), "{out}");
    // A markup written for toys rather than for the new kind: the overlap, the gap and the
    // contribution's example are said once, for the contribution, and the rule says they were. The
    // flow waits for the rule, named with the error at its line, though rulec says it for the
    // contribution (the gap is at the rule's line 13).
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
    assert!(
        out.ends_with(
            "waiting for rules/product_price.rule (rulec E101 at line 13) to pass:\n  pricing.flow:7: [dandori E005] use rule price from \"rules/product_price.rule\"\nritsu check: 5 files (rulec 4, dandori 1): 2 fail (3 errors), 1 waiting; borders between the languages: 0 checked, 0 undecided\n"
        ),
        "{out}"
    );
}

/// The `file` and the `line` of a finding are of one file (DESIGN 8.3, rulec's §15.318). rulec
/// checks a contribution merged into the rule it adds to, and what it then finds at a line of the
/// rule — here the gap of the rule's table, at its line 13 — was written with the contribution as
/// its `file`, the rule's line as its `line`, and the rule in `where`. In a module the finding was
/// filed under the file its line is in; a project that is not a module files it the same way. The
/// text names the file of the line it cites too: the overlap is placed at the rule's row, line 18
/// of the rule, and was said with the contribution's path.
#[test]
fn a_finding_is_filed_under_the_file_its_line_is_in() {
    let dir = here().join("tests/projects/product_line");
    let t = TempDir::new("product-line-places");
    ritsu_testkit::tmp::copy_dir(&dir, t.path());
    let f = t.path().join("rules/contrib/garden_price.rule");
    std::fs::write(&f, std::fs::read_to_string(&f).unwrap().replace("  when item_kind garden", "  when item_kind toys")).unwrap();
    for lang in ["en", "ja"] {
        let (code, out, err) = ritsu_in(t.path(), &["check", "--format", "json", "--lang", lang]);
        assert_eq!(code, 1, "{out}{err}");
        let v = ritsu_base::json::parse(&out).unwrap_or_else(|e| panic!("{e:?}\n{out}"));
        let s = |j: &ritsu_base::json::Json, k: &str| j.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let n = |j: &ritsu_base::json::Json, k: &str| j.get(k).and_then(|x| x.as_int()).unwrap_or(-1);
        let of_rulec: Vec<&ritsu_base::json::Json> = v.get("diagnostics").and_then(|d| d.as_arr()).unwrap().iter().filter(|d| s(d, "tool") == "rulec").collect();
        let at: Vec<(String, String, i128)> = of_rulec.iter().map(|d| (s(d, "code"), s(d, "file"), n(d, "line"))).collect();
        assert_eq!(
            at,
            [
                ("E105".to_string(), "rules/contrib/garden_price.rule".to_string(), 7),
                ("E101".to_string(), "rules/product_price.rule".to_string(), 13),
                ("E107".to_string(), "rules/contrib/garden_price.rule".to_string(), 12),
            ],
            "{out}"
        );
        for d in &of_rulec {
            let w = d.get("where").unwrap();
            assert_eq!((s(d, "file"), n(d, "line"), n(d, "column")), (s(w, "file"), n(w, "line"), n(w, "column")), "{out}");
        }
        // the files that do not pass are the two they were, and the flow waits for the rule
        let files: Vec<(String, bool)> = v.get("files").and_then(|d| d.as_arr()).unwrap().iter().map(|f| (s(f, "file"), f.get("ok").and_then(|x| x.as_bool()).unwrap())).collect();
        assert_eq!(
            files,
            [
                ("rules/contrib/garden_kind.rule".to_string(), true),
                ("rules/contrib/garden_price.rule".to_string(), false),
                ("rules/product_price.rule".to_string(), false),
                ("rules/shared/product_kinds.rule".to_string(), true),
                ("pricing.flow".to_string(), false),
            ],
            "{out}"
        );
    }
    let (code, out, _) = ritsu_in(t.path(), &["check", "--lang", "en"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("error[rulec E105]: Overlapping rows: the same input matches clause garden_price:garden_markup and table markup_of row 3\n  --> rules/product_price.rule:18 table markup_of\n"), "{out}");
    assert!(out.contains("error[rulec E101]: Completeness gap: some input matches no row\n  --> rules/product_price.rule:13 table markup_of\n"), "{out}");
    assert!(out.contains("error[rulec E107]: Example does not hold: price should be 23USD but no value came out\n  --> rules/contrib/garden_price.rule:12 examples\n"), "{out}");
    assert!(
        out.ends_with(
            "waiting for rules/product_price.rule (rulec E101 at line 13) to pass:\n  pricing.flow:7: [dandori E005] use rule price from \"rules/product_price.rule\"\nritsu check: 5 files (rulec 4, dandori 1): 2 fail (3 errors), 1 waiting; borders between the languages: 0 checked, 0 undecided\n"
        ),
        "{out}"
    );
    let (code, out, _) = ritsu_in(t.path(), &["check", "--lang", "ja"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("\n  --> rules/product_price.rule:18 表 markup_of\n"), "{out}");
}

/// A contribution that does not pass (DESIGN 8.3): rulec says what is wrong with it once, at the
/// contribution, and every file that fails for it waits for it. The rule it adds to, whose check
/// fails only for the contribution's error, which rulec says for the contribution ("its errors are
/// said above"), waits for it with no line of its own, and so does each line that reads that rule:
/// the price rule's `import enum`, the other contribution's `to`, the flow's `use rule`. A
/// contribution that cannot be read says it once too, and the rule it adds to waits for it at its
/// first line, where rulec says it cannot read the contribution (E067).
#[test]
fn a_contribution_that_does_not_pass_is_what_the_rest_waits_for() {
    let dir = here().join("tests/projects/product_line");
    let t = TempDir::new("product-line-kinds");
    ritsu_testkit::tmp::copy_dir(&dir, t.path());
    let f = t.path().join("rules/contrib/garden_kind.rule");
    let kind = std::fs::read_to_string(&f).unwrap();
    std::fs::write(&f, kind.replace("extend kind with garden", "extend kinds with garden")).unwrap();
    let (code, out, _) = ritsu_in(t.path(), &["check", "--lang", "en"]);
    assert_eq!(code, 1, "{out}");
    let errors: Vec<&str> = out.lines().filter(|l| l.starts_with("error[")).collect();
    assert_eq!(errors, ["error[rulec E067]: The file it adds to has no enum kinds"], "{out}");
    let waiting = "\
note rules/shared/product_kinds.rule: its errors are said above
waiting for rules/contrib/garden_kind.rule (rulec E067 at line 5) to pass:
  rules/contrib/garden_price.rule:3: [rulec E067] to \"../product_price.rule\" signature:d5379dca48aa13d2
  rules/product_price.rule:4: [rulec E066] import enum kind from \"shared/product_kinds.rule\"
  pricing.flow:7: [dandori E005] use rule price from \"rules/product_price.rule\"
ritsu check: 5 files (rulec 4, dandori 1): 1 fail (1 error), 4 waiting; borders between the languages: 0 checked, 0 undecided
";
    assert!(out.ends_with(waiting), "{out}");
    let (_, out, _) = ritsu_in(t.path(), &["check", "--lang", "ja"]);
    assert!(out.contains("\nrules/contrib/garden_kind.rule（rulec E067、5 行目）が通るのを待っています:\n"), "{out}");
    assert!(out.ends_with("検査を通らないもの 1 個（エラー 1 件）、待っているもの 4 個。言語の境目: 確かめた 0 か所、決められない 0 か所\n"), "{out}");
    let (_, json, _) = ritsu_in(t.path(), &["check", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let none = serde_json::Value::Null;
    let files: Vec<(&str, bool, &serde_json::Value)> = v["files"].as_array().unwrap().iter().map(|f| (f["file"].as_str().unwrap(), f["ok"].as_bool().unwrap(), f.get("waits_for").unwrap_or(&none))).collect();
    let kinds = serde_json::json!(["rules/contrib/garden_kind.rule"]);
    assert_eq!(
        files,
        [
            ("rules/contrib/garden_kind.rule", false, &none),
            ("rules/contrib/garden_price.rule", false, &kinds),
            ("rules/product_price.rule", false, &kinds),
            ("rules/shared/product_kinds.rule", false, &kinds),
            ("pricing.flow", false, &kinds),
        ]
    );

    // a contribution that cannot be read
    std::fs::write(&f, format!("{kind}\n@@@ broken @@@\n")).unwrap();
    let (code, out, _) = ritsu_in(t.path(), &["check", "--lang", "en"]);
    assert_eq!(code, 1, "{out}");
    let errors: Vec<&str> = out.lines().filter(|l| l.starts_with("error[")).collect();
    assert_eq!(errors, ["error[rulec E004]: The line does not start with a word"], "{out}");
    let waiting = "\
waiting for rules/contrib/garden_kind.rule (rulec E004 at line 7) to pass:
  rules/contrib/garden_price.rule:3: [rulec E067] to \"../product_price.rule\" signature:d5379dca48aa13d2
  rules/product_price.rule:4: [rulec E066] import enum kind from \"shared/product_kinds.rule\"
  rules/shared/product_kinds.rule:1: [rulec E067] enums product_kinds(product_kinds) v1
  pricing.flow:7: [dandori E005] use rule price from \"rules/product_price.rule\"
ritsu check: 5 files (rulec 4, dandori 1): 1 fail (1 error), 4 waiting; borders between the languages: 0 checked, 0 undecided
";
    assert!(out.ends_with(waiting), "{out}");
}

/// A `.proto` that cannot be read (DESIGN 8.3): every language reads it with ritsu's one reader,
/// and ritsu says it once (E101). Each line that reads it waits for it: the rule's `import proto`
/// (rulec E013), the flow's `use proto` (dandori E016), and sakai's lines of the file and of the
/// rule (E106, E105); nothing is said of the lines that use the types and the API the file gives.
/// A `.proto` is read with the ones it imports: when one of those cannot be read, the flow's `use
/// proto` of the file that imports it waits for it too, as does the `use proto` of the file itself,
/// and the JSON says each reads the file that cannot be read.
#[test]
fn a_proto_that_cannot_be_read_is_waited_for() {
    let t = TempDir::new("shop-proto-waiting");
    ritsu_testkit::tmp::copy_dir(&here().join(SHOP_EN), t.path());
    let order = t.path().join("proto/shop/ordering/v1/order.proto");
    let kept = std::fs::read_to_string(&order).unwrap();
    std::fs::write(&order, kept.replacen("enum OrderStatus {", "enum OrderStatus {{", 1)).unwrap();
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--lang", "en"]);
    assert_eq!(code, 1, "{out}");
    let errors: Vec<&str> = out.lines().filter(|l| l.starts_with("error[")).collect();
    assert_eq!(errors, ["error[ritsu E101]: proto/shop/ordering/v1/order.proto:14:19: proto/shop/ordering/v1/order.proto does not read as a .proto: a value is expected where `{` is"], "{out}");
    let waiting = "\
waiting for proto/shop/ordering/v1/order.proto (ritsu E101 at line 14) to pass:
  billing/rules/billing_need.rule:4: [rulec E013] import proto \"../../proto/shop/ordering/v1/order.proto\" OrderStatus -> order_status
  ordering/fulfillment.flow:11: [dandori E016] use proto orders from \"../proto/shop/ordering/v1/order.proto\"
  proto/shop/ordering/v1/order.proto:14: [sakai E106] enum OrderStatus {{
  billing/rules/billing_need.rule:4: [sakai E105] import proto \"../../proto/shop/ordering/v1/order.proto\" OrderStatus -> order_status
ritsu check: 21 files (rulec 4, koyomi 3, chobo 1, proto 5, dandori 2, sakai 6): 1 fail (1 error), 2 waiting; borders between the languages: 0 checked, 0 undecided
";
    assert!(out.ends_with(waiting), "{out}");

    std::fs::write(&order, kept).unwrap();
    let stock = t.path().join("proto/warehouse/v1/stock.proto");
    let s = std::fs::read_to_string(&stock).unwrap();
    std::fs::write(&stock, s.replacen("message ReserveRequest {", "message ReserveRequest {{", 1)).unwrap();
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--lang", "en"]);
    assert_eq!(code, 1, "{out}");
    let errors: Vec<&str> = out.lines().filter(|l| l.starts_with("error[")).collect();
    assert_eq!(errors, ["error[ritsu E101]: proto/warehouse/v1/stock.proto:18:25: proto/warehouse/v1/stock.proto does not read as a .proto: a field is expected where `{` is"], "{out}");
    let waiting = "\
waiting for proto/warehouse/v1/stock.proto (ritsu E101 at line 18) to pass:
  ordering/fulfillment.flow:8: [dandori E016] use proto shop from \"../proto/shop/ordering/v1/fulfillment.proto\"
  ordering/fulfillment.flow:9: [dandori E016] use proto warehouse from \"../proto/warehouse/v1/stock.proto\"
  proto/warehouse/v1/stock.proto:18: [sakai E106] message ReserveRequest {{
ritsu check: 21 files (rulec 4, koyomi 3, chobo 1, proto 5, dandori 2, sakai 6): 1 fail (1 error), 1 waiting; borders between the languages: 0 checked, 0 undecided
";
    assert!(out.ends_with(waiting), "{out}");
    let (_, json, _) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let reads: Vec<(&str, u64, &str)> = v["diagnostics"].as_array().unwrap().iter().filter(|d| d.get("waits_for").is_some()).map(|d| (d["file"].as_str().unwrap(), d["line"].as_u64().unwrap(), d["reads"].as_str().unwrap())).collect();
    assert_eq!(
        reads,
        [
            ("ordering/fulfillment.flow", 8, "proto/warehouse/v1/stock.proto"),
            ("ordering/fulfillment.flow", 9, "proto/warehouse/v1/stock.proto"),
            ("proto/warehouse/v1/stock.proto", 18, "proto/warehouse/v1/stock.proto"),
        ],
        "{json}"
    );
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

/// The test project of an expense request's approvals: a dates file of the deadline of a stage,
/// three flows that read it, one directly and two through the flows they run (`flow "…"`), a gate
/// that reads the flow at the top, and requirements whose links point at the dates file. Every
/// file passes.
const APPROVALS: &str = "tests/projects/approvals";

/// A copy of [`APPROVALS`] in `t`, with each of `edits` made: in `file`, the first `name` changed
/// into `into`.
fn approvals_with(t: &TempDir, edits: &[(&str, &str, &str)]) {
    ritsu_testkit::tmp::copy_dir(&here().join(APPROVALS), t.path());
    for (file, name, into) in edits {
        let f = t.path().join(file);
        let s = std::fs::read_to_string(&f).unwrap();
        assert!(s.contains(name), "{file} has no {name}");
        std::fs::write(&f, s.replacen(name, into, 1)).unwrap();
    }
}

/// One claim of the dates file failing: the deadline allows at most 10 days, and the third business
/// day after a request that comes on Christmas Eve is 11 days on (DESIGN 8.3). koyomi's error is
/// said once, in its place. Each line that reads a file that does not pass because of it — the
/// flow's `use dates`, the flows' `flow "…"`, the gate's `workflow … from` and the requirements'
/// links to the dates file — is said once, under the dates file, after the rest, and nothing is
/// said of the lines that would use what they read (no E002 for the names of the dates file). The
/// summary counts the file that fails and the five that wait apart; the exit is 1. The JSON keeps
/// each diagnostic as its language writes it, with the file each line that waits reads and the
/// files it waits for, and gives each file that waits the files it waits for.
#[test]
fn one_mistake_is_said_once_and_what_waits_for_it_after() {
    let (code, out, _) = ritsu_in(&here().join(APPROVALS), &["check", ".", "--root", ".", "--lang", "en"]);
    assert_eq!(code, 0, "{out}");
    let t = TempDir::new("approvals-waiting");
    approvals_with(&t, &[("dates/approval_terms.cal", "due <= received + 11 days", "due <= received + 10 days")]);
    let mut failures = Vec::new();
    for lang in ["en", "ja"] {
        let (code, out, err) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--lang", lang]);
        assert_eq!((code, err.as_str()), (1, ""), "{out}");
        golden(&format!("tests/golden/check/approvals-waiting.{lang}.txt"), &out, &mut failures);
    }
    let (code, json, _) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--format", "json"]);
    assert_eq!(code, 1, "{json}");
    golden("tests/golden/check/approvals-waiting.json", &json, &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));

    let (_, out, _) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--lang", "en"]);
    let errors: Vec<&str> = out.lines().filter(|l| l.starts_with("error[")).collect();
    assert_eq!(errors, ["error[koyomi E301]: dates/approval_terms.cal:19:3: The claim due_within_eleven_days fails for 3 of the 730 days of received"], "{out}");
    let waiting = "\
waiting for dates/approval_terms.cal (koyomi E301 at line 19) to pass:
  flows/approvals.flow:24: [dandori E015] flow \"joint.flow\"
  flows/approvals.flow:29: [dandori E015] flow \"stage.flow\"
  flows/joint.flow:20: [dandori E015] flow \"stage.flow\"
  flows/stage.flow:5: [dandori E005] use dates terms from \"../dates/approval_terms.cal\"
  gates/expense.gate:14: [sekisho E208] workflow approvals from \"../flows/approvals.flow\"
  requirements/expense.req:13: [yuen E203] satisfied by koyomi \"../dates/approval_terms.cal\" date due
  requirements/expense.req:19: [yuen E203] verified by koyomi \"../dates/approval_terms.cal\" claim due_is_open
ritsu check: 7 files (koyomi 2, dandori 3, sekisho 1, yuen 1): 1 fail (1 error), 5 waiting; borders between the languages: 0 checked, 0 undecided
";
    assert!(out.ends_with(waiting), "{out}");
    // yuen's line of the errors goes with them: the requirements have none of their own
    assert!(!out.contains("requirements/expense.req: "), "{out}");

    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    type Diag<'a> = (&'a str, &'a str, &'a str, u64, Option<&'a str>, &'a serde_json::Value);
    let said: Vec<Diag<'_>> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| (d["tool"].as_str().unwrap(), d["code"].as_str().unwrap(), d["file"].as_str().unwrap(), d["line"].as_u64().unwrap(), d["reads"].as_str(), d.get("waits_for").unwrap_or(&serde_json::Value::Null)))
        .collect();
    let terms = serde_json::json!(["dates/approval_terms.cal"]);
    let none = serde_json::Value::Null;
    assert_eq!(
        said,
        [
            ("koyomi", "E301", "dates/approval_terms.cal", 19, None, &none),
            ("dandori", "E015", "flows/approvals.flow", 24, Some("flows/joint.flow"), &terms),
            ("dandori", "E015", "flows/approvals.flow", 29, Some("flows/stage.flow"), &terms),
            ("dandori", "E015", "flows/joint.flow", 20, Some("flows/stage.flow"), &terms),
            ("dandori", "E005", "flows/stage.flow", 5, Some("dates/approval_terms.cal"), &terms),
            ("sekisho", "E208", "gates/expense.gate", 14, Some("flows/approvals.flow"), &terms),
            ("yuen", "E203", "requirements/expense.req", 13, Some("dates/approval_terms.cal"), &terms),
            ("yuen", "E203", "requirements/expense.req", 19, Some("dates/approval_terms.cal"), &terms),
        ]
    );
    // every line that waits is still its language's error
    assert!(v["diagnostics"].as_array().unwrap().iter().all(|d| d["severity"] == "error"), "{json}");
    let files: Vec<(&str, bool, &serde_json::Value)> = v["files"].as_array().unwrap().iter().map(|f| (f["file"].as_str().unwrap(), f["ok"].as_bool().unwrap(), f.get("waits_for").unwrap_or(&none))).collect();
    assert_eq!(
        files,
        [
            ("calendars/office.cal", true, &none),
            ("dates/approval_terms.cal", false, &none),
            ("flows/approvals.flow", false, &terms),
            ("flows/joint.flow", false, &terms),
            ("flows/stage.flow", false, &terms),
            ("gates/expense.gate", false, &terms),
            ("requirements/expense.req", false, &terms),
        ]
    );
    assert_eq!(v["ok"], false);

    // what each file's language says of it, but for the lines that wait
    holds_to_the_languages(t.path(), ".", "en");
    holds_to_the_languages(t.path(), ".", "ja");
}

/// A mistake that has nothing to do with the file a line waits for is said in its place, in a file
/// that waits too (DESIGN 8.3): the flow that runs the stages side by side writes a field its record
/// does not have. That flow fails; its line that reads the stage's flow still waits for the dates
/// file; and the line of the flow at the top that reads it waits for both, as does the gate.
#[test]
fn a_mistake_in_a_file_that_waits_is_said() {
    let t = TempDir::new("approvals-beside");
    approvals_with(
        &t,
        &[("dates/approval_terms.cal", "due <= received + 11 days", "due <= received + 10 days"), ("flows/joint.flow", "\"{s.approver} sent it back\"", "\"{s.approvr} sent it back\"")],
    );
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--lang", "en"]);
    assert_eq!(code, 1, "{out}");
    let errors: Vec<&str> = out.lines().filter(|l| l.starts_with("error[")).collect();
    assert_eq!(
        errors,
        [
            "error[koyomi E301]: dates/approval_terms.cal:19:3: The claim due_within_eleven_days fails for 3 of the 730 days of received",
            "error[dandori E002]: flows/joint.flow:30:34: `StageAnswer` has no field `approvr` (verdict, approver)",
        ],
        "{out}"
    );
    let waiting = "\
waiting for dates/approval_terms.cal (koyomi E301 at line 19) to pass:
  flows/approvals.flow:29: [dandori E015] flow \"stage.flow\"
  flows/joint.flow:20: [dandori E015] flow \"stage.flow\"
  flows/stage.flow:5: [dandori E005] use dates terms from \"../dates/approval_terms.cal\"
  requirements/expense.req:13: [yuen E203] satisfied by koyomi \"../dates/approval_terms.cal\" date due
  requirements/expense.req:19: [yuen E203] verified by koyomi \"../dates/approval_terms.cal\" claim due_is_open
waiting for dates/approval_terms.cal (koyomi E301 at line 19) and flows/joint.flow (dandori E002 at line 30) to pass:
  flows/approvals.flow:24: [dandori E015] flow \"joint.flow\"
  gates/expense.gate:14: [sekisho E208] workflow approvals from \"../flows/approvals.flow\"
ritsu check: 7 files (koyomi 2, dandori 3, sekisho 1, yuen 1): 2 fail (2 errors), 4 waiting; borders between the languages: 0 checked, 0 undecided
";
    assert!(out.ends_with(waiting), "{out}");
    let (_, out, _) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--lang", "ja"]);
    assert!(out.contains("\ndates/approval_terms.cal（koyomi E301、19 行目）と flows/joint.flow（dandori E002、30 行目）が通るのを待っています:\n"), "{out}");
    assert!(out.ends_with("検査を通らないもの 2 個（エラー 2 件）、待っているもの 4 個。言語の境目: 確かめた 0 か所、決められない 0 か所\n"), "{out}");
    let (_, json, _) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let gate = v["files"].as_array().unwrap().iter().find(|f| f["file"] == "gates/expense.gate").unwrap();
    assert_eq!(gate["waits_for"], serde_json::json!(["dates/approval_terms.cal", "flows/joint.flow"]), "{json}");
    let joint = v["files"].as_array().unwrap().iter().find(|f| f["file"] == "flows/joint.flow").unwrap();
    assert!(joint.get("waits_for").is_none() && joint["ok"] == false, "{joint}");
    holds_to_the_languages(t.path(), ".", "en");
}

/// Every pair of languages waits the same way (DESIGN 8.3). A rule that does not pass (two rows of
/// its table overlap), read by a flow, a gate and a module of kaname: rulec's error once, and the
/// three `use rule` lines under it. A calendar that does not pass, read by a dates file that a flow
/// reads, and a book that does not pass, read by the same flow: the dates file's `use calendar` and
/// the flow's `use dates` wait for the calendar, and the flow's `use book` for the book.
#[test]
fn every_pair_of_languages_waits_the_same_way() {
    let t = TempDir::new("cold-chain-waiting");
    ritsu_testkit::tmp::copy_dir(&here().join("tests/projects/cold_chain"), t.path());
    let rule = t.path().join("rules/cold_chain_departure.rule");
    let s = std::fs::read_to_string(&rule).unwrap();
    std::fs::write(&rule, s.replacen("| controlled_ambient | >25℃       | -        | false         |", "| controlled_ambient | >=25℃      | -        | false         |", 1)).unwrap();
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--lang", "en"]);
    assert_eq!(code, 1, "{out}");
    let errors: Vec<&str> = out.lines().filter(|l| l.starts_with("error[")).collect();
    assert_eq!(errors, ["error[rulec E105]: Overlapping rows: the same input matches row 6 and row 7"], "{out}");
    let waiting = "\
waiting for rules/cold_chain_departure.rule (rulec E105 at line 22) to pass:
  departure.kn:6: [kaname E201] use rule cold_chain_departure from \"rules/cold_chain_departure.rule\"
  cold_chain.flow:4: [dandori E005] use rule departure from \"rules/cold_chain_departure.rule\"
  cold_chain.gate:5: [sekisho E201] use rule cold_chain_departure from \"rules/cold_chain_departure.rule\"
ritsu check: 8 files (rulec 2, kaname 2, dandori 2, sekisho 2): 1 fail (1 error), 3 waiting; borders between the languages: 0 checked, 0 undecided
";
    assert!(out.ends_with(waiting), "{out}");
    holds_to_the_languages(t.path(), ".", "en");
    holds_to_the_languages(t.path(), ".", "ja");

    let t = TempDir::new("invoice-waiting");
    ritsu_testkit::tmp::copy_dir(&here().join("tests/projects/invoice"), t.path());
    let calendar = t.path().join("calendars/weekdays.cal");
    std::fs::write(&calendar, std::fs::read_to_string(&calendar).unwrap().replacen("closed weekly sat, sun\n", "closed weekly sat, sunday\n", 1)).unwrap();
    let book = t.path().join("books/stock.book");
    std::fs::write(&book, std::fs::read_to_string(&book).unwrap().replacen("  at least 0 refused as out_of_stock\n", "", 1)).unwrap();
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--lang", "en"]);
    assert_eq!(code, 1, "{out}");
    let errors: Vec<&str> = out.lines().filter(|l| l.starts_with("error[")).map(|l| l.split(": ").next().unwrap()).collect();
    assert_eq!(errors, ["error[koyomi E002]", "error[chobo E020]"], "{out}");
    let waiting = "\
waiting for calendars/weekdays.cal (koyomi E002 at line 5) to pass:
  dates/payment_terms.cal:3: [koyomi E015] use calendar \"../calendars/weekdays.cal\"
  invoice.flow:11: [dandori E005] use dates terms from \"dates/payment_terms.cal\"
waiting for books/stock.book (chobo E020 at line 8) to pass:
  invoice.flow:12: [dandori E005] use book stock from \"books/stock.book\"
ritsu check: 10 files (rulec 2, koyomi 4, chobo 2, dandori 2): 2 fail (2 errors), 2 waiting, 6 pass (6 warnings); borders between the languages: 6 checked, 6 undecided
";
    assert!(out.ends_with(waiting), "{out}");
}

/// A request waits for what it reads as every other language does (ringi's DESIGN 6.5): the test
/// project of two requests, one that reads a rule and a calendar, one that uses a shared route that
/// reads the same calendar, every file passing as it is. A rule whose rows overlap, a calendar that
/// does not read and a shared route that names an approver it does not declare: each language's
/// error once, in its place, and under it each `use` line that reads it (ringi's E202); the request
/// that uses the shared route waits for the calendar too, through the route.
#[test]
fn a_request_waits_for_what_it_reads() {
    let (code, out, _) = ritsu_in(&here().join("tests/projects/requests"), &["check", ".", "--root", ".", "--lang", "en"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.ends_with("ritsu check: 5 files (rulec 1, koyomi 1, ringi 3): all pass; borders between the languages: 0 checked, 0 undecided\n"), "{out}");
    let t = TempDir::new("requests-waiting");
    ritsu_testkit::tmp::copy_dir(&here().join("tests/projects/requests"), t.path());
    for (file, name, into) in [
        ("rules/authority.rule", "| goods    | >1_000_000JPY  |", "| goods    | >=1_000_000JPY |"),
        ("calendars/office.cal", "closed weekly sat, sun\n", "closed weekly sat, sunday\n"),
        ("routes/standard.ringi", "  head  heads  when long", "  head  head_of_team  when long"),
    ] {
        let f = t.path().join(file);
        let s = std::fs::read_to_string(&f).unwrap();
        assert!(s.contains(name), "{file} has no {name}");
        std::fs::write(&f, s.replacen(name, into, 1)).unwrap();
    }
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--lang", "en"]);
    assert_eq!(code, 1, "{out}");
    let errors: Vec<&str> = out.lines().filter(|l| l.starts_with("error[")).map(|l| l.split(": ").next().unwrap()).collect();
    assert_eq!(errors, ["error[rulec E105]", "error[koyomi E002]", "error[ringi E004]"], "{out}");
    let waiting = "\
waiting for rules/authority.rule (rulec E105 at line 17) to pass:
  purchase.ringi:5: [ringi E202] use rule authority from \"rules/authority.rule\"
waiting for calendars/office.cal (koyomi E002 at line 5) to pass:
  leave.ringi:4: [ringi E202] use calendar office from \"calendars/office.cal\"
  purchase.ringi:4: [ringi E202] use calendar office from \"calendars/office.cal\"
  routes/standard.ringi:4: [ringi E202] use calendar office from \"../calendars/office.cal\"
waiting for calendars/office.cal (koyomi E002 at line 5) and routes/standard.ringi (ringi E004 at line 11) to pass:
  leave.ringi:5: [ringi E202] use route standard from \"routes/standard.ringi\"
ritsu check: 5 files (rulec 1, koyomi 1, ringi 3): 3 fail (3 errors), 2 waiting; borders between the languages: 0 checked, 0 undecided
";
    assert!(out.ends_with(waiting), "{out}");
    holds_to_the_languages(t.path(), ".", "en");
    holds_to_the_languages(t.path(), ".", "ja");
}

/// `ritsu check --through <day>` holds every request of ringi's to being usable through that day,
/// as `ritsu ringi check --through` does (ringi's DESIGN 9.6, 16.1): the test project of a request
/// that reads an organization and masters, with the rows of its account codes ending. Without the
/// day the end is a fact under the request's `ok`; through a later day it is ringi's E107.
#[test]
fn ritsu_check_holds_the_requests_to_a_day() {
    let t = TempDir::new("requests-through");
    ritsu_testkit::tmp::copy_dir(&here().join("tests/projects/requests-org"), t.path());
    std::fs::write(t.path().join("masters/accounts.csv"), "code,label,valid_from,valid_to\n5101,Travel,2026-01-01,2027-03-31\n5103,Supplies,2026-01-01,2027-03-31\n").unwrap();
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--lang", "en"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("\n  used through 2027-03-31: the rows of the master `m.accounts` end on 2027-03-31 (`masters/accounts.csv`, line 2)\n"), "{out}");
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--lang", "en", "--through", "2027-03-31"]);
    assert_eq!(code, 0, "{out}");
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--lang", "en", "--through", "2028-03-31"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("error[ringi E107]: expense.ringi:9:3: The request cannot be used through 2028-03-31 (`--through`): it can be used through 2027-03-31\n"), "{out}");
    // the warning is W506; the president's `check` skipped at the top is what the shape of the
    // organization does, counted under the line of `ok` when the file passes
    assert!(out.ends_with("ritsu check: 3 files (ringi 3): 1 fail (1 error, 1 warning); borders between the languages: 0 checked, 0 undecided\n"), "{out}");
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--root", ".", "--lang", "ja", "--through", "2028-03-31"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("expense.ringi:9:3: 2028-03-31（`--through`）まで使えません。使える最後の日は 2027-03-31 です\n"), "{out}");
    let (code, _, err) = ritsu_in(t.path(), &["check", ".", "--through", "soon"]);
    // the words of `ritsu ringi check --through` (the same example of a date)
    assert_eq!((code, err.as_str()), (2, "error: `--through soon` is not a date such as 2027-04-01\n"));
}
