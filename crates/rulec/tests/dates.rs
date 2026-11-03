//! Every day of the years 0001 to 9999 is a date at every door but the Ruby runner (§15.279).
//!
//! A date on the wire is `YYYY-MM-DD`, a day of the Gregorian calendar taken back before 1582
//! (§10.2), and the reference evaluator takes every day of the years 0001 to 9999 (`door::civil`).
//! Not every reader of the generated code did. The TypeScript and JavaScript runners and MCP
//! servers refused the years 0001 to 0099 as not dates, because `Date.UTC` reads such a year as one
//! of the 1900s. The Go runner read every day before 1677-09-22 as that day, and every day after
//! 2262-04-11 as that day, because a `time.Duration` holds no more. The page of `rulec doc` read
//! the years 0001 to 0099 as the 1900s and answered for them, and took 2026-02-30 for 2026-03-02.
//! None of it was seen, because the vectors are made of the rule's own days and the days beside
//! them. The Ruby runner still reads the days before 1582-10-15 in the Julian calendar, so it is
//! left out here.
//!
//! The materials are the two rules of the tree whose dates have no range, so that every day is in
//! it: `tests/corpus/coupon_order.rule` and its Japanese version `tests/corpus/適用順序.rule`. Both
//! coupons are of the same kind, so the answer is which due date comes first.

use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, ready, side_by_side, skip};
use rulec::eval::Val;
use rulec::i18n::Lang;
use rulec::json::{Json, quote};
use std::collections::{BTreeMap, HashMap};
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

/// Whether a command answers. swiftc leaves an empty directory in its TMPDIR almost every time it
/// answers `--version`, so the probe gets a TMPDIR of its own, which goes when this returns.
fn have(cmd: &str, args: &[&str]) -> bool {
    let tmp = TempDir::new("dates-probe");
    Command::new(cmd).args(args).env("TMPDIR", tmp.path()).output().map(|o| o.status.success()).unwrap_or(false)
}

