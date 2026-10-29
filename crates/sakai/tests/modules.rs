//! Modules of ritsu's (ritsu's DESIGN 17.6): in a module, a path a `.ctx` writes that starts with
//! `@<name>/` (`use context`, `covers`, `except`, `proto root`, `code`, and the paths of the entries,
//! `<tool> "<path>"`) is read from the root of the dependency `<name>` that the module's `ritsu.mod`
//! requires, the path a dependency's own file writes from that dependency's root, and `sakai check`
//! reads such a path from the command line, with no flag. A map reads a context of a dependency as
//! it reads its own. A file of a dependency is in the map's scope when a context of the map owns it
//! (`covers "."` does not go into a dependency), or when `covers` names the dependency (`covers
//! "@lib"`, and then each of its artifacts must be owned); a reference to a file of a dependency that
//! no context owns is E103, with a note that says how to take it in. A path out of the module, into
//! a module nested in it and a dependency the module does not require are each E012 (a path that
//! cannot be one from the root), and a file a dependency does not have is E009 (the path is not
//! there), with the words ritsu-base has for them; outside every module, `@…` is a plain relative
//! path, and the error for a path that is not there says so.
//!
//! The material is `tests/modules/`, in English (`en/`) and in Japanese (`ja/`): the module at hand
//! (`app`, whose map reads its billing and lib's pricing, and the same map with `covers ".",
//! "@lib"`), its path dependency (`lib`: the pricing, its rule, which applies base's, which the
//! pricing owns too, and lib's own map), the dependency's dependency (`base`, rules alone, which app
//! reaches only as `@lib/@base/…`), a module nested in app (`app/inner`) and a directory outside
//! every module (`loose`). The command runs in this process with every language joined, as `ritsu
//! sakai` runs it, in the material's directories (one at a time: the working directory is the
//! process's), so the paths it prints are the ones given to it. In a module the root is the module's
//! (its `ritsu.mod`); outside every module the root would be this repository's (its `.git`), so the
//! commands there give `--root .`. A walk of the material's own directory, outside every module,
//! does not enter the modules in it, and says so in a line each. The golden files are
//! `tests/golden/modules/` (`SAKAI_BLESS=1` writes them).

mod common;

use std::path::Path;
use std::sync::Mutex;

/// The working directory is the process's: one command at a time moves it.
static HERE: Mutex<()> = Mutex::new(());

/// `sakai` with `args`, run in `dir` (under `tests/modules/`) with every language joined: the
/// command as written, what it printed and how it ended.
fn sakai(dir: &str, args: &[&str]) -> (u8, String) {
    let _one = HERE.lock().unwrap_or_else(|e| e.into_inner());
    let back = std::env::current_dir().unwrap();
    std::env::set_current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/modules").join(dir)).unwrap();
    let args_owned: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = sakai::run::run(&args_owned, common::suite(), &mut out, &mut err);
    std::env::set_current_dir(back).unwrap();
    let said = format!("$ sakai {}\n{}{}exit {code}\n\n", args.join(" "), String::from_utf8_lossy(&out), String::from_utf8_lossy(&err)).replace(env!("CARGO_PKG_VERSION"), "<version>");
    (code, said)
}

