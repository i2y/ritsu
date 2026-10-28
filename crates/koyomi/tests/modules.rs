//! Modules (ritsu's DESIGN 17.6): in a module, a path a `.cal` writes that starts with `@<name>/`
//! (`use calendar`, `source … = file`) is read from the root of the dependency `<name>` that the
//! module's `ritsu.mod` requires, the path a dependency's own file writes from that dependency's
//! root, and `koyomi check` reads such a path from the command line, with no flag. A path out of the
//! module, into a module nested in it, a dependency the module does not require and a file a
//! dependency does not have are each E015 (the calendar cannot be read), with the words ritsu-base
//! has for them; outside every module, `@…` is a plain relative path, and the error for a file that
//! is not there says so.
//!
//! The material is `tests/modules/`, in English (`en/`) and in Japanese (`ja/`): the module at hand
//! (`app`), its path dependency (`lib`, whose calendar reads the closed days of `base` by
//! `source … = file "@base/…"`), the dependency's dependency (`base`, which app reaches only as
//! `@lib/@base/…`), a module nested in app (`app/inner`) and a directory outside every module
//! (`loose`). The commands run in the material's directories, so the paths they print are the ones
//! given to them. A walk of the material's own directory, outside every module, does not enter the
//! modules in it, and says so in a line each. The golden files are `tests/golden/modules/`
//! (`KOYOMI_BLESS=1` writes them).

use std::path::Path;
use std::process::Command;

/// `koyomi` with `args`, run in `dir` (under `tests/modules/`), with no language from the
/// environment: the command as written, what it printed and how it ended.
fn koyomi(dir: &str, args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_koyomi"))
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/modules").join(dir))
        .env_remove("KOYOMI_LANG")
        .env_remove("RITSU_LANG")
        .args(args)
        .output()
        .expect("cannot start koyomi");
    let code = o.status.code().unwrap_or(-1);
    let said = format!("$ koyomi {}\n{}{}exit {code}\n\n", args.join(" "), String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).replace(env!("CARGO_PKG_VERSION"), "<version>");
    (code, said)
}

/// The commands in `dir`, each with the exit code it must end with, as one transcript.
fn transcript(out: &mut String, dir: &str, runs: &[(&[&str], i32)]) {
    out.push_str(&format!("# in {dir}\n\n"));
    for (args, want) in runs {
        let (code, said) = koyomi(dir, args);
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
            // the due date rolls on lib's calendar, which reads base's closed days (`@lib/@base/…`)
            (&["check", "dates/net10.cal"], 0),
            (&["check", "dates/net10.cal", "--format", "json"], 0),
            // the single command reads `@` from the command line
            (&["check", "@lib/calendars/office.cal"], 0),
            (&["check", "@lib/calendars/office.cal", "--format", "json"], 0),
            // the four errors
            (&["check", "errors/not_required.cal"], 1),
            (&["check", "errors/outside.cal"], 1),
            (&["check", "errors/nested.cal"], 1),
            (&["check", "errors/missing.cal"], 1),
            (&["check", "errors/missing.cal", "--format", "json"], 1),
        ],
    );
    // from any directory of the module, `@lib/…` names the same file
    transcript(&mut t, "en/app/dates", &[(&["check", "@lib/calendars/office.cal"], 0)]);
    // outside every module, `@lib/…` is a path beside the file
    transcript(&mut t, "en/loose", &[(&["check", "net10.cal"], 1), (&["check", "net10.cal", "--format", "json"], 1)]);
    // a walk that starts outside every module does not enter one, and says so in a line each
    transcript(&mut t, "en", &[(&["check", "."], 1)]);
    for said in [
        "note: `./app` has a `ritsu.mod` of its own: it is another module, and was not read; run `koyomi check ./app` to read it",
        "The `@shop` of `@shop/calendars/office.cal` is no dependency",
        "`../../lib/calendars/office.cal` leads outside the module `example.com/acme/app`",
        "`../inner/calendars/office.cal` leads into `inner`, a directory below with a `ritsu.mod` of its own, without an `@`",
        "The dependency `@lib` (`example.com/acme/lib`, `../lib`) has no `calendars/offices.cal`",
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
            (&["check", "--lang", "ja", "日付/十日後払い.cal"], 0),
            (&["check", "--lang", "ja", "日付/十日後払い.cal", "--format", "json"], 0),
            (&["check", "--lang", "ja", "@lib/カレンダー/事務所.cal"], 0),
            (&["check", "--lang", "ja", "@lib/カレンダー/事務所.cal", "--format", "json"], 0),
            (&["check", "--lang", "ja", "誤り/依存に無い.cal"], 1),
            (&["check", "--lang", "ja", "誤り/外を指す.cal"], 1),
            (&["check", "--lang", "ja", "誤り/入れ子.cal"], 1),
            (&["check", "--lang", "ja", "誤り/無いファイル.cal"], 1),
            (&["check", "--lang", "ja", "誤り/無いファイル.cal", "--format", "json"], 1),
        ],
    );
    transcript(&mut t, "ja/アプリ/日付", &[(&["check", "--lang", "ja", "@lib/カレンダー/事務所.cal"], 0)]);
    transcript(&mut t, "ja/外", &[(&["check", "--lang", "ja", "十日後払い.cal"], 1), (&["check", "--lang", "ja", "十日後払い.cal", "--format", "json"], 1)]);
    transcript(&mut t, "ja", &[(&["check", "--lang", "ja", "."], 1)]);
    for said in [
        "備考: `./アプリ` は自分の `ritsu.mod` を持つ別のモジュールなので、読んでいません。読むには `koyomi check ./アプリ` を実行してください",
        "`@shop/カレンダー/事務所.cal` の `@shop` は依存ではありません",
        "`../../部品/カレンダー/事務所.cal` は、モジュール `example.com/acme/app` の外を指します",
        "`../入れ子/カレンダー/事務所.cal` は、`ritsu.mod` を持つ下のディレクトリ `入れ子` の中を、`@` を通らずに指しています",
        "依存 `@lib`（`example.com/acme/lib`、`../部品`）に `カレンダー/本社.cal` はありません",
        "`@` で始まるパスが依存を指すのは、`ritsu.mod` のあるモジュールの中だけです",
    ] {
        assert!(t.contains(said), "{said}\n{t}");
    }
    ritsu_testkit::golden(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/modules/ja.txt"), &t);
}
