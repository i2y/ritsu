//! The names a target spells as one identifier, or cannot take (W001 and E016, DESIGN 15, 17). Every
//! file of `tests/spellings/` gives W001 once, naming the targets `common::SPELLING_FILES` says, and what `check`
//! prints of it, in English and in Japanese, is its golden file in `tests/golden/spellings/`
//! (`KOYOMI_BLESS=1 cargo test` writes them again; read the diff). `gen` stops at the targets it
//! names (E016) and writes the others. The same file with the alias the note gives
//! (`tests/spellings/renamed/`) gives nothing. That the named target breaks, and that the others
//! take both files, is tests/targets.rs's. The files of `tests/spellings/across/` are the forms of
//! files given together.

mod common;

use koyomi::check::{Checked, Options, check, check_text, render};
use koyomi::codegen;
use common::targets_of;
use koyomi::naming::{TARGETS, Target};
use ritsu_base::text::Lang;
use std::path::Path;
use std::process::{Command, Output};

/// Whether a package of ritsu's breaks where the file's W001 says: every form but the Python
/// module named like one of the standard library's, which a package holds as
/// `<package>.dates.<alias>`.
fn breaks_in_a_package(path: &str) -> bool {
    !["python_library_module", "python_imported_module", "Pythonの標準ライブラリのモジュール", "Pythonの読み込むモジュール"].iter().any(|n| path.ends_with(&format!("/{n}.cal")))
}

fn koyomi(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_koyomi")).args(args).env_remove("KOYOMI_LANG").env_remove("RITSU_LANG").output().unwrap()
}

