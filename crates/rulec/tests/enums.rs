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
use ritsu_testkit::run::finish;

/// How long a run of rulec may take: `rulec test` talks to the servers of the generated code,
/// and waits for their answers without a limit of its own. Past it, rulec is stopped with every
/// process it started, and the test fails saying what it ran and what it printed until then.
const RULEC_LIMIT: std::time::Duration = std::time::Duration::from_secs(20 * 60);

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
        let (c, _, e) = run(&rulec::targets::up_to_0_40(&["gen", shipping, returns, "--out", &out]));
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

/// A program, run in `dir`, that is to print `want`.
fn prints(dir: &Path, program: &[&str], want: &str) {
    let o = Command::new(program[0]).args(&program[1..]).current_dir(dir).output().unwrap_or_else(|e| panic!("cannot start {}: {e}", program[0]));
    assert!(o.status.success(), "{} in {}:\n{}{}", program.join(" "), dir.display(), String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), want, "{} in {}", program.join(" "), dir.display());
}

/// A rule's module hands on, beside the type of each enum it reads from a file of enums, what the
/// enums file's module has for the enum (§15.319): the functions of its order and of its display
/// names, in the languages where they are functions and not methods of the type — TypeScript,
/// JavaScript, Python, Ruby with its signatures, PHP. An app written against the rule's module alone
/// calls them, and the type is the enums module's own. Go, Rust, Swift and Java have them as methods
/// of the one type the rule's module names.
#[test]
fn a_rules_module_hands_on_the_order_and_display_names_of_the_enums_it_reads() {
    let tmp = TempDir::new("enums-handed");
    let out = tmp.path().to_string_lossy().into_owned();
    let (c, _, e) = run(&["gen", "tests/corpus/catalog_shipping.rule", "--target", "python", "--target", "typescript", "--target", "javascript", "--target", "ruby", "--target", "php", "--out", &out]);
    assert_eq!(c, 0, "{e}");
    // Fragile goods to the rest of the country, 10 kg: 14USD, and 7USD for the packing.
    let want = "21 Electronics 2 Rest of the country";
    if ready(Need::Python, || have("python3"), "no python3, so the Python app was not run") {
        let py = tmp.path().join("python");
        std::fs::write(
            py.join("app.py"),
            "import catalog\n\
             from catalog_shipping import Category, Kilogram, Zone, catalog_shipping, category_label, zone_label, zone_rank\n\n\
             assert Category is catalog.Category and Zone is catalog.Zone\n\
             print(catalog_shipping(Category.ELECTRONICS, Zone.NATIONAL, Kilogram(10)), category_label(Category.ELECTRONICS), zone_rank(Zone.NATIONAL), zone_label(Zone.NATIONAL))\n",
        )
        .unwrap();
        prints(&py, &["python3", "app.py"], want);
        match mypy() {
            Some(my) => {
                let o = Command::new(&my[0]).args(&my[1..]).args(["--strict", "--no-color-output", "app.py"]).current_dir(&py).output().expect("cannot start mypy");
                assert!(o.status.success(), "the app does not pass mypy --strict:\n{}", String::from_utf8_lossy(&o.stdout));
            }
            None => skip("neither mypy nor uvx, so the types of the Python app were not checked"),
        }
    }
    if ready(Need::Node, || have("node"), "no node, so the TypeScript and JavaScript apps were not run") {
        let ts = tmp.path().join("typescript");
        std::fs::write(ts.join("package.json"), "{\"type\": \"module\"}\n").unwrap();
        std::fs::write(
            ts.join("app.ts"),
            "import * as catalog from \"./catalog.ts\";\n\
             import { Category, Zone, catalog_shipping, categoryLabel, parseCategory, zoneLabel, zoneRank, type Kilogram } from \"./catalog_shipping.ts\";\n\n\
             const goods = parseCategory(\"electronics\");\n\
             if (Category !== catalog.Category || goods !== catalog.Category.ELECTRONICS) throw new Error(\"two types of one enum\");\n\
             console.log(String(catalog_shipping(goods, Zone.NATIONAL, 10n as Kilogram)), categoryLabel(goods), zoneRank(Zone.NATIONAL), zoneLabel(Zone.NATIONAL));\n",
        )
        .unwrap();
        prints(&ts, &["node", "--no-warnings", "app.ts"], want);
        match ritsu_testkit::tools::find("TSC", Some(Path::new("tools/node_modules/.bin/tsc")), "tsc", &["--version"]) {
            Some(tsc) => {
                let mut c = Command::new(&tsc);
                // with the two flags a project that compiles the modules may set (§15.342)
                c.args(["--strict", "--noUnusedLocals", "--noUnusedParameters", "--noEmit", "--allowImportingTsExtensions", "--erasableSyntaxOnly", "--module", "nodenext", "--target", "es2022", "--types", "node"]);
                if let Ok(types) = std::fs::canonicalize(root().join("tools/node_modules/@types")) {
                    c.arg("--typeRoots").arg(types);
                }
                let o = c.arg("app.ts").current_dir(&ts).output().expect("cannot start tsc");
                assert!(
                    o.status.success(),
                    "the app does not pass tsc --strict --noUnusedLocals --noUnusedParameters:\n{}{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                );
            }
            None => skip("tsc is not here (npm ci --prefix tools puts it there); the TypeScript app is not held to tsc --strict"),
        }
        let js = tmp.path().join("javascript");
        std::fs::write(
            js.join("app.mjs"),
            "import * as catalog from \"./catalog.mjs\";\n\
             import { Category, Zone, catalog_shipping, categoryLabel, parseCategory, zoneLabel, zoneRank } from \"./catalog_shipping.mjs\";\n\n\
             const goods = parseCategory(\"electronics\");\n\
             if (Category !== catalog.Category || goods !== catalog.Category.ELECTRONICS) throw new Error(\"two types of one enum\");\n\
             console.log(String(catalog_shipping(goods, Zone.NATIONAL, 10n)), categoryLabel(goods), zoneRank(Zone.NATIONAL), zoneLabel(Zone.NATIONAL));\n",
        )
        .unwrap();
        prints(&js, &["node", "app.mjs"], want);
    }
    if ready(Need::Ruby, || have("ruby"), "no ruby, so the Ruby app was not run") {
        let rb = tmp.path().join("ruby");
        std::fs::write(
            rb.join("app.rb"),
            "require_relative \"catalog_shipping\"\n\n\
             raise \"two types of one enum\" unless CatalogShipping::Category.equal?(Catalog::Category)\n\
             goods = CatalogShipping::Category::ELECTRONICS\n\
             zone = CatalogShipping::Zone::NATIONAL\n\
             puts [CatalogShipping.catalog_shipping(goods, zone, 10), CatalogShipping.category_label(goods), CatalogShipping.zone_rank(zone), CatalogShipping.zone_label(zone)].join(\" \")\n",
        )
        .unwrap();
        prints(&rb, &["ruby", "app.rb"], want);
        if have("steep") && have("rbs") {
            std::fs::write(rb.join("Steepfile"), "target :app do\n  signature \"sig\"\n  check \"catalog.rb\", \"catalog_shipping.rb\"\nend\n").unwrap();
            let o = Command::new("steep").arg("check").current_dir(&rb).output().expect("cannot start steep");
            let said = String::from_utf8_lossy(&o.stdout).into_owned();
            assert!(said.contains("No type error detected"), "the Ruby does not pass steep against its signature:\n{said}");
        } else {
            skip("steep or rbs is not here; the Ruby is not held to its signature");
        }
    }
    if ready(Need::Php, || have("php"), "no php, so the PHP app was not run") {
        let php = tmp.path().join("php");
        std::fs::write(
            php.join("app.php"),
            "<?php\n\ndeclare(strict_types=1);\n\nrequire_once __DIR__ . '/catalog_shipping.php';\n\n\
             use CatalogShipping\\Category;\nuse CatalogShipping\\Zone;\n\n\
             if (Category::ELECTRONICS !== \\Catalog\\Category::ELECTRONICS) {\n    throw new \\RuntimeException('two types of one enum');\n}\n\
             echo implode(' ', [CatalogShipping\\catalog_shipping(Category::ELECTRONICS, Zone::NATIONAL, 10), CatalogShipping\\category_label(Category::ELECTRONICS), CatalogShipping\\zone_rank(Zone::NATIONAL), CatalogShipping\\zone_label(Zone::NATIONAL)]), \"\\n\";\n",
        )
        .unwrap();
        prints(&php, &["php", "app.php"], want);
        // The rule's name for the enum (`class_alias`) as a type PHP holds a value to when it runs:
        // a parameter and what is returned take the owner's value, `instanceof` says so both ways,
        // and a value of another enum is refused. No static analyser is run (§15.319).
        std::fs::write(
            php.join("types.php"),
            "<?php\n\ndeclare(strict_types=1);\n\nrequire_once __DIR__ . '/catalog_shipping.php';\n\n\
             use CatalogShipping\\Category;\nuse CatalogShipping\\Zone;\n\n\
             function goods(Category $c): Category\n{\n    return $c;\n}\n\n\
             $owners = \\Catalog\\Category::ELECTRONICS;\n\
             $said = [goods($owners)->name, var_export($owners instanceof Category, true), var_export(Category::FOOD instanceof \\Catalog\\Category, true)];\n\
             try {\n    goods(Zone::NATIONAL);\n    $said[] = 'let through';\n} catch (\\TypeError $e) {\n    $said[] = 'TypeError';\n}\n\
             echo implode(' ', $said), \"\\n\";\n",
        )
        .unwrap();
        prints(&php, &["php", "types.php"], "ELECTRONICS true true TypeError");
    }
}

/// `--python-package` (§15.319) writes the Python modules to be put inside a package: a rule's
/// module reads the module of the enums file by a relative import, and an app's package reads both
/// where it puts them (`<package>/rulec/python/`). The same rule's module without the flag reads it
/// from the top of the output directory, so `--check` holds a directory to the flag it was written
/// with. The flag wants `--target python`, and refuses the Connect service, which is read and
/// type-checked at the top of the output directory beside buf's modules (§15.327).
#[test]
fn python_package_writes_modules_a_package_reads() {
    let tmp = TempDir::new("enums-package");
    let shop = tmp.path().join("shop");
    let out = shop.join("rulec").to_string_lossy().into_owned();
    let write = |more: &[&str]| run(&[&["gen", "tests/corpus/catalog_shipping.rule", "--target", "python", "--out", &out][..], more].concat());
    let (c, _, e) = write(&["--python-package"]);
    assert_eq!(c, 0, "{e}");
    let module = std::fs::read_to_string(shop.join("rulec/python/catalog_shipping.py")).unwrap();
    assert!(module.contains("\nfrom .catalog import Category as Category, Zone as Zone, "), "{module}");
    assert_eq!(write(&["--python-package", "--check"]).0, 0, "--check with the flag the directory was written with");
    assert_eq!(write(&["--check"]).0, 1, "--check without it");
    if ready(Need::Python, || have("python3"), "no python3, so the app over the package was not run") {
        std::fs::write(shop.join("__init__.py"), "").unwrap();
        std::fs::write(
            tmp.path().join("app.py"),
            "from shop.rulec.python import catalog\n\
             from shop.rulec.python.catalog_shipping import Category, Kilogram, Zone, catalog_shipping, category_label\n\n\
             assert Category is catalog.Category\n\
             print(catalog_shipping(Category.ELECTRONICS, Zone.NATIONAL, Kilogram(10)), category_label(Category.ELECTRONICS))\n",
        )
        .unwrap();
        prints(tmp.path(), &["python3", "app.py"], "21 Electronics");
        match mypy() {
            Some(my) => {
                let o = Command::new(&my[0]).args(&my[1..]).args(["--strict", "--no-color-output", "app.py"]).current_dir(tmp.path()).output().expect("cannot start mypy");
                assert!(o.status.success(), "the app does not pass mypy --strict:\n{}", String::from_utf8_lossy(&o.stdout));
            }
            None => skip("neither mypy nor uvx, so the types of the app were not checked"),
        }
    }
    let flag = "`--python-package` says how the Python modules are put, and there is no `--target python`";
    for (args, said) in [
        (&["gen", "tests/corpus/catalog_shipping.rule", "--target", "go", "--python-package"][..], flag.to_string()),
        (&["gen", "tests/corpus/catalog_shipping.rule", "--python-package"][..], flag.to_string()),
        (
            &["gen", "tests/corpus/catalog_shipping.rule", "--target", "python", "--target", "proto", "--extra", "connect", "--extra", "runner", "--python-package"][..],
            "`--extra connect` writes a service that is read at the top of the output directory beside the protobuf modules buf writes there, and type-checked there (`mypy --strict`); a module put inside a package (`--python-package`) reads its neighbours by relative imports, which the type checker cannot follow from there".to_string(),
        ),
    ] {
        let at = tmp.path().join("refused").to_string_lossy().into_owned();
        let (c, o, e) = run(&[args, &["--out", &at]].concat());
        assert_eq!((c, o.as_str(), e.trim_end()), (2, "", format!("error: {said}").as_str()), "{args:?}");
        assert!(!tmp.path().join("refused").exists(), "{args:?} wrote files");
    }
}

/// The runner and the MCP server beside modules written to go inside a package (`--python-package`,
/// §15.327) read them as the modules of a package their directory makes, so `rulec test` holds the
/// modules a package puts in place to the reference evaluator as it holds any other: two rules that
/// read an `enums` file, in English and in Japanese, through the runner, its display names and the
/// MCP server over stdio and HTTP. A rule that reads no `enums` file is written the same with the
/// flag and without it, its runner and its MCP server too.
#[test]
fn rulec_test_runs_the_modules_written_to_go_inside_a_package() {
    let tmp = TempDir::new("enums-package-runner");
    let write = |rules: &[&str], at: &Path, more: &[&str]| {
        let out = at.to_string_lossy().into_owned();
        let (c, _, e) = run(&[&["gen"][..], rules, &["--target", "python", "--extra", "runner", "--extra", "mcp", "--extra", "vectors", "--out", &out], more].concat());
        assert_eq!(c, 0, "{e}");
    };
    let generated = tmp.path().join("gen");
    write(&["tests/corpus/catalog_shipping.rule", "tests/corpus/品目の送料.rule"], &generated, &["--python-package"]);
    for alias in ["catalog_shipping", "goods_shipping"] {
        for file in [format!("{alias}_runner.py"), format!("{alias}_mcp.py")] {
            let code = std::fs::read_to_string(generated.join("python").join(&file)).unwrap();
            assert!(code.contains(&format!("\nm = importlib.import_module(\"_rulec.{alias}\")\n")), "{file} does not read the module from the package:\n{code}");
            assert!(!code.contains(&format!("\nimport {alias} as m\n")), "{file} reads the module by its name");
        }
    }
    let (plain, flagged) = (tmp.path().join("plain"), tmp.path().join("flagged"));
    write(&["tests/corpus/期間区分.rule"], &plain, &[]);
    write(&["tests/corpus/期間区分.rule"], &flagged, &["--python-package"]);
    let python = std::fs::read_dir(plain.join("python")).unwrap().count();
    assert_eq!(std::fs::read_dir(flagged.join("python")).unwrap().count(), python);
    for f in std::fs::read_dir(plain.join("python")).unwrap().flatten() {
        let name = f.file_name();
        assert_eq!(std::fs::read(f.path()).unwrap(), std::fs::read(flagged.join("python").join(&name)).unwrap(), "{name:?} differs with the flag");
    }
    if !ready(Need::Python, || have("python3"), "no python3, so the runner and the MCP server were not run") {
        return;
    }
    let o = finish(Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmpdir_in(&generated)).args(["test", generated.to_str().unwrap()]), RULEC_LIMIT, "rulec test running the generated code");
    let said = o.stdout.clone() + &o.stderr;
    assert!(o.ok, "{said}");
    for alias in ["catalog_shipping", "goods_shipping"] {
        for via in ["(Python)", "(Python, labels)", "(Python, MCP)", "(Python, MCP/HTTP)"] {
            assert!(said.lines().any(|l| l.starts_with(&format!("ok    {alias} {via} "))), "{alias} {via} did not run:\n{said}");
        }
    }
}

