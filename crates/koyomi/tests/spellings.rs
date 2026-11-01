//! The names a target spells as one identifier, or cannot take (W001 and E016, DESIGN 15). Every
//! file of `tests/spellings/` gives W001 once, naming the targets its name says, and what `check`
//! prints of it, in English and in Japanese, is its golden file in `tests/golden/spellings/`
//! (`KOYOMI_BLESS=1 cargo test` writes them again; read the diff). `gen` stops at the targets it
//! names (E016) and writes the others. The same file with the alias the note gives
//! (`tests/spellings/renamed/`) gives nothing. That the named target breaks, and that the others
//! take both files, is tests/targets.rs's.

mod common;

use koyomi::check::{Checked, Options, check, check_text, render};
use koyomi::codegen;
use koyomi::naming::{TARGETS, Target};
use ritsu_base::text::Lang;
use std::path::Path;
use std::process::{Command, Output};

/// The targets a file of tests/spellings is about, by its name: Go, PostgreSQL, Python, or Rust
/// with the Wasm module built from the same code (but for the module's own name, which only Rust
/// declares).
fn targets_of(path: &str) -> Vec<Target> {
    let name = path.rsplit('/').next().unwrap();
    if name.starts_with("go_") || name.starts_with("Go") {
        vec![Target::Go]
    } else if name.starts_with("postgresql_") || name.starts_with("PostgreSQL") {
        vec![Target::Sql]
    } else if name.starts_with("python_") || name.starts_with("Python") {
        vec![Target::Python]
    } else if name.starts_with("rust_module") || name.starts_with("Rustのモジュール") {
        vec![Target::Rust]
    } else if name.starts_with("rust_") || name.starts_with("Rust") {
        vec![Target::Rust, Target::Wasm]
    } else {
        panic!("{path}: a file of tests/spellings starts with the target it is about")
    }
}

/// Whether a package of ritsu's breaks where the file's W001 says: every form but the Python
/// module named like one of the standard library's, which a package holds as
/// `<package>.dates.<alias>`.
fn breaks_in_a_package(path: &str) -> bool {
    !targets_of(path).contains(&Target::Python)
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
        let ts = codegen::build(&o, &u, Target::TypeScript, false).unwrap();
        for d in &u.dates {
            if !ts.iter().any(|(_, body)| body.contains(&format!("export function {}(", d.alias))) {
                failures.push(format!("{path}: {} is not in the TypeScript", d.alias));
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
        assert_eq!(common::SPELLING_FILES.iter().find(|(p, _)| p == e).unwrap().1, common::SPELLING_FILES.iter().find(|(p, _)| p == j).unwrap().1, "{e} and {j} stop the target alike");
    }
}

/// The names the Go code declares beside the dates' functions that no file of tests/spellings
/// takes: `ParseDate`, `KoyomiError`, and `IsOpen`, which it declares when the file has a calendar.
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
/// it: a calendar's too, since a calendar is a Python module as well. The four the generated code
/// imports itself are E009, and a name that only starts like one is not told.
#[test]
fn the_modules_of_pythons_standard_library() {
    let codes = |body: &str| -> Vec<&'static str> { check_text("tests/spellings/inline.cal", body, &Options::default()).diags.iter().map(|d| d.code).collect() };
    let dates = |alias: &str| format!("dates {alias} v1\n\ninputs\n  received : date  range >=2026-01-01 <=2026-01-31\n\ndate due = received\n  + 30 days\n");
    for alias in ["collections", "site", "random", "queue", "secrets"] {
        assert_eq!(codes(&dates(alias)), ["W001"], "{alias}");
    }
    for alias in ["datetime", "json", "sys", "typing"] {
        assert_eq!(codes(&dates(alias)), ["E009"], "{alias}");
    }
    for alias in ["collections_", "my_site", "secret"] {
        assert!(codes(&dates(alias)).is_empty(), "{alias}");
    }
    let o = check_text("tests/spellings/inline.cal", "calendar secrets v1\n\nclosed weekly sat, sun\n", &Options::default());
    let w: Vec<String> = o.diags.iter().map(|d| d.render(Lang::En)).collect();
    assert!(w.len() == 1 && w[0].contains("the file's alias `secrets` is the module's name, which a module of Python's standard library has"), "{w:?}");
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
