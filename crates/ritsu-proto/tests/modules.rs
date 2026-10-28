//! Modules of ritsu's (ritsu's DESIGN 17.6) and the imports of `.proto` files. An import is looked for
//! in the file's own module first. A file in a dependency (`@lib/…`, `@lib/@base/…` from the root of
//! the module at hand) reads its imports from the root of its own module, as protoc reads them with
//! `-I` at that root: the roots of the module that uses it are not the dependency's, so a file of the
//! module at hand at the same path is not read in its place. What the file's own module does not
//! have is looked for in the module's dependencies, the canonical order, each dependency's root and
//! the roots its files' packages make (`@lib/proto`): the module at hand imports lib's line by the path
//! of its package, and lib's line base's category. An import is never written with `@`: protoc does
//! not see the dependencies by their names, so in a module such an import is not read, and is told as
//! not found. Outside every module, a directory whose name starts with `@` is a directory, an import
//! reads it as it always did, and no import is looked for in it (M14).
//!
//! One run of protoc reads an import from the first root that has it, for every file at once
//! (`protoc_roots`): the roots are the ones ritsu reads each file's imports from, in the order it
//! first reads from them, and when the module's own file and a dependency's are at one path, the
//! files that read them cannot be given to one run, unless the two have the same text, which one
//! run reads as one file. Nor can a file be given that two files reach by two names (lib's money,
//! by the path of its package and from lib's root): protoc reads it twice, and declares its types
//! twice. ritsu's tests of `ritsu gen` give protoc these roots (the tools level,
//! `crates/ritsu/tests/modules_gen.rs`).
//!
//! The material is `tests/modules/`, in English (`en/`) and in Japanese (`ja/`, whose directories are
//! named in Japanese and whose `.proto` files, which name their types in ASCII, say what they are in
//! Japanese): the module at hand (`app`, with an invoice, a charge whose import is written with `@`,
//! a money of its own at the path lib's money is at in lib, a receipt of lib's lines, a refund of
//! its own money and a statement of lib's total), its path dependency (`lib`: an order, its money, a
//! line of base's category and a total of lib's money), the dependency's dependency (`base`: a rate
//! and its category, which app reaches as `@lib/@base/…`) and a directory outside every module
//! (`loose`, with a directory named `@lib`).

use ritsu_proto::{Clash, Issue, Protos, dependency_roots, load, load_from, protoc_roots, roots_for};
use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};

/// The directories of one language's material.
struct Material {
    app: PathBuf,
    lib: PathBuf,
    base: PathBuf,
    loose: PathBuf,
}

fn material(lang: &str, app: &str, lib: &str, base: &str, loose: &str) -> Material {
    let at = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/modules").join(lang);
    Material { app: at.join(app), lib: at.join(lib), base: at.join(base), loose: at.join(loose) }
}

fn english() -> Material {
    material("en", "app", "lib", "base", "loose")
}

fn japanese() -> Material {
    material("ja", "アプリ", "部品", "土台", "外")
}

const ORDER: &str = "@lib/proto/acme/orders/v1/order.proto";
const MONEY: &str = "@lib/proto/acme/orders/v1/money.proto";
const RATE: &str = "@lib/@base/proto/acme/tax/v1/rate.proto";
const CATEGORY: &str = "@lib/@base/proto/acme/tax/v1/category.proto";
const OWN_MONEY: &str = "proto/acme/orders/v1/money.proto";
const INVOICE: &str = "proto/acme/billing/v1/invoice.proto";
const CHARGE: &str = "proto/acme/billing/v1/charge.proto";
const LINE: &str = "@lib/proto/acme/orders/v1/line.proto";
const TOTAL: &str = "@lib/proto/acme/orders/v1/total.proto";
const RECEIPT: &str = "proto/acme/billing/v1/receipt.proto";
const REFUND: &str = "proto/acme/billing/v1/refund.proto";
const STATEMENT: &str = "proto/acme/billing/v1/statement.proto";

/// What each import of `file` came to.
fn imports_of(ps: &Protos, file: &str) -> Vec<Option<String>> {
    ps.imports.get(file).cloned().unwrap_or_else(|| panic!("{file} was not read: {:?}", ps.order))
}

/// The fields of the message `Money` as `file` reads it.
fn money_fields(ps: &Protos, file: &str) -> Vec<String> {
    ps.files[file].message("Money").unwrap().fields.iter().map(|f| f.name.clone()).collect()
}

/// The imports told as not found: the file and the import as written.
fn not_found(issues: &[Issue]) -> Vec<(String, String)> {
    issues
        .iter()
        .map(|i| match i {
            Issue::NotFound { file, import, .. } => (file.clone(), import.path.clone()),
            other => panic!("an issue other than an import not found: {other:?}"),
        })
        .collect()
}

