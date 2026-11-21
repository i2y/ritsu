//! `affected` follows what the files the links name read, through every level (DESIGN 8): a
//! change to a file that a linked file reads reaches the requirements that name something in the
//! linked file, though no byte of that file changes. The fixture `reads` is a refund desk in
//! English (`refunds.req`) with its Japanese version beside it (`refunds.ja.req`): a rule that
//! applies another rule, a dates file that uses a calendar made of a table, a gate that uses the
//! rule and the dates, a context whose term means the rule's output, an OpenAPI document whose
//! `$ref` reaches a second file, and a `.proto` that imports another. Each diff of
//! `reads/changes` changes one file. What no linked file reads reaches no requirement: code a
//! context only owns, the rule an input takes its value from (`fed by`), the copy a rule is held
//! to, a file nothing reads, and a table only a calendar no requirement names reads; a file of a
//! scope that a linked file reads is within reach. The fixture `contributions` is a rule and a
//! contribution to it; a spec's contract is geas's to answer, and yuen follows it to the
//! requirements of the spec.

mod common;

use common::{golden, run, run_with};
use ritsu_base::naming::{Name, Tool};
use ritsu_ports::{Read, Reference, References, Said};
use std::path::Path;
use std::rc::Rc;

const R: &str = "tests/fixtures/reads";
const C: &str = "tests/fixtures/contributions";

/// `affected` of the project `req` (`refunds.req` or `refunds.ja.req`) over `changes/<diff>`:
/// what it prints, held to the golden file `tests/golden/affected/<name>`, which must exit 0.
fn affected(req: &str, diff: &str, lang: &str, name: &str, failures: &mut Vec<String>) -> String {
    let r = run(&["affected", &format!("{R}/{req}"), "--root", R, "--diff", &format!("{R}/changes/{diff}"), "--lang", lang]);
    assert_eq!(r.code, 0, "{diff}: {}{}", r.stdout, r.stderr);
    golden(&format!("tests/golden/affected/{name}"), &r.stdout, failures);
    r.stdout
}