#[test]
fn every_file_gives_w001_once_for_its_target_and_says_what_its_golden_files_say() {
    let mut failures = Vec::new();
    for (path, _) in common::SPELLING_FILES {
        let o = check(path).unwrap();
        let codes: Vec<&str> = o.diags.iter().map(|d| d.code).collect();
        if codes != ["W001"] {
            failures.push(format!("{path} gives {codes:?}, not W001 once"));
        }
        if o.has_errors() {
            failures.push(format!("{path} does not pass check"));
        }
        if let Some(Checked::Dates(m, _)) = &o.checked {
            let named: Vec<Target> = koyomi::spellings::find(m).into_iter().flat_map(|f| f.targets).collect();
            if named != targets_of(path) {
                failures.push(format!("{path}: W001 names {named:?}"));
            }
        } else {
            failures.push(format!("{path} is not a dates file that passes check"));
        }
        let stem = path.rsplit('/').next().unwrap().trim_end_matches(".cal");
        for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
            let golden = format!("tests/golden/spellings/{stem}.{tag}.txt");
            if let Err(e) = ritsu_testkit::golden::check(Path::new(&golden), &render(&o, lang)) {
                failures.push(format!("{path} ({tag}): {e}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// E016: the build of each target a file's W001 names stops, with the warning made an error at the
/// same place, with the same headline and reasons, and in place of the last note what to do; every
/// other target builds, with the names as they are written. A package of ritsu's stops alike, but
/// for the Python module named like one of the standard library's.
#[test]
fn the_targets_w001_names_stop_and_the_others_build() {
    let mut failures = Vec::new();
    for (path, _) in common::SPELLING_FILES {
        let o = check(path).unwrap();
        let w = &o.diags[0];
        let u = codegen::unit_of(o.checked.as_ref().unwrap(), Lang::En);
        let named = targets_of(path);
        for t in TARGETS {
            match codegen::build(&o, &u, t, false) {
                Err(ds) if named.contains(&t) => {
                    let [e] = ds.as_slice() else {
                        failures.push(format!("{path} {}: {} diagnostics, not E016 once", t.key(), ds.len()));
                        continue;
                    };
                    let reasons = |n: &[ritsu_base::text::Text]| n[..n.len() - 1].to_vec();
                    if e.code != "E016" || !e.is_error() || e.place() != w.place() || e.message != w.message || e.notes.len() != w.notes.len() || reasons(&e.notes) != reasons(&w.notes) {
                        failures.push(format!("{path} {}: E016 says otherwise than W001:\n{}{}", t.key(), e.render(Lang::En), w.render(Lang::En)));
                    }
                    let last = &e.notes[e.notes.len() - 1];
                    if !last.en.ends_with(". For a target you do not use, name the ones you use with `--target`.") || !last.ja.ends_with("。使わない出力先なら、`--target` で使う出力先を挙げてください。") {
                        failures.push(format!("{path} {}: the last note of E016 is {last:?}", t.key()));
                    }
                }
                Ok(files) if !named.contains(&t) => {
                    if files.is_empty() {
                        failures.push(format!("{path}: gen writes nothing for {}", t.key()));
                    }
                }
                Ok(_) => failures.push(format!("{path}: {} builds, though W001 names it", t.key())),
                Err(ds) => failures.push(format!("{path}: {} stops, though W001 does not name it:\n{}", t.key(), ds[0].render(Lang::En))),
            }
        }
        // The aliases are in the code as they are written: every target but Go has each.
        if let Ok(ts) = codegen::build(&o, &u, Target::TypeScript, false) {
            for d in &u.dates {
                if !ts.iter().any(|(_, body)| body.contains(&format!("export function {}(", d.alias))) {
                    failures.push(format!("{path}: {} is not in the TypeScript", d.alias));
                }
            }
        }
        // The targets a package of ritsu's is made in.
        for t in [Target::TypeScript, Target::Python, Target::Go] {
            let stops = codegen::package_module(&o, &u, t).is_err();
            if stops != (named.contains(&t) && breaks_in_a_package(path)) {
                failures.push(format!("{path}: the package's {} {}", t.key(), if stops { "stops" } else { "builds" }));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// `koyomi gen` writes no file when a target it is to write stops (E016): given every target, it prints
/// each E016 once (one finding names Rust and Wasm alike), says on the standard error which targets
/// stop, and exits 1, and `--check` stops the same way. With `--target` naming another target, it
/// writes that one.
#[test]
fn gen_writes_nothing_when_a_target_it_writes_stops() {
    let out = ritsu_testkit::TempDir::new("spellings-gen");
    let dir = out.path().to_str().unwrap();
    for (path, _) in common::SPELLING_FILES {
        let named = targets_of(path);
        for check in [false, true] {
            let mut args = koyomi::cli::up_to_0_40(&["gen", path, "--out", dir]);
            if check {
                args.push("--check");
            }
            let o = koyomi(&args);
            let (stdout, stderr) = (String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
            assert_eq!(o.status.code(), Some(1), "{path}: {stdout}{stderr}");
            assert_eq!(stdout.matches("error[E016]: ").count(), 1, "{path}: {stdout}");
            let keys: Vec<String> = named.iter().map(|t| format!("`{}`", t.key())).collect();
            assert_eq!(stderr, format!("error: `{path}` cannot be made into the code of {}, so nothing is generated\n", keys.join(", ")), "{path}");
            assert!(std::fs::read_dir(dir).unwrap().next().is_none(), "{path}: gen wrote a file");
        }
        let other = TARGETS.iter().find(|t| !named.contains(t)).unwrap();
        let o = koyomi(&["gen", path, "--out", dir, "--target", other.key()]);
        assert_eq!(o.status.code(), Some(0), "{path} --target {}: {}", other.key(), String::from_utf8_lossy(&o.stderr));
        std::fs::remove_dir_all(dir).unwrap();
        std::fs::create_dir(dir).unwrap();
        let o = koyomi(&["gen", path, "--out", dir, "--target", named[0].key()]);
        assert_eq!(o.status.code(), Some(1), "{path} --target {}", named[0].key());
        assert!(std::fs::read_dir(dir).unwrap().next().is_none(), "{path} --target {}: gen wrote a file", named[0].key());
    }
    // Japanese
    let o = koyomi(&koyomi::cli::up_to_0_40(&["gen", "tests/spellings/Rustの関数.cal", "--out", dir, "--lang", "ja"]));
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stdout).starts_with("エラー[E016]: tests/spellings/Rustの関数.cal:"));
    assert_eq!(String::from_utf8_lossy(&o.stderr), "エラー: `tests/spellings/Rustの関数.cal` からは `rust`、`wasm` のコードを生成できないので、何も生成しません\n");
    // A file that stops a target stops the run, the files beside it included.
    let o = koyomi(&koyomi::cli::up_to_0_40(&["gen", "tests/spellings/renamed/go_functions.cal", "tests/spellings/go_functions.cal", "--out", dir]));
    assert_eq!(o.status.code(), Some(1));
    assert!(std::fs::read_dir(dir).unwrap().next().is_none(), "gen wrote the renamed file");
    let o = koyomi(&koyomi::cli::up_to_0_40(&["gen", "tests/spellings/renamed/go_functions.cal", "--out", dir]));
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn the_renamed_files_give_nothing() {
    for (path, _) in common::SPELLING_FILES {
        let p = common::renamed(path);
        let o = check(&p).unwrap();
        let codes: Vec<&str> = o.diags.iter().map(|d| d.code).collect();
        assert!(codes.is_empty(), "{p} gives {codes:?}");
    }
}

/// Every file under tests/spellings is listed, and every Japanese one has the English one beside
/// it, in the same place of the list (English first).
#[test]
fn every_file_is_listed_in_english_and_in_japanese() {
    let listed: Vec<&str> = common::SPELLING_FILES.iter().map(|(p, _)| *p).collect();
    for dir in ["tests/spellings", "tests/spellings/renamed"] {
        for e in std::fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            if e.path().is_dir() {
                continue;
            }
            let p = format!("tests/spellings/{}", e.file_name().to_string_lossy());
            assert!(listed.contains(&p.as_str()), "{dir}/{} is not in common::SPELLING_FILES", e.file_name().to_string_lossy());
        }
    }
    let (en, ja): (Vec<&str>, Vec<&str>) = listed.iter().partition(|p| p.is_ascii());
    assert_eq!(en.len(), ja.len(), "as many Japanese files as English ones");
    for (e, j) in en.iter().zip(&ja) {
        assert_eq!(targets_of(e), targets_of(j), "{e} and {j} are about the same targets");
        let says = |f: &str| common::SPELLING_FILES.iter().find(|(p, _)| *p == f).unwrap().1;
        assert_eq!(says(e), says(j), "{e} and {j} stop the targets alike");
    }
}

/// The names the Go code declares beside the dates' functions that no file of tests/spellings
/// takes as written: `ParseDate`, `KoyomiError` and `IsOpen` as other aliases spell them, and `IsOpen`,
/// which it declares when the file has a calendar.
#[test]
fn the_other_names_the_go_code_declares() {
    let warned = |body: &str| -> Vec<String> {
        let o = check_text("tests/spellings/inline.cal", body, &Options::default());
        assert!(!o.has_errors(), "{body}");
        o.diags.iter().filter(|d| d.code == "W001").map(|d| d.render(Lang::En)).collect()
    };
    let with = |date: &str, cal: bool| {
        let calendar = if cal { "use calendar \"../fixtures/calendars/weekends.cal\"\n" } else { "" };
        format!("dates inline v1\n{calendar}\ninputs\n  received : date  range >=2026-01-01 <=2026-01-31\n\ndate {date} = received\n  + 1 day\n")
    };
    for (alias, own) in [("parse_date_", "ParseDate"), ("koyomi_error_", "KoyomiError")] {
        let w = warned(&with(alias, false));
        assert!(w.len() == 1 && w[0].contains(&format!("`{own}`")), "{alias}: {w:?}");
    }
    let w = warned(&with("is_open_", true));
    assert!(w.len() == 1 && w[0].contains("`IsOpen`"), "is_open_ with a calendar: {w:?}");
    assert!(warned(&with("is_open_", false)).is_empty(), "with no calendar, the Go code declares no IsOpen");
    // A calendar file's alias is a Go package as well.
    let o = check_text("tests/spellings/inline.cal", "calendar for_ v1\n\nclosed weekly sat, sun\n", &Options::default());
    let codes: Vec<&str> = o.diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, ["W001"], "a calendar named for_ is the Go package for");
}

/// `is_open`, the function of the calendar: with no calendar it is a name like any other; beside a
/// calendar it is E009 when the date takes the date input alone, as every target's code then
/// declares the function twice (PostgreSQL's `is_open(day date)` has the same types), and W001 of
/// the five that have the name twice when the date takes an integer input as well.
#[test]
fn is_open_beside_a_calendar() {
    let with = |cal: bool, n: bool| {
        let calendar = if cal { "use calendar \"../fixtures/calendars/weekends.cal\"\n" } else { "" };
        let input = if n { "  n : int range >=0 <=3\n" } else { "" };
        let plus = if n { "n" } else { "1" };
        format!("dates inline v1\n{calendar}\ninputs\n  received : date  range >=2026-01-01 <=2026-01-31\n{input}\ndate is_open = received\n  + {plus} days\n")
    };
    let check_of = |body: &str| check_text("tests/spellings/inline.cal", body, &Options::default());
    let o = check_of(&with(false, false));
    assert!(o.diags.is_empty(), "{:?}", o.diags.iter().map(|d| d.code).collect::<Vec<_>>());
    let o = check_of(&with(false, true));
    assert!(o.diags.is_empty(), "{:?}", o.diags.iter().map(|d| d.code).collect::<Vec<_>>());
    let o = check_of(&with(true, false));
    let codes: Vec<&str> = o.diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, ["E009"]);
    for (lang, says) in [(Lang::En, "The alias is_open collides with is_open, the function of the calendar"), (Lang::Ja, "別名 is_open は、カレンダーの関数 is_open とぶつかります")] {
        assert!(o.diags[0].render(lang).contains(says), "{}", o.diags[0].render(lang));
    }
    let o = check_of(&with(true, true));
    let codes: Vec<&str> = o.diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, ["W001"]);
    assert_eq!(o.spelled[0].targets, [Target::TypeScript, Target::Python, Target::Go, Target::Rust, Target::Wasm]);
}

/// The names the code of no target breaks on, which E009 used to stop: nothing is said of them. And
/// the ones `check` still stops, whatever the target: a word of koyomi or a reserved word.
#[test]
fn the_names_nothing_breaks_on_and_the_words_no_target_takes() {
    let codes = |body: &str| -> Vec<&'static str> { check_text("tests/spellings/inline.cal", body, &Options::default()).diags.iter().map(|d| d.code).collect() };
    let date = |alias: &str| format!("dates inline v1\n\ninputs\n  received : date  range >=2026-01-01 <=2026-01-31\n\ndate {alias} = received\n  + 30 days\n");
    let input = |alias: &str| format!("dates inline v1\n\ninputs\n  received({alias}) : date  range >=2026-01-01 <=2026-01-31\n\ndate due = received\n  + 30 days\n");
    let file = |alias: &str| format!("dates {alias} v1\n\ninputs\n  received : date  range >=2026-01-01 <=2026-01-31\n\ndate due = received\n  + 30 days\n");
    // A date's function: `isinstance` only hides what a file with an integer input calls, and the
    // Python module's `frozenset` and imports are read before the date's function is.
    for alias in ["datetime", "timedelta", "frozenset", "err", "isinstance"] {
        assert!(codes(&date(alias)).is_empty(), "the date {alias}");
    }
    // A parameter is the function's own.
    for alias in ["datetime", "timedelta", "frozenset", "isinstance", "str", "tuple", "is_open", "koyomi_error", "parse_date"] {
        assert!(codes(&input(alias)).is_empty(), "the input {alias}");
    }
    assert_eq!(codes(&input("err")), ["W001"]);
    // A file's alias: Rust's `std`, `core` and `alloc` are other paths than the module's.
    for alias in ["std", "core", "alloc"] {
        assert!(codes(&file(alias)).is_empty(), "the file {alias}");
    }
    for (alias, why) in [("date", "date"), ("range", "date"), ("int", "date"), ("bool", "date"), ("super", "date"), ("json", "file"), ("public", "file")] {
        let body = if why == "date" { date(alias) } else { file(alias) };
        assert_eq!(codes(&body), ["E009"], "{alias}");
    }
}

/// A file's alias that names one of PostgreSQL's own schemas: `pg_…`, and `information_schema`,
/// which only a superuser creates a function in (tests/targets.rs). A calendar's alias is a schema
/// as well. A name that only looks like them is not told.
#[test]
fn the_schemas_of_postgresql() {
    let found = |body: &str| -> Vec<(Vec<Target>, String)> { check_text("tests/spellings/inline.cal", body, &Options::default()).spelled.into_iter().map(|f| (f.targets, f.diag.render(Lang::En))).collect() };
    let dates = |alias: &str| format!("dates {alias} v1\n\ninputs\n  received : date  range >=2026-01-01 <=2026-01-31\n\ndate due = received\n  + 30 days\n");
    for alias in ["pg_catalog", "pg_toast", "pg_temp", "pg_terms", "information_schema"] {
        let w = found(&dates(alias));
        assert!(w.len() == 1 && w[0].0 == [Target::Sql] && w[0].1.contains("a name of PostgreSQL's own schemas"), "{alias}: {w:?}");
    }
    for alias in ["pgterms", "information_schemas", "my_pg_terms"] {
        assert!(found(&dates(alias)).is_empty(), "{alias}");
    }
    let w = found("calendar pg_days v1\n\nclosed weekly sat, sun\n");
    assert!(w.len() == 1 && w[0].0 == [Target::Sql], "{w:?}");
}

/// The files given together (DESIGN 17): the later file's W001 names the targets beside the pair, and
/// the earlier file; the earlier file's output does not change, and what `check` prints of the two,
/// in English and in Japanese, is the golden file of the later one in
/// `tests/golden/spellings/across/`. `gen` of a named target stops at the later file (E016) and writes
/// nothing; `gen` of the others writes both; and given the other way round, the warning is of the
/// other file. The version of the later file with the alias the note gives is told nothing, and
/// `gen` of every target writes both.
#[test]
fn the_files_given_together() {
    let out = ritsu_testkit::TempDir::new("spellings-across");
    let dir = out.path().to_str().unwrap();
    let mut failures = Vec::new();
    for (first, later, named) in common::ACROSS {
        let stem = later.rsplit('/').next().unwrap().trim_end_matches(".cal");
        for (lang, tag) in [("en", "en"), ("ja", "ja")] {
            let o = koyomi(&["check", first, later, "--lang", lang]);
            assert_eq!(o.status.code(), Some(0), "{first} {later}");
            let stdout = String::from_utf8_lossy(&o.stdout);
            let golden = format!("tests/golden/spellings/across/{stem}.{tag}.txt");
            if let Err(e) = ritsu_testkit::golden::check(Path::new(&golden), &stdout) {
                failures.push(format!("{later} ({tag}): {e}"));
            }
            let alone = koyomi(&["check", first, "--lang", lang]);
            let alone = String::from_utf8_lossy(&alone.stdout);
            assert!(stdout.starts_with(alone.as_ref()), "{first} says the same with or without {later}");
        }
        let o = koyomi(&["check", first, later, "--format", "json"]);
        let lines: Vec<serde_json::Value> = String::from_utf8_lossy(&o.stdout).lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines[0]["diagnostics"].as_array().unwrap().len(), 0, "{first}");
        assert_eq!(lines[1]["diagnostics"].as_array().unwrap().len(), named.len(), "{later}");
        assert_eq!(lines[1]["ok"], true, "{later}");
        for t in *named {
            let o = koyomi(&["gen", first, later, "--out", dir, "--target", t.key()]);
            let stdout = String::from_utf8_lossy(&o.stdout);
            assert_eq!(o.status.code(), Some(1), "{later} {}: {stdout}", t.key());
            assert!(stdout.starts_with(&format!("error[E016]: {later}:")), "{later} {}: {stdout}", t.key());
            assert!(stdout.contains(first), "{later} {}: E016 names the earlier file: {stdout}", t.key());
            assert!(std::fs::read_dir(dir).unwrap().next().is_none(), "{later} {}: gen wrote a file", t.key());
        }
        for t in TARGETS.iter().filter(|t| !named.contains(t)) {
            let o = koyomi(&["gen", first, later, "--out", dir, "--target", t.key()]);
            assert_eq!(o.status.code(), Some(0), "{later} {}: {}", t.key(), String::from_utf8_lossy(&o.stdout));
        }
        std::fs::remove_dir_all(dir).unwrap();
        std::fs::create_dir(dir).unwrap();
        // the other way round
        let o = koyomi(&["check", later, first, "--format", "json"]);
        let lines: Vec<serde_json::Value> = String::from_utf8_lossy(&o.stdout).lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines[0]["diagnostics"].as_array().unwrap().len(), 0, "{later}, given first");
        assert_eq!(lines[1]["diagnostics"].as_array().unwrap().len(), named.len(), "{first}, given second");
        // with the alias the note gives
        let renamed = common::renamed_across(later);
        let o = koyomi(&["check", first, &renamed, "--format", "json"]);
        for l in String::from_utf8_lossy(&o.stdout).lines() {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            assert_eq!(v["diagnostics"].as_array().unwrap().len(), 0, "{renamed}: {l}");
        }
        let mut args = vec!["gen", first, &renamed, "--out", dir];
        for t in TARGETS {
            args.extend(["--target", t.key()]);
        }
        let o = koyomi(&args);
        assert_eq!(o.status.code(), Some(0), "{renamed}: {}", String::from_utf8_lossy(&o.stdout));
        std::fs::remove_dir_all(dir).unwrap();
        std::fs::create_dir(dir).unwrap();
    }
    // Two files of one alias are not compared: every target has the one name for both, so `gen` says
    // both write it, and `check` of a directory can hold the versions of one file.
    let same = out.path().join("same.cal");
    std::fs::copy(common::ACROSS[0].0, &same).unwrap();
    let same = same.to_string_lossy().to_string();
    let o = koyomi(&["check", common::ACROSS[0].0, &same]);
    assert!(!String::from_utf8_lossy(&o.stdout).contains("W001"), "{}", String::from_utf8_lossy(&o.stdout));
    // A calendar is compared too: its alias makes a package and a schema as a dates file's does.
    std::fs::write(out.path().join("paydays.cal"), "calendar paydays v1\n\nclosed weekly sat, sun\n").unwrap();
    let cal = out.path().join("paydays.cal").to_string_lossy().to_string();
    let o = koyomi(&["check", common::ACROSS[0].0, &cal]);
    assert!(String::from_utf8_lossy(&o.stdout).contains("W001"), "{}", String::from_utf8_lossy(&o.stdout));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// What rustc's lint `non_snake_case` takes as it is: an alias with no two `_` in a row once the
/// `_` at its ends are left out. A date with a time is told once, of its own function when its
/// alias has two `_` in a row and of its time's when only `<alias>_at` has; and a calendar file's
/// alias is the module Rust declares it as.
#[test]
fn the_names_rustc_warns_of() {
    use koyomi::spellings::rust_snake;
    for ok in ["payment", "payment_", "payment_terms_", "x_1_2", "net_30"] {
        assert!(rust_snake(ok), "{ok}");
    }
    for warned in ["payment__terms", "a__b_", "cutoff__at"] {
        assert!(!rust_snake(warned), "{warned}");
    }
    let named = |body: &str| -> Vec<(Vec<Target>, String)> {
        let o = check_text("tests/spellings/inline.cal", body, &Options::default());
        assert!(!o.has_errors(), "{body}");
        let Some(Checked::Dates(m, _)) = &o.checked else { panic!("{body}") };
        koyomi::spellings::find(m).into_iter().map(|f| (f.targets, f.diag.render(Lang::En))).collect()
    };
    let at = |alias: &str| format!("dates inline v1\nuse calendar \"../fixtures/calendars/weekends.cal\"\n\ninputs\n  received : date  range >=2026-01-01 <=2026-01-31\n\ndate {alias} = received\n  roll following\n  at 17:00\n");
    let w = named(&at("cut__off"));
    assert!(w.len() == 1 && w[0].0 == [Target::Rust, Target::Wasm] && w[0].1.contains("`cut__off` is a name rustc warns of"), "cut__off: {w:?}");
    let w = named(&at("cutoff_"));
    assert!(w.len() == 1 && w[0].1.contains("`cutoff__at` is a name rustc warns of"), "cutoff_: {w:?}");
    assert!(named(&at("cutoff")).is_empty());
    let o = check_text("tests/spellings/inline.cal", "calendar cal__x v1\n\nclosed weekly sat, sun\n", &Options::default());
    let codes: Vec<&str> = o.diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, ["W001"], "a calendar named cal__x is the Rust module cal__x");
    assert!(o.diags[0].render(Lang::En).contains("is the module's name, which rustc warns of"));
}

/// A file's alias that is a module of Python's standard library, as `sys.stdlib_module_names` lists
/// it: a calendar's too, since a calendar is a Python module as well. The ones the generated code
/// imports itself are told the same, with a note that says so; `json` is a reserved word of
/// PostgreSQL (E009). A name that only starts like one is not told.
#[test]
fn the_modules_of_pythons_standard_library() {
    let codes = |body: &str| -> Vec<&'static str> { check_text("tests/spellings/inline.cal", body, &Options::default()).diags.iter().map(|d| d.code).collect() };
    let dates = |alias: &str| format!("dates {alias} v1\n\ninputs\n  received : date  range >=2026-01-01 <=2026-01-31\n\ndate due = received\n  + 30 days\n");
    for alias in ["collections", "site", "random", "queue", "secrets", "datetime", "sys", "typing"] {
        assert_eq!(codes(&dates(alias)), ["W001"], "{alias}");
    }
    assert_eq!(codes(&dates("json")), ["E009"]);
    for alias in ["collections_", "my_site", "secret"] {
        assert!(codes(&dates(alias)).is_empty(), "{alias}");
    }
    let o = check_text("tests/spellings/inline.cal", "calendar secrets v1\n\nclosed weekly sat, sun\n", &Options::default());
    let w: Vec<String> = o.diags.iter().map(|d| d.render(Lang::En)).collect();
    assert!(w.len() == 1 && w[0].contains("the file's alias `secrets` is the module's name, which a module of Python's standard library has"), "{w:?}");
    let imports = |alias: &str, says: &str| {
        let o = check_text("tests/spellings/inline.cal", &dates(alias), &Options::default());
        assert!(o.diags[0].render(Lang::En).contains(says), "{alias}: {}", o.diags[0].render(Lang::En));
    };
    imports("datetime", "imports the standard library's `datetime` itself");
    imports("sys", "The runner `gen` writes beside the module imports the standard library's `sys` too");
    imports("typing", "The runner `gen` writes beside the module imports the standard library's `typing` too");
}

/// The JSON of W001: a warning, with the keys every diagnostic has.
#[test]
fn the_json_of_w001() {
    let o = check("tests/spellings/go_functions.cal").unwrap();
    let v = koyomi::check::to_json(&o, Lang::En);
    let d = &v["diagnostics"][0];
    assert_eq!(d["code"], "W001");
    assert_eq!(d["severity"], "warning");
    assert_eq!(v["ok"], true);
    assert_eq!(d["notes"].as_array().unwrap().len(), 2);
}
