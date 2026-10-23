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
    // The Wasm module: one function, the dates under the keys of out, by their names.
    assert_eq!(v["wasm"]["file"], "wasm/payment_terms_wasm.rs");
    assert_eq!(v["wasm"]["module"], "payment_terms.wasm");
    assert_eq!(v["wasm"]["package"], "koyomi:payment-terms@1.0.0");
    assert_eq!(v["wasm"]["world"], "payment-terms");
    assert_eq!(v["wasm"]["call_signature"], "call: func(input: string) -> string");
    assert_eq!(v["wasm"]["functions"][1]["key"], "支払日");
    assert_eq!(v["wasm"]["functions"][1]["at_key"], "支払日.at");
    assert_eq!(v["wasm"]["functions"][1]["params"][0]["name"], "受領日");
    assert_eq!(v["wasm"]["is_open"]["in"], "date");
    assert_eq!(v["wasm"]["answers"][1]["refused"], serde_json::json!(["input", "range", "data", "reject", "date"]));
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
            if t == koyomi::naming::Target::Wasm {
                // The world declares the one function; the crate root writes each key of out, and
                // reads {"date": …} when the file has a calendar.
                let wit = &files.iter().find(|(r, _)| r.ends_with(".wit")).unwrap().1;
                let sig = entry["call_signature"].as_str().unwrap();
                assert!(wit.contains(&format!("export {sig};")), "{p}: the .wit does not export `{sig}`");
                for (k, w) in [("wit", ".wit"), ("runner", "_runner.mjs")] {
                    let name = entry[k].as_str().unwrap();
                    assert!(name.ends_with(w) && files.iter().any(|(r, _)| r == &format!("wasm/{name}")), "{p}: {k} = {name} is not a file gen writes");
                }
                let mut keys: Vec<String> = Vec::new();
                for f in entry["functions"].as_array().into_iter().flatten() {
                    keys.push(f["key"].as_str().unwrap().to_string());
                    if let Some(s) = f["at_key"].as_str() {
                        keys.push(s.to_string());
                    }
                }
                if entry["is_open"].is_object() {
                    keys.push("date".into());
                    keys.push("open".into());
                }
                assert!(!keys.is_empty(), "{p} wasm");
                for k in keys {
                    let lit = serde_json::to_string(&k).unwrap();
                    assert!(module.contains(&lit), "{p}: the Wasm crate root has no {lit}");
                    n += 1;
                }
                continue;
            }
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
    assert_eq!(v["wasm"]["file"], "wasm/payment_20th_close_next_10th_wasm.rs");
    assert_eq!(v["wasm"]["package"], "koyomi:payment-20th-close-next-10th@1.0.0");
    assert_eq!(v["wasm"]["build"], "rustc --edition 2021 -C opt-level=s -C lto -C panic=abort -C strip=symbols --target wasm32-unknown-unknown --crate-type cdylib payment_20th_close_next_10th_wasm.rs -o payment_20th_close_next_10th.wasm");
    assert_eq!(v["wasm"]["functions"][1]["key"], "payment");
    assert_eq!(v["wasm"]["functions"][1]["at_key"], "payment.at");
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
    assert!(v["wasm"]["functions"][1]["at_key"].is_null());
    // A calendar file's Wasm module answers {"date": …} alone.
    let c = api("examples/calendars/england_and_wales.cal");
    assert_eq!(c["wasm"]["file"], "wasm/england_and_wales_wasm.rs");
    assert_eq!(c["wasm"]["is_open"]["out"], "open");
    assert!(c["wasm"]["functions"].is_null());
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
