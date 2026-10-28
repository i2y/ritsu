//! `ritsu gen` (DESIGN 9.3; PLAN E.7): a project's rules, dates, clients of the books and workflows
//! as one package for each of TypeScript, Python and Go.
//!
//! - The packages pass the target's own type checker: `tsc --strict`, `mypy --strict`, `go vet` (and
//!   gofmt), with the clients of the books on PostgreSQL and on TigerBeetle.
//! - A workflow reads the rules, the dates and the books from its package: its imports name the
//!   package's rules/, dates/ and books/, the books its transport takes are the package's clients,
//!   and the rule's and the date's activities, run, answer through the package's modules.
//! - `--check` says what is stale, missing or left over, and writing again puts it right.
//! - Every file begins with the head of DESIGN 9.2, naming the project's file it is made from.
//! - An amount of money or of a quantity is the package's one type of its unit (`units`), in the
//!   rules' modules and the books' clients alike: code written by hand that gives a book what a rule
//!   returns, with no workflow between them, is held to the book's unit by the type checker, and
//!   runs (`tests/projects/marketplace`, its checkout in `service/`).
//!
//! The project is `tests/projects/stockroom` (English): a rule of rulec's, a dates file and a
//! calendar of koyomi's, a book of chobo's, and a flow of dandori's that uses all three. The
//! Japanese one, `tests/projects/通販`, is generated too, where it can be: its two flows have
//! Japanese names, so the Go of both would be the package `workflow`, which ritsu gen refuses.
//! sekisho's example (`crates/sekisho/examples/refunds`) is the project with gates: their Cedar
//! goes once into `<out>/cedar/`, and the code of their requests into each package's `authz/`.

mod type_checkers;

use ritsu_testkit::{Need, TempDir, golden, ready};
use std::path::{Path, PathBuf};
use std::process::Command;
use type_checkers::*;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const STOCKROOM: &str = "tests/projects/stockroom";
const SHOP: &str = "tests/projects/通販";
/// sekisho's example, a project with two gates (English and Japanese) beside the rules, the dates,
/// the calendar and the flow they read.
const REFUNDS: &str = "../sekisho/examples/refunds";
/// The module the Go is vetted in, and the import path of the package's directory in it.
const MODULE: &str = "gocheck/generated";

/// `ritsu`, run in `dir`, with no language asked of the environment.
fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).args(args);
    for v in ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG", "SEKISHO_LANG"] {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// `ritsu gen` of the project at `project` (from this crate), its root the project's directory,
/// writing under `out`.
fn generate(project: &Path, out: &Path, more: &[&str]) -> (i32, String, String) {
    let o = out.to_str().unwrap();
    let mut args = vec!["gen", "--root", ".", "--out", o, "--module", MODULE];
    args.extend_from_slice(more);
    ritsu_in(project, &args)
}

/// Every file under `dir`, from it, in order.
fn files(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, d: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.push(p.strip_prefix(base).unwrap().to_string_lossy().to_string());
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

fn sha256(bytes: &[u8]) -> String {
    ritsu_base::sha256::hex(bytes)
}

// ── the type checkers ────────────────────────────────────────────────────────────────────────────

/// The packages of the English project, with the books on PostgreSQL, on TigerBeetle and on the MySQL
/// family, and the TypeScript and the Python of the Japanese one, pass `tsc --strict`, `mypy --strict`
/// and `go vet`.
#[test]
fn the_packages_pass_the_type_checkers() {
    let t = TempDir::new("gen-types");
    let mut made = Vec::new();
    for books in ["postgres", "tigerbeetle", "mysql"] {
        let out = t.path().join(format!("stockroom-{books}"));
        let (code, _, err) = generate(&here().join(STOCKROOM), &out, &["--books", books]);
        assert_eq!(code, 0, "ritsu gen of the stockroom ({books}):\n{err}");
        made.push(out);
    }
    let shop = t.path().join("shop");
    let (code, _, err) = generate(&here().join(SHOP), &shop, &["--target", "typescript"]);
    assert_eq!(code, 0, "ritsu gen --target typescript of 通販:\n{err}");
    if ritsu_testkit::need(Need::Node) {
        match tsc() {
            Some((tsc, modules)) => {
                for out in made.iter().chain([&shop]) {
                    type_check_typescript(&out.join("typescript"), &tsc, &modules);
                }
                println!("tsc --strict passes the TypeScript of four packages");
            }
            None => ritsu_testkit::skip("TypeScript (crates/dandori/tools/temporal) or tigerbeetle-node (crates/chobo/tools/runner) is not installed; the TypeScript of the packages is not type-checked"),
        }
    }
    if ritsu_testkit::need(Need::Python) {
        match mypy() {
            Some((mypy, python, tigerbeetle)) => {
                for out in &made {
                    type_check_python(&out.join("python"), &mypy, &python, std::slice::from_ref(&tigerbeetle));
                }
                println!("mypy --strict passes the Python of three packages");
            }
            None => ritsu_testkit::skip("mypy, Temporal's Python SDK (crates/dandori/tools/temporal-python) or tigerbeetle (crates/chobo/tools/runner) is not installed; the Python of the packages is not type-checked"),
        }
    }
    if ready(Need::Go, have_go, "go is not installed; the Go of the packages is not vetted") {
        match go_module() {
            Some(gomod) => {
                for out in &made {
                    let name = out.file_name().unwrap().to_string_lossy().to_string();
                    type_check_go(&t.path().join(format!("go-{name}")), &out.join("go"), &gomod);
                }
                println!("go vet and gofmt pass the Go of three packages");
            }
            None => ritsu_testkit::skip("the go.mod of crates/dandori/tools/temporal-go or crates/chobo/tools/runner/go is missing; the Go of the packages is not vetted"),
        }
    }
}

// ── the enums rules share ────────────────────────────────────────────────────────────────────────

/// Two rules that read their enums from one file of enums (rulec's DESIGN §15.212): the shipping
/// fee and the returns of an order, both over the categories and zones of `rules/shared/catalog.rule`.
const CATALOG: &str = "tests/projects/catalog";

/// The file of enums is one module of each package, beside the rules, and the module of each rule
/// imports its types from it: in Python from `.catalog`, in TypeScript from `"./catalog"`, in Go
/// from the package under `--module`. An app that calls both rules hands them one value, and the
/// packages pass `mypy --strict`, `tsc --strict` and `go vet`.
#[test]
fn a_file_of_enums_is_one_module_the_rules_of_the_package_import() {
    let t = TempDir::new("gen-enums");
    let out = t.path().join("catalog");
    let (code, _, err) = generate(&here().join(CATALOG), &out, &[]);
    assert_eq!(code, 0, "ritsu gen of the catalog:\n{err}");
    let py = read(&out.join("python/generated/rules/catalog_shipping.py"));
    assert!(py.contains("from .catalog import Category as Category, Zone as Zone"), "{py}");
    assert!(!py.contains("class Category("), "the rule declares a Category of its own:\n{py}");
    let ts = read(&out.join("typescript/rules/catalog_shipping.ts"));
    assert!(ts.contains("from \"./catalog\";"), "{ts}");
    let go = read(&out.join("go/rules/catalogshipping/catalog_shipping.go"));
    assert!(go.contains(&format!("\"{MODULE}/rules/catalog\"")) && go.contains("type Category = catalog.Category"), "{go}");
    for f in ["python/generated/rules/catalog.py", "typescript/rules/catalog.ts", "go/rules/catalog/catalog.go"] {
        assert!(out.join(f).is_file(), "{f} is not written");
    }

    if ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not installed; the app over the package is not run") {
        let app = out.join("python/app.py");
        std::fs::write(
            &app,
            "from generated.rules import catalog, catalog_returns, catalog_shipping\n\n\
             goods = catalog.Category.ELECTRONICS\n\
             zone = catalog.Zone.NATIONAL\n\
             fee = catalog_shipping.catalog_shipping(goods, zone, catalog_shipping.Kilogram(10))\n\
             back = catalog_returns.catalog_returns(goods, zone, False)\n\
             print(fee, back.window, back.free_return, catalog.category_label(goods))\n",
        )
        .unwrap();
        let o = Command::new("python3").arg("app.py").current_dir(out.join("python")).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "21 14 True Electronics");
        std::fs::remove_file(&app).unwrap();
        match mypy() {
            Some((mypy, python, _)) => {
                type_check_python(&out.join("python"), &mypy, &python, &[]);
                println!("mypy --strict passes the Python of the catalog");
            }
            None => ritsu_testkit::skip("mypy or Temporal's Python SDK (crates/dandori/tools/temporal-python) is not installed; the Python of the catalog is not type-checked"),
        }
    }
    if ritsu_testkit::need(Need::Node) {
        match tsc() {
            Some((tsc, modules)) => {
                type_check_typescript(&out.join("typescript"), &tsc, &modules);
                println!("tsc --strict passes the TypeScript of the catalog");
            }
            None => ritsu_testkit::skip("TypeScript (crates/dandori/tools/temporal) is not installed; the TypeScript of the catalog is not type-checked"),
        }
    }
    if ready(Need::Go, have_go, "go is not installed; the Go of the catalog is not vetted") {
        // the rules import nothing outside the package, so the module requires nothing
        let gomod = ("module gocheck\n\ngo 1.24\n".to_string(), String::new());
        type_check_go(&t.path().join("go-catalog"), &out.join("go"), &gomod);
        println!("go vet and gofmt pass the Go of the catalog");
    }
}

/// The Go of a package put in a module as ritsu gen writes it, its directory named `go`
/// (`--module gocheck/gen/go`): the root of the package cannot be `package go`, a word Go keeps, and
/// is the package's name instead (`generated`), or, with Go keeping that too (`--name go`), `pgo`.
/// go vet and gofmt pass both.
#[test]
fn a_package_whose_directory_is_named_go_vets() {
    if !ready(Need::Go, have_go, "go is not installed; the package at a directory named go is not vetted") {
        return;
    }
    let t = TempDir::new("gen-go-dir");
    for (more, want) in [(&[][..], "generated"), (&["--name", "go"][..], "pgo")] {
        let out = t.path().join(format!("out-{want}"));
        let mut args = vec!["gen", "--root", ".", "--out", out.to_str().unwrap(), "--target", "go", "--module", "gocheck/gen/go"];
        args.extend_from_slice(more);
        let (code, _, err) = ritsu_in(&here().join(CATALOG), &args);
        assert_eq!(code, 0, "{err}");
        assert_has(&out.join("go/doc.go"), &format!("// Package {want} is the rules, the dates, the clients of the books and the workflows of a project, as\n// one package (ritsu gen): rules/, dates/, books/ and flows/ under the import path gocheck/gen/go.\n"));
        assert_has(&out.join("go/doc.go"), &format!("\npackage {want}\n"));
        let module = t.path().join(format!("go-{want}"));
        std::fs::create_dir_all(module.join("gen")).unwrap();
        std::fs::write(module.join("go.mod"), "module gocheck\n\ngo 1.24\n").unwrap();
        ritsu_testkit::tmp::copy_dir(&out.join("go"), &module.join("gen/go"));
        let o = go_in(&module).args(["vet", "./..."]).output().unwrap();
        assert!(o.status.success(), "go vet finds fault with the package at gen/go ({want}):\n{}", said(&o));
        let fmt = Command::new("gofmt").args(["-l", "gen"]).current_dir(&module).output().unwrap();
        assert!(fmt.status.success() && fmt.stdout.is_empty(), "gofmt would write these otherwise:\n{}", said(&fmt));
    }
    println!("go vet and gofmt pass the package at a directory named go, as package generated and as package pgo");
}

