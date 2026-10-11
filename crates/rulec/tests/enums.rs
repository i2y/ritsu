//! The enums of a rule beyond a list of names (§15.210–§15.212): an order a cell compares in, a
//! name each value is shown by, and a file of enums that several rules read, so that the code of
//! the rules takes one type where it took one type per rule.
//!
//! What the agreement across the languages holds over the corpus (tests/threeway.rs) is not
//! repeated here. These are the things it does not reach: an app that calls two rules with one
//! value, what `rulec gen` and `rulec api` say of a file of enums, the commands that have nothing
//! to say of one, the display names every language gives back, and a name that is a keyword in
//! some language.

use std::path::{Path, PathBuf};
use std::process::Command;
use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, ready, skip};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("cannot start rulec");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn run(args: &[&str]) -> (i32, String, String) {
    run_in(&root(), args)
}

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn json(text: &str) -> rulec::json::Json {
    rulec::json::parse(text.trim()).unwrap_or_else(|e| panic!("not JSON: {e}\n{text}"))
}

fn str_at<'a>(j: &'a rulec::json::Json, path: &[&str]) -> &'a str {
    let mut at = j;
    for k in path {
        at = at.get(k).unwrap_or_else(|| panic!("no `{k}` in {}", rulec::json::unparse(j)));
    }
    at.as_str().unwrap_or_else(|| panic!("`{}` is not a string", path.join(".")))
}

/// The two rules of the corpus that read the catalog's enums, in English and in Japanese: the
/// rules, the module the file of enums becomes, the module of each rule, and the display name the
/// app is to read back for `electronics`.
const SHARED: &[(&str, &str, &str, &str, &str, &str)] = &[
    ("tests/corpus/catalog_shipping.rule", "tests/corpus/catalog_returns.rule", "catalog", "catalog_shipping", "catalog_returns", "Electronics"),
    ("tests/corpus/品目の送料.rule", "tests/corpus/品目の返品.rule", "goods_catalog", "goods_shipping", "goods_returns", "電子機器"),
];

/// `mypy`, installed or through `uvx`, the way tests/threeway.rs finds it.
fn mypy() -> Option<Vec<String>> {
    if have("mypy") {
        return Some(vec!["mypy".into()]);
    }
    let ok = Command::new("uvx").args(["mypy", "--version"]).output().map(|o| o.status.success()).unwrap_or(false);
    ok.then(|| vec!["uvx".into(), "mypy".into()])
}

/// One app imports the code of two rules and the module of the enums they share, and hands the
/// same value to both. The rules declare no type of their own for the enum: each imports it, so
/// `mypy --strict` takes the one value for both parameters, which two types of the same name
/// would not let through.
#[test]
fn an_app_passes_one_value_of_a_shared_enum_to_two_rules() {
    if !ready(Need::Python, || have("python3"), "no python3, so the app was not run") {
        return;
    }
    for (shipping, returns, enums, ship_mod, ret_mod, label) in SHARED {
        let tmp = TempDir::new("enums-app");
        let out = tmp.path().to_string_lossy().into_owned();
        // The file of enums is not given: `gen` writes its module because the rules read it.
        let (c, _, e) = run(&["gen", shipping, returns, "--out", &out]);
        assert_eq!(c, 0, "{e}");
        let py = tmp.path().join("python");
        let module = std::fs::read_to_string(py.join(format!("{ship_mod}.py"))).unwrap();
        // `X as X`: the types are the module's to hand on, as its own would be.
        assert!(module.contains(&format!("from {enums} import Category as Category, Zone as Zone")), "{ship_mod}.py does not import the shared types:\n{module}");
        assert!(!module.contains("class Category("), "{ship_mod}.py declares a Category of its own");
        std::fs::write(
            py.join("app.py"),
            format!(
                "from {enums} import Category, Zone, category_label, zone_rank\n\
                 from {ship_mod} import Kilogram, {ship_mod}\n\
                 from {ret_mod} import {ret_mod}\n\n\
                 goods = Category.ELECTRONICS\n\
                 zone = Zone.NATIONAL\n\
                 fee = {ship_mod}(goods, zone, Kilogram(10))\n\
                 back = {ret_mod}(goods, zone, False)\n\
                 print(fee, back.window, back.free_return, category_label(goods), zone_rank(zone))\n"
            ),
        )
        .unwrap();
        let o = Command::new("python3").arg("app.py").current_dir(&py).output().expect("cannot start python3");
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        // Fragile goods to the rest of the country: 14USD and 7USD for the packing; fourteen days
        // to send them back unopened, free within the country.
        assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), format!("21 14 True {label} 2"));
        let Some(my) = mypy() else {
            skip("neither mypy nor uvx, so the types of the app were not checked");
            continue;
        };
        let o = Command::new(&my[0]).args(&my[1..]).args(["--strict", "--no-color-output", "app.py"]).current_dir(&py).output().expect("cannot start mypy");
        assert!(o.status.success(), "the app does not pass mypy --strict:\n{}", String::from_utf8_lossy(&o.stdout));
    }
}