/// `rulec gen` of a rule writes the module of the file of enums it reads, in every language that
/// names a type across files, without the file being given; NumPy, SQL and Wasm hold an enum by its
/// value and get none. Swift compiles one list of files, so the rule says which modules go into it.
#[test]
fn gen_writes_the_module_of_an_enums_file_beside_the_rule() {
    let tmp = TempDir::new("enums-gen");
    let out = tmp.path().to_string_lossy().into_owned();
    let (c, _, e) = run(&rulec::targets::up_to_0_40(&["gen", "tests/corpus/catalog_shipping.rule", "--out", &out]));
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
    args.extend(rulec::targets::UP_TO_0_40.iter().map(|s| s.to_string()));
    let (c, _, e) = run_in(d, &args.iter().map(String::as_str).collect::<Vec<_>>());
    assert_eq!(c, 0, "{e}");
    let o = finish(Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmpdir_in(&generated)).args(["test", generated.to_str().unwrap()]), RULEC_LIMIT, "rulec test running the generated code");
    let said = o.stdout.clone() + &o.stderr;
    assert!(o.ok, "{said}");
    assert!(said.contains("ok    plan_receipt (Python, labels) 4 display names"), "{said}");
    assert!(said.contains("ok    plan_fee (Python, labels) 4 display names"), "{said}");
    assert!(said.lines().any(|l| l.starts_with("ok    fare (Python)")), "{said}");
    for l in said.lines().filter(|l| l.starts_with("warning") || l.starts_with("skip")) {
        eprintln!("{l}");
    }
}