fn a_dependency_reads_its_imports_from_its_own_root(m: &Material) {
    // the roots given are the module at hand's (protoc's `-I .` at app's root): lib's order reads
    // lib's money, not app's at the same path
    for roots in [vec![".".to_string()], vec![]] {
        let (ps, issues) = load_from(&m.app, ORDER, &roots, &[]).unwrap();
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(imports_of(&ps, ORDER), vec![Some(MONEY.to_string())]);
        assert_eq!(ps.order, vec![ORDER.to_string(), MONEY.to_string()]);
        assert_eq!(money_fields(&ps, MONEY), vec!["pence"]);
        assert!(!ps.files.contains_key(OWN_MONEY));
    }
    // the dependency's dependency, two `@` down
    let (ps, issues) = load_from(&m.app, RATE, &[".".to_string()], &[]).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(imports_of(&ps, RATE), vec![Some(CATEGORY.to_string())]);
    // the roots of a file in a dependency are the dependency's root
    assert_eq!(roots_for(&m.app, ORDER, &[".".to_string()]), vec!["@lib"]);
    assert_eq!(roots_for(&m.app, RATE, &[".".to_string()]), vec!["@lib/@base"]);
    assert_eq!(roots_for(&m.app, INVOICE, &[".".to_string()]), vec!["."]);
    // a set of files read together: each reads its own module's money
    let files = [INVOICE, OWN_MONEY, ORDER, MONEY].map(String::from);
    let (ps, issues) = load(&m.app, &files, &[".".to_string()]);
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(imports_of(&ps, INVOICE), vec![Some(OWN_MONEY.to_string())]);
    assert_eq!(imports_of(&ps, ORDER), vec![Some(MONEY.to_string())]);
    assert_eq!((money_fields(&ps, OWN_MONEY), money_fields(&ps, MONEY)), (vec!["units".to_string(), "currency".to_string()], vec!["pence".to_string()]));
    // lib reads its order the same way in lib itself
    let (ps, issues) = load_from(&m.lib, "proto/acme/orders/v1/order.proto", &[".".to_string()], &[]).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(imports_of(&ps, "proto/acme/orders/v1/order.proto"), vec![Some("proto/acme/orders/v1/money.proto".to_string())]);
    assert_eq!(money_fields(&ps, "proto/acme/orders/v1/money.proto"), vec!["pence"]);
}

fn an_import_written_with_at_is_not_read_in_a_module(m: &Material) {
    let (ps, issues) = load_from(&m.app, CHARGE, &[".".to_string()], &[]).unwrap();
    assert_eq!(not_found(&issues), vec![(CHARGE.to_string(), ORDER.to_string())]);
    assert_eq!(imports_of(&ps, CHARGE), vec![None]);
    assert!(ps.unread.contains(CHARGE));
    assert_eq!(ps.order, vec![CHARGE.to_string()]);
    // not even when lib's order is among the files read
    let files = [CHARGE, ORDER, MONEY].map(String::from);
    let (ps, issues) = load(&m.app, &files, &[".".to_string()]);
    assert_eq!(not_found(&issues), vec![(CHARGE.to_string(), ORDER.to_string())]);
    assert_eq!(imports_of(&ps, CHARGE), vec![None]);
    assert_eq!(imports_of(&ps, ORDER), vec![Some(MONEY.to_string())]);
}

fn outside_every_module_a_directory_named_with_at_is_a_directory(m: &Material) {
    let (ps, issues) = load_from(&m.loose, CHARGE, &[".".to_string()], &[]).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(imports_of(&ps, CHARGE), vec![Some(ORDER.to_string())]);
    assert_eq!(roots_for(&m.loose, ORDER, &[".".to_string()]), vec!["."]);
    let files = [CHARGE, ORDER].map(String::from);
    let (ps, issues) = load(&m.loose, &files, &[".".to_string()]);
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(imports_of(&ps, CHARGE), vec![Some(ORDER.to_string())]);
}

