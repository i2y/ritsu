//! `range from koyomi "<file>" date <name>` (§15.174; ritsu's DESIGN 7.5 (b)): a date input that
//! takes only the days a koyomi date comes to, read through ritsu's port of dates with koyomi
//! joined as `ritsu rulec` joins it. The tables are checked over those days — completeness, the
//! rows nothing reaches, the examples — the generated code refuses any other day, the certificate
//! carries the days and where they come from, and both re-checkers (`tools/recheck.py`, and the
//! Lean one when it is built) hold the cover to them. Where no koyomi is joined the rule is not
//! checked over every day instead: E129 says so. The materials are in `tests/days/` (English).
//!
//! The golden files are `tests/days/golden/`; `RULEC_BLESS=1` writes them.

use ritsu_ports::Rules;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn days_dir() -> PathBuf {
    root().join("tests/days")
}

fn koyomi() -> rulec::days::Port {
    Arc::new(koyomi::ports::Engine)
}

/// What `rulec check` prints for `src` (written as `name` in a copy of `tests/days/`), with koyomi
/// joined or not, in `lang`.
fn check_text(dir: &Path, name: &str, src: &str, joined: bool, lang: rulec::i18n::Lang) -> String {
    let path = dir.join(name);
    std::fs::write(&path, src).unwrap();
    let shown = name.to_string();
    let here = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir).unwrap();
    let port = joined.then(koyomi);
    let out = rulec::days::with(port, || {
        rulec::i18n::with(lang, || {
            let r = rulec::report(src, &shown);
            let lines: Vec<String> = src.lines().map(String::from).collect();
            format!("{}{}", rulec::findings_text(&r.diags, &lines), rulec::check_tail(&r.shadow, &r.diags, &shown, 0))
        })
    });
    std::env::set_current_dir(here).unwrap();
    out
}

fn golden(name: &str, got: &str, failures: &mut Vec<String>) {
    let p = days_dir().join("golden").join(name);
    if std::env::var("RULEC_BLESS").is_ok() || !p.exists() {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, got).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&p).unwrap();
    if want != got {
        failures.push(format!("{name} differs:\n--- want\n{want}\n--- got\n{got}\nRULEC_BLESS=1 cargo test --test days rewrites it"));
    }
}

fn settlement() -> String {
    std::fs::read_to_string(days_dir().join("settlement.rule")).unwrap()
}

/// The table with its first two rows meeting on 2026-07-01 to 07-05, where no payment day falls.
fn rows_meeting_off_the_days() -> String {
    settlement()
        .replace("| <=2026-06-30              | first_half     |", "| <=2026-07-05              | first_half     |")
        .replace("| >=2026-07-10 <=2026-12-10 | second_half    |", "| >=2026-07-01 <=2026-12-10 | second_half    |")
}

/// A copy of `tests/days/` to write the variants into, removed at the end.
fn scratch(tag: &str) -> ritsu_testkit::TempDir {
    let t = ritsu_testkit::TempDir::new(tag);
    for f in ["payment_terms.cal", "settlement.rule"] {
        std::fs::copy(days_dir().join(f), t.path().join(f)).unwrap();
    }
    t
}