// ── the units ────────────────────────────────────────────────────────────────────────────────────

/// The marketplace (English): a rule that splits a sale into the marketplace's fee and the shop's
/// part, in cents (`rules/sale_split.rule`), the book that records the sale (chobo's example), and a
/// checkout written by hand in each language on the package (`service/`), with no workflow between
/// the rule and the book.
const MARKETPLACE: &str = "tests/projects/marketplace";

/// The marketplace at `app`, laid out as its checkout is written against: the package's TypeScript
/// and Python under `generated/` (`--out generated`), its Go put at `gen/` in the module
/// `marketplace` (`--module marketplace/gen`), with the books kept where `books` says. With
/// `dollars`, the rule counts whole dollars (`money[USD]`) where the book counts cents.
fn marketplace(app: &Path, books: &str, dollars: bool) {
    ritsu_testkit::tmp::copy_dir(&here().join(MARKETPLACE), app);
    if dollars {
        let rule = app.join("rules/sale_split.rule");
        std::fs::write(&rule, read(&rule).replace("USDc", "USD")).unwrap();
    }
    let (code, _, err) = ritsu_in(app, &["gen", "--root", ".", "--out", "generated", "--module", "marketplace/gen", "--books", books]);
    assert_eq!(code, 0, "ritsu gen of the marketplace ({books}):\n{err}");
    std::fs::rename(app.join("generated/go"), app.join("gen")).unwrap();
}

/// The module `marketplace` at `app`, requiring what the package's Go imports.
fn marketplace_module(app: &Path, gomod: &(String, String)) {
    std::fs::write(app.join("go.mod"), gomod.0.replacen("module gocheck", "module marketplace", 1)).unwrap();
    std::fs::write(app.join("go.sum"), &gomod.1).unwrap();
}

fn said(o: &std::process::Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

/// The amounts of money and of the quantities are the package's one type of their unit (rulec's
/// DESIGN §15.248): `units` declares it, the rule's module hands it on, and the book's client takes
/// it. So the checkout, which gives the book's sale the fee and the shop's part the rule returns,
/// passes tsc --strict, mypy --strict and go vet with the books on PostgreSQL, on TigerBeetle and on
/// the MySQL family; and when the rule counts whole dollars instead, each of the three stops the
/// checkout where the dollars meet the book's cents.
#[test]
fn the_checkout_hands_the_book_the_rules_amounts_in_their_units_type() {
    let t = TempDir::new("gen-units");
    let mut apps = Vec::new();
    for books in ["postgres", "tigerbeetle", "mysql"] {
        let app = t.path().join(format!("app-{books}"));
        marketplace(&app, books, false);
        apps.push(app);
    }
    let app = &apps[0];
    assert_has(&app.join("generated/typescript/units.ts"), "export type USDc = bigint & { readonly __rulec: \"USDc\" }; // money[USDc]");
    assert_has(&app.join("generated/typescript/rules/sale_split.ts"), "import type { USDc } from \"../units\";\nexport type { USDc };");
    assert_has(&app.join("generated/typescript/books/marketplace.ts"), "export type saleArgs = { order: string; shop: string; to_shop: units.USDc; fee: units.USDc };");
    assert_has(&app.join("generated/typescript/index.ts"), "export * as units from \"./units\";");
    assert_has(&app.join("generated/python/generated/units.py"), "USDc = NewType(\"USDc\", int)  # money[USDc]");
    assert_has(&app.join("generated/python/generated/rules/sale_split.py"), "from ..units import USDc as USDc");
    assert_has(&app.join("generated/python/generated/books/marketplace.py"), "def do(self, *, order: str, shop: str, to_shop: units.USDc, fee: units.USDc) -> Result:");
    assert_has(&app.join("gen/units/units.go"), "type USDc int64");
    assert_has(&app.join("gen/rules/salesplit/sale_split.go"), "type USDc = units.USDc");
    // the runtime of each client reads an amount as an int64
    for (app, books) in apps.iter().zip(["postgres", "tigerbeetle", "mysql"]) {
        let book = read(&app.join("gen/books/marketplace/book.go"));
        assert!(book.contains("\tFee     units.USDc\n") && book.contains("int64(args.Fee)") && !book.contains(", args.Fee"), "{books}:\n{book}");
    }
    let dollars = t.path().join("dollars");
    marketplace(&dollars, "postgres", true);
    assert_has(&dollars.join("generated/typescript/units.ts"), "export type USD = bigint & { readonly __rulec: \"USD\" }; // money[USD]");

    if ritsu_testkit::need(Need::Node) {
        match tsc() {
            Some((tsc, modules)) => {
                for app in &apps {
                    type_check_typescript(app, &tsc, &modules);
                }
                // what it says is on chobo's site (website/chobo/docs/together.md)
                let o = tsc_in(&dollars, &tsc, &modules);
                assert!(!o.status.success(), "tsc --strict passes the checkout of a rule of dollars");
                golden(here().join("tests/golden/gen/checkout-dollars.tsc.txt"), &said(&o));
                println!("tsc --strict passes the checkout on three books, and stops it where a rule of dollars meets the book's cents");
            }
            None => ritsu_testkit::skip("TypeScript (crates/dandori/tools/temporal) or tigerbeetle-node (crates/chobo/tools/runner) is not installed; the checkout is not type-checked"),
        }
    }
    if ritsu_testkit::need(Need::Python) {
        match mypy() {
            Some((mypy, python, tigerbeetle)) => {
                for app in apps.iter().chain([&dollars]) {
                    let o = mypy_in(app, &mypy, &python, std::slice::from_ref(&tigerbeetle), &["service/checkout.py"], &[app.join("generated/python")]);
                    let s = said(&o);
                    if app == &dollars {
                        assert!(!o.status.success() && s.contains("service/checkout.py") && s.contains("incompatible type \"USD\"; expected \"USDc\""), "{s}");
                    } else {
                        assert!(o.status.success(), "{}: mypy --strict finds fault with the checkout:\n{s}", app.display());
                    }
                }
                println!("mypy --strict passes the checkout on three books, and stops it where a rule of dollars meets the book's cents");
            }
            None => ritsu_testkit::skip("mypy, Temporal's Python SDK (crates/dandori/tools/temporal-python) or tigerbeetle (crates/chobo/tools/runner) is not installed; the checkout is not type-checked"),
        }
    }
    if ready(Need::Go, have_go, "go is not installed; the checkout is not vetted") {
        match go_module() {
            Some(gomod) => {
                for app in apps.iter().chain([&dollars]) {
                    marketplace_module(app, &gomod);
                    let o = go_in(app).args(["vet", "./..."]).output().unwrap();
                    let s = said(&o);
                    if app == &dollars {
                        assert!(!o.status.success() && s.contains("service/checkout.go") && s.contains("cannot use") && s.contains("units.USDc") && s.contains("salesplit.USD"), "{s}");
                    } else {
                        assert!(o.status.success(), "{}: go vet finds fault with the checkout:\n{s}", app.display());
                        let fmt = Command::new("gofmt").args(["-l", "gen", "service"]).current_dir(app).output().unwrap();
                        assert!(fmt.status.success() && fmt.stdout.is_empty(), "{}: gofmt would write these otherwise:\n{}", app.display(), said(&fmt));
                    }
                }
                println!("go vet and gofmt pass the checkout on three books, and go vet stops it where a rule of dollars meets the book's cents");
            }
            None => ritsu_testkit::skip("the go.mod of crates/dandori/tools/temporal-go or crates/chobo/tools/runner/go is missing; the checkout is not vetted"),
        }
    }
}

/// A rule that writes yen as `円`, and one that writes it as `JPY`.
const PRICE_RULE: &str = "rule price v1\ndescription \"The price of a size, with tax. Written for ritsu's tests\"\n\nenum size = small | large\n\ninputs\n  size : size\n\noutputs\n  price : money[円, incl_tax]  round down(1円)\n\ntable pick\npolicy unique\n| size  | -> price : money[円, incl_tax] |\n| small | 500円                          |\n| large | 800円                          |\n";
const COUPON_RULE: &str = "rule coupon v1\ndescription \"What a total with tax comes to with the coupon: 100 yen off a total of 600 yen or more. Written for ritsu's tests\"\n\ninputs\n  total : money[JPY, incl_tax]  range >=0JPY <=100000JPY\n\noutputs\n  pay : money[JPY, incl_tax]  round down(1JPY)\n\ndefine less : money[JPY, incl_tax] = total - 100JPY\n\ntable off\npolicy unique\n| total    | -> pay : money[JPY, incl_tax] |\n| >=600JPY | less                          |\n| <600JPY  | total                         |\n";

/// A unit is one type in a package however its rules spell it: the price that one rule returns in
/// `円` is what the other takes in `JPY`. The `units` module declares `YenInclTax`, and `JPYInclTax`
/// as another name of it; each rule's module names the type as its rule writes it. Code that hands
/// one rule's answer to the other passes tsc --strict, mypy --strict and go vet, and runs (800 yen,
/// 100 off). Each rule's module used to declare a brand of its own, and none of the three took it.
#[test]
fn a_unit_is_one_type_across_the_rules_however_they_spell_it() {
    let t = TempDir::new("gen-spellings");
    let app = t.path().join("app");
    std::fs::create_dir_all(app.join("rules")).unwrap();
    std::fs::write(app.join("rules/price.rule"), PRICE_RULE).unwrap();
    std::fs::write(app.join("rules/coupon.rule"), COUPON_RULE).unwrap();
    let (code, _, err) = ritsu_in(&app, &["gen", "--root", ".", "--out", "generated", "--module", "spellings/gen"]);
    assert_eq!(code, 0, "{err}");
    std::fs::rename(app.join("generated/go"), app.join("gen")).unwrap();
    assert_has(&app.join("generated/typescript/units.ts"), "export type YenInclTax = bigint & { readonly __rulec: \"YenInclTax\" }; // money[円, incl_tax]\nexport type JPYInclTax = YenInclTax; // money[JPY, incl_tax], the same unit\n");
    assert_has(&app.join("generated/python/generated/units.py"), "YenInclTax = NewType(\"YenInclTax\", int)  # money[円, incl_tax]\nJPYInclTax = YenInclTax  # money[JPY, incl_tax], the same unit\n");
    assert_has(&app.join("gen/units/units.go"), "type JPYInclTax = YenInclTax\n");
    assert_has(&app.join("generated/typescript/rules/coupon.ts"), "import type { JPYInclTax } from \"../units\";");
    std::fs::write(app.join("app.ts"), "import { rules } from \"./generated/typescript/index\";\n\nexport const pay = rules.coupon.coupon(rules.price.price(rules.price.Size.LARGE));\n").unwrap();
    std::fs::write(app.join("app.py"), "from generated.rules import coupon, price\n\nprint(coupon.coupon(price.price(price.Size.LARGE)))\n").unwrap();
    std::fs::create_dir_all(app.join("app")).unwrap();
    std::fs::write(
        app.join("app/main.go"),
        "package main\n\nimport (\n\t\"fmt\"\n\n\t\"spellings/gen/rules/coupon\"\n\t\"spellings/gen/rules/price\"\n)\n\nfunc main() {\n\tp, err := price.Price(price.Input{Size: price.SizeLarge})\n\tif err != nil {\n\t\tpanic(err)\n\t}\n\tpay, err := coupon.Coupon(coupon.Input{Total: p})\n\tif err != nil {\n\t\tpanic(err)\n\t}\n\tfmt.Println(pay)\n}\n",
    )
    .unwrap();
    if ritsu_testkit::need(Need::Node) {
        match tsc() {
            Some((tsc, modules)) => {
                type_check_typescript(&app, &tsc, &modules);
                println!("tsc --strict passes the price of one rule given to the other");
            }
            None => ritsu_testkit::skip("TypeScript (crates/dandori/tools/temporal) is not installed; the two rules are not type-checked"),
        }
    }
    if ritsu_testkit::need(Need::Python) {
        match mypy() {
            Some((mypy, python, _)) => {
                let o = mypy_in(&app, &mypy, &python, &[], &["app.py"], &[app.join("generated/python")]);
                assert!(o.status.success(), "mypy --strict finds fault with the price given to the coupon:\n{}", said(&o));
                let o = Command::new(&python).arg("app.py").env("PYTHONPATH", app.join("generated/python")).current_dir(&app).output().unwrap();
                assert!(o.status.success(), "{}", said(&o));
                assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "700");
                println!("mypy --strict passes the price of one rule given to the other, and it runs");
            }
            None => ritsu_testkit::skip("mypy or Temporal's Python SDK (crates/dandori/tools/temporal-python) is not installed; the two rules are not type-checked"),
        }
    }
    if ready(Need::Go, have_go, "go is not installed; the two rules are not vetted") {
        std::fs::write(app.join("go.mod"), "module spellings\n\ngo 1.24\n").unwrap();
        let o = go_in(&app).args(["vet", "./..."]).output().unwrap();
        assert!(o.status.success(), "go vet finds fault with the price given to the coupon:\n{}", said(&o));
        let o = go_in(&app).args(["run", "./app"]).output().unwrap();
        assert!(o.status.success(), "{}", said(&o));
        assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "700");
        println!("go vet passes the price of one rule given to the other, and it runs");
    }
}

