//! `fed by` (DESIGN 7.16, X17): where a place says hand-written code takes its value from, held by
//! `ritsu check` to what can come from there. `tests/projects/fed_by` is a project joined from end
//! to end (an OpenAPI document to a flow, a book to the flow and to a rule, a rule to a rule, a rule
//! to itself, a JSON Schema to a gate, a dates file and a module of kaname), and it holds. Each test
//! breaks a copy of it or lays out a small one:
//!
//! - The six ways of breaking a border that nothing saw: the flow's amount narrowed below its
//!   contract, a rule's band raised above what the next rule takes, the next rule narrowed, a rule
//!   carrying over more than it takes back, a book with no upper limit, and a JSON Schema's name,
//!   type and range changed (each E207 or E208).
//! - The three answers (P5) over each kind of source and of value: units, steps and tax, the values
//!   of an enum, days, a contract's open end, an end a rule does not count, `required` and `null`, a
//!   `.proto`'s field, a workflow's and a module's places.
//! - What stops at the language that holds the place, for what names no value and for a place with
//!   no type to hold it to; a file or a thing that is not there; a place fed by its own output, and
//!   two rules that feed each other.
//! - What does not move: everything generated, for every target of every language; a rule's
//!   `meaning:`; what yuen takes the hash of. `rulec fmt` keeps the form, and the pages and `api` of
//!   each language say it, with what it does not check.
//! - sakai holds a `fed by` across a boundary to what any reference needs, and `ritsu mod report`
//!   gives a new E207 to the dependency that was raised.

use ritsu_base::modules::store::{self, Store};
use ritsu_ports::{Items, References};
use ritsu_testkit::TempDir;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).args(args);
    for v in ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "SEKISHO_LANG", "KANAME_LANG"] {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The files of a project, by their paths from its root.
#[derive(Clone)]
struct Files(BTreeMap<String, String>);

impl Files {
    fn none() -> Files {
        Files(BTreeMap::new())
    }

    /// `tests/projects/fed_by`.
    fn project() -> Files {
        fn walk(dir: &Path, from: &Path, out: &mut BTreeMap<String, String>) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    walk(&p, from, out);
                } else {
                    out.insert(p.strip_prefix(from).unwrap().to_str().unwrap().to_string(), std::fs::read_to_string(&p).unwrap());
                }
            }
        }
        let root = here().join("tests/projects/fed_by");
        let mut out = BTreeMap::new();
        walk(&root, &root, &mut out);
        Files(out)
    }

    fn with(mut self, file: &str, body: &str) -> Files {
        self.0.insert(file.to_string(), body.to_string());
        self
    }

    /// The first `from` in the file becomes `to`.
    fn edit(mut self, file: &str, from: &str, to: &str) -> Files {
        let body = self.0.get_mut(file).unwrap_or_else(|| panic!("no file {file}"));
        assert!(body.contains(from), "{file} has no {from:?}");
        *body = body.replacen(from, to, 1);
        self
    }

    fn lay_out(&self, tag: &str) -> TempDir {
        let t = TempDir::new(tag);
        for (name, body) in &self.0 {
            let p = t.path().join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        }
        t
    }

    /// Every file with its `fed by` taken off, as `ritsu_base::fed` takes it off.
    fn without_the_form(&self) -> Files {
        Files(self.0.iter().map(|(n, b)| (n.clone(), ritsu_base::fed::without(b).into_owned())).collect())
    }

    /// `ritsu check .`: the exit, the English text, and the JSON.
    fn check(&self, tag: &str) -> (i32, String, Value) {
        let t = self.lay_out(tag);
        let (code, text, _) = ritsu_in(t.path(), &["check", "."]);
        let (_, json, err) = ritsu_in(t.path(), &["check", ".", "--format", "json"]);
        let v: Value = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{tag}: {e}: {json}{err}"));
        (code, text, v)
    }
}

/// The records of X17.
fn records(v: &Value) -> Vec<&Value> {
    v["borders"]["records"].as_array().unwrap().iter().filter(|r| r["check"] == "fed_by").collect()
}

/// One record that does not hold, as a test states it.
#[derive(Debug, PartialEq)]
struct Against {
    file: String,
    about: String,
    code: String,
    value: Option<String>,
}

fn against(file: &str, about: &str, code: &str, value: Option<&str>) -> Against {
    Against { file: file.to_string(), about: about.to_string(), code: code.to_string(), value: value.map(str::to_string) }
}

/// The records of X17 that do not hold, in their order.
fn not_held(v: &Value) -> Vec<Against> {
    records(v)
        .into_iter()
        .filter(|r| r["result"] != "held")
        .map(|r| Against {
            file: r["at"]["file"].as_str().unwrap().to_string(),
            about: r["about"].as_str().unwrap().to_string(),
            code: r["code"].as_str().unwrap().to_string(),
            value: r["example"]["value"].as_str().map(str::to_string),
        })
        .collect()
}

/// The record of a place.
fn record<'a>(v: &'a Value, about: &str) -> &'a Value {
    records(v).into_iter().find(|r| r["about"] == about).unwrap_or_else(|| panic!("no record of {about}: {}", v["borders"]))
}

/// The project as it is holds: the check passes, and it is the records that say so.
fn holds(files: &Files, tag: &str, count: usize) -> Value {
    let (code, text, v) = files.check(tag);
    assert_eq!(code, 0, "{tag}:\n{text}");
    assert_eq!(not_held(&v), vec![], "{tag}:\n{text}");
    assert_eq!(records(&v).len(), count, "{tag}: {}", v["borders"]);
    v
}

/// The project stops at these places: what X17 says, and every one of those codes among what
/// `ritsu check` says.
fn stops(files: &Files, tag: &str, want: &[Against]) -> (String, Value) {
    let (code, text, v) = files.check(tag);
    assert_eq!(not_held(&v).iter().collect::<Vec<_>>(), want.iter().collect::<Vec<_>>(), "{tag}:\n{text}");
    let error = want.iter().any(|w| w.code.starts_with('E'));
    assert_eq!(code, i32::from(error), "{tag}:\n{text}");
    for w in want {
        assert!(text.contains(&format!("[ritsu {}]", w.code)) || text.contains(&format!("[{}]", w.code)), "{tag}: no {}:\n{text}", w.code);
    }
    (text, v)
}

fn golden(name: &str, got: &str, failures: &mut Vec<String>) {
    if let Err(e) = ritsu_testkit::golden::check(&here().join("tests/golden/fed_by").join(name), got) {
        failures.push(e);
    }
}

// ── The project ────────────────────────────────────────────────────────────────────────────────

