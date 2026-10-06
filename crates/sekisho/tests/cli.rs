//! The command (DESIGN 11): what `sekisho` prints and exits with, as `ritsu sekisho` runs it (every
//! language joined) and as the binary of sekisho's own crate runs it (none joined: E209, exit 2).
//! The pages of `--help` are golden files in `tests/golden/cli/`.

mod common;

use sekisho::suite::Suite;

/// The command run as a function: the exit code, what it printed, and what it printed on stderr.
fn run(args: &[&str], suite: Suite) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = sekisho::run::run(&args, suite, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

#[test]
fn version_and_help() {
    let (code, out, _) = run(&["--version"], Suite::default());
    assert_eq!((code, out.as_str()), (0, format!("sekisho {}\n", env!("CARGO_PKG_VERSION")).as_str()));
    for (lang, tag) in [("en", "en"), ("ja", "ja")] {
        let (code, out, _) = run(&["--help", "--lang", lang], Suite::default());
        assert_eq!(code, 0);
        ritsu_testkit::golden(format!("tests/golden/cli/help.{tag}.txt"), &out.replace(env!("CARGO_PKG_VERSION"), "<version>"));
        for cmd in ["check", "gen", "vectors", "api", "explain"] {
            let (code, out, _) = run(&[cmd, "--help", "--lang", lang], Suite::default());
            assert_eq!(code, 0);
            ritsu_testkit::golden(format!("tests/golden/cli/{cmd}.{tag}.txt"), &out);
            let (code, again, _) = run(&["help", cmd, "--lang", lang], Suite::default());
            assert_eq!((code, again), (0, out));
        }
    }
}

#[test]
fn check_exits_by_what_it_finds() {
    // every language joined: the example passes
    let (code, out, _) = run(&["check", "examples/refunds/refunds.gate", "examples/refunds/refunds.ja.gate"], common::joined());
    assert_eq!(code, 0, "{out}");
    assert_eq!(out.lines().filter(|l| l.contains(": ok — ")).count(), 2, "{out}");
    // an error
    let (code, out, _) = run(&["check", "tests/mutants/E101_unknown_role.gate"], common::joined());
    assert_eq!(code, 1, "{out}");
    // a language not joined: E209, exit 2
    let (code, out, _) = run(&["check", "examples/refunds/refunds.gate"], Suite::default());
    assert_eq!(code, 2);
    assert!(out.contains("error[E209]") && out.contains("ritsu sekisho check examples/refunds/refunds.gate"), "{out}");
    // a file that is not there, a budget that is no positive number, a flag it does not take
    for args in [&["check", "tests/no-such.gate"][..], &["check", "--budget", "0", "examples/refunds/refunds.gate"], &["check", "--fast", "x.gate"], &["frobnicate"], &["check"]] {
        let (code, _, err) = run(args, common::joined());
        assert_eq!(code, 2, "{args:?}");
        assert!(err.starts_with("error: "), "{args:?}: {err}");
    }
}

#[test]
fn check_in_json() {
    let (code, out, _) = run(&["check", "tests/mutants/E101_unknown_role.gate", "--format", "json"], common::joined());
    assert_eq!(code, 1);
    let v = ritsu_base::json::parse(&out).unwrap();
    let keys: Vec<&str> = v.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, vec!["file", "ok", "summary", "diagnostics"]);
    assert_eq!(v.get("diagnostics").and_then(|d| d.as_arr()).map(|d| d.len()), Some(1));
    let (code, out, _) = run(&["check", "examples/refunds/refunds.gate", "--format", "json", "--lang", "ja"], common::joined());
    assert_eq!(code, 0);
    let v = ritsu_base::json::parse(&out).unwrap();
    assert_eq!(v.get("summary").and_then(|s| s.as_str()), Some("action 3、ポリシー 10（permit 7、forbid 3）、期待 3、職務の分離 1"));
}