/// The commands in `dir`, each with the exit code it must end with, as one transcript.
fn transcript(out: &mut String, dir: &str, runs: &[(&[&str], u8)]) {
    out.push_str(&format!("# in {dir}\n\n"));
    for (args, want) in runs {
        let (code, said) = sakai(dir, args);
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
            // the map reads lib's context (`@lib/…`), which owns lib's rule and base's, which the
            // rule applies (`@lib/@base/…`): both are in the scope of `covers "."`
            (&["check", "--lang", "en", "shop.ctx"], 0),
            (&["check", "--lang", "en", "shop.ctx", "--format", "json"], 0),
            // `covers "@lib"` takes in every file of lib
            (&["check", "--lang", "en", "covers_lib.ctx"], 0),
            // the single command reads `@` from the command line: lib's own map
            (&["check", "--lang", "en", "@lib/totals.ctx"], 0),
            // the four errors
            (&["check", "--lang", "en", "errors/not_required.ctx"], 1),
            (&["check", "--lang", "en", "errors/outside.ctx"], 1),
            (&["check", "--lang", "en", "errors/nested.ctx"], 1),
            (&["check", "--lang", "en", "errors/missing.ctx"], 1),
            (&["check", "--lang", "en", "errors/missing.ctx", "--format", "json"], 1),
            // a file of a dependency no context owns, which an artifact of the scope refers to
            (&["check", "--lang", "en", "errors/unowned.ctx"], 1),
            (&["check", "--lang", "en", "errors/unowned.ctx", "--format", "json"], 1),
        ],
    );
    // from any directory of the module, `@lib/…` names the same file
    transcript(&mut t, "en/app/contexts", &[(&["check", "--lang", "en", "@lib/totals.ctx"], 0)]);
    // lib's own map in lib: base's VAT rate (`@base/…`) is in its scope as it is in app's
    transcript(&mut t, "en/lib", &[(&["check", "--lang", "en", "totals.ctx"], 0)]);
    // outside every module, `@lib/…` is a path beside the file
    transcript(&mut t, "en/loose", &[(&["check", "--lang", "en", "shop.ctx", "--root", "."], 1), (&["check", "--lang", "en", "shop.ctx", "--root", ".", "--format", "json"], 1)]);
    // a walk that starts outside every module does not enter one, and says so in a line each
    transcript(&mut t, "en", &[(&["check", "--lang", "en", ".", "--root", "."], 1)]);
    for said in [
        "note: `app` has a `ritsu.mod` of its own: it is another module, and was not read; run `sakai check app` to read it",
        "The `@shop` of `@shop/contexts/pricing.ctx` is no dependency",
        "`../../lib/contexts/pricing.ctx` leads outside the module `example.com/acme/app`",
        "`../inner/contexts/pricing.ctx` leads into `inner`, a directory below with a `ritsu.mod` of its own, without an `@`",
        "The dependency `@lib` (`example.com/acme/lib`, `../lib`) has no `contexts/pricings.ctx`",
        "A path that starts with `@` names a dependency only in a module",
        "The file @lib/rules/order_total.rule refers to rulec \"@lib/@base/rules/vat_rate.rule\", which is outside the map's scope (apply)",
        "@lib/@base/rules/vat_rate.rule is a file of the dependency `@lib/@base`: bring the dependency's context that owns it into the map with `use context \"@lib/@base/…\"`, or write `\"@lib/@base\"` in `covers` and own the file in a context of the map.",
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
            (&["check", "--lang", "ja", "通販.ctx"], 0),
            (&["check", "--lang", "ja", "通販.ctx", "--format", "json"], 0),
            (&["check", "--lang", "ja", "依存も範囲に.ctx"], 0),
            (&["check", "--lang", "ja", "@lib/合計.ctx"], 0),
            (&["check", "--lang", "ja", "誤り/依存に無い.ctx"], 1),
            (&["check", "--lang", "ja", "誤り/外を指す.ctx"], 1),
            (&["check", "--lang", "ja", "誤り/入れ子.ctx"], 1),
            (&["check", "--lang", "ja", "誤り/無いファイル.ctx"], 1),
            (&["check", "--lang", "ja", "誤り/無いファイル.ctx", "--format", "json"], 1),
            (&["check", "--lang", "ja", "誤り/持ち主の無い依存.ctx"], 1),
            (&["check", "--lang", "ja", "誤り/持ち主の無い依存.ctx", "--format", "json"], 1),
        ],
    );
    transcript(&mut t, "ja/アプリ/コンテキスト", &[(&["check", "--lang", "ja", "@lib/合計.ctx"], 0)]);
    transcript(&mut t, "ja/部品", &[(&["check", "--lang", "ja", "合計.ctx"], 0)]);
    transcript(&mut t, "ja/外", &[(&["check", "--lang", "ja", "通販.ctx", "--root", "."], 1), (&["check", "--lang", "ja", "通販.ctx", "--root", ".", "--format", "json"], 1)]);
    transcript(&mut t, "ja", &[(&["check", "--lang", "ja", ".", "--root", "."], 1)]);
    for said in [
        "備考: `アプリ` は自分の `ritsu.mod` を持つ別のモジュールなので、読んでいません。読むには `sakai check アプリ` を実行してください",
        "`@shop/コンテキスト/値付け.ctx` の `@shop` は依存ではありません",
        "`../../部品/コンテキスト/値付け.ctx` は、モジュール `example.com/acme/app` の外を指します",
        "`../入れ子/コンテキスト/値付け.ctx` は、`ritsu.mod` を持つ下のディレクトリ `入れ子` の中を、`@` を通らずに指しています",
        "依存 `@lib`（`example.com/acme/lib`、`../部品`）に `コンテキスト/値段.ctx` はありません",
        "`@` で始まるパスが依存を指すのは、`ritsu.mod` のあるモジュールの中だけです",
        "@lib/規則/注文の合計.rule が、地図の範囲の外の rulec \"@lib/@base/規則/付加価値税率.rule\" を参照しています（apply）",
        "@lib/@base/規則/付加価値税率.rule は依存 `@lib/@base` のファイルです。それを持つ依存のコンテキストを `use context \"@lib/@base/…\"` で地図に入れるか、`covers` に `\"@lib/@base\"` を書き、地図のどれかのコンテキストに持たせてください。",
    ] {
        assert!(t.contains(said), "{said}\n{t}");
    }
    ritsu_testkit::golden(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/modules/ja.txt"), &t);
}