/// An enum of one value, with a display name and without, the rule's own and one an `enums` file
/// declares: every language's code reads it, and answers as the reference does. A function over
/// the values of such an enum has one answer and no choice to make; Ruby read no `case` with only
/// an `else` (`expected a when or in clause after case`).
#[test]
fn an_enum_of_one_value_is_written_so_that_every_language_reads_it() {
    if !ready(Need::Python, || have("python3"), "no python3, so nothing was run") {
        return;
    }
    let tmp = TempDir::new("enums-one-value");
    let generated = tmp.path().join("gen");
    let dir = root().join("tests/one_value");
    let mut args: Vec<String> = vec!["gen".into()];
    for f in ["one_way.rule", "一つの道.rule"] {
        args.push(dir.join(f).to_string_lossy().into_owned());
    }
    args.extend(["--out".into(), generated.to_string_lossy().into_owned()]);
    args.extend(rulec::targets::UP_TO_0_40.iter().map(|s| s.to_string()));
    let (c, _, e) = run(&args.iter().map(String::as_str).collect::<Vec<_>>());
    assert_eq!(c, 0, "{e}");
    let o = finish(Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmpdir_in(&generated)).args(["test", generated.to_str().unwrap()]), RULEC_LIMIT, "rulec test running the generated code");
    let said = o.stdout.clone() + &o.stderr;
    assert!(o.ok, "{said}");
    for alias in ["one_way", "one_way_ja"] {
        assert!(said.lines().any(|l| l.starts_with(&format!("ok    {alias} (Python)"))), "{said}");
        assert!(said.contains(&format!("ok    {alias} (Python, labels) 2 display names")), "{said}");
        if ready(Need::Ruby, || have("ruby"), "ruby is not installed") {
            assert!(said.lines().any(|l| l.starts_with(&format!("ok    {alias} (Ruby)"))), "{said}");
            assert!(said.contains(&format!("ok    {alias} (Ruby, labels) 2 display names")), "{said}");
        }
    }
    for l in said.lines().filter(|l| l.starts_with("warning") || l.starts_with("skip")) {
        eprintln!("{l}");
    }
}

