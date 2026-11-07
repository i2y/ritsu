//! `never` (§15.282): a combination that does not happen, written where the person who approves the
//! rule reads it.
//!
//! What is tested is the whole line the feature draws, the same one `constraint` draws (§15.55): the
//! check stops asking for rows for what the line takes out, the rows that meet only there do not
//! overlap, an example that falls under it is an error, the vectors keep clear of it, the page says
//! it, and a line over definitions is shown from them or stopped. The door is in `tests/door.rs`,
//! the re-checkers in `tests/cert.rs` and `tests/lean.rs`, and whether what the check says of a line
//! over definitions is true is held to every small request in `tests/from_relations.rs`.

use ritsu_testkit::{Need, TempDir, ready};
use rulec::diag::Diag;
use rulec::i18n::{self, Lang};
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(lang: &str, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", lang).current_dir(root()).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn corpus(name: &str) -> String {
    format!("tests/corpus/{name}.rule")
}

fn text(name: &str) -> String {
    std::fs::read_to_string(root().join(corpus(name))).unwrap()
}

/// What `check` says of a rule's text, read as if it stood beside the corpus's contracts.
fn diags(src: &str, lang: Lang) -> Vec<Diag> {
    let path = root().join("tests/corpus/under_test.rule");
    i18n::with(lang, || rulec::report(src, &path.to_string_lossy()).diags)
}

fn codes(ds: &[Diag]) -> Vec<&'static str> {
    ds.iter().map(|d| d.code).filter(|c| c.starts_with('E') || *c == "W133").collect()
}

fn one<'a>(ds: &'a [Diag], code: &str) -> &'a Diag {
    let found: Vec<&Diag> = ds.iter().filter(|d| d.code == code).collect();
    assert_eq!(found.len(), 1, "{code}: {ds:#?}");
    found[0]
}

fn write(d: &Path, name: &str, body: &str) -> String {
    let p = d.join(name);
    std::fs::write(&p, body).unwrap();
    p.to_string_lossy().into_owned()
}

const LINES: [(&str, &[(&str, &str)]); 3] = [
    ("never has_tool = true, all_parts = true\n", &[("has_tool", "true"), ("all_parts", "true")]),
    ("never tier = gold, tools = 0\n", &[("tier", "gold"), ("tools", "0")]),
    ("never tools = 0, total > 0USD\n", &[("tools", "0")]),
];

/// With each line the tables are complete; with it gone, E101 asks for the combination it took out.
#[test]
fn a_line_takes_its_combination_out_of_the_tables() {
    let src = text("order_fee");
    assert_eq!(codes(&diags(&src, Lang::En)), Vec::<&str>::new());
    for (line, says) in LINES {
        assert!(src.contains(line), "{line}");
        let ds = diags(&src.replace(line, ""), Lang::En);
        let gap = ds.iter().find(|d| d.code == "E101").unwrap_or_else(|| panic!("{line}: no E101: {ds:#?}"));
        for (name, value) in says {
            let (_, v) = gap.witness.inputs.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("{line}: {:?}", gap.witness.inputs));
            assert_eq!(v.text(), *value, "{line}");
        }
    }
    // A line that takes out less than the gap leaves the rest of it for a row.
    let narrower = src.replace("never tools = 0, total > 0USD\n", "never tools = 0, total > 5USD\n");
    assert_eq!(codes(&diags(&narrower, Lang::En)), ["E101"]);
}

const OVERLAP: &str = r#"rule overlap v1

inputs
  a : bool
  b : bool

never a = true, b = true

outputs
  fee : number  round down(1)

table t
policy unique
| a    | b     | -> fee : number |
| true | -     | 1               |
| -    | true  | 2               |
| false | false | 3              |
"#;

/// Two rows that meet only where a line says nothing comes are not an overlap.
#[test]
fn rows_that_meet_only_inside_a_line_do_not_overlap() {
    assert_eq!(codes(&diags(OVERLAP, Lang::En)), Vec::<&str>::new());
    let without = OVERLAP.replace("never a = true, b = true\n", "");
    assert_eq!(codes(&diags(&without, Lang::En)), ["E105"]);
}

