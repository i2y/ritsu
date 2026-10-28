//! Modules (ritsu's DESIGN 17.6): in a module, a path a gate writes that starts with `@<name>/` (the
//! eight `use`s, `workflow … from`) is read from the root of the dependency `<name>` that the
//! module's `ritsu.mod` requires, the path a dependency's own file writes from that dependency's
//! root, and `sekisho check` reads such a path from the command line, with no flag. A gate reads a
//! rule and a book of a dependency as it reads its own (the book is chobo's, its transfer guarded).
//! A path out of the module, into a module nested in it, a dependency the module does not require
//! and a file a dependency does not have are each E201 at the path, with the words ritsu-base has
//! for them; outside every module, `@…` is a plain relative path, and the error for a file that is
//! not there says so.
//!
//! The material is `tests/modules/`, in English (`en/`) and in Japanese (`ja/`): the module at hand
//! (`app`), its path dependency (`lib`: a rule that applies base's, a book, a gate), the
//! dependency's dependency (`base`, which app reaches only as `@lib/@base/…`), a module nested in
//! app (`app/inner`) and a directory outside every module (`loose`). The command runs in this
//! process with every language joined, as `ritsu sekisho` runs it, in the material's directories
//! (one at a time: the working directory is the process's), so the paths it prints are the ones
//! given to it. The golden files are `tests/golden/modules/` (`SEKISHO_BLESS=1` writes them).

mod common;

use std::path::Path;
use std::sync::Mutex;

/// The working directory is the process's: one command at a time moves it.
static HERE: Mutex<()> = Mutex::new(());

/// `sekisho` with `args`, run in `dir` (under `tests/modules/`) with every language joined: the
/// command as written, what it printed and how it ended.
fn sekisho(dir: &str, args: &[&str]) -> (u8, String) {
    let _one = HERE.lock().unwrap_or_else(|e| e.into_inner());
    let back = std::env::current_dir().unwrap();
    std::env::set_current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/modules").join(dir)).unwrap();
    let args_owned: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = sekisho::run::run(&args_owned, common::joined(), &mut out, &mut err);
    std::env::set_current_dir(back).unwrap();
    let said = format!("$ sekisho {}\n{}{}exit {code}\n\n", args.join(" "), String::from_utf8_lossy(&out), String::from_utf8_lossy(&err)).replace(env!("CARGO_PKG_VERSION"), "<version>");
    (code, said)
}

/// The commands in `dir`, each with the exit code it must end with, as one transcript.
fn transcript(out: &mut String, dir: &str, runs: &[(&[&str], u8)]) {
    out.push_str(&format!("# in {dir}\n\n"));
    for (args, want) in runs {
        let (code, said) = sekisho(dir, args);
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
            // the gate calls lib's rule, which applies base's (`@lib/@base/…`), and guards a
            // transfer of lib's book
            (&["check", "--lang", "en", "gates/purchases.gate"], 0),
            (&["check", "--lang", "en", "gates/purchases.gate", "--format", "json"], 0),
            // the single command reads `@` from the command line: lib's gate reads base's rule
            (&["check", "--lang", "en", "@lib/gates/approvals.gate"], 0),
            // the four errors
            (&["check", "--lang", "en", "errors/not_required.gate"], 1),
            (&["check", "--lang", "en", "errors/outside.gate"], 1),
            (&["check", "--lang", "en", "errors/nested.gate"], 1),
            (&["check", "--lang", "en", "errors/missing.gate"], 1),
            (&["check", "--lang", "en", "errors/missing.gate", "--format", "json"], 1),
        ],
    );
    // from any directory of the module, `@lib/…` names the same file
    transcript(&mut t, "en/app/gates", &[(&["check", "--lang", "en", "@lib/gates/approvals.gate"], 0)]);
    // outside every module, `@lib/…` is a path beside the file
    transcript(&mut t, "en/loose", &[(&["check", "--lang", "en", "purchases.gate"], 1), (&["check", "--lang", "en", "purchases.gate", "--format", "json"], 1)]);
    for said in [
        "The `@shop` of `@shop/rules/approval.rule` is no dependency",
        "`../../lib/rules/approval.rule` leads outside the module `example.com/acme/app`",
        "`../inner/rules/approval_limit.rule` leads into `inner`, a directory below with a `ritsu.mod` of its own, without an `@`",
        "The dependency `@lib` (`example.com/acme/lib`, `../lib`) has no `rules/approvals.rule`",
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
            (&["check", "--lang", "ja", "関所/購入.gate"], 0),
            (&["check", "--lang", "ja", "関所/購入.gate", "--format", "json"], 0),
            (&["check", "--lang", "ja", "@lib/関所/承認.gate"], 0),
            (&["check", "--lang", "ja", "誤り/依存に無い.gate"], 1),
            (&["check", "--lang", "ja", "誤り/外を指す.gate"], 1),
            (&["check", "--lang", "ja", "誤り/入れ子.gate"], 1),
            (&["check", "--lang", "ja", "誤り/無いファイル.gate"], 1),
            (&["check", "--lang", "ja", "誤り/無いファイル.gate", "--format", "json"], 1),
        ],
    );
    transcript(&mut t, "ja/アプリ/関所", &[(&["check", "--lang", "ja", "@lib/関所/承認.gate"], 0)]);
    transcript(&mut t, "ja/外", &[(&["check", "--lang", "ja", "購入.gate"], 1), (&["check", "--lang", "ja", "購入.gate", "--format", "json"], 1)]);
    for said in [
        "`@shop/規則/承認.rule` の `@shop` は依存ではありません",
        "`../../部品/規則/承認.rule` は、モジュール `example.com/acme/app` の外を指します",
        "`../入れ子/規則/承認の上限.rule` は、`ritsu.mod` を持つ下のディレクトリ `入れ子` の中を、`@` を通らずに指しています",
        "依存 `@lib`（`example.com/acme/lib`、`../部品`）に `規則/決裁.rule` はありません",
        "`@` で始まるパスが依存を指すのは、`ritsu.mod` のあるモジュールの中だけです",
    ] {
        assert!(t.contains(said), "{said}\n{t}");
    }
    ritsu_testkit::golden(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/modules/ja.txt"), &t);
}