/// A discount by tier, for the input `member` whose `range` lets in silver and gold, and for an
/// input `referrer` of the same enum with none, when `with_referrer` puts it in.
fn ranged_tiers(rows: &str, with_referrer: bool) -> String {
    let (input, column, dash) = if with_referrer { ("\n  referrer : tier", " referrer |", " -        |") } else { ("", "", "") };
    let rows: String = rows.lines().map(|l| format!("| {l} |{dash} {} |\n", if l.contains("silver") { "5%         " } else { "10%        " })).collect();
    format!(
        "rule member_discount v1\ndescription \"A discount by the member's tier\"\n\nenum tier = basic < silver < gold < platinum\n\n\
         inputs\n  member : tier  range >=silver <=gold{input}\n\noutputs\n  discount : rate  round down(1%)\n\n\
         table discount_of\npolicy unique\n| member |{column} -> discount  |\n{rows}"
    )
}

/// A value of an ordered enum that the `range` of every input of the enum keeps out never arrives,
/// so no row has to name it (§15.210, §15.233): W111 leaves it out — but not while another name of
/// the type can bring it. A row whose cell names only values the range keeps out matches nothing
/// (E102); a set that names one beside values the range lets in is the set of those, as a set of
/// numbers past a range is.
#[test]
fn a_value_every_range_keeps_out_is_not_said_to_be_unused() {
    let tmp = TempDir::new("enums-ranged-w111");
    let d = tmp.path();
    let cases: [(&str, String, i32, &[&str], &[&str]); 4] = [
        ("kept_out", ranged_tiers("silver\n>=gold ", false), 0, &[], &["W111"]),
        ("brought_by_referrer", ranged_tiers("silver\n>=gold ", true), 0, &["warning[W111]: Values of type tier appear in no row", "Values that never appear: basic"], &[]),
        ("named_alone", ranged_tiers("basic \nsilver\n>=gold ", false), 1, &["error[E102]: Unreachable row: row 1 never matches"], &["W111"]),
        ("named_in_a_set", ranged_tiers("basic, silver\n>=gold ", false), 0, &[], &["W111", "E102"]),
    ];
    for (name, src, code, said, unsaid) in cases {
        let file = format!("{name}.rule");
        std::fs::write(d.join(&file), &src).unwrap();
        let (c, out, err) = run_in(d, &["check", &file]);
        let all = format!("{out}{err}");
        assert_eq!(c, code, "{name}:\n{src}\n{all}");
        for s in said {
            assert!(all.contains(s), "{name}: `{s}` is not said:\n{all}");
        }
        for s in unsaid {
            assert!(!all.contains(s), "{name}: `{s}` is said:\n{all}");
        }
    }
}

