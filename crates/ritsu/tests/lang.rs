//! What a command writes for others to keep is in the language `--lang` asks, English without it,
//! whatever the environment says (DESIGN 4.1, `Lang::made`): the code `gen` generates (its
//! comments, and the sentences the code says), a page, the JSON of `api` and `schema`, the vectors
//! and the scenarios, the adapter's template, the files of a new project. Each command of ritsu and
//! of its languages that writes is run on an example three times: with no `*_LANG` variable, with
//! every one of them `ja`, and with `--lang ja`.
//!
//! - What it writes with the variables is what it writes without them, byte for byte, and its exit
//!   is the same: every target of `ritsu gen`, with the gates' code asking Cedar and Verified
//!   Permissions, and the books' clients of each database; every target and extra of `rulec gen`,
//!   a rule written in Japanese, and the `.proto` with Connect.
//! - What it writes with `--lang ja` is Japanese, where its English holds sentences of ritsu's own;
//!   what holds none (its case says so: the clients of the books, `rulec graph`, `rulec fmt`) is the
//!   same whatever is asked.
//! - The text a run prints (`generated: …`) still follows the variables, and the help of `--lang`
//!   of a command that writes says both.
//!
//! The site of `ritsu mod site` is held to the same in tests/modules_site.rs.

mod common;

use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::process::Command;

const LANG_VARS: [&str; 11] = common::LANG_VARS;

/// Where a command puts what it writes.
#[derive(Clone, Copy, PartialEq)]
enum Writes {
    /// Under `--out <dir>`, which the run adds.
    Out,
    /// On standard output.
    Stdout,
    /// In the directory it runs in (`ritsu ringi init`), an empty one.
    Here,
    /// Over the file it is given (`rulec fmt`): a copy of it, in the directory written to.
    Over,
}

/// One command: a name for the messages, the directory it runs in (from the repository's root),
/// its arguments after `ritsu`, where it writes, and whether its `--lang ja` holds Japanese.
struct Case {
    name: &'static str,
    dir: &'static str,
    args: &'static [&'static str],
    writes: Writes,
    japanese: bool,
}

const fn case(name: &'static str, dir: &'static str, args: &'static [&'static str], writes: Writes, japanese: bool) -> Case {
    Case { name, dir, args, writes, japanese }
}

const RULES: &str = "crates/kaname/examples/payout/rules";
const PAYOUT: &str = "crates/kaname/examples/payout";