#[test]
fn explain() {
    let (code, out, _) = run(&["explain", "e101"], Suite::default());
    assert_eq!(code, 0);
    assert!(out.starts_with("E101 (error) — A name names nothing\n"), "{out}");
    let (code, out, _) = run(&["explain", "E101", "--lang", "ja"], Suite::default());
    assert_eq!(code, 0);
    assert!(out.contains("principal in 系"), "the Japanese example: {out}");
    let (code, out, _) = run(&["explain", "--all", "--format", "markdown"], Suite::default());
    assert_eq!((code, out), (0, std::fs::read_to_string("docs/codes.md").unwrap()));
    let (code, out, _) = run(&["explain", "--all", "--format", "markdown", "--lang", "ja"], Suite::default());
    assert_eq!((code, out), (0, std::fs::read_to_string("docs/codes.ja.md").unwrap()));
    let (code, out, _) = run(&["explain", "E101", "--format", "json"], Suite::default());
    assert_eq!(code, 0);
    assert_eq!(ritsu_base::json::parse(&out).unwrap().get("code").and_then(|c| c.as_str()), Some("E101"));
    let (code, _, err) = run(&["explain", "E999"], Suite::default());
    assert_eq!(code, 2);
    assert!(err.contains("E999"));
}

/// The binary itself: no other language is in it.
#[test]
fn the_binary_of_the_crate() {
    let bin = env!("CARGO_BIN_EXE_sekisho");
    let out = std::process::Command::new(bin).args(["check", "examples/refunds/refunds.gate"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("error[E209]"), "{text}");
    let out = std::process::Command::new(bin).args(["check", "tests/mutants/E101_unknown_role.gate"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let out = std::process::Command::new(bin).arg("--version").output().unwrap();
    assert_eq!((out.status.code(), String::from_utf8_lossy(&out.stdout).to_string()), (Some(0), format!("sekisho {}\n", env!("CARGO_PKG_VERSION"))));
}

/// What `ritsu check` prints for the files of sekisho (`check::checked`): what `sekisho check`
/// prints, a finding at a time, and a verdict for each file.
#[test]
fn checked_as_ritsu_check_prints_it() {
    let root = std::env::current_dir().unwrap();
    let files = vec!["examples/refunds/refunds.gate".to_string(), "tests/mutants/E101_unknown_role.gate".to_string(), "tests/no-such.gate".to_string()];
    let units = sekisho::check::checked(&root, &files, &common::joined(), ritsu_base::text::Lang::En);
    let verdicts: Vec<ritsu_ports::Verdict> = units.iter().map(|u| u.verdict).collect();
    assert_eq!(verdicts, vec![ritsu_ports::Verdict::Passes, ritsu_ports::Verdict::Fails, ritsu_ports::Verdict::Unchecked]);
    let (_, out, _) = run(&["check", "tests/mutants/E101_unknown_role.gate"], common::joined());
    assert_eq!(units[1].text(), out);
    let f = units[1].findings().next().unwrap();
    assert_eq!((f.code.as_str(), f.file.as_deref()), ("E101", Some("tests/mutants/E101_unknown_role.gate")));
    // a language not joined: not checked, as the command exits 2
    let units = sekisho::check::checked(&root, &files[..1], &Suite::default(), ritsu_base::text::Lang::En);
    assert_eq!(units[0].verdict, ritsu_ports::Verdict::Unchecked);
}

/// `gen --target cedar` (DESIGN 5): the four files of each gate under `<out>/cedar/`, named by its
/// alias; `--check` says which differs; nothing from a file that does not pass, nor from one that
/// reads a language the run does not join (E209).
#[test]
fn gen_writes_the_cedar_of_each_gate() {
    let t = ritsu_testkit::TempDir::new("sekisho-gen");
    let out = t.path().to_string_lossy().to_string();
    let (code, said, _) = run(&["gen", "examples/refunds/refunds.gate", "examples/refunds/refunds.ja.gate", "--target", "cedar", "--out", &out], common::joined());
    assert_eq!(code, 0, "{said}");
    let files = ["refunds.cedar", "refunds.cedarschema", "refunds.cedarschema.json", "refunds.policies.json", "refunds_ja.cedar", "refunds_ja.cedarschema", "refunds_ja.cedarschema.json", "refunds_ja.policies.json"];
    for f in files {
        assert!(t.exists(&format!("cedar/{f}")), "{f}: {said}");
        assert!(said.contains(&format!("generated: {out}/cedar/{f}\n")), "{said}");
    }
    let head = format!("// Code generated by sekisho {}. DO NOT EDIT.\n// Source: refunds.gate (gate refunds v1, sha256:", env!("CARGO_PKG_VERSION"));
    assert!(t.read("cedar/refunds.cedar").starts_with(&head));
    assert!(t.read("cedar/refunds.cedarschema").starts_with(&head));
    assert!(t.read("cedar/refunds.policies.json").starts_with("{\"templates\":{},\"staticPolicies\":{\"refunds/staff_view_orders\":"));
    // the same files again: nothing to say
    let (code, said, _) = run(&["gen", "examples/refunds/refunds.gate", "examples/refunds/refunds.ja.gate", "--target", "cedar", "--out", &out, "--check"], common::joined());
    assert_eq!((code, said.as_str()), (0, ""));
    // a file changed and a file gone
    t.write("cedar/refunds.cedar", "// changed\n");
    std::fs::remove_file(t.path().join("cedar/refunds_ja.policies.json")).unwrap();
    let (code, said, _) = run(&["gen", "examples/refunds/refunds.gate", "examples/refunds/refunds.ja.gate", "--target", "cedar", "--out", &out, "--check"], common::joined());
    assert_eq!(code, 1);
    assert_eq!(said, format!("differs from what gen writes: {out}/cedar/refunds.cedar\nmissing: {out}/cedar/refunds_ja.policies.json\n"));
    // what sekisho writes in the files is in the language of --lang: the head, and the doc of a
    // computed value
    let (code, said, _) = run(&["gen", "examples/refunds/refunds.gate", "--target", "cedar", "--out", &out, "--lang", "ja"], common::joined());
    assert_eq!(code, 0);
    assert!(said.starts_with(&format!("生成しました: {out}/cedar/refunds.cedar\n")), "{said}");
    let schema = t.read("cedar/refunds.cedarschema");
    assert!(schema.contains("\n// もと: refunds.gate（gate refunds v1、sha256:"), "{schema}");
    assert!(schema.contains("生成したコードが計算し、呼ぶ側からは受け取らない"), "{schema}");
    // a file that does not pass: its diagnostics, and nothing written
    let (code, said, err) = run(&["gen", "tests/mutants/E101_unknown_role.gate", "--target", "cedar", "--out", &out], common::joined());
    assert_eq!(code, 1);
    assert!(said.starts_with("error[E101]: "), "{said}");
    assert_eq!(err, "error: `tests/mutants/E101_unknown_role.gate` does not pass check, so nothing is generated from it\n");
    assert!(!t.exists("cedar/shop.cedar"));
    // no language joined: E209, and the command to run instead
    let (code, said, _) = run(&["gen", "examples/refunds/refunds.gate", "--target", "cedar", "--out", &out], Suite::default());
    assert_eq!(code, 2);
    assert!(said.contains("error[E209]") && said.contains(&format!("ritsu sekisho gen examples/refunds/refunds.gate --target cedar --out {out}")), "{said}");
    // no target, a target it does not know
    for args in [&["gen", "examples/refunds/refunds.gate"][..], &["gen", "examples/refunds/refunds.gate", "--target", "typescript"]] {
        let (code, _, err) = run(args, common::joined());
        assert_eq!(code, 2, "{args:?}");
        assert!(err.starts_with("error: "), "{args:?}: {err}");
    }
}

/// `vectors` (DESIGN 6.1): every combination the check walks, as a test of `cedar run-tests`, a
/// combination whose number is a cell of two ends once at each end; the decision and the policies
/// that decide it are the check's.
#[test]
fn vectors_are_every_combination_at_both_ends() {
    let (code, out, _) = run(&["vectors", "examples/refunds/refunds.gate"], common::joined());
    assert_eq!(code, 0);
    let v = ritsu_base::json::parse(&out).unwrap();
    let tests = v.as_arr().unwrap();
    // 1,078 combinations: view_order's 18 and export_refunds' 4 once, refund_order's 1,056 at both
    // ends of the cell of amount (1 and 50, or 51 and 10,000)
    assert_eq!(tests.len(), 18 + 1_056 * 2 + 4);
    let name = |t: &ritsu_base::json::Json| t.get("name").and_then(|n| n.as_str()).unwrap().to_string();
    assert_eq!(name(&tests[0]), "view_order 1");
    assert_eq!((name(&tests[18]), name(&tests[19])), ("refund_order 1 (amount 1)".to_string(), "refund_order 1 (amount 50)".to_string()));
    let keys: Vec<&str> = tests[18].as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, vec!["name", "request", "entities", "decision", "reason", "num_errors"]);
    // the combinations allowed (tests/walk/golden/refunds.txt): view_order's 7 and 1, refund_order's
    // 36 + 48 + 24 + 4 less the 24 that clerks_refund_within_their_limit and
    // managers_refund_in_period both allow (at both ends), export_refunds' 1
    let allowed = tests.iter().filter(|t| t.get("decision").and_then(|d| d.as_str()) == Some("allow")).count();
    assert_eq!(allowed, 7 + 1 + (36 + 48 + 24 + 4 - 24) * 2 + 1);
    // each policy that decides is named by its @id
    let reasons: std::collections::BTreeSet<String> = tests.iter().flat_map(|t| t.get("reason").and_then(|r| r.as_arr()).unwrap().iter().map(|x| x.as_str().unwrap().to_string())).collect();
    assert_eq!(reasons.len(), 10);
    assert!(reasons.iter().all(|r| r.starts_with("refunds/")), "{reasons:?}");
    // the Japanese version: the same tests, but for the namespace and the head of the ids
    let (code, ja, _) = run(&["vectors", "examples/refunds/refunds.ja.gate"], common::joined());
    assert_eq!(code, 0);
    assert_eq!(ja.replace("ShopJa::", "Shop::").replace("\"refunds_ja/", "\"refunds/"), out);
    // one action
    let (code, one, _) = run(&["vectors", "examples/refunds/refunds.gate", "--action", "export_refunds"], common::joined());
    assert_eq!(code, 0);
    assert_eq!(ritsu_base::json::parse(&one).unwrap().as_arr().unwrap().len(), 4);
    let (code, one, _) = run(&["vectors", "examples/refunds/refunds.ja.gate", "--action", "返金を書き出す"], common::joined());
    assert_eq!((code, ritsu_base::json::parse(&one).unwrap().as_arr().unwrap().len()), (0, 4));
    // what it does not take
    let (code, _, err) = run(&["vectors", "examples/refunds/refunds.gate", "--action", "delete_order"], common::joined());
    assert_eq!(code, 2);
    assert_eq!(err, "error: `examples/refunds/refunds.gate` has no action `delete_order` (`view_order`, `refund_order`, `export_refunds`)\n");
    let (code, _, err) = run(&["vectors", "tests/mutants/E101_unknown_role.gate"], common::joined());
    assert_eq!((code, err.as_str()), (1, "error: `tests/mutants/E101_unknown_role.gate` does not pass check, so it has no vectors\n"));
    let (code, said, _) = run(&["vectors", "examples/refunds/refunds.gate"], Suite::default());
    assert!(code == 2 && said.contains("ritsu sekisho vectors examples/refunds/refunds.gate"), "{said}");
    let (code, _, _) = run(&["vectors", "examples/refunds/refunds.gate", "examples/refunds/refunds.ja.gate"], common::joined());
    assert_eq!(code, 2);
}

/// The vectors of a gate that relates entities (`tests/walk/relations/tenants.gate`): each way the
/// terms are one entity or not is an id, the principal's own its id, and a group it is a member
/// of a parent. The golden is `tests/golden/vectors/tenants.json`.
#[test]
fn vectors_of_relations() {
    let (code, out, _) = run(&["vectors", "tests/walk/relations/tenants.gate"], common::joined());
    assert_eq!(code, 0);
    ritsu_testkit::golden("tests/golden/vectors/tenants.json", &out);
}

/// `api` (DESIGN 11): what the gate declares, as JSON — the files gen writes, each role, type and
/// workflow with its entity in Cedar, each action with the operations it guards (each as the JSON
/// of its reference, its path from the root) and its context, each policy with its `@id` and the
/// file it is written in. The goldens are `tests/golden/api/`, with ritsu's version written
/// `<version>`; the example is run with its directory as the root.
#[test]
fn api_says_what_the_gate_declares() {
    for (path, golden) in [("examples/refunds/refunds.gate", "refunds.json"), ("examples/refunds/refunds.ja.gate", "refunds.ja.json"), ("tests/gen/papers.gate", "papers.json")] {
        let root = common::root_of(path).unwrap();
        let (code, out, _) = run(&["api", path, "--root", root.to_str().unwrap()], common::joined());
        assert_eq!(code, 0, "{path}");
        let v = ritsu_base::json::parse(&out).unwrap();
        let keys: Vec<&str> = v.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec!["sekisho", "name", "alias", "version", "source_sha256", "description", "namespace", "uses", "today", "cedar", "roles", "types", "workflows", "actions", "policies", "expects", "separations"]);
        let shown = out.replacen(&format!("\"sekisho\": \"{}\"", env!("CARGO_PKG_VERSION")), "\"sekisho\": \"<version>\"", 1);
        ritsu_testkit::golden(format!("tests/golden/api/{golden}"), &shown);
    }
    let (code, _, err) = run(&["api", "tests/mutants/E101_unknown_role.gate"], common::joined());
    assert_eq!((code, err.as_str()), (1, "error: `tests/mutants/E101_unknown_role.gate` does not pass check, so it has no api\n"));
    let (code, said, _) = run(&["api", "examples/refunds/refunds.gate"], Suite::default());
    assert!(code == 2 && said.contains("ritsu sekisho api examples/refunds/refunds.gate"), "{said}");
    let (code, _, _) = run(&["api"], common::joined());
    assert_eq!(code, 2);
}

/// The root the references are written from (DESIGN 2.6): `--root`, else the nearest directory
/// above the first path given that holds `.git`. An operation an action guards is named with its
/// document's path from the root, by its `operationId` however the `guards` line writes it; a
/// document outside the root has no reference (E201), and a root that is no directory stops the
/// command (exit 2).
#[test]
fn the_root_the_references_are_written_from() {
    let guards_of = |out: &str| -> Vec<String> {
        let v = ritsu_base::json::parse(out).unwrap();
        let a = &v.get("actions").and_then(|a| a.as_arr()).unwrap()[0];
        a.get("guards").and_then(|g| g.as_arr()).unwrap().iter().map(|g| g.get("reference").and_then(|r| r.get("text")).and_then(|t| t.as_str()).unwrap().to_string()).collect()
    };
    // the crate's directory as the root: the example's document from there, and the operation
    // written by its method and path named by its operationId
    let (code, out, _) = run(&["api", "tests/gen/guards.gate", "--root", "."], common::joined());
    assert_eq!(code, 0, "{out}");
    assert_eq!(guards_of(&out), ["openapi \"examples/refunds/api/orders.json\" operation getOrder", "openapi \"examples/refunds/api/orders.json\" operation refundOrder"]);
    // no --root: the nearest directory above that holds .git, the repository's
    let (code, out, _) = run(&["api", "tests/gen/guards.gate"], common::joined());
    assert_eq!(code, 0, "{out}");
    let at = ritsu_base::paths::from_root(&ritsu_base::paths::find_root(std::path::Path::new(".")), std::path::Path::new("examples/refunds/api/orders.json")).unwrap();
    assert_eq!(guards_of(&out)[0], format!("openapi \"{at}\" operation getOrder"));
    // a root the document is outside of
    let (code, out, err) = run(&["check", "tests/gen/guards.gate", "--root", "tests/gen"], common::joined());
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.starts_with("error[E201]: tests/gen/guards.gate:5:1: `../../examples/refunds/api/orders.json` is outside the root\n"), "{out}");
    let (code, out, err) = run(&["gen", "tests/gen/guards.gate", "--target", "cedar", "--root", "tests/gen", "--check"], common::joined());
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("error[E201]") && err.contains("does not pass check"), "{out}{err}");
    // a root that is no directory
    for args in [&["check", "tests/gen/guards.gate", "--root", "tests/no-such-dir"][..], &["api", "tests/gen/guards.gate", "--root", "tests/gen/guards.gate"], &["vectors", "tests/gen/guards.gate", "--root", "tests/no-such-dir"], &["gen", "tests/gen/guards.gate", "--target", "cedar", "--root", "tests/no-such-dir"]] {
        let (code, out, err) = run(args, common::joined());
        assert_eq!((code, out.as_str()), (2, ""), "{args:?}");
        assert!(err.starts_with("error: `--root tests/") && err.ends_with("` is not a directory\n"), "{args:?}: {err}");
    }
}
