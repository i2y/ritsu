//! The groups of code and the settings `sakai build` writes (PLAN C.6, C.11; DESIGN 7.1): which
//! group may import which, the settings the example keeps (sakai writes them, and they are held
//! here as golden files: `SAKAI_BLESS=1` writes them again), `--check` (E502), and the maps whose
//! settings cannot be written (E501).

mod common;

use sakai::build::areas::{self, Language};
use sakai::build::{self, Target};
use sakai::i18n::Lang;
use std::path::{Path, PathBuf};

fn checked(dir: &Path, map: &str) -> sakai::check::Outcome {
    let o = sakai::check::check_map(dir, map).unwrap();
    let text: String = o.diags.iter().map(|d| d.render(Lang::En)).collect();
    assert!(!o.has_errors(), "{text}");
    o
}

/// Each group of a language, a line each: its name, its directories, and who may import it.
fn table(dir: &Path, map: &str, language: Language) -> String {
    let o = checked(dir, map);
    let c = o.checked.as_ref().unwrap();
    let a = areas::areas(c, language).unwrap_or_else(|ds| panic!("{}", ds.iter().map(|d| d.render(Lang::En)).collect::<String>()));
    let mut s = String::new();
    for (i, x) in a.areas.iter().enumerate() {
        let who: Vec<String> = a.importers(i).iter().map(|&j| areas::name(&c.model, &a.areas[j])).collect();
        s.push_str(&format!("{} [{}] <- {}\n", areas::name(&c.model, x), x.roots.join(", "), who.join(", ")));
    }
    s
}

fn example() -> PathBuf {
    std::fs::canonicalize(common::EXAMPLE).unwrap()
}

/// PLAN C.6: the published language of ordering is imported by ordering, by billing's layer and
/// by delivery (the partner); inventory's by inventory, ordering (the conformist), delivery's
/// layer and the code made from ordering's proto, which imports it; the shared kernel by billing
/// and delivery. A shared kernel imports nothing but itself.
#[test]
fn the_groups_of_the_example_and_who_may_import_them() {
    let want = "\
sakai-ordering [ordering] <- sakai-ordering, sakai-ordering-pl-shop.ordering.v1
sakai-ordering-pl-shop.ordering.v1 [shop/ordering/v1] <- sakai-ordering, sakai-ordering-pl-shop.ordering.v1, sakai-delivery, sakai-delivery-layer-inventory, sakai-billing-layer-ordering
sakai-inventory [inventory] <- sakai-inventory, sakai-inventory-pl-warehouse.v1
sakai-inventory-pl-warehouse.v1 [warehouse/v1] <- sakai-ordering, sakai-ordering-pl-shop.ordering.v1, sakai-inventory, sakai-inventory-pl-warehouse.v1, sakai-delivery-layer-inventory
sakai-delivery [delivery] <- sakai-delivery, sakai-delivery-pl-shop.delivery.v1, sakai-delivery-layer-inventory
sakai-delivery-pl-shop.delivery.v1 [shop/delivery/v1] <- sakai-ordering, sakai-delivery, sakai-delivery-pl-shop.delivery.v1, sakai-delivery-layer-inventory, sakai-billing, sakai-billing-layer-ordering
sakai-delivery-layer-inventory [delivery/acl/inventory] <- sakai-delivery, sakai-delivery-pl-shop.delivery.v1, sakai-delivery-layer-inventory
sakai-billing [billing] <- sakai-billing, sakai-billing-layer-ordering
sakai-billing-layer-ordering [billing/acl/ordering] <- sakai-billing, sakai-billing-layer-ordering
sakai-billing-kernel-delivery [calendars] <- sakai-delivery, sakai-delivery-pl-shop.delivery.v1, sakai-delivery-layer-inventory, sakai-billing, sakai-billing-layer-ordering, sakai-billing-kernel-delivery
sakai-reviews [reviews] <- sakai-reviews
";
    for language in [Language::Python, Language::TypeScript, Language::Java, Language::Go] {
        assert_eq!(table(&example(), "通販.ctx", language), want, "{}", language.word());
    }
}

/// A directory of the inside that holds the code made from the published language: the code goes
/// to the deeper group, and the downstream may import it but not the inside around it.
#[test]
fn a_published_language_inside_the_inside() {
    let dir = std::fs::canonicalize("tests/maps/入れ子").unwrap();
    let want = "\
sakai-inventory [warehouse] <- sakai-inventory, sakai-inventory-pl-warehouse.v1
sakai-inventory-pl-warehouse.v1 [warehouse/v1] <- sakai-inventory, sakai-inventory-pl-warehouse.v1, sakai-ordering
sakai-ordering [ordering] <- sakai-ordering
";
    assert_eq!(table(&dir, "入れ子.ctx", Language::Python), want);
}