/// The cases, each a variant of `settlement.rule`: the name of its golden file, the change, and
/// whether koyomi is joined. Every one but the first is a mistake the check names.
fn variants() -> Vec<(&'static str, String, bool)> {
    let s = settlement();
    vec![
        // passes: the two gaps between the rows hold no payment day
        ("passes", s.clone(), true),
        // passes: two rows meet on 2026-07-01 to 07-05, where no payment day falls
        ("passes-rows-meeting-off-the-days", rows_meeting_off_the_days(), true),
        // a payment day no row takes (E101), with that day as the example
        ("E101-a-payment-day-no-row-takes", s.replace(">=2026-07-10 <=2026-12-10", ">=2026-07-11 <=2026-12-10").replace("| 2026-07-10 | second_half |\n", ""), true),
        // a row whose dates are none of the payment days (E102)
        ("E102-a-row-on-no-payment-day", s.replace("| >=2027-01-01              | year_end       |\n", "| >=2027-01-01              | year_end       |\n| >=2026-07-01 <=2026-07-09 | second_half    |\n"), true),
        // an example on a day koyomi does not pay on (E019)
        ("E019-an-example-on-another-day", s.replace("| 2027-01-10 | year_end    |\n", "| 2027-01-10 | year_end    |\n| 2026-07-05 | second_half |\n"), true),
        // the plain range over the same days: the gaps are gaps (E101)
        ("E101-the-plain-range", s.replace("range from koyomi \"payment_terms.cal\" date payment", "range >=2026-02-10 <=2027-01-10"), true),
        // the shape of `range from` (E065)
        ("E065-a-shape-with-no-date", s.replace("date payment", ""), true),
        // a date koyomi's file does not have (E130)
        ("E130-a-date-the-file-does-not-have", s.replace("date payment", "date due"), true),
        // a file that is not there (E130)
        ("E130-a-file-that-is-not-there", s.replace("payment_terms.cal", "no_such_terms.cal"), true),
        // no koyomi joined (E129)
        ("E129-no-koyomi-joined", s.clone(), false),
    ]
}

