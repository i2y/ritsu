//! The dandori binary of this crate holds no rulec (ritsu's DESIGN 2.3): a workflow that uses no rule
//! runs as it always has, and one that uses a rule is told to run with `ritsu dandori`, which reads
//! the rules in the same process. The command is `dandori::cli::run`, whatever port it is handed;
//! the tests hand it rulec's own (ritsu's DESIGN 3.3), the binary one that reads no rule.

use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The binary, run in the crate's directory.
fn binary(args: &[&str]) -> (u8, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_dandori")).current_dir(root()).args(args).output().expect("could not run dandori");
    (o.status.code().unwrap_or(-1) as u8, String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The command with rulec's port, as `ritsu dandori` runs it, in the crate's directory (where the
/// tests run).
fn with_rulec(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = dandori::cli::run(&args, Rc::new(rulec::ports::Engine::new()), &mut out, &mut err);
    (code, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned())
}

/// The files under a directory, by their paths from it.
fn written(dir: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push((p.strip_prefix(dir).unwrap().display().to_string(), std::fs::read_to_string(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

/// A flow that uses no rule never asks the port: the binary prints, writes and exits as the command
/// with rulec's port does, for each command.
#[test]
fn a_flow_without_rules_runs_as_it_does_with_rulec() {
    let tmp = TempDir::new("cli");
    let mut compared = 0;
    for flow in ["examples/fulfillment/arrange_delivery.flow", "examples/fulfillment/arrange_delivery.ja.flow", "tests/flows/timeouts.flow", "tests/flows/connect.flow", "tests/flows/service.flow"] {
        assert!(!std::fs::read_to_string(root().join(flow)).unwrap().contains("use rule"), "{flow} uses a rule");
        for (k, args) in [
            vec!["check", flow],
            vec!["check", flow, "--lang", "ja"],
            vec!["check", flow, "--format", "json"],
            vec!["scenarios", flow],
            vec!["api", flow],
            vec!["api", flow, "--lang", "ja"],
            vec!["doc", flow],
            vec!["doc", flow, "--format", "html", "--lang", "ja"],
            vec!["gen", flow, "--target", "temporal-typescript", "--out", "@"],
            vec!["gen", flow, "--target", "asl", "--out", "@"],
        ]
        .into_iter()
        .enumerate()
        {
            let (a, b) = (tmp.path().join(format!("{compared}-{k}-binary")), tmp.path().join(format!("{compared}-{k}-rulec")));
            let place = |dir: &Path| args.iter().map(|x| if *x == "@" { dir.to_str().unwrap().to_string() } else { x.to_string() }).collect::<Vec<_>>();
            let (pa, pb) = (place(&a), place(&b));
            let (ca, oa, ea) = binary(&pa.iter().map(String::as_str).collect::<Vec<_>>());
            let (cb, ob, eb) = with_rulec(&pb.iter().map(String::as_str).collect::<Vec<_>>());
            let shown = |s: &str, dir: &Path| s.replace(dir.to_str().unwrap(), "@");
            assert_eq!((ca, shown(&oa, &a), shown(&ea, &a)), (cb, shown(&ob, &b), shown(&eb, &b)), "{args:?}");
            assert_eq!(written(&a), written(&b), "{args:?}: what is written");
        }
        compared += 1;
    }
    assert_eq!(compared, 5);
}

/// A flow that uses a rule is told once, at its first `use rule`, that this binary reads no rule
/// and the same command to run through `ritsu dandori` (E018, in either language), with exit 2;
/// nothing that follows from the rules it cannot read is said. With rulec's port the same flow
/// passes (ritsu's DESIGN 2.3).
#[test]
fn a_flow_with_rules_is_told_to_run_with_ritsu_dandori() {
    let flow = "examples/hotel/temporal/hotel.flow";
    let (code, out, err) = binary(&["check", flow]);
    assert_eq!((code, out.as_str()), (2, ""), "{err}");
    assert_eq!(
        err,
        format!("error[E018]: {flow}:4:10: this dandori cannot read rules, and the flow uses the rule `hold`\n     4 | use rule hold from \"../rules/hold_amount.rule\"\n  = The binary of dandori's own crate holds no other language; run it with every language joined, through ritsu: `ritsu dandori check {flow}`.\n")
    );
    let (code, _, ja) = binary(&["check", flow, "--lang", "ja"]);
    assert_eq!(code, 2);
    assert!(ja.contains("  = dandori 単独のバイナリには、ほかの言語が入っていません。同じコマンドを、すべての言語をつないだ ritsu で、`ritsu dandori check examples/hotel/temporal/hotel.flow --lang ja` のように走らせてください。"), "{ja}");
    let (code, json, _) = binary(&["gen", flow, "--target", "temporal-typescript", "--format", "json"]);
    assert_eq!(code, 2, "{json}");
    assert_eq!(with_rulec(&["check", flow]), (0, String::new(), format!("{flow}: ok\n")));
}

/// So is a flow that uses a dates file or a book: once, at the first of them this binary cannot
/// read, with exit 2. rulec's port alone does not read them either; with koyomi's and chobo's
/// joined too, the flow passes.
#[test]
fn a_flow_with_dates_and_books_is_told_to_run_with_ritsu_dandori() {
    let flow = "examples/invoice/invoice.flow";
    let (code, out, err) = binary(&["check", flow]);
    assert_eq!((code, out.as_str()), (2, ""), "{err}");
    assert_eq!(
        err,
        format!("error[E018]: {flow}:10:11: this dandori cannot read dates files, and the flow uses the dates file `terms`\n    10 | use dates terms from \"dates/payment_terms.cal\"\n  = The binary of dandori's own crate holds no other language; run it with every language joined, through ritsu: `ritsu dandori check {flow}`.\n")
    );
    let (code, _, ja) = binary(&["check", flow, "--lang", "ja"]);
    assert_eq!(code, 2);
    assert!(ja.contains("エラー[E018]: examples/invoice/invoice.flow:10:11: この dandori は日付のファイルを読めません。このフローは日付のファイル `terms` を使います"), "{ja}");
    // with rulec alone, the dates file is still not read
    let (code, _, err) = with_rulec(&["check", flow]);
    assert_eq!(code, 2, "{err}");
    // a book alone
    let dir = ritsu_testkit::TempDir::new("cli-book");
    let book = std::fs::read_to_string(root().join("examples/invoice/books/stock.book")).unwrap();
    std::fs::write(dir.path().join("stock.book"), book).unwrap();
    let only_book = dir.path().join("receive.flow");
    std::fs::write(&only_book, "workflow receive v1\n\nuse book stock from \"stock.book\"\n  lambda \"arn:aws:lambda:eu-west-2:123456789012:function:stock\"\n\ninputs\n  delivery : string\n  sku      : string\n\ntask receive(delivery: string, sku: string, qty: int)\n  book stock.receive.do\n\nflow\n  receive(delivery: delivery, sku: sku, qty: 10)\n").unwrap();
    let (code, _, err) = binary(&["check", only_book.to_str().unwrap()]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains(":3:10: this dandori cannot read books, and the flow uses the book `stock`"), "{err}");
    // with every port joined, both pass
    let joined = |path: &str| {
        let args: Vec<String> = ["check", path].iter().map(|a| a.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = dandori::cli::run_with_ports(&args, Rc::new(rulec::ports::Engine::new()), Rc::new(koyomi::ports::Engine), Rc::new(chobo::ports::Engine), &mut out, &mut err);
        (code, String::from_utf8(err).unwrap())
    };
    assert_eq!(joined(flow), (0, format!("{flow}: ok\n")));
    assert_eq!(joined(only_book.to_str().unwrap()).0, 0);
}

/// With rulec's port alone (a program that joins rulec only), `scenarios` leaves the flow's rules to
/// rulec and says so (`"rules": "rulec"`), and `run` computes them as the scenario says, as `ritsu
/// dandori run` does: each scenario runs to its end and takes every answer it has. The page draws
/// the runs of the same scenarios.
#[test]
fn with_rulec_alone_the_scenarios_run_with_their_rules_computed() {
    let t = TempDir::new("rulec-alone");
    let flow = "examples/hotel/temporal/hotel.flow";
    let dir = t.path().join("out");
    let (code, _, err) = with_rulec(&["scenarios", flow, "--out", dir.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    let scenarios = written(&dir);
    assert!(scenarios.len() > 1, "{scenarios:?}");
    for (name, text) in &scenarios {
        let sc: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(sc["rules"], "rulec", "{name}");
        let (code, _, err) = with_rulec(&["run", flow, "--scenario", dir.join(name).to_str().unwrap()]);
        assert_eq!((code, err.as_str()), (0, ""), "{name}");
    }
    let (code, page, err) = with_rulec(&["doc", flow, "--format", "html"]);
    assert_eq!(code, 0, "{err}");
    assert!(page.contains(&format!("<h2>Scenarios</h2><p class=\"note\">{} scenarios", scenarios.len())), "the page draws {} runs", scenarios.len());
}

/// A program that keeps one port of rules for many commands (a library, a server) is answered for
/// the rules as they are when each command runs: the walk of `scenarios` and the runs of the page
/// ask the disk each question once while they ask the port again and again (DESIGN 36.3), and the
/// next command asks the disk again, so a rule changed between two commands is read anew. What the
/// kept port writes then is what a new port writes.
#[test]
fn a_port_kept_across_commands_reads_a_rule_changed_between_them() {
    let t = TempDir::new("kept-port");
    for d in ["temporal", "rules", "specs"] {
        for (name, text) in written(&root().join("examples/hotel").join(d)) {
            t.write(&format!("{d}/{name}"), &text);
        }
    }
    let flow = t.path().join("temporal/hotel.flow").to_str().unwrap().to_string();
    let ask = |port: &Rc<rulec::ports::Engine>, args: &[&str]| {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = dandori::cli::run(&args, port.clone(), &mut out, &mut err);
        assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
        String::from_utf8(out).unwrap()
    };
    let kept = Rc::new(rulec::ports::Engine::new());
    let before = (ask(&kept, &["scenarios", &flow]), ask(&kept, &["doc", &flow, "--format", "html"]));
    // the desk looks at a stay of seven nights or more, not fifteen
    let rule = t.path().join("rules/hold_amount.rule");
    let text = std::fs::read_to_string(&rule).unwrap();
    let changed = text.replace("| <=14   | auto        |", "| <=6    | auto        |").replace("| >=15   | review      |", "| >=7    | review      |");
    assert_ne!(changed, text);
    std::fs::write(&rule, changed).unwrap();
    let after = (ask(&kept, &["scenarios", &flow]), ask(&kept, &["doc", &flow, "--format", "html"]));
    let new = Rc::new(rulec::ports::Engine::new());
    let anew = (ask(&new, &["scenarios", &flow]), ask(&new, &["doc", &flow, "--format", "html"]));
    assert!(after.0 != before.0 && after.1 != before.1, "the walk reads the rule as it is now");
    assert!(after == anew, "the kept port answers as a new one does");
}

/// `cli::run` takes the words after the program's name and writes where it is told: a page asked
/// for to `out`, what is wrong to `err`, and answers the exit code.
#[test]
fn the_command_writes_where_it_is_told() {
    let (code, out, err) = with_rulec(&["--version"]);
    assert_eq!((code, out, err), (0, format!("dandori {}\n", env!("CARGO_PKG_VERSION")), String::new()));
    let (code, out, err) = with_rulec(&["check", "--help"]);
    assert!(code == 0 && out.starts_with("dandori check") && err.is_empty(), "{code} {out} {err}");
    let (code, out, err) = with_rulec(&["nope"]);
    assert_eq!((code, out.as_str(), err.as_str()), (2, "", "error: there is no command `nope`; run `dandori --help`\n"));
    let (code, out, err) = with_rulec(&[]);
    assert!(code == 2 && out.is_empty() && err.contains("Usage"), "{code} {out} {err}");
}

/// `gen` builds every platform given before it writes, each into `<out>/<target>/`, in the order of
/// `TARGETS` whatever the order given; what it writes is what each platform alone writes, and
/// `--check` writes nothing and says what is missing or differs, with exit 1 (DESIGN 21). When one
/// platform cannot take the flow, nothing is written.
#[test]
fn gen_writes_each_platform_into_its_own_directory() {
    let tmp = TempDir::new("cli-gen");
    let flow = "examples/fulfillment/arrange_delivery.flow";
    let at = |d: &str| tmp.path().join(d).to_str().unwrap().to_string();
    let (code, out, err) = binary(&["gen", flow, "--target", "temporal-typescript", "--target", "asl", "--out", &at("both")]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    let said: Vec<&str> = out.lines().collect();
    assert!(said[0].ends_with("/both/asl/arrange_delivery.asl.json") && said[1..].iter().all(|l| l.contains("/both/temporal-typescript/arrange_delivery/")), "{out}");
    for t in ["asl", "temporal-typescript"] {
        let (code, _, err) = binary(&["gen", flow, "--target", t, "--out", &at("one")]);
        assert_eq!(code, 0, "{err}");
    }
    assert_eq!(written(&tmp.path().join("both")), written(&tmp.path().join("one")));
    // --check, in either order of the flags: the same; a file changed and a file gone
    assert_eq!(binary(&["gen", flow, "--target", "asl", "--target", "temporal-typescript", "--out", &at("both"), "--check"]), (0, String::new(), String::new()));
    std::fs::write(tmp.path().join("both/asl/arrange_delivery.asl.json"), "{}").unwrap();
    std::fs::remove_file(tmp.path().join("both/temporal-typescript/arrange_delivery/io.ts")).unwrap();
    let (code, out, err) = binary(&["gen", flow, "--target", "asl", "--target", "temporal-typescript", "--out", &at("both"), "--check"]);
    assert_eq!((code, err.as_str()), (1, ""));
    assert_eq!(
        out.replace(tmp.path().to_str().unwrap(), "@"),
        "generated file is stale or hand-edited: @/both/asl/arrange_delivery.asl.json\nmissing: @/both/temporal-typescript/arrange_delivery/io.ts\n"
    );
    // a flow Step Functions cannot take: nothing is written for Temporal either
    let cancel = tmp.path().join("cancel.flow");
    std::fs::write(&cancel, "workflow w v1\n\nflow\n  pass\n\non cancel\n  pass\n").unwrap();
    let (code, out, err) = binary(&["gen", cancel.to_str().unwrap(), "--target", "temporal-typescript", "--target", "asl", "--out", &at("none")]);
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    assert!(err.starts_with("error[E050]:") && err.matches("error[").count() == 1, "{err}");
    assert!(!tmp.path().join("none").exists());
}

/// The verb and the name of 0.40.0 stop with exit 2 and say the word now, in either language; so
/// do no `--target` and one given twice.
#[test]
fn the_words_of_0_40_stop_with_the_words_now() {
    let flow = "examples/fulfillment/arrange_delivery.flow";
    let stop = |args: &[&str], en: &str| {
        let (code, out, err) = binary(args);
        assert_eq!((code, out.as_str(), err.as_str()), (2, "", format!("error: {en}\n").as_str()), "{args:?}");
    };
    stop(&["build", flow, "--target", "asl"], "`dandori build` is `dandori gen` from 0.41.0");
    stop(&["help", "build"], "`dandori build` is `dandori gen` from 0.41.0");
    stop(&["gen", flow, "--target", "temporal"], "`temporal` is `temporal-typescript` from 0.41.0");
    stop(&["run", flow, "--scenario", "s.json", "--target", "temporal"], "`temporal` is `temporal-typescript` from 0.41.0");
    let usage = "dandori gen <file.flow> --target asl|temporal-typescript|temporal-python|temporal-go|durable|argo|pydantic-graph... [--out <dir>] [--check] [--format json|text]";
    stop(&["gen", flow], &format!("`dandori gen` needs `--target asl|temporal-typescript|temporal-python|temporal-go|durable|argo|pydantic-graph`\nusage: {usage}"));
    stop(&["gen", flow, "--target", "asl", "--target", "asl"], "`--target asl` is given twice");
    assert_eq!(binary(&["build", flow, "--lang", "ja"]), (2, String::new(), "エラー: `dandori build` は 0.41.0 から `dandori gen` です\n".to_string()));
    assert_eq!(binary(&["gen", flow, "--target", "temporal", "--lang", "ja"]), (2, String::new(), "エラー: `temporal` は 0.41.0 から `temporal-typescript` です\n".to_string()));
    assert_eq!(
        binary(&["gen", flow, "--lang", "ja"]),
        (2, String::new(), format!("エラー: `dandori gen` には `--target asl|temporal-typescript|temporal-python|temporal-go|durable|argo|pydantic-graph` が要ります\n使い方: {usage}\n"))
    );
}

/// Answers the run does not take are an error of the scenario's (DESIGN 36): the run is printed to
/// its end, as with every answer taken, and then the answers left over — which they are, the first
/// three of them cut short, and why a run leaves answers — and the run ends with 1, as one that runs
/// out of answers does. In either language, from the binary and with rulec's port alike.
#[test]
fn answers_the_run_does_not_take_are_said_after_the_run() {
    let t = TempDir::new("left-over");
    let flow = t.path().join("two_calls.flow");
    std::fs::write(&flow, "workflow two_calls v1\n\ninputs\n  id : string\n\noutputs\n  id : string\n\ntask first(id: string) -> string\ntask second(id: string) -> string\n\nflow\n  let a = first(id: id)\n  let b = second(id: a)\n  succeed id = b\n").unwrap();
    let scenario = |name: &str, answers: serde_json::Value| {
        let f = t.path().join(name);
        std::fs::write(&f, serde_json::json!({ "input": { "id": "A-1" }, "answers": answers }).to_string()).unwrap();
        f.to_string_lossy().into_owned()
    };
    let (flow, two) = (flow.to_string_lossy().into_owned(), scenario("two.json", serde_json::json!([{ "ok": "x" }, { "ok": "y" }])));
    let (code, whole, err) = binary(&["run", &flow, "--scenario", &two]);
    assert_eq!((code, err.as_str()), (0, ""), "every answer taken");
    let why = "  The run came to its end before a call took it: the scenario may count the calls wrong, or be written for another way through the flow, or for the flow before it changed.\n";
    let three = scenario("three.json", serde_json::json!([{ "ok": "x" }, { "ok": "y" }, { "ok": "z" }]));
    for run in [binary, with_rulec] {
        assert_eq!(run(&["run", &flow, "--scenario", &three]), (1, whole.clone(), format!("error: the run did not take answer 3 of the scenario: {{\"ok\":\"z\"}}\n{why}")));
        assert_eq!(
            run(&["run", &flow, "--scenario", &three, "--lang", "ja"]),
            (
                1,
                whole.clone(),
                "エラー: シナリオの 3 番目の結果は使われませんでした: {\"ok\":\"z\"}\n  実行は、その結果を受け取る呼び出しの前に終わりました。シナリオが呼び出しの数を数え違えているか、フローの別の分岐や、変わる前のフローに合わせて書かれているのかもしれません。\n".to_string()
            )
        );
    }
    // four left over: the first three said, each cut at 60 characters
    let long = "a".repeat(80);
    let six = scenario("six.json", serde_json::json!([{ "ok": "x" }, { "ok": "y" }, { "ok": long }, { "error": "failure" }, { "ok": 3 }, { "ok": 4 }]));
    let (code, out, err) = binary(&["run", &flow, "--scenario", &six]);
    let cut = format!("{{\"ok\":\"{}…", "a".repeat(52));
    assert_eq!((code, out, err), (1, whole, format!("error: the run did not take answers 3 to 6 of the scenario (4 of them): {cut}, {{\"error\":\"failure\"}}, {{\"ok\":3}}, …\n{why}")));
}

/// A loop that goes round as long as a task says so: tests/fixtures/history.flow polls up to 5000
/// times, and history_round.flow up to 9000 times in each of 100 rounds. A run that keeps to the
/// answer that goes round again is cut long before the loop runs out, where `dandori run` would run
/// out of answers, and is not written; the walk goes again from the round where it first gave that
/// answer again, with each other answer there (DESIGN 36). A scenario takes the arm that goes round
/// again, and every scenario runs under `dandori run` with each answer it gives taken.
#[test]
fn a_loop_that_goes_round_on_a_task_s_answer_has_a_scenario_that_goes_round() {
    let t = TempDir::new("go-round");
    for flow in ["tests/fixtures/history.flow", "tests/fixtures/history_round.flow"] {
        let (code, out, err) = binary(&["scenarios", flow]);
        assert_eq!((code, err.as_str()), (0, ""), "{flow}");
        let scenarios: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap();
        let mut covers = std::collections::BTreeSet::new();
        for (i, sc) in scenarios.iter().enumerate() {
            covers.extend(sc["covers"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()));
            let f = t.path().join(format!("{i}.json"));
            std::fs::write(&f, sc.to_string()).unwrap();
            let (code, _, err) = binary(&["run", flow, "--scenario", f.to_str().unwrap()]);
            assert_eq!((code, err.as_str()), (0, ""), "{flow} {i}: {sc}");
        }
        assert!(covers.contains("4:arm0") && covers.contains("4:arm1"), "{flow}: no scenario goes round and leaves the loop both: {covers:?}");
    }
}

/// What a command says of a file or a directory it cannot read, make or write is said in the
/// language asked, in the words every command of ritsu says them in (ritsu's DESIGN 6.2, item 10):
/// a flow `check`, `api` and `doc` cannot read, and a directory of `--out` that `gen`, `scenarios`
/// and `doc` cannot make or write into. The reason is the system's own words, in both.
#[test]
fn a_file_a_command_cannot_read_or_write_is_said_in_either_language() {
    let t = TempDir::new("cli-files");
    let at = |s: &str| s.replace(t.path().to_str().unwrap(), "@");
    let says = |args: &[&str], en: &str, ja: &str| {
        for (lang, head, want) in [("en", "error: ", en), ("ja", "エラー: ", ja)] {
            let mut with_lang: Vec<&str> = args.to_vec();
            with_lang.extend(["--lang", lang]);
            let (code, out, err) = binary(&with_lang);
            assert_eq!((code, out.as_str(), at(&err).as_str()), (2, "", format!("{head}{want}\n").as_str()), "{with_lang:?}");
        }
    };
    // a flow that is not there
    let missing = t.path().join("missing.flow");
    for cmd in ["check", "api", "doc"] {
        says(&[cmd, missing.to_str().unwrap()], "`@/missing.flow` is not there", "`@/missing.flow` がありません");
    }
    // a directory of `--out` under a file, which cannot be made
    let flow = "examples/fulfillment/arrange_delivery.flow";
    t.write("file", "x");
    let under = t.path().join("file/out");
    for (args, made) in [
        (vec!["gen", flow, "--target", "asl", "--out", under.to_str().unwrap()], under.join("asl")),
        (vec!["scenarios", flow, "--out", under.to_str().unwrap()], under.clone()),
        (vec!["doc", flow, "--out", under.to_str().unwrap()], under.clone()),
    ] {
        let os = std::fs::create_dir_all(&made).unwrap_err().to_string();
        let shown = at(made.to_str().unwrap());
        says(&args, &format!("cannot create `{shown}`: {os}"), &format!("`{shown}` を作れません: {os}"));
    }
    // a file of `--out` that is a directory, which cannot be written
    for (args, file) in [
        (vec!["gen", flow, "--target", "asl", "--out", "@/gen"], "gen/asl/arrange_delivery.asl.json"),
        (vec!["scenarios", flow, "--out", "@/scenarios"], "scenarios/001.json"),
        (vec!["doc", flow, "--out", "@/doc"], "doc/arrange_delivery.md"),
    ] {
        let p = t.path().join(file);
        std::fs::create_dir_all(&p).unwrap();
        let os = std::fs::write(&p, "x").unwrap_err().to_string();
        let args: Vec<String> = args.iter().map(|a| a.replace('@', t.path().to_str().unwrap())).collect();
        says(&args.iter().map(String::as_str).collect::<Vec<_>>(), &format!("cannot write `@/{file}`: {os}"), &format!("`@/{file}` に書けません: {os}"));
    }
}

/// What stops `dandori run` before or during the run is said in the language asked: no
/// `--scenario`, a scenario that cannot be read, a `--target` the table does not take, a scenario
/// that runs out of answers, a run that goes past its 20000 steps, and a `now` that is not a time,
/// from the binary and, where the scenario says the rules are computed, with rulec's port. What
/// stops it before the run (2) is said as every command of ritsu says it, after the word of an error
/// (ritsu's DESIGN 6.2, item 10); what stops the run itself (1), as the run says it.
#[test]
fn what_stops_dandori_run_is_said_in_either_language() {
    let t = TempDir::new("cli-run");
    let flow = t.path().join("two_calls.flow");
    std::fs::write(&flow, "workflow two_calls v1\n\ninputs\n  id : string\n\noutputs\n  id : string\n\ntask first(id: string) -> string\ntask second(id: string) -> string\n\nflow\n  let a = first(id: id)\n  let b = second(id: a)\n  succeed id = b\n").unwrap();
    let flow = flow.to_str().unwrap().to_string();
    let scenario = |name: &str, sc: serde_json::Value| {
        let f = t.path().join(name);
        std::fs::write(&f, sc.to_string()).unwrap();
        f.to_str().unwrap().to_string()
    };
    let at = |s: &str| s.replace(t.path().to_str().unwrap(), "@");
    let says = |run: fn(&[&str]) -> (u8, String, String), args: &[&str], code: u8, en: &str, ja: &str| {
        for (lang, head, want) in [("en", "error: ", en), ("ja", "エラー: ", ja)] {
            let mut with_lang: Vec<&str> = args.to_vec();
            with_lang.extend(["--lang", lang]);
            let (c, out, err) = run(&with_lang);
            let head = if code == 2 { head } else { "" };
            assert_eq!((c, out.as_str(), at(&err).as_str()), (code, "", format!("{head}{want}\n").as_str()), "{with_lang:?}");
        }
    };
    // no `--scenario`: what the command needs, then how it is run
    for (lang, want) in [
        ("en", "error: `dandori run` needs `--scenario <file.json>`\nusage: dandori run <file.flow> --scenario <file.json> "),
        ("ja", "エラー: `dandori run` には `--scenario <file.json>` が要ります\n使い方: dandori run <file.flow> --scenario <file.json> "),
    ] {
        let (c, out, err) = binary(&["run", &flow, "--lang", lang]);
        assert!(c == 2 && out.is_empty() && err.starts_with(want) && err.lines().count() == 2, "{err}");
    }
    let missing = t.path().join("missing.json");
    says(binary, &["run", &flow, "--scenario", missing.to_str().unwrap()], 2, "`@/missing.json` is not there", "`@/missing.json` がありません");
    let not_json = t.path().join("not.json");
    std::fs::write(&not_json, "not json").unwrap();
    let why = serde_json::from_str::<serde_json::Value>("not json").unwrap_err().to_string();
    says(binary, &["run", &flow, "--scenario", not_json.to_str().unwrap()], 2, &format!("`@/not.json` is not JSON: {why}"), &format!("`@/not.json` は JSON として読めません: {why}"));
    // the table refuses a target it does not take before the run
    let none = scenario("none.json", serde_json::json!({ "input": { "id": "A-1" }, "answers": [] }));
    let (code, _, err) = binary(&["run", &flow, "--scenario", &none, "--target", "nope", "--lang", "ja"]);
    assert!(code == 2 && err.starts_with("エラー: `--target` に `nope` は使えません。"), "{err}");
    says(binary, &["run", &flow, "--scenario", &none], 1, "the scenario has no answer for call 1 (first)", "シナリオに、1 番目の呼び出し（first）の結果がありません");
    // a run past its steps
    let many = t.path().join("many.flow");
    std::fs::write(&many, "workflow many v1\n\ninputs\n  items : list[string]\n\ntask touch(item: string) -> string\n\nflow\n  for item in items at most 30000\n    touch(item: item)\n").unwrap();
    let n = 20_005;
    let long = scenario("long.json", serde_json::json!({ "input": { "items": vec!["x"; n] }, "answers": vec![serde_json::json!({ "ok": "y" }); n] }));
    says(binary, &["run", many.to_str().unwrap(), "--scenario", &long], 1, "the run took more than 20000 steps", "実行が 20000 ステップを超えました");
    // a `now` that is not a time: of the scenario's clock alone, and of a scenario that cannot be run
    let now_en = "the scenario's `now`, `nope`, is not a time (`2026-03-31T15:30:00Z`)";
    let now_ja = "シナリオの `now` の `nope` は時刻（`2026-03-31T15:30:00Z` の形）ではありません";
    let bad_now = scenario("bad_now.json", serde_json::json!({ "input": { "id": "A-1" }, "answers": [{ "ok": "x" }, { "ok": "y" }], "now": "nope" }));
    says(binary, &["run", &flow, "--scenario", &bad_now], 1, now_en, now_ja);
    let computed = scenario("computed.json", serde_json::json!({ "input": { "id": "A-1" }, "answers": [{ "ok": "x" }, { "ok": "y" }], "now": "nope", "rules": "rulec" }));
    says(with_rulec, &["run", &flow, "--scenario", &computed], 2, &format!("the scenario `@/computed.json` cannot be run: {now_en}"), &format!("シナリオ `@/computed.json` を流せません: {now_ja}"));
}

/// rulec's port with its evaluator answering every call as `say` gives: what a run says when rulec
/// cannot answer a call the check let through. Every other question is rulec's.
struct RulesSaying {
    inner: rulec::ports::Engine,
    say: fn() -> ritsu_ports::RuleError,
}

type Ranges<'a> = &'a [(String, Option<i128>, Option<i128>)];
type Saids = Vec<ritsu_ports::Said>;

impl ritsu_ports::Rules for RulesSaying {
    fn facts(&self, rule: &Path) -> Result<ritsu_ports::RuleFacts, Saids> {
        self.inner.facts(rule)
    }
    fn preconditions_hold(&self, rule: &Path, ranges: Ranges, min_len: Option<i128>, max_len: Option<i128>) -> Result<Vec<(ritsu_ports::Precondition, ritsu_ports::Answer<ritsu_ports::Values>)>, Saids> {
        self.inner.preconditions_hold(rule, ranges, min_len, max_len)
    }
    fn preconditions_hold_given(&self, rule: &Path, ranges: Ranges, values: &[(String, Vec<ritsu_ports::Value>)], min_len: Option<i128>, max_len: Option<i128>) -> Result<Vec<(ritsu_ports::Precondition, ritsu_ports::Answer<ritsu_ports::Values>)>, Saids> {
        self.inner.preconditions_hold_given(rule, ranges, values, min_len, max_len)
    }
    fn output_values(&self, rule: &Path, output: &str) -> Result<ritsu_ports::Found<ritsu_ports::OutputValues>, Saids> {
        self.inner.output_values(rule, output)
    }
    fn outputs_over(&self, rule: &Path, output: &str, ranges: Ranges) -> Result<ritsu_ports::Found<Vec<(ritsu_ports::Value, ritsu_ports::Values)>>, Saids> {
        self.inner.outputs_over(rule, output, ranges)
    }
    fn outputs_together(&self, rule: &Path, outputs: &[String], ranges: Ranges) -> Result<ritsu_ports::Found<Vec<(Vec<ritsu_ports::Value>, ritsu_ports::Values)>>, Saids> {
        self.inner.outputs_together(rule, outputs, ranges)
    }
    fn checked_over(&self, rule: &Path, input: &str, days: &ritsu_ports::DaySet) -> Result<ritsu_ports::Answer<ritsu_base::text::Text>, Saids> {
        self.inner.checked_over(rule, input, days)
    }
    fn output_values_over(&self, rule: &Path, output: &str, ranges: Ranges) -> Result<ritsu_ports::Found<ritsu_ports::OutputValues>, Saids> {
        self.inner.output_values_over(rule, output, ranges)
    }
    fn eval(&self, _rule: &Path, _inputs: &ritsu_ports::Values) -> Result<ritsu_ports::Values, ritsu_ports::RuleError> {
        Err((self.say)())
    }
    fn vector_inputs(&self, rule: &Path) -> Result<Vec<ritsu_ports::Values>, Saids> {
        self.inner.vector_inputs(rule)
    }
    fn doc(&self, rule: &Path, shown: &str, html: bool, lang: ritsu_base::text::Lang) -> Result<String, Saids> {
        self.inner.doc(rule, shown, html, lang)
    }
    fn rests_on(&self, rule: &Path) -> Option<String> {
        self.inner.rests_on(rule)
    }
}

/// koyomi's port, but the dates files cannot be read (`facts`), or koyomi computes no date of them
/// (`eval`): what a run says when koyomi cannot answer a call the check let through.
struct DatesSaying {
    facts_gone: bool,
}

impl ritsu_ports::Dates for DatesSaying {
    fn facts(&self, file: &Path) -> Result<ritsu_ports::DateFacts, Saids> {
        if self.facts_gone {
            return Err(gone(file));
        }
        koyomi::ports::Engine.facts(file)
    }
    fn values(&self, file: &Path, date: &str) -> Result<ritsu_ports::Found<ritsu_ports::DaySet>, Saids> {
        koyomi::ports::Engine.values(file, date)
    }
    fn days(&self, file: &Path, date: &str) -> Result<ritsu_ports::Found<(i64, i64)>, Saids> {
        koyomi::ports::Engine.days(file, date)
    }
    fn span(&self, file: &Path, date: &str) -> Result<ritsu_ports::Found<ritsu_ports::DaySpan>, Saids> {
        koyomi::ports::Engine.span(file, date)
    }
    fn input_for(&self, file: &Path, date: &str, day: ritsu_ports::Day) -> Result<Option<Vec<(String, i64)>>, Saids> {
        koyomi::ports::Engine.input_for(file, date, day)
    }
    fn eval(&self, _file: &Path, _inputs: &[(String, i64)]) -> Result<Vec<(String, ritsu_ports::DateValue)>, Saids> {
        Ok(Vec::new())
    }
    fn calendar(&self, file: &Path) -> Result<ritsu_ports::CalendarFacts, Saids> {
        koyomi::ports::Engine.calendar(file)
    }
    fn rests_on(&self, file: &Path) -> Option<String> {
        koyomi::ports::Engine.rests_on(file)
    }
}

/// chobo's port, but a book cannot be opened for a run (`open_gone`), or its ledger takes no
/// operation: what a run says when chobo cannot answer a call the check let through.
struct BooksSaying {
    open_gone: bool,
}

impl ritsu_ports::Books for BooksSaying {
    fn facts(&self, file: &Path) -> Result<ritsu_ports::BookFacts, Saids> {
        chobo::ports::Engine.facts(file)
    }
    fn refusals(&self, file: &Path, transfer: &str, amounts: (i128, i128)) -> Result<ritsu_ports::Found<Vec<(String, Vec<String>)>>, Saids> {
        chobo::ports::Engine.refusals(file, transfer, amounts)
    }
    fn open(&self, file: &Path) -> Result<Box<dyn ritsu_ports::Ledger>, Saids> {
        if self.open_gone {
            return Err(gone(file));
        }
        Ok(Box::new(Closed(chobo::ports::Engine.open(file)?)))
    }
}

/// A ledger that takes no operation.
struct Closed(Box<dyn ritsu_ports::Ledger>);

impl ritsu_ports::Ledger for Closed {
    fn apply(&mut self, _call: &ritsu_ports::BookCall) -> Result<ritsu_ports::BookOutcome, ritsu_base::text::Text> {
        Err(ritsu_base::tr!("帳簿が閉じています", "the ledger is closed"))
    }
    fn pass(&mut self, seconds: u64) -> Vec<(String, Vec<String>)> {
        self.0.pass(seconds)
    }
    fn balance(&self, account: &str, args: &[String]) -> Result<ritsu_ports::Balance, ritsu_base::text::Text> {
        self.0.balance(account, args)
    }
    fn accounts(&self) -> Vec<(String, Vec<String>, ritsu_ports::Balance)> {
        self.0.accounts()
    }
    fn holds(&self) -> Vec<(String, Vec<String>, String)> {
        self.0.holds()
    }
}

/// What a port says of a file that is gone.
fn gone(file: &Path) -> Saids {
    vec![ritsu_ports::Said { code: String::new(), file: file.display().to_string(), line: None, message: ritsu_base::tr!("ファイルが消えました", "the file is gone") }]
}

/// A call of a rule, a date or a book that the language cannot answer stops the run, and what
/// stops it is said in the language asked, the English as it was: rulec cannot read the rule, or
/// gives it no answer; koyomi computes no date; chobo cannot open the book, or does not take the
/// operation (with `dandori run`, ports that answer the check and not the run); and koyomi cannot
/// read the dates file (of the run itself, `computed::run_said`, the check having read it).
#[test]
fn a_call_a_language_cannot_answer_stops_the_run_in_either_language() {
    let t = TempDir::new("cli-stops");
    let rulec = || Rc::new(rulec::ports::Engine::with_dates(std::sync::Arc::new(koyomi::ports::Engine)));
    // the first scenario `ritsu dandori scenarios` writes for the flow that goes through a call the
    // language answers (`covers` has `<site>:<language>`), with nothing done to the books before the
    // run when `untouched`, and the flow's model
    let first = |flow: &str, language: &str, untouched: bool| {
        let args: Vec<String> = ["scenarios", flow].iter().map(|a| a.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = dandori::cli::run_with_ports(&args, rulec(), Rc::new(koyomi::ports::Engine), Rc::new(chobo::ports::Engine), &mut out, &mut err);
        assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
        let list: Vec<serde_json::Value> = serde_json::from_str(&String::from_utf8(out).unwrap()).unwrap();
        let goes = |sc: &&serde_json::Value| {
            sc["covers"].as_array().is_some_and(|c| c.iter().any(|x| x.as_str().is_some_and(|x| x.ends_with(&format!(":{language}")))))
                && (!untouched || sc["books"].as_object().is_none_or(|b| b.values().all(|ops| ops.as_array().is_none_or(Vec::is_empty))))
        };
        let sc = list.iter().find(goes).unwrap_or_else(|| panic!("no scenario of {flow} calls {language}")).clone();
        let f = t.path().join(format!("{}-{language}.json", Path::new(flow).file_stem().unwrap().to_str().unwrap()));
        std::fs::write(&f, sc.to_string()).unwrap();
        let model = dandori::sources::with_ports(rulec(), Rc::new(koyomi::ports::Engine), Rc::new(chobo::ports::Engine), || dandori::check::built(Path::new(flow))).unwrap().1.model.unwrap();
        (f.to_str().unwrap().to_string(), sc, model)
    };
    let says = |flow: &str, sc: &str, rules: Rc<dyn ritsu_ports::Rules>, dates: Rc<dyn ritsu_ports::Dates>, books: Rc<dyn ritsu_ports::Books>, en: &str, ja: &str| {
        for (lang, want) in [("en", en), ("ja", ja)] {
            let args: Vec<String> = ["run", flow, "--scenario", sc, "--lang", lang].iter().map(|a| a.to_string()).collect();
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let code = dandori::cli::run_with_ports(&args, rules.clone(), dates.clone(), books.clone(), &mut out, &mut err);
            assert_eq!((code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap()), (1, String::new(), format!("{want}\n")), "{flow} {lang}");
        }
    };
    let hotel = "examples/hotel/temporal/hotel.flow";
    let (sc, _, _) = first(hotel, "rulec", false);
    let (koyomi, chobo) = (Rc::new(koyomi::ports::Engine), Rc::new(chobo::ports::Engine));
    let unread = Rc::new(RulesSaying { inner: rulec::ports::Engine::new(), say: || ritsu_ports::RuleError::Unread(gone(Path::new("hold_amount.rule"))) });
    says(hotel, &sc, unread, koyomi.clone(), chobo.clone(), "rulec cannot read the rule hold: the file is gone", "rulec が規則 hold を読めません: ファイルが消えました");
    let no_answer = Rc::new(RulesSaying { inner: rulec::ports::Engine::new(), say: || ritsu_ports::RuleError::Contradiction(ritsu_base::tr!("どの行にも当たりません", "no row takes the inputs")) });
    says(hotel, &sc, no_answer, koyomi.clone(), chobo.clone(), "rulec gives the rule hold no answer: no row takes the inputs", "rulec が規則 hold の結果を返しません: どの行にも当たりません");
    // the invoice holds the goods (a book), then counts the day it is due (a date)
    let invoice = "examples/invoice/invoice.flow";
    let (sc, written, m) = first(invoice, "koyomi", false);
    let dates_file = m.rules.iter().find(|r| matches!(r.kind, dandori::model::RuleKind::Date(_))).unwrap().info.path.display().to_string();
    let book = m.books[0].path.display().to_string();
    says(invoice, &sc, rulec(), Rc::new(DatesSaying { facts_gone: false }), chobo.clone(), &format!("koyomi gives no date payment of {dates_file}"), &format!("koyomi が {dates_file} の日付 payment を返しません"));
    says(invoice, &sc, rulec(), koyomi.clone(), Rc::new(BooksSaying { open_gone: true }), &format!("chobo cannot open the book {book}: the file is gone"), &format!("chobo が帳簿 {book} を開けません: ファイルが消えました"));
    let (untouched, _, _) = first(invoice, "chobo", true);
    says(invoice, &untouched, rulec(), koyomi.clone(), Rc::new(BooksSaying { open_gone: false }), "chobo does not take the operation stock.reserve.hold of task reserve: the ledger is closed", "chobo がタスク reserve の操作 stock.reserve.hold を受け付けません: 帳簿が閉じています");
    let ran = dandori::computed::run_said(&m, &written, dandori::render::View::Asl, rulec(), Rc::new(DatesSaying { facts_gone: true }), chobo);
    let Err(dandori::computed::Stopped::Run(why)) = ran else { panic!("the run goes on without the dates file") };
    assert_eq!(why, ritsu_base::tr!("koyomi が日付のファイル {dates_file} を読めません: ファイルが消えました", "koyomi cannot read the dates file {dates_file}: the file is gone"));
}

/// Why an API document cannot be read is said in the language asked, the English as it was: a file
/// of `use openapi` that is not JSON or not OpenAPI 3, one of `use smithy` that is not a model in the
/// JSON AST, and a `.proto` of `use proto` that is not proto3, does not read, or names a type nothing
/// has. What the JSON reader says is in its own words in both.
#[test]
fn an_api_document_dandori_cannot_read_is_said_in_either_language() {
    let t = TempDir::new("cli-apis");
    let not_json = serde_json::from_str::<serde_json::Value>("not json").unwrap_err().to_string();
    for (kind, file, text, en, ja) in [
        (
            "openapi",
            "not_json.json",
            "not json",
            format!("not_json.json is not JSON ({not_json}); dandori reads an OpenAPI document or a Smithy model written as JSON"),
            format!("not_json.json は JSON ではありません（{not_json}）。dandori が読むのは、JSON で書いた OpenAPI の文書か Smithy のモデルです"),
        ),
        ("openapi", "swagger.json", r#"{"swagger": "2.0"}"#, "swagger.json is not an OpenAPI 3 document".into(), "swagger.json は OpenAPI 3 の文書ではありません".into()),
        ("smithy", "not_smithy.json", r#"{"openapi": "3.0.0"}"#, "not_smithy.json is not a Smithy model in the JSON AST".into(), "not_smithy.json は、JSON AST で書いた Smithy のモデルではありません".into()),
        ("proto", "edition.proto", "edition = \"2023\";\npackage a;\n", "edition.proto: editions are not read; dandori reads proto3".into(), "edition.proto: editions は読みません。dandori が読むのは proto3 だけです".into()),
        ("proto", "proto2.proto", "syntax = \"proto2\";\npackage a;\n", "proto2.proto: `proto2` is not read; dandori reads proto3".into(), "proto2.proto: `proto2` は読みません。dandori が読むのは proto3 だけです".into()),
        ("proto", "broken.proto", "syntax = \"proto3\";\nmessage {\n", "broken.proto:2:9: a name is expected where `{` is".into(), "broken.proto:2:9: 名前が要るところに `{` があります".into()),
        (
            "proto",
            "typo.proto",
            "syntax = \"proto3\";\npackage shop.v1;\nmessage Order { Moneyy total = 1; }\n",
            "there is no type `Moneyy` (in `shop.v1.Order`)".into(),
            "型 `Moneyy` はありません（`shop.v1.Order` から探しました）".into(),
        ),
        ("proto", "dotted.proto", "syntax = \"proto3\";\npackage shop.v1;\nmessage Order { .shop.v1.Moneyy total = 1; }\n", "there is no type `.shop.v1.Moneyy`".into(), "型 `.shop.v1.Moneyy` はありません".into()),
    ] {
        t.write(file, text);
        t.write("w.flow", &format!("workflow w v1\n\nuse {kind} x from \"{file}\"\n\nflow\n  succeed\n"));
        let flow = t.path().join("w.flow");
        for (lang, head, note) in [("en", format!("could not read the API `{file}`"), en), ("ja", format!("API `{file}` を読めませんでした"), ja)] {
            let (code, out, err) = binary(&["check", flow.to_str().unwrap(), "--lang", lang]);
            let note = format!("  = {}\n", note.replace(file, &t.path().join(file).display().to_string()));
            assert!(code == 1 && out.is_empty() && err.contains(&head) && err.ends_with(&note), "{file} {lang}:\n{err}");
        }
    }
}