/// What a run of the checkout prints: the first sale's answer, the second's (the same order again),
/// and the balances after them. A sale of $9.99 on the basic plan is 149 cents of fee and 850 to the
/// shop.
fn checkout_ran(lang: &str) -> String {
    format!("{lang}: done, then done_before; shop-1 850, fees 149")
}

/// One sale through the TypeScript checkout, compiled by tsc, on the PostgreSQL of the environment.
const CHECKOUT_TS: &str = r#"// One sale through the checkout, and the same sale again, then the balances.
const { Client } = require("pg");
const { books } = require("./dist/generated/typescript/index");
const { checkout } = require("./dist/service/checkout");
(async () => {
  const db = new Client({ host: process.env.PGHOST, port: Number(process.env.PGPORT), user: process.env.PGUSER, database: process.env.PGDATABASE });
  await db.connect();
  const book = books.marketplace.postgres(db, { tenant: "typescript" });
  const r = await checkout(book, "A-1", "shop-1", "basic", 999n);
  const again = await checkout(book, "A-1", "shop-1", "basic", 999n);
  const shop = await book.balance.shop_balance({ shop: "shop-1" });
  const fees = await book.balance.fees();
  console.log(`typescript: ${r.result}, then ${again.result}; shop-1 ${shop.posted}, fees ${fees.posted}`);
  await db.end();
})().catch((e) => { console.error(e); process.exit(1); });
"#;

/// The same in Python, through psycopg.
const CHECKOUT_PY: &str = r#""""One sale through the checkout, and the same sale again, then the balances."""
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [os.path.join(here, "generated", "python"), os.path.join(here, "service")]

import psycopg  # noqa: E402

from checkout import checkout  # noqa: E402
from generated import units  # noqa: E402
from generated.books import marketplace  # noqa: E402
from generated.rules import sale_split  # noqa: E402

conn = psycopg.connect(host=os.environ["PGHOST"], port=os.environ["PGPORT"], user=os.environ["PGUSER"], dbname=os.environ["PGDATABASE"], autocommit=True)
book = marketplace.postgres(conn, tenant="python")
r = checkout(book, "A-1", "shop-1", sale_split.Plan.BASIC, units.USDc(999))
again = checkout(book, "A-1", "shop-1", sale_split.Plan.BASIC, units.USDc(999))
shop = book.balance.shop_balance(shop="shop-1")
fees = book.balance.fees()
print(f"python: {r.result}, then {again.result}; shop-1 {shop.posted}, fees {fees.posted}")
conn.close()
"#;

/// The same in Go, on the book `open` makes (with what it imports for that).
fn checkout_go(imports: &str, open: &str) -> String {
    format!(
        r#"// One sale through the checkout, and the same sale again, then the balances.
package main

import (
	"context"
	"fmt"
	"os"

{imports}
	"marketplace/gen/books/marketplace"
	"marketplace/gen/rules/salesplit"
	"marketplace/service"
)

func main() {{
	ctx := context.Background()
{open}
	r, err := service.Checkout(ctx, book, "A-1", "shop-1", salesplit.PlanBasic, 999)
	if err != nil {{
		panic(err)
	}}
	again, err := service.Checkout(ctx, book, "A-1", "shop-1", salesplit.PlanBasic, 999)
	if err != nil {{
		panic(err)
	}}
	shop, err := book.Balance.Shop_balance(ctx, marketplace.Shop_balanceAccount{{Shop: "shop-1"}})
	if err != nil {{
		panic(err)
	}}
	fees, err := book.Balance.Fees(ctx)
	if err != nil {{
		panic(err)
	}}
	fmt.Printf("go: %s, then %s; shop-1 %d, fees %d\n", r.Outcome, again.Outcome, shop.Posted, fees.Posted)
}}
"#
    )
}

/// `go run` of the checkout's Go at `app`, with `env`: what it prints.
fn run_checkout_go(app: &Path, gomod: &(String, String), imports: &str, open: &str, env: &[(&str, String)]) -> String {
    marketplace_module(app, gomod);
    std::fs::create_dir_all(app.join("cmd/run")).unwrap();
    std::fs::write(app.join("cmd/run/main.go"), checkout_go(imports, open)).unwrap();
    let o = go_in(app).args(["run", "./cmd/run"]).envs(env.iter().map(|(k, v)| (*k, v.as_str()))).output().unwrap();
    assert!(o.status.success(), "{}: the checkout's Go does not run:\n{}", app.display(), said(&o));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

/// The checkout runs, recording the sale: on a PostgreSQL the test starts, in TypeScript (compiled by
/// tsc, through pg), in Python (through psycopg) and in Go (through pgx); and in Go on TigerBeetle and
/// on MySQL too, whose clients read an amount as the int64 its type of the unit is. The fee is the
/// rule's, the rest is the shop's, and the same order again is done before.
#[test]
fn the_checkout_records_the_sale() {
    let t = TempDir::new("gen-checkout");
    let go = ready(Need::Go, have_go, "go is not installed; the checkout's Go is not run");
    let gomod = if go { go_module() } else { None };
    if go && gomod.is_none() {
        ritsu_testkit::skip("the go.mod of crates/dandori/tools/temporal-go or crates/chobo/tools/runner/go is missing; the checkout's Go is not run");
    }
    if ready(Need::Postgres, || ritsu_testkit::pg::bin().is_ok(), "PostgreSQL is not here; the checkout is not run on it") {
        let pg = ritsu_testkit::pg::Postgres::start().unwrap_or_else(|e| panic!("{e}"));
        let app = t.path().join("app-postgres");
        marketplace(&app, "postgres", false);
        let o = pg.psql("postgres").arg("-f").arg(app.join("generated/typescript/books/marketplace.sql")).output().unwrap();
        assert!(o.status.success(), "the book's schema: {}", said(&o));
        let env = [("PGHOST", pg.socket.to_string_lossy().to_string()), ("PGPORT", pg.port.to_string()), ("PGUSER", pg.user.clone()), ("PGDATABASE", "postgres".to_string())];
        let mut ran = Vec::new();
        let pg_module = workspace().join("crates/chobo/tools/runner/node_modules/pg");
        if ritsu_testkit::need(Need::Node) {
            match tsc() {
                Some((tsc, _)) if pg_module.exists() => {
                    let tsconfig = serde_json::json!({
                        "compilerOptions": { "strict": true, "module": "commonjs", "target": "es2022", "outDir": "dist", "rootDir": ".", "skipLibCheck": true, "types": [] },
                        "include": ["service/**/*.ts", "generated/typescript/**/*.ts"]
                    });
                    std::fs::write(app.join("tsconfig.json"), serde_json::to_string_pretty(&tsconfig).unwrap()).unwrap();
                    let o = Command::new(&tsc).arg("-p").arg(app.join("tsconfig.json")).output().unwrap();
                    assert!(o.status.success(), "tsc of the checkout: {}", said(&o));
                    std::fs::create_dir_all(app.join("node_modules")).unwrap();
                    std::os::unix::fs::symlink(&pg_module, app.join("node_modules/pg")).unwrap();
                    std::fs::write(app.join("run.js"), CHECKOUT_TS).unwrap();
                    let o = Command::new("node").arg("run.js").envs(env.iter().map(|(k, v)| (*k, v.as_str()))).current_dir(&app).output().unwrap();
                    assert!(o.status.success(), "the checkout's TypeScript does not run: {}", said(&o));
                    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), checkout_ran("typescript"));
                    ran.push("TypeScript");
                }
                _ => ritsu_testkit::skip("TypeScript (crates/dandori/tools/temporal) or pg (crates/chobo/tools/runner) is not installed; the checkout's TypeScript is not run"),
            }
        }
        let python = workspace().join("crates/chobo/tools/runner/.venv/bin/python");
        if ready(Need::Python, || python.exists(), "chobo's runner of Python (crates/chobo/tools/runner/.venv) is not installed; the checkout's Python is not run") {
            std::fs::write(app.join("run.py"), CHECKOUT_PY).unwrap();
            let o = Command::new(&python).args(["-I", "run.py"]).envs(env.iter().map(|(k, v)| (*k, v.as_str()))).current_dir(&app).output().unwrap();
            assert!(o.status.success(), "the checkout's Python does not run: {}", said(&o));
            assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), checkout_ran("python"));
            ran.push("Python");
        }
        if let Some(gomod) = &gomod {
            let url = format!("host={} port={} user={} dbname=postgres", pg.socket.display(), pg.port, pg.user);
            let open = "\tconn, err := pgx.Connect(ctx, os.Getenv(\"DATABASE_URL\"))\n\tif err != nil {\n\t\tpanic(err)\n\t}\n\tdefer conn.Close(ctx)\n\tbook := marketplace.Postgres(conn, \"go\")";
            assert_eq!(run_checkout_go(&app, gomod, "\t\"github.com/jackc/pgx/v5\"\n", open, &[("DATABASE_URL", url)]), checkout_ran("go"));
            ran.push("Go");
        }
        println!("the checkout ran on PostgreSQL in {}", ran.join(", "));
    }
    let Some(gomod) = gomod else { return };
    if ready(Need::TigerBeetle, || ritsu_testkit::tigerbeetle::bin().is_ok(), "TigerBeetle is not here; the checkout's Go is not run on it") {
        let tb = ritsu_testkit::tigerbeetle::TigerBeetle::start().unwrap_or_else(|e| panic!("{e}"));
        let app = t.path().join("app-tigerbeetle");
        marketplace(&app, "tigerbeetle", false);
        let open = "\tclient, err := tb.NewClient(tb.ToUint128(0), []string{os.Getenv(\"TB_ADDRESS\")})\n\tif err != nil {\n\t\tpanic(err)\n\t}\n\tdefer client.Close()\n\tbook := marketplace.TigerBeetle(client, \"go\")";
        assert_eq!(run_checkout_go(&app, &gomod, "\ttb \"github.com/tigerbeetle/tigerbeetle-go\"\n", open, &[("TB_ADDRESS", tb.address.clone())]), checkout_ran("go"));
        println!("the checkout's Go ran on TigerBeetle");
    }
    if ready(Need::MySql, || ritsu_testkit::mysql::bin().is_ok(), "MySQL is not here; the checkout's Go is not run on it") {
        let my = ritsu_testkit::mysql::MySql::start().unwrap_or_else(|e| panic!("{e}"));
        let db = my.database("checkout").unwrap_or_else(|e| panic!("{e}"));
        let app = t.path().join("app-mysql");
        marketplace(&app, "mysql", false);
        let o = my.mysql(&db).stdin(std::fs::File::open(app.join("generated/typescript/books/marketplace.sql")).unwrap()).output().unwrap();
        assert!(o.status.success(), "the book's tables: {}", said(&o));
        let open = "\tcfg := mysql.NewConfig()\n\tcfg.User, cfg.Net, cfg.Addr, cfg.DBName = os.Getenv(\"MYSQL_USER\"), \"tcp\", os.Getenv(\"MYSQL_ADDR\"), os.Getenv(\"MYSQL_DB\")\n\tconnector, err := mysql.NewConnector(cfg)\n\tif err != nil {\n\t\tpanic(err)\n\t}\n\tdb := sql.OpenDB(connector)\n\tdefer db.Close()\n\tbook := marketplace.MySQL(db, \"go\", false)";
        let env = [("MYSQL_USER", my.user.clone()), ("MYSQL_ADDR", format!("{}:{}", my.host, my.port)), ("MYSQL_DB", db.clone())];
        assert_eq!(run_checkout_go(&app, &gomod, "\t\"database/sql\"\n\n\t\"github.com/go-sql-driver/mysql\"\n", open, &env), checkout_ran("go"));
        println!("the checkout's Go ran on {}", my.label);
    }
}