const ROW_INSIDE: &str = r##"rule row_inside v1

shape order = jsonschema "contracts/order_lines.schema.json" "#/$defs/Order"

inputs
  has_fragile : bool  from any order.lines where fragile = true
  all_fragile : bool  from all order.lines where fragile = true

never has_fragile = false, all_fragile = true

outputs
  fee : money[USD]  round up(1USD)

table t
policy unique
| has_fragile | all_fragile | -> fee : money[USD] |
| true        | true        | 9USD                |
| true        | false       | 3USD                |
| false       | true        | 99USD               |
| false       | false       | 0USD                |
"##;

/// W133 tells a person to delete the row and write the line. With the line written it has nothing
/// left to say, and the row is what a row a `constraint` leaves out is: `coverage` names it, with
/// the line.
#[test]
fn a_row_inside_a_line_is_the_lines_and_coverage_names_it() {
    let without = ROW_INSIDE.replace("never has_fragile = false, all_fragile = true\n", "");
    let ds = diags(&without, Lang::En);
    let w = one(&ds, "W133");
    let said = w.notes.join("\n");
    assert!(said.contains("delete this row and write this line instead: `never has_fragile = false, all_fragile = true`"), "{said}");
    assert_eq!(codes(&diags(ROW_INSIDE, Lang::En)), Vec::<&str>::new());
    let t = TempDir::new("never-row");
    std::fs::create_dir(t.path().join("contracts")).unwrap();
    std::fs::copy(root().join("tests/corpus/contracts/order_lines.schema.json"), t.path().join("contracts/order_lines.schema.json")).unwrap();
    let p = write(t.path(), "r.rule", ROW_INSIDE);
    let (_, out, e) = run("en", &["coverage", &p]);
    assert!(out.contains("`never has_fragile = false, all_fragile = true` takes this row's combination out of the tables"), "{out}{e}");
}

/// E101's note shows the line to write only where the definitions rule the combination out, and the
/// line it shows passes.
#[test]
fn the_note_on_e101_shows_a_line_that_passes() {
    let line = "never has_fragile = false, all_fragile = true\n";
    let src = text("fragile_lines_fee").replace(line, "");
    let ds = diags(&src, Lang::En);
    let gap = one(&ds, "E101");
    let said = gap.notes.join("\n");
    assert!(said.contains(&format!("comes out of the tables with this line, and no row: `{}`", line.trim_end())), "{said}");
    // Where a request gives it, nothing says it does not happen.
    let beside = "rule t v1\n\nshape order = jsonschema \"contracts/order_lines.schema.json\" \"#/$defs/Order\"\n\ninputs\n  a : bool  from any order.lines where fragile = true\n  b : bool  from any order.lines where fragile = false\n\noutputs\n  fee : number  round down(1)\n\ntable t\npolicy unique\n| a | b | -> fee : number |\n| true | - | 1 |\n| false | false | 2 |\n";
    let ds = diags(beside, Lang::En);
    let gap = ds.iter().find(|d| d.code == "E101").unwrap_or_else(|| panic!("{ds:#?}"));
    assert!(!gap.notes.join("\n").contains("never"), "{:#?}", gap.notes);
}

/// `has_tool <= all_parts` cannot be written, and E018 says what can.
#[test]
fn e018_points_at_never() {
    let src = text("order_fee").replace("never has_tool = true, all_parts = true\n", "constraint has_tool <= all_parts\n");
    let ds = diags(&src, Lang::En);
    let e = ds.iter().find(|d| d.code == "E018").unwrap_or_else(|| panic!("{ds:#?}"));
    assert!(e.notes.join("\n").contains("`never`"), "{:#?}", e.notes);
}

