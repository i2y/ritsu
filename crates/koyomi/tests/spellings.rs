//! The names a target spells as one identifier, or cannot take (W001, DESIGN 15). Every file of
//! `tests/spellings/` gives W001 once, naming the targets its name says, and what `check` prints
//! of it, in English and in Japanese, is its golden file in `tests/golden/spellings/`
//! (`KOYOMI_BLESS=1 cargo test` writes them again; read the diff). The same file with the alias
//! the note gives (`tests/spellings/renamed/`) gives nothing. That the named target stops, and
//! that the others take both files, is tests/targets.rs's.

mod common;

use koyomi::check::{Checked, Options, check, check_text, render};
use koyomi::codegen;
use koyomi::naming::{TARGETS, Target};
use ritsu_base::text::Lang;
use std::path::Path;

/// The targets a file of tests/spellings is about, by its name: Go, PostgreSQL, or Rust with the
/// Wasm module built from the same code (but for the module's own name, which only Rust declares).
fn targets_of(path: &str) -> Vec<Target> {
    let name = path.rsplit('/').next().unwrap();
    if name.starts_with("go_") || name.starts_with("Go") {
        vec![Target::Go]
    } else if name.starts_with("postgresql_") || name.starts_with("PostgreSQL") {
        vec![Target::Sql]
    } else if name.starts_with("rust_module") || name.starts_with("Rustのモジュール") {
        vec![Target::Rust]
    } else if name.starts_with("rust_") || name.starts_with("Rust") {
        vec![Target::Rust, Target::Wasm]
    } else {
        panic!("{path}: a file of tests/spellings starts with the target it is about")
    }
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

/// W001 is a warning: `gen` writes every target of a file it warns about, and renames nothing.
#[test]
fn every_target_is_written_with_the_names_as_they_are() {
    for (path, _) in common::SPELLING_FILES {
        let mut o = check(path).unwrap();
        let c = o.checked.take().unwrap();
        let u = codegen::unit_of(&c, Lang::En);
        for t in TARGETS {
            let files = codegen::files(&u, t);
            assert!(!files.is_empty(), "{path}: gen writes nothing for {}", t.key());
        }
        // The aliases are in the code as they are written: every target but Go has each.
        let ts = codegen::files(&u, Target::TypeScript);
        for d in &u.dates {
            assert!(ts.iter().any(|(_, body)| body.contains(&format!("export function {}(", d.alias))), "{path}: {} is not in the TypeScript", d.alias);
        }
    }
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