// ── the gates ────────────────────────────────────────────────────────────────────────────────────

/// The TypeScript sekisho's code imports, from sekisho's runner (tools/runner-ts): Cedar's own
/// WebAssembly build.
fn cedar_wasm() -> Option<PathBuf> {
    Some(workspace().join("crates/sekisho/tools/runner-ts/node_modules/@cedar-policy")).filter(|p| p.join("cedar-wasm/nodejs/cedar_wasm.d.ts").exists())
}

/// The gates of a project (sekisho's DESIGN 5.6): their Cedar once beside the packages, in
/// `<out>/cedar/`, whatever the targets; in each package, the code of their requests in `authz/`,
/// which reads the package's rules/ and dates/, in the indexes, with the Cedar it asks among the
/// dependencies at the versions sekisho's runners lock (the AWS SDK's client of Verified Permissions,
/// or boto3, with `--authorizer avp`). tsc --strict, mypy --strict and go vet pass the packages;
/// `--check` finds nothing stale after a write, and says when a gate changed.
#[test]
fn the_gates_of_a_project_go_into_the_package() {
    let t = TempDir::new("gen-gates");
    let project = t.path().join("refunds");
    ritsu_testkit::tmp::copy_dir(&here().join(REFUNDS), &project);
    let out = t.path().join("out");
    let (code, _, err) = generate(&project, &out, &[]);
    assert_eq!(code, 0, "{err}");
    let mut listed: Vec<String> = files(&out).into_iter().filter(|f| f.starts_with("cedar/") || f.contains("/authz/")).collect();
    listed.sort();
    assert_eq!(
        listed,
        [
            "cedar/refunds.cedar",
            "cedar/refunds.cedarschema",
            "cedar/refunds.cedarschema.json",
            "cedar/refunds.policies.json",
            "cedar/refunds_ja.cedar",
            "cedar/refunds_ja.cedarschema",
            "cedar/refunds_ja.cedarschema.json",
            "cedar/refunds_ja.policies.json",
            "go/authz/refunds/refunds.go",
            "go/authz/refundsja/refundsja.go",
            "python/generated/authz/__init__.py",
            "python/generated/authz/refunds.py",
            "python/generated/authz/refunds_ja.py",
            "typescript/authz/index.ts",
            "typescript/authz/refunds.ts",
            "typescript/authz/refunds_ja.ts",
        ]
    );
    let authz = out.join("typescript/authz/refunds.ts");
    assert_has(&authz, "// Source: refunds.gate (gate refunds v1, sha256:");
    assert_has(&authz, "import * as rule_refund_limit from \"../rules/refund_limit\";\nimport * as dates_refund_terms from \"../dates/refund_terms\";\nimport * as dates_england_and_wales from \"../dates/england_and_wales\";\n");
    assert_has(&out.join("typescript/index.ts"), "export * as authz from \"./authz/index\";\n");
    assert_has(&out.join("typescript/authz/index.ts"), "export * as refunds from \"./refunds\";\nexport * as refunds_ja from \"./refunds_ja\";\n");
    assert_has(&out.join("typescript/package.json"), "    \"@cedar-policy/cedar-wasm\": \"4.13.0\",\n");
    assert_has(&out.join("python/generated/__init__.py"), "from . import authz as authz\n");
    assert_has(&out.join("python/generated/authz/__init__.py"), "from . import refunds as refunds\nfrom . import refunds_ja as refunds_ja\n");
    assert_has(&out.join("python/pyproject.toml"), "  \"cedarpy==4.12.1\",\n");
    assert_has(&out.join("python/generated/authz/refunds.py"), "# Source: refunds.gate (gate refunds v1, sha256:");
    assert_has(&out.join("go/doc.go"), "//\tgithub.com/cedar-policy/cedar-go v1.8.0\n");
    assert_has(&out.join("go/authz/refunds/refunds.go"), &format!("\"{MODULE}/rules/refundlimit\""));
    // the Cedar is sekisho gen's, but the head names the file from the project's root
    let cedar = read(&out.join("cedar/refunds.cedar"));
    assert!(cedar.starts_with("// Code generated by sekisho ") && cedar.contains("\n// Source: refunds.gate (gate refunds v1, sha256:"), "{cedar}");
    type_check_the_gates(&t, &out);
    let (code, stdout, _) = generate(&project, &out, &["--check"]);
    assert_eq!((code, stdout.as_str()), (0, ""), "a package just written is not stale");
    // a gate changed: its Cedar and its code in each language, and nothing else
    let gate = project.join("refunds.gate");
    std::fs::write(&gate, read(&gate).replace("A sketch of a shop's own terms", "A sketch of the shop's own terms")).unwrap();
    let (code, stdout, _) = generate(&project, &out, &["--check"]);
    assert_eq!(code, 1);
    let o = out.display().to_string();
    assert_eq!(
        stdout,
        format!(
            "generated file is stale or hand-edited: {o}/typescript/authz/refunds.ts\ngenerated file is stale or hand-edited: {o}/python/generated/authz/refunds.py\ngenerated file is stale or hand-edited: {o}/go/authz/refunds/refunds.go\ngenerated file is stale or hand-edited: {o}/cedar/refunds.cedar\ngenerated file is stale or hand-edited: {o}/cedar/refunds.cedarschema\ngenerated file is stale or hand-edited: {o}/cedar/refunds.cedarschema.json\n"
        )
    );
    // asking Verified Permissions: the AWS SDK's client and boto3, and no Cedar in the packages
    let avp = t.path().join("avp");
    let (code, _, err) = generate(&project, &avp, &["--authorizer", "avp"]);
    assert_eq!(code, 0, "{err}");
    let pkg = read(&avp.join("typescript/package.json"));
    assert!(pkg.contains("    \"@aws-sdk/client-verifiedpermissions\": \"3.1146.0\",\n") && !pkg.contains("cedar-wasm"), "{pkg}");
    let toml = read(&avp.join("python/pyproject.toml"));
    assert!(toml.contains("  \"boto3==1.43.103\",\n") && !toml.contains("cedarpy"), "{toml}");
    let doc = read(&avp.join("go/doc.go"));
    assert!(doc.contains("//\tgithub.com/aws/aws-sdk-go-v2/service/verifiedpermissions v1.41.1\n") && !doc.contains("cedar-go"), "{doc}");
    assert_has(&avp.join("typescript/authz/refunds.ts"), "export async function authorizeRefundOrder(avp: VerifiedPermissions, store: Store, principal: Principal, input: RefundOrderInput");
}