/// A rule with two coupons, in the language its messages are generated in: its alias, its four
/// inputs in the order declared (the two kinds, then the two due dates), and the kind both coupons
/// are of.
struct Case {
    rule: &'static str,
    lang: &'static str,
    alias: &'static str,
    inputs: [&'static str; 4],
    kind: &'static str,
}

const CASES: [Case; 2] = [
    Case { rule: "tests/corpus/coupon_order.rule", lang: "en", alias: "coupon_order", inputs: ["a_kind", "b_kind", "a_due", "b_due"], kind: "fixed" },
    Case { rule: "tests/corpus/適用順序.rule", lang: "ja", alias: "apply_order", inputs: ["A種別", "B種別", "A期限", "B期限"], kind: "額引き" },
];

/// The days tried, in order: the first and the last of the years every door takes, a leap day of
/// the first years, days of the years 0001 to 0099 and the first day after them, days before the
/// Gregorian calendar started and its first day, and the two days at each end of what a signed
/// 64-bit count of nanoseconds from 1970 reaches (1677-09-21 and 2262-04-11 hold the ends).
const FAR: [&str; 17] = [
    "0001-01-01",
    "0004-02-29",
    "0050-01-01",
    "0099-12-31",
    "0100-01-01",
    "1500-02-28",
    "1500-03-01",
    "1582-10-04",
    "1582-10-10",
    "1582-10-15",
    "1600-01-01",
    "1677-09-21",
    "1677-09-22",
    "1949-12-31",
    "2262-04-11",
    "2262-04-12",
    "9999-12-31",
];

/// Strings that are not days, which every door refuses: leap days of the Julian calendar that the
/// Gregorian does not have, two of them before 1582 (1500-02-29, 0100-02-29) and one after
/// (1900-02-29), and a day no month has (2026-02-30).
const NOT_DAYS: [&str; 4] = ["1500-02-29", "0100-02-29", "1900-02-29", "2026-02-30"];

/// The pairs of due dates tried: each day against itself, against the next one both ways, and the
/// first against the last both ways.
fn pairs() -> Vec<(&'static str, &'static str)> {
    let mut p: Vec<(&str, &str)> = FAR.iter().map(|d| (*d, *d)).collect();
    for w in FAR.windows(2) {
        p.push((w[0], w[1]));
        p.push((w[1], w[0]));
    }
    p.push((FAR[0], FAR[FAR.len() - 1]));
    p.push((FAR[FAR.len() - 1], FAR[0]));
    p
}

fn lang_of(c: &Case) -> Lang {
    if c.lang == "ja" { Lang::Ja } else { Lang::En }
}

/// The input object of a pair of due dates, as a vector writes it.
fn in_object(c: &Case, a: &str, b: &str) -> String {
    let [ka, kb, da, db] = c.inputs;
    format!("{{{}:{},{}:{},{}:{},{}:{}}}", quote(ka), quote(c.kind), quote(kb), quote(c.kind), quote(da), quote(a), quote(db), quote(b))
}

/// What the reference evaluator says of a pair of due dates: the vector and its expected record,
/// as `rulec gen --extra vectors` writes them; or, for a pair it refuses, the line of the file of
/// refused inputs.
enum Said {
    Answer { vector: String, expected: String },
    Refused(String),
}

fn reference(c: &Case, a: &str, b: &str) -> Said {
    let src = std::fs::read_to_string(root().join(c.rule)).unwrap();
    let (f, cc) = rulec::prepare(&src, c.rule).unwrap_or_else(|d| panic!("{}: {d:?}", c.rule));
    rulec::i18n::with(lang_of(c), || {
        let obj = in_object(c, a, b);
        let j = rulec::json::parse(&obj).unwrap();
        if let Some(r) = rulec::door::refusal(&f, &cc, &j) {
            let why = "a string that is not a day (§15.279)";
            return Said::Refused(format!("{{\"in\":{obj},\"refused\":{},\"error\":{},\"why\":{}}}", quote(r.kind), quote(&r.error), quote(why)));
        }
        let day = |s: &str| {
            let (y, m, d) = rulec::door::civil(s).unwrap_or_else(|| panic!("{s} is not a day"));
            Val::Date(y, m, d)
        };
        let [ka, kb, da, db] = c.inputs;
        let input: BTreeMap<String, Val> = [
            (ka.to_string(), Val::Enum(c.kind.to_string())),
            (kb.to_string(), Val::Enum(c.kind.to_string())),
            (da.to_string(), day(a)),
            (db.to_string(), day(b)),
        ]
        .into_iter()
        .collect();
        let (outputs, trace, fired, _) = rulec::eval::run_all_traced(&f, &cc, input.clone().into_iter().collect::<HashMap<_, _>>());
        let v = rulec::vectors::Vector { input, outputs, trace, fired, why: "a day far from the rule's own (§15.279)".into() };
        Said::Answer { vector: rulec::vectors::to_json(&f, &cc, &v), expected: rulec::vectors::expected_json(&f, &cc, &v) }
    })
}

/// The targets held to the far days, each with its runner, and the MCP servers and the Connect
/// service beside them: every target but Ruby.
const GEN: [&str; 32] = [
    "--target", "python", "--target", "numpy", "--target", "typescript", "--target", "javascript", "--target", "rust",
    "--target", "php", "--target", "go", "--target", "swift", "--target", "java", "--target", "sql", "--target", "wasm",
    "--target", "proto", "--extra", "runner", "--extra", "mcp", "--extra", "connect", "--extra", "vectors",
];

/// The runners that read a far day differently before (§15.279): each has to have run.
const FIXED: [&str; 5] = ["typescript/runner", "typescript/mcp", "javascript/runner", "javascript/mcp", "go/runner"];

/// Lines added to the end of a file.
fn append(p: &Path, lines: &[String]) {
    let mut body = std::fs::read_to_string(p).unwrap_or_default();
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
    for l in lines {
        body.push_str(l);
        body.push('\n');
    }
    std::fs::write(p, body).unwrap();
}

/// `rulec test` over a generated directory, as JSON, with a SKIP line for whatever it did not run.
fn rulec_test(out: &Path) -> (i32, Json) {
    let (code, said, e) = rulec("en", Some(&tmpdir_in(out)), &["test", &out.to_string_lossy(), "--format", "json"]);
    let j = rulec::json::parse(said.lines().next().unwrap_or_else(|| panic!("no result\n{e}"))).unwrap_or_else(|x| panic!("{x}: {said}"));
    for why in j.get("skipped").and_then(|v| v.as_arr()).unwrap_or_default() {
        skip(&format!("rulec test: {}", why.as_str().unwrap_or_default()));
    }
    (code, j)
}

/// Every way the rule is reached answers each pair of far days as the reference evaluator does,
/// the record giving back the days as they were written, and refuses each string that is not a
/// day with the evaluator's sentence: the runners of every language but Ruby, the MCP servers, the
/// Connect service, the WASI module and the function on PostgreSQL. The English rule and the
/// Japanese version run side by side.
#[test]
fn every_door_reads_every_day_of_the_years_0001_to_9999() {
    if !ready(Need::Python, || have("python3", &["--version"]), "python3 is not here") {
        return;
    }
    side_by_side(&CASES, |c| {
        let t = TempDir::new("dates");
        let out = t.path().join("out");
        let o = out.to_string_lossy().into_owned();
        let mut args = vec!["gen", c.rule, "--out", o.as_str()];
        args.extend(GEN);
        let (code, said, e) = rulec(c.lang, None, &args);
        assert_eq!(code, 0, "{}: {said}{e}", c.rule);

        let (mut vectors, mut expected, mut refused) = (Vec::new(), Vec::new(), Vec::new());
        for (a, b) in pairs() {
            match reference(c, a, b) {
                Said::Answer { vector, expected: x } => {
                    vectors.push(vector);
                    expected.push(x);
                }
                Said::Refused(l) => panic!("{}: the reference evaluator refuses {a} and {b}: {l}", c.rule),
            }
        }
        for d in NOT_DAYS {
            match reference(c, d, "2026-01-01") {
                Said::Refused(l) => refused.push(l),
                Said::Answer { .. } => panic!("{}: the reference evaluator answers {d}", c.rule),
            }
        }
        let v = out.join("vectors");
        append(&v.join(format!("{}.jsonl", c.alias)), &vectors);
        append(&v.join(format!("{}.expected.jsonl", c.alias)), &expected);
        append(&v.join(format!("{}.refused.jsonl", c.alias)), &refused);
        let count = |name: &str| std::fs::read_to_string(v.join(name)).unwrap().lines().filter(|l| !l.trim().is_empty()).count();
        let (n, r) = (count(&format!("{}.jsonl", c.alias)), count(&format!("{}.refused.jsonl", c.alias)));

        let (code, j) = rulec_test(&out);
        assert_eq!(code, 0, "{}: {j:?}", c.rule);
        let mut ran = Vec::new();
        for res in j.get("results").and_then(|v| v.as_arr()).unwrap_or_default() {
            if res.get("rule").and_then(|v| v.as_str()) != Some(c.alias) || res.get("ran").and_then(|v| v.as_bool()) != Some(true) {
                continue;
            }
            assert_eq!(res.get("ok").and_then(|v| v.as_bool()), Some(true), "{}: {res:?}", c.rule);
            assert_eq!(res.get("vectors").and_then(|v| v.as_int()), Some(n as i128), "{}: not every vector was run: {res:?}", c.rule);
            assert_eq!(res.get("refused").and_then(|v| v.as_int()), Some(r as i128), "{}: not every input was put: {res:?}", c.rule);
            ran.push(format!("{}/{}", res.get("lang").and_then(|v| v.as_str()).unwrap_or_default(), res.get("via").and_then(|v| v.as_str()).unwrap_or_default()));
        }
        for way in FIXED {
            assert!(ran.iter().any(|r| r == way), "{}: {way} did not run: {ran:?}", c.rule);
        }
        println!("dates [{}]: {} vectors with {} far, {} refused with {} not days, through {}", c.lang, n, vectors.len(), r, refused.len(), ran.join(", "));
    });
}

/// A stand-in for the browser, as much of it as the panel of the page touches.
const DOM: &str = r##"
class El {
  constructor(tag) {
    this.tagName = tag.toUpperCase();
    this.children = [];
    this.listeners = {};
    this.classList = { add() {}, remove() {}, toggle() {}, contains: () => false };
    this.style = {};
    this.value = "";
    this.checked = false;
    this._text = "";
    this.name = "";
  }
  get textContent() { return this._text; }
  set textContent(v) { this._text = String(v); }
  appendChild(c) { this.children.push(c); c.parentNode = this; return c; }
  remove() { const p = this.parentNode; if (p) p.children = p.children.filter((x) => x !== this); }
  addEventListener(k, f) { (this.listeners[k] ||= []).push(f); }
  click() { for (const f of this.listeners.click || []) f(); }
  querySelectorAll() { return []; }
  get elements() {
    const o = {};
    const walk = (n) => { if (n.name) o[n.name] = n; n.children.forEach(walk); };
    this.children.forEach(walk);
    return o;
  }
}
const ids = {};
for (const id of ["try-form", "try-rows", "try-buttons", "try-result", "try-record"]) {
  ids["#" + id] = new El(id === "try-form" ? "form" : "div");
}
globalThis.document = { createElement: (t) => new El(t), querySelector: (s) => ids[s] ?? null, querySelectorAll: () => [] };
globalThis.CSS = { escape: (s) => s };
globalThis.history = { replaceState() {} };
globalThis.location = { hash: "", search: "" };
globalThis.window = { addEventListener() {} };
globalThis.window.parent = globalThis.window;
export { ids };
"##;

/// Driving the panel: for each pair of due dates, both kinds set to the one of the case, the dates
/// typed into their fields, and Compute; what the result and the record say, one object a pair.
const DRIVE: &str = r##"
const fields = ids["#try-form"].elements;
const compute = ids["#try-buttons"].children[0];
const said = [];
const [ka, kb, da, db] = RULE.inputs.map((inp) => inp.alias);
for (const [a, b] of JSON.parse(process.env.PAGE_PAIRS)) {
  fields[ka].value = process.env.PAGE_KIND;
  fields[kb].value = process.env.PAGE_KIND;
  fields[da].value = a;
  fields[db].value = b;
  compute.click();
  said.push({ result: ids["#try-result"].textContent, record: ids["#try-record"].textContent });
}
console.log(JSON.stringify(said));
"##;

/// The page's panel reads each far day as the day written, and gives the answer and the record the
/// reference evaluator gives; a string that is not a day, and an empty field, it refuses with the
/// runner's sentence, where the module's own refusals go, and keeps no record. Before, it answered
/// 0050-01-01 against 1949-12-31 as if the first were 1950-01-01, took 2026-02-30 for 2026-03-02,
/// and stopped with a `RangeError` on an empty field.
#[test]
fn the_page_reads_every_day_and_refuses_a_day_the_calendar_does_not_have() {
    if !ready(Need::Node, || have("node", &["--version"]), "node is not here") {
        return;
    }
    side_by_side(&CASES, |c| {
        let (code, html, e) = rulec(c.lang, None, &["doc", c.rule, "--format", "html"]);
        assert_eq!(code, 0, "{}: {e}", c.rule);
        let head = html.find("<script type=\"module\">").expect("the page has no script") + "<script type=\"module\">".len();
        let body = &html[head..head + html[head..].find("</script>").expect("the script is not closed")];
        let t = TempDir::new("dates-page");
        std::fs::write(t.path().join("dom.mjs"), DOM).unwrap();
        let p = t.path().join("page.mjs");
        std::fs::write(&p, format!("import {{ ids }} from \"./dom.mjs\";\n{body}\n{DRIVE}")).unwrap();

        let mut tried: Vec<(&str, &str)> = pairs();
        tried.extend(NOT_DAYS.iter().map(|d| (*d, "2026-01-01")));
        tried.push(("", "2026-01-01"));
        let arg = format!("[{}]", tried.iter().map(|(a, b)| format!("[{},{}]", quote(a), quote(b))).collect::<Vec<_>>().join(","));
        let o = Command::new("node").arg(&p).env("PAGE_PAIRS", &arg).env("PAGE_KIND", c.kind).output().expect("cannot start node");
        let out = String::from_utf8_lossy(&o.stdout).into_owned();
        assert!(o.status.success(), "{}: {out}{}", c.rule, String::from_utf8_lossy(&o.stderr));
        let said = rulec::json::parse(out.trim()).unwrap_or_else(|_| panic!("{}: {out}", c.rule));
        let said = said.as_arr().unwrap_or_default();
        assert_eq!(said.len(), tried.len(), "{}: {out}", c.rule);

        let src = std::fs::read_to_string(root().join(c.rule)).unwrap();
        let (f, _) = rulec::prepare(&src, c.rule).unwrap();
        let output = f.outputs[0].name.text.clone();
        let not_date = rulec::i18n::with(lang_of(c), || rulec::door::not_date(c.inputs[2]));
        for ((a, b), s) in tried.iter().zip(said.iter()) {
            let result = s.get("result").and_then(|v| v.as_str()).unwrap_or_default();
            let record = s.get("record").and_then(|v| v.as_str()).unwrap_or_default();
            if a.is_empty() || NOT_DAYS.contains(a) {
                assert_eq!(result, format!("{not_date}: {a}"), "{}: {a} against {b}", c.rule);
                assert_eq!(record, "", "{}: {a} against {b}", c.rule);
                continue;
            }
            let Said::Answer { expected, .. } = reference(c, a, b) else { panic!("{}: the reference evaluator refuses {a} and {b}", c.rule) };
            let x = rulec::json::parse(&expected).unwrap();
            let value = x.get("observed").and_then(|o| o.get(&output)).and_then(|v| v.as_str()).unwrap_or_default().to_string();
            assert_eq!(result, format!("{output} = {value}"), "{}: {a} against {b}", c.rule);
            assert_eq!(record, expected, "{}: {a} against {b}", c.rule);
        }
        println!("dates [{}]: the page answered {} pairs and refused {}", c.lang, tried.len() - NOT_DAYS.len() - 1, NOT_DAYS.len() + 1);
    });
}
