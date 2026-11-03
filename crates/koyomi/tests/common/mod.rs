//! What koyomi's tests share besides ritsu-testkit (DESIGN 10.8): the files the generated code
//! is held to, and whether rustc can build for a target.

#![allow(dead_code)]

use koyomi::naming::Target::{self, Go, Python, Rust, Sql, TypeScript, Wasm};

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

/// The files of `tests/spellings/` (DESIGN 15, 17): each writes a name some targets spell as another
/// one, or cannot take, and `check` warns about it once (W001), in English and in Japanese. Beside
/// each are the targets W001 names, with what the tool of each says of the code: the line it stops
/// at (for Python, what the runner stops with, or what mypy says when the code answers as it should;
/// for the Wasm module of `__proto__`, what it answers a host), or nothing when the code loads and
/// answers for one function with another. tests/targets.rs runs each file through the targets W001
/// does not name, and the named ones with this line.
pub const SPELLING_FILES: &[(&str, &[(Target, &str)])] = &[
    ("tests/spellings/go_functions.cal", &[(Go, "Net30 redeclared in this block")]),
    ("tests/spellings/go_time_function.cal", &[(Go, "CutoffAt redeclared in this block")]),
    ("tests/spellings/go_date_type.cal", &[(Go, "Date redeclared in this block")]),
    ("tests/spellings/go_error_type.cal", &[(Go, "KoyomiError redeclared in this block")]),
    ("tests/spellings/go_parse_date.cal", &[(Go, "ParseDate redeclared in this block")]),
    ("tests/spellings/go_test_main.cal", &[(Go, "TestMain redeclared in this block")]),
    ("tests/spellings/go_err_parameter.cal", &[(Go, "value of interface type error")]),
    ("tests/spellings/go_package_keyword.cal", &[(Go, "expected 'IDENT', found 'goto'")]),
    ("tests/spellings/go_package_main.cal", &[(Go, "is a program, not an importable package")]),
    ("tests/spellings/calendar_function.cal", CALENDAR_FUNCTION),
    ("tests/spellings/postgresql_functions.cal", &[(Sql, "")]),
    ("tests/spellings/postgresql_time_function.cal", &[(Sql, "cannot change return type of existing function")]),
    ("tests/spellings/postgresql_parameters.cal", &[(Sql, "used more than once")]),
    ("tests/spellings/postgresql_parameter_names.cal", &[(Sql, "cannot change name of input parameter")]),
    ("tests/spellings/postgresql_schema.cal", &[(Sql, "unacceptable schema name")]),
    ("tests/spellings/rust_function.cal", &[(Rust, SNAKE), (Wasm, SNAKE)]),
    ("tests/spellings/rust_time_function.cal", &[(Rust, SNAKE), (Wasm, SNAKE)]),
    ("tests/spellings/rust_parameter.cal", &[(Rust, SNAKE), (Wasm, SNAKE)]),
    ("tests/spellings/rust_module.cal", &[(Rust, SNAKE)]),
    ("tests/spellings/python_library_module.cal", &[(Python, "has no attribute")]),
    ("tests/spellings/python_imported_module.cal", &[(Python, "cannot import name")]),
    ("tests/spellings/python_str.cal", &[(Python, "missing 1 required positional argument")]),
    ("tests/spellings/python_isinstance.cal", &[(Python, "has no attribute 'year'")]),
    ("tests/spellings/python_tuple.cal", &[(Python, "is not valid as a type")]),
    ("tests/spellings/wasm_proto_input.cal", &[(Wasm, PROTO_MISSING)]),
    ("tests/spellings/Goの関数.cal", &[(Go, "Net30 redeclared in this block")]),
    ("tests/spellings/Goの時刻の関数.cal", &[(Go, "CutoffAt redeclared in this block")]),
    ("tests/spellings/Goの日付の型.cal", &[(Go, "Date redeclared in this block")]),
    ("tests/spellings/Goのエラーの型.cal", &[(Go, "KoyomiError redeclared in this block")]),
    ("tests/spellings/Goの日付を読む関数.cal", &[(Go, "ParseDate redeclared in this block")]),
    ("tests/spellings/GoのTestMain.cal", &[(Go, "TestMain redeclared in this block")]),
    ("tests/spellings/Goの引数err.cal", &[(Go, "value of interface type error")]),
    ("tests/spellings/Goのキーワードのパッケージ.cal", &[(Go, "expected 'IDENT', found 'goto'")]),
    ("tests/spellings/Goのmainのパッケージ.cal", &[(Go, "is a program, not an importable package")]),
    ("tests/spellings/カレンダーの関数.cal", CALENDAR_FUNCTION),
    ("tests/spellings/PostgreSQLの関数.cal", &[(Sql, "")]),
    ("tests/spellings/PostgreSQLの時刻の関数.cal", &[(Sql, "cannot change return type of existing function")]),
    ("tests/spellings/PostgreSQLの引数.cal", &[(Sql, "used more than once")]),
    ("tests/spellings/PostgreSQLの引数の名前.cal", &[(Sql, "cannot change name of input parameter")]),
    ("tests/spellings/PostgreSQLのスキーマ.cal", &[(Sql, "unacceptable schema name")]),
    ("tests/spellings/Rustの関数.cal", &[(Rust, SNAKE), (Wasm, SNAKE)]),
    ("tests/spellings/Rustの時刻の関数.cal", &[(Rust, SNAKE), (Wasm, SNAKE)]),
    ("tests/spellings/Rustの引数.cal", &[(Rust, SNAKE), (Wasm, SNAKE)]),
    ("tests/spellings/Rustのモジュール.cal", &[(Rust, SNAKE)]),
    ("tests/spellings/Pythonの標準ライブラリのモジュール.cal", &[(Python, "has no attribute")]),
    ("tests/spellings/Pythonの読み込むモジュール.cal", &[(Python, "cannot import name")]),
    ("tests/spellings/Pythonの組み込みのstr.cal", &[(Python, "missing 1 required positional argument")]),
    ("tests/spellings/Pythonの組み込みのisinstance.cal", &[(Python, "has no attribute 'year'")]),
    ("tests/spellings/Pythonの組み込みのtuple.cal", &[(Python, "is not valid as a type")]),
    ("tests/spellings/Wasmの入力の__proto__.cal", &[(Wasm, PROTO_MISSING)]),
];