/// An example under a line is an input the door refuses, in English and in the Japanese version.
#[test]
fn an_example_under_a_line_is_e019() {
    for (name, row, lang) in [
        ("order_fee", ("| false    | false     | basic | 0     | 0USD   | 2USD   |", "| true     | true      | basic | 1     | 50USD  | 12USD  |"), Lang::En),
        ("注文の手数料", ("| false    | false    | 通常 | 0      | 0USD   | 2USD      |", "| true     | true     | 通常 | 1      | 50USD  | 12USD     |"), Lang::Ja),
    ] {
        let src = text(name);
        assert!(src.contains(row.0), "{name}");
        let ds = diags(&src.replace(row.0, row.1), lang);
        one(&ds, "E019");
    }
}

/// No vector of any line's rule falls under a line, and every combination a rule leaves in is tried.
#[test]
fn the_vectors_keep_clear_of_the_lines() {
    let (code, out, e) = run("en", &["vectors", &corpus("order_fee")]);
    assert_eq!(code, 0, "{e}");
    let (mut n, mut gold_with_tools) = (0, 0);
    for line in out.lines().filter(|l| !l.trim().is_empty()) {
        let j = rulec::json::parse(line).unwrap();
        let i = j.get("in").unwrap();
        let t = |k: &str| i.get(k).and_then(|v| v.as_bool()).unwrap();
        let int = |k: &str| i.get(k).and_then(|v| v.as_int()).unwrap();
        let tier = i.get("tier").and_then(|v| v.as_str()).unwrap();
        assert!(!(t("has_tool") && t("all_parts")), "{line}");
        assert!(!(tier == "gold" && int("tools") == 0), "{line}");
        assert!(!(int("tools") == 0 && int("total") > 0), "{line}");
        n += 1;
        if tier == "gold" && int("tools") > 0 {
            gold_with_tools += 1;
        }
    }
    assert!(n > 0 && gold_with_tools > 0, "{out}");
    // The first candidates are under a line here (`has_tool` and `all_parts` both true), and the
    // boundary of `total` is still tried on both sides, with the other inputs where they can be.
    for edge in ["\"total\":100,", "\"total\":101,"] {
        assert!(out.contains(edge.trim_end_matches(',')), "{edge}\n{out}");
    }
}

/// The page lists the lines and tells a line shown from definitions from a caller's promise.
#[test]
fn the_page_says_which_combinations_do_not_happen() {
    for (name, lang, heading, how, line) in [
        ("order_fee", "en", "These combinations do not happen", "the caller's promise", "never tier = gold, tools = 0"),
        ("注文の手数料", "ja", "この組み合わせは起こらない", "呼ぶ側の約束です", "never 区分 = 上位, 工具数 = 0"),
        ("fragile_lines_fee", "en", "These combinations do not happen", "shown from the definitions", "never has_fragile = true, fragile = 0"),
        ("割れ物の明細の手数料", "ja", "この組み合わせは起こらない", "定義から確かめました", "never 割れ物あり = true, 割れ物数 = 0"),
    ] {
        let (code, out, e) = run(lang, &["doc", &corpus(name), "--lang", lang]);
        assert_eq!(code, 0, "{name}: {e}");
        for want in [heading, how, line] {
            assert!(out.contains(want), "{name}: no `{want}`:\n{out}");
        }
    }
}

const V1: &str = "rule tool_fee v1\n\ninputs\n  has_tool  : bool\n  all_parts : bool\n\noutputs\n  fee : money[USD]  round up(1USD)\n\ntable tool_fee\npolicy unique\n| has_tool | all_parts | -> fee : money[USD] |\n| true     | true      | 9USD                |\n| true     | false     | 5USD                |\n| false    | true      | 0USD                |\n| false    | false     | 2USD                |\n";

