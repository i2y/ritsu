//! `ritsu mod` (DESIGN 17.19, 17.21; PLAN B.10): its table, held to golden files in English and in
//! Japanese, and listed by `ritsu --help`; a flag no command knows, refused by each of the fifteen
//! (exit 2); the commands of the stages to come, which say this version does not have them yet;
//! `init`, and the `ritsu.mod` it writes; `add` with a directory and `remove`, which change one line
//! and keep the comments, the order and the line endings; `list` of the example in both languages
//! and as JSON, held to golden files; `where`; and, with the modules of the example from a registry
//! (placed in a store of the test's own), `lock` (the lock it writes is the one the tests of the
//! tree write with `Lock::new`), `lock --check`, the lock `add` and `remove` write again, `vendor`
//! and `vendor --check`, and `clean` with `--dry-run` and `--keep`. A tree of paths needs no lock:
//! `lock` writes none, and removes one that is there.

use ritsu_base::modules::lock::{Lock, Locked};
use ritsu_base::modules::store::{self, Store};
use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::process::Command;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// `ritsu`, run in `dir` with the store at `store` and no language asked of the environment: the
/// exit code, what it printed and what it said on stderr.
fn ritsu_in(dir: &Path, store: &Path, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).args(args).env(store::VARIABLE, store);
    for v in ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG", "SEKISHO_LANG", "KANAME_LANG"] {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn golden(path: &str, got: &str, failures: &mut Vec<String>) {
    if let Err(e) = ritsu_testkit::golden::check(&here().join(path), got) {
        failures.push(e);
    }
}

/// `from` copied to `to`, file by file.
fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if e.file_type().unwrap().is_dir() {
            copy_dir(&src, &dst);
        } else {
            std::fs::write(&dst, std::fs::read(&src).unwrap()).unwrap();
        }
    }
}

/// The modules of the example (`shop`, `pricing`, `tax`), copied into a directory of their own.
fn example() -> TempDir {
    let t = TempDir::new("mod-cli");
    for m in ["shop", "pricing", "tax"] {
        copy_dir(&here().join("tests/projects/modules").join(m), &t.path().join(m));
    }
    t
}

/// The file `rel` of `t`, with `from` replaced by `to` (once, and it must be there).
fn replace(t: &TempDir, rel: &str, from: &str, to: &str) {
    let text = t.read(rel);
    assert!(text.contains(from), "{rel} has no `{from}`");
    t.write(rel, text.replacen(from, to, 1));
}

fn json(out: &str) -> serde_json::Value {
    serde_json::from_str(out).unwrap_or_else(|e| panic!("{e}: {out}"))
}

const COMMANDS: [&str; 15] = ["init", "add", "get", "remove", "lock", "fetch", "list", "versions", "vendor", "clean", "where", "push", "diff", "report", "site"];

