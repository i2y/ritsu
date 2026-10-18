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
const RUNS: [(&str, &str, &str); 4] = [("payout", "settle", "精算する"), ("late_payment", "claim", "遅れの請求"), ("media_mail", "quote", "見積もる"), ("discount_split", "split", "按分する")];

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
