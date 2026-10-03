//! What koyomi's tests share besides ritsu-testkit (DESIGN 10.8): the files the generated code
//! is held to.

#![allow(dead_code)]

/// Every file the generated code is held to (tests/targets.rs): the examples that pass check,
/// and the fixtures made to use every operation over 1900–2100. The READMEs count their
/// vectors (tests/docs.rs).
pub const TARGET_FILES: &[&str] = &[
    "examples/calendars/東京の営業日.cal",
    "examples/calendars/民法142条の休日.cal",
    "examples/calendars/england_and_wales.cal",
    "examples/支払_20日締め翌月10日払い.cal",
    "examples/民法の期間.cal",
    "examples/締め日と支払日を受け取る.cal",
    "examples/net30.cal",
    "examples/payment_20th_close_next_10th.cal",
    "tests/fixtures/calendars/休みの書き方を全部使う.cal",
    "tests/fixtures/helpers_月を足す.cal",
    "tests/fixtures/helpers_月を引く.cal",
    "tests/fixtures/helpers_月の日と締め.cal",
    "tests/fixtures/helpers_営業日.cal",
    "tests/fixtures/helpers_断る.cal",
    "tests/fixtures/helpers_日付が一つも無い.cal",
];
