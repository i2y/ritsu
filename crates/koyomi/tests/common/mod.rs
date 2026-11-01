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

/// The files of `tests/spellings/` (DESIGN 15): each writes names some targets spell as one
/// identifier, or cannot take, and `check` warns about it once (W001), in English and in Japanese.
/// Beside each is what the tool of the targets it names says of the code: the line it stops at
/// (for Python, what the runner stops with), or nothing when the code loads and answers for one
/// function with another. tests/targets.rs runs each file through the targets W001 does not name,
/// and the named ones with this line.
pub const SPELLING_FILES: &[(&str, &str)] = &[
    ("tests/spellings/go_functions.cal", "Net30 redeclared in this block"),
    ("tests/spellings/go_time_function.cal", "CutoffAt redeclared in this block"),
    ("tests/spellings/go_date_type.cal", "Date redeclared in this block"),
    ("tests/spellings/go_test_main.cal", "TestMain redeclared in this block"),
    ("tests/spellings/go_package_keyword.cal", "expected 'IDENT', found 'goto'"),
    ("tests/spellings/go_package_main.cal", "is a program, not an importable package"),
    ("tests/spellings/postgresql_functions.cal", ""),
    ("tests/spellings/postgresql_time_function.cal", "cannot change return type of existing function"),
    ("tests/spellings/postgresql_parameters.cal", "used more than once"),
    ("tests/spellings/postgresql_parameter_names.cal", "cannot change name of input parameter"),
    ("tests/spellings/rust_function.cal", "should have a snake case name"),
    ("tests/spellings/rust_time_function.cal", "should have a snake case name"),
    ("tests/spellings/rust_parameter.cal", "should have a snake case name"),
    ("tests/spellings/rust_module.cal", "should have a snake case name"),
    ("tests/spellings/python_library_module.cal", "has no attribute"),
    ("tests/spellings/Goの関数.cal", "Net30 redeclared in this block"),
    ("tests/spellings/Goの時刻の関数.cal", "CutoffAt redeclared in this block"),
    ("tests/spellings/Goの日付の型.cal", "Date redeclared in this block"),
    ("tests/spellings/GoのTestMain.cal", "TestMain redeclared in this block"),
    ("tests/spellings/Goのキーワードのパッケージ.cal", "expected 'IDENT', found 'goto'"),
    ("tests/spellings/Goのmainのパッケージ.cal", "is a program, not an importable package"),
    ("tests/spellings/PostgreSQLの関数.cal", ""),
    ("tests/spellings/PostgreSQLの時刻の関数.cal", "cannot change return type of existing function"),
    ("tests/spellings/PostgreSQLの引数.cal", "used more than once"),
    ("tests/spellings/PostgreSQLの引数の名前.cal", "cannot change name of input parameter"),
    ("tests/spellings/Rustの関数.cal", "should have a snake case name"),
    ("tests/spellings/Rustの時刻の関数.cal", "should have a snake case name"),
    ("tests/spellings/Rustの引数.cal", "should have a snake case name"),
    ("tests/spellings/Rustのモジュール.cal", "should have a snake case name"),
    ("tests/spellings/Pythonの標準ライブラリのモジュール.cal", "has no attribute"),
];

/// The same files with the alias their W001's note gives (`tests/spellings/renamed/`): `check`
/// says nothing of them, and tests/targets.rs runs every one through all six targets.
pub fn renamed(path: &str) -> String {
    path.replacen("tests/spellings/", "tests/spellings/renamed/", 1)
}

/// Whether rustc has the standard library of `target` installed (`rustup target add <target>`).
pub fn rust_target(target: &str) -> bool {
    let Ok(o) = std::process::Command::new("rustc").args(["--print", "sysroot"]).output() else { return false };
    if !o.status.success() {
        return false;
    }
    let root = String::from_utf8_lossy(&o.stdout).trim().to_string();
    std::path::Path::new(&root).join("lib/rustlib").join(target).join("lib").is_dir()
}
