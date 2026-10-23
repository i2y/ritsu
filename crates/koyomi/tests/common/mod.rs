//! What koyomi's tests share besides ritsu-testkit (DESIGN 10.8): the files the generated code
//! is held to, and whether rustc can build for a target.

#![allow(dead_code)]

/// Every file the generated code is held to (tests/targets.rs): the examples that pass check, in
/// English and in Japanese (the examples on England and Wales, the English versions of the Japanese
/// examples, and the Japanese versions), and the fixtures made to use every operation over
/// 1900–2100, in English and in Japanese. The READMEs count their vectors (tests/docs.rs).
pub const TARGET_FILES: &[&str] = &[
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
    "examples/closing_and_payment_days_as_inputs.ja.cal",
    "examples/civil_code_period_end.ja.cal",
    "tests/fixtures/calendars/every_way_to_close.cal",
    "tests/fixtures/helpers_add_months.cal",
    "tests/fixtures/helpers_subtract_months.cal",
    "tests/fixtures/helpers_days_of_months.cal",
    "tests/fixtures/helpers_business_days.cal",
    "tests/fixtures/helpers_reject.cal",
    "tests/fixtures/helpers_no_dates.cal",
    "tests/fixtures/calendars/休みの書き方を全部使う.cal",
    "tests/fixtures/helpers_月を足す.cal",
    "tests/fixtures/helpers_月を引く.cal",
    "tests/fixtures/helpers_月の日と締め.cal",
    "tests/fixtures/helpers_営業日.cal",
    "tests/fixtures/helpers_エラーにする.cal",
    "tests/fixtures/helpers_日付が一つも無い.cal",
];

/// Whether rustc has the standard library of `target` installed (`rustup target add <target>`).
pub fn rust_target(target: &str) -> bool {
    let Ok(o) = std::process::Command::new("rustc").args(["--print", "sysroot"]).output() else { return false };
    if !o.status.success() {
        return false;
    }
    let root = String::from_utf8_lossy(&o.stdout).trim().to_string();
    std::path::Path::new(&root).join("lib/rustlib").join(target).join("lib").is_dir()
}