const SNAKE: &str = "should have a snake case name";

/// What the Wasm module of an input named `__proto__` answers a host that writes the inputs as an
/// object literal (tests/wasm.rs): the code builds.
pub const PROTO_MISSING: &str = "__proto__ is missing";

const CALENDAR_FUNCTION: &[(Target, &str)] = &[
    (TypeScript, "has already been declared"),
    (Python, "takes 1 positional argument"),
    (Go, "IsOpen redeclared in this block"),
    (Rust, "defined multiple times"),
    (Wasm, "defined multiple times"),
];

/// The targets a file of [`SPELLING_FILES`] is about.
pub fn targets_of(path: &str) -> Vec<Target> {
    SPELLING_FILES.iter().find(|(p, _)| *p == path).unwrap().1.iter().map(|(t, _)| *t).collect()
}

/// The same files with the alias their W001's note gives (`tests/spellings/renamed/`): `check`
/// says nothing of them, and tests/targets.rs runs every one through all six targets.
pub fn renamed(path: &str) -> String {
    path.replacen("tests/spellings/", "tests/spellings/renamed/", 1)
}

/// Two files of `tests/spellings/across/` given together, in this order, whose names are one in the
/// targets beside them (DESIGN 17): the later file's W001 names each, and the earlier file. Each
/// later file has a version with the alias the note gives (`tests/spellings/across/renamed/`):
/// given after the same earlier file, `check` says nothing of it.
pub const ACROSS: &[(&str, &str, &[Target])] = &[
    ("tests/spellings/across/pay_days.cal", "tests/spellings/across/paydays.cal", &[Go]),
    ("tests/spellings/across/pay_days.cal", "tests/spellings/across/pay_days_.cal", &[Go, Wasm]),
    ("tests/spellings/across/long_first.cal", "tests/spellings/across/long_second.cal", &[Sql]),
    ("tests/spellings/across/支払日.cal", "tests/spellings/across/支払日二.cal", &[Go]),
    ("tests/spellings/across/支払日.cal", "tests/spellings/across/支払日三.cal", &[Go, Wasm]),
    ("tests/spellings/across/長い別名一.cal", "tests/spellings/across/長い別名二.cal", &[Sql]),
];

/// The version of a later file of [`ACROSS`] with the alias the note gives.
pub fn renamed_across(path: &str) -> String {
    path.replacen("tests/spellings/across/", "tests/spellings/across/renamed/", 1)
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
