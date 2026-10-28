//! Modules of ritsu's (ritsu's DESIGN 17.6): in a module, a path a `.req` writes that starts with
//! `@<name>/` (a link's `<tool> "<path>" …`, a `scope`, a source's `file "…"`, `openspec "…"` and
//! the naming it borrows) is read from the root of the dependency `<name>` that the module's
//! `ritsu.mod` requires, the path a dependency's own file writes from that dependency's root, and
//! `yuen check` reads such a path from the command line, with no flag. Requirements come from a
//! dependency's document and are met by a dependency's rules, read as the module's own; a
//! dependency's requirements hold the reviews written in the dependency, as the end of a requirement
//! writes its sources' paths from the root of its own module; and `yuen source fetch` leaves a
//! dependency's document to the dependency. A path out of the module, into a module nested in it and
//! a dependency the module does not require are each E013 (a naming that cannot be resolved), and a
//! file a dependency does not have is E201 (what a link names is not there), with the words
//! ritsu-base has for them; outside every module, `@…` is a plain relative path, whose copy is the
//! project's own to fetch, and the errors for the files that are not there say so.
//!
//! The material is `tests/modules/`, in English (`en/`) and in Japanese (`ja/`): the module at hand
//! (`app`), its path dependency (`lib`: the pricing policy, its rule, which applies base's, and lib's
//! own requirements), the dependency's dependency (`base`, which app names as `@lib/@base/…`), a
//! module nested in app (`app/inner`) and a directory outside every module (`loose`). The reviews
//! were written by `yuen review` in each module (their records are its `reviewed/`). The command runs
//! in this process with every language joined, as `ritsu yuen` runs it, in the material's
//! directories (one at a time: the working directory is the process's), so the paths it prints are
//! the ones given to it. In a module the root is the module's (its `ritsu.mod`); outside every module
//! the root would be this repository's (its `.git`), so the commands there give `--root .`. A walk
//! of the material's own directory, outside every module, does not enter the modules in it, and
//! says so in a line each. The golden files are `tests/golden/modules/` (`YUEN_BLESS=1` writes
//! them).

mod common;

use std::path::Path;
use std::sync::Mutex;

/// The working directory is the process's: one command at a time moves it.
static HERE: Mutex<()> = Mutex::new(());

/// `yuen` with `args`, run in `dir` (under `tests/modules/`) with every language joined: the command
/// as written, what it printed and how it ended.
fn yuen(dir: &str, args: &[&str]) -> (u8, String) {
    let _one = HERE.lock().unwrap_or_else(|e| e.into_inner());
    let back = std::env::current_dir().unwrap();
    std::env::set_current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/modules").join(dir)).unwrap();
    let args_owned: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = yuen::run::run(&args_owned, common::suite(), &mut out, &mut err);
    std::env::set_current_dir(back).unwrap();
    let said = format!("$ yuen {}\n{}{}exit {code}\n\n", args.join(" "), String::from_utf8_lossy(&out), String::from_utf8_lossy(&err)).replace(env!("CARGO_PKG_VERSION"), "<version>");
    (code, said)
}

