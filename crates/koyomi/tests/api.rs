//! `koyomi api` (PLAN B.10, DESIGN 8): the JSON of every example that passes check is its
//! golden file in `tests/golden/api/`. `KOYOMI_BLESS=1 cargo test` writes them again.

use koyomi::check::{Checked, check};

const EXAMPLES: [&str; 15] = [
    "examples/calendars/england_and_wales.cal",
    "examples/net30.cal",
    "examples/close_20th_pay_10th.cal",
    "examples/close_and_pay_on_given_days.cal",
    "examples/period_of_months.cal",
    "examples/calendars/tokyo_business_days.cal",
    "examples/calendars/civil_code_142_days.cal",
    "examples/payment_20th_close_next_10th.cal",
    "examples/closing_and_payment_days_as_inputs.cal",
    "examples/civil_code_period_end.cal",
    "examples/calendars/東京の営業日.cal",
    "examples/calendars/民法142条の休日.cal",
    "examples/payment_20th_close_next_10th.ja.cal",
    "examples/civil_code_period_end.ja.cal",
    "examples/closing_and_payment_days_as_inputs.ja.cal",
];

fn api(path: &str) -> serde_json::Value {
    let o = check(path).unwrap();
    assert!(!o.has_errors(), "{path}");
    match &o.checked {
        Some(Checked::Calendar(c)) => koyomi::api::calendar_file_json(c),
        Some(Checked::Dates(m, _)) => koyomi::api::dates_json(m),
        None => unreachable!(),
    }
}