const CASES: &[Case] = &[
    // ritsu gen, each target
    case("gen approvals", "crates/ritsu/tests/projects/approvals", &["gen", ".", "--target", "typescript", "--target", "python", "--target", "go", "--target", "cedar"], Writes::Out, true),
    case("gen approvals avp", "crates/ritsu/tests/projects/approvals", &["gen", ".", "--target", "typescript", "--target", "python", "--target", "go", "--authorizer", "avp"], Writes::Out, true),
    case("gen marketplace", "crates/ritsu/tests/projects/marketplace", &["gen", ".", "--target", "rust"], Writes::Out, true),
    case("gen settlement", "crates/ritsu/tests/projects/settlement", &["gen", ".", "--target", "go", "--target", "lean"], Writes::Out, true),
    // the kinds of request are written in both languages whatever is asked
    case("gen requests", "crates/ritsu/tests/projects/requests-org", &["gen", ".", "--target", "requests", "--catalog", "https://example.com/c.json"], Writes::Out, false),
    // koyomi
    case("koyomi gen", "crates/koyomi/examples", &["koyomi", "gen", "close_20th_pay_10th.cal", "--target", "typescript", "--target", "python", "--target", "go", "--target", "rust", "--target", "sql", "--extra", "runner"], Writes::Out, true),
    case("koyomi doc", "crates/koyomi/examples", &["koyomi", "doc", "close_20th_pay_10th.cal"], Writes::Stdout, true),
    case("koyomi doc html", "crates/koyomi/examples", &["koyomi", "doc", "close_20th_pay_10th.cal", "--format", "html"], Writes::Stdout, true),
    case("koyomi api", "crates/koyomi/examples", &["koyomi", "api", "close_20th_pay_10th.cal"], Writes::Stdout, false),
    case("koyomi vectors", "crates/koyomi/examples", &["koyomi", "vectors", "close_20th_pay_10th.cal"], Writes::Stdout, false),
    // chobo
    case("chobo gen", "crates/chobo/examples/refunds", &["chobo", "gen", "refunds.book", "--target", "sql", "--target", "typescript", "--target", "python", "--target", "go", "--target", "rust"], Writes::Out, false),
    case("chobo gen tigerbeetle", "crates/chobo/examples/refunds", &["chobo", "gen", "refunds.book", "--target", "typescript", "--books", "tigerbeetle"], Writes::Out, false),
    case("chobo gen mysql", "crates/chobo/examples/refunds", &["chobo", "gen", "refunds.book", "--target", "sql", "--target", "go", "--books", "mysql"], Writes::Out, false),
    case("chobo doc", "crates/chobo/examples/refunds", &["chobo", "doc", "refunds.book"], Writes::Stdout, true),
    case("chobo doc html", "crates/chobo/examples/refunds", &["chobo", "doc", "refunds.book", "--format", "html"], Writes::Out, true),
    case("chobo api", "crates/chobo/examples/refunds", &["chobo", "api", "refunds.book"], Writes::Stdout, false),
    case("chobo scenarios", "crates/chobo/examples/refunds", &["chobo", "scenarios", "refunds.book"], Writes::Out, false),
    // dandori
    case("dandori gen aws", "crates/dandori/examples/order/aws", &["dandori", "gen", "order.flow", "--target", "asl", "--target", "durable"], Writes::Out, false),
    case("dandori gen temporal", "crates/dandori/examples/order/temporal", &["dandori", "gen", "order.flow", "--target", "temporal-typescript", "--target", "temporal-python", "--target", "temporal-go"], Writes::Out, false),
    case("dandori gen argo", "crates/dandori/examples/fulfillment", &["dandori", "gen", "arrange_delivery.flow", "--target", "argo"], Writes::Out, false),
    case("dandori doc", "crates/dandori/examples/order/temporal", &["dandori", "doc", "order.flow"], Writes::Stdout, true),
    case("dandori doc html", "crates/dandori/examples/order/temporal", &["dandori", "doc", "order.flow", "--format", "html"], Writes::Out, true),
    case("dandori api", "crates/dandori/examples/order/temporal", &["dandori", "api", "order.flow"], Writes::Stdout, false),
    case("dandori scenarios", "crates/dandori/examples/order/temporal", &["dandori", "scenarios", "order.flow"], Writes::Out, false),
    // sekisho
    case("sekisho gen", "crates/sekisho/examples/refunds", &["sekisho", "gen", "refunds.gate", "--target", "cedar", "--target", "typescript", "--target", "python", "--target", "go"], Writes::Out, true),
    case("sekisho gen avp", "crates/sekisho/examples/refunds", &["sekisho", "gen", "refunds.gate", "--target", "go", "--authorizer", "avp"], Writes::Out, true),
    case("sekisho doc", "crates/sekisho/examples/refunds", &["sekisho", "doc", "refunds.gate"], Writes::Stdout, true),
    case("sekisho doc html", "crates/sekisho/examples/refunds", &["sekisho", "doc", "refunds.gate", "--format", "html"], Writes::Stdout, true),
    case("sekisho api", "crates/sekisho/examples/refunds", &["sekisho", "api", "refunds.gate"], Writes::Stdout, false),
    case("sekisho vectors", "crates/sekisho/examples/refunds", &["sekisho", "vectors", "refunds.gate"], Writes::Stdout, false),
    // sakai
    case("sakai gen", "crates/sakai/examples/shop", &["sakai", "gen", "shop.ctx", "--target", "import-linter", "--target", "dependency-cruiser", "--target", "archunit", "--target", "go-arch-lint"], Writes::Out, true),
    case("sakai export", "crates/sakai/examples/shop", &["sakai", "export", "cml", "shop.ctx"], Writes::Stdout, true),
    case("sakai doc", "crates/sakai/examples/shop", &["sakai", "doc", "shop.ctx"], Writes::Stdout, true),
    case("sakai doc html", "crates/sakai/examples/shop", &["sakai", "doc", "shop.ctx", "--format", "html"], Writes::Out, true),
    case("sakai api", "crates/sakai/examples/shop", &["sakai", "api", "shop.ctx"], Writes::Stdout, false),
    case("sakai vectors", "crates/sakai/examples/shop", &["sakai", "vectors", "shop.ctx"], Writes::Stdout, false),
    // yuen
    case("yuen doc", "crates/yuen/examples/payment_terms", &["yuen", "doc", "payment_terms.req"], Writes::Stdout, true),
    case("yuen doc html", "crates/yuen/examples/payment_terms", &["yuen", "doc", "payment_terms.req", "--format", "html"], Writes::Out, true),
    case("yuen api", "crates/yuen/examples/payment_terms", &["yuen", "api", "payment_terms.req"], Writes::Stdout, true),
    case("yuen export reqif", "crates/yuen/examples/payment_terms", &["yuen", "export", "reqif", "payment_terms.req"], Writes::Stdout, false),
    case("yuen export prov", "crates/yuen/examples/payment_terms", &["yuen", "export", "prov", "payment_terms.req"], Writes::Stdout, false),
    // ringi
    case("ringi doc", "crates/ringi/examples", &["ringi", "doc", "leave.ringi"], Writes::Stdout, true),
    case("ringi doc html", "crates/ringi/examples", &["ringi", "doc", "leave.ringi", "--format", "html"], Writes::Stdout, true),
    case("ringi doc longest", "crates/ringi/examples", &["ringi", "doc", "leave.ringi", "--longest"], Writes::Stdout, true),
    // the two versions alike, which exits 0: the page and the JSON follow `--lang` alone
    case("ringi diff", "crates/ringi/examples", &["ringi", "diff", "leave.ringi", "leave.ringi", "--format", "markdown"], Writes::Stdout, true),
    case("ringi diff html", "crates/ringi/examples", &["ringi", "diff", "leave.ringi", "leave.ringi", "--format", "html"], Writes::Stdout, true),
    case("ringi diff json", "crates/ringi/examples", &["ringi", "diff", "leave.ringi", "leave.ringi", "--format", "json"], Writes::Stdout, false),
    case("ringi diff versions", "crates/ringi/examples", &["ringi", "diff", "--versions", "2026-04-01", "2026-05-01", "expense.ringi", "--format", "markdown"], Writes::Stdout, true),
    case("ringi api", "crates/ringi/examples", &["ringi", "api", "leave.ringi"], Writes::Stdout, true),
    case("ringi vectors", "crates/ringi/examples", &["ringi", "vectors", "leave.ringi"], Writes::Stdout, false),
    case("ringi extract", "crates/ringi/examples", &["ringi", "extract", "leave.ringi"], Writes::Out, false),
    case("ringi init", "", &["ringi", "init", "proj", "--module", "example.com/p"], Writes::Here, true),
    // rulec gen, every target and extra
    case(
        "rulec gen",
        RULES,
        &[
            "rulec", "gen", "card_fee.rule", "--target", "python", "--target", "numpy", "--target", "typescript", "--target", "javascript", "--target", "rust", "--target", "ruby",
            "--target", "php", "--target", "go", "--target", "swift", "--target", "java", "--target", "sql", "--target", "wasm", "--target", "proto", "--extra", "runner", "--extra",
            "mcp", "--extra", "round-test", "--extra", "vectors",
        ],
        Writes::Out,
        true,
    ),
    case("rulec gen ja", RULES, &["rulec", "gen", "カード手数料.rule", "--target", "python", "--target", "typescript", "--target", "go", "--target", "rust", "--extra", "runner", "--extra", "mcp"], Writes::Out, true),
    case("rulec gen connect", RULES, &["rulec", "gen", "card_fee.rule", "--target", "python", "--target", "proto", "--extra", "connect"], Writes::Out, true),
    case("rulec doc", RULES, &["rulec", "doc", "card_fee.rule"], Writes::Stdout, true),
    case("rulec doc html", RULES, &["rulec", "doc", "card_fee.rule", "--format", "html"], Writes::Stdout, true),
    case("rulec doc customer", RULES, &["rulec", "doc", "card_fee.rule", "--audience", "customer"], Writes::Stdout, true),
    case("rulec api", RULES, &["rulec", "api", "card_fee.rule"], Writes::Stdout, true),
    case("rulec vectors", RULES, &["rulec", "vectors", "card_fee.rule"], Writes::Stdout, true),
    case("rulec schema", RULES, &["rulec", "schema", "card_fee.rule"], Writes::Stdout, true),
    case("rulec adapter", RULES, &["rulec", "adapter", "card_fee.rule"], Writes::Stdout, true),
    // what holds no sentence of ritsu's
    case("rulec graph", RULES, &["rulec", "graph", "card_fee.rule"], Writes::Stdout, false),
    case("rulec certificate", RULES, &["rulec", "certificate", "card_fee.rule"], Writes::Stdout, false),
    case("rulec fmt", RULES, &["rulec", "fmt", "card_fee.rule"], Writes::Over, false),
    // kaname
    case("kaname doc", PAYOUT, &["kaname", "doc", "payout.kn"], Writes::Stdout, true),
    case("kaname doc html", PAYOUT, &["kaname", "doc", "payout.kn", "--format", "html"], Writes::Stdout, true),
    case("kaname vectors", PAYOUT, &["kaname", "vectors", "payout.kn"], Writes::Stdout, true),
    case("kaname api", PAYOUT, &["kaname", "api", "payout.kn"], Writes::Stdout, false),
];

