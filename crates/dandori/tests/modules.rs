//! Modules (ritsu's DESIGN 17.6): in a module, a path a flow writes that starts with `@<name>/`
//! (`use rule|dates|book|proto … from`, a task's `flow`, the key files of a secret) is read from the
//! root of the dependency `<name>` that the module's `ritsu.mod` requires, the path a dependency's
//! own file writes from that dependency's root, and `dandori check` reads such a path from the
//! command line, with no flag. A flow reads a rule, a book (chobo's), a `.proto` and a child flow of
//! a dependency as it reads its own; the `.proto` of a dependency imports its own files from the
//! dependency's root. A path out of the module, into a module nested in it, a dependency the module
//! does not require and a file a dependency does not have are each an error of the flow's own code
//! for what cannot be read there (E005 for a rule or a book, E015 for a child flow, E016 for a
//! `.proto`), with the words ritsu-base has for them; outside every module, `@…` is a plain relative
//! path, and the error for a file that is not there says so.
//!
//! The material is `tests/modules/`, in English (`en/`) and in Japanese (`ja/`): the module at hand
//! (`app`), its path dependency (`lib`: a rule that applies base's, a book, the order service's
//! `.proto`, a flow), the dependency's dependency (`base`, which app reaches only as `@lib/@base/…`),
//! a module nested in app (`app/inner`) and a directory outside every module (`loose`). The command
//! runs in this process with the rules, the dates files and the books joined, as `ritsu dandori` runs
//! it, in the material's directories (one at a time: the working directory is the process's), so the
//! paths it prints are the ones given to it. The golden files are beside the material,
//! `tests/modules/en.txt` and `tests/modules/ja.txt` (`DANDORI_BLESS=1` writes them).

use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// The working directory is the process's: one command at a time moves it.
static HERE: Mutex<()> = Mutex::new(());

/// `dandori` with `args`, run in `dir` (under `tests/modules/`) with rulec, koyomi and chobo
/// joined: the command as written, what it printed and how it ended.
fn dandori(dir: &str, args: &[&str]) -> (u8, String) {
    let _one = HERE.lock().unwrap_or_else(|e| e.into_inner());
    let back = std::env::current_dir().unwrap();
    std::env::set_current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/modules").join(dir)).unwrap();
    let args_owned: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let rules = Rc::new(rulec::ports::Engine::with_dates(Arc::new(koyomi::ports::Engine)));
    let code = dandori::cli::run_with_ports(&args_owned, rules, Rc::new(koyomi::ports::Engine), Rc::new(chobo::ports::Engine), &mut out, &mut err);
    std::env::set_current_dir(back).unwrap();
    let said = format!("$ dandori {}\n{}{}exit {code}\n\n", args.join(" "), String::from_utf8_lossy(&out), String::from_utf8_lossy(&err)).replace(env!("CARGO_PKG_VERSION"), "<version>");
    (code, said)
}

/// The commands in `dir`, each with the exit code it must end with, as one transcript.
fn transcript(out: &mut String, dir: &str, runs: &[(&[&str], u8)]) {
    out.push_str(&format!("# in {dir}\n\n"));
    for (args, want) in runs {
        let (code, said) = dandori(dir, args);
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
            // the order reads lib's rule (which applies base's: `@lib/@base/…`), its book, its
            // .proto (which imports lib's money.proto) and its flow
            (&["check", "--lang", "en", "orders/place_order.flow"], 0),
            (&["check", "--lang", "en", "orders/place_order.flow", "--format", "json"], 0),
            // the single command reads `@` from the command line
            (&["check", "--lang", "en", "@lib/flows/price.flow"], 0),
            // the four errors
            (&["check", "--lang", "en", "errors/not_required.flow"], 1),
            (&["check", "--lang", "en", "errors/outside.flow"], 1),
            (&["check", "--lang", "en", "errors/nested.flow"], 1),
            (&["check", "--lang", "en", "errors/missing.flow"], 1),
            (&["check", "--lang", "en", "errors/missing.flow", "--format", "json"], 1),
        ],
    );
    // from any directory of the module, `@lib/…` names the same file
    transcript(&mut t, "en/app/orders", &[(&["check", "--lang", "en", "@lib/flows/price.flow"], 0)]);
    // outside every module, `@lib/…` is a path beside the file
    transcript(&mut t, "en/loose", &[(&["check", "--lang", "en", "place_order.flow"], 1), (&["check", "--lang", "en", "place_order.flow", "--format", "json"], 1)]);
    for said in [
        "the `@shop` of `@shop/rules/order_total.rule` is no dependency",
        "`../../lib/books/ledger.book` leads outside the module `example.com/acme/app`",
        "`../inner/flows/price.flow` leads into `inner`, a directory below with a `ritsu.mod` of its own, without an `@`",
        "the dependency `@lib` (`example.com/acme/lib`, `../lib`) has no `proto/acme/orders/v1/orders.proto`",
        "a path that starts with `@` names a dependency only in a module",
    ] {
        assert!(t.contains(said), "{said}\n{t}");
    }
    ritsu_testkit::golden(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/modules/en.txt"), &t);
}

#[test]
fn a_module_in_japanese() {
    let mut t = String::new();
    transcript(
        &mut t,
        "ja/アプリ",
        &[
            (&["check", "--lang", "ja", "注文/注文を受ける.flow"], 0),
            (&["check", "--lang", "ja", "注文/注文を受ける.flow", "--format", "json"], 0),
            (&["check", "--lang", "ja", "@lib/流れ/値付け.flow"], 0),
            (&["check", "--lang", "ja", "誤り/依存に無い.flow"], 1),
            (&["check", "--lang", "ja", "誤り/外を指す.flow"], 1),
            (&["check", "--lang", "ja", "誤り/入れ子.flow"], 1),
            (&["check", "--lang", "ja", "誤り/無いファイル.flow"], 1),
            (&["check", "--lang", "ja", "誤り/無いファイル.flow", "--format", "json"], 1),
        ],
    );
    transcript(&mut t, "ja/アプリ/注文", &[(&["check", "--lang", "ja", "@lib/流れ/値付け.flow"], 0)]);
    transcript(&mut t, "ja/外", &[(&["check", "--lang", "ja", "注文を受ける.flow"], 1), (&["check", "--lang", "ja", "注文を受ける.flow", "--format", "json"], 1)]);
    for said in [
        "`@shop/規則/注文の合計.rule` の `@shop` は依存ではありません",
        "`../../部品/帳簿/台帳.book` は、モジュール `example.com/acme/app` の外を指します",
        "`../入れ子/流れ/値付け.flow` は、`ritsu.mod` を持つ下のディレクトリ `入れ子` の中を、`@` を通らずに指しています",
        "依存 `@lib`（`example.com/acme/lib`、`../部品`）に `proto/acme/orders/v1/orders.proto` はありません",
        "`@` で始まるパスが依存を指すのは、`ritsu.mod` のあるモジュールの中だけです",
    ] {
        assert!(t.contains(said), "{said}\n{t}");
    }
    ritsu_testkit::golden(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/modules/ja.txt"), &t);
}
