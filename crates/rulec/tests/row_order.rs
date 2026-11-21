//! The type a checker reads off the generated code does not turn on the order of a table's rows
//! (§15.329).
//!
//! Python binds a table's column to a local in each row, and mypy takes the local's type from its
//! first binding. A column of money whose first row handed on an input (`price`, a `JPY`) and whose
//! second answered `0JPY` (an `int`) failed `mypy --strict` with "Incompatible types in assignment
//! (expression has type "int", variable has type "JPY")", while the same rule with the two rows the
//! other way round passed: `JPY` is an `int`, and an `int` is not a `JPY`. A quantity and a rate
//! failed the same way, and so did a table inside a walk handing on a field of an element. Now a
//! column of money, a quantity or a rate whose first row hands on a name as it stands is annotated
//! with the whole number it holds before its rows, so mypy reads every such column as an `int`
//! whatever its rows. TypeScript declares every column with `let` and the type of the value it
//! holds before the rows, so its reading never turned on the order.
//!
//! The materials are in `tests/row_order`, English first, with a Japanese version of each beside it:
//! a column of money whose first row hands on the input and one whose first row answers the whole
//! number, and a mass, a rate and a column of money inside a walk, each handing on the input first.

use ritsu_testkit::run::finish;
use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, need, skip};
use std::path::{Path, PathBuf};
use std::process::Command;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/row_order")
}

/// How long a run of rulec may take.
const RULEC_LIMIT: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// Every rule here written into `out` with `flags`.
fn generate(out: &Path, flags: &[&str]) {
    let mut names: Vec<String> = std::fs::read_dir(here())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".rule"))
        .collect();
    names.sort();
    let mut args: Vec<&str> = vec!["gen"];
    args.extend(names.iter().map(String::as_str));
    args.extend(["--out", out.to_str().unwrap()]);
    args.extend(flags);
    let o = finish(
        Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmpdir_in(out)).current_dir(here()).args(&args),
        RULEC_LIMIT,
        "rulec gen",
    );
    assert_eq!(o.code, Some(0), "{}{}", o.stdout, o.stderr);
}

fn read(out: &Path, rel: &str) -> String {
    std::fs::read_to_string(out.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// Each rule's Python, with the columns annotated before their rows and the ones that are not:
/// (the file, the annotations it writes, the ones it does not).
const ANNOTATED: &[(&str, &[&str], &[&str])] = &[
    ("python/price_first.py", &["    counted: int\n    if price_given:  # row 1: true | price\n"], &[]),
    ("python/price_first_ja.py", &["    counted: int\n    if price_given:  # row 1: true | 価格\n"], &[]),
    // The whole number first: mypy reads `int`, which the input's `JPY` is, and the code is
    // written as it was.
    ("python/zero_first.py", &[], &["counted: int"]),
    ("python/zero_first_ja.py", &[], &["counted: int"]),
    ("python/kinds_first.py", &["    billed_weight: int\n    applied: int\n", "        charged: int\n"], &[]),
    ("python/kinds_first_ja.py", &["    billed_weight: int\n    applied: int\n", "        charged: int\n"], &[]),
];

/// A column of money, a quantity or a rate whose first row hands on an input or a field of an
/// element as it stands is annotated with `int` before its rows; a column whose first row answers
/// a whole number is written as it was.
#[test]
fn a_column_whose_first_row_hands_on_a_branded_value_is_annotated() {
    let t = TempDir::new("row-order");
    let out = t.path().join("out");
    generate(&out, &["--target", "python"]);
    for (file, says, not) in ANNOTATED {
        let body = read(&out, file);
        for s in *says {
            assert!(body.contains(s), "{file} does not say {s:?}:\n{body}");
        }
        for s in *not {
            assert!(!body.contains(s), "{file} says {s:?}:\n{body}");
        }
    }
}

/// `mypy`, installed or through `uvx`.
fn mypy() -> Option<Vec<String>> {
    let have = |cmd: &str, args: &[&str]| Command::new(cmd).args(args).output().map(|o| o.status.success()).unwrap_or(false);
    if have("mypy", &["--version"]) {
        return Some(vec!["mypy".into()]);
    }
    have("uvx", &["mypy", "--version"]).then(|| vec!["uvx".into(), "mypy".into()])
}

/// The Python of every rule here passes `mypy --strict`, and its TypeScript `tsc --strict`, whichever
/// row of a column comes first.
#[test]
fn the_generated_code_type_checks_whichever_row_comes_first() {
    if !need(Need::Python) {
        return;
    }
    let t = TempDir::new("row-order-types");
    let out = t.path().join("out");
    generate(&out, &["--target", "python", "--target", "typescript"]);
    let modules = ["price_first", "price_first_ja", "zero_first", "zero_first_ja", "kinds_first", "kinds_first_ja"];
    match mypy() {
        None => skip("mypy is not here, nor uvx; the Python is not held to mypy --strict"),
        Some(my) => {
            let py = out.join("python");
            let files: Vec<String> = modules.iter().map(|m| format!("{m}.py")).collect();
            let o = Command::new(&my[0])
                .args(&my[1..])
                .args(["--strict", "--no-color-output", "--cache-dir"])
                .arg(t.path().join("mypy-cache"))
                .args(&files)
                .current_dir(&py)
                .output()
                .expect("cannot start mypy");
            assert!(o.status.success(), "the Python does not pass mypy --strict:\n{}", String::from_utf8_lossy(&o.stdout));
        }
    }
    let tsc = ritsu_testkit::tools::find("TSC", Some(Path::new("tools/node_modules/.bin/tsc")), "tsc", &["--version"]);
    match tsc {
        None => skip("tsc is not here (npm ci --prefix tools puts it there); the TypeScript is not held to tsc --strict"),
        Some(tsc) if need(Need::Node) => {
            let ts = out.join("typescript");
            let files: Vec<PathBuf> = modules.iter().map(|m| ts.join(format!("{m}.ts"))).collect();
            // with the two flags a project that compiles the module may set (§15.342)
            let o = Command::new(&tsc)
                .args(["--strict", "--noUnusedLocals", "--noUnusedParameters", "--noEmit", "--allowImportingTsExtensions", "--target", "es2022"])
                .args(&files)
                .current_dir(&ts)
                .output()
                .expect("cannot start tsc");
            assert!(
                o.status.success(),
                "the TypeScript does not pass tsc --strict --noUnusedLocals --noUnusedParameters:\n{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
        }
        Some(_) => {}
    }
}