/// `rulec diff` reads a line as it reads a constraint: a combination only the new version takes out
/// is a change, one both take out is nobody's case.
#[test]
fn diff_reads_a_line() {
    let t = TempDir::new("never-diff");
    let v1 = write(t.path(), "v1.rule", V1);
    let with_line = V1
        .replace("rule tool_fee v1", "rule tool_fee v2")
        .replace("outputs\n", "never has_tool = true, all_parts = true\n\noutputs\n")
        .replace("| true     | true      | 9USD                |\n", "");
    let v2 = write(t.path(), "v2.rule", &with_line);
    let v3 = write(t.path(), "v3.rule", &with_line.replace("rule tool_fee v2", "rule tool_fee v3").replace("| false    | false     | 2USD                |", "| false    | false     | 3USD                |"));
    let (_, out, e) = run("en", &["diff", &v1, &v2]);
    assert!(out.contains("fee: 9 → -") && out.contains("outside this region the two versions answer alike"), "{out}{e}");
    let (_, out, e) = run("en", &["diff", &v2, &v3]);
    assert!(out.contains("fee: 2 → 3"), "{out}{e}");
    assert!(out.contains("0 unrealized") && !out.contains("?"), "the combination both versions take out is nobody's case:\n{out}{e}");
    assert!(out.contains("outside this region the two versions answer alike"), "{out}{e}");
    // A record of the old version that falls under the new version's line is named with it.
    let r = write(t.path(), "r.jsonl", "{\"in\":{\"has_tool\":true,\"all_parts\":true},\"out\":{\"fee\":9}}\n{\"in\":{\"has_tool\":false,\"all_parts\":false},\"out\":{\"fee\":2}}\n");
    let (_, out, e) = run("en", &["diff", &v1, &v2, "--fixtures", &r]);
    assert!(out.contains("falls under the new version's `never has_tool = true, all_parts = true`"), "{out}{e}");
}

const CALLEE: &str = "rule callee v1\n\ninputs\n  a : bool\n  b : bool\n\nnever a = true, b = true\n\noutputs\n  fee : number  round down(1)\n\ntable t\npolicy unique\n| a | b | -> fee : number |\n| true | false | 1 |\n| false | true | 2 |\n| false | false | 3 |\n";

/// A callee's line is the caller's to keep once values are bound to it: the same line said of the
/// names passed, or a literal or a range that misses one of its conditions.
#[test]
fn apply_holds_a_callee_s_line() {
    let t = TempDir::new("never-apply");
    write(t.path(), "callee.rule", CALLEE);
    let digest = rulec::sha256::short(CALLEE.as_bytes());
    let caller = |extra: &str, x: &str, y: &str| {
        format!("rule caller v1\n\ninputs\n  x : bool\n  y : bool\n\n{extra}outputs\n  fee : number  round down(1)\n\napply c = \"callee.rule\" sha256:{digest}\n  a = {x}\n  b = {y}\n  fee -> fee\n")
    };
    let check = |src: String| {
        let p = write(t.path(), "caller.rule", &src);
        i18n::with(Lang::En, || rulec::report(&src, &p).diags)
    };
    let ds = check(caller("", "x", "y"));
    assert!(ds.iter().any(|d| d.code == "E043" && d.title.contains("never")), "{ds:#?}");
    let ds = check(caller("never x = true, y = true\n\n", "x", "y"));
    assert!(!ds.iter().any(|d| d.code == "E043"), "{ds:#?}");
    let ds = check(caller("", "x", "false"));
    assert!(!ds.iter().any(|d| d.code == "E043"), "a literal that misses a condition keeps the line: {ds:#?}");
}

/// `--format json` carries the request behind E142 and E123, so a tool can read it.
#[test]
fn the_request_is_in_the_json() {
    let (code, out, _) = run("en", &["check", "tests/mutants/m_e142.rule", "--format", "json"]);
    assert_eq!(code, 1);
    let d: Vec<rulec::json::Json> = out.lines().filter_map(|l| rulec::json::parse(l).ok()).collect();
    let e = d.iter().find(|x| x.get("code").and_then(|v| v.as_str()) == Some("E142")).unwrap_or_else(|| panic!("{out}"));
    let r = e.get("request").unwrap_or_else(|| panic!("{out}"));
    let c = r.get("collections").and_then(|v| v.as_arr()).unwrap_or_else(|| panic!("{out}"));
    assert_eq!(c[0].get("path").and_then(|v| v.as_str()), Some("order.lines"), "{out}");
    let (_, out, _) = run("en", &["check", "tests/mutants/m_e123reads.rule", "--format", "json"]);
    assert!(out.contains("\"request\":"), "{out}");
}