/// The note of E012 says what found the root (ritsu's DESIGN 17.6). A root that holds a `ritsu.mod`
/// is a module's, and the note says so. Here the map is in a directory below that root which holds
/// a `.git`, and a module does not reach past a `.git`: the map is in no module, and its path is held
/// to the root. A root that holds no `ritsu.mod` keeps the note it had (the golden files of the
/// mutant `E012_outside_the_root`).
#[test]
fn the_note_of_a_path_out_of_the_root_says_the_root_is_a_module_s() {
    let t = std::env::temp_dir().join(format!("sakai-module-root-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&t);
    std::fs::create_dir_all(t.join("repo/.git")).unwrap();
    std::fs::write(t.join("ritsu.mod"), "module app\n").unwrap();
    std::fs::write(t.join("repo/shop.ctx"), "map Shop(shop) v1\nuse context \"alpha.ctx\"\ncovers \".\"\n").unwrap();
    std::fs::write(t.join("repo/alpha.ctx"), "context Alpha(a) v1\nowns\n  dir \"../../outside\"\n").unwrap();
    let os = sakai::check::check_args_with(&t, &["repo/shop.ctx".to_string()], &common::suite());
    let _ = std::fs::remove_dir_all(&t);
    let os = os.unwrap();
    let [en, ja] = [ritsu_base::text::Lang::En, ritsu_base::text::Lang::Ja].map(|l| os.iter().map(|o| sakai::check::render(o, l)).collect::<String>());
    assert!(en.contains("error[E012]: repo/alpha.ctx:3:7: The path \"../../outside\" goes outside the root"), "{en}");
    assert!(en.contains("= The root is the nearest directory above the path given to sakai that holds .git or ritsu.mod; here, it is the module's root, which holds ritsu.mod (--root changes it)."), "{en}");
    assert!(ja.contains("= ルートは、sakai に渡したパスの上で .git か ritsu.mod を持つ一番近いディレクトリです。ここでは ritsu.mod を持つモジュールのルートです（--root で替えられます）。"), "{ja}");
}