/// A rule named like an enum it reads from an `enums` file (§15.239): `rule size` and `enum size`
/// make `func Size` and the type `Size` in Go, which one package cannot hold both of. The rule's
/// package then names the type by the package that owns it, `sizes.Size`, as Java names it by its
/// class, and declares no alias of it; the constants and the parser are there as for any other
/// enum it reads. Before, the package declared both and did not compile, and `check` said nothing.
/// For a rule and an enum it declares itself under one name, the code written for them puts a number
/// on the enum's type in Go and Java (§15.326, `tests/same_names.rs`), and W121 says the type each
/// writes and the two names that met, keeping no target from being written (§15.337).
#[test]
fn a_rule_named_like_the_enum_it_reads_compiles_in_go() {
    let t = TempDir::new("enums-rule-named-like-its-enum");
    let d = t.path();
    std::fs::create_dir_all(d.join("shared")).unwrap();
    std::fs::write(d.join("shared/sizes.rule"), "enums sizes v1\ndescription \"Parcel sizes\"\n\nenum size = small | large\n").unwrap();
    let rule = "rule size v1\ndescription \"The fee of a parcel by its size; the rule is named like the enum it reads\"\n\n\
                import enum size from \"shared/sizes.rule\"\n\ninputs\n  parcel : size\n\noutputs\n  fee : number  round down(1)\n\n\
                table fee_of\npolicy unique\n| parcel | -> fee : number |\n| small  | 5               |\n| large  | 9               |\n\n\
                examples\n| parcel | -> fee |\n| small  | 5      |\n";
    std::fs::write(d.join("size.rule"), rule).unwrap();
    let (c, out, e) = run_in(d, &["check", "size.rule"]);
    assert_eq!((c, out.as_str()), (0, "ok size.rule\n"), "{e}");
    let (c, out, e) = run_in(d, &rulec::targets::up_to_0_40(&["gen", "size.rule", "--out", "gen"]));
    assert_eq!(c, 0, "{out}{e}");
    let go = std::fs::read_to_string(d.join("gen/go/size/size.go")).unwrap();
    assert!(!go.contains("type Size = sizes.Size"), "{go}");
    assert!(go.contains("func Size(in Input) (int64, error)"), "{go}");
    assert!(go.contains("\tParcel sizes.Size"), "{go}");
    assert!(go.contains("func ParseSize(s string) (sizes.Size, bool) { return sizes.ParseSize(s) }"), "{go}");
    let (_, api, _) = run_in(d, &["api", "size.rule"]);
    let j = json(&api);
    let field = j.get("go").and_then(|g| g.get("input_fields")).and_then(|x| x.as_arr()).and_then(|a| a.first().cloned()).unwrap();
    assert_eq!(field.get("type").and_then(|x| x.as_str()), Some("sizes.Size"), "{api}");
    if ready(Need::Go, || Command::new("go").arg("version").output().is_ok_and(|o| o.status.success()), "go is not installed") {
        let o = finish(Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmpdir_in(&d.join("gen"))).current_dir(d).args(["test", "gen"]), RULEC_LIMIT, "rulec test running the generated code");
        let said = o.stdout.clone() + &o.stderr;
        assert!(o.ok, "{said}");
        assert!(said.lines().any(|l| l.starts_with("ok    size (Go)")), "{said}");
    }
    // The rule's own enum of the same name: W121, saying the type Go and Java write, and no target.
    let own = rule.replace("import enum size from \"shared/sizes.rule\"", "enum size = small | large");
    std::fs::write(d.join("own.rule"), &own).unwrap();
    let (c, out, _) = run_in(d, &["check", "own.rule"]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains("warning[W121]: The type of enum `size` is written `Size2` in the code generated for Go and Java"), "{out}");
    assert!(out.contains(" Go: the rule's package declares `Size` for the rule, and one of the names made of the enum is `Size` as well; a package holds one thing of a name, so the enum's type is `Size2` there. The names made of it follow (`Size2Small`, `Size2Large`, `ParseSize2`)."), "{out}");
    assert!(out.contains(" Java: the enum is nested in the rule's class `Size`, which holds no type of its own name, so the enum's type is `Size2` there."), "{out}");
    let path = d.join("own.rule").to_string_lossy().into_owned();
    let ds = rulec::check_source(&own, &path);
    assert!(ds.iter().all(|d| d.code != "W121" || d.targets.is_empty()), "{:?}", ds.iter().map(|d| (d.code, d.targets.clone())).collect::<Vec<_>>());
    let (c, out, e) = run_in(d, &rulec::targets::up_to_0_40(&["gen", "own.rule", "--out", "gen-own"]));
    assert_eq!(c, 0, "{out}{e}");
}