#[test]
fn every_example_has_its_golden_api() {
    let mut failures = Vec::new();
    for p in EXAMPLES {
        let text = serde_json::to_string_pretty(&api(p)).unwrap() + "\n";
        let name = std::path::Path::new(p).file_stem().unwrap().to_string_lossy().to_string();
        let golden = format!("tests/golden/api/{name}.json");
        if let Err(e) = ritsu_testkit::golden::check(std::path::Path::new(&golden), &text) {
            failures.push(format!("{p}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn what_the_api_says_of_the_payment_terms() {
    let v = api("examples/payment_20th_close_next_10th.ja.cal");
    assert_eq!(v["alias"], "payment_terms");
    assert_eq!(v["calendar"]["file"], "calendars/東京の営業日.cal");
    assert_eq!(v["calendar"]["offset"], "+09:00");
    assert_eq!(v["calendar"]["data_range"]["to"], "2027-12-31");
    assert_eq!(v["calendar"]["sources"][0]["file"], "calendars/data/syukujitsu.csv");
    assert_eq!(v["calendar"]["sources"][0]["sha256"], "cec37a743c96995c");
    assert_eq!(v["calendar"]["sources"][0]["rows"], 1067);
    assert_eq!(v["inputs"][0]["range"]["max"], "2027-11-20");
    assert_eq!(v["dates"][1]["at"]["time"], "09:00");
    assert_eq!(v["typescript"]["functions"][1]["signature"], "export function payment(received: string): string");
    assert_eq!(v["typescript"]["functions"][1]["at_signature"], "export function payment_at(received: string): string");
    assert_eq!(v["python"]["functions"][1]["signature"], "def payment(received: date) -> date");
    assert_eq!(v["go"]["functions"][1]["signature"], "func Payment(received Date) (Date, error)");
    assert_eq!(v["go"]["functions"][1]["at_signature"], "func PaymentAt(received Date) (string, error)");
    assert_eq!(v["go"]["module"], "paymentterms");
    assert_eq!(v["go"]["is_open"], "func IsOpen(day Date) (bool, error)");
    assert_eq!(v["rust"]["functions"][0]["signature"], "pub fn closing(received: Date) -> Result<Date, KoyomiError>");
    assert_eq!(v["sql"]["functions"][1]["signature"], "payment_terms.payment(received date) RETURNS date");
    assert_eq!(v["sql"]["functions"][1]["at_signature"], "payment_terms.payment_at(received date) RETURNS timestamptz");
    assert_eq!(v["wire"]["at"], "RFC 3339 in UTC, ending in Z, as dandori's timestamp");
}

#[test]
fn a_date_takes_only_the_inputs_it_uses() {
    let v = api("examples/civil_code_period_end.ja.cal");
    // 起算日 uses the start only; 満了日 the start and the months.
    assert_eq!(v["dates"][0]["params"], serde_json::json!(["起点"]));
    assert_eq!(v["dates"][1]["params"], serde_json::json!(["起点", "月数"]));
    assert_eq!(v["typescript"]["functions"][1]["signature"], "export function last_day(origin: string, month_count: number): string");
    assert_eq!(v["sources"][0]["kind"], "law");
    assert_eq!(v["sources"][0]["revision"], "129AC0000000089_20260624_508AC0000000045");
    assert_eq!(v["sources"][0]["pins"].as_array().unwrap().len(), 4);
}

#[test]
fn every_signature_is_in_the_generated_code() {
    // PLAN C.8: the api and the generators name things through the same functions
    // (src/naming.rs), so each signature the api prints is in the file gen writes, as it is.
    let mut n = 0;
    let fixtures = ["tests/fixtures/helpers_business_days.cal", "tests/fixtures/calendars/every_way_to_close.cal", "tests/fixtures/helpers_営業日.cal", "tests/fixtures/calendars/休みの書き方を全部使う.cal"];
    for p in EXAMPLES.iter().chain(fixtures.iter()) {
        let v = api(p);
        let mut o = check(p).unwrap();
        let checked = o.checked.take().unwrap();
        let u = koyomi::codegen::unit_of(&checked, ritsu_base::text::Lang::En);
        for t in koyomi::naming::TARGETS {
            let entry = &v[t.key()];
            let files = koyomi::codegen::files(&u, t);
            let (path, module) = &files[0];
            assert_eq!(entry["file"].as_str().unwrap(), path, "{p} {}", t.key());
            let mut sigs: Vec<String> = Vec::new();
            for f in entry["functions"].as_array().into_iter().flatten() {
                sigs.push(f["signature"].as_str().unwrap().to_string());
                if let Some(s) = f["at_signature"].as_str() {
                    sigs.push(s.to_string());
                }
            }
            if let Some(s) = entry["is_open"].as_str() {
                sigs.push(s.to_string());
            }
            assert!(!sigs.is_empty(), "{p} {}", t.key());
            for s in sigs {
                assert!(module.contains(&s), "{p}: the {} code has no `{s}`", t.key());
                n += 1;
            }
        }
    }
    assert!(n >= 100, "{n} signatures");
}

/// What `what_the_api_says_of_the_payment_terms` checks, of the English version: the same terms,
/// with English names, on the English version of the Tokyo calendar.
#[test]
fn what_the_api_says_of_the_payment_terms_in_english() {
    let v = api("examples/payment_20th_close_next_10th.cal");
    assert_eq!(v["alias"], "payment_20th_close_next_10th");
    assert_eq!(v["calendar"]["file"], "calendars/tokyo_business_days.cal");
    assert_eq!(v["calendar"]["offset"], "+09:00");
    assert_eq!(v["calendar"]["data_range"]["to"], "2027-12-31");
    assert_eq!(v["calendar"]["sources"][0]["file"], "calendars/data/syukujitsu.csv");
    assert_eq!(v["calendar"]["sources"][0]["sha256"], "cec37a743c96995c");
    assert_eq!(v["calendar"]["sources"][0]["rows"], 1067);
    assert_eq!(v["inputs"][0]["range"]["max"], "2027-11-20");
    assert_eq!(v["dates"][1]["at"]["time"], "09:00");
    assert_eq!(v["typescript"]["functions"][1]["signature"], "export function payment(received: string): string");
    assert_eq!(v["typescript"]["functions"][1]["at_signature"], "export function payment_at(received: string): string");
    assert_eq!(v["python"]["functions"][1]["signature"], "def payment(received: date) -> date");
    assert_eq!(v["go"]["functions"][1]["signature"], "func Payment(received Date) (Date, error)");
    assert_eq!(v["go"]["functions"][1]["at_signature"], "func PaymentAt(received Date) (string, error)");
    assert_eq!(v["go"]["module"], "payment20thclosenext10th");
    assert_eq!(v["go"]["is_open"], "func IsOpen(day Date) (bool, error)");
    assert_eq!(v["rust"]["functions"][0]["signature"], "pub fn closing(received: Date) -> Result<Date, KoyomiError>");
    assert_eq!(v["sql"]["functions"][1]["signature"], "payment_20th_close_next_10th.payment(received date) RETURNS date");
    assert_eq!(v["sql"]["functions"][1]["at_signature"], "payment_20th_close_next_10th.payment_at(received date) RETURNS timestamptz");
    assert_eq!(v["wire"]["at"], "RFC 3339 in UTC, ending in Z, as dandori's timestamp");
}

/// The terms on England and Wales, a calendar with no offset: its functions give dates only.
#[test]
fn what_the_api_says_of_terms_in_england_and_wales() {
    let v = api("examples/close_20th_pay_10th.cal");
    assert_eq!(v["alias"], "close_20th_pay_10th");
    assert_eq!(v["calendar"]["file"], "calendars/england_and_wales.cal");
    assert!(v["calendar"]["offset"].is_null(), "England and Wales changes its clocks, so its calendar has no offset");
    assert_eq!(v["calendar"]["data_range"]["to"], "2028-12-31");
    assert_eq!(v["calendar"]["sources"][0]["file"], "calendars/data/bank-holidays.json");
    assert_eq!(v["calendar"]["sources"][0]["sha256"], "538b3482c28b85ec");
    assert_eq!(v["calendar"]["sources"][0]["rows"], 83);
    assert_eq!(v["inputs"][0]["range"]["max"], "2028-11-20");
    assert!(v["dates"][1]["at"].is_null());
    assert!(v["typescript"]["functions"][1]["at_signature"].is_null());
    assert_eq!(v["go"]["module"], "close20thpay10th");
    assert_eq!(v["sql"]["functions"][1]["signature"], "close_20th_pay_10th.payment(received date) RETURNS date");
}

#[test]
fn a_date_takes_only_the_inputs_it_uses_in_english() {
    let v = api("examples/civil_code_period_end.cal");
    // first_day uses the origin only; last_day the origin and the months.
    assert_eq!(v["dates"][0]["params"], serde_json::json!(["origin"]));
    assert_eq!(v["dates"][1]["params"], serde_json::json!(["origin", "month_count"]));
    assert_eq!(v["typescript"]["functions"][1]["signature"], "export function last_day(origin: string, month_count: number): string");
    assert_eq!(v["sources"][0]["kind"], "law");
    assert_eq!(v["sources"][0]["revision"], "129AC0000000089_20260624_508AC0000000045");
    assert_eq!(v["sources"][0]["pins"].as_array().unwrap().len(), 4);
    let v = api("examples/period_of_months.cal");
    assert_eq!(v["dates"][1]["params"], serde_json::json!(["origin", "month_count"]));
}