/// The commands in `dir`, each with the exit code it must end with, as one transcript.
fn transcript(out: &mut String, dir: &str, runs: &[(&[&str], u8)]) {
    out.push_str(&format!("# in {dir}\n\n"));
    for (args, want) in runs {
        let (code, said) = yuen(dir, args);
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
            // the requirements come from lib's policy and are met by lib's rule and base's
            // (`@lib/@base/…`)
            (&["check", "--lang", "en", "billing.req"], 0),
            (&["check", "--lang", "en", "billing.req", "--format", "json"], 0),
            // the single command reads `@` from the command line: lib's requirements, reviewed in lib
            (&["check", "--lang", "en", "@lib/pricing.req"], 0),
            // lib's policy is lib's to fetch
            (&["source", "fetch", "--lang", "en", "billing.req"], 0),
            // the four errors
            (&["check", "--lang", "en", "errors/not_required.req"], 1),
            (&["check", "--lang", "en", "errors/outside.req"], 1),
            (&["check", "--lang", "en", "errors/nested.req"], 1),
            (&["check", "--lang", "en", "errors/missing.req"], 1),
            (&["check", "--lang", "en", "errors/missing.req", "--format", "json"], 1),
        ],
    );
    // from any directory of the module, `@lib/…` names the same file
    transcript(&mut t, "en/app/errors", &[(&["check", "--lang", "en", "@lib/pricing.req"], 0)]);
    // outside every module, `@lib/…` is a path beside the file, and its copy is the project's own
    let mut loose = String::new();
    transcript(
        &mut loose,
        "en/loose",
        &[
            (&["check", "--lang", "en", "billing.req", "--root", "."], 1),
            (&["check", "--lang", "en", "billing.req", "--root", ".", "--format", "json"], 1),
            (&["source", "fetch", "--lang", "en", "billing.req", "--root", "."], 0),
        ],
    );
    assert!(loose.contains("no url, so the copy cannot be fetched") && !loose.contains("file of a dependency"), "{loose}");
    t.push_str(&loose);
    // a walk that starts outside every module does not enter one, and says so in a line each
    transcript(&mut t, "en", &[(&["check", "--lang", "en", ".", "--root", "."], 1)]);
    for said in [
        "note: `app` has a `ritsu.mod` of its own: it is another module, and was not read; run `yuen check app` to read it",
        "The `@shop` of `@shop/rules/order_total.rule` is no dependency",
        "`../../lib/rules/order_total.rule` leads outside the module `example.com/acme/app`",
        "`../inner/rules/order_total.rule` leads into `inner`, a directory below with a `ritsu.mod` of its own, without an `@`",
        "The dependency `@lib` (`example.com/acme/lib`, `../lib`) has no `rules/order_totals.rule`",
        "A path that starts with `@` names a dependency only in a module",
        "is a file of a dependency, so it is not fetched here",
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
            (&["check", "--lang", "ja", "請求.req"], 0),
            (&["check", "--lang", "ja", "請求.req", "--format", "json"], 0),
            (&["check", "--lang", "ja", "@lib/値付け.req"], 0),
            (&["source", "fetch", "--lang", "ja", "請求.req"], 0),
            (&["check", "--lang", "ja", "誤り/依存に無い.req"], 1),
            (&["check", "--lang", "ja", "誤り/外を指す.req"], 1),
            (&["check", "--lang", "ja", "誤り/入れ子.req"], 1),
            (&["check", "--lang", "ja", "誤り/無いファイル.req"], 1),
            (&["check", "--lang", "ja", "誤り/無いファイル.req", "--format", "json"], 1),
        ],
    );
    transcript(&mut t, "ja/アプリ/誤り", &[(&["check", "--lang", "ja", "@lib/値付け.req"], 0)]);
    let mut loose = String::new();
    transcript(
        &mut loose,
        "ja/外",
        &[
            (&["check", "--lang", "ja", "請求.req", "--root", "."], 1),
            (&["check", "--lang", "ja", "請求.req", "--root", ".", "--format", "json"], 1),
            (&["source", "fetch", "--lang", "ja", "請求.req", "--root", "."], 0),
        ],
    );
    assert!(loose.contains("url が無いので取れません") && !loose.contains("依存のファイル"), "{loose}");
    t.push_str(&loose);
    transcript(&mut t, "ja", &[(&["check", "--lang", "ja", ".", "--root", "."], 1)]);
    for said in [
        "備考: `アプリ` は自分の `ritsu.mod` を持つ別のモジュールなので、読んでいません。読むには `yuen check アプリ` を実行してください",
        "`@shop/規則/注文の合計.rule` の `@shop` は依存ではありません",
        "`../../部品/規則/注文の合計.rule` は、モジュール `example.com/acme/app` の外を指します",
        "`../入れ子/規則/注文の合計.rule` は、`ritsu.mod` を持つ下のディレクトリ `入れ子` の中を、`@` を通らずに指しています",
        "依存 `@lib`（`example.com/acme/lib`、`../部品`）に `規則/注文合計.rule` はありません",
        "`@` で始まるパスが依存を指すのは、`ritsu.mod` のあるモジュールの中だけです",
        "は依存のファイルなので、取り直しません",
    ] {
        assert!(t.contains(said), "{said}\n{t}");
    }
    ritsu_testkit::golden(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/modules/ja.txt"), &t);
}

/// A scope over a directory (DESIGN 5.2) does not take in another module below it, a directory with
/// a `ritsu.mod` of its own (ritsu's DESIGN 17.6): its files are that module's, as a walk of ritsu's
/// leaves them, in a module and outside every module alike. Were the module's file in scope, no
/// requirement would lead to it (E404).
#[test]
fn a_scope_does_not_take_in_another_module() {
    for in_a_module in [false, true] {
        let t = common::TempDir::new("modules-scope");
        if in_a_module {
            t.write("ritsu.mod", b"module example.com/acme/app\n");
        }
        t.write("pay.req", b"requirements pay v1\nrole accounting\n\nscope file \"src\"\n\nrequirement pay\n  text \"x\"\n  owner accounting\n  decided 2026-10-28 by accounting \"example\"\n  satisfied by file \"src/pay.py\"\n  not verified \"example\"\n");
        t.write("src/pay.py", b"code\n");
        t.write("src/vendor/ritsu.mod", b"module example.com/acme/vendor\n");
        t.write("src/vendor/report.py", b"code\n");
        common::yuen(t.path(), &["review", ".", "--root", ".", "--all", "--by", "accounting", "--date", "2026-10-28"]);
        let r = common::yuen(t.path(), &["check", ".", "--root", ".", "--lang", "en"]);
        assert!(common::codes(&r.stdout).is_empty() && r.code == 0, "{in_a_module}: {}{}", r.stdout, r.stderr);
        assert!(!r.stdout.contains("report.py"), "{in_a_module}: {}", r.stdout);
    }
}