#[test]
fn the_tables_are_checked_over_koyomis_days() {
    let t = scratch("days-check");
    let mut failures = Vec::new();
    for (name, src, joined) in variants() {
        for (lang, tag) in [(rulec::i18n::Lang::En, "en"), (rulec::i18n::Lang::Ja, "ja")] {
            let out = check_text(t.path(), "variant.rule", &src, joined, lang);
            let code = name.split('-').next().unwrap();
            if code == "passes" {
                assert_eq!(out, "ok variant.rule\n", "{name} ({tag})");
            } else {
                assert!(out.contains(&format!("[{code}]")), "{name} ({tag}) does not say {code}:\n{out}");
            }
            golden(&format!("{name}.{tag}.txt"), &out, &mut failures);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The command exits 2 for E129, as the receiving languages' own binaries do for a run with the
/// other language not joined; with koyomi joined (`ritsu rulec`) the file passes. This runs the
/// binary of rulec's own crate, which has no koyomi.
#[test]
fn the_crate_binary_refuses_with_exit_2() {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(days_dir()).args(["check", "settlement.rule"]).env_remove("RULEC_LANG").env_remove("RITSU_LANG").output().unwrap();
    assert_eq!(o.status.code(), Some(2), "{}", String::from_utf8_lossy(&o.stdout));
    assert!(String::from_utf8_lossy(&o.stdout).contains("error[E129]: This rulec reads no koyomi file"));
}

/// The certificate says where the days come from and what they are, and both re-checkers hold
/// the cover to them: the two boxes between the rows are impossible because no payment day is in
/// them, and each row's point is a payment day.
#[test]
fn the_certificate_carries_the_days_and_the_rechecks_hold() {
    certificate_holds(&settlement(), r#"{"days_axis":0}"#, "2 impossible outside koyomi's days");
    // the two rows that meet only off the days are apart on them, which both re-checkers prove
    certificate_holds(&rows_meeting_off_the_days(), r#""days_apart":[{"a":1,"b":2,"axis":0}]"#, "1 apart on koyomi's days");
}

/// The certificate of `src` (written beside `payment_terms.cal`) carries the days and `needle`,
/// and both re-checkers prove every claim, `tools/recheck.py` saying `said` among them.
fn certificate_holds(src: &str, needle: &str, said: &str) {
    let dir = scratch("days-cert-rule");
    let path = dir.path().join("variant.rule");
    std::fs::write(&path, src).unwrap();
    let src = src.to_string();
    let cert = rulec::days::with(Some(koyomi()), || {
        let (f, c) = rulec::prepare(&src, &path.to_string_lossy()).map_err(|d| format!("{d:?}")).unwrap();
        rulec::cert::certificate(&f, &c, &src, &path.to_string_lossy())
    });
    let v = ritsu_base::json::parse(&cert).unwrap();
    let days = v.get("days").and_then(|d| d.get("pay_day")).expect("the days of pay_day");
    assert_eq!(days.get("from").map(|f| f.to_string()).as_deref(), Some(r#"{"tool":"koyomi","file":"payment_terms.cal","date":"payment"}"#));
    let sha = ritsu_base::sha256::hex(&std::fs::read(days_dir().join("payment_terms.cal")).unwrap());
    assert_eq!(days.get("sha256").and_then(|s| s.as_str()), Some(sha.as_str()));
    let listed: Vec<String> = match days.get("days") {
        Some(ritsu_base::json::Json::Arr(xs)) => xs.iter().map(|x| x.as_str().unwrap().to_string()).collect(),
        other => panic!("{other:?}"),
    };
    assert_eq!(listed.len(), 12, "the 10th of each month from 2026-02 to 2027-01");
    assert!(cert.contains(needle), "{cert}");
    let cert_path = dir.path().join("cert.json");
    std::fs::write(&cert_path, &cert).unwrap();
    if ritsu_testkit::ready(ritsu_testkit::Need::Python, || ritsu_testkit::tools::on_path("python3").is_some(), "python3 is not on the PATH; tools/recheck.py is not run") {
        let o = Command::new("python3").current_dir(root()).args(["tools/recheck.py", "--rule", path.to_str().unwrap(), cert_path.to_str().unwrap()]).output().unwrap();
        let out = String::from_utf8_lossy(&o.stdout);
        assert!(o.status.success(), "{out}{}", String::from_utf8_lossy(&o.stderr));
        assert!(out.contains(said) && out.contains("every claim this program states was proved"), "{out}");
    }
    let lean = root().join("../../proofs/.lake/build/bin/rulec-recheck");
    if ritsu_testkit::ready(ritsu_testkit::Need::Lean, || lean.exists(), "ritsu's proofs/ is not built (lake build there makes it); the Lean re-check is not run") {
        let o = Command::new(&lean).current_dir(root()).args(["--rule", path.to_str().unwrap(), cert_path.to_str().unwrap()]).output().unwrap();
        let out = String::from_utf8_lossy(&o.stdout);
        assert!(o.status.success() && out.contains("OK: every claim this program states was proved"), "{out}{}", String::from_utf8_lossy(&o.stderr));
    }
}

/// The generated code refuses a day koyomi does not pay on, and answers one it does (Python, the
/// generator's first language; the other backends write the same test from the same function).
#[test]
fn the_generated_code_refuses_another_day() {
    if !ritsu_testkit::ready(ritsu_testkit::Need::Python, || ritsu_testkit::tools::on_path("python3").is_some(), "python3 is not on the PATH; the generated Python is not run") {
        return;
    }
    let path = days_dir().join("settlement.rule");
    let t = ritsu_testkit::TempDir::new("days-gen");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = rulec::days::with(Some(koyomi()), || rulec::codegen::generate(&[path.to_str().unwrap()], t.path().to_str().unwrap(), false, false, &mut out, &mut err));
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    let dir = t.path().join("python");
    assert!(dir.join("settlement.py").exists(), "{}", String::from_utf8_lossy(&out));
    // 20494 is 2026-02-10, a payment day; 20500 is 2026-02-16, not one
    let probe = "import settlement\nprint(settlement.settlement(20494))\ntry:\n    settlement.settlement(20500)\n    print('answered')\nexcept settlement.RuleInputError as e:\n    print('refused:', e.args[0])\n";
    let o = Command::new("python3").current_dir(&dir).args(["-c", probe]).output().unwrap();
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "{out}{}", String::from_utf8_lossy(&o.stderr));
    assert!(out.contains("refused: pay_day is not a day payment of payment_terms.cal comes to"), "{out}");
}

/// The vectors take only payment days, and cover what can be covered over them: every row, and no
/// boundary pair, since the days on either side of each boundary of the table are not payment
/// days (an unrealizable side is no obligation, §9.1).
#[test]
fn the_vectors_and_their_coverage_are_over_the_days() {
    let src = settlement();
    let path = days_dir().join("settlement.rule");
    let (a, vs, _) = rulec::days::with(Some(koyomi()), || {
        let (f, c) = rulec::prepare(&src, &path.to_string_lossy()).map_err(|d| format!("{d:?}")).unwrap();
        rulec::coverage::audit_file(&f, &c, &path.to_string_lossy())
    });
    assert!(a.ok(), "{}", rulec::coverage::render(&a, &vs, &[]));
    assert!(!vs.is_empty());
    let paid: Vec<i128> = vec![20494, 20522, 20553, 20583, 20614, 20644, 20675, 20706, 20736, 20767, 20797, 20828];
    for v in &vs {
        match v.input.get("pay_day") {
            Some(rulec::eval::Val::Date(y, m, d)) => {
                let day = rulec::types::date_ord(*y, *m, *d).num;
                assert!(paid.contains(&day), "a vector on {y}-{m}-{d}, which is not a payment day");
            }
            other => panic!("{other:?}"),
        }
    }
}

/// The generated code of every language whose toolchain is here answers the rule's vectors — the
/// payment days — with the door for the other days in it (`rulec test` over what `rulec gen`
/// writes with koyomi joined). A language whose toolchain is missing is named, as `rulec test`
/// names it, and the run is not failed for it.
#[test]
fn the_generated_code_answers_in_every_language_here() {
    let path = days_dir().join("settlement.rule");
    let t = ritsu_testkit::TempDir::new("days-all");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = rulec::days::with(Some(koyomi()), || rulec::codegen::generate(&[path.to_str().unwrap()], t.path().to_str().unwrap(), false, false, &mut out, &mut err));
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    let run = rulec::runtest::run(t.path()).expect("rulec test runs");
    print!("{}", rulec::runtest::render(&run));
    for s in &run.skipped {
        println!("SKIP: {s}");
    }
    assert!(run.ok(), "{}", rulec::runtest::render(&run));
    assert!(!run.results.is_empty(), "no language ran");
}

/// ritsu's port: a rule over a plain range, which passes its own check over every day, held to a
/// set of days (`checked_over`) — complete and without overlap still, and a row no day of the set
/// reaches is the example; and the facts of a rule with `range from koyomi` carry the days as a
/// precondition.
#[test]
fn the_port_answers_over_a_set_of_days() {
    let t = scratch("days-port");
    let plain = settlement()
        .replace("range from koyomi \"payment_terms.cal\" date payment", "range >=2026-02-10 <=2027-01-10")
        .replace(">=2026-07-10 <=2026-12-10", ">=2026-07-01 <=2026-12-31");
    let p = t.path().join("plain.rule");
    std::fs::write(&p, &plain).unwrap();
    let engine = rulec::ports::Engine::with_dates(koyomi());
    let paid: ritsu_ports::DaySet = [20494, 20522, 20553, 20583, 20614, 20644, 20675, 20706, 20736, 20767, 20797, 20828].into_iter().collect();
    assert_eq!(engine.checked_over(&p, "pay_day", &paid).unwrap(), ritsu_ports::Answer::Holds);
    // only the first half's days: the rows after them are reached by none
    let first_half: ritsu_ports::DaySet = paid.iter().copied().filter(|d| *d <= 20634).collect();
    match engine.checked_over(&p, "pay_day", &first_half).unwrap() {
        ritsu_ports::Answer::Fails(t) => assert!(t.en.starts_with("[E102]"), "{t:?}"),
        other => panic!("{other:?}"),
    }
    assert!(matches!(engine.checked_over(&p, "batch", &paid).unwrap(), ritsu_ports::Answer::Undecided(_)));
    let facts = engine.facts(&t.path().join("settlement.rule")).unwrap();
    assert!(facts.preconditions.iter().any(|x| matches!(x, ritsu_ports::Precondition::Days { input, days, .. } if input == "pay_day" && days.len() == 12)));
    // the koyomi date is one of the things the rule names outside itself (sakai and yuen read it)
    let refs = ritsu_ports::References::references(&engine, t.path(), "settlement.rule").unwrap();
    let r = refs.iter().find(|r| r.how == "range from koyomi").expect("the koyomi date is named");
    assert_eq!((r.line, r.target.text().as_str()), (7, r#"koyomi "payment_terms.cal" date payment"#));
}