/// An OpenAPI document to a flow, a book to the flow and to a rule, a rule to a rule, a rule to
/// itself, a JSON Schema to a gate, a dates file and a module: the summary, and a record of each.
#[test]
fn the_project_holds_from_end_to_end() {
    let (code, text, v) = Files::project().check("fed-project");
    assert_eq!(code, 0, "{text}");
    assert_eq!(
        text.trim_end().lines().last(),
        Some("ritsu check: 8 files (rulec 3, koyomi 1, chobo 1, kaname 1, dandori 1, sekisho 1): all pass; borders between the languages: 10 checked, 0 undecided")
    );
    let b = &v["borders"];
    assert_eq!((b["held"].as_u64(), b["failed"].as_u64(), b["undecided"].as_u64()), (Some(10), Some(0), Some(0)), "{b}");
    assert_eq!(records(&v).len(), 10);
    let r = record(&v, "input amount");
    assert_eq!(r["at"]["tool"], "dandori");
    assert_eq!(r["at"]["file"], "refund.flow");
    assert_eq!(r["at"]["line"], 5);
    assert_eq!(r["to"]["tool"], "openapi");
    assert_eq!(r["to"]["file"], "api/refunds.json");
    assert_eq!(r["source"], "openapi \"api/refunds.json\" schema RefundCreate property amount");
    assert_eq!(r["takes"], "money[USDc] >=1 <=1000000");
    assert_eq!(r["gives"], "integer >=1 <=1000000");
    assert_eq!(r["result"], "held");
    assert_eq!(r["code"], Value::Null);
    let mut failures = Vec::new();
    golden("records.json", &(serde_json::to_string_pretty(b).unwrap() + "\n"), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ── The six ways of breaking it ────────────────────────────────────────────────────────────────

/// The flow takes less than its contract lets through.
#[test]
fn a_flow_narrowed_below_its_contract() {
    let broken = Files::project().edit("refund.flow", "<=1000000USDc  fed by openapi", "<=100000USDc  fed by openapi");
    let (text, _) = stops(&broken, "fed-1", &[against("refund.flow", "input amount", "E207", Some("100001"))]);
    assert!(text.contains("`input amount` takes money[USDc] >=1 <=100000, and its source openapi \"api/refunds.json\" schema RefundCreate property amount can be 100001"), "{text}");
}

/// A rule's output is raised above what the next rule takes: the value, and the input that gives it.
#[test]
fn a_rule_raised_above_what_the_next_takes() {
    let broken = Files::project().edit("refund_limit.rule", "| >1000USDc  | 2                |", "| >1000USDc  | 3                |");
    let (text, v) = stops(&broken, "fed-2", &[against("review.rule", "input band", "E207", Some("3"))]);
    assert_eq!(record(&v, "input band")["example"]["at"], "left = 1001");
    assert!(text.contains("rulec \"refund_limit.rule\" output band comes to 3 at left = 1001."), "{text}");
    assert!(text.contains("Which of the two is a person's decision"), "{text}");
}

/// The rule that takes it is narrowed.
#[test]
fn the_rule_that_takes_it_narrowed() {
    let broken = Files::project().edit("review.rule", "range >=0 <=2  fed by", "range >=0 <=1  fed by");
    stops(&broken, "fed-3", &[against("review.rule", "input band", "E207", Some("2"))]);
}

/// A rule's output put back into its own input, and a row that carries over more than it takes.
#[test]
fn a_rule_that_carries_over_more_than_it_takes_back() {
    let broken = Files::project().edit("research.rule", "| >=100 | true              | 0                      |", "| >=100 | true              | total                  |");
    let (_, v) = stops(&broken, "fed-4", &[against("research.rule", "input progress", "E207", Some("300"))]);
    assert_eq!(record(&v, "input progress")["example"]["at"], "learning = 100, progress = 200");
}

/// A book takes what its upper limit does not rule out: up to chobo's limit, though no write has
/// come near it yet.
#[test]
fn a_book_with_no_upper_limit() {
    let broken = Files::project().edit("refunds.book", "  at most 100000 refused as more_than_paid\n", "");
    let (text, _) = stops(
        &broken,
        "fed-5",
        &[
            against("refund.flow", "record Balance field left", "E207", Some("10000001")),
            against("refund_limit.rule", "input left", "E207", Some("10000001")),
        ],
    );
    assert!(text.contains("`record Balance field left` takes money[USDc] >=0 <=10000000, and its source chobo \"refunds.book\" account remaining can be 10000001"), "{text}");
    // the same with no lower limit: the balance can go below what the place takes
    let broken = Files::project().edit("refunds.book", "  at least 0 refused as nothing_left\n", "");
    stops(
        &broken,
        "fed-5-low",
        &[
            against("refund.flow", "record Balance field left", "E207", Some("-1")),
            against("refund_limit.rule", "input left", "E207", Some("-1")),
        ],
    );
}

/// A JSON Schema's name, type and range, each read by a gate, a dates file and a module.
#[test]
fn a_json_schema_changed() {
    let schema = "schemas/order.schema.json";
    // a definition with another name: every naming that goes through it, its pointer too
    let broken = Files::project().edit(schema, "\"Order\": {", "\"Purchase\": {");
    stops(
        &broken,
        "fed-6-name",
        &[
            against("bill.kn", "record Line field items", "E208", None),
            against("bill.kn", "function total parameter order", "E208", None),
            against("desk.gate", "resource Order attribute amount", "E208", None),
            against("due.cal", "input issued_on", "E208", None),
        ],
    );
    // a property with another name
    let broken = Files::project().edit(schema, "\"items\"]", "\"lines\"]").edit(schema, "\"items\": {", "\"lines\": {");
    let (text, _) = stops(&broken, "fed-6-property", &[against("bill.kn", "record Line field items", "E208", None)]);
    assert!(text.contains("The source jsonschema \"schemas/order.schema.json\" def Order property items of `record Line field items` is not there"), "{text}");
    // another type
    let broken = Files::project().edit(schema, "\"amount\": { \"type\": \"integer\", \"minimum\": 1, \"maximum\": 1000000 }", "\"amount\": { \"type\": \"string\" }");
    let (text, _) = stops(
        &broken,
        "fed-6-type",
        &[
            against("bill.kn", "function total parameter order", "E207", None),
            against("desk.gate", "resource Order attribute amount", "E207", None),
        ],
    );
    assert!(text.contains("the types differ"), "{text}");
    // a wider range
    let broken = Files::project().edit(schema, "\"maximum\": 1000000 }", "\"maximum\": 2000000 }");
    stops(
        &broken,
        "fed-6-range",
        &[
            against("bill.kn", "function total parameter order", "E207", Some("1000001")),
            against("desk.gate", "resource Order attribute amount", "E207", Some("1000001")),
        ],
    );
    // a day outside what the dates file takes
    let broken = Files::project().edit(schema, "\"2026-06-01\"", "\"2027-06-01\"");
    stops(&broken, "fed-6-day", &[against("due.cal", "input issued_on", "E207", Some("\"2027-06-01\""))]);
}

/// The same project broken four ways, in both languages, word by word.
#[test]
fn what_it_says_in_both_languages() {
    let mut failures = Vec::new();
    let cases: [(&str, Files); 4] = [
        ("rule", Files::project().edit("refund_limit.rule", "| >1000USDc  | 2                |", "| >1000USDc  | 3                |")),
        ("book", Files::project().edit("refunds.book", "  at most 100000 refused as more_than_paid\n", "")),
        ("not-there", Files::project().edit("schemas/order.schema.json", "\"Order\": {", "\"Purchase\": {")),
        ("undecided", Files::project().edit("schemas/order.schema.json", "\"type\": \"string\", \"enum\": [\"2026-04-01\", \"2026-05-01\", \"2026-06-01\"]", "\"type\": \"string\"")),
    ];
    for (name, files) in cases {
        let t = files.lay_out(&format!("fed-says-{name}"));
        for lang in ["en", "ja"] {
            let (_, out, _) = ritsu_in(t.path(), &["check", ".", "--lang", lang]);
            golden(&format!("{name}.{lang}.txt"), &out, &mut failures);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The ledger's reproductions show what their entries say: the rules pass rulec, so what comes out
/// is the border's answer and no other language's error.
#[test]
fn the_ledgers_reproductions() {
    let l = ritsu_cross::codes::ledger();
    for (code, exit, why) in [("E207", 1, None), ("W207", 0, Some("none of the rule's vectors")), ("E208", 1, Some("has no output `wheat`"))] {
        let ritsu_base::ledger::Repro::Dir { files, .. } = &l.find(code).unwrap().repro else {
            panic!("{code} has no files");
        };
        let laid = files.iter().fold(Files::none(), |f, (n, b)| f.with(n, b));
        let (got, text, v) = laid.check(&format!("fed-ledger-{code}"));
        assert_eq!(got, exit, "{code}:\n{text}");
        let lines: Vec<&str> = text.lines().filter(|l| l.starts_with("error[") || l.starts_with("warning[")).collect();
        assert_eq!(lines.len(), 1, "{code}:\n{text}");
        assert!(lines[0].contains(&format!("[ritsu {code}]")), "{code}:\n{text}");
        if let Some(why) = why {
            assert!(records(&v)[0]["why"].as_str().unwrap().contains(why), "{code}: {}", v["borders"]);
        }
    }
}

// ── The three answers, by the kind of value ────────────────────────────────────────────────────

const FEEDER: &str = "rule feeder v1\n\ninputs\n  x : number  range >=0 <=3\n\noutputs\n  y : number  round down(1)\n\ntable t\npolicy unique\n| x   | -> y : number |\n| 0   | 1             |\n| >=1 | 2             |\n";

/// A rule of one input over `feeder`'s output.
fn taker(input: &str) -> String {
    format!("rule taker v1\n\ninputs\n  {input}\n\noutputs\n  z : bool\n\ntable t\npolicy unique\n| v | -> z : bool |\n| - | true       |\n")
}

/// Undecided: rulec counts an end above what the place takes (it moves each input over its own
/// range), and none of the rule's vectors comes to such a value. Exit 0, a reason, a note of what
/// would decide it, and a warning in the summary.
#[test]
fn a_rules_end_no_vector_reaches_is_undecided() {
    let carry = "rule carried_over v1\n\ninputs\n  total : number  range >=0 <=300\n  cost  : number  range >=1 <=200\n\noutputs\n  ahead : number  round down(1)\n\ndefine left : number = total - cost\n\ntable finish\npolicy unique\n| left | -> ahead : number |\n| >=0  | 0                 |\n| <0   | total             |\n";
    let research = "rule research v1\n\ninputs\n  progress : number  range >=0 <=250  fed by rulec \"carried_over.rule\" output ahead\n\noutputs\n  far : bool\n\ntable far_of\npolicy unique\n| progress | -> far : bool |\n| >=100    | true          |\n| <100     | false         |\n";
    let files = Files::none().with("carried_over.rule", carry).with("research.rule", research);
    let (text, v) = stops(&files, "fed-undecided", &[against("research.rule", "input progress", "W207", None)]);
    let r = record(&v, "input progress");
    assert_eq!(r["gives"], "number >=0 <=300");
    assert!(r["why"].as_str().unwrap().contains("rulec counts the output as >=0 <=300, and none of the rule's vectors"), "{r}");
    assert!(text.contains("warning[ritsu W207]"), "{text}");
    assert!(text.contains("Whether `input progress` takes what can come from rulec \"carried_over.rule\" output ahead cannot be decided"), "{text}");
    assert!(text.contains("Write the rule so that rulec counts the output's ends"), "{text}");
    assert_eq!(v["borders"]["undecided"], 1);
    // the place that takes what rulec counts holds
    holds(&Files::none().with("carried_over.rule", carry).with("research.rule", &research.replace("<=250", "<=300")), "fed-undecided-widened", 1);
}

/// A field of an element, and an input read through a path of a contract. The path says where the
/// generated code reads the value out of the caller's object, and `fed by` where the hand-written
/// code that fills the object takes it from, so they stand together on one line. Both are held to
/// what can come from the source, and both reach the record, the page and the description.
#[test]
fn a_field_of_an_element_and_an_input_with_a_path() {
    let recruit = "rule recruit v1\n\ninputs\n  barracks : bool\n\noutputs\n  veterancy : number  round down(1)\n\ntable t\npolicy unique\n| barracks | -> veterancy : number |\n| true     | 1                     |\n| false    | 0                     |\n";
    let schema = r##"{"$schema": "https://json-schema.org/draft/2020-12/schema", "$defs": {"Side": {"type": "object", "required": ["morale", "units"], "properties": {"morale": {"type": "integer", "minimum": 0, "maximum": 2}, "units": {"type": "array", "maxItems": 9, "items": {"type": "object", "required": ["veterancy"], "properties": {"veterancy": {"type": "integer", "minimum": 0, "maximum": 2}}}}}}}}"##;
    let battle = "rule battle v1\n\nshape side = jsonschema \"side.schema.json\" \"#/$defs/Side\"\n\ninputs\n  morale : number  range >=0 <=2  from side.morale  fed by rulec \"recruit.rule\" output veterancy\n\nelements units from side.units\n  veterancy : number  range >=0 <=2  from veterancy  fed by rulec \"recruit.rule\" output veterancy\n\noutputs\n  veterans : number  round down(1)\n\ntable seasoned\npolicy unique\n| veterancy | -> seasoned : bool |\n| >=1       | true               |\n| <1        | false              |\n\ncount veterans_of over units where seasoned  range >=0 <=9\n\ntable total\npolicy unique\n| morale | veterans_of | -> veterans : number |\n| -      | -           | veterans_of          |\n";
    let files = Files::none().with("recruit.rule", recruit).with("side.schema.json", schema).with("battle.rule", battle);
    let v = holds(&files, "fed-field", 2);
    assert_eq!(record(&v, "elements units field veterancy")["takes"], "number >=0 <=2");
    assert_eq!(record(&v, "input morale")["takes"], "number >=0 <=2");

    // the rule gives more than the field and the input take
    let raised = files.clone().edit("recruit.rule", "| true     | 1 ", "| true     | 3 ");
    stops(
        &raised,
        "fed-field-raised",
        &[against("battle.rule", "input morale", "E207", Some("3")), against("battle.rule", "elements units field veterancy", "E207", Some("3"))],
    );

    // the page and the description say so, of a field as of an input
    let t = files.lay_out("fed-field-pages");
    let (_, page, _) = ritsu_in(t.path(), &["rulec", "doc", "battle.rule"]);
    assert_eq!(page.matches("Comes from `rulec \"recruit.rule\" output veterancy`").count(), 2, "{page}");
    let (_, api, _) = ritsu_in(t.path(), &["rulec", "api", "battle.rule"]);
    let api: Value = serde_json::from_str(&api).unwrap();
    assert_eq!(api["fed_by"][0]["name"], "morale");
    assert_eq!(api["fed_by"][1]["elements"], "units");
    assert_eq!(api["fed_by"][1]["name"], "veterancy");

    // nothing generated moves, for these two places either
    let gen_of = |f: &Files, tag: &str| {
        let t = f.lay_out(tag);
        let (code, out, err) = ritsu_in(t.path(), &["gen", "battle.rule", "--target", "python", "--out", "gen"]);
        assert_eq!(code, 0, "{out}{err}");
        tree(&t.path().join("gen"))
    };
    assert_eq!(gen_of(&files, "fed-field-gen-a"), gen_of(&files.without_the_form(), "fed-field-gen-b"));
}

/// A contribution becomes the rule it adds to, and that rule's lines name their sources from its
/// own directory: they are asked of once, for that rule. What the contribution writes itself is
/// read from the contribution's directory.
#[test]
fn a_contribution_answers_for_its_own_lines() {
    let base = "rule base v1\n\ninputs\n  v : number  range >=0 <=9  fed by rulec \"feeder.rule\" output y\n\noutputs\n  z : number  round half_even(1)\n\nopen bonus : rate  combine sum  range >=0% <=100%\n\ndefine z : number = half_even(v * (100% + bonus), 1)\n";
    let contribution = |line: &str| format!("contribution extra v1\nto \"../rules/base.rule\" signature:d7fb87de920ecdd6\n\ninputs\n  w : number  range >=0 <=9{line}\n\ntable extra_of\npolicy unique\n| w   | -> extra : rate[step 1%] |\n| >=5 | 10%                      |\n| <5  | 0%                       |\n\ninto bonus\n  extra\n");
    let files = Files::none().with("rules/feeder.rule", FEEDER).with("rules/base.rule", base);
    let own = files.clone().with("mods/extra.rule", &contribution(""));
    let (code, text, v) = own.check("fed-contribution");
    assert!(!text.contains("E208"), "{text}");
    assert_eq!(records(&v).len(), 1, "{code}: {}", v["borders"]);
    assert_eq!(records(&v)[0]["at"]["file"], "rules/base.rule");

    // what the contribution writes is read from where it stands
    let written = files.with("mods/extra.rule", &contribution("  fed by rulec \"../rules/feeder.rule\" output y"));
    let (_, text, v) = written.check("fed-contribution-own");
    assert!(!text.contains("E208"), "{text}");
    assert_eq!(records(&v).len(), 2, "{}", v["borders"]);
    assert!(records(&v).iter().any(|r| r["at"]["file"] == "mods/extra.rule" && r["about"] == "input w"), "{}", v["borders"]);
}

/// Undecided: the rule that is the source does not pass its own check. That is rulec's to say.
#[test]
fn a_source_its_language_does_not_pass_is_undecided() {
    let broken = Files::project().edit("refund_limit.rule", "| >1000USDc  | 2                |\n", "");
    let (code, text, v) = broken.check("fed-unanswered");
    assert_eq!(code, 1, "{text}");
    let r = record(&v, "input band");
    assert_eq!((r["result"].as_str(), r["code"].as_str()), (Some("undecided"), Some("W207")), "{r}");
    assert!(r["why"].as_str().unwrap().contains("rulec does not pass refund_limit.rule"), "{r}");
    assert!(text.contains("error[rulec E"), "rulec says what is wrong:\n{text}");
}

/// Undecided: a contract's date that lists no days, against a place with a range of days.
#[test]
fn a_contracts_date_with_no_list_of_days() {
    let open = Files::project().edit("schemas/order.schema.json", "\"type\": \"string\", \"enum\": [\"2026-04-01\", \"2026-05-01\", \"2026-06-01\"]", "\"type\": \"string\"");
    let (_, v) = stops(&open, "fed-contract-date", &[against("due.cal", "input issued_on", "W207", None)]);
    assert!(record(&v, "input issued_on")["why"].as_str().unwrap().contains("does not say which days"), "{}", v["borders"]);
}

/// Numbers in other units are other numbers: the place and the source count the same way or the
/// border does not hold, whatever the ends are. A step and a tax are part of how a value is counted.
#[test]
fn units_steps_and_tax() {
    // another currency, from a book
    let broken = Files::project().edit("refund.flow", "left : money[USDc]  range >=0USDc <=10000000USDc", "left : money[JPY]  range >=0JPY <=10000000JPY");
    let (_, v) = stops(&broken, "fed-unit-book", &[against("refund.flow", "record Balance field left", "E207", None)]);
    assert!(record(&v, "record Balance field left")["why"].as_str().unwrap().contains("counts in money[JPY], and the source in USD"), "{}", v["borders"]);
    // the same currency counted in its major unit
    let broken = Files::project()
        .edit("refund_limit.rule", "left : money[USDc]  range >=0USDc <=10000000USDc", "left : money[USD]  range >=0USD <=100000USD")
        .edit("refund_limit.rule", "<=1000USDc", "<=10USD")
        .edit("refund_limit.rule", ">1000USDc", ">10USD");
    stops(&broken, "fed-unit-major", &[against("refund_limit.rule", "input left", "E207", None)]);
    // tax
    let book = "book b v1\n\nunit JPY incl_tax\n\naccount left : JPY\n  at least 0 refused as none_left\n  at most 100 refused as too_much\naccount world : JPY outside\n\ntransfer t(request: string, amount: JPY)\n  key request\n  move amount from world to left\n";
    let rule = |ty: &str| taker(&format!("v : money[JPY{ty}]  range >=0JPY <=100JPY  fed by chobo \"b.book\" account left"));
    holds(&Files::none().with("b.book", book).with("r.rule", &rule(", incl_tax")), "fed-tax-same", 1);
    stops(&Files::none().with("b.book", book).with("r.rule", &rule(", excl_tax")), "fed-tax-other", &[against("r.rule", "input v", "E207", None)]);
    // a step
    let source = "rule s v1\n\ninputs\n  x : number  range >=0 <=1\n\noutputs\n  y : rate[step 1%]  round down(1%)\n\ntable t\npolicy unique\n| x | -> y : rate[step 1%] |\n| 0 | 1%                  |\n| 1 | 2%                  |\n";
    let with_step = |step: &str, top: &str| taker(&format!("v : rate[step {step}]  range >=0% <={top}%  fed by rulec \"s.rule\" output y"));
    holds(&Files::none().with("s.rule", source).with("r.rule", &with_step("1%", "5")), "fed-step-same", 1);
    stops(&Files::none().with("s.rule", source).with("r.rule", &with_step("0.1%", "5")), "fed-step-other", &[against("r.rule", "input v", "E207", None)]);
    // a number from a rule against a quantity
    let files = Files::none().with("feeder.rule", FEEDER).with("r.rule", &taker("v : mass[kg]  range >=0kg <=9kg  fed by rulec \"feeder.rule\" output y"));
    stops(&files, "fed-unit-number", &[against("r.rule", "input v", "E207", None)]);
}

/// The values of an enum, by their public names: from a rule, a document and a `.proto`, and an enum
/// read as a string.
#[test]
fn the_values_of_an_enum() {
    let source = "rule tiers v1\n\nenum tier = basic | gold | platinum\n\ninputs\n  x : number  range >=0 <=2\n\noutputs\n  t : tier\n\ntable t1\npolicy unique\n| x | -> t : tier |\n| 0 | basic      |\n| 1 | gold       |\n| 2 | platinum   |\n";
    let taking = |values: &str| format!("rule taker v1\n\nenum tier = {values}\n\ninputs\n  v : tier  fed by rulec \"tiers.rule\" output t\n\noutputs\n  z : bool\n\ntable t1\npolicy unique\n| v | -> z : bool |\n| - | true       |\n");
    holds(&Files::none().with("tiers.rule", source).with("r.rule", &taking("basic | gold | platinum | bronze")), "fed-enum-more", 1);
    let (_, v) = stops(&Files::none().with("tiers.rule", source).with("r.rule", &taking("basic | gold")), "fed-enum-less", &[against("r.rule", "input v", "E207", Some("platinum"))]);
    assert_eq!(record(&v, "input v")["example"]["at"], "x = 2");
    // into a string, every value is one
    holds(&Files::none().with("tiers.rule", source).with("r.rule", &taker("v : string  fed by rulec \"tiers.rule\" output t")), "fed-enum-string", 1);
    // from a document: the values it lists, against the place's
    let flow = |line: &str| format!("workflow w v1\n\nenum Reason = damaged | late\n\ninputs\n  {line}\n\noutputs\n  done : bool\n\nflow\n  succeed done = true\n");
    let doc = "{ \"$defs\": { \"Req\": { \"type\": \"object\", \"required\": [\"reason\"], \"properties\": { \"reason\": { \"type\": \"string\", \"enum\": [\"damaged\", \"late\", \"stolen\"] } } } } }\n";
    let files = Files::none().with("s.json", doc).with("w.flow", &flow("reason : Reason  fed by jsonschema \"s.json\" def Req property reason"));
    stops(&files, "fed-enum-doc", &[against("w.flow", "input reason", "E207", Some("\"stolen\""))]);
    holds(&Files::none().with("s.json", &doc.replace(", \"stolen\"", "")).with("w.flow", &flow("reason : Reason  fed by jsonschema \"s.json\" def Req property reason")), "fed-enum-doc-held", 1);
    // any string, against an enum
    let any = doc.replace(", \"enum\": [\"damaged\", \"late\", \"stolen\"]", "");
    stops(&Files::none().with("s.json", &any).with("w.flow", &flow("reason : Reason  fed by jsonschema \"s.json\" def Req property reason")), "fed-enum-any", &[against("w.flow", "input reason", "E207", None)]);
}

const DUE: &str = "dates terms v1\n\ninputs\n  received : date  range >=2026-01-01 <=2026-12-31\n\ndate due = received\n  + 30 days\n";

/// Days: a date of a dates file against a rule's range and a dates file's, with the input that
/// comes to the day; and a rule's `range from koyomi`.
#[test]
fn days_from_a_dates_file() {
    let rule = |range: &str| taker(&format!("v : date  range {range}  fed by koyomi \"terms.cal\" date due"));
    holds(&Files::none().with("terms.cal", DUE).with("r.rule", &rule(">=2026-01-01 <=2027-12-31")), "fed-days-rule", 1);
    let (_, v) = stops(&Files::none().with("terms.cal", DUE).with("r.rule", &rule(">=2026-01-01 <=2026-12-31")), "fed-days-rule-out", &[against("r.rule", "input v", "E207", Some("2027-01-01"))]);
    assert_eq!(record(&v, "input v")["example"]["at"], "received = 2026-12-02");
    let cal = |range: &str| format!("dates next v1\n\ninputs\n  from_day : date  range {range}  fed by koyomi \"terms.cal\" date due\n\ndate later = from_day\n  + 1 day\n");
    holds(&Files::none().with("terms.cal", DUE).with("next.cal", &cal(">=2026-01-01 <=2027-12-31")), "fed-days-cal", 1);
    stops(&Files::none().with("terms.cal", DUE).with("next.cal", &cal(">=2026-02-01 <=2027-12-31")), "fed-days-cal-out", &[against("next.cal", "input from_day", "E207", Some("2026-01-31"))]);
    // a date against what is no date
    stops(&Files::none().with("terms.cal", DUE).with("r.rule", &taker("v : number  range >=0 <=9  fed by koyomi \"terms.cal\" date due")), "fed-days-type", &[against("r.rule", "input v", "E207", None)]);
    // the days of the same date as the rule's range: every day comes
    holds(&Files::none().with("terms.cal", DUE).with("other.cal", DUE).with("r.rule", &taker("v : date  range from koyomi \"other.cal\" date due  fed by koyomi \"terms.cal\" date due")), "fed-days-koyomi", 1);
}

/// A contract's number: an open end is read as E122 reads it, and a `.proto`'s field with no rule
/// lets 0 through.
#[test]
fn a_proto_field() {
    let proto = |field: &str| format!("syntax = \"proto3\";\npackage shop.v1;\n\nenum Status {{\n  STATUS_UNSPECIFIED = 0;\n  STATUS_PAID = 1;\n}}\n\nmessage Order {{\n  {field}\n}}\n");
    let rule = taker("v : mass[g]  range >=1g <=40kg  fed by proto \"order.proto\" message Order field weight_g");
    // no rule on the field: 0 goes through, as E122 reads it
    let (_, v) = stops(&Files::none().with("order.proto", &proto("int64 weight_g = 1;")).with("r.rule", &rule), "fed-proto-zero", &[against("r.rule", "input v", "E207", Some("0"))]);
    assert_eq!(record(&v, "input v")["takes"], "mass[g] >=1 <=40000");
    // a field that may be left unset: its absence is the example, and a place of `T?` takes it
    let optional = proto("optional int64 weight_g = 1;");
    stops(&Files::none().with("order.proto", &optional).with("r.rule", &rule), "fed-proto-unset", &[against("r.rule", "input v", "E207", Some("none"))]);
    // a field with no such message
    let (_, v) = stops(&Files::none().with("order.proto", &proto("int64 weight = 1;")).with("r.rule", &rule), "fed-proto-field", &[against("r.rule", "input v", "E208", None)]);
    assert!(record(&v, "input v")["why"].as_str().unwrap().contains("has no field `weight_g`"), "{}", v["borders"]);
    // a string field into a number
    stops(&Files::none().with("order.proto", &proto("string weight_g = 1;")).with("r.rule", &rule), "fed-proto-type", &[against("r.rule", "input v", "E207", None)]);
    // an enum's values, by the names a rule calls them, without the zero that proto3 reads as not set
    let enumed = "rule taker v1\n\nenum status = paid\n\ninputs\n  v : status  fed by proto \"order.proto\" message Order field status\n\noutputs\n  z : bool\n\ntable t1\npolicy unique\n| v | -> z : bool |\n| - | true       |\n";
    holds(&Files::none().with("order.proto", &proto("Status status = 1;")).with("r.rule", enumed), "fed-proto-enum", 1);
    let other = enumed.replace("enum status = paid", "enum status = shipped");
    stops(&Files::none().with("order.proto", &proto("Status status = 1;")).with("r.rule", &other), "fed-proto-enum-other", &[against("r.rule", "input v", "E207", Some("\"paid\""))]);
}

/// What a contract's number lets through (an open end, an exclusive end, a list of values), its
/// other types, its `required` and its `null`.
#[test]
fn what_a_contract_leaves_open() {
    let doc = |property: &str, required: &str| format!("{{ \"$defs\": {{ \"N\": {{ \"type\": \"integer\", \"minimum\": 1, \"maximum\": 9 }}, \"Req\": {{ \"type\": \"object\", \"required\": [{required}], \"properties\": {{ \"n\": {property} }} }} }} }}\n");
    let source = "fed by jsonschema \"s.json\" def Req property n";
    let of = |property: &str, input: &str, tag: &str| -> (Files, String) {
        (Files::none().with("s.json", &doc(property, "\"n\"")).with("r.rule", &taker(&format!("{input}  {source}"))), tag.to_string())
    };
    let int = "v : number  range >=1 <=9";
    for (property, input, tag) in [
        ("{ \"type\": \"integer\", \"minimum\": 1, \"maximum\": 9 }", int, "ends"),
        ("{ \"type\": \"integer\", \"minimum\": 3, \"maximum\": 7 }", int, "inside"),
        ("{ \"type\": \"integer\", \"exclusiveMinimum\": 0, \"exclusiveMaximum\": 10 }", int, "exclusive"),
        ("{ \"type\": \"integer\", \"minimum\": 0.5, \"maximum\": 9.5 }", int, "fractions"),
        ("{ \"enum\": [1, 2, 9] }", int, "list"),
        ("{ \"const\": 9 }", int, "const"),
        ("{ \"$ref\": \"#/$defs/N\" }", int, "ref"),
        ("{ \"type\": \"boolean\" }", "v : bool", "bool"),
        ("{ \"type\": \"string\" }", "v : string", "string"),
        ("{ \"type\": \"string\", \"enum\": [\"a\"] }", "v : string", "string-list"),
    ] {
        let (files, tag) = of(property, input, &format!("fed-open-{tag}"));
        holds(&files, &tag, 1);
    }
    for (property, input, tag, value) in [
        ("{ \"type\": \"integer\" }", int, "open", Some("0")),
        ("{ \"type\": \"integer\", \"minimum\": 1 }", int, "no-maximum", Some("10")),
        ("{ \"type\": \"integer\", \"maximum\": 9 }", int, "no-minimum", Some("0")),
        ("{ \"type\": \"integer\", \"exclusiveMaximum\": 11 }", "v : number  range >=-9 <=9", "exclusive-top", Some("-10")),
        ("{ \"enum\": [1, 2, 10] }", int, "list-out", Some("10")),
        ("{ \"type\": \"integer\", \"minimum\": 1, \"maximum\": 9 }", "v : bool", "int-bool", None),
        ("{ \"type\": \"boolean\" }", "v : string", "bool-string", None),
        ("{ \"type\": \"number\" }", int, "fraction-number", None),
        ("{ \"type\": \"object\" }", int, "object", None),
        ("{ \"type\": \"string\" }", int, "string-number", None),
    ] {
        let (files, tag) = of(property, input, &format!("fed-open-{tag}"));
        stops(&files, &tag, &[against("r.rule", "input v", "E207", value)]);
    }
    // a contract's fraction reads only as a rate, and then its range is not compared
    let (files, tag) = of("{ \"type\": \"number\" }", "v : rate[step 1%]  range >=0% <=100%", "fed-open-rate");
    let (_, v) = stops(&files, &tag, &[against("r.rule", "input v", "W207", None)]);
    assert!(record(&v, "input v")["why"].as_str().unwrap().contains("a contract's fraction is not compared"), "{}", v["borders"]);
    // a place with no end writes nothing to compare: a workflow's input can
    let flow = "workflow w v1\n\ninputs\n  v : int  fed by jsonschema \"s.json\" def Req property n\n\noutputs\n  done : bool\n\nflow\n  succeed done = true\n";
    holds(&Files::none().with("s.json", &doc("{ \"type\": \"integer\" }", "\"n\"")).with("w.flow", flow), "fed-open-no-end", 1);
    // not in `required`, and `null`: the place must say it may have none
    let inside = "{ \"type\": \"integer\", \"minimum\": 1, \"maximum\": 9 }";
    let optional = "v : number?  range >=1 <=9";
    let unlisted = |input: &str| Files::none().with("s.json", &doc(inside, "")).with("r.rule", &taker(&format!("{input}  {source}")));
    let (_, v) = stops(&unlisted(int), "fed-unlisted", &[against("r.rule", "input v", "E207", Some("none"))]);
    assert!(record(&v, "input v")["why"].as_str().unwrap().contains("`required` does not list this property"), "{}", v["borders"]);
    holds(&unlisted(optional), "fed-unlisted-optional", 1);
    let nullable = "{ \"type\": [\"integer\", \"null\"], \"minimum\": 1, \"maximum\": 9 }";
    let (files, tag) = of(nullable, int, "fed-null");
    let (_, v) = stops(&files, &tag, &[against("r.rule", "input v", "E207", Some("none"))]);
    assert!(record(&v, "input v")["why"].as_str().unwrap().contains("lets null through"), "{}", v["borders"]);
    let (files, tag) = of(nullable, optional, "fed-null-optional");
    holds(&files, &tag, 1);
}

/// A workflow's output, a field of its record and a field of a module's: what is written on the
/// place is what can come.
#[test]
fn a_workflow_and_a_module_as_sources() {
    let files = Files::project()
        .with("a.rule", &taker("v : bool  fed by dandori \"refund.flow\" output paid"))
        .with("b.rule", &taker("v : money[USDc]  range >=0USDc <=10000000USDc  fed by dandori \"refund.flow\" record Balance field left"))
        .with("c.rule", &taker("v : number  range >=0 <=20  fed by kaname \"bill.kn\" record Line field items"));
    let v = holds(&files, "fed-places", 13);
    assert_eq!(record(&v, "input v")["result"], "held");
    let narrow = Files::project()
        .with("b.rule", &taker("v : money[USDc]  range >=0USDc <=100USDc  fed by dandori \"refund.flow\" record Balance field left"))
        .with("c.rule", &taker("v : number  range >=5 <=20  fed by kaname \"bill.kn\" record Line field items"));
    stops(
        &narrow,
        "fed-places-narrow",
        &[against("b.rule", "input v", "E207", Some("10000000")), against("c.rule", "input v", "E207", Some("1"))],
    );
    // nothing written on the source's side: the end the place writes cannot be said to hold
    let flow = "workflow w v1\n\nrecord Row\n  n : int\n\ninputs\n  x : int  range >=0 <=9\n\noutputs\n  done : bool\n\nflow\n  succeed done = true\n";
    let files = Files::none().with("w.flow", flow).with("r.rule", &taker("v : number  range >=0 <=9  fed by dandori \"w.flow\" record Row field n"));
    stops(&files, "fed-places-open", &[against("r.rule", "input v", "W207", None)]);
    // nothing in the source: E208
    let files = Files::none().with("w.flow", flow).with("r.rule", &taker("v : number  range >=0 <=9  fed by dandori \"w.flow\" output nothing"));
    stops(&files, "fed-places-missing", &[against("r.rule", "input v", "E208", None)]);
}

// ── What stops, and what is not there ──────────────────────────────────────────────────────────

/// What names no value stops at the language of the place, each with its code, and so does a form
/// that does not read: the check alone says it, with nothing else joined.
#[test]
fn what_names_no_value_stops_at_each_language() {
    let t = TempDir::new("fed-no-value");
    let each: [(&str, &str, String, &str); 5] = [
        ("rulec", "r.rule", taker("v : number  range >=0 <=9  fed by NAME"), "E144"),
        ("koyomi", "d.cal", "dates d v1\n\ninputs\n  v : date  range >=2026-01-01 <=2026-12-31  fed by NAME\n\ndate e = v\n  + 1 day\n".to_string(), "E017"),
        ("dandori", "w.flow", "workflow w v1\n\ninputs\n  v : int  range >=0 <=9  fed by NAME\n\noutputs\n  done : bool\n\nflow\n  succeed done = true\n".to_string(), "E023"),
        ("sekisho", "g.gate", "gate g v1\ndescription \"x\"\nnamespace G\n\nrole clerk\n  description \"x\"\n  can look\n\nprincipal User\n  description \"x\"\n  roles clerk\n\nresource Order\n  description \"x\"\n  attributes\n    v : number  range >=0 <=9  fed by NAME\n\naction look\n  description \"x\"\n  principal User\n  resource Order\n\npermit clerks\n  description \"x\"\n  principal in clerk\n  action look\n".to_string(), "E009"),
        ("kaname", "m.kn", "module m v1\ndescription \"x\"\n\nrecord R\n  v : number  range >=0 <=9  fed by NAME\n".to_string(), "E009"),
    ];
    for (tool, file, body, code) in each {
        for name in ["sakai \"map.ctx\" context Billing", "geas \"c.geas\" claim paid", "file \"notes.md\"", "rulec \"x.rule\" table t", "sekisho \"g.gate\" action look", "excel \"x.xlsx\"", "rulec x.rule output y", "rulec \"/etc/x.rule\" output y", ""] {
            t.write(file, body.replace("fed by NAME", &format!("fed by {name}")));
            let (exit, out, err) = ritsu_in(t.path(), &[tool, "check", file]);
            let said = format!("{out}{err}");
            assert_eq!(exit, 1, "{tool}: `{name}`:\n{said}");
            assert!(said.contains(code), "{tool}: `{name}`: no {code}:\n{said}");
        }
        std::fs::remove_file(t.path().join(file)).unwrap();
    }
}

/// A place the form cannot be written on stops there, with the language's code.
#[test]
fn where_the_form_cannot_be_written() {
    let t = TempDir::new("fed-places");
    let cases: [(&str, &str, String, &str); 9] = [
        // with a `from` the generated code works out, which nothing carries
        ("rulec", "r.rule", "rule r v1\n\nshape o(o) = proto \"o.proto\" shop.Order\n\ninputs\n  v(v) : number  range >=0 <=9  from count o.lines  fed by rulec \"x.rule\" output y\n\noutputs\n  z : bool\n\ntable t\npolicy unique\n| v | -> z : bool |\n| - | true       |\n".to_string(), "E144"),
        // with `random`, whose value is the rule's own draw
        ("rulec", "r.rule", "rule r v1\n\ninputs\n  v : number  range >=0 <=9  random  fed by rulec \"x.rule\" output y\n\noutputs\n  z : bool\n\ntable t\npolicy unique\n| v | -> z : bool |\n| - | true       |\n".to_string(), "E144"),
        // an output
        ("rulec", "r.rule", "rule r v1\n\ninputs\n  v : number  range >=0 <=9\n\noutputs\n  z : bool  fed by rulec \"x.rule\" output y\n\ntable t\npolicy unique\n| v | -> z : bool |\n| - | true       |\n".to_string(), "E144"),
        // an input a machine carries, whose value the machine's last call gave it
        ("rulec", "r.rule", "rule r v1\n\nenum st = a | b\n\ninputs\n  state : st  fed by rulec \"x.rule\" output after_it\n\noutputs\n  after_it : st\n\ntable step\npolicy unique\n| state | -> after_it : st |\n| a     | b               |\n| b     | b               |\n\nmachine m over step\n  carry state -> after_it\n  initial a\n  final b\n".to_string(), "E144"),
        // a `date` line of a dates file
        ("koyomi", "d.cal", "dates d v1\n\ninputs\n  v : date  range >=2026-01-01 <=2026-12-31\n\ndate e = v  fed by rulec \"x.rule\" output y\n  + 1 day\n".to_string(), "E017"),
        // a flow's output
        ("dandori", "w.flow", "workflow w v1\n\ninputs\n  v : int  range >=0 <=9\n\noutputs\n  done : bool  fed by rulec \"x.rule\" output y\n\nflow\n  succeed done = true\n".to_string(), "E023"),
        // a computed value of a gate
        ("sekisho", "g.gate", "gate g v1\ndescription \"x\"\nnamespace G\n\nrole clerk\n  description \"x\"\n  can look\n\nprincipal User\n  description \"x\"\n  roles clerk\n\nresource Order\n  description \"x\"\n  attributes\n    v : number  range >=0 <=9\n\naction look\n  description \"x\"\n  principal User\n  resource Order\n  context\n    w = limits(amount: 1).small  fed by rulec \"x.rule\" output y\n\npermit clerks\n  description \"x\"\n  principal in clerk\n  action look\n".to_string(), "E009"),
        // a parameter whose `)` is on its line
        ("kaname", "m.kn", "module m v1\ndescription \"x\"\n\nfn f(v: number range >=0 <=9 fed by rulec \"x.rule\" output y) -> number\n  v\n".to_string(), "E009"),
        // a function's answer
        ("kaname", "m.kn", "module m v1\ndescription \"x\"\n\nfn f(\n  v: number range >=0 <=9\n) -> number fed by rulec \"x.rule\" output y\n  v\n".to_string(), "E009"),
    ];
    for (tool, file, body, code) in cases {
        t.write(file, &body);
        let (exit, out, err) = ritsu_in(t.path(), &[tool, "check", file]);
        let said = format!("{out}{err}");
        assert_eq!(exit, 1, "{tool}:\n{body}\n{said}");
        assert!(said.contains(code), "{tool}: no {code}:\n{body}\n{said}");
    }
    // a parameter over several lines, the comma after the naming ending it, and a comment after it
    t.write("m.kn", "module m v1\ndescription \"x\"\n\nfn f(\n  a: number range >=0 <=9 fed by rulec \"x.rule\" output y,  # where it comes from\n  b: number range >=0 <=9\n) -> number range >=0 <=18\n  a + b\n");
    let (exit, out, err) = ritsu_in(t.path(), &["kaname", "check", "m.kn"]);
    assert_eq!(exit, 0, "{out}{err}");
}

/// A file or a thing that is not there, by each language that answers for it, and a file of another
/// language's.
#[test]
fn a_source_that_is_not_there() {
    let cases: [(&str, &str); 7] = [
        ("rulec \"nothing.rule\" output y", "there is no file nothing.rule"),
        ("rulec \"feeder.rule\" output nothing", "has no output `nothing` (its outputs are `y`)"),
        ("chobo \"feeder.rule\" account left", "a file of chobo is a .book file"),
        ("openapi \"nothing.json\" schema S property p", "there is no file nothing.json"),
        ("jsonschema \"s.json\" def Nothing property p", "the document does not hold"),
        ("jsonschema \"s.json\" pointer /$defs/Nothing", "the document does not hold"),
        ("openapi \"s.json\" schema S property p", "is no OpenAPI document"),
    ];
    for (i, (source, why)) in cases.into_iter().enumerate() {
        let files = Files::none()
            .with("feeder.rule", FEEDER)
            .with("s.json", "{ \"$defs\": { \"S\": { \"type\": \"integer\" } } }\n")
            .with("r.rule", &taker(&format!("v : number  range >=0 <=9  fed by {source}")));
        let (_, v) = stops(&files, &format!("fed-missing-{i}"), &[against("r.rule", "input v", "E208", None)]);
        assert!(record(&v, "input v")["why"].as_str().unwrap().contains(why), "{source}: {}", v["borders"]);
    }
    // the path goes from the directory of the file it is written in
    let files = Files::none().with("rules/feeder.rule", FEEDER).with("rules/deep/r.rule", &taker("v : number  range >=0 <=9  fed by rulec \"../feeder.rule\" output y"));
    let v = holds(&files, "fed-path", 1);
    assert_eq!(record(&v, "input v")["to"]["file"], "rules/feeder.rule");
    assert_eq!(record(&v, "input v")["source"], "rulec \"rules/feeder.rule\" output y");
    // out of the root
    let files = Files::none().with("ritsu.mod", "module example.com/acme/x\nversion 0.1.0\n").with("r.rule", &taker("v : number  range >=0 <=9  fed by rulec \"../feeder.rule\" output y"));
    let (_, v) = stops(&files, "fed-outside", &[against("r.rule", "input v", "E208", None)]);
    assert_eq!(record(&v, "input v")["to"], Value::Null);
}

/// A rule's output into its own input is the usual way to carry a value over, and two rules can feed
/// each other: nothing runs in a circle, as each answers for its own values.
#[test]
fn its_own_output_and_a_circle() {
    let own = "rule own v1\n\ninputs\n  n : number  range >=0 <=9  fed by rulec \"own.rule\" output again\n\noutputs\n  again : number  round down(1)\n\ntable t\npolicy unique\n| n   | -> again : number |\n| <=4 | 1                 |\n| >4  | 2                 |\n";
    let v = holds(&Files::none().with("own.rule", own), "fed-own", 1);
    assert_eq!(record(&v, "input n")["to"]["file"], "own.rule");
    let one = "rule one v1\n\ninputs\n  n : number  range >=0 <=9  fed by rulec \"two.rule\" output m\n\noutputs\n  m : number  round down(1)\n\ntable t\npolicy unique\n| n   | -> m : number |\n| <=4 | 1             |\n| >4  | 2             |\n";
    let two = one.replace("rule one", "rule two").replace("two.rule", "one.rule");
    holds(&Files::none().with("one.rule", one).with("two.rule", &two), "fed-circle", 2);
}

// ── What does not move ─────────────────────────────────────────────────────────────────────────

/// Every file under a directory, by its path from it.
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, from: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, from, out);
            } else {
                out.insert(p.strip_prefix(from).unwrap().to_str().unwrap().to_string(), std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// Whatever each generator writes for every target, with and without the form: the same bytes.
#[test]
fn nothing_generated_moves() {
    let with = Files::project().lay_out("fed-gen-with");
    let without = Files::project().without_the_form().lay_out("fed-gen-without");
    let mut compared = 0;
    let commands: [(&str, &[&str]); 7] = [
        ("rulec refund_limit", &["rulec", "gen", "refund_limit.rule", "--target", "python", "--target", "numpy", "--target", "typescript", "--target", "javascript", "--target", "rust", "--target", "ruby", "--target", "php", "--target", "go", "--target", "swift", "--target", "java", "--target", "sql", "--target", "wasm", "--target", "proto", "--extra", "runner", "--out", "out"]),
        ("rulec research", &["rulec", "gen", "research.rule", "--target", "python", "--target", "typescript", "--target", "go", "--out", "out"]),
        ("koyomi", &["koyomi", "gen", "due.cal", "--target", "typescript", "--target", "python", "--target", "go", "--target", "rust", "--target", "sql", "--target", "wasm", "--extra", "runner", "--out", "out"]),
        ("dandori", &["dandori", "gen", "refund.flow", "--target", "asl", "--target", "temporal-typescript", "--target", "temporal-python", "--target", "temporal-go", "--target", "durable", "--target", "argo", "--target", "pydantic-graph", "--out", "out"]),
        ("sekisho", &["sekisho", "gen", "desk.gate", "--target", "cedar", "--target", "typescript", "--target", "python", "--target", "go", "--module", "example.com/desk", "--out", "out"]),
        ("the packages", &["gen", ".", "--target", "typescript", "--target", "python", "--target", "go", "--target", "rust", "--target", "cedar", "--module", "example.com/refunds", "--out", "out"]),
        ("the packages for another database", &["gen", ".", "--target", "typescript", "--books", "mysql", "--out", "out"]),
    ];
    for (name, args) in commands {
        for t in [&with, &without] {
            let _ = std::fs::remove_dir_all(t.path().join("out"));
            let (code, out, err) = ritsu_in(t.path(), args);
            assert_eq!(code, 0, "{name}:\n{out}{err}");
        }
        let (a, b) = (tree(&with.path().join("out")), tree(&without.path().join("out")));
        assert!(!a.is_empty(), "{name} wrote nothing");
        assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>(), "{name}: the files differ");
        for (file, bytes) in &a {
            assert!(bytes == &b[file], "{name}: {file} differs with `fed by`");
        }
        compared += a.len();
    }
    eprintln!("compared: {compared} generated files, each the same with and without `fed by`");
    // the kaname module's own head names the same hash too
    assert_eq!(ritsu_base::fed::hash(&Files::project().0["bill.kn"]), ritsu_base::fed::hash(&Files::project().without_the_form().0["bill.kn"]));
}

/// What a rule's `meaning:` is taken from does not read the form, so a rule that applies another
/// keeps its pin; and what a language gives yuen to take the hash of is the same.
#[test]
fn a_rules_meaning_and_what_yuen_takes_the_hash_of() {
    let callee = "rule callee(callee) v1\n\ninputs\n  n(n) : number  range >=0 <=9\n\noutputs\n  m(m) : number  round down(1)\n\ntable t(t)\npolicy unique\n| n   | -> m : number |\n| <=4 | 1             |\n| >4  | 2             |\n";
    let caller = "rule caller(caller) v1\n\ninputs\n  k(k) : number  range >=0 <=9\n\noutputs\n  m(m) : number  round down(1)\n\napply 準用(apply_it) = \"callee.rule\"\n  n = k\n";
    let t = TempDir::new("fed-meaning");
    t.write("callee.rule", callee);
    t.write("caller.rule", caller);
    let (code, out, err) = ritsu_in(t.path(), &["rulec", "source", "pin", "caller.rule"]);
    assert_eq!(code, 0, "{out}{err}");
    let pinned = std::fs::read_to_string(t.path().join("caller.rule")).unwrap();
    assert!(pinned.contains("meaning:"), "{pinned}");
    t.write("callee.rule", callee.replace("range >=0 <=9\n", "range >=0 <=9  fed by rulec \"source.rule\" output n\n"));
    let (code, out, err) = ritsu_in(t.path(), &["rulec", "source", "pin", "caller.rule"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(std::fs::read_to_string(t.path().join("caller.rule")).unwrap(), pinned, "the pin moved with `fed by`");
    let (code, out, err) = ritsu_in(t.path(), &["rulec", "check", "caller.rule"]);
    assert_eq!(code, 0, "{out}{err}");

    // the items of each language: the same text for the same naming, with the form or without it
    let with = Files::project().lay_out("fed-items-with");
    let without = Files::project().without_the_form().lay_out("fed-items-without");
    let items = |root: &Path, file: &str, ports: &dyn Items| -> Vec<(String, String)> {
        ports.items(root, file).unwrap_or_else(|e| panic!("{file}: {e:?}")).into_iter().map(|i| (i.naming.text(), i.text)).collect()
    };
    let joined = ritsu_project::Joined::new();
    let languages: [(&str, &dyn Items); 5] = [
        ("refund_limit.rule", joined.rulec.as_ref()),
        ("due.cal", joined.koyomi.as_ref()),
        ("refund.flow", joined.dandori.as_ref()),
        ("desk.gate", joined.sekisho.as_ref()),
        ("bill.kn", joined.kaname.as_ref()),
    ];
    for (file, ports) in languages {
        let (a, b) = (items(with.path(), file, ports), items(without.path(), file, ports));
        assert!(!a.is_empty(), "{file} holds no item");
        assert_eq!(a, b, "{file}: an item moved with `fed by`");
    }
    // and the same languages say the reference, for sakai and yuen
    let said = |file: &str, ports: &dyn References| -> Vec<(String, String)> {
        ports.references(with.path(), file).unwrap().into_iter().filter(|r| r.how == "fed by").map(|r| (r.how, r.target.text())).collect()
    };
    assert_eq!(said("refund_limit.rule", joined.rulec.as_ref()), [("fed by".to_string(), "chobo \"refunds.book\" account remaining".to_string())]);
    assert_eq!(said("due.cal", joined.koyomi.as_ref()), [("fed by".to_string(), "jsonschema \"schemas/order.schema.json\" def Order property issued_on".to_string())]);
    assert_eq!(said("refund.flow", joined.dandori.as_ref()).len(), 3);
    assert_eq!(said("desk.gate", joined.sekisho.as_ref()), [("fed by".to_string(), "jsonschema \"schemas/order.schema.json\" def Order property amount".to_string())]);
    assert_eq!(said("bill.kn", joined.kaname.as_ref()).len(), 2);
}

/// `rulec fmt` aligns the tables and leaves a line of `inputs` as written, the form with it (the two
/// words with blanks between them, and a comment after the naming), and a second run leaves it.
#[test]
fn rulec_fmt_keeps_it() {
    let t = TempDir::new("fed-fmt");
    t.write("feeder.rule", FEEDER);
    t.write("r.rule", "rule taker v1\n\ninputs\n  v : number range >=0 <=9   fed   by rulec \"feeder.rule\" output y   # from the feeder\n  w : number  range >=0 <=9\n\noutputs\n  z : bool\n\ntable t\npolicy unique\n| v | -> z : bool |\n| - | true       |\n");
    let (code, out, err) = ritsu_in(t.path(), &["rulec", "fmt", "r.rule"]);
    assert_eq!(code, 0, "{out}{err}");
    let once = std::fs::read_to_string(t.path().join("r.rule")).unwrap();
    assert!(once.contains("fed   by rulec \"feeder.rule\" output y   # from the feeder"), "{once}");
    assert!(once.contains("| - | true        |"), "the table is aligned: {once}");
    let (code, out, err) = ritsu_in(t.path(), &["rulec", "fmt", "--check", "r.rule"]);
    assert_eq!(code, 0, "{once}\n{out}{err}");
    let (code, out, err) = ritsu_in(t.path(), &["check", "."]);
    assert_eq!(code, 0, "{once}\n{out}{err}");
    assert!(out.contains("borders between the languages: 1 checked, 0 undecided"), "{out}");
}

// ── What is said of it ─────────────────────────────────────────────────────────────────────────

/// Each language's page says where the value comes from, and what is and is not checked; each `api`
/// says the naming as written.
#[test]
fn the_pages_and_the_api_of_each_language() {
    let t = Files::project().lay_out("fed-pages");
    let en = "`ritsu check` holds the range of its values; the hand-written code that carries the value is not checked";
    let ja = "値の範囲は `ritsu check` が確かめます。値を運ぶ手書きのコードは確かめません";
    let pages: [(&str, &str, &str, &[&str]); 5] = [
        ("rulec", "refund_limit.rule", "chobo \"refunds.book\" account remaining", &["html"]),
        ("koyomi", "due.cal", "jsonschema \"schemas/order.schema.json\" def Order property issued_on", &["markdown", "html"]),
        ("dandori", "refund.flow", "openapi \"api/refunds.json\" schema RefundCreate property amount", &["md", "html"]),
        ("sekisho", "desk.gate", "jsonschema \"schemas/order.schema.json\" def Order property amount", &["markdown", "html"]),
        ("kaname", "bill.kn", "jsonschema \"schemas/order.schema.json\" def Order property items", &["markdown", "html"]),
    ];
    let unescaped = |s: &str| s.replace("&quot;", "\"").replace("&#34;", "\"").replace("&#x27;", "'").replace("&#96;", "`").replace("<code>", "`").replace("</code>", "`");
    for (tool, file, source, formats) in pages {
        let formats: Vec<Option<&str>> = if tool == "rulec" { vec![None, Some("html")] } else { formats.iter().copied().map(Some).collect() };
        for format in formats {
            for (lang, says) in [("en", en), ("ja", ja)] {
                let mut args = vec![tool, "doc", file, "--lang", lang];
                if let Some(f) = format {
                    args.extend(["--format", f]);
                }
                let (code, out, err) = ritsu_in(t.path(), &args);
                assert_eq!(code, 0, "{tool} {format:?} {lang}: {err}");
                let page = unescaped(&out);
                assert!(page.contains(says), "{tool} {format:?} {lang}: the page does not say what is checked");
                assert!(page.contains(source), "{tool} {format:?} {lang}: the page does not name {source}");
            }
        }
    }
    let api = |tool: &str, file: &str| -> Value {
        let (code, out, err) = ritsu_in(t.path(), &[tool, "api", file]);
        assert_eq!(code, 0, "{tool}: {err}");
        serde_json::from_str(&out).unwrap_or_else(|e| panic!("{tool}: {e}: {out}"))
    };
    let r = api("rulec", "refund_limit.rule");
    assert_eq!(r["fed_by"], serde_json::json!([{"name": "left", "alias": "left", "source": "chobo \"refunds.book\" account remaining"}]));
    assert_eq!(api("koyomi", "due.cal")["inputs"][0]["fed_by"], "jsonschema \"schemas/order.schema.json\" def Order property issued_on");
    let d = api("dandori", "refund.flow");
    assert_eq!(d["inputs"][0]["fed_by"], "openapi \"api/refunds.json\" schema RefundCreate property amount");
    assert_eq!(d["records"][0]["fields"][0]["fed_by"], "chobo \"refunds.book\" account remaining");
    assert_eq!(api("sekisho", "desk.gate")["types"][1]["attributes"][0]["fed_by"], "jsonschema \"schemas/order.schema.json\" def Order property amount");
    let k = api("kaname", "bill.kn");
    assert_eq!(k["records"][0]["fields"][0]["fed_by"], "jsonschema \"schemas/order.schema.json\" def Order property items");
    assert_eq!(k["functions"][0]["params"][2]["fed_by"], "jsonschema \"schemas/order.schema.json\" pointer /$defs/Order/properties/amount");
    // a rule with none says none: what a reader of `api` sees before the form is what it sees now
    let without = Files::project().without_the_form().lay_out("fed-pages-without");
    let (_, out, _) = ritsu_in(without.path(), &["rulec", "api", "refund_limit.rule"]);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v.get("fed_by"), None, "{out}");
    let (_, out, _) = ritsu_in(without.path(), &["koyomi", "api", "due.cal"]);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["inputs"][0].get("fed_by"), None, "{out}");
}

/// `ritsu explain` has the three codes, in both languages.
#[test]
fn ritsu_explains_the_codes() {
    let t = TempDir::new("fed-explain");
    for (code, en, ja) in [
        ("E207", "What can come from a place's source is not what the place takes", "出どころがとりうる値を、受け取る側が受け取りません"),
        ("W207", "Whether a place takes what can come from its source cannot be decided", "出どころの値を受け取るかを決められません"),
        ("E208", "The source of a `fed by` is not there", "`fed by` の出どころがありません"),
    ] {
        for (lang, title) in [("en", en), ("ja", ja)] {
            let (exit, out, err) = ritsu_in(t.path(), &["explain", code, "--lang", lang]);
            assert_eq!(exit, 0, "{code}: {err}");
            assert!(out.starts_with(code) && out.contains(title), "{code} {lang}: {out}");
        }
    }
}

// ── sakai and the modules ──────────────────────────────────────────────────────────────────────

/// A value hand-written code carries across a boundary needs what any reference across one needs.
#[test]
fn sakai_holds_it_to_the_boundary() {
    let beta = |published: &str| format!("context Beta(b) v1\nowns\n  dir \"b\"\n{published}");
    let alpha = |relation: &str| format!("context Alpha(a) v1\nowns\n  dir \"a\"\n{relation}");
    let base = Files::none()
        .with("map.ctx", "map Map(m) v1\nuse context \"alpha.ctx\"\nuse context \"beta.ctx\"\ncovers \".\"\n")
        .with("b/feeder.rule", FEEDER)
        .with("a/r.rule", &taker("v : number  range >=0 <=9  fed by rulec \"../b/feeder.rule\" output y"));
    // no relationship
    let files = base.clone().with("alpha.ctx", &alpha("")).with("beta.ctx", &beta(""));
    let (code, text, _) = files.check("fed-sakai-none");
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("error[sakai E201]"), "{text}");
    assert!(text.contains("fed by"), "{text}");
    // upstream, through a language the rule is not in
    let upstream = "\nupstream Beta conformist\n  through rulec.feeder.v1\n";
    let other = "\npublished language rulec.feeder.v1\n  rulec \"b/other.rule\"\n";
    let files = base.clone().with("alpha.ctx", &alpha(upstream)).with("beta.ctx", &beta(other)).with("b/other.rule", &FEEDER.replace("rule feeder", "rule other"));
    let (code, text, _) = files.check("fed-sakai-inside");
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("error[sakai E202]"), "{text}");
    assert!(text.contains("A `fed by` says where the value hand-written code carries comes from"), "{text}");
    // the rule published
    let published = "\npublished language rulec.feeder.v1\n  rulec \"b/feeder.rule\"\n";
    let files = base.clone().with("alpha.ctx", &alpha(upstream)).with("beta.ctx", &beta(published));
    let (code, text, v) = files.check("fed-sakai-published");
    assert_eq!(code, 0, "{text}");
    assert_eq!(not_held(&v), vec![], "{text}");
    // a book is not something a context can publish: only a shared kernel lets it cross
    let book = "book b v1\n\nunit USD scale 2\n\naccount left : USD\n  at least 0 refused as none_left\n  at most 100 refused as too_much\naccount world : USD outside\n\ntransfer t(request: string, amount: USD)\n  key request\n  move amount from world to left\n";
    let files = Files::none()
        .with("map.ctx", "map Map(m) v1\nuse context \"alpha.ctx\"\nuse context \"beta.ctx\"\ncovers \".\"\n")
        .with("b/b.book", book)
        .with("a/r.rule", &taker("v : money[USDc]  range >=0USDc <=10000USDc  fed by chobo \"../b/b.book\" account left"))
        .with("alpha.ctx", &alpha(upstream))
        .with("beta.ctx", &beta(published))
        .with("b/feeder.rule", FEEDER);
    let (code, text, _) = files.check("fed-sakai-book");
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("error[sakai E202]"), "{text}");
}

/// A JSON Schema is no context's artifact, as a JSON or a Smithy model is not: a `fed by` that names
/// one under another context's directory crosses nothing sakai can hold. What this holds is that it
/// stays so until a schema can be published, and the question is in DESIGN 7.16.
#[test]
fn a_json_schema_belongs_to_no_context() {
    let files = Files::none()
        .with("map.ctx", "map Map(m) v1\nuse context \"alpha.ctx\"\nuse context \"beta.ctx\"\ncovers \".\"\n")
        .with("alpha.ctx", "context Alpha(a) v1\nowns\n  dir \"a\"\n")
        .with("beta.ctx", "context Beta(b) v1\nowns\n  dir \"b\"\n")
        .with("b/order.schema.json", "{ \"$defs\": { \"Order\": { \"type\": \"object\", \"properties\": { \"y\": { \"type\": \"integer\", \"minimum\": 0, \"maximum\": 9 } }, \"required\": [\"y\"] } } }\n")
        .with("a/r.rule", &taker("v : number  range >=0 <=9  fed by jsonschema \"../b/order.schema.json\" def Order property y"));
    let (code, text, v) = files.check("fed-sakai-schema");
    assert_eq!(code, 0, "{text}");
    assert!(!text.contains("sakai E20"), "{text}");
    assert_eq!(record(&v, "input v")["result"], "held", "{text}");
}

/// yuen reads the fifteenth tool as it reads the others: a requirement satisfied by a property or a
/// pointer of a JSON Schema has its version reviewed, and a changed or a renamed property stops it.
#[test]
fn a_requirement_on_a_json_schema() {
    let req = "requirements order v1\n\nrole sales\n\nrequirement order_amount\n  text \"An order's amount is a whole number of cents, from 1 to 1,000,000\"\n  owner sales\n  decided 2026-11-07 by sales \"a sales meeting\"\n  satisfied by jsonschema \"schemas/order.schema.json\" def Order property amount\n  satisfied by jsonschema \"schemas/order.schema.json\" pointer /$defs/Order/properties/items\n  not verified \"nothing in this project checks the schema\"\n";
    let t = Files::project().with("order.req", req).lay_out("fed-yuen-schema");
    let (code, out, _) = ritsu_in(t.path(), &["check", "."]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("error[yuen E301]"), "not reviewed yet:\n{out}");
    let (code, out, err) = ritsu_in(t.path(), &["yuen", "review", ".", "--root", ".", "--all", "--by", "sales", "--date", "2026-11-07"]);
    assert_eq!(code, 0, "{out}{err}");
    let (code, out, _) = ritsu_in(t.path(), &["check", "."]);
    assert_eq!(code, 0, "{out}");
    let schema = t.path().join("schemas/order.schema.json");
    let body = std::fs::read_to_string(&schema).unwrap();
    std::fs::write(&schema, body.replace("\"maximum\": 1000000", "\"maximum\": 500000")).unwrap();
    let (code, out, _) = ritsu_in(t.path(), &["check", "."]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("error[yuen E303]"), "the property changed:\n{out}");
    std::fs::write(&schema, body.replace("\"amount\": {", "\"total\": {")).unwrap();
    let (code, out, _) = ritsu_in(t.path(), &["check", "."]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("error[yuen E202]"), "the property is gone:\n{out}");
    assert!(out.contains("has no property amount"), "{out}");
}

/// A source in a dependency, named by `@`: what the place takes against what the version in the lock
/// gives, and a new E207 in `ritsu mod report` when the dependency is raised to a version that gives
/// more, which the report gives to the place's file.
#[test]
fn a_source_in_a_dependency() {
    if Command::new("git").arg("--version").output().is_err() {
        eprintln!("SKIP: no git");
        return;
    }
    let git = |dir: &Path, args: &[&str]| {
        let o = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=ritsu-test", "-c", "user.email=ritsu-test@example.com", "-c", "commit.gpgsign=false", "-c", "init.defaultBranch=main"])
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .expect("could not run git");
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    };
    let t = TempDir::new("fed-module");
    let store_dir = t.path().join("store");
    let store_at = Store::at(&store_dir);
    t.write("v1/ritsu.mod", "module example.com/acme/feeder\nversion 1.0.0\n");
    t.write("v1/rules/feeder.rule", FEEDER);
    t.write("v2/ritsu.mod", "module example.com/acme/feeder\nversion 2.0.0\n");
    t.write("v2/rules/feeder.rule", FEEDER.replace("| >=1 | 2             |", "| >=1 | 20            |"));
    for v in ["v1", "v2"] {
        store::put_directory(&store_at, &t.path().join(v), "2026-11-07", &[]).unwrap_or_else(|e| panic!("{}", e.en));
    }
    let run = |args: &[&str]| -> (i32, String, String) {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
        c.current_dir(t.path().join("app")).args(args).env(store::VARIABLE, &store_dir);
        c.env_remove("RITSU_LANG");
        let o = c.output().expect("could not run ritsu");
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
    };
    t.write("app/ritsu.mod", "module example.com/acme/app\nversion 0.1.0\n\nrequire feeder oci \"example.com/acme/feeder\" 1.0.0\n");
    t.write("app/r.rule", taker("v : number  range >=0 <=9  fed by rulec \"@feeder/rules/feeder.rule\" output y"));
    let lock = || {
        let (code, out, err) = run(&["mod", "lock"]);
        assert_eq!(code, 0, "{out}{err}");
    };
    lock();
    let repo = t.path().join("app");
    let (code, out, err) = run(&["check", ".", "--format", "json"]);
    assert_eq!(code, 0, "{out}{err}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(not_held(&v), vec![], "{out}");
    assert_eq!(record(&v, "input v")["source"], "rulec \"@feeder/rules/feeder.rule\" output y");
    assert_eq!(record(&v, "input v")["to"]["file"], "@feeder/rules/feeder.rule");
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "feeder 1.0.0"]);
    git(&repo, &["tag", "one"]);
    t.write("app/ritsu.mod", "module example.com/acme/app\nversion 0.1.0\n\nrequire feeder oci \"example.com/acme/feeder\" 2.0.0\n");
    lock();
    let (code, out, err) = run(&["mod", "report", "--base", "one", "--format", "json"]);
    assert_eq!((code, err.as_str()), (1, ""), "{out}");
    let v: Value = serde_json::from_str(&out).unwrap();
    let new: Vec<(&str, &str, &str)> = v["check"]["new"].as_array().unwrap().iter().map(|d| (d["tool"].as_str().unwrap(), d["code"].as_str().unwrap(), d["file"].as_str().unwrap())).collect();
    assert_eq!(new, [("ritsu", "E207", "r.rule")], "{out}");
    let (_, text, _) = run(&["mod", "report", "--base", "one"]);
    assert!(text.contains("error[ritsu E207] r.rule"), "{text}");
}
