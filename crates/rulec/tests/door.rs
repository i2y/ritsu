//! The door of the generated code, in every language (§15.200).
//!
//! What the proof assumed is refused at run time by the code generated in every language: an
//! input outside its type, enum or range, a combination a `constraint` rules out and a day a
//! koyomi date does not come to — and, the other guard, an element that two rows the checker
//! could not prove apart both match (W114). The NumPy plan carried none of the last three, and
//! its evaluator took a number that was not whole for the integer below it: it answered a
//! declared value above the cover, and every other language refused it. It refuses all of them
//! now, with the sentence the other languages raise, in the language the plan was written in,
//! and with the place of the element it is about.
//!
//! The materials: `tests/corpus/express_delivery_quote.rule` (two amounts) and
//! `tests/date_constraint/hotel_stay.rule` (two dates), English first, with
//! `tests/corpus/速達の見積.rule` and `tests/date_constraint/宿泊料金.rule` beside them;
//! `tests/days/settlement.rule` for the days of a koyomi date; `tests/pages/half_bound.rule` and
//! its Japanese version `tests/mutants/m_w114.rule` for the W114 guard.

use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, ready, skip};
use rulec::json::Json;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

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

/// Whether a command answers. swiftc leaves an empty directory in its TMPDIR almost every time it
/// answers `--version`, so the probe gets a TMPDIR of its own, which goes when this returns.
fn have(cmd: &str, args: &[&str]) -> bool {
    let tmp = TempDir::new("door-probe");
    Command::new(cmd).args(args).env("TMPDIR", tmp.path()).output().map(|o| o.status.success()).unwrap_or(false)
}

/// Whether the NumPy plan can be run here: `python3` with `numpy`.
fn numpy_here() -> bool {
    ready(Need::Python, || have("python3", &["-c", "import numpy"]), "python3 with numpy is not here; the NumPy plan is not run")
}

/// A rule written for the door, with the language its messages are generated in, the alias its
/// files are named by, an input that breaks its constraint as a vector writes it, and the
/// sentence every language refuses that input with.
struct Case {
    rule: &'static str,
    lang: &'static str,
    alias: &'static str,
    broken: &'static str,
    said: &'static str,
}

const CASES: [Case; 4] = [
    Case {
        rule: "tests/corpus/express_delivery_quote.rule",
        lang: "en",
        alias: "express_delivery_quote",
        broken: r#"{"in":{"weight":1000,"express":false,"declared":200000,"cover":100000}}"#,
        said: "the constraint does not hold: declared <= cover",
    },
    Case {
        rule: "tests/corpus/速達の見積.rule",
        lang: "ja",
        alias: "express_quote",
        broken: r#"{"in":{"重さ":1000,"速達":false,"申告額":200000,"補償額":100000}}"#,
        said: "制約が成り立ちません: 申告額 <= 補償額",
    },
    Case {
        rule: "tests/date_constraint/hotel_stay.rule",
        lang: "en",
        alias: "hotel_stay",
        broken: r#"{"in":{"check_in":"2026-08-03","check_out":"2026-08-01"}}"#,
        said: "the constraint does not hold: check_in <= check_out",
    },
    Case {
        rule: "tests/date_constraint/宿泊料金.rule",
        lang: "ja",
        alias: "hotel_stay_ja",
        broken: r#"{"in":{"チェックイン日":"2026-08-03","チェックアウト日":"2026-08-01"}}"#,
        said: "制約が成り立ちません: チェックイン日 <= チェックアウト日",
    },
];

