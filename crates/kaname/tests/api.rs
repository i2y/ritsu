//! `kaname api` (DESIGN 11.1): each of the examples, in English and in Japanese, as the tools
//! outside read it, held to `tests/golden/api/` (`KANAME_BLESS=1` writes them again); and what stops
//! the command.
//!
//! These tests join rulec, koyomi and chobo but not ritsu's checks across the borders, which kaname
//! does not hold even as a dev-dependency: the places those checks decide wait for them here, and
//! the refusals of the transfers are not known. `crates/ritsu/tests/kaname.rs` reads the same command
//! through `ritsu kaname`, with the verdicts.

mod common;

use kaname::suite::Suite;
use ritsu_testkit::TempDir;

fn run(args: &[&str], suite: Suite) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = kaname::run::run(&args, suite, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

const EXAMPLES: [&str; 4] = ["payout", "late_payment", "media_mail", "discount_split"];

#[test]
fn each_example_reads_as_the_golden_files_hold() {
    for name in EXAMPLES {
        for (kn, golden) in [(format!("examples/{name}/{name}.kn"), name.to_string()), (format!("examples/{name}/{name}.ja.kn"), format!("{name}.ja"))] {
            let root = format!("examples/{name}");
            let (code, json, err) = run(&["api", &kn, "--root", &root], common::joined());
            assert_eq!(code, 0, "{kn}: {json}{err}");
            let json = json.replacen(&format!("\"kaname\": \"{}\"", env!("CARGO_PKG_VERSION")), "\"kaname\": \"<version>\"", 1);
            ritsu_testkit::golden(format!("tests/golden/api/{golden}.json"), &json);
        }
    }
}

#[test]
fn what_the_api_says_and_what_stops_it() {
    // the late payment: its effect waits for the date of koyomi, and the call says where
    let (_, json, _) = run(&["api", "examples/late_payment/late_payment.kn", "--root", "examples/late_payment"], common::joined());
    let v = ritsu_base::json::parse(&json).unwrap();
    let ritsu_base::json::Json::Obj(top) = &v else { panic!("{json}") };
    let keys: Vec<&str> = top.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["kaname", "file", "module", "generated", "uses", "enums", "records", "functions", "transfers"]);
    assert!(json.contains("\"check\": \"X6\""), "{json}");
    assert!(json.contains("\"outcome\": \"waiting\""), "{json}");
    assert!(json.contains("\"go\": \"Claim\""), "{json}");
    // a module whose names do not pass has no api: exit 1, with what check says
    let t = TempDir::new("kaname-api");
    let f = t.write("main.kn", "module main v1\n\nfn f(n: number range >=0 <=9) -> number\n  m + 1\n");
    let (code, out, err) = run(&["api", f.to_str().unwrap()], common::joined());
    assert_eq!(code, 1);
    assert!(out.contains("error[E101]: "), "{out}");
    assert!(err.starts_with("error: the names or the types of "), "{err}");
    // a module whose analysis finds something wrong has one: where it can fail is what it is read for
    let f = t.write("area.kn", "module area v1\n\nfn area(side: number range >=1 <=8) -> number range >=0 <=24 total\n  side * (10 - side)\n");
    let (code, json, _) = run(&["api", f.to_str().unwrap()], common::joined());
    assert_eq!(code, 0, "{json}");
    assert!(json.contains("\"code\": \"W301\"") && json.contains("\"outcome\": \"undecided\""), "{json}");
    // the binary of kaname's own crate reads no rule: E202, exit 2
    let (code, out, _) = run(&["api", "examples/payout/payout.kn"], Suite::default());
    assert_eq!(code, 2);
    assert!(out.contains("[E202]"), "{out}");
    // one file only
    assert_eq!(run(&["api"], common::joined()).0, 2);
}
