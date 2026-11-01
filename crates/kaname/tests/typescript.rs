//! The TypeScript kaname generates (DESIGN 10, 16.4), held to the reference interpreter. Every module
//! of the examples and of `tests/modules/` goes into one package, beside the TypeScript rulec,
//! koyomi and chobo write for the rules, the dates and the books they read (each language's own
//! generator, called as `ritsu gen` calls it), with the messages in English and in Japanese:
//!
//! - `tsc --strict --erasableSyntaxOnly` (TypeScript 7.0.2, `tools/`) passes the package, which Node
//!   could run by stripping its types;
//! - the package, compiled by tsc, runs every vector of every function in Node, the inputs outside
//!   what the parameters declare, and the values of types that hold themselves nested as deep as a
//!   host may give one and one deeper (DESIGN 28.1), and every answer is the reference interpreter's:
//!   the value, or the kind, the module, the line and the message of the failure (a case that does
//!   not stop in the interpreter, past its 200 calls, is not run);
//! - a date a host gives that is no day of the calendar (2026-02-30), or a string that is no date,
//!   is outside the range of every parameter, the ends written or not
//!   (`a_day_of_no_calendar_is_outside_every_range`).
//!
//! Without Node or the tools (`npm ci --prefix tools`), the test says SKIP and passes.

mod common;

use common::targets::*;
use kaname::r#gen::Target;
use ritsu_base::text::Lang;
use ritsu_testkit::{Need, TempDir};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const LIMIT: Duration = Duration::from_secs(900);

fn tsc() -> Option<PathBuf> {
    ritsu_testkit::tools::find("TSC", Some(Path::new("tools/node_modules/.bin/tsc")), "tsc", &["--version"])
}

/// The `tsconfig.json` the package in `dir` is compiled with: strict, with Node's types from `tools/`,
/// into `dist/`.
fn tsconfig(dir: &Path) {
    let types = tools().join("node_modules/@types");
    let tsconfig = format!(
        "{{\n  \"compilerOptions\": {{\n    \"strict\": true,\n    \"erasableSyntaxOnly\": true,\n    \"module\": \"commonjs\",\n    \"target\": \"es2022\",\n    \"rootDir\": \".\",\n    \"outDir\": \"dist\",\n    \"types\": [\"node\"],\n    \"typeRoots\": [{}],\n    \"skipLibCheck\": true\n  }},\n  \"include\": [\"**/*.ts\"],\n  \"exclude\": [\"dist\", \"node_modules\"]\n}}\n",
        ritsu_base::json::quote(&types.to_string_lossy())
    );
    std::fs::write(dir.join("tsconfig.json"), tsconfig).unwrap();
}