/// The settings the example keeps are what sakai writes from its map (`--lang ja`).
#[test]
fn the_settings_of_the_example_are_what_the_map_writes() {
    let ex = example();
    let bless = std::env::var("SAKAI_BLESS").is_ok();
    let mut failures = Vec::new();
    for t in Target::ALL {
        let b = build::run(&ex, "通販.ctx", t, None, !bless, Lang::Ja).unwrap();
        if b.outcome.has_errors() || b.done.is_none() {
            failures.push(b.outcome.diags.iter().map(|d| d.render(Lang::En)).collect::<String>());
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// `--check` says it when the map changed after the settings were written: here, the partnership
/// of ordering and delivery is gone, and each tool's settings would change.
#[test]
fn check_says_when_the_map_moved_on() {
    let dir = common::TempDir::new();
    common::copy_dir(&example(), dir.path());
    for t in Target::ALL {
        let b = build::run(dir.path(), "通販.ctx", t, None, true, Lang::Ja).unwrap();
        assert!(!b.outcome.has_errors(), "{} is up to date in the copy", t.word());
        // Checked in another language, the words differ, and the note says why.
        let b = build::run(dir.path(), "通販.ctx", t, None, true, Lang::En).unwrap();
        let text: String = b.outcome.diags.iter().map(|d| d.render(Lang::En)).collect();
        assert!(text.contains("error[E502]") && text.contains("The file was written with --lang ja"), "{text}");
    }
    for (f, line) in [("contexts/受注.ctx", "partnership with 配送\n"), ("contexts/配送.ctx", "partnership with 受注\n")] {
        let p = dir.path().join(f);
        let s = std::fs::read_to_string(&p).unwrap();
        std::fs::write(&p, s.replace(line, "")).unwrap();
    }
    for t in Target::ALL {
        let b = build::run(dir.path(), "通販.ctx", t, None, true, Lang::Ja).unwrap();
        let codes: Vec<&str> = b.outcome.diags.iter().map(|d| d.code).collect();
        assert_eq!(codes, ["E502"], "{}", t.word());
        assert!(b.done.is_none());
    }
    // Written again, it is up to date.
    for t in Target::ALL {
        assert!(build::run(dir.path(), "通販.ctx", t, None, false, Lang::Ja).unwrap().done.is_some());
        let b = build::run(dir.path(), "通販.ctx", t, None, true, Lang::Ja).unwrap();
        assert!(b.outcome.diags.is_empty(), "{}", t.word());
    }
}

/// What `build` says when it cannot write the settings, for each reason.
fn e501(base: &str, edits: &[(&str, &str, &str)], map: &str, t: Target) -> String {
    let dir = common::variant(base, edits);
    let b = build::run(dir.path(), map, t, None, false, Lang::En).unwrap();
    let text: String = b.outcome.diags.iter().map(|d| d.render(Lang::En)).collect();
    let codes: Vec<&str> = b.outcome.diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, ["E501"], "{text}");
    assert!(b.done.is_none());
    text
}

fn example_variant(edits: &[(&str, &str, &str)]) -> common::TempDir {
    let dir = common::TempDir::new();
    common::copy_dir(&example(), dir.path());
    for (file, old, new) in edits {
        let p = dir.path().join(file);
        if old.is_empty() && new.is_empty() {
            std::fs::remove_file(&p).unwrap();
            continue;
        }
        let s = std::fs::read_to_string(&p).unwrap();
        assert!(s.contains(old), "{file} has no {old:?}");
        std::fs::write(&p, s.replacen(old, new, 1)).unwrap();
    }
    dir
}

#[test]
fn what_cannot_be_written() {
    // No code of the language.
    let t = e501("基本", &[], "基本.ctx", Target::GoArchLint);
    assert!(t.contains("The map has no `code go` line"), "{t}");
    // A directory whose name is not a Python module's.
    let t = e501(
        "基本",
        &[("ctx/受注.ctx", "\"../py/ordering\"", "\"../py/ordering\", \"../py/my-pkg\""), ("py/my-pkg/x.py", "", "X = 1\n")],
        "基本.ctx",
        Target::ImportLinter,
    );
    assert!(t.contains("`my-pkg` is not a name a Python module can have"), "{t}");
    // A file of code named alone.
    let t = e501("基本", &[("ctx/受注.ctx", "\"../py/ordering\"\n", "\"../py/ordering\"\n  file \"../py/ordering/fulfill.py\"\n")], "基本.ctx", Target::ImportLinter);
    assert!(t.contains("names one file of code"), "{t}");
    // A module right in the place of the Python code.
    let t = e501("基本", &[("ctx/受注.ctx", "\"../py/ordering\"", "\"../py/ordering\", \"../py\""), ("py/top.py", "", "X = 1\n")], "基本.ctx", Target::ImportLinter);
    assert!(t.contains("The module py/top.py is right in the place of the Python code"), "{t}");
    // ArchUnit without the place of the tests, and Go without go.mod.
    for (edits, t, says) in [
        (vec![("通販.ctx", "code java \"java/src/main/java\"\n  test \"java/src/test/java\"\n", "code java \"java/src/main/java\"\n")], Target::ArchUnit, "has no `test` line under it"),
        (vec![("go/go.mod", "", "")], Target::GoArchLint, "There is no go.mod in"),
        (vec![("contexts/レビュー.ctx", "\"../java/src/main/java/reviews\"", "\"../java/src/main/java\"")], Target::ArchUnit, "is in the default package"),
    ] {
        let dir = example_variant(&edits);
        if says.contains("default package") {
            dir.write("java/src/main/java/Main.java", "public class Main {}\n");
        }
        let b = build::run(dir.path(), "通販.ctx", t, None, false, Lang::En).unwrap();
        let text: String = b.outcome.diags.iter().map(|d| d.render(Lang::En)).collect();
        assert!(text.contains("error[E501]") && text.contains(says), "{text}");
    }
}
