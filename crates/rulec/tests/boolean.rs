//! A truth value at the door of the generated code, in every language (§15.203).
//!
//! A truth value is JSON's `true` or `false` and nothing else. Before, the languages split on
//! anything else: the string `"false"` was true in Python, Ruby, PHP, NumPy and SQL, false in
//! TypeScript, JavaScript, Rust, Go, Swift, Java and Wasm, and only the MCP servers refused it.
//! Every door refuses it now, the way it refuses a number that is not one, and with the same
//! sentence in every language — in the language the code was generated in. An optional truth
//! value takes `null` as the absent value and holds any other value to the same.
//!
//! The materials: `tests/corpus/express_delivery_quote.rule` (`express : bool`, read from a
//! `.proto` contract) and `tests/optional/parcel_cover.rule` (`signature : bool?`), English
//! first, with `tests/corpus/速達の見積.rule` and `tests/optional/小包の補償.rule` beside them.

use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, need, ready, skip};
use rulec::json::Json;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// `rulec` with its messages in `lang` and, when there is one, `tmp` as its TMPDIR: `rulec test`
/// asks swiftc for its version, and swiftc leaves an empty directory in its TMPDIR almost every
/// time.
fn rulec(lang: &str, tmp: Option<&Path>, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rulec"));
    c.env("RULEC_LANG", lang).current_dir(root()).args(args);
    if let Some(t) = tmp {
        c.env("TMPDIR", t);
    }
    let o = c.output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// Whether a command answers, with a TMPDIR of its own for swiftc's leftovers.
fn have(cmd: &str, args: &[&str]) -> bool {
    let tmp = TempDir::new("boolean-probe");
    Command::new(cmd).args(args).env("TMPDIR", tmp.path()).output().map(|o| o.status.success()).unwrap_or(false)
}

/// A rule with a truth value among its inputs: the language its messages are generated in, the
/// alias its files are named by, the input, an answered call as a vector writes it with the
/// input's value left as `@`, and the sentence every language refuses a value that is not one
/// with.
struct Case {
    rule: &'static str,
    lang: &'static str,
    alias: &'static str,
    input: &'static str,
    call: &'static str,
    said: &'static str,
}

const EXPRESS: [Case; 2] = [
    Case {
        rule: "tests/corpus/express_delivery_quote.rule",
        lang: "en",
        alias: "express_delivery_quote",
        input: "express",
        call: r#"{"in":{"weight":1000,"express":@,"declared":0,"cover":0}}"#,
        said: "express is not a boolean",
    },
    Case {
        rule: "tests/corpus/速達の見積.rule",
        lang: "ja",
        alias: "express_quote",
        input: "速達",
        call: r#"{"in":{"重さ":1000,"速達":@,"申告額":0,"補償額":0}}"#,
        said: "速達 が真偽ではありません",
    },
];

const SIGNATURE: [Case; 2] = [
    Case {
        rule: "tests/optional/parcel_cover.rule",
        lang: "en",
        alias: "parcel_cover",
        input: "signature",
        call: r#"{"in":{"declared":null,"pieces":null,"ship_on":null,"signature":@}}"#,
        said: "signature is not a boolean",
    },
    Case {
        rule: "tests/optional/小包の補償.rule",
        lang: "ja",
        alias: "parcel_cover_ja",
        input: "署名",
        call: r#"{"in":{"申告額":null,"個数":null,"発送日":null,"署名":@}}"#,
        said: "署名 が真偽ではありません",
    },
];

/// What is not a truth value: the two strings that look like one, the two numbers that C and
/// SQLite take for one, and a value of no scalar kind at all.
const NOT_ONE: [&str; 5] = [r#""false""#, r#""true""#, "1", "0", "[]"];

/// `rulec gen` of one rule into a directory of its own, in the case's language.
fn generate(c: &Case) -> (TempDir, PathBuf) {
    let t = TempDir::new("boolean");
    let out = t.path().join("out");
    let (code, said, e) = rulec(c.lang, None, &["gen", c.rule, "--out", &out.to_string_lossy()]);
    assert_eq!(code, 0, "{}: {said}{e}", c.rule);
    (t, out)
}

/// `rulec test` over a generated directory, as JSON, with a SKIP line for whatever it did not run.
fn rulec_test(out: &Path) -> Json {
    let (_, said, e) = rulec("en", Some(&tmpdir_in(out)), &["test", &out.to_string_lossy(), "--format", "json"]);
    let j = rulec::json::parse(said.lines().next().unwrap_or_else(|| panic!("no result\n{e}"))).unwrap_or_else(|x| panic!("{x}: {said}"));
    for why in j.get("skipped").and_then(|v| v.as_arr()).unwrap_or_default() {
        skip(&format!("rulec test: {}", why.as_str().unwrap_or_default()));
    }
    j
}

/// Each refused call through `rulec test`: every way it reaches the rule — the runners, the WASI
/// module, the function on PostgreSQL, the MCP servers over stdio and HTTP, the Connect service —
/// has to stop without an answer, and the vectors keep their answers.
fn refused_everywhere(c: &Case, values: &[&str]) {
    let (_t, out) = generate(c);
    let lines: Vec<String> = values.iter().map(|v| c.call.replace('@', v)).collect();
    std::fs::write(out.join("vectors").join(format!("{}.refused.jsonl", c.alias)), lines.join("\n") + "\n").unwrap();
    let j = rulec_test(&out);
    let mut ran = Vec::new();
    for r in j.get("results").and_then(|v| v.as_arr()).unwrap_or_default() {
        if r.get("rule").and_then(|v| v.as_str()) != Some(c.alias) || r.get("ran").and_then(|v| v.as_bool()) != Some(true) {
            continue;
        }
        assert_eq!(r.get("ok").and_then(|v| v.as_bool()), Some(true), "{}: {r:?}", c.rule);
        assert_eq!(r.get("refused").and_then(|v| v.as_int()), Some(values.len() as i128), "{}: not every call was tried: {r:?}", c.rule);
        ran.push(format!("{}/{}", r.get("lang").and_then(|v| v.as_str()).unwrap_or_default(), r.get("via").and_then(|v| v.as_str()).unwrap_or_default()));
    }
    for via in ["python/runner", "python/mcp", "typescript/runner", "numpy/runner"] {
        assert!(ran.iter().any(|x| x == via), "{}: {via} did not run: {ran:?}", c.rule);
    }
    println!("boolean: {} refused by {}", c.rule, ran.join(", "));
}

/// A truth value that is not one is refused by every way the rule is reached, and the vectors
/// are answered as before.
#[test]
fn every_door_refuses_a_truth_value_that_is_not_one() {
    if !ready(Need::Python, || have("python3", &["--version"]), "python3 is not here") {
        return;
    }
    for c in &EXPRESS {
        refused_everywhere(c, &[NOT_ONE[0], NOT_ONE[1], NOT_ONE[2], NOT_ONE[3], NOT_ONE[4], "null"]);
    }
}

/// An optional truth value takes `null` as the absent value — the vectors pass it — and holds
/// anything else to `true` and `false`; the string "null" is not the absent value.
#[test]
fn an_optional_truth_value_takes_null_and_nothing_else() {
    if !ready(Need::Python, || have("python3", &["--version"]), "python3 is not here") {
        return;
    }
    for c in &SIGNATURE {
        refused_everywhere(c, &[NOT_ONE[1], NOT_ONE[3], r#""null""#]);
    }
}

/// Each language's runner, handed a truth value that is not one, stops and says the same sentence,
/// in the language the rule's messages were generated in; `false` itself is answered.
#[test]
fn every_language_says_the_same_sentence() {
    // Every language there is a toolchain for, the tools level's (ritsu's DESIGN 10.2).
    if !need(Need::Python) {
        return;
    }
    let ready_b = |b: &rulec::backend::Backend| b.ready.map(|r| r()).unwrap_or(Ok(()));
    let present: Vec<&rulec::backend::Backend> =
        rulec::backend::ALL.iter().filter(|b| have(b.tool, &["--version"]) || have(b.tool, &["version"])).filter(|b| ready_b(b).is_ok()).collect();
    for b in rulec::backend::ALL {
        if !present.iter().any(|p| p.id == b.id) {
            skip(&format!("{} is not here; {} is not run", b.tool, b.name));
        }
    }
    println!("boolean: {}", present.iter().map(|b| b.name).collect::<Vec<_>>().join(", "));
    for c in EXPRESS.iter().chain(SIGNATURE.iter()) {
        let (_t, out) = generate(c);
        let pkg = c.alias.replace('_', "");
        let one = out.join("vectors").join(".one.jsonl");
        for b in &present {
            let plan = (b.run)(c.alias, &pkg);
            let cwd = out.join(&plan.cwd);
            if !cwd.exists() {
                continue;
            }
            if let Some((cmd, args)) = &plan.build {
                let built = Command::new(cmd).current_dir(&cwd).env("TMPDIR", tmpdir_in(&out)).args(args).output().unwrap_or_else(|e| panic!("{}: {cmd}: {e}", b.name));
                assert!(built.status.success(), "{}: {} does not build:\n{}", c.rule, b.name, String::from_utf8_lossy(&built.stderr));
            }
            let run = |value: &str| -> (bool, String) {
                std::fs::write(&one, format!("{}\n", c.call.replace('@', value))).unwrap();
                let o = Command::new(&plan.cmd)
                    .current_dir(&cwd)
                    .env("TMPDIR", tmpdir_in(&out))
                    .args(&plan.args)
                    .stdin(std::fs::File::open(&one).unwrap())
                    .output()
                    .unwrap_or_else(|e| panic!("{}: {}: {e}", c.rule, b.name));
                (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
            };
            for value in NOT_ONE {
                let (answered, said) = run(value);
                assert!(!answered, "{}: {} answered {} = {value}:\n{said}", c.rule, b.name, c.input);
                assert!(said.contains(c.said), "{}: {} does not say `{}` of {value}:\n{said}", c.rule, b.name, c.said);
            }
            let (answered, said) = run("false");
            assert!(answered, "{}: {} refused {} = false:\n{said}", c.rule, b.name, c.input);
        }
    }
}

/// The projection functions hand a truth value read from the caller's object to the door as it
/// came, where they turned it into one with `bool(…)`, `Boolean(…)`, `? true : false` and
/// `(bool)`: the string "false" in the contract's field was true in four of the five.
#[test]
fn a_projected_truth_value_is_held_to_the_door() {
    if !ready(Need::Python, || have("python3", &["--version"]), "python3 is not here") {
        return;
    }
    for c in &EXPRESS {
        let (_t, out) = generate(c);
        let module: String = c.alias.split('_').map(|w| w[..1].to_uppercase() + &w[1..]).collect();
        let alias = c.alias;
        // the object the contract describes, in protojson's form
        let object = |express: &str| format!(r#"{{"weightG":"1000","express":{express},"declaredJpy":"0","coverJpy":"0"}}"#);
        let mut tried = Vec::new();
        for (express, refused) in [(r#""false""#, true), ("1", true), ("false", false)] {
            let case = object(express);
            let mut runs: Vec<(&str, Command)> = Vec::new();
            let py = format!(
                "import json, {alias} as m\ntry:\n    print(m.{alias}_from(json.loads({case:?})))\nexcept m.RuleInputError as e:\n    print('refused', e.what)\n"
            );
            let mut p = Command::new("python3");
            p.current_dir(out.join("python")).args(["-B", "-c", &py]);
            runs.push(("Python", p));
            if have("node", &["--version"]) {
                for (lang, sub, file) in [("JavaScript", "javascript", format!("{alias}.mjs")), ("TypeScript", "typescript", format!("{alias}.ts"))] {
                    let js = format!(
                        "import('./{file}').then(m => {{ try {{ console.log(String(m.{alias}_from(JSON.parse({case:?})))); }} catch (e) {{ console.log('refused', e.what); }} }});"
                    );
                    let mut n = Command::new("node");
                    n.current_dir(out.join(sub)).args(["--no-warnings", "--input-type=module", "-e", &js]);
                    runs.push((lang, n));
                }
            }
            if have("ruby", &["--version"]) {
                let rb = format!(
                    "require 'json'\nrequire './{alias}.rb'\nbegin\n  puts {module}.{alias}_from(JSON.parse({case:?}))\nrescue {module}::RuleInputError => e\n  puts \"refused #{{e.what}}\"\nend\n"
                );
                let mut r = Command::new("ruby");
                r.current_dir(out.join("ruby")).args(["-e", &rb]);
                runs.push(("Ruby", r));
            }
            if have("php", &["--version"]) {
                let php = format!(
                    "require './{alias}.php'; try {{ echo \\{module}\\{alias}_from(json_decode('{}', true)), \"\\n\"; }} catch (\\{module}\\RuleInputError $e) {{ echo 'refused ', $e->what, \"\\n\"; }}",
                    case.replace('\\', "\\\\").replace('\'', "\\'")
                );
                let mut h = Command::new("php");
                h.current_dir(out.join("php")).args(["-r", &php]);
                runs.push(("PHP", h));
            }
            for (lang, mut cmd) in runs {
                let o = cmd.output().unwrap_or_else(|e| panic!("{lang}: {e}"));
                let said = String::from_utf8_lossy(&o.stdout).trim().to_string();
                assert!(o.status.success(), "{}: {lang} failed on {case}:\n{said}\n{}", c.rule, String::from_utf8_lossy(&o.stderr));
                if refused {
                    assert_eq!(said, format!("refused {}", c.said), "{}: {lang} on {case}", c.rule);
                } else {
                    assert!(!said.starts_with("refused"), "{}: {lang} refused {case}: {said}", c.rule);
                }
                tried.push(lang);
            }
        }
        println!("boolean: {} projected by {}", c.rule, tried.join(", "));
    }
}