/// A contribution stands for the file it comes down to (§15.231), and one that adds to an `enums`
/// file is refused where a decision is asked for, as the file is (§15.240): `doc` as HTML and for
/// customers, `schema`. Before, the HTML page of the enums file came out through the contribution,
/// with a panel to try a case on a file that decides nothing. Its Markdown page still does.
#[test]
fn an_enums_file_reached_through_a_contribution_is_refused_as_the_file_is() {
    let t = TempDir::new("enums-through-contribution");
    let d = t.path();
    std::fs::create_dir_all(d.join("rules")).unwrap();
    std::fs::create_dir_all(d.join("mods/m")).unwrap();
    std::fs::write(d.join("rules/content.rule"), "enums content v1\ndescription \"Events\"\n\nenum event = harvest \"Harvest\" | quiet \"Quiet\"  open\n").unwrap();
    std::fs::write(d.join("mods/m/content.rule"), "contribution m_content v1\ndescription \"A storm\"\nto \"../../rules/content.rule\"\n\nextend event with storm \"Storm\"\n").unwrap();
    let (c, _, e) = run_in(d, &["source", "pin", "mods/m/content.rule"]);
    assert_eq!(c, 0, "{e}");
    for args in [vec!["doc", "mods/m/content.rule", "--format", "html", "--out", "pages"], vec!["doc", "mods/m/content.rule", "--audience", "customer"], vec!["schema", "mods/m/content.rule"]] {
        let (c, _, e) = run_in(d, &args);
        assert_eq!(c, 2, "{args:?}: {e}");
        assert!(e.contains("error: `mods/m/content.rule` adds to `rules/content.rule`, an enums file that decides nothing"), "{args:?}: {e}");
    }
    assert!(!d.join("pages").exists(), "a page was written");
    let (c, out, e) = run_in(d, &["doc", "mods/m/content.rule"]);
    assert_eq!(c, 0, "{e}");
    assert!(out.contains("storm"), "{out}");
}