/// What one run left: its exit, its standard output and error, and the files it wrote (each path
/// from the directory written to, and the bytes).
struct Run {
    code: i32,
    stdout: String,
    stderr: String,
    files: Vec<(String, Vec<u8>)>,
}

impl Run {
    /// What the run wrote for others to keep: its files, or its standard output.
    fn written(&self, writes: Writes) -> Vec<(String, Vec<u8>)> {
        match writes {
            Writes::Stdout => vec![("<stdout>".to_string(), self.stdout.clone().into_bytes())],
            Writes::Out | Writes::Here | Writes::Over => self.files.clone(),
        }
    }
}

fn files_under(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                out.push((p.strip_prefix(root).unwrap().to_string_lossy().into_owned(), std::fs::read(&p).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// Runs `c` into the scratch directory `out`, with the variables `vars` and `more` arguments.
fn run(c: &Case, out: &Path, vars: &[(&str, &str)], more: &[&str]) -> Run {
    std::fs::create_dir_all(out).unwrap();
    let dir: PathBuf = match c.writes {
        Writes::Here => out.to_path_buf(),
        // a command that writes over its file is given a copy of it, where it writes
        Writes::Over => {
            let file = c.args.last().unwrap();
            std::fs::copy(common::root().join(c.dir).join(file), out.join(file)).unwrap();
            out.to_path_buf()
        }
        Writes::Out | Writes::Stdout => common::root().join(c.dir),
    };
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    cmd.current_dir(&dir).args(c.args).args(more);
    if c.writes == Writes::Out {
        cmd.arg("--out").arg(out);
    }
    for v in LANG_VARS {
        cmd.env_remove(v);
    }
    for (k, v) in vars {
        cmd.env(k, v);
    }
    let o = cmd.output().unwrap_or_else(|e| panic!("{}: {e}", c.name));
    let shown = |b: &[u8]| String::from_utf8_lossy(b).replace(&out.display().to_string(), "<out>");
    Run { code: o.status.code().unwrap_or(-1), stdout: shown(&o.stdout), stderr: shown(&o.stderr), files: files_under(out) }
}

/// Whether a text holds hiragana: Japanese, where the English is in ritsu's own words.
fn japanese(bytes: &[u8]) -> bool {
    String::from_utf8_lossy(bytes).chars().any(|ch| ('\u{3041}'..='\u{309f}').contains(&ch))
}

/// The paths of two lists of files that differ, or that one has and the other has not.
fn differing(a: &[(String, Vec<u8>)], b: &[(String, Vec<u8>)]) -> Vec<String> {
    let mut out: Vec<String> = a.iter().filter(|(p, x)| b.iter().find(|(q, _)| q == p).is_none_or(|(_, y)| y != x)).map(|(p, _)| p.clone()).collect();
    out.extend(b.iter().filter(|(p, _)| !a.iter().any(|(q, _)| q == p)).map(|(p, _)| p.clone()));
    out
}

/// The three runs of one case, and what is wrong with them.
fn held(c: &Case, at: &Path) -> Vec<String> {
    let every_ja: Vec<(&str, &str)> = LANG_VARS.iter().map(|v| (*v, "ja")).collect();
    let plain = run(c, &at.join("plain"), &[], &[]);
    let vars = run(c, &at.join("vars"), &every_ja, &[]);
    let ja = run(c, &at.join("ja"), &[], &["--lang", "ja"]);
    let mut wrong = Vec::new();
    if plain.code != 0 || ja.code != 0 {
        wrong.push(format!("{}: exits {} and {} with --lang ja:\n{}{}", c.name, plain.code, ja.code, plain.stderr, ja.stderr));
        return wrong;
    }
    let written = plain.written(c.writes);
    if written.is_empty() || written.iter().all(|(_, b)| b.is_empty()) {
        wrong.push(format!("{}: wrote nothing", c.name));
    }
    if vars.code != plain.code {
        wrong.push(format!("{}: exits {} with every *_LANG=ja, {} without", c.name, vars.code, plain.code));
    }
    let changed = differing(&written, &vars.written(c.writes));
    if !changed.is_empty() {
        wrong.push(format!("{}: every *_LANG=ja changes what it writes: {}", c.name, changed.join(", ")));
    }
    let in_ja = ja.written(c.writes);
    let by_lang = differing(&written, &in_ja);
    if c.japanese && (by_lang.is_empty() || !in_ja.iter().any(|(_, b)| japanese(b))) {
        wrong.push(format!("{}: --lang ja writes no Japanese", c.name));
    }
    if !c.japanese && !by_lang.is_empty() {
        wrong.push(format!("{}: --lang ja changes what it writes, which holds no sentence of ritsu's: {}", c.name, by_lang.join(", ")));
    }
    // what a run of ritsu gen and rulec gen prints is text, which the variables choose
    if (c.args[0] == "gen" || c.args[..2] == ["rulec", "gen"]) && (japanese(plain.stdout.as_bytes()) || !japanese(vars.stdout.as_bytes())) {
        wrong.push(format!("{}: what it prints does not follow the variables:\n{}\n{}", c.name, plain.stdout, vars.stdout));
    }
    wrong
}

#[test]
fn the_variables_change_nothing_a_command_writes_and_lang_ja_still_writes_japanese() {
    // every target of ritsu gen, every language that writes
    let gen_targets: Vec<&str> = CASES.iter().filter(|c| c.args[0] == "gen").flat_map(|c| c.args.windows(2).filter(|w| w[0] == "--target").map(|w| w[1])).collect();
    for target in ["typescript", "python", "go", "rust", "cedar", "lean", "requests"] {
        assert!(gen_targets.contains(&target), "no case of ritsu gen --target {target}");
    }
    for tool in ["koyomi", "chobo", "dandori", "sekisho", "sakai", "yuen", "ringi"] {
        assert!(CASES.iter().any(|c| c.args[0] == tool), "no case of {tool}");
    }
    // every command of rulec and kaname that writes, and those that write nothing in a language
    for (tool, commands) in [("rulec", &["gen", "doc", "api", "vectors", "schema", "adapter", "graph", "certificate", "fmt"][..]), ("kaname", &["doc", "vectors", "api"][..])] {
        for command in commands {
            assert!(CASES.iter().any(|c| c.args[0] == tool && c.args[1] == *command), "no case of {tool} {command}");
        }
    }
    let t = TempDir::new("written-lang");
    // a few cases at a time: each run is a process of its own
    let next = std::sync::atomic::AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
    let failures: Vec<String> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                let (t, next) = (&t, &next);
                s.spawn(move || {
                    let mut wrong = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        let Some(c) = CASES.get(i) else { break };
                        wrong.extend(held(c, &t.path().join(i.to_string())));
                    }
                    wrong
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The help of `--lang` of a command that writes says that the flag alone gives the language of
/// what it writes, and the variables the language of the text; a command that only prints keeps
/// the help that names the variables.
#[test]
fn the_help_of_lang_says_what_it_chooses() {
    let help = |args: &[&str], vars: &[(&str, &str)]| {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
        c.args(args).arg("--help");
        for v in LANG_VARS {
            c.env_remove(v);
        }
        for (k, v) in vars {
            c.env(k, v);
        }
        let o = c.output().unwrap();
        assert_eq!(o.status.code(), Some(0), "{args:?}");
        String::from_utf8_lossy(&o.stdout).into_owned()
    };
    for (args, var) in [
        (&["gen"][..], "RITSU_LANG"),
        (&["ringi", "init"][..], "RINGI_LANG"),
        (&["koyomi", "gen"][..], "KOYOMI_LANG"),
        (&["koyomi", "doc"][..], "KOYOMI_LANG"),
        (&["chobo", "doc"][..], "CHOBO_LANG"),
        (&["dandori", "doc"][..], "DANDORI_LANG"),
        (&["sekisho", "gen"][..], "SEKISHO_LANG"),
        (&["sakai", "doc"][..], "SAKAI_LANG"),
        (&["yuen", "doc"][..], "YUEN_LANG"),
        (&["ringi", "api"][..], "RINGI_LANG"),
        (&["rulec", "gen"][..], "RULEC_LANG"),
        (&["rulec", "doc"][..], "RULEC_LANG"),
        (&["rulec", "api"][..], "RULEC_LANG"),
        (&["rulec", "vectors"][..], "RULEC_LANG"),
        (&["rulec", "schema"][..], "RULEC_LANG"),
        (&["rulec", "adapter"][..], "RULEC_LANG"),
        (&["kaname", "doc"][..], "KANAME_LANG"),
        (&["kaname", "vectors"][..], "KANAME_LANG"),
        (&["ringi", "doc"][..], "RINGI_LANG"),
        (&["ringi", "diff"][..], "RINGI_LANG"),
    ] {
        let en = help(args, &[]);
        assert!(en.contains("what it writes is in en, whatever the environment says") && en.contains(var), "ritsu {args:?} --help:\n{en}");
        assert_eq!(en.matches("--lang ja|en  ").count(), 1, "ritsu {args:?} --help says --lang once:\n{en}");
        // the help is text: the variable chooses its language
        let ja = help(args, &[(var, "ja")]);
        assert!(ja.contains("書く物は en（環境変数では変わらない）"), "ritsu {args:?} --help with {var}=ja:\n{ja}");
    }
    // the code of chobo's books and dandori's workflows holds no sentence of ritsu's: their `gen`
    // takes the `--lang` of the text, as a command that only prints does
    for (args, var) in [(&["koyomi", "check"][..], "KOYOMI_LANG"), (&["chobo", "gen"][..], "CHOBO_LANG"), (&["dandori", "gen"][..], "DANDORI_LANG")] {
        let text = help(args, &[]);
        assert!(text.contains(&format!("the language of the text; else the {var} environment variable")) && !text.contains("what it writes"), "ritsu {args:?} --help:\n{text}");
    }
    for (args, var) in [(&["rulec", "check"][..], "RULEC_LANG"), (&["rulec", "graph"][..], "RULEC_LANG"), (&["kaname", "check"][..], "KANAME_LANG"), (&["kaname", "api"][..], "KANAME_LANG")] {
        let text = help(args, &[]);
        assert!(text.contains(var) && !text.contains("what it writes"), "ritsu {args:?} --help:\n{text}");
    }
}