/// `ritsu mod --help` in both languages, held to golden files: the fifteen commands of DESIGN 17.19,
/// in its order. Each command has its page, which `ritsu mod help <command>` prints too; `ritsu
/// --help` lists `ritsu mod`, and `ritsu help mod` is `ritsu mod --help`.
#[test]
fn ritsu_mod_has_a_table_of_its_own() {
    let t = TempDir::new("mod-help");
    let mut failures = Vec::new();
    for lang in ["en", "ja"] {
        let (code, out, err) = ritsu_in(t.path(), t.path(), &["mod", "--help", "--lang", lang]);
        assert_eq!((code, err.as_str()), (0, ""), "{out}");
        golden(&format!("tests/golden/modules/mod-help.{lang}.txt"), &out, &mut failures);
        assert_eq!(ritsu_in(t.path(), t.path(), &["--lang", lang, "help", "mod"]).1, out);
        let listed: Vec<&str> = out.lines().filter_map(|l| l.strip_prefix("  ritsu mod ")).filter_map(|l| l.split(' ').next()).collect();
        assert_eq!(listed, COMMANDS);
        for c in COMMANDS {
            let (code, page, err) = ritsu_in(t.path(), t.path(), &["mod", c, "--help", "--lang", lang]);
            assert!(code == 0 && err.is_empty() && page.starts_with(&format!("ritsu mod {c} — ")), "{c}: {page}{err}");
            assert_eq!(ritsu_in(t.path(), t.path(), &["mod", "help", c, "--lang", lang]).1, page, "{c}");
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    let (code, out, _) = ritsu_in(t.path(), t.path(), &["--help"]);
    assert!(code == 0 && out.contains("\n  ritsu mod <command> ...  "), "{out}");
    // with no command, the table on stderr
    let (code, out, err) = ritsu_in(t.path(), t.path(), &["mod"]);
    assert!(code == 2 && out.is_empty() && err.starts_with("ritsu mod "), "{err}");
}

/// A flag no command takes stops each of the fifteen before it does anything (exit 2), in both
/// languages; so does a command `ritsu mod` does not have, and a command given the wrong number of
/// arguments (its usage line).
#[test]
fn every_command_refuses_a_flag_it_does_not_know() {
    let t = TempDir::new("mod-flags");
    for c in COMMANDS {
        let (code, out, err) = ritsu_in(t.path(), t.path(), &["mod", c, "--frob"]);
        assert_eq!((code, out.as_str(), err.as_str()), (2, "", format!("error: unknown flag `--frob`; run `ritsu mod {c} --help`\n").as_str()), "{c}");
        let (code, _, err) = ritsu_in(t.path(), t.path(), &["mod", c, "--frob", "--lang", "ja"]);
        assert_eq!((code, err), (2, format!("エラー: `--frob` というフラグはありません。`ritsu mod {c} --help` を読んでください\n")), "{c}");
    }
    assert_eq!(ritsu_in(t.path(), t.path(), &["mod", "nope"]), (2, String::new(), "error: there is no command `nope`; run `ritsu mod --help`\n".into()));
    assert_eq!(ritsu_in(t.path(), t.path(), &["mod", "add", "pricing"]), (2, String::new(), "ritsu mod add <name> <source> [--format text|json]\n".into()));
    assert_eq!(ritsu_in(t.path(), t.path(), &["mod", "lock", "extra"]).0, 2);
    assert_eq!(ritsu_in(t.path(), t.path(), &["mod", "list", "--format", "yaml"]).0, 2);
}

/// The commands the stages to come make say that this version does not have them yet (exit 2),
/// and write nothing.
#[test]
fn the_commands_to_come_say_this_version_has_them_not() {
    let t = example();
    let shop = t.path().join("shop");
    let before = t.read("shop/ritsu.mod");
    for args in [
        &["fetch"][..],
        &["versions", "tax"],
        &["push", "--dry-run"],
        &["site", "modules.list", "--out", "site"],
        &["get", "pricing@latest"],
    ] {
        let (code, out, err) = ritsu_in(&shop, t.path(), &[&["mod"][..], args].concat());
        assert_eq!((code, out.as_str(), err), (2, "", format!("error: `ritsu mod {}` is not in this version yet\n", args[0])), "{args:?}");
    }
    let (code, _, err) = ritsu_in(&shop, t.path(), &["mod", "add", "tax", "example.com/acme/tax@1.0.0", "--lang", "ja"]);
    assert_eq!((code, err.as_str()), (2, "エラー: `ritsu mod add` のレジストリの形は、この版にはまだありません（ディレクトリの形 `./…`・`../…` は使えます）\n"));
    assert_eq!(t.read("shop/ritsu.mod"), before);
}

/// `init` writes the `ritsu.mod` of DESIGN 17.3 (the name, the version, the least ritsu that reads
/// it, `exclude "generated"`), in the directory ritsu runs in or the one `--root` names; it does
/// not write over one that is there, nor take a name or a version not of its form.
#[test]
fn init_writes_a_ritsu_mod() {
    let t = TempDir::new("mod-init");
    let v = env!("CARGO_PKG_VERSION");
    let (code, out, err) = ritsu_in(t.path(), t.path(), &["mod", "init", "example.com/acme/pricing"]);
    assert_eq!((code, out.as_str(), err.as_str()), (0, "wrote ritsu.mod: module example.com/acme/pricing 0.1.0\n", ""));
    assert_eq!(t.read("ritsu.mod"), format!("module example.com/acme/pricing\nversion 0.1.0\nritsu >={v}\n\nexclude \"generated\"\n"));
    let (code, out, _) = ritsu_in(t.path(), t.path(), &["mod", "list"]);
    assert_eq!((code, out.as_str()), (0, "module example.com/acme/pricing 0.1.0\nritsu mod list: 0 dependencies (0 by a path, 0 from a registry); no lock is needed\n"));
    assert_eq!(ritsu_in(t.path(), t.path(), &["mod", "init", "example.com/acme/tax"]), (2, String::new(), "error: `ritsu.mod` is there already\n".into()));
    std::fs::create_dir(t.path().join("tax")).unwrap();
    let (code, out, _) = ritsu_in(t.path(), t.path(), &["mod", "init", "tax", "--version", "1.0.0", "--root", "tax", "--lang", "ja"]);
    assert_eq!((code, out.as_str()), (0, "ritsu.mod を書きました（モジュール tax 1.0.0）\n"));
    assert!(t.read("tax/ritsu.mod").starts_with("module tax\nversion 1.0.0\n"));
    let (code, _, err) = ritsu_in(t.path(), t.path(), &["mod", "init", "Acme", "--root", "tax"]);
    assert_eq!((code, err.as_str()), (2, "error: `Acme` cannot name a module: the `A` at character 1 is not taken\n"));
    std::fs::create_dir(t.path().join("x")).unwrap();
    let (code, _, err) = ritsu_in(&t.path().join("x"), t.path(), &["mod", "init", "x", "--version", "v1.0.0"]);
    assert_eq!((code, err.as_str()), (2, "error: `v1.0.0` is not a version: write it with no `v` before it\n"));
    assert!(!t.path().join("x/ritsu.mod").exists());
}

/// `add` with a directory writes one `require … path` line after the last `require`, with the path
/// from the root of the module wherever ritsu runs in it, and `remove` takes that line out: every
/// other byte (the comments, the order, CRLF and the last line with no ending) stays. A name that is
/// required already, a directory with no `ritsu.mod`, and a name no line requires stop (exit 2),
/// and change nothing.
#[test]
fn add_and_remove_change_one_line() {
    let t = example();
    let shop = t.path().join("shop");
    let mod_crlf = "# The Acme shop.\r\nmodule example.com/acme/shop\r\nversion 0.1.0 # the first\r\n\r\n# its prices\r\nrequire pricing path \"../pricing\"\r\n\r\nexclude \"generated\"";
    t.write("shop/ritsu.mod", mod_crlf);
    std::fs::create_dir_all(shop.join("orders")).unwrap();
    // from a directory of the module: the path given from there, written from the root
    let (code, out, err) = ritsu_in(&shop.join("orders"), t.path(), &["mod", "add", "tax", "../../tax"]);
    assert_eq!((code, out.as_str(), err.as_str()), (0, "added `require tax path \"../tax\"` to ritsu.mod (example.com/acme/tax 1.0.0)\n", ""));
    let added = "# The Acme shop.\r\nmodule example.com/acme/shop\r\nversion 0.1.0 # the first\r\n\r\n# its prices\r\nrequire pricing path \"../pricing\"\r\nrequire tax path \"../tax\"\r\n\r\nexclude \"generated\"";
    assert_eq!(t.read("shop/ritsu.mod"), added);
    assert!(!shop.join("ritsu.lock").exists(), "a tree of paths needs no lock");
    let (code, _, err) = ritsu_in(&shop, t.path(), &["mod", "add", "tax", "../tax"]);
    assert_eq!((code, err.as_str()), (2, "error: the dependency `tax` is already on line 7 of `ritsu.mod`\n"));
    let (code, _, err) = ritsu_in(&shop, t.path(), &["mod", "add", "orders", "./orders", "--lang", "ja"]);
    assert_eq!((code, err.as_str()), (2, "エラー: `./orders` に `ritsu.mod` がありません。パスの依存は、`ritsu.mod` のあるディレクトリを指します\n"));
    let (code, _, err) = ritsu_in(&shop, t.path(), &["mod", "add", "Tax", "../tax"]);
    assert_eq!((code, err.as_str()), (2, "error: `Tax` cannot name a dependency: the `T` at character 1 is not taken\n"));
    assert_eq!(t.read("shop/ritsu.mod"), added);
    let (code, out, _) = ritsu_in(&shop, t.path(), &["mod", "remove", "pricing", "--lang", "ja"]);
    assert_eq!((code, out.as_str()), (0, "ritsu.mod から `require pricing path \"../pricing\"` を消しました\n"));
    assert_eq!(
        t.read("shop/ritsu.mod"),
        "# The Acme shop.\r\nmodule example.com/acme/shop\r\nversion 0.1.0 # the first\r\n\r\n# its prices\r\nrequire tax path \"../tax\"\r\n\r\nexclude \"generated\""
    );
    let (code, _, err) = ritsu_in(&shop, t.path(), &["mod", "remove", "pricing"]);
    assert_eq!((code, err.as_str()), (2, "error: `ritsu.mod` requires no `pricing`\n"));
    // the require put back after the last one: the file as it was, with `tax` after `pricing`'s place
    let (code, out, _) = ritsu_in(&shop, t.path(), &["mod", "add", "pricing", "../pricing", "--format", "json"]);
    assert_eq!(code, 0);
    assert_eq!(
        json(&out),
        serde_json::json!({
            "ritsu": env!("CARGO_PKG_VERSION"),
            "root": ".",
            "require": {"name": "pricing", "module": "example.com/acme/pricing", "version": "1.1.0", "source": "path", "path": "../pricing"},
            "manifest": "written",
            "lock": "none",
        })
    );
    assert_eq!(
        t.read("shop/ritsu.mod"),
        "# The Acme shop.\r\nmodule example.com/acme/shop\r\nversion 0.1.0 # the first\r\n\r\n# its prices\r\nrequire tax path \"../tax\"\r\nrequire pricing path \"../pricing\"\r\n\r\nexclude \"generated\""
    );
    // a ritsu.mod with no require: a blank line, then the line, at the end
    t.write("tax/ritsu.mod", "module example.com/acme/tax\nversion 1.0.0\n");
    std::fs::create_dir_all(t.path().join("rates")).unwrap();
    t.write("rates/ritsu.mod", "module example.com/acme/rates\n");
    let (code, _, err) = ritsu_in(&t.path().join("tax"), t.path(), &["mod", "add", "rates", "../rates"]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(t.read("tax/ritsu.mod"), "module example.com/acme/tax\nversion 1.0.0\n\nrequire rates path \"../rates\"\n");
}

/// `ritsu mod list` of the example `shop`, and of its twin with Japanese names (`店`), in both
/// languages and as JSON (DESIGN 17.21), held to golden files: the two modules the paths reach,
/// each with its prefix, what requires it, and where it is. No lock: the tree is of paths.
#[test]
fn the_list_of_the_example() {
    let empty = TempDir::new("mod-list-no-store");
    let mut failures = Vec::new();
    for (dir, name) in [("tests/projects/modules/shop", "shop"), ("tests/projects/modules.ja/店", "店")] {
        let d = here().join(dir);
        for lang in ["en", "ja"] {
            let (code, out, err) = ritsu_in(&d, empty.path(), &["mod", "list", "--lang", lang]);
            assert_eq!((code, err.as_str()), (0, ""), "{out}");
            golden(&format!("tests/golden/modules/list-{name}.{lang}.txt"), &out, &mut failures);
        }
        let (code, out, _) = ritsu_in(&d, empty.path(), &["mod", "list", "--format", "json"]);
        assert_eq!(code, 0);
        golden(&format!("tests/golden/modules/list-{name}.json"), &out, &mut failures);
        let v = json(&out);
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["ritsu", "root", "module", "lock", "modules", "diagnostics"]);
        let keys: Vec<&str> = v["modules"][0].as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["name", "version", "source", "path", "digest", "files_digest", "prefix", "required_by", "present", "where", "verified"]);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // from a directory of the module, and from above it with --root
    let shop = here().join("tests/projects/modules/shop");
    let (_, at_root, _) = ritsu_in(&shop, empty.path(), &["mod", "list"]);
    assert_eq!(ritsu_in(&shop.join("orders"), empty.path(), &["mod", "list"]).1, at_root);
    assert_eq!(ritsu_in(&here().join("tests/projects/modules"), empty.path(), &["mod", "list", "--root", "shop"]).1, at_root);
    // outside every module
    let (code, _, err) = ritsu_in(empty.path(), empty.path(), &["mod", "list"]);
    assert_eq!((code, err.as_str()), (2, "error: there is no `ritsu.mod` here or above; run `ritsu mod init <name>` to make a module\n"));
}

/// `ritsu mod where` says where an `@` path is on the disk, from where ritsu runs (a file of the
/// store, whole); a path that does not start with `@`, a dependency the module does not require and
/// a file the dependency does not have stop (exit 2).
#[test]
fn where_says_where_an_at_path_is() {
    let t = example();
    let (shop, at) = (t.path().join("shop"), t.path().join("store"));
    assert_eq!(ritsu_in(&shop, &at, &["mod", "where", "@pricing/rules/order_total.rule"]), (0, "../pricing/rules/order_total.rule\n".into(), String::new()));
    assert_eq!(ritsu_in(&shop.join("orders"), &at, &["mod", "where", "@pricing/@tax"]).1, "../../tax\n");
    let (code, _, err) = ritsu_in(&shop, &at, &["mod", "where", "rules/x.rule"]);
    assert_eq!((code, err.as_str()), (2, "error: `rules/x.rule` does not start with `@`; `ritsu mod where` says where a path that names a dependency by `@<name>/` is\n"));
    let (code, _, err) = ritsu_in(&shop, &at, &["mod", "where", "@tax/rules/vat_rate.rule"]);
    assert_eq!((code, err.as_str()), (2, "error: the `@tax` of `@tax/rules/vat_rate.rule` is no dependency: the `ritsu.mod` of the module `example.com/acme/shop` has no `require tax …`\n"));
    let (code, _, err) = ritsu_in(&shop, &at, &["mod", "where", "@pricing/rules/nope.rule"]);
    assert!(code == 2 && err.starts_with("error: ") && err.contains("nope.rule"), "{err}");
}

/// The example with `pricing` and `tax` from a registry (DESIGN 17.26), placed in a store of the
/// test's own: `ritsu mod lock` writes the lock the tests of the tree write with `Lock::new` (the
/// two `oci` lines), which `ritsu check` holds the tree to; `lock --check` passes on it, and fails
/// (exit 1, writing nothing) when `ritsu.mod` asks for what the lock does not pin. `add` of a path to
/// the module `tax` writes the lock again (the path wins: a `path` line), and `remove` takes it
/// back to the lock it was.
#[test]
fn lock_pins_the_modules_from_a_registry() {
    let t = example();
    replace(&t, "pricing/ritsu.mod", "require tax path \"../tax\"", "require tax oci \"example.com/acme/tax\" 1.0.0");
    replace(&t, "shop/ritsu.mod", "require pricing path \"../pricing\"", "require pricing oci \"example.com/acme/pricing\" 1.1.0");
    let store = Store::at(t.path().join("store"));
    let put = |dir: &str| -> Locked { store::put_directory(&store, &t.path().join(dir), "2026-10-28", &[]).unwrap_or_else(|e| panic!("{}", e.en)).locked() };
    let (tax, pricing) = (put("tax"), put("pricing"));
    let shop = t.path().join("shop");
    let at = t.path().join("store");
    let (code, out, err) = ritsu_in(&shop, &at, &["mod", "lock"]);
    assert_eq!((code, out.as_str(), err.as_str()), (0, "wrote ritsu.lock: 2 modules\n", ""));
    let want = Lock::new(vec![pricing.clone(), tax.clone()]).text();
    assert_eq!(t.read("shop/ritsu.lock"), want);
    let (code, out, err) = ritsu_in(&shop, &at, &["check"]);
    assert_eq!(code, 0, "{out}{err}");
    let (code, out, _) = ritsu_in(&shop, &at, &["mod", "lock", "--check", "--lang", "ja"]);
    assert_eq!((code, out.as_str()), (0, "ritsu.lock は書き直すところがありません（モジュール 2 個）\n"));
    let (code, out, _) = ritsu_in(&shop, &at, &["mod", "lock", "--check", "--format", "json"]);
    let v = json(&out);
    assert_eq!((code, &v["check"], &v["lock"]), (0, &serde_json::json!(true), &serde_json::json!("same")));
    assert_eq!(v["modules"][0]["name"], "example.com/acme/pricing");
    assert_eq!(v["modules"][1], serde_json::json!({"name": "example.com/acme/tax", "source": "oci", "version": "1.0.0", "path": null, "digest": v["modules"][1]["digest"], "files_digest": v["modules"][1]["files_digest"]}));
    // a lock another ritsu wrote, the same otherwise, is the same lock
    t.write("shop/ritsu.lock", want.replace(&format!("\nritsu {}\n", env!("CARGO_PKG_VERSION")), "\nritsu 0.37.0\n"));
    assert_eq!(ritsu_in(&shop, &at, &["mod", "lock"]).1, "ritsu.lock is up to date: 2 modules\n");
    assert!(t.read("shop/ritsu.lock").contains("\nritsu 0.37.0\n"));
    t.write("shop/ritsu.lock", &want);
    // ritsu.mod asks for a version of tax the lock does not pin: --check fails, and writes nothing
    replace(&t, "tax/ritsu.mod", "version 1.0.0", "version 1.1.0");
    let tax11 = put("tax");
    replace(&t, "shop/ritsu.mod", "require pricing oci", "require tax oci \"example.com/acme/tax\" 1.1.0\nrequire pricing oci");
    let (code, out, _) = ritsu_in(&shop, &at, &["mod", "lock", "--check"]);
    assert_eq!(code, 1, "{out}");
    let old_tax = tax.text();
    let new_tax = tax11.text();
    assert_eq!(out, format!("- {old_tax}\n+ {new_tax}\nritsu.lock would be written again; run `ritsu mod lock`\n"));
    assert_eq!(t.read("shop/ritsu.lock"), want);
    let (code, out, _) = ritsu_in(&shop, &at, &["mod", "lock"]);
    assert_eq!((code, out.as_str()), (0, "wrote ritsu.lock: 2 modules\n"));
    assert_eq!(t.read("shop/ritsu.lock"), Lock::new(vec![pricing.clone(), tax11.clone()]).text());
    let (code, out, err) = ritsu_in(&shop, &at, &["check"]);
    assert_eq!(code, 0, "{out}{err}");
    // add of a path to the module tax: the path wins, and the lock has its line
    replace(&t, "shop/ritsu.mod", "require tax oci \"example.com/acme/tax\" 1.1.0\n", "");
    assert_eq!(ritsu_in(&shop, &at, &["mod", "lock"]).0, 0);
    assert_eq!(t.read("shop/ritsu.lock"), want);
    let (code, out, err) = ritsu_in(&shop, &at, &["mod", "add", "tax", "../tax"]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(out, "added `require tax path \"../tax\"` to ritsu.mod (example.com/acme/tax 1.1.0)\nwrote ritsu.lock: 2 modules\n");
    let lock = t.read("shop/ritsu.lock");
    assert!(lock.ends_with(&format!("{}\npath example.com/acme/tax \"../tax\"\n", pricing.text())), "{lock}");
    let (code, out, _) = ritsu_in(&shop, &at, &["mod", "remove", "tax"]);
    assert_eq!((code, out.as_str()), (0, "removed `require tax path \"../tax\"` from ritsu.mod\nwrote ritsu.lock: 2 modules\n"));
    assert_eq!(t.read("shop/ritsu.lock"), want);
    // a version in no store: the lock cannot be chosen, and nothing changes
    let empty = TempDir::new("mod-cli-empty-store");
    std::fs::remove_file(shop.join("ritsu.lock")).unwrap();
    let before = t.read("shop/ritsu.mod");
    let (code, out, err) = ritsu_in(&shop, empty.path(), &["mod", "add", "tax", "../tax"]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert_eq!(err, "error: the dependency `example.com/acme/pricing` 1.1.0 is neither in the store nor in this module's copy of it; run `ritsu mod fetch`\n");
    assert_eq!(t.read("shop/ritsu.mod"), before);
    assert!(!shop.join("ritsu.lock").exists());
}

/// `ritsu mod list` of the tree from a registry: the digests and where the files are, and a module
/// of the lock whose files are not here (`present: false`), which is what the list is for (exit 0);
/// a lock that does not match is said with E304 (exit 1).
#[test]
fn the_list_says_where_the_modules_from_a_registry_are() {
    let t = example();
    replace(&t, "pricing/ritsu.mod", "require tax path \"../tax\"", "require tax oci \"example.com/acme/tax\" 1.0.0");
    replace(&t, "shop/ritsu.mod", "require pricing path \"../pricing\"", "require pricing oci \"example.com/acme/pricing\" 1.1.0");
    let store = Store::at(t.path().join("store"));
    let tax = store::put_directory(&store, &t.path().join("tax"), "2026-10-28", &[]).unwrap();
    store::put_directory(&store, &t.path().join("pricing"), "2026-10-28", &[]).unwrap();
    let (shop, at) = (t.path().join("shop"), t.path().join("store"));
    assert_eq!(ritsu_in(&shop, &at, &["mod", "lock"]).0, 0);
    let (code, out, _) = ritsu_in(&shop, &at, &["mod", "list"]);
    assert_eq!(code, 0, "{out}");
    let v = env!("CARGO_PKG_VERSION");
    assert_eq!(
        out,
        format!(
            "module example.com/acme/shop 0.1.0\n@pricing       example.com/acme/pricing 1.1.0  registry, in the store\n@pricing/@tax  example.com/acme/tax 1.0.0      registry, in the store\nritsu mod list: 2 dependencies (0 by a path, 2 from a registry); ritsu.lock written by ritsu {v}\n"
        )
    );
    let (_, out, _) = ritsu_in(&shop, &at, &["mod", "list", "--format", "json"]);
    let j = json(&out);
    assert_eq!(j["lock"], serde_json::json!({"lock": 1, "ritsu": v}));
    let m = &j["modules"][1];
    assert_eq!((m["source"].as_str(), m["where"].as_str(), m["present"].as_bool(), m["path"].is_null()), (Some("oci"), Some("store"), Some(true), true));
    assert_eq!((m["digest"].as_str(), m["files_digest"].as_str()), (Some(tax.digest.as_str()), Some(tax.files_digest.as_str())));
    assert_eq!(m["required_by"], serde_json::json!([{"module": "example.com/acme/pricing", "name": "tax", "version": "1.1.0"}]));
    // the files of tax gone from the store: listed, not here
    store::remove(&store.entry(&tax.digest)).unwrap();
    let (code, out, err) = ritsu_in(&shop, &at, &["mod", "list", "--lang", "ja"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("\n@pricing/@tax  example.com/acme/tax 1.0.0      レジストリ、取り出していない\n"), "{out}");
    assert!(out.ends_with("。取り出していないもの 1 個\n"), "{out}");
    assert_eq!(err, "備考: 依存 `example.com/acme/tax` 1.0.0 が、置き場にも同梱のコピーにもありません。`ritsu mod fetch` を走らせてください\n");
    let (_, out, _) = ritsu_in(&shop, &at, &["mod", "list", "--format", "json"]);
    let m = &json(&out)["modules"][1];
    assert_eq!((m["present"].as_bool(), m["where"].is_null()), (Some(false), true));
    // a lock with no line for pricing: E304
    store::put_directory(&store, &t.path().join("tax"), "2026-10-28", &[]).unwrap();
    let lock = t.read("shop/ritsu.lock");
    t.write("shop/ritsu.lock", lock.lines().filter(|l| !l.contains("acme/pricing")).map(|l| format!("{l}\n")).collect::<String>());
    let (code, out, _) = ritsu_in(&shop, &at, &["mod", "list"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("error[ritsu E304]: ritsu.mod:7:9: `ritsu.lock` does not match `ritsu.mod`: the lock has no line for `require pricing oci \"example.com/acme/pricing\" 1.1.0`; run `ritsu mod lock`\n"), "{out}");
    assert!(out.contains("\n@pricing  example.com/acme/pricing  registry, not in the lock\n"), "{out}");
    assert!(out.ends_with("; 1 error\n"), "{out}");
}

/// `ritsu mod vendor` puts the modules of the lock in `.ritsu/modules/` from the store, in the
/// store's shape (no `refs`, no `tmp`), and `ritsu check` reads them there with no store; `--check`
/// says a copy that differs and an entry the lock does not pin (exit 1), and `vendor` puts it right.
/// A module in neither the store nor the copy stops `vendor` (exit 2). `clean --dry-run` lists the
/// entries and removes none; `--keep` keeps what a lock pins; `clean` removes the rest, and the
/// index of versions that leads to them.
#[test]
fn vendor_and_clean() {
    let t = example();
    replace(&t, "pricing/ritsu.mod", "require tax path \"../tax\"", "require tax oci \"example.com/acme/tax\" 1.0.0");
    replace(&t, "shop/ritsu.mod", "require pricing path \"../pricing\"", "require pricing oci \"example.com/acme/pricing\" 1.1.0");
    let store = Store::at(t.path().join("store"));
    let tax = store::put_directory(&store, &t.path().join("tax"), "2026-10-28", &[]).unwrap();
    let pricing = store::put_directory(&store, &t.path().join("pricing"), "2026-10-28", &[]).unwrap();
    let (shop, at) = (t.path().join("shop"), t.path().join("store"));
    let empty = TempDir::new("mod-cli-vendor-empty");
    assert_eq!(ritsu_in(&shop, &at, &["mod", "lock"]).0, 0);
    let (code, out, _) = ritsu_in(&shop, &at, &["mod", "vendor", "--check"]);
    assert_eq!((code, out.as_str()), (1, "missing: example.com/acme/pricing 1.1.0\nmissing: example.com/acme/tax 1.0.0\nritsu mod vendor --check: .ritsu/modules does not match the lock; run `ritsu mod vendor`\n"));
    let (code, out, err) = ritsu_in(&shop, &at, &["mod", "vendor"]);
    assert_eq!((code, out.as_str(), err.as_str()), (0, "ritsu mod vendor: .ritsu/modules holds the 2 modules of the lock (2 placed, 0 removed)\n", ""));
    let own = Store::vendored(&shop);
    for p in [&tax, &pricing] {
        assert!(own.has_files(&p.digest) && own.record(&p.digest).is_some(), "{}", p.name);
    }
    assert!(!shop.join(".ritsu/modules/refs").exists() && !shop.join(".ritsu/modules/tmp").exists());
    let (code, out, _) = ritsu_in(&shop, &at, &["mod", "vendor", "--check", "--lang", "ja"]);
    assert_eq!((code, out.as_str()), (0, "ritsu mod vendor --check: .ritsu/modules は lock に合います（モジュール 2 個）\n"));
    // the copy is read with no store
    let (code, out, err) = ritsu_in(&shop, empty.path(), &["check", "--format", "json"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(json(&out)["dependencies"][0]["where"], "vendor");
    assert_eq!(ritsu_in(&shop, empty.path(), &["mod", "list"]).1.lines().nth(1), Some("@pricing       example.com/acme/pricing 1.1.0  registry, in this module's copy"));
    // a file of the copy changed, and an entry the lock does not pin
    let f = own.files(&tax.digest).join("rules/vat_rate.rule");
    let mut perm = std::fs::metadata(&f).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perm.set_readonly(false);
    std::fs::set_permissions(&f, perm).unwrap();
    std::fs::write(&f, std::fs::read_to_string(&f).unwrap().replace("| standard | 20%", "| standard | 21%")).unwrap();
    let stray = format!("sha256:{}", "0".repeat(64));
    std::fs::create_dir_all(own.entry(&stray)).unwrap();
    let (code, out, _) = ritsu_in(&shop, &at, &["mod", "vendor", "--check"]);
    assert_eq!(code, 1);
    assert_eq!(out, format!("differs from the lock: example.com/acme/tax 1.0.0\nnot in the lock: {stray}\nritsu mod vendor --check: .ritsu/modules does not match the lock; run `ritsu mod vendor`\n"));
    let (_, out, _) = ritsu_in(&shop, &at, &["mod", "vendor", "--check", "--lang", "ja"]);
    assert_eq!(out, format!("example.com/acme/tax 1.0.0 が lock と違います\n{stray} は lock にありません\nritsu mod vendor --check: .ritsu/modules が lock に合いません。`ritsu mod vendor` を走らせてください\n"));
    let (code, out, _) = ritsu_in(&shop, &at, &["mod", "vendor"]);
    assert_eq!((code, out.as_str()), (0, "ritsu mod vendor: .ritsu/modules holds the 2 modules of the lock (1 placed, 1 removed)\n"));
    assert_eq!(ritsu_in(&shop, &at, &["mod", "vendor", "--check"]).0, 0);
    // clean: --dry-run removes nothing; --keep keeps what the lock pins; then the store is empty
    let (code, out, _) = ritsu_in(&shop, &at, &["mod", "clean", "--dry-run"]);
    assert_eq!(code, 0);
    let mut digests = [pricing.digest.clone(), tax.digest.clone()];
    digests.sort();
    let lines: Vec<String> = digests
        .iter()
        .map(|d| {
            let what = if *d == tax.digest { "example.com/acme/tax 1.0.0" } else { "example.com/acme/pricing 1.1.0" };
            format!("would remove {d} ({what})")
        })
        .collect();
    assert_eq!(out, format!("{}\n{}\nritsu mod clean --dry-run: 2 entries of the store would be removed, 0 kept; nothing was removed\n", lines[0], lines[1]));
    let (_, out, _) = ritsu_in(&shop, &at, &["mod", "clean", "--dry-run", "--lang", "ja"]);
    assert_eq!(out.lines().next(), Some(format!("{}（{}）を消します", digests[0], if digests[0] == tax.digest { "example.com/acme/tax 1.0.0" } else { "example.com/acme/pricing 1.1.0" }).as_str()));
    assert!(out.ends_with("ritsu mod clean --dry-run: 置き場の項目 2 個を消し、0 個を残すことになります。何も消していません\n"), "{out}");
    assert!(store.has_files(&tax.digest) && store.has_files(&pricing.digest));
    let (code, out, _) = ritsu_in(t.path(), &at, &["mod", "clean", "--keep", "shop/ritsu.lock", "--lang", "ja"]);
    assert_eq!((code, out.as_str()), (0, "ritsu mod clean: 置き場の項目を 0 個消し、2 個残しました\n"));
    let (code, out, _) = ritsu_in(t.path(), &at, &["mod", "clean"]);
    assert_eq!(code, 0);
    assert!(out.ends_with("ritsu mod clean: removed 2 entries of the store, kept 0\n"), "{out}");
    assert!(!store.has(&tax.digest) && !store.has(&pricing.digest));
    assert!(std::fs::read_dir(at.join("refs")).map(|mut d| d.next().is_none()).unwrap_or(true), "the index of versions leads nowhere now");
    // the store is empty, and the copy is not: vendor has nothing to place; with neither, it stops
    let (code, out, _) = ritsu_in(&shop, &at, &["mod", "vendor"]);
    assert_eq!((code, out.as_str()), (0, "ritsu mod vendor: .ritsu/modules holds the 2 modules of the lock (0 placed, 0 removed)\n"));
    store::remove(&shop.join(".ritsu")).unwrap();
    let (code, out, err) = ritsu_in(&shop, &at, &["mod", "vendor"]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert_eq!(err, "error: `example.com/acme/pricing 1.1.0` is not in the store; run `ritsu mod fetch`\n");
    assert!(!shop.join(".ritsu").exists());
    let (code, _, err) = ritsu_in(&shop, &at, &["mod", "clean", "--keep", "nope.lock"]);
    assert!(code == 2 && err.starts_with("error: cannot read `nope.lock`, given with --keep, as a lock ("), "{err}");
}

/// A tree of paths needs no lock (DESIGN 17.4): `lock` writes none and says so; a lock left there
/// (from when a dependency came from a registry) is what `--check` fails on and `lock` removes.
#[test]
fn a_tree_of_paths_needs_no_lock() {
    let t = example();
    let shop = t.path().join("shop");
    let (code, out, _) = ritsu_in(&shop, t.path(), &["mod", "lock", "--check"]);
    assert_eq!((code, out.as_str()), (0, "no lock is needed: no dependency comes from a registry\n"));
    let (code, out, _) = ritsu_in(&shop, t.path(), &["mod", "lock", "--lang", "ja"]);
    assert_eq!((code, out.as_str()), (0, "lock は要りません。レジストリから取る依存がありません\n"));
    assert!(!shop.join("ritsu.lock").exists());
    t.write("shop/ritsu.lock", "lock 1\nritsu 0.37.0\npath example.com/acme/pricing \"../pricing\"\npath example.com/acme/tax \"../tax\"\n");
    let (code, out, _) = ritsu_in(&shop, t.path(), &["mod", "lock", "--check", "--format", "json"]);
    assert_eq!(code, 1);
    assert_eq!(json(&out), serde_json::json!({"ritsu": env!("CARGO_PKG_VERSION"), "root": ".", "check": true, "lock": "removed", "modules": []}));
    assert!(shop.join("ritsu.lock").exists());
    let (code, out, _) = ritsu_in(&shop, t.path(), &["mod", "lock"]);
    assert_eq!((code, out.as_str()), (0, "removed ritsu.lock: no dependency comes from a registry, so the tree needs no lock\n"));
    assert!(!shop.join("ritsu.lock").exists());
    // a tree that cannot be read: the lock is not chosen (exit 2), and its diagnostics are said
    replace(&t, "shop/ritsu.mod", "require pricing path \"../pricing\"", "require pricing path \"../nowhere\"");
    let (code, out, err) = ritsu_in(&shop, t.path(), &["mod", "lock"]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert!(err.starts_with("error[ritsu E302]: ritsu.mod:7:9: `../nowhere`, where `require pricing path \"../nowhere\"` leads, has no `ritsu.mod`"), "{err}");
    assert!(err.ends_with("error: the modules could not be read (1 error); nothing was written\n"), "{err}");
}
