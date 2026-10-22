//! `kaname run` (DESIGN 11.1): one function of each of the examples, in English and in
//! Japanese, on an input of `tests/inputs/`, as text with `--trace` and as JSON, held to
//! `tests/golden/run/` (`KANAME_BLESS=1` writes them again); and what stops the command.

mod common;

use kaname::suite::Suite;

fn run(args: &[&str], suite: Suite) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = kaname::run::run(&args, suite, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn joined(args: &[&str]) -> (u8, String, String) {
    run(args, common::joined())
}

/// The examples, each with the function run and its Japanese version's.
const RUNS: [(&str, &str, &str); 8] = [
    ("order_flow", "replay", "順に進める"),
    ("stock_diff", "changes", "違いの一覧"),
    ("handles", "tag", "タグを読む"),
    ("catalog", "paths", "パス"),
    ("payout", "settle", "精算する"),
    ("late_payment", "claim", "遅れの請求"),
    ("media_mail", "quote", "見積もる"),
    ("discount_split", "split", "按分する"),
];

#[test]
fn each_example_runs_as_the_golden_files_hold() {
    for (name, en, ja) in RUNS {
        for (kn, function, input, lang, golden) in [
            (format!("examples/{name}/{name}.kn"), en, format!("tests/inputs/{name}.json"), "en", name.to_string()),
            (format!("examples/{name}/{name}.ja.kn"), ja, format!("tests/inputs/{name}.ja.json"), "ja", format!("{name}.ja")),
        ] {
            let root = format!("examples/{name}");
            let (code, text, err) = joined(&["run", &kn, function, "--input", &input, "--trace", "--lang", lang]);
            assert_eq!(code, 0, "{kn}: {text}{err}");
            ritsu_testkit::golden(format!("tests/golden/run/{golden}.txt"), &text);
            let (code, json, err) = joined(&["run", &kn, function, "--input", &input, "--format", "json", "--root", &root, "--lang", lang]);
            assert_eq!(code, 0, "{kn}: {json}{err}");
            let json = json.replacen(&format!("\"kaname\": \"{}\"", env!("CARGO_PKG_VERSION")), "\"kaname\": \"<version>\"", 1);
            ritsu_testkit::golden(format!("tests/golden/run/{golden}.json"), &json);
        }
    }
}

#[test]
fn the_answer_of_a_run_is_the_examples() {
    // the input of `late_payment` is the first example of the module: the answer it writes
    let (_, text, _) = joined(&["run", "examples/late_payment/late_payment.kn", "claim", "--input", "tests/inputs/late_payment.json"]);
    assert!(text.ends_with("= Claim { due: 2026-03-31, days_late: 50, compensation: 70GBP, interest: 1150GBPc }\n"), "{text}");
    // a parameter may be named by its alias, as in the module
    let t = ritsu_testkit::TempDir::new("kaname-run");
    let input = t.write("in.json", r#"{"invoice": {"id": "inv_1", "invoiced": "2026-03-02", "amount": 100000}, "paid_on": "2026-05-20", "reference_rate": 50}"#);
    let (code, text, _) = joined(&["run", "examples/late_payment/late_payment.ja.kn", "claim", "--input", input.to_str().unwrap(), "--lang", "ja"]);
    assert_eq!(code, 0);
    assert!(text.ends_with("= 請求 { 期日: 2026-03-31, 遅れた日数: 50, 定額の補償: 70GBP, 利息: 1150GBPc }\n"), "{text}");
}

#[test]
fn a_failure_is_an_answer_of_the_run() {
    let t = ritsu_testkit::TempDir::new("kaname-run-fails");
    // an argument outside its range is the failure `input`, said as the answer: exit 0
    let input = t.write("in.json", r#"{"invoice": {"id": "inv_1", "invoiced": "2026-03-02", "amount": 0}, "paid_on": "2026-05-20", "reference_rate": 50}"#);
    let (code, text, _) = joined(&["run", "examples/late_payment/late_payment.kn", "claim", "--input", input.to_str().unwrap()]);
    assert_eq!((code, text.as_str()), (0, "fails: input (line 18): The argument `invoice.amount` of `claim` is 0GBPc, outside its range >=1GBPc <=100000000GBPc\n"));
    let (code, text, _) = joined(&["run", "examples/late_payment/late_payment.ja.kn", "遅れの請求", "--input", input.to_str().unwrap(), "--lang", "ja"]);
    assert_eq!(code, 0);
    assert_eq!(text, "失敗: input（18 行目）: 関数 `遅れの請求` の引数 `請求書.金額` の 0GBPc は、範囲 >=1GBPc <=100000000GBPc の外です\n");
}

#[test]
fn what_stops_the_command() {
    let (code, _, err) = joined(&["run", "examples/late_payment/late_payment.kn", "claim"]);
    assert_eq!((code, err.as_str()), (2, "error: `kaname run` needs `--input <file.json|->`\n"));
    let (code, _, err) = joined(&["run", "examples/late_payment/late_payment.kn", "--input", "tests/inputs/late_payment.json"]);
    assert_eq!((code, err.as_str()), (2, "error: `kaname run` takes a .kn file and the name of a function\n"));
    let (code, _, err) = joined(&["run", "examples/late_payment/late_payment.kn", "settle", "--input", "tests/inputs/late_payment.json"]);
    assert_eq!((code, err.as_str()), (2, "error: `examples/late_payment/late_payment.kn` has no function `settle` (its functions are `claim`)\n"));
    let t = ritsu_testkit::TempDir::new("kaname-run-stops");
    let bad = t.write("bad.json", "{\"invoice\": ");
    let (code, _, err) = joined(&["run", "examples/late_payment/late_payment.kn", "claim", "--input", bad.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(err.starts_with(&format!("error: {} is not JSON: ", bad.display())), "{err}");
    // a module that does not pass check is not run: exit 1
    let broken = t.write("broken.kn", "module broken v1\n\nfn f(n: number range >=0 <=9) -> number\n  n + x\n");
    let (code, out, err) = joined(&["run", broken.to_str().unwrap(), "f", "--input", bad.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(out.starts_with("error[E101]: "), "{out}");
    assert!(err.ends_with("does not pass check, so it is not run\n"), "{err}");
    // the binary of kaname's own crate reads no rule: E202, exit 2
    let (code, out, _) = run(&["run", "examples/late_payment/late_payment.kn", "claim", "--input", "tests/inputs/late_payment.json"], Suite::default());
    assert_eq!(code, 2);
    assert!(out.starts_with("error[E202]: "), "{out}");
}

#[test]
fn a_run_that_does_not_stop_exits_1() {
    // on a stack of its own, as deep as the evaluation needs (the binary's main thread gives it)
    let (code, out) = std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(|| {
            let t = ritsu_testkit::TempDir::new("kaname-run-deep");
            let kn = t.write("deep.kn", "module deep v1\n\nfn countdown(n: number range >=0 <=100000) -> number\n  if n <= 0 then 0 else countdown(n: n - 1) + 1\n");
            let input = t.write("in.json", r#"{"n": 100000}"#);
            let (code, out, _) = joined(&["run", kn.to_str().unwrap(), "countdown", "--input", input.to_str().unwrap()]);
            (code, out)
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(code, 1);
    assert!(out.ends_with(&format!("does not stop: the calls of functions go deeper than {} (`countdown`, called on line 4)\n", kaname::interp::DEPTH)), "{out}");
}

/// A value of a type that holds itself, nested as deep as a host may give one (200), is read from the
/// input and answered, and one nested deeper fails at the entry with `input`, the type named (DESIGN
/// 28.1): `run` reads the input as deep as the arguments' JSON can nest with their values of such types
/// one deeper than that, a level of JSON for a record and a list, two for a value of a sum with fields
/// (DESIGN 11.1). A value deeper still is no JSON it reads. The binary of kaname's own crate runs on a
/// main thread of half the 8 MiB a process is given (`ulimit -s 4096`): the reading, the entry and a
/// walk of 200 calls stay inside it.
#[test]
fn a_value_nested_as_deep_as_a_host_may_give_one_is_read() {
    let t = ritsu_testkit::TempDir::new("kaname-run-nested");
    let kn = t.write("trees.kn", include_str!("modules/trees.kn"));
    let nest = |n: usize, leaf: &str, wrap: &dyn Fn(&str) -> String| (1..n).fold(leaf.to_string(), |s, _| wrap(&s));
    let category = |n| format!(r#"{{"c": {}}}"#, nest(n, r#"{"name": "a", "children": []}"#, &|s| format!(r#"{{"name": "a", "children": [{s}]}}"#)));
    let expr = |n| format!(r#"{{"e": {}}}"#, nest(n, r#"{"num": {"value": 1}}"#, &|s| format!(r#"{{"neg": {{"inner": {s}}}}}"#)));
    let node = |n| format!(r#"{{"n": {}}}"#, nest(n, r#"{"value": 1}"#, &|s| format!(r#"{{"value": 1, "next": {s}}}"#)));
    let run = |function: &str, input: String, lang: &str| -> (i32, String, String) {
        let at = t.write("in.json", input);
        let o = std::process::Command::new("sh")
            .args(["-c", "ulimit -s 4096 && exec \"$@\"", "sh", env!("CARGO_BIN_EXE_kaname"), "run", kn.to_str().unwrap(), function, "--input", at.to_str().unwrap(), "--lang", lang])
            .env_remove("KANAME_LANG")
            .env_remove("RITSU_LANG")
            .output()
            .unwrap();
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).lines().last().unwrap_or("").to_string(), String::from_utf8_lossy(&o.stderr).into_owned())
    };
    for (function, input, said) in [
        ("size", category(200), "= 200".to_string()),
        ("value_of", expr(200), "= -1".to_string()),
        ("length", node(200), "= 200".to_string()),
        ("size", category(201), "fails: input (line 55): The argument `c` of `size` nests `Category` more than 200 deep".to_string()),
        ("value_of", expr(201), "fails: input (line 95): The argument `e` of `value_of` nests `Expr` more than 200 deep".to_string()),
        ("length", node(201), "fails: input (line 157): The argument `n` of `length` nests `Node` more than 200 deep".to_string()),
    ] {
        assert_eq!(run(function, input, "en"), (0, said, String::new()), "{function}");
    }
    assert_eq!(run("size", category(201), "ja").1, "失敗: input（55 行目）: 関数 `size` の引数 `c` は、`Category` の入れ子が 200 段を超えています");
    // a category is two levels of JSON, and the input one more: 1 + 2 × 201 = 403
    let (code, out, err) = run("size", category(260), "en");
    assert_eq!((code, out.as_str()), (2, ""));
    assert!(err.starts_with("error: ") && err.ends_with(" is not JSON: byte 5434: nested deeper than 403 levels\n"), "{err}");
    // five levels of JSON a step (a value of a sum, its fields, a dict, a pair, a list): 1 + 5 × 201 =
    // 1,006, within the 1,024 `run` reads at most
    let wide = t.write("wide.kn", "module wide v1\n\nenum Deep\n  leaf\n  node\n    kids : dict[string, list[Deep]]\n\nfn height(d: Deep) -> number\n  match d\n    node n => 1 + n.kids.values().flatten().sum(x => height(d: x))\n    else => 0\n");
    let deep = |n| format!(r#"{{"d": {}}}"#, nest(n, r#""leaf""#, &|s| format!(r#"{{"node": {{"kids": [["k", [{s}]]]}}}}"#)));
    let run_wide = |input: String| -> String {
        let at = t.write("in.json", input);
        let o = std::process::Command::new("sh").args(["-c", "ulimit -s 4096 && exec \"$@\"", "sh", env!("CARGO_BIN_EXE_kaname"), "run", wide.to_str().unwrap(), "height", "--input", at.to_str().unwrap()]).env_remove("KANAME_LANG").env_remove("RITSU_LANG").output().unwrap();
        String::from_utf8_lossy(&o.stdout).lines().last().unwrap_or("").to_string() + &String::from_utf8_lossy(&o.stderr)
    };
    assert_eq!(run_wide(deep(200)), "= 199");
    assert_eq!(run_wide(deep(201)), "fails: input (line 8): The argument `d` of `height` nests `Deep` more than 200 deep");
}
