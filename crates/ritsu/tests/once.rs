//! A file is checked once in a run (DESIGN 4.15): `ritsu check`, `ritsu run` and `ritsu gen` of the
//! test projects, the examples and a project the test writes (a module that modules in other
//! directories read), and the page, the api, the scenarios, the run of a scenario and the code a
//! language's own command makes of a file that reads other languages' files (`ritsu dandori doc`,
//! `ritsu dandori run`, `ritsu sekisho gen`, …), ringi's examples and each of `ritsu ringi`'s
//! commands on them, the check and `gen` in Japanese too (the requests, the organization, the
//! masters and the shared route they read,
//! the files a request is lowered to, and the data of the organization and the masters), ringi's
//! check in Japanese and then rulec's check of the rule its route applies, with one run's engines on
//! a thread of their own (the rule is checked once, in Japanese, by rulec's port), `ritsu
//! impact`'s `gen --check` of a module of requests in Japanese and in English, a run of `requests`
//! whose flow reads the API the run writes and one whose flow reads another API, and the
//! one question of the page in the browser (ritsu-wasm), each run in this
//! process from the project's directory with the paths written from there, and every file each
//! language checked whole counted (`ritsu_base::tally`, which each language's check counts into).
//! A file asked about by many (its own check, the checks across the languages, the files that read
//! it, a rule that applies it, the run) is checked by the first, and the others are answered from
//! what that check found. A rule that does not pass rulec's check is checked once in each language,
//! since the other languages are handed its errors in both (DESIGN 4.15): its two checks are let
//! through. A rule a contribution of the run reaches is two rules, alone and merged (rulec's
//! §15.231), and the check of the rule alone is counted apart (`rulec.alone`). A run of `requests`
//! whose flow reads the API it writes makes the API from the requests before the check, with their
//! flows and gates taken to pass, unread, by the run's ringi, which keeps what it checks so where
//! they pass (ringi's DESIGN 9.3): every file of that run is checked once, as is every file of a run
//! whose flow reads another API, which makes nothing before the check. The one test of the file runs
//! the commands one after another, since each runs in the directory of its project.

use ritsu_base::text::Lang;
use std::path::{Path, PathBuf};