/// tsc --strict, mypy --strict and go vet on the packages of the gates at `out`, each with the
/// Cedar of its language from sekisho's runners.
fn type_check_the_gates(t: &TempDir, out: &Path) {
    if ritsu_testkit::need(Need::Node) {
        match (tsc(), cedar_wasm()) {
            (Some((tsc, mut modules)), Some(cedar)) => {
                modules.push(cedar);
                type_check_typescript(&out.join("typescript"), &tsc, &modules);
                println!("tsc --strict passes the TypeScript package of the gates");
            }
            _ => ritsu_testkit::skip("TypeScript (crates/dandori/tools/temporal), tigerbeetle-node (crates/chobo/tools/runner) or cedar-wasm (crates/sekisho/tools/runner-ts) is not installed; the TypeScript of the gates is not type-checked"),
        }
    }
    if ritsu_testkit::need(Need::Python) {
        match (mypy(), cedarpy()) {
            (Some((mypy, python, tigerbeetle)), Some(cedarpy)) => {
                type_check_python(&out.join("python"), &mypy, &python, &[tigerbeetle, cedarpy]);
                println!("mypy --strict passes the Python package of the gates");
            }
            _ => ritsu_testkit::skip("mypy, Temporal's Python SDK (crates/dandori/tools/temporal-python), tigerbeetle (crates/chobo/tools/runner) or cedarpy (crates/sekisho/tools/runner-py) is not installed; the Python of the gates is not type-checked"),
        }
    }
    if ready(Need::Go, have_go, "go is not installed; the Go of the gates is not vetted") {
        // the gates, the rules and the dates they read, in a module that requires what sekisho's
        // runner of Go locks (cedar-go, the AWS SDK's client); the flows are vetted with Temporal's
        // SDK by the_packages_pass_the_type_checkers
        let runner = workspace().join("crates/sekisho/tools/runner-go");
        if !runner.join("go.mod").exists() {
            ritsu_testkit::skip("the go.mod of crates/sekisho/tools/runner-go is missing; the Go of the gates is not vetted");
            return;
        }
        let gomod = read(&runner.join("go.mod")).replacen("module sekisho-runner", "module gocheck", 1);
        let gates = t.path().join("go-gates");
        go_module_in(&gates, &out.join("go"), &(gomod, read(&runner.join("go.sum"))));
        std::fs::remove_dir_all(gates.join("generated/flows")).unwrap();
        let o = go_in(&gates).args(["vet", "./..."]).output().unwrap();
        assert!(o.status.success(), "go vet finds fault with the Go of the gates:\n{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        let fmt = Command::new("gofmt").arg("-l").arg("generated").current_dir(&gates).output().unwrap();
        assert!(fmt.status.success() && fmt.stdout.is_empty(), "gofmt would write these otherwise:\n{}", String::from_utf8_lossy(&fmt.stdout));
        println!("go vet and gofmt pass the Go of the gates, with the rules and the dates they read");
    }
}

// ── reading from the package ─────────────────────────────────────────────────────────────────────

fn assert_has(file: &Path, what: &str) {
    let text = read(file);
    assert!(text.contains(what), "{} does not have {what:?}:\n{text}", file.display());
}

/// The workflow's imports name the package's own modules, and nothing of rulec's, koyomi's or
/// chobo's is put beside the workflow.
#[test]
fn a_workflow_imports_the_rules_dates_and_books_of_its_package() {
    let t = TempDir::new("gen-reads");
    let out = t.path().join("out");
    let (code, _, err) = generate(&here().join(STOCKROOM), &out, &[]);
    assert_eq!(code, 0, "{err}");
    let ts = out.join("typescript/flows/order");
    assert_has(&ts.join("rules.ts"), "from \"../../rules/delivery\";");
    assert_has(&ts.join("rules.ts"), "from \"../../dates/payment_terms\";");
    assert_has(&ts.join("io.ts"), "import type { Book as Book0 } from \"../../books/stock\";");
    assert_has(&ts.join("io.ts"), "  books?: Books;\n");
    let py = out.join("python/generated/flows/order");
    assert_has(&py.join("rules.py"), "from ...rules.delivery import ");
    assert_has(&py.join("rules.py"), "from ...dates.payment_terms import payment, payment_at\n");
    assert_has(&py.join("io.py"), "from ...books.stock import Book as _Book0\n");
    assert_has(&py.join("io.py"), "        books: Books | None = None,\n");
    let go = out.join("go/flows/order");
    assert_has(&go.join("rules.go"), &format!("\t\"{MODULE}/dates/paymentterms\"\n\t\"{MODULE}/rules/delivery\"\n"));
    assert_has(&go.join("io_books.go"), &format!("\tddBook0 \"{MODULE}/books/stock\"\n"));
    assert_has(&go.join("io_books.go"), "\tStock *ddBook0.Book\n");
    for target in ["typescript", "python/generated", "go"] {
        let under = files(&out.join(target).join("flows"));
        assert!(under.iter().all(|f| !f.contains("rulec/") && !f.contains("koyomi/") && !f.contains("chobo/")), "{target}/flows holds code of another language's: {under:?}");
    }
}

/// The rule's and the date's activities of the workflow, run, answer through the package's modules:
/// in Python, called as the worker calls them; in Go, from a test of the workflow's package. And the
/// books the Python transport takes are the package's clients.
#[test]
fn the_activities_of_a_workflow_answer_through_its_package() {
    let t = TempDir::new("gen-run");
    let out = t.path().join("out");
    let (code, _, err) = generate(&here().join(STOCKROOM), &out, &[]);
    assert_eq!(code, 0, "{err}");
    let python = workspace().join("crates/dandori/tools/temporal-python/.venv/bin/python");
    if ready(Need::Python, || python.exists(), "Temporal's Python SDK (crates/dandori/tools/temporal-python) is not installed; the Python activities are not run") {
        let script = "import asyncio, json\n\
                      from generated.flows.order import io, rules\n\
                      from generated.books import stock\n\
                      print(json.dumps([asyncio.run(rules.rule_delivery({'member': False, 'amount': 299})), asyncio.run(rules.rule_delivery({'member': False, 'amount': 300})), asyncio.run(rules.dates_terms_payment({'received': '2026-04-01'})), io.Books.__annotations__['stock'] is stock.Book]))\n";
        let o = Command::new(&python).args(["-B", "-c", script]).current_dir(out.join("python")).output().unwrap();
        assert!(o.status.success(), "the Python activities do not run:\n{}", String::from_utf8_lossy(&o.stderr));
        let got = String::from_utf8_lossy(&o.stdout).trim().to_string();
        assert_eq!(got, r#"[{"carrier": "standard"}, {"carrier": "next_day"}, {"day": "2026-05-08", "at": "2026-05-08T09:00:00Z"}, true]"#);
        println!("the Python activities answer through the package: {got}");
    }
    if ready(Need::Go, have_go, "go is not installed; the Go activities are not run") {
        let Some(gomod) = go_module() else {
            ritsu_testkit::skip("the go.mod of crates/dandori/tools/temporal-go or crates/chobo/tools/runner/go is missing; the Go activities are not run");
            return;
        };
        let module = t.path().join("go");
        go_module_in(&module, &out.join("go"), &gomod);
        let test = format!(
            "package order\n\nimport (\n\t\"context\"\n\t\"encoding/json\"\n\t\"fmt\"\n\t\"testing\"\n\n\tstock \"{MODULE}/books/stock\"\n)\n\n\
             func TestTheActivitiesAnswerThroughThePackage(t *testing.T) {{\n\
             \tvar got []any\n\
             \tfor _, c := range []struct {{\n\t\tname string\n\t\targs map[string]any\n\t}}{{\n\
             \t\t{{\"rule_delivery\", map[string]any{{\"member\": false, \"amount\": 299}}}},\n\
             \t\t{{\"rule_delivery\", map[string]any{{\"member\": false, \"amount\": 300}}}},\n\
             \t\t{{\"dates_terms_payment\", map[string]any{{\"received\": \"2026-04-01\"}}}},\n\
             \t}} {{\n\
             \t\tf := ddRules[c.name].(func(context.Context, map[string]any) (any, error))\n\
             \t\tv, err := f(context.Background(), c.args)\n\
             \t\tif err != nil {{\n\t\t\tt.Fatal(err)\n\t\t}}\n\
             \t\tgot = append(got, v)\n\
             \t}}\n\
             \tvar b Books\n\
             \tb.Stock = (*stock.Book)(nil)\n\
             \tgot = append(got, fmt.Sprintf(\"%T\", b.Stock))\n\
             \ttext, _ := json.Marshal(got)\n\
             \tfmt.Println(string(text))\n\
             }}\n"
        );
        std::fs::write(module.join("generated/flows/order/package_test.go"), test).unwrap();
        let o = go_in(&module).args(["test", "-count=1", "-run", "TestTheActivitiesAnswerThroughThePackage", "-v", "./generated/flows/order/"]).output().unwrap();
        let text = String::from_utf8_lossy(&o.stdout).to_string();
        assert!(o.status.success(), "the Go activities do not run:\n{text}{}", String::from_utf8_lossy(&o.stderr));
        let line = text.lines().find(|l| l.starts_with('[')).unwrap_or_default().to_string();
        assert_eq!(line, r#"[{"carrier":"standard"},{"carrier":"next_day"},{"at":"2026-05-08T09:00:00Z","day":"2026-05-08"},"*stock.Book"]"#);
        println!("the Go activities answer through the package: {line}");
    }
}

// ── --check ──────────────────────────────────────────────────────────────────────────────────────

/// `--check` writes nothing, and says what is stale, missing, or left over from an earlier gen; gen
/// puts it right, removing what it wrote before and writes no more. A change to the project is a
/// change to the packages.
#[test]
fn gen_check_says_what_is_stale() {
    let t = TempDir::new("gen-check");
    let project = t.path().join("stockroom");
    ritsu_testkit::tmp::copy_dir(&here().join(STOCKROOM), &project);
    let out = t.path().join("out");
    let (code, _, err) = generate(&project, &out, &[]);
    assert_eq!(code, 0, "{err}");
    let (code, stdout, _) = generate(&project, &out, &["--check"]);
    assert_eq!((code, stdout.as_str()), (0, ""), "a package just written is not stale");
    // a file edited, one taken away, and one an earlier gen wrote that this one does not
    let rule = out.join("typescript/rules/delivery.ts");
    std::fs::write(&rule, read(&rule).replace("next_day", "next-day")).unwrap();
    std::fs::remove_file(out.join("python/generated/dates/payment_terms.py")).unwrap();
    std::fs::copy(out.join("go/rules/delivery/delivery.go"), out.join("go/rules/delivery/old.go")).unwrap();
    let o = out.display().to_string();
    let (code, stdout, _) = generate(&project, &out, &["--check"]);
    assert_eq!(code, 1);
    assert_eq!(
        stdout,
        format!("missing: {o}/python/generated/dates/payment_terms.py\ngenerated file is stale or hand-edited: {o}/typescript/rules/delivery.ts\ngenerated no more, and still there: {o}/go/rules/delivery/old.go\n")
    );
    let (code, stdout, _) = generate(&project, &out, &[]);
    assert_eq!(code, 0);
    assert_eq!(stdout, format!("generated: {o}/typescript/rules/delivery.ts\ngenerated: {o}/python/generated/dates/payment_terms.py\nremoved, as it is generated no more: {o}/go/rules/delivery/old.go\n"));
    let (code, stdout, _) = generate(&project, &out, &["--check"]);
    assert_eq!((code, stdout.as_str()), (0, ""));
    // the rule changed: its module in every language, and nothing else
    let src = project.join("rules/delivery.rule");
    std::fs::write(&src, read(&src).replace("Written for the example", "Written for the example, again")).unwrap();
    let (code, stdout, _) = generate(&project, &out, &["--check"]);
    assert_eq!(code, 1);
    assert_eq!(
        stdout,
        format!("generated file is stale or hand-edited: {o}/typescript/rules/delivery.ts\ngenerated file is stale or hand-edited: {o}/python/generated/rules/delivery.py\ngenerated file is stale or hand-edited: {o}/go/rules/delivery/delivery.go\n")
    );
    // in Japanese: the prose of the generated code is in the language asked, as `rulec gen`'s is, so a
    // package written in Japanese is checked in Japanese
    let ja = t.path().join("ja");
    let (code, _, err) = generate(&project, &ja, &["--lang", "ja"]);
    assert_eq!(code, 0, "{err}");
    let (code, stdout, _) = generate(&project, &ja, &["--check", "--lang", "ja"]);
    assert_eq!((code, stdout.as_str()), (0, ""));
    let (code, stdout, _) = generate(&project, &ja, &["--check"]);
    assert_eq!(code, 1, "a package written in Japanese is not what the English writes: {stdout}");
    let rule = ja.join("typescript/rules/delivery.ts");
    std::fs::write(&rule, read(&rule).replace("next_day", "next-day")).unwrap();
    std::fs::remove_file(ja.join("python/generated/dates/payment_terms.py")).unwrap();
    std::fs::copy(ja.join("go/rules/delivery/delivery.go"), ja.join("go/rules/delivery/old.go")).unwrap();
    let j = ja.display().to_string();
    let (code, stdout, _) = generate(&project, &ja, &["--check", "--lang", "ja"]);
    assert_eq!(code, 1);
    assert_eq!(
        stdout,
        format!("ありません: {j}/python/generated/dates/payment_terms.py\n生成物が古いか手で編集されています: {j}/typescript/rules/delivery.ts\nもう生成しないファイルが残っています: {j}/go/rules/delivery/old.go\n")
    );
    let (code, stdout, _) = generate(&project, &ja, &["--lang", "ja"]);
    assert_eq!(code, 0);
    assert_eq!(stdout, format!("生成しました: {j}/typescript/rules/delivery.ts\n生成しました: {j}/python/generated/dates/payment_terms.py\nもう生成しないので消しました: {j}/go/rules/delivery/old.go\n"));
}

/// The package.json of the app's form says it is ritsu's at the end of its `description`, JSON
/// having no comment, and `--check` and gen read it so: where a run with `--out` a directory too
/// deep left its package, `--check` counts the package.json with the rest of that package, and the
/// next run takes them all out. A package.json of someone else's stays, and so does one another
/// generator wrote.
#[test]
fn gen_reads_the_apps_package_json_as_ritsus() {
    let t = TempDir::new("gen-package-json");
    let project = t.path().join("stockroom");
    ritsu_testkit::tmp::copy_dir(&here().join(STOCKROOM), &project);
    let out = t.path().join("out");
    let (code, _, err) = generate(&project, &out, &["--target", "typescript"]);
    assert_eq!(code, 0, "{err}");
    // the run with `--out` too deep, and two package.json files of someone else's beside it
    let deep = out.join("typescript/typescript");
    let (code, _, err) = generate(&project, &out.join("typescript"), &["--target", "typescript"]);
    assert_eq!(code, 0, "{err}");
    let ours = read(&deep.join("package.json"));
    let head = format!(" Code generated by ritsu {}. DO NOT EDIT.\",", env!("CARGO_PKG_VERSION"));
    assert!(ours.lines().nth(3).is_some_and(|l| l.starts_with("  \"description\": \"") && l.ends_with(&head)), "{ours}");
    let theirs = out.join("typescript/theirs/package.json");
    let other = out.join("typescript/client/package.json");
    for (p, text) in [
        (&theirs, "{\n  \"name\": \"theirs\",\n  \"private\": true,\n  \"description\": \"The scripts of the stockroom\"\n}\n"),
        (&other, "{\n  \"name\": \"client\",\n  \"private\": true,\n  \"description\": \"The client of the stockroom's API. Code generated by clientgen 1.4.0. DO NOT EDIT.\"\n}\n"),
    ] {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    let left: Vec<String> = files(&deep).iter().map(|f| format!("{}/{f}", deep.display())).collect();
    assert!(left.contains(&format!("{}/package.json", deep.display())), "{left:?}");
    let (code, stdout, _) = generate(&project, &out, &["--target", "typescript", "--check"]);
    assert_eq!(code, 1);
    assert_eq!(stdout, left.iter().map(|p| format!("generated no more, and still there: {p}\n")).collect::<String>());
    let (code, stdout, err) = generate(&project, &out, &["--target", "typescript"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(stdout, left.iter().map(|p| format!("removed, as it is generated no more: {p}\n")).collect::<String>());
    assert!(files(&deep).is_empty() && theirs.exists() && other.exists());
    let (code, stdout, _) = generate(&project, &out, &["--target", "typescript", "--check"]);
    assert_eq!((code, stdout.as_str()), (0, ""));
}

/// `--format json` (DESIGN 9.3): what the lines say, as one object for a program: every file of the
/// packages with what became of it (the ones that stayed the same too), and, when nothing is
/// generated, why, with the exit code and the diagnostics of the files that stopped it. Nothing goes
/// to the standard error, and the exit codes are the text's.
#[test]
fn gen_says_what_it_did_as_json() {
    let t = TempDir::new("gen-json");
    let project = t.path().join("stockroom");
    ritsu_testkit::tmp::copy_dir(&here().join(STOCKROOM), &project);
    let out = t.path().join("out");
    let o = out.display().to_string();
    let json = |more: &[&str]| -> (i32, serde_json::Value) {
        let mut args = vec!["--target", "typescript", "--format", "json"];
        args.extend_from_slice(more);
        let (code, stdout, stderr) = generate(&project, &out, &args);
        assert_eq!(stderr, "", "`ritsu gen {}` writes nothing to the standard error", args.join(" "));
        (code, serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("{e}: {stdout}")))
    };
    // the files that did not stay the same, each with its state, in the order of the JSON
    let moved = |v: &serde_json::Value| -> Vec<(String, String)> {
        v["files"].as_array().unwrap().iter().filter(|f| f["state"] != "same").map(|f| (f["state"].as_str().unwrap().to_string(), f["path"].as_str().unwrap().replace(&o, "<out>"))).collect()
    };
    let (code, v) = json(&[]);
    assert_eq!(code, 0);
    let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["ritsu", "root", "ok", "check", "out", "targets", "files", "diagnostics", "error"], "the keys, in their order");
    assert_eq!((&v["ritsu"], &v["root"], &v["ok"], &v["check"], &v["out"]), (&serde_json::json!(env!("CARGO_PKG_VERSION")), &serde_json::json!("."), &serde_json::json!(true), &serde_json::json!(false), &serde_json::json!(o)));
    assert_eq!((&v["targets"], &v["diagnostics"], &v["error"]), (&serde_json::json!(["typescript"]), &serde_json::json!([]), &serde_json::Value::Null));
    let written = v["files"].as_array().unwrap();
    assert_eq!(written.len(), files(&out.join("typescript")).len(), "every file of the package, once");
    assert!(written.iter().all(|f| f["state"] == "written" && f["target"] == "typescript"));
    let file = |rel: &str| written.iter().find(|f| f["path"] == format!("{o}/typescript/{rel}")).unwrap_or_else(|| panic!("no {rel}"));
    assert_eq!(file("rules/delivery.ts")["from"], "rules/delivery.rule");
    assert_eq!(file("flows/order/workflow.ts")["from"], "orders/order.flow");
    assert_eq!(file("index.ts")["from"], serde_json::Value::Null, "what the package itself is made of comes from no file");
    let (code, v) = json(&["--check"]);
    assert_eq!((code, &v["ok"], &v["check"]), (0, &serde_json::json!(true), &serde_json::json!(true)));
    assert_eq!(moved(&v), Vec::<(String, String)>::new(), "a package just written is the same");
    // a file edited, one taken away, and one an earlier gen wrote that this one does not; a file a
    // person put there, without the head of ritsu's generators, is not ritsu's to count (DESIGN 9.3)
    let rule = out.join("typescript/rules/delivery.ts");
    std::fs::write(&rule, read(&rule).replace("next_day", "next-day")).unwrap();
    std::fs::remove_file(out.join("typescript/dates/payment_terms.ts")).unwrap();
    std::fs::copy(out.join("typescript/rules/delivery.ts"), out.join("typescript/rules/old.ts")).unwrap();
    std::fs::write(out.join("typescript/rules/notes.ts"), "// a person's notes, beside the package\n").unwrap();
    let (code, v) = json(&["--check"]);
    assert_eq!((code, &v["ok"]), (1, &serde_json::json!(false)));
    let state = |s: &str, p: &str| (s.to_string(), format!("<out>/typescript/{p}"));
    assert_eq!(moved(&v), [state("stale", "rules/delivery.ts"), state("missing", "dates/payment_terms.ts"), state("left", "rules/old.ts")]);
    let (code, v) = json(&[]);
    assert_eq!(code, 0);
    assert_eq!(moved(&v), [state("written", "rules/delivery.ts"), state("written", "dates/payment_terms.ts"), state("removed", "rules/old.ts")]);
    assert!(out.join("typescript/rules/notes.ts").is_file(), "a file without the head stays where it is");
    // nothing generated: the arguments, then a file that does not pass its check
    let (code, v) = json(&["--name", "Bad"]);
    assert_eq!((code, &v["ok"], &v["root"], &v["files"]), (2, &serde_json::json!(false), &serde_json::Value::Null, &serde_json::json!([])));
    assert_eq!(v["error"]["code"], 2);
    assert!(v["error"]["message"].as_str().unwrap().starts_with("`--name Bad` cannot name a package"), "{}", v["error"]);
    let src = project.join("rules/delivery.rule");
    std::fs::write(&src, read(&src).replacen("\nenum ", "\nenumx ", 1)).unwrap();
    let (code, v) = json(&[]);
    assert_eq!((code, &v["files"], &v["error"]["code"]), (1, &serde_json::json!([]), &serde_json::json!(1)));
    assert_eq!(v["error"]["message"], "2 file(s) do not pass check, so nothing is generated: rules/delivery.rule, orders/order.flow");
    let tools: Vec<(&str, &str)> = v["diagnostics"].as_array().unwrap().iter().map(|d| (d["tool"].as_str().unwrap(), d["file"].as_str().unwrap())).collect();
    assert_eq!(tools.first(), Some(&("rulec", "rules/delivery.rule")), "rulec's diagnostic first, as `ritsu check --format json` writes it");
    assert!(tools.iter().skip(1).all(|t| *t == ("dandori", "orders/order.flow")) && tools.len() > 1, "{tools:?}");
    assert_eq!(v["diagnostics"][0]["code"], "E005");
    let (_, v) = json(&["--lang", "ja"]);
    assert_eq!(v["error"]["message"], "検査を通らないファイルが 2 個あるので、何も生成しません: rules/delivery.rule, orders/order.flow");
}

// ── the heads ────────────────────────────────────────────────────────────────────────────────────

/// Every file of a package begins with the head of DESIGN 9.2: what wrote it, with ritsu's version,
/// and for a file made from one of the project's, that file by its path from the project's root and
/// the digest of its bytes. The golden is the shape of the three packages: each file, what wrote it,
/// and what it is made from.
#[test]
fn every_file_says_what_wrote_it_and_what_it_is_made_from() {
    let t = TempDir::new("gen-heads");
    let out = t.path().join("out");
    let (code, _, err) = generate(&here().join(STOCKROOM), &out, &[]);
    assert_eq!(code, 0, "{err}");
    golden(here().join("tests/golden/gen/stockroom.txt"), &shape(&out, &here().join(STOCKROOM)));
}

/// The shape of the three packages under `out`: each file, what wrote it (its head, DESIGN 9.2, with
/// ritsu's version), and the file of the project at `project` it is made from, whose digest the head
/// holds.
fn shape(out: &Path, project: &Path) -> String {
    let version = env!("CARGO_PKG_VERSION");
    let mut shape = String::new();
    for target in ["typescript", "python", "go"] {
        shape.push_str(&format!("{target}:\n"));
        for f in files(&out.join(target)) {
            let text = read(&out.join(target).join(&f));
            let mut lines = text.lines();
            let (first, second) = (lines.next().unwrap_or(""), lines.next().unwrap_or(""));
            let by = if f.ends_with("py.typed") {
                "(empty)".to_string()
            } else if f.ends_with("package.json") {
                assert!(text.contains(&format!("Code generated by ritsu {version}. DO NOT EDIT.")), "{f}");
                "ritsu".to_string()
            } else {
                let tool = ["rulec", "koyomi", "chobo", "dandori", "kaname", "sekisho", "ritsu"].into_iter().find(|t| first.ends_with(&format!("Code generated by {t} {version}. DO NOT EDIT."))).unwrap_or_else(|| panic!("{target}/{f} begins otherwise: {first}"));
                tool.to_string()
            };
            let from = match second.split_once("Source: ") {
                Some((_, s)) => {
                    let path = s.split(" (").next().unwrap();
                    let digest = s.rsplit("sha256:").next().unwrap().trim_end_matches(')');
                    let bytes = std::fs::read(project.join(path)).unwrap_or_else(|_| panic!("{target}/{f} names {path}, which is not a file of the project"));
                    assert_eq!(digest, &sha256(&bytes)[..16], "{target}/{f}: the digest of {path}");
                    format!(" <- {path}")
                }
                None => String::new(),
            };
            shape.push_str(&format!("  {f}: {by}{from}\n"));
        }
    }
    shape.replace(version, "<version>")
}

/// The head names the file a part is made from as its path, which may hold a line break: Unix lets
/// a file's name have one. The head writes it as `U+000A` (ritsu-emit's `one_line`), so the rest of the
/// name stays in the comment. Written as it was, the rest of this name was a line of the package's
/// Python, a statement before everything else in the module (DESIGN 9.2).
#[test]
fn a_files_name_stays_in_the_head() {
    let t = TempDir::new("gen-name");
    let project = t.path().join("project");
    ritsu_testkit::tmp::copy_dir(&here().join(STOCKROOM), &project);
    let rule = read(&project.join("rules/delivery.rule")).replace("rule delivery v1", "rule pickup v1");
    std::fs::write(project.join("rules/pickup\nprint('ran') #.rule"), rule).unwrap();
    let out = t.path().join("out");
    let (code, _, err) = generate(&project, &out, &[]);
    assert_eq!(code, 0, "{err}");
    let mut seen = 0;
    for target in ["typescript", "python", "go"] {
        for f in files(&out.join(target)) {
            for line in read(&out.join(target).join(&f)).lines().filter(|l| l.contains("print('ran')")) {
                assert!(line.starts_with("// Source: rules/pickupU+000Aprint('ran') #.rule (") || line.starts_with("# Source: rules/pickupU+000Aprint('ran') #.rule ("), "{target}/{f}: {line}");
                seen += 1;
            }
        }
    }
    assert_eq!(seen, 3, "the head of the rule's module in each language");
}

// ── what it refuses ──────────────────────────────────────────────────────────────────────────────

/// Two files of the project that would write one file of a package (the Go of two flows with
/// Japanese names, both the package `workflow`), a file that does not pass its check, a flow that
/// reads a file the project does not hold, a module named by a word the target keeps, and a name no
/// package can take: each says so and writes nothing.
#[test]
fn what_cannot_be_a_package_is_refused() {
    let t = TempDir::new("gen-refused");
    let out = t.path().join("out");
    let (code, stdout, err) = generate(&here().join(SHOP), &out, &["--target", "go"]);
    assert_eq!((code, stdout.as_str()), (2, ""));
    assert_eq!(
        err,
        "error: `delivery/配送の手配.flow` and `ordering/受注.flow` both write go/flows/workflow/doc.go of the package; give one of them another name (a rule or a dates file another alias, a book another name, a workflow a name or a file name in ASCII)\n"
    );
    let (code, _, err) = generate(&here().join(SHOP), &out, &["--target", "go", "--lang", "ja"]);
    assert_eq!(code, 2);
    assert_eq!(
        err,
        "エラー: `delivery/配送の手配.flow` と `ordering/受注.flow` が、パッケージの同じ go/flows/workflow/doc.go を書きます。どちらかの名前を変えてください（規則と日付のファイルは別名、帳簿は名前、ワークフローは名前かファイルの名前を ASCII で）\n"
    );
    assert!(!out.exists(), "nothing is written when a package is refused");
    // a rule that does not pass its check
    let project = t.path().join("broken");
    ritsu_testkit::tmp::copy_dir(&here().join(STOCKROOM), &project);
    let rule = project.join("rules/delivery.rule");
    std::fs::write(&rule, read(&rule).replace("| false  | <300GBP  | standard   |\n", "")).unwrap();
    let (code, stdout, err) = generate(&project, &out, &[]);
    assert_eq!((code, stdout.as_str()), (1, ""));
    assert!(err.contains("[rulec E"), "the rule's diagnostics, with the tool's word: {err}");
    // and the flow that reads it
    assert!(err.contains("[dandori E005]"), "{err}");
    assert!(err.ends_with("error: 2 file(s) do not pass check, so nothing is generated: rules/delivery.rule, orders/order.flow\n"), "{err}");
    assert!(!out.exists());
    // the flow alone: what it reads is not given
    let (code, _, err) = ritsu_in(&here().join(STOCKROOM), &["gen", "--root", ".", "orders/order.flow", "--out", out.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(err.starts_with("error: `orders/order.flow` reads "), "{err}");
    assert!(err.ends_with(", which is not one of the project's files; a package is made of the project's files, so give ritsu gen that file too\n"), "{err}");
    assert!(!out.exists());
    // a module of kaname alone: the rule and the book it reads are not given
    let (code, _, err) = ritsu_in(&here().join(SETTLEMENT), &["gen", "--root", ".", "payout.kn", "--out", out.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert_eq!(err, "error: `payout.kn` reads rules/card_fee.rule, which is not one of the project's files; a package is made of the project's files, so give ritsu gen that file too\n");
    assert!(!out.exists());
    // a rule whose alias is a word Python keeps: rulec only warns of it (W121), and the package's
    // Python cannot import it
    let kept = t.path().join("kept");
    ritsu_testkit::tmp::copy_dir(&here().join(STOCKROOM), &kept);
    let rule = kept.join("rules/delivery.rule");
    std::fs::write(&rule, read(&rule).replace("rule delivery v1", "rule pass v1")).unwrap();
    let (code, _, err) = generate(&kept, &out, &["--target", "python"]);
    assert_eq!(code, 2);
    assert_eq!(err, "error: `rules/delivery.rule` makes the module `pass`, a word Python keeps, which the package cannot hold in its rules/; give it another name (an alias)\n");
    let (code, _, err) = generate(&kept, &out, &["--target", "typescript"]);
    assert_eq!(code, 0, "TypeScript takes the name: {err}");
    std::fs::remove_dir_all(&out).unwrap();
    let (code, _, err) = generate(&here().join(STOCKROOM), &out, &["--name", "Stock-Room"]);
    assert_eq!(code, 2);
    assert_eq!(err, "error: `--name Stock-Room` cannot name a package: it starts with a lowercase letter and has only lowercase letters, digits and `_` (and is not a word Python keeps)\n");
}

/// The package of a project whose rules are added to by contributions (rulec's §15.231): each rule's
/// module is the rule merged with the project's contributions that reach it, its header names them,
/// and a contribution has no module of its own. The new kind is a member of the enum's module, and
/// the price module answers for it.
#[test]
fn a_rules_module_is_the_rule_merged_with_the_projects_contributions() {
    let t = TempDir::new("gen-product-line");
    let out = t.path().join("product_line");
    let (code, _, err) = generate(&here().join("tests/projects/product_line"), &out, &[]);
    assert_eq!(code, 0, "ritsu gen of the product line:\n{err}");
    let rules: Vec<String> = files(&out).into_iter().filter(|f| f.starts_with("python/generated/rules/")).collect();
    assert_eq!(rules, ["python/generated/rules/__init__.py", "python/generated/rules/product_kinds.py", "python/generated/rules/product_price.py"]);
    let price = read(&out.join("python/generated/rules/product_price.py"));
    assert!(price.lines().any(|l| l.starts_with("# Contributions: garden_price = contrib/garden_price.rule (signature:")), "{price}");
    let kinds = read(&out.join("python/generated/rules/product_kinds.py"));
    assert!(kinds.lines().any(|l| l.starts_with("# Contributions: garden_kind = ")) && kinds.contains("GARDEN = \"garden\""), "{kinds}");
    if ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not installed; the app over the package is not run") {
        let app = out.join("python/app.py");
        std::fs::write(
            &app,
            "from generated.rules import product_kinds, product_price\n\n\
             print(product_price.product_price(product_kinds.Kind.GARDEN, product_price.USD(20)), product_kinds.kind_label(product_kinds.Kind.GARDEN))\n",
        )
        .unwrap();
        let o = Command::new("python3").arg("app.py").current_dir(out.join("python")).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "23 Garden supplies");
    }
}

// ── the functional cores (kaname's modules) ──────────────────────────────────────────────────────

/// A project with modules of kaname (kaname's examples, its DESIGN 17.6 and 17.7): `payout`
/// builds the two transfers of a ledger with the fee a card's rule gives, and `late_payment` calls a
/// dates file and a rule of late charges.
const SETTLEMENT: &str = "tests/projects/settlement";

/// The modules of kaname go into each package under glue/, beside the rules, the dates and the books
/// they call, with kaname's helper and the types of the transfers the modules build (kaname's DESIGN
/// 10): every file says what wrote it and what it is made from, the package's index and description
/// say glue/ and call the modules the functional cores, and the packages pass `tsc --strict`,
/// `mypy --strict`, `go vet` and gofmt.
#[test]
fn the_modules_of_kaname_go_into_the_package() {
    let t = TempDir::new("gen-kaname");
    let out = t.path().join("settlement");
    let (code, _, err) = generate(&here().join(SETTLEMENT), &out, &[]);
    assert_eq!(code, 0, "ritsu gen of the settlement:\n{err}");
    golden(here().join("tests/golden/gen/settlement.txt"), &shape(&out, &here().join(SETTLEMENT)));
    let index = read(&out.join("typescript/index.ts"));
    assert!(index.contains("export * as glue from \"./glue/index\";") && index.contains("the workflows and the functional cores (kaname's modules) of a project, as one package"), "{index}");
    assert!(read(&out.join("python/generated/__init__.py")).contains("from . import glue as glue"));
    // the comment of doc.go, its lines joined
    let doc = read(&out.join("go/doc.go")).lines().map(|l| l.trim_start_matches("//").trim()).collect::<Vec<_>>().join(" ");
    assert!(doc.contains("the workflows and the functional cores (kaname's modules) of a project") && doc.contains("rules/, dates/, books/, flows/ and glue/ under the import path"), "{doc}");
    if ritsu_testkit::need(Need::Node) {
        match tsc() {
            Some((tsc, modules)) => {
                type_check_typescript(&out.join("typescript"), &tsc, &modules);
                println!("tsc --strict passes the TypeScript of the package");
            }
            None => ritsu_testkit::skip("TypeScript (crates/dandori/tools/temporal) or tigerbeetle-node (crates/chobo/tools/runner) is not installed; the TypeScript of the package is not type-checked"),
        }
    }
    if ritsu_testkit::need(Need::Python) {
        match mypy() {
            Some((mypy, python, _)) => {
                type_check_python(&out.join("python"), &mypy, &python, &[]);
                println!("mypy --strict passes the Python of the package");
            }
            None => ritsu_testkit::skip("mypy, Temporal's Python SDK (crates/dandori/tools/temporal-python) or tigerbeetle (crates/chobo/tools/runner) is not installed; the Python of the package is not type-checked"),
        }
    }
    if ready(Need::Go, have_go, "go is not installed; the Go of the package is not vetted") {
        match go_module() {
            Some(gomod) => {
                type_check_go(&t.path().join("go-settlement"), &out.join("go"), &gomod);
                println!("go vet and gofmt pass the Go of the package");
            }
            None => ritsu_testkit::skip("the go.mod of crates/dandori/tools/temporal-go or crates/chobo/tools/runner/go is missing; the Go of the package is not vetted"),
        }
    }
}

/// `--check` says nothing of a package with modules of kaname just written; once a module changes, it
/// names that module's code in each language, and nothing else.
#[test]
fn gen_check_says_a_module_of_kaname_is_stale() {
    let t = TempDir::new("gen-check-kaname");
    let project = t.path().join("settlement");
    ritsu_testkit::tmp::copy_dir(&here().join(SETTLEMENT), &project);
    let out = t.path().join("out");
    let (code, _, err) = generate(&project, &out, &[]);
    assert_eq!(code, 0, "{err}");
    let (code, stdout, _) = generate(&project, &out, &["--check"]);
    assert_eq!((code, stdout.as_str()), (0, ""), "a package just written is not stale");
    let kn = project.join("payout.kn");
    std::fs::write(&kn, read(&kn).replace("which the host makes", "which the host applies")).unwrap();
    let o = out.display().to_string();
    let (code, stdout, _) = generate(&project, &out, &["--check"]);
    assert_eq!(code, 1);
    assert_eq!(
        stdout,
        format!("generated file is stale or hand-edited: {o}/typescript/glue/payout.ts\ngenerated file is stale or hand-edited: {o}/python/generated/glue/payout.py\ngenerated file is stale or hand-edited: {o}/go/glue/payout/payout.go\n")
    );
    let (code, _, _) = generate(&project, &out, &[]);
    assert_eq!(code, 0);
    let (code, stdout, _) = generate(&project, &out, &["--check"]);
    assert_eq!((code, stdout.as_str()), (0, ""));
}

/// The modules of the settlement, checked as `ritsu gen` checks them: with what the checks across the
/// borders came to at their calls (`with`), or without (as kaname's own tests check them).
fn settlement_modules(with: bool) -> Vec<std::rc::Rc<kaname::check::Checked>> {
    let joined = ritsu_project::Joined::new();
    let mut suite: kaname::suite::Suite = joined.ports().into();
    if with {
        let v: std::rc::Rc<dyn ritsu_ports::Verdicts> = std::rc::Rc::new(ritsu_cross::ProgramVerdicts::new(&joined));
        suite = suite.with_verdicts(v);
    }
    let run = kaname::check::Run::new(suite);
    ["payout.kn", "late_payment.kn"]
        .iter()
        .map(|f| {
            let path = format!("{SETTLEMENT}/{f}");
            run.file(&path).unwrap_or_else(|e| panic!("{path}: {e}")).checked.clone().unwrap_or_else(|| panic!("{path} does not pass"))
        })
        .collect()
}

/// The places of the modules the generated code checks when it runs (kaname's DESIGN 10.4): those the
/// analysis does not show, and of them, the ones the checks across the borders decide.
fn checked_places(modules: &[std::rc::Rc<kaname::check::Checked>]) -> (usize, usize) {
    let places = modules.iter().flat_map(|c| c.analysis.get().map(|a| a.fns.iter().flat_map(|f| f.places.iter().filter(|p| p.failing())).cloned().collect::<Vec<_>>()).unwrap_or_default());
    places.fold((0, 0), |(all, across), p| (all + 1, across + usize::from(p.what.across().is_some())))
}

/// The code `ritsu gen` writes for the modules of kaname is made with what the checks across the
/// borders came to at their calls, as `ritsu check` decides them (kaname's DESIGN 10.4, 14.5): a
/// place they show is not checked again when the code runs, where the code made without them (as
/// kaname's own tests make it) checks it. The code `ritsu gen` writes answers every vector of every
/// function as the reference interpreter does, in each language, and each input outside what the
/// parameters declare: the value, or the kind, the module, the line and the message of the failure.
#[test]
fn the_code_of_the_modules_answers_as_the_reference_does() {
    let t = TempDir::new("gen-kaname-vectors");
    let out = t.path().join("settlement");
    let (code, _, err) = generate(&here().join(SETTLEMENT), &out, &[]);
    assert_eq!(code, 0, "{err}");
    let modules = settlement_modules(true);
    let (with, with_across) = checked_places(&modules);
    let (without, without_across) = checked_places(&settlement_modules(false));
    assert!(with < without && with_across == 0 && without_across > 0, "with the verdicts {with} places ({with_across} across), without {without} ({without_across} across)");
    println!("the code checks {with} places with the verdicts of the checks across the borders, {without} without them ({without_across} of them across the borders)");
    the_modules_answer_as_the_reference_does(&t, &out, &modules, "settlement");
}

/// The code of `modules`, in the package at `out` (`name` names the module of Go made under `t`),
/// answers every vector of every function as the reference interpreter does, in each language, and
/// each input outside what the parameters declare.
fn the_modules_answer_as_the_reference_does(t: &TempDir, out: &Path, modules: &[std::rc::Rc<kaname::check::Checked>], name: &str) {
    let suite: kaname::suite::Suite = ritsu_project::Joined::new().ports().into();
    let (mut cases, diverged) = kaname::r#gen::harness::cases(modules, &suite, ritsu_base::text::Lang::En);
    let more = kaname::r#gen::harness::outside_cases(modules, &suite, ritsu_base::text::Lang::En);
    let (vectors, outside) = (cases.len() - diverged, more.len());
    cases.extend(more);
    let run = cases.len() - diverged;
    let json = kaname::r#gen::harness::cases_json(&cases);
    let held = |target: &str, stdout: &str| {
        let (agree, wrong) = kaname::r#gen::harness::compare(&cases, stdout);
        assert!(wrong.is_empty(), "the {target} answers otherwise than the reference:\n{}", wrong.join("\n"));
        println!("{target}: {agree} of {run} cases agree with the reference: {vectors} vectors and {outside} inputs outside what the parameters declare");
    };
    let program = |target: kaname::r#gen::Target| kaname::r#gen::harness::program(modules, target, "generated", MODULE);
    let tools = workspace().join("crates/kaname/tools");
    let tsc = tools.join("node_modules/.bin/tsc");
    if ritsu_testkit::need(Need::Node) {
        if tsc.exists() {
            let dir = out.join("typescript");
            std::fs::write(dir.join("harness.ts"), program(kaname::r#gen::Target::TypeScript)).unwrap();
            std::fs::write(dir.join("cases.json"), &json).unwrap();
            let tsconfig = serde_json::json!({
                "compilerOptions": {
                    "strict": true, "erasableSyntaxOnly": true, "module": "commonjs", "target": "es2022", "rootDir": ".", "outDir": "dist",
                    "skipLibCheck": true, "types": ["node"], "typeRoots": [tools.join("node_modules/@types")]
                },
                "include": ["**/*.ts"],
                "exclude": ["dist", "node_modules"]
            });
            std::fs::write(dir.join("tsconfig.json"), serde_json::to_string_pretty(&tsconfig).unwrap()).unwrap();
            let o = Command::new(&tsc).arg("-p").arg("tsconfig.json").current_dir(&dir).output().unwrap();
            assert!(o.status.success(), "tsc:\n{}", said(&o));
            let o = Command::new("node").arg("dist/harness.js").arg("cases.json").current_dir(&dir).output().unwrap();
            assert!(o.status.success(), "node:\n{}", said(&o));
            held("TypeScript", &String::from_utf8_lossy(&o.stdout));
        } else {
            ritsu_testkit::skip("tsc is not installed in crates/kaname/tools (npm ci --prefix crates/kaname/tools); the TypeScript of the modules is not run");
        }
    }
    // a package with gates imports cedarpy where it is imported, which sekisho's runner of Python has
    let python = if out.join("python/generated/authz").is_dir() { workspace().join("crates/sekisho/tools/runner-py/.venv/bin/python") } else { PathBuf::from("python3") };
    if ready(Need::Python, || Command::new(&python).arg("--version").output().is_ok_and(|o| o.status.success()), &format!("{} is not installed; the Python of the modules is not run", python.display())) {
        let dir = out.join("python");
        std::fs::write(dir.join("harness.py"), program(kaname::r#gen::Target::Python)).unwrap();
        std::fs::write(dir.join("cases.json"), &json).unwrap();
        let o = Command::new(&python).arg("harness.py").arg("cases.json").current_dir(&dir).output().unwrap();
        assert!(o.status.success(), "python3:\n{}", said(&o));
        held("Python", &String::from_utf8_lossy(&o.stdout));
    }
    if ready(Need::Go, have_go, "go is not installed; the Go of the modules is not run") {
        match go_module() {
            Some(gomod) => {
                let dir = t.path().join(format!("go-{name}"));
                go_module_in(&dir, &out.join("go"), &gomod);
                std::fs::create_dir_all(dir.join("harness")).unwrap();
                std::fs::write(dir.join("harness/main.go"), program(kaname::r#gen::Target::Go)).unwrap();
                std::fs::write(dir.join("cases.json"), &json).unwrap();
                let bin = dir.join("harness.bin");
                let o = go_in(&dir).args(["build", "-o"]).arg(&bin).arg("./harness").output().unwrap();
                assert!(o.status.success(), "go build:\n{}", said(&o));
                let o = Command::new(&bin).arg("cases.json").current_dir(&dir).output().unwrap();
                assert!(o.status.success(), "the harness:\n{}", said(&o));
                held("Go", &String::from_utf8_lossy(&o.stdout));
            }
            None => ritsu_testkit::skip("the go.mod of crates/dandori/tools/temporal-go or crates/chobo/tools/runner/go is missing; the Go of the modules is not run"),
        }
    }
}

// ── the units written with a sign ────────────────────────────────────────────────────────────────

/// The project of the cold chain (DESIGN 5.1): a rule that takes the temperature of a hold in ℃ and
/// its humidity in steps of 1%, and a gate, a flow and a module of kaname that give it them, each in
/// English and in Japanese (tests/cross.rs holds what `ritsu check` says of it).
const COLD_CHAIN: &str = "tests/projects/cold_chain";

/// The project's two modules of kaname, checked with the verdicts of the checks across the borders,
/// as `ritsu gen` makes their code.
fn cold_chain_modules() -> Vec<std::rc::Rc<kaname::check::Checked>> {
    let joined = ritsu_project::Joined::new();
    let v: std::rc::Rc<dyn ritsu_ports::Verdicts> = std::rc::Rc::new(ritsu_cross::ProgramVerdicts::new(&joined));
    let suite: kaname::suite::Suite = kaname::suite::Suite::from(joined.ports()).with_verdicts(v);
    let run = kaname::check::Run::new(suite);
    ["departure.kn", "departure.ja.kn"]
        .iter()
        .map(|f| {
            let path = format!("{COLD_CHAIN}/{f}");
            run.file(&path).unwrap_or_else(|e| panic!("{path}: {e}")).checked.clone().unwrap_or_else(|| panic!("{path} does not pass"))
        })
        .collect()
}

/// A unit written with a sign goes into the package as any unit does (DESIGN 5.1, 9.3): ℃ is the
/// type `Celsius` of the package's units, which the rules take, the modules of kaname give and the
/// gates' code hands the rule. The packages pass tsc --strict and mypy --strict whole, and go vet the
/// gates with the Cedar of sekisho's runner and the flows with Temporal's SDK; the code of the
/// modules answers every vector as the reference interpreter does.
#[test]
fn the_units_written_with_a_sign_go_into_the_package() {
    let t = TempDir::new("gen-signs");
    let out = t.path().join("cold_chain");
    let (code, _, err) = generate(&here().join(COLD_CHAIN), &out, &[]);
    assert_eq!(code, 0, "ritsu gen of the cold chain:\n{err}");
    assert_has(&out.join("typescript/units.ts"), "export type Celsius = bigint & { readonly __rulec: \"Celsius\" }; // temperature[℃]\n");
    assert_has(&out.join("python/generated/units.py"), "Celsius = NewType(\"Celsius\", int)  # temperature[℃]\n");
    assert_has(&out.join("go/units/units.go"), "// Celsius is an amount of temperature[℃].\ntype Celsius int64\n");
    assert_has(&out.join("typescript/rules/cold_chain_departure.ts"), "export function cold_chain_departure(temperature_class: TemperatureClass, temperature: Celsius, humidity: Rate): boolean {");
    assert_has(&out.join("typescript/glue/departure.ts"), "readonly temperature: units.Celsius;");
    assert_has(&out.join("typescript/authz/cold_chain.ts"), "r_Shipment.temperature as rule_cold_chain_departure.Celsius");
    type_check_the_gates(&t, &out);
    if ready(Need::Go, have_go, "go is not installed; the Go of the flows is not vetted") {
        match go_module() {
            Some(gomod) => {
                // the flows, the rules and the modules, without the gates, whose Cedar this module does not require
                let dir = t.path().join("go-flows");
                go_module_in(&dir, &out.join("go"), &gomod);
                std::fs::remove_dir_all(dir.join("generated/authz")).unwrap();
                let o = go_in(&dir).args(["vet", "./..."]).output().unwrap();
                assert!(o.status.success(), "go vet finds fault with the Go of the flows:\n{}", said(&o));
                println!("go vet passes the Go of the flows, with the rules and the modules");
            }
            None => ritsu_testkit::skip("the go.mod of crates/dandori/tools/temporal-go or crates/chobo/tools/runner/go is missing; the Go of the flows is not vetted"),
        }
    }
    the_modules_answer_as_the_reference_does(&t, &out, &cold_chain_modules(), "cold_chain");
}