fn a_file_imports_a_dependency_s_file_by_the_path_of_its_package(m: &Material) {
    for roots in [vec![], vec![".".to_string()]] {
        let (ps, issues) = load_from(&m.app, RECEIPT, &roots, &[]).unwrap();
        assert!(issues.is_empty(), "{issues:?}");
        // app has no line: lib's, under lib's `proto/`; and lib has no category: base's, under base's
        assert_eq!(imports_of(&ps, RECEIPT), vec![Some(LINE.to_string())]);
        assert_eq!(imports_of(&ps, LINE), vec![Some(CATEGORY.to_string())]);
        assert_eq!(ps.order, vec![RECEIPT.to_string(), LINE.to_string(), CATEGORY.to_string()]);
    }
    // read together with the dependencies' files, as a check of the project reads them
    let files = [RECEIPT, LINE, CATEGORY].map(String::from);
    let (ps, issues) = load(&m.app, &files, &[".".to_string()]);
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(imports_of(&ps, RECEIPT), vec![Some(LINE.to_string())]);
    assert_eq!(imports_of(&ps, LINE), vec![Some(CATEGORY.to_string())]);
    // the roots after the module's own: each dependency's root and the root its packages make, the
    // canonical order; a file of lib's has lib's dependencies
    let at = |p: &str| m.app.join(p);
    assert_eq!(dependency_roots(&m.app.join("proto/acme/billing/v1")), vec![at("@lib"), at("@lib/proto"), at("@lib/@base"), at("@lib/@base/proto")]);
    assert_eq!(dependency_roots(&m.app.join("@lib/proto/acme/orders/v1")), vec![at("@lib/@base"), at("@lib/@base/proto")]);
    // lib reads its line the same way in lib itself
    let (ps, issues) = load_from(&m.lib, "proto/acme/orders/v1/line.proto", &[], &[]).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(imports_of(&ps, "proto/acme/orders/v1/line.proto"), vec![Some("@base/proto/acme/tax/v1/category.proto".to_string())]);
    // one run of protoc: the root of the receipt's package, then the roots its imports were found
    // under, each the directory its files are in on the disk
    let p = protoc_roots(&m.app, RECEIPT, &[]).unwrap();
    assert_eq!(p.entry, "acme/billing/v1/receipt.proto");
    assert_eq!(real(&p.includes), real(&[m.app.join("proto"), m.lib.join("proto"), m.base.join("proto")]));
}

fn the_module_s_own_file_at_the_same_path_is_read_first(m: &Material) {
    let (ps, issues) = load_from(&m.app, REFUND, &[], &[]).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(imports_of(&ps, REFUND), vec![Some(OWN_MONEY.to_string())]);
    assert_eq!(money_fields(&ps, OWN_MONEY), vec!["units", "currency"]);
    // lib's total reads lib's money, which app's statement reaches: two copies, each file holding the
    // money of its own module
    let (ps, issues) = load_from(&m.app, STATEMENT, &[], &[]).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(imports_of(&ps, STATEMENT), vec![Some(TOTAL.to_string())]);
    assert_eq!(imports_of(&ps, TOTAL), vec![Some(MONEY.to_string())]);
    assert_eq!(money_fields(&ps, MONEY), vec!["pence"]);
    let files = [REFUND, STATEMENT, OWN_MONEY, TOTAL, MONEY].map(String::from);
    let (ps, issues) = load(&m.app, &files, &[".".to_string()]);
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(imports_of(&ps, REFUND), vec![Some(OWN_MONEY.to_string())]);
    assert_eq!(imports_of(&ps, TOTAL), vec![Some(MONEY.to_string())]);
    // one run of protoc reads app's money for lib's total too: the statement cannot be given to it
    assert_eq!(
        protoc_roots(&m.app, STATEMENT, &[]),
        Err(Clash::TwoFiles { import: "acme/orders/v1/money.proto".to_string(), read: OWN_MONEY.to_string(), meant: MONEY.to_string(), by: TOTAL.to_string() })
    );
    // the refund alone can
    let p = protoc_roots(&m.app, REFUND, &[]).unwrap();
    assert_eq!((p.entry.as_str(), real(&p.includes)), ("acme/billing/v1/refund.proto", real(&[m.app.join("proto")])));
}

fn outside_every_module_no_dependency_is_looked_in(m: &Material) {
    let (ps, issues) = load_from(&m.loose, RECEIPT, &[".".to_string()], &[]).unwrap();
    assert_eq!(not_found(&issues), vec![(RECEIPT.to_string(), "acme/orders/v1/line.proto".to_string())]);
    assert_eq!(imports_of(&ps, RECEIPT), vec![None]);
    assert!(dependency_roots(&m.loose.join("proto/acme/billing/v1")).is_empty());
}

/// The directories, each as the disk names it.
fn real(dirs: &[PathBuf]) -> Vec<PathBuf> {
    dirs.iter().map(|d| std::fs::canonicalize(d).unwrap_or_else(|e| panic!("{}: {e}", d.display()))).collect()
}

#[test]
fn a_file_imports_a_dependency_s_file_by_the_path_of_its_package_in_english() {
    a_file_imports_a_dependency_s_file_by_the_path_of_its_package(&english());
}