/// `rulec gen` of a rule writes the module of the file of enums it reads, in every language that
/// names a type across files, without the file being given; NumPy, SQL and Wasm hold an enum by its
/// value and get none. Swift compiles one list of files, so the rule says which modules go into it.
#[test]
fn gen_writes_the_module_of_an_enums_file_beside_the_rule() {
    let tmp = TempDir::new("enums-gen");
    let out = tmp.path().to_string_lossy().into_owned();
    let (c, _, e) = run(&["gen", "tests/corpus/catalog_shipping.rule", "--out", &out]);
    assert_eq!(c, 0, "{e}");
    for f in [
        "python/catalog.py",
        "typescript/catalog.ts",
        "javascript/catalog.mjs",
        "rust/catalog.rs",
        "ruby/catalog.rb",
        "ruby/sig/catalog.rbs",
        "php/catalog.php",
        "go/catalog/catalog.go",
        "go/catalog/go.mod",
        "swift/catalog.swift",
        "java/Catalog.java",
    ] {
        assert!(tmp.path().join(f).is_file(), "{f} is not written");
    }
    for f in ["numpy/catalog.json", "sql/catalog.sql", "wasm/catalog_wasm.rs"] {
        assert!(!tmp.path().join(f).exists(), "{f} is written");
    }
    let owners = std::fs::read_to_string(tmp.path().join("vectors/catalog_shipping.owners.json")).unwrap();
    assert_eq!(owners.trim(), "[\"catalog\"]");
    // What every language is to give back as the display names, the file the runners read.
    let names = std::fs::read_to_string(tmp.path().join("vectors/catalog_shipping.labels.expected")).unwrap();
    assert!(names.lines().any(|l| l == "Rest of the country"), "{names}");
}

/// `rulec api` of a file of enums names the module each language gets and the enums in it, with
/// the order and the display names; `rulec api` of a rule that reads one says, of each enum, the
/// file it comes from and the module its type is imported from.
#[test]
fn api_says_where_a_shared_enum_comes_from() {
    let (c, out, e) = run(&["api", "tests/corpus/shared/catalog.rule"]);
    assert_eq!(c, 0, "{e}");
    let j = json(&out);
    assert_eq!(str_at(&j, &["enums"]), "catalog");
    assert_eq!(str_at(&j, &["python", "module"]), "catalog");
    assert_eq!(str_at(&j, &["go", "package"]), "catalog");
    assert!(j.get("rule").is_none(), "an enums file has no function to name: {out}");
    let zone = match j.get("python").and_then(|p| p.get("enums")).and_then(|e| e.as_arr()) {
        Some(es) => es.iter().find(|e| e.get("name").and_then(|n| n.as_str()) == Some("zone")).cloned().expect("no zone"),
        None => panic!("no enums: {out}"),
    };
    assert_eq!(zone.get("ordered").and_then(|b| b.as_bool()), Some(true));
    assert_eq!(str_at(&zone, &["rank"]), "zone_rank");
    assert_eq!(str_at(&zone, &["label"]), "zone_label");

    let (c, out, e) = run(&["api", "tests/corpus/catalog_shipping.rule"]);
    assert_eq!(c, 0, "{e}");
    let j = json(&out);
    for lang in ["python", "typescript", "rust", "go", "java"] {
        let Some(es) = j.get(lang).and_then(|p| p.get("enums")).and_then(|e| e.as_arr()) else { panic!("no {lang} enums: {out}") };
        for en in es {
            assert_eq!(str_at(en, &["from"]), "shared/catalog.rule", "{lang}");
            assert_eq!(str_at(en, &["module"]), "catalog", "{lang}");
        }
    }
}

/// A file of enums decides nothing, so what is about a decision — a wire, an adapter, the answers
/// two versions give, a page to try a case on — is refused with the reason, and what it holds is
/// still checked, rendered, described and certified.
#[test]
fn an_enums_file_is_refused_where_a_decision_is_asked_for() {
    let f = "tests/corpus/shared/catalog.rule";
    for args in [vec!["schema", f], vec!["adapter", f], vec!["diff", f, f], vec!["doc", f, "--format", "html"], vec!["doc", f, "--audience", "customer"]] {
        let (c, _, e) = run(&args);
        assert_eq!(c, 2, "{args:?}: {e}");
        assert!(e.contains("is an enums file and decides nothing"), "{args:?}: {e}");
    }
    for args in [vec!["check", f], vec!["api", f], vec!["doc", f], vec!["certificate", f], vec!["fmt", "--check", f]] {
        let (c, _, e) = run(&args);
        assert_eq!(c, 0, "{args:?}: {e}");
    }
}