/// `rulec gen` of one rule into a directory of its own, in the case's language.
fn generate(c: &Case) -> (TempDir, PathBuf) {
    let t = TempDir::new("door");
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

/// Every language, held by `rulec test` to the vectors and to refusing the input that breaks the
/// constraint: as a runner, as a WASI module, as a function on PostgreSQL, as an MCP server. The
/// answers to the vectors are the ones they were — what changed is the door.
#[test]
fn every_language_refuses_an_input_that_breaks_a_constraint() {
    if !ready(Need::Python, || have("python3", &["--version"]), "python3 is not here") {
        return;
    }
    for c in &CASES {
        let (_t, out) = generate(c);
        std::fs::write(out.join("vectors").join(format!("{}.refused.jsonl", c.alias)), format!("{}\n", c.broken)).unwrap();
        let j = rulec_test(&out);
        let mut ran = Vec::new();
        for r in j.get("results").and_then(|v| v.as_arr()).unwrap_or_default() {
            if r.get("rule").and_then(|v| v.as_str()) != Some(c.alias) || r.get("ran").and_then(|v| v.as_bool()) != Some(true) {
                continue;
            }
            assert_eq!(r.get("ok").and_then(|v| v.as_bool()), Some(true), "{}: {r:?}", c.rule);
            assert_eq!(r.get("refused").and_then(|v| v.as_int()), Some(1), "{}: the input was not tried: {r:?}", c.rule);
            ran.push(format!("{}/{}", r.get("lang").and_then(|v| v.as_str()).unwrap_or_default(), r.get("via").and_then(|v| v.as_str()).unwrap_or_default()));
        }
        assert!(ran.iter().any(|x| x.starts_with("numpy/")), "{}: NumPy did not run: {ran:?}", c.rule);
        assert!(ran.len() >= 2, "{}: {ran:?}", c.rule);
        println!("door: {} refused by {}", c.rule, ran.join(", "));
    }
}

/// Each language's runner, handed the input that breaks the constraint, stops and says the same
/// sentence — the one the rule's messages were generated in.
#[test]
fn every_language_says_the_same_sentence() {
    let ready_b = |b: &rulec::backend::Backend| b.ready.map(|r| r()).unwrap_or(Ok(()));
    let present: Vec<&rulec::backend::Backend> =
        rulec::backend::ALL.iter().filter(|b| have(b.tool, &["--version"]) || have(b.tool, &["version"])).filter(|b| ready_b(b).is_ok()).collect();
    for b in rulec::backend::ALL {
        if !present.iter().any(|p| p.id == b.id) {
            skip(&format!("{} is not here; {} is not run", b.tool, b.name));
        }
    }
    assert!(present.iter().any(|b| b.id == "numpy") || !numpy_here(), "NumPy is here and was not picked");
    println!("door: {}", present.iter().map(|b| b.name).collect::<Vec<_>>().join(", "));
    for c in &CASES {
        let (_t, out) = generate(c);
        let one = out.join("vectors").join(".broken.jsonl");
        std::fs::write(&one, format!("{}\n", c.broken)).unwrap();
        let pkg = c.alias.replace('_', "");
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
            let o = Command::new(&plan.cmd)
                .current_dir(&cwd)
                .env("TMPDIR", tmpdir_in(&out))
                .args(&plan.args)
                .stdin(std::fs::File::open(&one).unwrap())
                .output()
                .unwrap_or_else(|e| panic!("{}: {}: {e}", c.rule, b.name));
            let said = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
            assert!(!o.status.success(), "{}: {} answered an input that breaks the constraint:\n{said}", c.rule, b.name);
            assert!(said.contains(c.said), "{}: {} does not say `{}`:\n{said}", c.rule, b.name, c.said);
        }
    }
}

/// What the NumPy plan raises for each of a list of calls, one line each: the class and the
/// message, or the answer.
const TRY: &str = "import sys, json, numpy as np, rulec_np\n\
r = rulec_np.load(sys.argv[1])\n\
for cols in json.loads(sys.argv[2]):\n\
\x20   try:\n\
\x20       out = r(**{k: np.array(v) for k, v in cols.items()})\n\
\x20       print('answered', {k: v.tolist() for k, v in out.items()})\n\
\x20   except Exception as e:\n\
\x20       print(type(e).__name__, '|', e)\n";

fn try_numpy(out: &Path, plan: &str, calls: &str) -> Vec<String> {
    let o = Command::new("python3")
        .current_dir(out.join("numpy"))
        .args(["-B", "-c", TRY, plan, calls])
        .output()
        .expect("cannot start python3");
    assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).lines().map(String::from).collect()
}