#[test]
fn a_file_imports_a_dependency_s_file_by_the_path_of_its_package_in_japanese() {
    a_file_imports_a_dependency_s_file_by_the_path_of_its_package(&japanese());
}

#[test]
fn the_module_s_own_file_at_the_same_path_is_read_first_in_english() {
    the_module_s_own_file_at_the_same_path_is_read_first(&english());
}

#[test]
fn the_module_s_own_file_at_the_same_path_is_read_first_in_japanese() {
    the_module_s_own_file_at_the_same_path_is_read_first(&japanese());
}

#[test]
fn outside_every_module_no_dependency_is_looked_in_in_english() {
    outside_every_module_no_dependency_is_looked_in(&english());
}

#[test]
fn outside_every_module_no_dependency_is_looked_in_in_japanese() {
    outside_every_module_no_dependency_is_looked_in(&japanese());
}

#[test]
fn a_dependency_reads_its_imports_from_its_own_root_in_english() {
    a_dependency_reads_its_imports_from_its_own_root(&english());
}

#[test]
fn a_dependency_reads_its_imports_from_its_own_root_in_japanese() {
    a_dependency_reads_its_imports_from_its_own_root(&japanese());
}

#[test]
fn an_import_written_with_at_is_not_read_in_a_module_in_english() {
    an_import_written_with_at_is_not_read_in_a_module(&english());
}

#[test]
fn an_import_written_with_at_is_not_read_in_a_module_in_japanese() {
    an_import_written_with_at_is_not_read_in_a_module(&japanese());
}

#[test]
fn outside_every_module_a_directory_named_with_at_is_a_directory_in_english() {
    outside_every_module_a_directory_named_with_at_is_a_directory(&english());
}

#[test]
fn outside_every_module_a_directory_named_with_at_is_a_directory_in_japanese() {
    outside_every_module_a_directory_named_with_at_is_a_directory(&japanese());
}

/// The English material copied where a test may add to it: `app`, `lib` and `base`.
fn copied() -> TempDir {
    let t = TempDir::new("proto-modules");
    let at = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/modules/en");
    for m in ["app", "lib", "base"] {
        ritsu_testkit::tmp::copy_dir(&at.join(m), &t.path().join(m));
    }
    t
}

/// app's own money with the text of lib's: lib's total reads lib's, and one run of protoc, which
/// reads app's for it (the first root that has it), reads the same file, so the statement can be
/// given to one run.
#[test]
fn two_files_of_one_text_at_one_path_are_one_file_to_protoc() {
    let t = copied();
    let app = t.path().join("app");
    t.write("app/proto/acme/orders/v1/money.proto", &t.read("lib/proto/acme/orders/v1/money.proto"));
    let p = protoc_roots(&app, STATEMENT, &[]).unwrap();
    assert_eq!(p.entry, "acme/billing/v1/statement.proto");
    assert_eq!(real(&p.includes), real(&[app.join("proto"), t.path().join("lib/proto")]));
}

/// A ledger of app's imports lib's order and lib's money, each by the path of its package; lib's
/// order imports the money from lib's root (`proto/acme/…`). ritsu reads one file by both names, and
/// one run of protoc would read it twice: `protoc_roots` names the file and the two names.
#[test]
fn a_file_two_files_reach_by_two_names_is_not_given_to_one_run() {
    let t = copied();
    let app = t.path().join("app");
    // app without a money of its own, so that the money by the path of its package is lib's
    std::fs::remove_file(app.join(OWN_MONEY)).unwrap();
    const LEDGER: &str = "proto/acme/billing/v1/ledger.proto";
    t.write(
        &format!("app/{LEDGER}"),
        "// A ledger of the module at hand: lib's orders, and an amount in lib's money, each imported by the\n// path of its package.\nsyntax = \"proto3\";\npackage acme.billing.v1;\n\nimport \"acme/orders/v1/order.proto\";\nimport \"acme/orders/v1/money.proto\";\n\nmessage Ledger {\n  repeated acme.orders.v1.Order orders = 1;\n  acme.orders.v1.Money balance = 2;\n}\n",
    );
    let (ps, issues) = load_from(&app, LEDGER, &[], &[]).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(imports_of(&ps, LEDGER), vec![Some(ORDER.to_string()), Some(MONEY.to_string())]);
    assert_eq!(imports_of(&ps, ORDER), vec![Some(MONEY.to_string())]);
    assert_eq!(
        protoc_roots(&app, LEDGER, &[]),
        Err(Clash::TwoNames { file: MONEY.to_string(), names: ["acme/orders/v1/money.proto".to_string(), "proto/acme/orders/v1/money.proto".to_string()] })
    );
}