fn tree() -> PathBuf {
    ritsu_base::paths::absolute(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

/// The files checked since the counts were last taken, and those of them checked more than once
/// that `allowed` does not let through, each with its language and how many times. A rule read for
/// what is made of it after its check (`rulec.prepare`: the code, the page, a port's answer) is read
/// as the check reads it, so the two are counted as one language's: a rule that passes is read once
/// in a run.
fn twice(allowed: &dyn Fn(&str, &Path, usize) -> bool) -> (usize, Vec<String>) {
    let all = taken();
    let mut out: Vec<String> = all.iter().filter(|(tool, file, n)| *n > 1 && !allowed(tool, file, *n)).map(|(tool, file, n)| format!("{tool} {} x{n}", file.display())).collect();
    out.sort();
    (all.len(), out)
}

/// The files checked since the counts were last taken, each with its language and how many times,
/// a rule read for what is made of it after its check counted with its check ([`twice`]).
fn taken() -> Vec<(&'static str, PathBuf, usize)> {
    let mut all: Vec<(&str, PathBuf, usize)> = Vec::new();
    for (tool, file, n) in ritsu_base::tally::take() {
        let tool = if tool == "rulec.prepare" { "rulec" } else { tool };
        match all.iter_mut().find(|(t, f, _)| *t == tool && *f == file) {
            Some(e) => e.2 += n,
            None => all.push((tool, file, n)),
        }
    }
    all
}

fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

/// A command of ritsu's, run in `dir` (from the tree's root), and the files it checked more than
/// once; a command that checked nothing is said too, since it says nothing of checking once.
fn counted(dir: &str, what: &str, run: impl FnOnce() -> u8, seen: &mut Vec<String>) {
    counted_but(dir, what, run, &|_, _, _| false, seen)
}

/// [`counted`], letting through what `allowed` says may be checked more than once.
fn counted_but(dir: &str, what: &str, run: impl FnOnce() -> u8, allowed: &dyn Fn(&str, &Path, usize) -> bool, seen: &mut Vec<String>) {
    std::env::set_current_dir(tree().join(dir)).unwrap();
    ritsu_base::tally::take();
    run();
    let (files, more) = twice(allowed);
    if files == 0 {
        seen.push(format!("{what} in {dir}: no file counted"));
    }
    for t in more {
        seen.push(format!("{what} in {dir}: {t}"));
    }
}

/// [`counted`], letting through the two checks of a rule under `dir` that does not pass rulec's
/// check: once in each language.
fn counted_failing(dir: &str, what: &str, run: impl FnOnce() -> u8, seen: &mut Vec<String>) {
    let fails = failing(&tree().join(dir));
    counted_but(dir, what, run, &|tool, file, n| tool == "rulec" && n == 2 && fails.iter().any(|r| file.ends_with(r)), seen)
}

/// The rules under `dir` (by their paths from it) that do not pass rulec's check, as a check from
/// `dir` of the files on the disk says: asked on a thread of their own, so that what rulec keeps on
/// the thread of a check (DESIGN 4.15) is not what the commands counted after find kept.
fn failing(dir: &Path) -> Vec<String> {
    let dir = dir.to_path_buf();
    std::thread::spawn(move || {
        std::env::set_current_dir(&dir).unwrap();
        files_of(&dir)
            .into_iter()
            .filter(|(p, _)| p.ends_with(".rule"))
            .filter(|(p, text)| rulec::has_error(&rulec::report(&String::from_utf8_lossy(text), p).diags))
            .map(|(p, _)| p)
            .collect()
    })
    .join()
    .unwrap()
}

/// A flow a request starts once it is approved, that reads the ticket through the server's API
/// `--target requests` writes (`out/requests/openapi.json` of the project).
const FOLLOW_FLOW: &str = r#"workflow follow v1
description "Reads the ticket it is started for through the server of the requests, and answers the state it is in"

use openapi requests from "../out/requests/openapi.json"
  url "https://requests.example.com"

enum Status = in_progress | paused | with_requester | approved | rejected | withdrawn | expired

record Ticket
  id     : string
  status : Status

record Seen
  ticket : Ticket

inputs
  request_id : string

outputs
  status : Status

task get_ticket(id: string) -> Seen
  http GET requests "/tickets/{id}"

flow
  let seen = get_ticket(id: request_id)
  succeed status = seen.ticket.status
"#;

#[test]
fn every_file_is_checked_once_in_a_run() {
    let mut seen = Vec::new();
    let out = ritsu_testkit::TempDir::new("once");
    let check = |a: &[&str]| -> u8 {
        let (mut o, mut e) = (Vec::new(), Vec::new());
        ritsu::check::run(&args(a), Lang::En, &mut o, &mut e)
    };
    for p in [
        "crates/ritsu/tests/projects/invoice",
        "crates/ritsu/tests/projects/shop",
        "crates/ritsu/tests/projects/approvals",
        "crates/ritsu/tests/projects/ticket_desk",
        "crates/ritsu/tests/projects/settlement",
        "crates/ritsu/tests/projects/marketplace",
        "crates/ritsu/tests/projects/stockroom",
        // rules that read the enums of an `enums` file, a contribution to one, and rules that apply
        // another (a module of ritsu's among them)
        "crates/ritsu/tests/projects/catalog",
        "crates/ritsu/tests/projects/product_line",
        "crates/ritsu/tests/projects/shared_states",
        "crates/rulec/tests/modules/en/app",
        "crates/rulec/experiments/library",
        "crates/sakai/examples/shop",
        "crates/dandori/examples/hotel/temporal",
        "modules/fmla",
    ] {
        counted_failing(p, "check", || check(&["."]), &mut seen);
    }
    // a module that modules in two directories and at the root read, each by a path written from its
    // own directory: its check, and what the checks across the borders ask of it, are one check
    let shared = out.path().join("shared_module");
    shared_module(&shared);
    let at = shared.to_string_lossy().to_string();
    counted(&at, "check", || check(&["."]), &mut seen);
    let to = out.path().join("shared_module_gen").to_string_lossy().to_string();
    counted(&at, "gen", || ritsu::package::command(&args(&["--root", ".", "--out", &to, "--target", "typescript", "--target", "python"]), Lang::En), &mut seen);
    counted("crates/dandori/examples/hotel/temporal", "check of a flow", || check(&["hotel.flow"]), &mut seen);
    counted("crates/sakai/examples/shop/ordering", "check of a flow", || check(&["fulfillment.flow"]), &mut seen);
    counted("crates/ritsu/tests/projects/invoice", "run", || ritsu::run::command(&args(&["run", "invoice.flow", "--scenario", "scenarios/invoice_paid.json"])[1..], Lang::En), &mut seen);
    // the product lines merged into the rules by the run, and not (the rule then refuses the kind
    // they add, which the run says before it starts)
    counted("crates/ritsu/tests/projects/product_line", "run", || ritsu::run::command(&args(&["pricing.flow", "--scenario", "scenarios/garden.json", "--with", "rules/contrib"]), Lang::En), &mut seen);
    counted("crates/ritsu/tests/projects/product_line", "run", || ritsu::run::command(&args(&["pricing.flow", "--scenario", "scenarios/garden.json"]), Lang::En), &mut seen);
    counted("crates/ritsu/tests/projects/shared_states", "run", || ritsu::run::command(&args(&["refunds.flow", "--scenario", "scenarios/shipped.json"]), Lang::En), &mut seen);
    // the flows whose `ritsu run` the speed is measured on, each with the first scenario
    // `ritsu dandori scenarios` writes for it
    for (p, flow) in [("crates/dandori/examples/hotel/temporal", "hotel.flow"), ("crates/sakai/examples/shop/ordering", "fulfillment.flow"), ("modules/fmla", "fmla_request.flow")] {
        std::env::set_current_dir(tree().join(p)).unwrap();
        let sc = out.path().join(flow.trim_end_matches(".flow"));
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let wrote = ritsu::languages::dandori(&args(&["scenarios", flow, "--out", &sc.to_string_lossy()]), &mut o, &mut e);
        assert_eq!(wrote, 0, "{}", String::from_utf8_lossy(&e));
        let first = sc.join("001.json").to_string_lossy().to_string();
        counted(p, "run", || ritsu::run::command(&args(&[flow, "--scenario", &first]), Lang::En), &mut seen);
    }
    for (p, name) in [
        ("crates/ritsu/tests/projects/invoice", "invoice"),
        ("crates/ritsu/tests/projects/shop", "shop"),
        ("crates/ritsu/tests/projects/settlement", "settlement"),
        ("crates/ritsu/tests/projects/catalog", "catalog"),
        ("crates/ritsu/tests/projects/product_line", "product_line"),
        ("crates/rulec/tests/modules/en/app", "app"),
    ] {
        let to = out.path().join(name).to_string_lossy().to_string();
        counted_failing(p, "gen", || ritsu::package::command(&args(&["--root", ".", "--out", &to, "--target", "typescript", "--target", "python"]), Lang::En), &mut seen);
    }
    // a language's own commands, as `ritsu <language> …` runs them: the page, the api, the scenarios,
    // the run of one of them and the code of a file that reads other languages' files
    let quiet = |run: &dyn Fn(&mut Vec<u8>, &mut Vec<u8>) -> u8| {
        let (mut o, mut e) = (Vec::new(), Vec::new());
        run(&mut o, &mut e)
    };
    for (p, flow) in [("crates/dandori/examples/hotel/temporal", "hotel.flow"), ("crates/sakai/examples/shop/ordering", "fulfillment.flow"), ("modules/fmla", "fmla_request.flow")] {
        let sc = out.path().join("scenarios").join(flow).to_string_lossy().to_string();
        let to = out.path().join("flows").join(flow).to_string_lossy().to_string();
        let first = out.path().join(flow.trim_end_matches(".flow")).join("001.json").to_string_lossy().to_string();
        for (what, a) in [
            ("dandori doc", vec!["doc", flow]),
            ("dandori api", vec!["api", flow]),
            ("dandori scenarios", vec!["scenarios", flow, "--out", &sc]),
            ("dandori run", vec!["run", flow, "--scenario", &first]),
            ("dandori gen", vec!["gen", flow, "--target", "temporal-typescript", "--out", &to]),
        ] {
            counted(p, what, || quiet(&|o, e| ritsu::languages::dandori(&args(&a), o, e)), &mut seen);
        }
    }
    // a flow with an arm of a `match` that rulec says no run takes: the scenarios, and the page that
    // shows them, ask rulec what the rule comes to with what the call can give it, once
    let sc = out.path().join("scenarios").join("tool_orders").to_string_lossy().to_string();
    for (what, a) in [("dandori doc", vec!["doc", "tool_orders.flow"]), ("dandori scenarios", vec!["scenarios", "tool_orders.flow", "--out", &sc])] {
        counted("crates/ritsu/tests/projects/rule_arms", what, || quiet(&|o, e| ritsu::languages::dandori(&args(&a), o, e)), &mut seen);
    }
    let to = out.path().join("gate").to_string_lossy().to_string();
    for (what, a) in [("sekisho doc", vec!["doc", "approvals.gate"]), ("sekisho api", vec!["api", "approvals.gate"]), ("sekisho gen", vec!["gen", "approvals.gate", "--target", "typescript", "--out", &to])] {
        counted("crates/sekisho/examples/approvals", what, || quiet(&|o, e| sekisho::run::run(&args(&a), ritsu_project::Joined::new().sekisho().into(), o, e)), &mut seen);
    }
    for cmd in ["doc", "api"] {
        counted("crates/kaname/examples/payout", &format!("kaname {cmd}"), || quiet(&|o, e| ritsu::languages::kaname(&args(&[cmd, "payout.kn"]), o, e)), &mut seen);
        counted("crates/yuen/examples/refunds", &format!("yuen {cmd}"), || quiet(&|o, e| yuen::run::run(&args(&[cmd, "refunds.req"]), ritsu_project::Joined::new().yuen(), o, e)), &mut seen);
        counted("crates/sakai/examples/shop", &format!("sakai {cmd}"), || quiet(&|o, e| sakai::run::run(&args(&[cmd, "shop.ctx"]), ritsu_project::Joined::new().sakai(), o, e)), &mut seen);
        counted("crates/rulec/tests/days", &format!("rulec {cmd}"), || {
            rulec::days::with(Some(ritsu_project::Joined::dates()), || rulec::cli::run(args(&[cmd, "settlement.rule"])));
            0
        }, &mut seen);
        counted("crates/koyomi/examples", &format!("koyomi {cmd}"), || {
            koyomi::run::run(args(&[cmd, "net30.cal"]));
            0
        }, &mut seen);
        counted("crates/chobo/examples/refunds", &format!("chobo {cmd}"), || {
            chobo::run::run(args(&[cmd, "refunds.book"]));
            0
        }, &mut seen);
    }
    // ringi's examples (ringi's DESIGN 16.1): five requests, each in English and in Japanese, read
    // one organization, one file of masters, one shared route and the calendars, books, rules, flows
    // and gates beside them. Each file is checked once, and so are the files each request is lowered
    // to (laid where the check lays them) and the data the organization and the masters read
    // (`ringi.data`): in the check of the directory, in the package, and in each of ringi's commands
    let examples = "crates/ringi/examples";
    counted(examples, "check", || check(&["."]), &mut seen);
    counted(examples, "check of two requests", || check(&["expense.ringi", "expense.ja.ringi"]), &mut seen);
    // every request held to being usable through a day: the check with the day is the one check of
    // each request, which the checks across the languages read too
    counted(examples, "check through a day", || check(&[".", "--through", "2027-06-30"]), &mut seen);
    let to = out.path().join("ringi").to_string_lossy().to_string();
    let catalog = "https://requests.example.com/a2ui/catalog.json";
    counted(examples, "gen", || ritsu::package::command(&args(&["--root", ".", "--out", &to, "--target", "typescript", "--target", "python", "--target", "requests", "--catalog", catalog]), Lang::En), &mut seen);
    // the screens too, whose functions read each kind's values.rule as the package does
    if !Path::new(&to).join("requests/a2ui/functions.js").is_file() {
        seen.push(format!("gen in {examples}: the screens are not written"));
    }
    // the requests alone: the functions are made from the checks the run made, with no package
    // before them to have read the rules
    let alone = out.path().join("ringi-requests").to_string_lossy().to_string();
    counted(examples, "gen of the requests alone", || ritsu::package::command(&args(&["--root", ".", "--out", &alone, "--target", "requests", "--catalog", catalog]), Lang::En), &mut seen);
    let functions = |dir: &str| std::fs::read_to_string(Path::new(dir).join("requests/a2ui/functions.js")).unwrap_or_default();
    if functions(&alone).is_empty() || functions(&alone) != functions(&to) {
        seen.push(format!("gen in {examples}: the functions of the screens the requests alone write are not those written beside the packages"));
    }
    // and in Japanese: ringi asks rulec of the files a request is lowered to in the run's language
    // first, and rulec reads in the run's language on the run's thread, so that the claims asked of
    // those files after their check read the `enums` file they import from what the check kept: a
    // run in Japanese checks each of them once too (DESIGN 4.15)
    counted(examples, "check in Japanese", || {
        let (mut o, mut e) = (Vec::new(), Vec::new());
        ritsu::check::run(&args(&["."]), Lang::Ja, &mut o, &mut e)
    }, &mut seen);
    let ja = out.path().join("ringi-ja").to_string_lossy().to_string();
    counted(examples, "gen in Japanese", || ritsu::package::command(&args(&["--root", ".", "--out", &ja, "--target", "typescript", "--target", "python", "--target", "requests", "--catalog", catalog]), Lang::Ja), &mut seen);
    let alone_ja = out.path().join("ringi-requests-ja").to_string_lossy().to_string();
    counted(examples, "gen of the requests alone in Japanese", || ritsu::package::command(&args(&["--root", ".", "--out", &alone_ja, "--target", "requests", "--catalog", catalog]), Lang::Ja), &mut seen);
    for dir in [&ja, &alone_ja] {
        if functions(dir) != functions(&to) {
            seen.push(format!("gen in {examples}: the functions of the screens a run in Japanese writes ({dir}) are not those of a run in English"));
        }
    }
    let extracted = out.path().join("expense_extracted").to_string_lossy().to_string();
    for (what, a) in [
        ("ringi check", vec!["check", "."]),
        ("ringi check in Japanese", vec!["check", ".", "--lang", "ja"]),
        ("ringi check of two requests", vec!["check", "leave.ringi", "leave.ja.ringi", "shared/company.ringi"]),
        ("ringi run", vec!["run", "expense.ringi"]),
        ("ringi simulate", vec!["simulate", "expense.ringi", "--requester", "e1201", "--on", "2027-04-01"]),
        ("ringi vectors", vec!["vectors", "leave.ringi"]),
        ("ringi api", vec!["api", "expense.ringi"]),
        ("ringi extract", vec!["extract", "expense.ringi", "--out", &extracted]),
    ] {
        counted(examples, what, || quiet(&|o, e| ritsu::languages::ringi(&args(&a), o, e)), &mut seen);
    }
    // a run in Japanese where rulec's port is asked of the rule a route applies before anything shows
    // that rule's own findings (ringi pins the rule as it lowers the request), and they are shown
    // after in Japanese: the port checks the rule in the run's language, and the findings are read
    // off that check (rulec's §15.320). One run's engines answer both, ringi's check of the request
    // and then rulec's check of the rule, as in a run: a check is kept for the engines that made it
    // (the port of dates it read through is theirs), so the engines of two runs would each check the
    // rule, in any language. ringi's check is to ask the port, which checks the rule once, and
    // rulec's check after it is not to check it again. On a thread of its own, as `failing` asks, so
    // that no check kept on this one answers for it
    let dir = tree().join(examples);
    let (passed, by_ringi, by_rulec, more) = std::thread::spawn(move || {
        std::env::set_current_dir(&dir).unwrap();
        ritsu_base::tally::take();
        let at = ritsu_base::paths::opened(&dir.join("shared/authority.rule"));
        let rule = |all: &[(&str, PathBuf, usize)]| all.iter().filter(|(t, f, _)| *t == "rulec" && *f == at).map(|(_, _, n)| *n).sum::<usize>();
        let twice = |all: &[(&str, PathBuf, usize)]| all.iter().filter(|(_, _, n)| *n > 1).map(|(t, f, n)| format!("{t} {} x{n}", f.display())).collect::<Vec<String>>();
        let joined = ritsu_project::Joined::new();
        let ja = rulec::i18n::Lang::Ja;
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let passed = rulec::i18n::with(ja, || ringi::run::run(&args(&["check", "purchase.ja.ringi", "--lang", "ja"]), joined.ringi(), &mut o, &mut e)) == 0;
        let by_ringi = taken();
        let _ = rulec::i18n::with(ja, || joined.rulec.checked(Path::new("."), &args(&["shared/authority.rule"]), Lang::Ja));
        let by_rulec = taken();
        let mut more = twice(&by_ringi);
        more.extend(twice(&by_rulec));
        (passed, rule(&by_ringi), rule(&by_rulec), more)
    })
    .join()
    .unwrap();
    if !passed || by_ringi != 1 || by_rulec != 0 || !more.is_empty() {
        seen.push(format!(
            "ringi check, then the rule it applies, with one run's engines in Japanese in {examples}: the request passes: {passed}; the rule checked {by_ringi} times in ringi's check and {by_rulec} times after it; {more:?}"
        ));
    }
    // `ritsu impact`'s `ritsu gen --check` (`package::fresh`) of a module of requests whose `target`
    // line names a language and `requests`, in Japanese and in English, the thread reading rulec in
    // neither before: the files a request is lowered to are checked once there too, and the
    // packages `ritsu gen` wrote in the same language are found fresh (a package's comments are
    // in the language of the run that writes it)
    let module = out.path().join("ringi-module");
    for dir in ["shared", "org", "masters"] {
        ritsu_testkit::tmp::copy_dir(&tree().join(examples).join(dir), &module.join(dir));
    }
    for file in ["leave.ringi", "expense.ja.ringi"] {
        std::fs::copy(tree().join(examples).join(file), module.join(file)).unwrap();
    }
    std::fs::write(module.join("ritsu.mod"), format!("module example.com/acme/requests\nversion 1.0.0\ntarget typescript requests\ncatalog \"{catalog}\"\n")).unwrap();
    let at = module.to_string_lossy().into_owned();
    for lang in [Lang::Ja, Lang::En] {
        std::env::set_current_dir(&module).unwrap();
        assert_eq!(ritsu::package::command(&args(&["--root", "."]), lang), 0, "gen of the module of requests ({lang:?})");
        let mut same = false;
        counted(&at, &format!("impact's gen --check ({lang:?})"), || {
            same = matches!(ritsu::package::fresh(&module, lang), ritsu::package::Fresh::Same);
            0
        }, &mut seen);
        if !same {
            seen.push(format!("impact's gen --check ({lang:?}) does not find the packages gen wrote fresh"));
        }
    }
    // a run of `requests` whose flow reads the API it writes (ringi's DESIGN 9.3): the leave request
    // starts a flow that reads its ticket through the API, and the gate it reads names that flow's
    // workflow. The API is made from the requests before the check, with their flows and gates
    // taken to pass, unread, by the run's ringi, which keeps what it checks so where they pass: every
    // file is checked once, the request and the data it reads, the gate, the rules, the calendars,
    // the books and the flow among them
    let reads = out.path().join("ringi-api");
    for dir in ["shared", "org", "masters"] {
        ritsu_testkit::tmp::copy_dir(&tree().join(examples).join(dir), &reads.join(dir));
    }
    std::fs::create_dir_all(reads.join("flows")).unwrap();
    std::fs::write(reads.join("flows/follow.flow"), FOLLOW_FLOW).unwrap();
    let leave = std::fs::read_to_string(tree().join(examples).join("leave.ringi")).unwrap();
    let started = leave
        .replacen("use route standard", "use flow follow from \"flows/follow.flow\"\nuse route standard", 1)
        .replacen("  on approved    post", "  on approved    launch follow(request_id: ticket.id)\n  on approved    post", 1);
    let gate = std::fs::read_to_string(reads.join("shared/company.gate")).unwrap();
    let named = gate.replacen("resource Team\n", "workflow follow from \"../flows/follow.flow\"\n  description \"Reads the ticket it is started for through the server\"\n\nresource Team\n", 1);
    if !started.contains("use flow follow") || !started.contains("launch follow(") || named == gate {
        seen.push("the leave example or the gate is not as the run whose flow reads the API reads it".to_string());
    }
    std::fs::write(reads.join("leave.ringi"), &started).unwrap();
    std::fs::write(reads.join("shared/company.gate"), &named).unwrap();
    let at = reads.to_string_lossy().into_owned();
    let mut code = 1;
    counted(&at, "gen of requests whose flow reads the API it writes", || {
        code = ritsu::package::command(&args(&["leave.ringi", "--root", ".", "--out", "out", "--target", "requests", "--catalog", catalog]), Lang::En);
        code
    }, &mut seen);
    if code != 0 || !reads.join("out/requests/openapi.json").is_file() {
        seen.push(format!("gen in {at}: the run whose flow reads the API it writes does not write it ({code})"));
    }
    // the same project with the flow reading another API, one the run does not write (the API the
    // run above wrote, copied): nothing is made before the check, and every file is checked once
    let other = out.path().join("ringi-other-api");
    for dir in ["shared", "org", "masters", "flows"] {
        ritsu_testkit::tmp::copy_dir(&reads.join(dir), &other.join(dir));
    }
    std::fs::copy(reads.join("leave.ringi"), other.join("leave.ringi")).unwrap();
    std::fs::create_dir_all(other.join("api")).unwrap();
    std::fs::copy(reads.join("out/requests/openapi.json"), other.join("api/requests.json")).unwrap_or_default();
    std::fs::write(other.join("flows/follow.flow"), FOLLOW_FLOW.replace("../out/requests/openapi.json", "../api/requests.json")).unwrap();
    let at = other.to_string_lossy().into_owned();
    let mut code = 1;
    counted(&at, "gen of requests whose flow reads another API", || {
        code = ritsu::package::command(&args(&["leave.ringi", "--root", ".", "--out", "out", "--target", "requests", "--catalog", catalog]), Lang::En);
        code
    }, &mut seen);
    if code != 0 {
        seen.push(format!("gen in {at}: the run whose flow reads another API does not pass ({code})"));
    }
    // the page in the browser (ritsu-wasm, DESIGN 8.7), on the shop it opens on: its one question
    // checks each file once, the check of the project (the text and the JSON), the page of a file of
    // each language that draws one (as HTML and as Markdown) and the code of a flow and of a gate
    let shop = tree().join("website/playground/shop");
    let ask = |path: &str| ritsu_wasm::playground::Request { files: files_of(&shop), lang: Lang::En, path: path.to_string(), target: None };
    // the shop opens on a rule that does not pass rulec's check, which is checked once in each
    // language: the other languages are handed its errors in both (DESIGN 4.15)
    let refused = failing(&shop);
    assert!(!refused.is_empty(), "the shop the page opens on has no rule that fails rulec's check");
    counted_but("website/playground/shop", "the page's check", || {
        ritsu_wasm::playground::check(&ask(""));
        0
    }, &|tool, file, n| tool == "rulec" && n == 2 && refused.iter().any(|r| file.ends_with(r)), &mut seen);
    for path in ["billing/rules/billing_need.rule", "billing/net30.cal", "warehouse/inventory.book", "ordering/ship_order.flow", "gates/orders.gate"] {
        counted("website/playground/shop", &format!("the page's doc of {path}"), || {
            ritsu_wasm::playground::doc(&ask(path));
            0
        }, &mut seen);
    }
    for path in ["ordering/ship_order.flow", "gates/orders.gate"] {
        counted("website/playground/shop", &format!("the page's gen of {path}"), || {
            ritsu_wasm::playground::generate(&ask(path));
            0
        }, &mut seen);
    }
    std::env::set_current_dir(tree()).unwrap();
    assert!(seen.is_empty(), "files checked more than once:\n{}", seen.join("\n"));
}

/// A project in `dir` whose modules `a/x.kn` and `b/y.kn` read `shared/big.kn` as
/// `../shared/big.kn`, and whose `z.kn` reads it as `shared/big.kn`: the shared module calls a rule
/// and makes the transfers of a book, and holds many functions besides.
fn shared_module(dir: &Path) {
    let mut big = String::from(
        r#"module big v1
description "What a payment comes to for its seller: the fee by the rule, and the two transfers of the ledger"

use rule fee from "../rules/fee.rule"
use book ledger from "../books/ledger.book"

record Payment
  id     : string
  seller : string
  amount : money[USDc]  range >=1USDc <=100_000_000USDc

record Settlement
  capture : ledger.capture.do
  fee     : ledger.processing_fee.do

fn fee_of(payment: Payment) -> money[USDc]
  description "The rule's fee on the payment"
  fee(amount: payment.amount).fee

fn settle(payment: Payment) -> Settlement total
  description "The amount goes to the seller, and the fee goes from the seller to the processor"
  Settlement {
    capture: ledger.capture.do(payment: payment.id, seller: payment.seller, amount: payment.amount),
    fee: ledger.processing_fee.do(payment: payment.id, seller: payment.seller, fee: fee_of(payment: payment)),
  }
"#,
    );
    for i in 1..=24 {
        big.push_str(&format!(
            "\nfn part_{i}(amount: money[USDc] range >=1USDc <=1_000_000USDc, count: number range >=1 <=50) -> money[USDc]\n  description \"The amount times the count, and {i} cents more\"\n  amount * count + {i}USDc\n"
        ));
    }
    big.push_str("\nexamples\n  fee_of(payment: Payment { id: \"p_1\", seller: \"s_1\", amount: 10000USDc }) = 330USDc\n  part_1(amount: 100USDc, count: 2) = 201USDc\n");
    let files = [
        (
            "rules/fee.rule",
            "rule fee v1\ndescription \"The fee on one payment: 3% of the amount and a fixed 30 cents\"\n\ninputs\n  amount : money[USDc]  range >=1USDc <=100_000_000USDc\n\noutputs\n  fee : money[USDc]  round half_up(1USDc)\n\ndefine fee : money[USDc] = amount × 3% + 30USDc\n\nexamples\n| amount    | -> fee  |\n| 10000USDc | 330USDc |\n".to_string(),
        ),
        (
            "books/ledger.book",
            r#"book ledger v1
description "A seller's money, counted in cents: a payment comes in for the seller, and the fee goes out of the seller's balance"

unit USD scale 2

account seller_balance(seller: string) : USD
  description "what is held for a seller"
  at least 0 refused as insufficient_balance
account cards : USD outside
account processor : USD outside

transfer capture(payment: string, seller: string, amount: USD)
  description "a payment taken for the seller"
  key payment
  move amount from cards to seller_balance(seller)

transfer processing_fee(payment: string, seller: string, fee: USD)
  description "the fee on that payment, out of the seller's balance"
  key payment
  move fee from seller_balance(seller) to processor
"#
            .to_string(),
        ),
        ("shared/big.kn", big),
        (
            "a/x.kn",
            "module x v1\ndescription \"A payment settled by the shared module, from a directory of its own\"\n\nuse module big from \"../shared/big.kn\"\n\nfn settle_here(payment: big.Payment) -> big.Settlement total\n  description \"The shared settlement\"\n  big.settle(payment: payment)\n".to_string(),
        ),
        (
            "b/y.kn",
            "module y v1\ndescription \"The fee of a payment by the shared module, from another directory\"\n\nuse module big from \"../shared/big.kn\"\n\nfn fee_here(payment: big.Payment) -> money[USDc]\n  description \"The shared fee\"\n  big.fee_of(payment: payment)\n".to_string(),
        ),
        (
            "z.kn",
            "module z v1\ndescription \"A part of an amount by the shared module, from the project's root\"\n\nuse module big from \"shared/big.kn\"\n\nfn twice(amount: money[USDc] range >=1USDc <=1_000_000USDc) -> money[USDc]\n  description \"The first part, for two\"\n  big.part_1(amount: amount, count: 2)\n".to_string(),
        ),
    ];
    for (rel, text) in files {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
    }
}

/// Every file under `dir`, by its path from it with `/` between the parts, as the page is given a
/// project.
fn files_of(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                todo.push(p);
            } else {
                let rel = p.strip_prefix(dir).unwrap().components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
                out.push((rel, std::fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}