#[test]
fn the_generated_typescript_answers_as_the_reference_does() {
    if !ritsu_testkit::need(Need::Node) {
        return;
    }
    let Some(tsc) = tsc() else {
        ritsu_testkit::skip("tsc is not installed (npm ci --prefix tools); the TypeScript kaname generates is not checked");
        return;
    };
    if !ritsu_testkit::tools::runs("node", &["--version"]) {
        ritsu_testkit::skip("node is not on the PATH; the TypeScript kaname generates is not run");
        return;
    }
    let version = ritsu_testkit::tools::version(&tsc, "--version");
    let t = TempDir::new("kaname-typescript");
    let root = keep_in().unwrap_or_else(|| t.path().to_path_buf());
    let report = deep(move || {
        let modules = modules();
        let checked: Vec<_> = modules.iter().map(|(c, _)| c.clone()).collect();
        let mut out = Vec::new();
        for lang in [Lang::En, Lang::Ja] {
            let dir = root.join(format!("ts-{}", if lang == Lang::Ja { "ja" } else { "en" }));
            write_package(&dir, Target::TypeScript, lang, &modules);
            std::fs::write(dir.join("harness.ts"), kaname::r#gen::harness::program(&checked, Target::TypeScript, PY_NAME, GO_PACKAGE)).unwrap();
            let (cases, counts) = cases(&modules, lang, Target::TypeScript);
            std::fs::write(dir.join("cases.json"), cases_json(&cases)).unwrap();
            tsconfig(&dir);
            let r = ritsu_testkit::run(Command::new(&tsc).arg("-p").arg(dir.join("tsconfig.json")).current_dir(&dir), LIMIT);
            assert!(r.ok, "tsc --strict --erasableSyntaxOnly finds fault with the TypeScript kaname generates ({lang:?}):\n{}", r.both());
            let r = ritsu_testkit::run(Command::new("node").arg(dir.join("dist/harness.js")).arg(dir.join("cases.json")).current_dir(&dir), LIMIT);
            assert!(r.ok, "node failed ({lang:?}):\n{}", r.both());
            let (agree, wrong) = compare(&cases, &r.stdout);
            assert!(wrong.is_empty(), "the TypeScript ({lang:?}) answers otherwise than the reference:\n{}", wrong.join("\n"));
            out.push(counts.line(lang, agree, modules.len()));
        }
        out
    });
    println!("tsc --strict --erasableSyntaxOnly ({version}) passes the package in English and in Japanese");
    for line in report {
        println!("{line}");
    }
}

/// The program of [`a_day_of_no_calendar_is_outside_every_range`]: the functions called as a host
/// calls them, with dates a host can give, each answer a line.
const CALENDAR: &str = r#"// Dates of no calendar given to the entry.
import * as calc from "./glue/calc";
import * as open from "./glue/open";

function said(f: () => bigint): string {
  try {
    return String(f());
  } catch (e) {
    return (e as Error).message;
  }
}

for (const a of ["2026-02-30", "2026-0x-yy", "2026-03-02"]) {
  console.log(`calc ${a}: ${said(() => calc.days(a, "2026-03-01"))}`);
}
for (const a of ["2026-02-30", "10000-01-01", "2026-03-02"]) {
  console.log(`open ${a}: ${said(() => open.days(a, "2026-03-01"))}`);
}
"#;

/// A host of TypeScript gives a date as a string, which can be no day of the calendar or no date at
/// all: the entry finds it outside the range of the parameter, where the range writes both ends
/// (calc's `days`: the day number of 2026-02-30 is that of 2026-03-02, between the ends, and
/// `2026-0x-yy` falls between them as a string) and where it leaves them to the type (open's `days`).
/// Before, the first answered 2026-02-30 as 2026-03-02, and the second stopped inside the function
/// with an exception of JavaScript (DESIGN 22.5).
#[test]
fn a_day_of_no_calendar_is_outside_every_range() {
    if !ritsu_testkit::need(Need::Node) {
        return;
    }
    let Some(tsc) = tsc() else {
        ritsu_testkit::skip("tsc is not installed (npm ci --prefix tools); the TypeScript kaname generates is not checked");
        return;
    };
    if !ritsu_testkit::tools::runs("node", &["--version"]) {
        ritsu_testkit::skip("node is not on the PATH; the TypeScript kaname generates is not run");
        return;
    }
    let t = TempDir::new("kaname-typescript-calendar");
    let dir = t.path().to_path_buf();
    deep({
        let dir = dir.clone();
        move || {
            let modules: Vec<_> = modules().into_iter().filter(|(_, s)| s == "tests/modules/calc.kn" || s == "tests/modules/open.kn").collect();
            assert_eq!(modules.len(), 2);
            write_package(&dir, Target::TypeScript, Lang::En, &modules);
        }
    });
    std::fs::write(dir.join("calendar.ts"), CALENDAR).unwrap();
    tsconfig(&dir);
    let r = ritsu_testkit::run(Command::new(&tsc).arg("-p").arg(dir.join("tsconfig.json")).current_dir(&dir), LIMIT);
    assert!(r.ok, "{}", r.both());
    let r = ritsu_testkit::run(Command::new("node").arg(dir.join("dist/calendar.js")).current_dir(&dir), LIMIT);
    assert!(r.ok, "{}", r.both());
    assert_eq!(
        r.stdout,
        "calc 2026-02-30: The argument `a` of `days` is 2026-02-30, outside its range >=2026-01-01 <=2026-12-31\n\
         calc 2026-0x-yy: The argument `a` of `days` is 2026-0x-yy, outside its range >=2026-01-01 <=2026-12-31\n\
         calc 2026-03-02: -1\n\
         open 2026-02-30: The argument `first` of `days` is 2026-02-30, outside its range >=0001-01-01 <=9999-12-31\n\
         open 10000-01-01: The argument `first` of `days` is 10000-01-01, outside its range >=0001-01-01 <=9999-12-31\n\
         open 2026-03-02: -1\n"
    );
    print!("{}", r.stdout);
}

/// What is `private` (DESIGN 28.2) is no part of what a host calls: the TypeScript of a `private`
/// function, record or enum is not exported, Python starts its name with `_`, and Go with `_`, which
/// no other package reads.
#[test]
fn what_is_private_is_hidden_from_the_host() {
    let o = common::check_text("hof.kn", include_str!("modules/hof.kn"));
    let c = o.analysed.clone().unwrap();
    let text = |t: Target| {
        let made = kaname::r#gen::package(&[(c.clone(), "tests/modules/hof.kn".to_string())], t, Lang::En, "kanametest");
        made.files.into_iter().find(|f| f.2.is_some()).map(|f| f.1).unwrap()
    };
    let ts = text(Target::TypeScript);
    for (hidden, shown) in [("function _place_of(", "export function _place_of("), ("interface _Bounds {", "export interface _Bounds"), ("const _Side = {", "export const _Side")] {
        assert!(ts.contains(hidden) && !ts.contains(shown), "{hidden}");
    }
    assert!(ts.contains("export function sides("));
    let py = text(Target::Python);
    for hidden in ["def _place_of(", "class _Bounds:", "class _Side(enum.Enum):"] {
        assert!(py.contains(hidden), "{hidden}");
    }
    assert!(py.contains("def sides("));
    let go = text(Target::Go);
    for hidden in ["func _placeOf(", "type _Bounds struct", "type _Side string"] {
        assert!(go.contains(hidden), "{hidden}");
    }
    assert!(go.contains("func Sides("));
}
