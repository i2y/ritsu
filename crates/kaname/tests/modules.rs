//! Modules of ritsu's (ritsu's DESIGN 17.6), not kaname's own `module` (which the files of
//! `tests/modules/*.kn` are): in a module of ritsu's (a directory with a `ritsu.mod`), a path a `.kn`
//! writes that starts with `@<name>/` (`use rule|dates|book|module … from`) is read from the root of
//! the dependency `<name>` that the `ritsu.mod` requires, the path a dependency's own file writes
//! from that dependency's root, and `kaname check` reads such a path from the command line, with no
//! flag. A module reads a rule, a book (chobo's) and a module of a dependency as it reads its own, and
//! a type of a file is one type whichever path names the file (`@lib/rules/order_total.rule` here,
//! `../rules/order_total.rule` in lib's own module). A path out of the module, into a module nested
//! in it, a dependency the module does not require and a file a dependency does not have are each
//! E201 at the path, with the words ritsu-base has for them; outside every module, `@…` is a plain
//! relative path, and the error for a file that is not there says so.
//!
//! The material is `tests/modules/en/` and `tests/modules/ja/` (beside kaname's own test modules):
//! the module at hand (`app`), its path dependency (`lib`: a rule that applies base's, a book, a
//! module), the dependency's dependency (`base`, which app reaches only as `@lib/@base/…`), a module
//! nested in app (`app/inner`) and a directory outside every module (`loose`). The command runs in
//! this process with every language joined, as `ritsu kaname` runs it, in the material's
//! directories (one at a time: the working directory is the process's), so the paths it prints are
//! the ones given to it. The golden files are `tests/golden/modules/` (`KANAME_BLESS=1` writes them).

mod common;

use std::path::Path;
use std::sync::Mutex;

/// The working directory is the process's: one command at a time moves it.
static HERE: Mutex<()> = Mutex::new(());

/// `kaname` with `args`, run in `dir` (under `tests/modules/`) with every language joined: the
/// command as written, what it printed and how it ended.
fn kaname(dir: &str, args: &[&str]) -> (u8, String) {
    let _one = HERE.lock().unwrap_or_else(|e| e.into_inner());
    let back = std::env::current_dir().unwrap();
    std::env::set_current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/modules").join(dir)).unwrap();
    let args_owned: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = kaname::run::run(&args_owned, common::joined(), &mut out, &mut err);
    std::env::set_current_dir(back).unwrap();
    let said = format!("$ kaname {}\n{}{}exit {code}\n\n", args.join(" "), String::from_utf8_lossy(&out), String::from_utf8_lossy(&err)).replace(env!("CARGO_PKG_VERSION"), "<version>");
    (code, said)
}

/// The commands in `dir`, each with the exit code it must end with, as one transcript.
fn transcript(out: &mut String, dir: &str, runs: &[(&[&str], u8)]) {
    out.push_str(&format!("# in {dir}\n\n"));
    for (args, want) in runs {
        let (code, said) = kaname(dir, args);
        assert_eq!(code, *want, "{said}");
        out.push_str(&said);
    }
}

#[test]
fn a_module_in_english() {
    let mut t = String::new();
    transcript(
        &mut t,
        "en/app",
        &[
            // the basket calls lib's rule (which applies base's: `@lib/@base/…`), its book and its
            // module, whose own rule and book are the same files by other paths
            (&["check", "--lang", "en", "checkout/basket.kn"], 0),
            (&["check", "--lang", "en", "checkout/basket.kn", "--format", "json"], 0),
            // the single command reads `@` from the command line
            (&["check", "--lang", "en", "@lib/checkout/charges.kn"], 0),
            // the four errors
            (&["check", "--lang", "en", "errors/not_required.kn"], 1),
            (&["check", "--lang", "en", "errors/outside.kn"], 1),
            (&["check", "--lang", "en", "errors/nested.kn"], 1),
            (&["check", "--lang", "en", "errors/missing.kn"], 1),
            (&["check", "--lang", "en", "errors/missing.kn", "--format", "json"], 1),
        ],
    );
    // from any directory of the module, `@lib/…` names the same file
    transcript(&mut t, "en/app/checkout", &[(&["check", "--lang", "en", "@lib/checkout/charges.kn"], 0)]);
    // outside every module, `@lib/…` is a path beside the file
    transcript(&mut t, "en/loose", &[(&["check", "--lang", "en", "order.kn"], 1), (&["check", "--lang", "en", "order.kn", "--format", "json"], 1)]);
    for said in [
        "The `@shop` of `@shop/rules/order_total.rule` is no dependency",
        "`../../lib/rules/order_total.rule` leads outside the module `example.com/acme/app`",
        "`../inner/checkout/basket.kn` leads into `inner`, a directory below with a `ritsu.mod` of its own, without an `@`",
        "The dependency `@lib` (`example.com/acme/lib`, `../lib`) has no `rules/order_totals.rule`",
        "A path that starts with `@` names a dependency only in a module",
    ] {
        assert!(t.contains(said), "{said}\n{t}");
    }
    ritsu_testkit::golden(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/modules/en.txt"), &t);
}

#[test]
fn a_module_in_japanese() {
    let mut t = String::new();
    transcript(
        &mut t,
        "ja/アプリ",
        &[
            (&["check", "--lang", "ja", "会計/かご.kn"], 0),
            (&["check", "--lang", "ja", "会計/かご.kn", "--format", "json"], 0),
            (&["check", "--lang", "ja", "@lib/会計/請求.kn"], 0),
            (&["check", "--lang", "ja", "誤り/依存に無い.kn"], 1),
            (&["check", "--lang", "ja", "誤り/外を指す.kn"], 1),
            (&["check", "--lang", "ja", "誤り/入れ子.kn"], 1),
            (&["check", "--lang", "ja", "誤り/無いファイル.kn"], 1),
            (&["check", "--lang", "ja", "誤り/無いファイル.kn", "--format", "json"], 1),
        ],
    );
    transcript(&mut t, "ja/アプリ/会計", &[(&["check", "--lang", "ja", "@lib/会計/請求.kn"], 0)]);
    transcript(&mut t, "ja/外", &[(&["check", "--lang", "ja", "注文.kn"], 1), (&["check", "--lang", "ja", "注文.kn", "--format", "json"], 1)]);
    for said in [
        "`@shop/規則/注文の合計.rule` の `@shop` は依存ではありません",
        "`../../部品/規則/注文の合計.rule` は、モジュール `example.com/acme/app` の外を指します",
        "`../入れ子/会計/かご.kn` は、`ritsu.mod` を持つ下のディレクトリ `入れ子` の中を、`@` を通らずに指しています",
        "依存 `@lib`（`example.com/acme/lib`、`../部品`）に `規則/注文合計.rule` はありません",
        "`@` で始まるパスが依存を指すのは、`ritsu.mod` のあるモジュールの中だけです",
    ] {
        assert!(t.contains(said), "{said}\n{t}");
    }
    ritsu_testkit::golden(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/modules/ja.txt"), &t);
}