/// The NumPy plan refuses the element that breaks the constraint, names its place, and says it in
/// the language the plan was written in; the elements around it are not what is refused.
#[test]
fn numpy_names_the_element_it_refuses() {
    if !numpy_here() {
        return;
    }
    let express = &CASES[0];
    let (_t, out) = generate(express);
    let got = try_numpy(
        &out,
        "express_delivery_quote.json",
        r#"[{"weight":[1000,1000],"express":[false,false],"declared":[0,200000],"cover":[0,100000]},
            {"weight":[1000],"express":[false],"declared":[100000],"cover":[100000]}]"#,
    );
    assert_eq!(got, ["RuleInputError | the constraint does not hold: declared <= cover (row 1)", "answered {'fee': [1300]}"], "{got:?}");
    let japanese = &CASES[1];
    let (_t, out) = generate(japanese);
    let got = try_numpy(
        &out,
        "express_quote.json",
        r#"[{"重さ":[1000],"速達":[false],"申告額":[200000],"補償額":[100000]},{"重さ":[50000],"速達":[false],"申告額":[0],"補償額":[0]}]"#,
    );
    assert_eq!(got, ["RuleInputError | 制約が成り立ちません: 申告額 <= 補償額 (行 0)", "RuleInputError | 重さ が範囲の外です: 50000 (行 0)"], "{got:?}");
    let dates = &CASES[2];
    let (_t, out) = generate(dates);
    let got = try_numpy(
        &out,
        "hotel_stay.json",
        r#"[{"check_in":["2026-08-01","2026-08-03"],"check_out":["2026-08-03","2026-08-01"]},{"check_in":["2026-13-01"],"check_out":["2026-12-31"]}]"#,
    );
    assert_eq!(got, ["RuleInputError | the constraint does not hold: check_in <= check_out (row 1)", "RuleInputError | check_in is not a date: '2026-13-01' (row 0)"], "{got:?}");
}

/// A number that is not whole is refused before its range is looked at, as every other door
/// refuses it: a float, a boolean, a string. A float that is a whole number is the integer it
/// equals, as SQL's door reads it.
#[test]
fn numpy_refuses_a_number_that_is_not_whole() {
    if !numpy_here() {
        return;
    }
    let (_t, out) = generate(&CASES[0]);
    let got = try_numpy(
        &out,
        "express_delivery_quote.json",
        r#"[{"weight":[1000.5],"express":[false],"declared":[0],"cover":[0]},
            {"weight":[true],"express":[false],"declared":[0],"cover":[0]},
            {"weight":["1000"],"express":[false],"declared":[0],"cover":[0]},
            {"weight":[1000.0],"express":[false],"declared":[0],"cover":[0]}]"#,
    );
    assert_eq!(
        got,
        [
            "RuleInputError | weight is not an integer: 1000.5 (row 0)",
            "RuleInputError | weight is not an integer: True (row 0)",
            "RuleInputError | weight is not an integer: '1000' (row 0)",
            "answered {'fee': [700]}",
        ],
        "{got:?}"
    );
}