const MACHINE: &str = "rule steps v1\n\nenum status = open | done | void\n\ninputs\n  status : status\n  paid   : bool\n  voided : bool\n\nnever paid = true, voided = true\n\noutputs\n  moved : status\n\ntable t\npolicy unique\n| status | paid  | voided | -> moved |\n| open   | true  | false  | done     |\n| open   | false | true   | void     |\n| open   | false | false  | open     |\n| done   | -     | -      | status   |\n| void   | -     | -      | status   |\n\nmachine steps over t\n  carry   status -> moved\n  initial open\n  final   done, void\n  never   done after void\n";

/// The word heads a line now, and a name that was legal stays legal: a field of an element could
/// always be named `never`, an input never could.
#[test]
fn a_declaration_named_never_reads_as_it_did() {
    let field = "rule word v1\n\nelements xs\n  never : bool\n\ncount n over xs where never  range >=0 <=5\n\noutputs\n  fee : money[USD]  round up(1USD)\n\ntable fee_of\npolicy unique\n| n   | -> fee : money[USD] |\n| <=3 | 1USD                |\n| >3  | 2USD                |\n";
    assert_eq!(codes(&diags(field, Lang::En)), Vec::<&str>::new());
    let input = "rule word v1\n\ninputs\n  never : bool\n\noutputs\n  fee : money[USD]  round up(1USD)\n\ntable fee_of\npolicy unique\n| never | -> fee : money[USD] |\n| true  | 1USD                |\n| false | 2USD                |\n";
    assert_eq!(codes(&diags(input, Lang::En)), ["E009"]);
}

/// Under `machine` the word is the machine's line, as it always was; outside it is this one.
#[test]
fn a_line_under_machine_is_the_machines() {
    let (f, _) = i18n::with(Lang::En, || rulec::prepare(MACHINE, "machine.rule")).unwrap_or_else(|d| panic!("{d:#?}"));
    assert_eq!(f.nevers.len(), 1, "{:?}", f.nevers);
    assert_eq!(f.machine.as_ref().unwrap().nevers.len(), 1);
    assert_eq!(f.nevers[0].said(), "never paid = true, voided = true");
}

/// A call of a scenario under a line is what an example under one is: E019, naming the line.
#[test]
fn a_call_of_a_scenario_under_a_line_is_e019() {
    let walk = "\nscenario paid_and_voided\n| paid  | voided | -> moved |\n| true  | true   | done     |\n";
    let d = one(&diags(&format!("{MACHINE}{walk}"), Lang::En), "E019").title.clone();
    assert_eq!(d, "Call 1 of scenario paid_and_voided falls under `never paid = true, voided = true`");
    let kept = "\nscenario paid\n| paid  | voided | -> moved |\n| true  | false  | done     |\n";
    assert_eq!(codes(&diags(&format!("{MACHINE}{kept}"), Lang::En)), Vec::<&str>::new());
}

const AT_A_CALL: &str = "rule refund_route v1\n\nenum path = at_once | review\n\ninputs\n  shipped : number  range >=0 <=100\n  asked   : number  range >=0 <=10000\n  express : bool\n\nnever shipped = 0, asked >= 1\nnever express = true, shipped >= 50\n\noutputs\n  route : path\n\ntable pick\npolicy unique\n| asked | -> route : path |\n| <=100 | at_once         |\n| >100  | review          |\n";