/// A rule the rule of a link applies: the change reaches that link's requirement by the `apply`
/// line, and, a level further, the requirements of the gate that uses the rule and of the context
/// whose term means its output. In English, then in Japanese.
#[test]
fn a_change_to_a_rule_a_rule_applies_reaches_the_rules_that_read_it() {
    let mut failures = Vec::new();
    let out = affected("refunds.req", "size.diff", "en", "reads-applied.txt", &mut failures);
    assert!(out.contains(&format!("files read by the files that requirements name, that the diff touches:\n  {R}/rules/size.rule\n")), "{out}");
    assert!(out.contains("    rulec \"rules/refund_band.rule\" line 12 (apply): met by bands_of_a_refund\n"), "{out}");
    assert!(out.contains("    sekisho \"gates/refunds.gate\" line 5 (use rule) -> rulec \"rules/refund_band.rule\" line 12 (apply): met by who_refunds\n"), "{out}");
    assert!(out.contains("    sakai \"contexts/desk.ctx\" line 9 (published language) -> rulec \"rules/refund_band.rule\" line 12 (apply): met by refund_words\n"), "{out}");
    assert!(out.ends_with("3 requirements touched; ask finance\n"), "{out}");
    let out = affected("refunds.ja.req", "size.ja.diff", "ja", "reads-applied.ja.txt", &mut failures);
    assert!(out.contains("    sekisho \"gates/refunds.ja.gate\" の 5 行目（use rule） -> rulec \"rules/refund_band.ja.rule\" の 12 行目（apply）: 満たす要件 返金できる人\n"), "{out}");
    assert!(out.ends_with("触る要件は 3 件です。経理 に見てもらってください\n"), "{out}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A file a link names is read by other linked files too: the change reaches the link's own
/// requirement, and the requirements of the gate and the context that read the file.
#[test]
fn a_change_to_a_linked_file_reaches_the_files_that_read_it() {
    let mut failures = Vec::new();
    let out = affected("refunds.req", "band.diff", "en", "reads-linked.txt", &mut failures);
    assert!(out.contains("files that requirements name, that the diff touches:\n  rulec \"rules/refund_band.rule\": met by bands_of_a_refund\n"), "{out}");
    assert!(out.contains(&format!("  {R}/rules/refund_band.rule\n    sekisho \"gates/refunds.gate\" line 5 (use rule): met by who_refunds\n    sakai \"contexts/desk.ctx\" line 9 (published language): met by refund_words\n")), "{out}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A calendar the dates use, and the table the calendar is made of: one level and two levels
/// down from the dates, two and three from the gate that uses them. In English and in Japanese,
/// and as JSON.
#[test]
fn a_change_to_a_calendar_and_to_its_table_reaches_the_dates_and_the_gate() {
    let mut failures = Vec::new();
    let out = affected("refunds.req", "office.diff", "en", "reads-calendar.txt", &mut failures);
    assert!(out.contains("    koyomi \"dates/refund_terms.cal\" line 3 (use calendar): met by refund_period\n"), "{out}");
    let out = affected("refunds.ja.req", "office.ja.diff", "ja", "reads-calendar.ja.txt", &mut failures);
    assert!(out.contains("    koyomi \"dates/refund_terms.ja.cal\" の 3 行目（use calendar）: 満たす要件 返金の期間\n"), "{out}");
    let out = affected("refunds.req", "closures.diff", "en", "reads-table.txt", &mut failures);
    assert!(
        out.contains("    sekisho \"gates/refunds.gate\" line 6 (use dates) -> koyomi \"dates/refund_terms.cal\" line 3 (use calendar) -> koyomi \"dates/office.cal\" line 5 (source): met by who_refunds\n"),
        "{out}"
    );
    assert!(out.contains("    koyomi \"dates/refund_terms.cal\" line 3 (use calendar) -> koyomi \"dates/office.cal\" line 5 (source): met by refund_period\n"), "{out}");
    affected("refunds.ja.req", "closures.diff", "ja", "reads-table.ja.txt", &mut failures);
    let j = run(&["affected", &format!("{R}/refunds.req"), "--root", R, "--diff", &format!("{R}/changes/closures.diff"), "--format", "json"]);
    assert_eq!(j.code, 0, "{}{}", j.stdout, j.stderr);
    golden("tests/golden/affected/reads-table.json", &j.stdout, &mut failures);
    let v: serde_json::Value = serde_json::from_str(&j.stdout).unwrap();
    assert_eq!(v["reads"][0]["file"], "data/closures.csv");
    let by = v["reads"][0]["read_by"].as_array().unwrap();
    assert_eq!(by.len(), 2, "{by:?}");
    assert_eq!(by[0]["artifact"]["path"], "gates/refunds.gate");
    assert_eq!(by[0]["way"].as_array().unwrap().len(), 3);
    assert_eq!(by[0]["way"][2], serde_json::json!({"reader": {"text": "koyomi \"dates/office.cal\"", "tool": "koyomi", "path": "dates/office.cal", "items": []}, "line": 5, "how": "source", "read": {"text": "file \"data/closures.csv\"", "tool": "file", "path": "data/closures.csv", "items": []}}));
    assert_eq!(by[0]["requirements"][0]["name"], "who_refunds");
    assert_eq!(by[1]["requirements"][0]["name"], "refund_period");
    assert_eq!(v["requirements"].as_array().unwrap().iter().map(|r| r["name"].as_str().unwrap()).collect::<Vec<_>>(), ["who_refunds", "refund_period"]);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The standard formats yuen reads itself: the second file of a document, which its `$ref`
/// reaches, and the file a `.proto` imports.
#[test]
fn a_change_to_what_a_document_refs_and_a_proto_imports_reaches_the_operation_and_the_service() {
    let mut failures = Vec::new();
    let out = affected("refunds.req", "schemas.diff", "en", "reads-ref.txt", &mut failures);
    assert!(out.contains(&format!("  {R}/api/schemas.yaml\n    openapi \"api/refunds.yaml\" line 13 ($ref): met by refund_api\n")), "{out}");
    assert!(out.ends_with("1 requirement touched; ask finance\n"), "{out}");
    let out = affected("refunds.req", "money.diff", "en", "reads-import.txt", &mut failures);
    assert!(out.contains(&format!("  {R}/proto/money.proto\n    proto \"proto/refunds.proto\" line 5 (import): met by refund_messages\n")), "{out}");
    let out = affected("refunds.ja.req", "money.diff", "ja", "reads-import.ja.txt", &mut failures);
    assert!(out.contains("    proto \"proto/refunds.proto\" の 5 行目（import）: 満たす要件 返金のメッセージ\n"), "{out}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// What no linked file reads: the code the context owns (where its code sits, not what its terms
/// mean), a file nothing reads, a table only a calendar no requirement names reads, that
/// calendar, and the map. No requirement is touched, in English or in Japanese.
#[test]
fn a_change_no_linked_file_reads_touches_no_requirement() {
    let mut failures = Vec::new();
    let out = affected("refunds.req", "unread.diff", "en", "reads-unread.txt", &mut failures);
    assert!(out.contains(&format!("other files the diff touches: {R}/app/desk.py, {R}/data/branch_closures.csv, {R}/dates/branch.cal, {R}/notes.txt, {R}/shop.ctx\nno requirement is touched\n")), "{out}");
    assert!(!out.contains("files read by"), "{out}");
    let out = affected("refunds.ja.req", "unread.diff", "ja", "reads-unread.ja.txt", &mut failures);
    assert!(out.ends_with("触る要件はありません\n"), "{out}");
    let j = run(&["affected", &format!("{R}/refunds.req"), "--root", R, "--diff", &format!("{R}/changes/unread.diff"), "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&j.stdout).unwrap();
    assert_eq!(v["reads"], serde_json::json!([]));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The rule an input of the band takes its value from (`fed by`, ritsu's DESIGN 7.16): the code
/// of the desk carries the value and `ritsu check` holds the ranges, and nothing the band answers
/// is made of that rule. Its change reaches no requirement, in English or in Japanese.
#[test]
fn a_change_to_where_a_value_comes_from_touches_no_requirement() {
    let mut failures = Vec::new();
    let out = affected("refunds.req", "fed.diff", "en", "reads-fed.txt", &mut failures);
    assert!(out.contains(&format!("other files the diff touches: {R}/rules/counter.rule\nno requirement is touched\n")), "{out}");
    let out = affected("refunds.ja.req", "fed.ja.diff", "ja", "reads-fed.ja.txt", &mut failures);
    assert!(out.contains(&format!("要件に関わらないファイル: {R}/rules/counter.ja.rule\n触る要件はありません\n")), "{out}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// koyomi's engine, but the calendar `dates/office.cal` reads the directory `data` whole: the
/// shape a language gives for a folder its files are made of, every file in it, which no line of
/// the file names one by one (an organization's folder of data).
struct ReadsTheFolder(koyomi::ports::Engine);

impl References for ReadsTheFolder {
    fn references(&self, root: &Path, file: &str) -> Result<Vec<Reference>, Vec<Said>> {
        self.0.references(root, file)
    }

    fn reads(&self, root: &Path, file: &str, named: Vec<Reference>) -> Vec<Read> {
        let mut out = self.0.reads(root, file, named);
        if file == "dates/office.cal" {
            out.push(Read { line: Some(5), target: Name::file(Tool::File, "data"), how: "data".into() });
        }
        out
    }
}

/// A directory a file reads stands for every file under it: a file of the folder that no line
/// names (`data/branch_closures.csv`, which only the unnamed calendar reads) reaches the dates
/// and the gate once the calendar reads the folder; a file beside the folder whose name starts
/// the same does not.
#[test]
fn a_directory_a_file_reads_reaches_every_file_under_it() {
    let t = common::TempDir::new("reads-folder");
    let d = t.path().join("reads");
    common::copy_dir(Path::new(R), &d);
    std::fs::write(d.join("database.txt"), "not in the folder\n").unwrap();
    let diff = t.path().join("folder.diff");
    std::fs::write(&diff, "--- a/data/branch_closures.csv\n+++ b/data/branch_closures.csv\n@@ -2 +2,2 @@\n 2027-08-13,Summer\n+2027-08-16,Summer\n--- a/database.txt\n+++ b/database.txt\n@@ -1 +1 @@\n-in no folder\n+not in the folder\n").unwrap();
    let (dir, diff) = (d.to_string_lossy().to_string(), diff.to_string_lossy().to_string());
    let args = ["affected", &format!("{dir}/refunds.req"), "--root", &dir, "--diff", &diff];
    let r = run_with(&args, common::suite_reading(Tool::Koyomi, Rc::new(ReadsTheFolder(koyomi::ports::Engine))));
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(
        r.stdout.contains(&format!("  {dir}/data/branch_closures.csv\n    sekisho \"gates/refunds.gate\" line 6 (use dates) -> koyomi \"dates/refund_terms.cal\" line 3 (use calendar) -> koyomi \"dates/office.cal\" line 5 (data): met by who_refunds\n")),
        "{}",
        r.stdout
    );
    assert!(r.stdout.contains(&format!("other files the diff touches: {dir}/database.txt\n")), "{}", r.stdout);
    assert!(r.stdout.ends_with("2 requirements touched; ask finance\n"), "{}", r.stdout);
    // without the folder, the same change reaches nothing
    let r = run(&args);
    assert!(r.stdout.contains(&format!("other files the diff touches: {dir}/data/branch_closures.csv, {dir}/database.txt\nno requirement is touched\n")), "{}", r.stdout);
}

/// A file of a scope that no requirement leads to (E404) is out of every requirement's reach,
/// unless a linked file reads it: then the change reaches the requirements of the link, and is not
/// counted among the changes no requirement reaches. The fixture with the scope `file "data"`: the
/// table the office's calendar is made of reaches the dates and the gate (exit 0), and the table
/// only the branch's calendar reads is out of reach (exit 1). In English, then in Japanese.
#[test]
fn a_file_of_a_scope_a_linked_file_reads_is_within_reach() {
    let t = common::TempDir::new("reads-scope");
    let d = t.path().join("reads");
    common::copy_dir(Path::new(R), &d);
    common::edit(&d, "refunds.req", "role finance \"Decides who refunds what\"\n", "role finance \"Decides who refunds what\"\n\nscope file \"data\"\n");
    common::edit(&d, "refunds.ja.req", "role 経理 \"誰が何を返金するかを決める\"\n", "role 経理 \"誰が何を返金するかを決める\"\n\nscope file \"data\"\n");
    let branch = t.path().join("branch.diff");
    std::fs::write(&branch, "--- a/data/branch_closures.csv\n+++ b/data/branch_closures.csv\n@@ -2 +2,2 @@\n 2027-08-13,Summer\n+2027-08-16,Summer\n").unwrap();
    let branch = branch.to_string_lossy().to_string();
    let dir = d.to_string_lossy().to_string();
    let closures = format!("{dir}/changes/closures.diff");

    let r = run(&["affected", &format!("{dir}/refunds.req"), "--root", &dir, "--diff", &closures]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains(&format!("  {dir}/data/closures.csv\n    sekisho \"gates/refunds.gate\" line 6 (use dates)")), "{}", r.stdout);
    assert!(r.stdout.contains("changes no requirement reaches: none\n"), "{}", r.stdout);
    assert!(r.stdout.ends_with("2 requirements touched; ask finance\n"), "{}", r.stdout);
    let r = run(&["affected", &format!("{dir}/refunds.req"), "--root", &dir, "--diff", &branch]);
    assert_eq!(r.code, 1, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains(&format!("changes no requirement reaches:\n  {dir}/data/branch_closures.csv (in the scope file \"data\", and no requirement leads to it)\n")), "{}", r.stdout);

    let r = run(&["affected", &format!("{dir}/refunds.ja.req"), "--root", &dir, "--diff", &closures, "--lang", "ja"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains(&format!("  {dir}/data/closures.csv\n    sekisho \"gates/refunds.ja.gate\" の 6 行目（use dates）")), "{}", r.stdout);
    assert!(r.stdout.contains("どの要件にも届かない変更: なし\n"), "{}", r.stdout);
    assert!(r.stdout.ends_with("触る要件は 2 件です。経理 に見てもらってください\n"), "{}", r.stdout);
    let r = run(&["affected", &format!("{dir}/refunds.ja.req"), "--root", &dir, "--diff", &branch, "--lang", "ja"]);
    assert_eq!(r.code, 1, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains(&format!("どの要件にも届かない変更:\n  {dir}/data/branch_closures.csv（範囲 file \"data\" にあり、どの要件からも辿れない）\n")), "{}", r.stdout);
}

/// The copy a rule is held to (`source … = file "…" sha256:…`) is not what the rule reads: the
/// rule's answers are its table, and a changed copy stops the rule's check (E038) until the pin is
/// written again, which is a change to the rule. A change to the copy alone is among the other
/// files, and touches no requirement through the rule, the gate that uses it or the context whose
/// words it gives. In English, then in Japanese.
#[test]
fn a_change_to_the_copy_a_rule_is_held_to_is_not_followed() {
    let t = common::TempDir::new("reads-pinned");
    let d = t.path().join("reads");
    common::copy_dir(Path::new(R), &d);
    std::fs::create_dir_all(d.join("rules/sources")).unwrap();
    std::fs::write(d.join("rules/sources/sizes.md"), "# Sizes of a refund\n\nA refund of up to 20 items is few, and a larger one is many.\n").unwrap();
    common::edit(&d, "rules/refund_band.rule", "\n\nenum band = small | large\n", "\n\nsource sizes = file \"sources/sizes.md\" sha256:fdb953de87a9cb15\n\nenum band = small | large\n");
    common::edit(&d, "rules/refund_band.ja.rule", "\n\nenum 区分(band) = 少数(small) | 多数(large)\n", "\n\nsource 大きさ(sizes) = file \"sources/sizes.md\" sha256:fdb953de87a9cb15\n\nenum 区分(band) = 少数(small) | 多数(large)\n");
    let diff = t.path().join("sizes.diff");
    std::fs::write(&diff, "--- a/rules/sources/sizes.md\n+++ b/rules/sources/sizes.md\n@@ -3 +3 @@\n-A refund of up to 20 items is few, and a larger one is many.\n+A refund of up to 25 items is few, and a larger one is many.\n").unwrap();
    let diff = diff.to_string_lossy().to_string();
    let dir = d.to_string_lossy().to_string();

    let r = run(&["affected", &format!("{dir}/refunds.req"), "--root", &dir, "--diff", &diff]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(!r.stdout.contains("files read by"), "{}", r.stdout);
    assert!(r.stdout.contains(&format!("other files the diff touches: {dir}/rules/sources/sizes.md\n")), "{}", r.stdout);
    assert!(r.stdout.ends_with("no requirement is touched\n"), "{}", r.stdout);

    let r = run(&["affected", &format!("{dir}/refunds.ja.req"), "--root", &dir, "--diff", &diff, "--lang", "ja"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(!r.stdout.contains("読むファイル"), "{}", r.stdout);
    assert!(r.stdout.contains(&format!("要件に関わらないファイル: {dir}/rules/sources/sizes.md\n")), "{}", r.stdout);
    assert!(r.stdout.ends_with("触る要件はありません\n"), "{}", r.stdout);
}

/// The rule files of the fixture `contributions`, as `ritsu check` gives them to rulec.
fn rule_files() -> Vec<String> {
    ["rules/fee.rule", "rules/fee.ja.rule", "rules/extra/courier.rule", "rules/extra/courier.ja.rule"].iter().map(|f| format!("{C}/{f}")).collect()
}

/// A rule reads the contributions merged into it, which it does not name: in a project rulec is
/// given (as `ritsu check` and `ritsu yuen affected` give it), the change to a contribution
/// reaches the requirement of the rule it adds to. An engine given no project merges none, and
/// the change reaches only the requirement that names the contribution.
#[test]
fn a_change_to_a_contribution_reaches_the_rule_it_adds_to() {
    let mut failures = Vec::new();
    let args = ["affected", &format!("{C}/fees.req"), "--root", C, "--diff", &format!("{C}/changes/courier.diff")];
    let r = run_with(&args, common::suite_of_project(rule_files()));
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    golden("tests/golden/affected/reads-contribution.txt", &r.stdout, &mut failures);
    assert!(r.stdout.contains(&format!("  {C}/rules/extra/courier.rule\n    rulec \"rules/fee.rule\" (contribution): met by fee_by_way\n")), "{}", r.stdout);
    assert!(r.stdout.ends_with("2 requirements touched; ask finance\n"), "{}", r.stdout);
    let ja = ["affected", &format!("{C}/fees.ja.req"), "--root", C, "--diff", &format!("{C}/changes/courier.ja.diff"), "--lang", "ja"];
    let r = run_with(&ja, common::suite_of_project(rule_files()));
    golden("tests/golden/affected/reads-contribution.ja.txt", &r.stdout, &mut failures);
    assert!(r.stdout.contains("    rulec \"rules/fee.ja.rule\"（contribution）: 満たす要件 戻し方ごとの手数料\n"), "{}", r.stdout);
    let r = run(&args);
    assert!(!r.stdout.contains("files read by"), "{}", r.stdout);
    assert!(r.stdout.ends_with("1 requirement touched; ask finance\n"), "{}", r.stdout);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A contribution reads the rule it adds to (its `to` line), with or without a project: the
/// change to the rule reaches the requirement that names the contribution.
#[test]
fn a_change_to_a_rule_reaches_the_contributions_to_it() {
    let mut failures = Vec::new();
    let r = run(&["affected", &format!("{C}/fees.req"), "--root", C, "--diff", &format!("{C}/changes/fee.diff")]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    golden("tests/golden/affected/reads-contribution-to.txt", &r.stdout, &mut failures);
    assert!(r.stdout.contains(&format!("  {C}/rules/fee.rule\n    rulec \"rules/extra/courier.rule\" line 3 (to): met by courier_fee\n")), "{}", r.stdout);
    let r = run(&["affected", &format!("{C}/fees.ja.req"), "--root", C, "--diff", &format!("{C}/changes/fee.ja.diff"), "--lang", "ja"]);
    assert!(r.stdout.contains(&format!("  {C}/rules/fee.ja.rule\n    rulec \"rules/extra/courier.ja.rule\" の 3 行目（to）: 満たす要件 集荷の手数料\n")), "{}", r.stdout);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A spec's contract, and the file its `$ref` reaches, are what the claims are held to: geas says
/// the diff changes them (`contract_changed`), and yuen follows that to every requirement that
/// names the spec or a claim of it. Before, the file was one the diff touches outside the
/// requirements.
#[test]
fn a_change_to_a_specs_contract_reaches_the_requirements_of_the_spec() {
    let t = common::fixture("geas");
    let g = t.path().join("geas");
    common::edit(&g, "greeter/greeter.geas", "  port auto\n", "  port auto\n  contract \"api.yaml\"\n");
    std::fs::write(
        g.join("greeter/api.yaml"),
        "openapi: 3.1.0\ninfo: { title: Greeter, version: \"1\" }\npaths:\n  /greet:\n    get:\n      responses:\n        \"200\":\n          description: A greeting\n          content:\n            application/json:\n              schema: { $ref: \"./schemas.yaml#/Greeting\" }\n",
    )
    .unwrap();
    std::fs::write(g.join("greeter/schemas.yaml"), "Greeting:\n  type: object\n  properties:\n    message: { type: string }\n  required: [message]\n").unwrap();
    std::fs::write(g.join("contract.diff"), "diff --git a/greeter/schemas.yaml b/greeter/schemas.yaml\n--- a/greeter/schemas.yaml\n+++ b/greeter/schemas.yaml\n@@ -4 +4,2 @@\n     message: { type: string }\n+  required: [message]\n").unwrap();
    let (dir, diff) = (g.to_string_lossy().to_string(), g.join("contract.diff").to_string_lossy().to_string());
    let r = run(&["affected", &format!("{dir}/greeter.req"), "--root", &dir, "--diff", &diff]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains(&format!("  the spec's contract {dir}/greeter/schemas.yaml changes: checked by greets_by_name, rejects_an_empty_name, totals_accumulate, unknown_paths_are_404\n")), "{}", r.stdout);
    assert!(!r.stdout.contains("other files the diff touches"), "{}", r.stdout);
    assert!(r.stdout.ends_with("4 requirements touched; ask api\n"), "{}", r.stdout);
    let r = run(&["affected", &format!("{dir}/greeter.req"), "--root", &dir, "--diff", &diff, "--lang", "ja"]);
    assert!(r.stdout.contains(&format!("  spec の契約 {dir}/greeter/schemas.yaml が変わります（確かめる要件 greets_by_name、rejects_an_empty_name、totals_accumulate、unknown_paths_are_404）\n")), "{}", r.stdout);
    let r = run(&["affected", &format!("{dir}/greeter.req"), "--root", &dir, "--diff", &diff, "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["specs"][0]["contract_changed"][0]["file"], "greeter/schemas.yaml");
    assert_eq!(v["specs"][0]["contract_changed"][0]["requirements"].as_array().unwrap().len(), 4);
    assert_eq!(v["others"], serde_json::json!([]));
}