/// A day a koyomi date does not come to is refused by the NumPy plan, with the sentence the other
/// languages raise; and every language, held by `rulec test` to refusing it, does (koyomi joined
/// as `ritsu rulec` joins it).
#[test]
fn every_language_refuses_a_day_koyomi_does_not_come_to() {
    if !ready(Need::Python, || have("python3", &["--version"]), "python3 is not here") {
        return;
    }
    let path = root().join("tests/days/settlement.rule");
    let t = TempDir::new("door-days");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let port: rulec::days::Port = Arc::new(koyomi::ports::Engine);
    let code = rulec::days::with(Some(port), || rulec::codegen::generate(&[path.to_str().unwrap()], t.path().to_str().unwrap(), false, false, &mut out, &mut err));
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    if numpy_here() {
        let got = try_numpy(t.path(), "settlement.json", r#"[{"pay_day":["2026-02-10","2026-02-16"]},{"pay_day":["2026-02-10"]}]"#);
        assert_eq!(got, ["RuleInputError | pay_day is not a day payment of payment_terms.cal comes to (row 1)", "answered {'batch': ['first_half']}"], "{got:?}");
    }
    std::fs::write(t.path().join("vectors/settlement.refused.jsonl"), "{\"in\":{\"pay_day\":\"2026-02-16\"}}\n").unwrap();
    let j = rulec_test(t.path());
    let mut ran = 0;
    for r in j.get("results").and_then(|v| v.as_arr()).unwrap_or_default() {
        if r.get("rule").and_then(|v| v.as_str()) != Some("settlement") || r.get("ran").and_then(|v| v.as_bool()) != Some(true) {
            continue;
        }
        assert_eq!(r.get("ok").and_then(|v| v.as_bool()), Some(true), "{r:?}");
        assert_eq!(r.get("refused").and_then(|v| v.as_int()), Some(1), "{r:?}");
        ran += 1;
    }
    assert!(ran >= 2, "{j:?}");
}

/// Where two rows of a `unique` table the checker could not prove apart both match, the NumPy plan
/// stops as the generated Python does, with the same sentence. No whole amount makes both match
/// here, so the overlap is planted in both: `doubled` is made the amount itself, and 5 yen then
/// matches both rows.
#[test]
fn numpy_stops_where_two_rows_match_as_python_does() {
    if !numpy_here() {
        return;
    }
    for (rule, lang, doubled, said, row) in [
        ("tests/pages/half_bound.rule", "en", "doubled", "table decide: row 1 and row 2 matched at the same time", "row"),
        ("tests/mutants/m_w114.rule", "ja", "倍額", "表 判定: 行1 と 行2 が同時に当てはまりました", "行"),
    ] {
        let c = Case { rule, lang, alias: "half_bound", broken: "", said: "" };
        let (_t, out) = generate(&c);
        // the plan: the definition of `doubled` read as the amount itself
        let plan_path = out.join("numpy/half_bound.json");
        let plan = std::fs::read_to_string(&plan_path).unwrap();
        let sum = format!(r#"{{"op":"define","name":"{doubled}","expr":{{"k":"bin","op":"+","l":{{"k":"name","n":"paid"}},"r":{{"k":"name","n":"paid"}}}}}}"#);
        let sum = if lang == "ja" { sum.replace("\"n\":\"paid\"", "\"n\":\"支払額\"") } else { sum };
        assert!(plan.contains(&sum), "{rule}: the plan does not define {doubled} as written:\n{plan}");
        let one = if lang == "ja" { r#""expr":{"k":"name","n":"支払額"}"# } else { r#""expr":{"k":"name","n":"paid"}"# };
        let planted = sum.replace(&sum[sum.find("\"expr\"").unwrap()..sum.len() - 1], one);
        std::fs::write(&plan_path, plan.replace(&sum, &planted)).unwrap();
        let input = if lang == "ja" { r#"[{"支払額":[0,5]}]"# } else { r#"[{"paid":[0,5]}]"# };
        let got = try_numpy(&out, "half_bound.json", input);
        assert_eq!(got, [format!("RuleContradictionError | {said} ({row} 1)")], "{rule}: {got:?}");
        // the generated Python, with the same definition planted
        let py_path = out.join("python/half_bound.py");
        let py = std::fs::read_to_string(&py_path).unwrap();
        // the generated Python names a value by its alias, in either language
        let (from, to) = ("doubled = paid + paid", "doubled = paid");
        assert!(py.contains(from), "{rule}: the Python does not define {doubled} as written");
        std::fs::write(&py_path, py.replacen(from, to, 1)).unwrap();
        let o = Command::new("python3")
            .current_dir(out.join("python"))
            .args(["-B", "-c", "import half_bound as m\ntry:\n    m.half_bound(5)\n    print('answered')\nexcept m.RuleContradictionError as e:\n    print('RuleContradictionError |', e)\n"])
            .output()
            .unwrap();
        let printed = String::from_utf8_lossy(&o.stdout).trim().to_string();
        assert_eq!(printed, format!("RuleContradictionError | {said}"), "{rule}: {}", String::from_utf8_lossy(&o.stderr));
    }
}