/// What the port answers of a line where another language calls the rule (§15.282): a condition no
/// value the call gives can meet shows the line holds, numbers that all can meet it are an example,
/// and a truth value leaves it undecided.
#[test]
fn the_port_decides_a_line_over_the_ranges_a_call_gives() {
    use ritsu_ports::{Answer, Precondition, Rules, Value};
    let t = TempDir::new("never-port");
    let p = PathBuf::from(write(t.path(), "r.rule", AT_A_CALL));
    let engine = rulec::ports::Engine::new();
    let answers = |ranges: &[(&str, i128, i128)]| {
        let given: Vec<(String, Option<i128>, Option<i128>)> = ranges.iter().map(|(n, a, b)| (n.to_string(), Some(*a), Some(*b))).collect();
        let got = i18n::with(Lang::En, || engine.preconditions_hold(&p, &given, None)).unwrap();
        got.into_iter().filter(|(q, _)| matches!(q, Precondition::Never { .. })).map(|(_, a)| a).collect::<Vec<_>>()
    };
    let got = answers(&[("shipped", 1, 49), ("asked", 0, 10000)]);
    assert!(matches!(got[0], Answer::Holds) && matches!(got[1], Answer::Holds), "{got:?}");
    let got = answers(&[("shipped", 0, 100), ("asked", 5, 10000)]);
    let Answer::Fails(example) = &got[0] else { panic!("{got:?}") };
    assert_eq!(example, &[("shipped".to_string(), Value::Int(0)), ("asked".to_string(), Value::Int(5))]);
    assert!(matches!(got[1], Answer::Undecided(_)), "{got:?}");
    // A number the call says nothing of is taken over the rule's own range, as for a `constraint`.
    let got = answers(&[("asked", 0, 10000)]);
    let Answer::Fails(example) = &got[0] else { panic!("{got:?}") };
    assert_eq!(example, &[("shipped".to_string(), Value::Int(0)), ("asked".to_string(), Value::Int(1))]);
    assert!(matches!(got[1], Answer::Undecided(_)), "{got:?}");
}

/// `rulec fmt` leaves a line as it is written, as it leaves a `constraint`, and changes nothing the
/// second time.
#[test]
fn fmt_leaves_a_line_alone() {
    let t = TempDir::new("never-fmt");
    let src = OVERLAP.replace("never a = true, b = true", "never  a=true ,b =  true");
    let p = write(t.path(), "r.rule", &src);
    assert_eq!(run("en", &["fmt", &p]).0, 0);
    let once = std::fs::read_to_string(&p).unwrap();
    assert!(once.contains("never  a=true ,b =  true"), "{once}");
    assert_eq!(run("en", &["fmt", &p]).0, 0);
    assert_eq!(std::fs::read_to_string(&p).unwrap(), once);
    assert_eq!(codes(&diags(&once, Lang::En)), Vec::<&str>::new());
}

const THROUGH_FROM: &str = r#"
import importlib.util, json, sys
spec = importlib.util.spec_from_file_location("t", sys.argv[1])
m = importlib.util.module_from_spec(spec)
sys.modules["t"] = m
spec.loader.exec_module(m)
def line(fragile):
    return {"sku": "a", "quantity": 1, "amount_usd": 1, "fragile": fragile}
fees = set()
for lines in ([line(True)], [line(False)], [line(True), line(False)], [line(True), line(True)], [line(True), line(True), line(False)]):
    fees.add(m.fragile_lines_fee_from({"lines": lines}))
print("through from", sorted(fees))
try:
    print("by hand", m.fragile_lines_fee(True, False, 0))
except m.RuleInputError as e:
    print("refused", e)
"#;

/// A line the definitions show holds of every call through `from`, and a caller who works the values
/// out by hand is refused rather than sent to an unreachable branch.
#[test]
fn a_line_over_definitions_holds_through_from_and_at_the_door() {
    let have = || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success());
    if !ready(Need::Python, have, "python3 is not there") {
        return;
    }
    let src = text("fragile_lines_fee");
    let path = root().join(corpus("fragile_lines_fee")).to_string_lossy().into_owned();
    let code = i18n::with(Lang::En, || {
        let (f, c) = rulec::prepare(&src, &path).unwrap();
        rulec::codegen::Gen::new(&f, &c, &src, &path).python()
    });
    let t = TempDir::new("never-from");
    let module = write(t.path(), "t.py", &code);
    let script = write(t.path(), "s.py", THROUGH_FROM);
    let o = Command::new("python3").args(["-B", &script, &module]).output().unwrap();
    let said = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(o.status.success(), "{said}{}", String::from_utf8_lossy(&o.stderr));
    let lines: Vec<&str> = said.lines().collect();
    assert_eq!(lines, ["through from [0, 3, 5, 9]", "refused this combination does not happen: never has_fragile = true, fragile = 0"], "{said}");
}