/// `-> <name>` reads an enum under the rule's own name for it; an enum the rule declares under the
/// name an import brings is two enums of one name (E034), and an enum the file does not hold is
/// E013, as a file that is not there is.
#[test]
fn an_imported_enum_is_renamed_and_its_name_is_held_apart() {
    let tmp = TempDir::new("enums-import");
    let d = tmp.path();
    std::fs::copy(root().join("tests/corpus/shared/catalog.rule"), d.join("catalog.rule")).unwrap();
    let rule = |name: &str, head: &str, ty: &str| {
        format!(
            "rule {name} v1\n\n{head}\n\ninputs\n  dest : {ty}\n\noutputs\n  near : bool\n\ntable near_or_far\npolicy unique\n| dest       | -> near |\n| <=regional | true    |\n| >regional  | false   |\n"
        )
    };
    std::fs::write(d.join("renamed.rule"), rule("renamed", "import enum zone from \"catalog.rule\" -> band", "band")).unwrap();
    let (c, out, e) = run_in(d, &["check", "renamed.rule"]);
    assert_eq!(c, 0, "{out}{e}");

    std::fs::write(d.join("twice.rule"), rule("twice", "import enum zone from \"catalog.rule\"\nenum zone = near | far", "zone")).unwrap();
    let (c, out, _) = run_in(d, &["check", "twice.rule"]);
    assert_eq!(c, 1);
    assert!(out.contains("error[E034]"), "{out}");

    std::fs::write(d.join("absent.rule"), rule("absent", "import enum region from \"catalog.rule\"", "region")).unwrap();
    let (c, out, _) = run_in(d, &["check", "absent.rule"]);
    assert_eq!(c, 1);
    assert!(out.contains("error[E013]"), "{out}");
}

/// A rule whose inputs are called `class`, `type` and `def`: each a keyword in some of the
/// languages. W121 says so, every language writes the name with `_` after it, `rulec api` names
/// it so, and the wire keeps the name declared.
const KEYWORDS: &str = "rule fare v1
description \"A fare by the class of the seat\"

enum seat = economy | business | first_class

inputs
  class : seat
  type : number  range >=0 <=10
  def : bool?

outputs
  fee : number  round down(1)

define big : bool = type > 5

table fees
policy unique
| class       | big   | def       | -> fee |
| economy     | true  | -         | 12     |
| economy     | false | -         | 10     |
| business    | -     | true      | 25     |
| business    | -     | not: true | 20     |
| first_class | -     | -         | 30     |

examples
| class   | type | def  | -> fee |
| economy | 1    | none | 10     |
";

/// The display names of the corpus's plans carry what a language's strings read specially (`$`,
/// `${…}`, `#{…}`, a backslash before `(` or `u`, `%`, `'`), and the rule above has names that
/// are keywords; `rulec test` asks every language there is a toolchain for to answer both, the
/// names through the runner's `--labels`, against the expected file `gen` writes.
#[test]
fn every_language_answers_the_display_names_and_the_keyword_names() {
    if !ready(Need::Python, || have("python3"), "no python3, so nothing was run") {
        return;
    }
    let tmp = TempDir::new("enums-langs");
    let d = tmp.path();
    std::fs::write(d.join("fare.rule"), KEYWORDS).unwrap();
    let (c, out, _) = run_in(d, &["check", "fare.rule"]);
    assert_eq!(c, 0, "{out}");
    for name in ["class", "type", "def"] {
        assert!(out.contains(&format!("warning[W121]: `{name}` is a keyword in a target language, so the generated code writes it as `{name}_`")), "{out}");
    }
    let (c, api, _) = run_in(d, &["api", "fare.rule"]);
    assert_eq!(c, 0);
    let j = json(&api);
    assert_eq!(str_at(&j, &["python", "signature"]), "def fare(class_: Seat, type_: int, def_: bool | None) -> int:");

    let generated = d.join("gen");
    let plans = [root().join("tests/corpus/plan_receipt.rule"), root().join("tests/corpus/プランの領収書.rule")];
    let mut args: Vec<String> = vec!["gen".into(), d.join("fare.rule").to_string_lossy().into_owned()];
    args.extend(plans.iter().map(|p| p.to_string_lossy().into_owned()));
    args.extend(["--out".into(), generated.to_string_lossy().into_owned()]);
    let (c, _, e) = run_in(d, &args.iter().map(String::as_str).collect::<Vec<_>>());
    assert_eq!(c, 0, "{e}");
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .env("TMPDIR", tmpdir_in(&generated))
        .args(["test", generated.to_str().unwrap()])
        .output()
        .expect("cannot start rulec test");
    let said = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{said}");
    assert!(said.contains("ok    plan_receipt (Python, labels) 4 display names"), "{said}");
    assert!(said.contains("ok    plan_fee (Python, labels) 4 display names"), "{said}");
    assert!(said.lines().any(|l| l.starts_with("ok    fare (Python)")), "{said}");
    for l in said.lines().filter(|l| l.starts_with("warning") || l.starts_with("skip")) {
        eprintln!("{l}");
    }
}
