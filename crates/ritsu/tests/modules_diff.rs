//! `ritsu mod diff` (DESIGN 17.9.5, 17.9.6, 17.21; PLAN 8.3 C.4), English first and the twin with
//! Japanese names beside it:
//!
//! - The example's `pricing` from a registry: tax and the three versions of pricing (each with the
//!   lock `ritsu mod lock` writes) placed in a store of the test's own, and compared by
//!   `<module>@<version>`. 1.0.0 → 1.1.0 moves the step of the weight and adds `eu` (answers that
//!   differ; an interface with a value added, and the number of a `connect` value changed); 1.1.0 →
//!   2.0.0 renames the output (changes that may break a user).
//! - Two modules the test writes, compared as directories: a desk (a calendar, a dates file that
//!   reads it, and a gate that reads a rule, from the materials of koyomi's and sekisho's diffs; the
//!   dates file's bytes do not change, its answers do) and a stock book (E050 and E051 at the top of
//!   its interface).
//! - Each in the three formats, held to golden files (`tests/golden/modules/diff-*`; `RITSU_BLESS=1`
//!   writes them). The digests of the store's versions are masked: the lock in each carries the
//!   version of ritsu.
//! - What each language says inside is what its own diff says: the JSON of each file's behavior is
//!   the one `ritsu <language> diff` prints for the same two files.
//! - The two-file form, with the spellings of DESIGN 17.9.1; a version against itself (exit 0); and
//!   what stops a diff (exit 2): a version not in the store, which names `ritsu mod fetch`; sides of
//!   two kinds; a directory in no module; a `--budget` that is no positive integer; and one too
//!   small for a walk, when nothing else differs.

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

/// The file `rel` of `t`, with `from` replaced by `to` (once, and it must be there).
fn replace(t: &Path, rel: &str, from: &str, to: &str) {
    let text = std::fs::read_to_string(t.join(rel)).unwrap();
    assert!(text.contains(from), "{rel} has no `{from}`");
    std::fs::write(t.join(rel), text.replacen(from, to, 1)).unwrap();
}

fn json(out: &str) -> serde_json::Value {
    serde_json::from_str(out).unwrap_or_else(|e| panic!("{e}: {out}"))
}

/// The text with each digest (`sha256:` and 64 hexadecimal digits) masked.
fn masked(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("sha256:") {
        let after = &rest[at + 7..];
        let hex = after.chars().take_while(|c| c.is_ascii_hexdigit()).count();
        out.push_str(&rest[..at]);
        if hex == 64 {
            out.push_str("sha256:<digest>");
            rest = &after[64..];
        } else {
            out.push_str("sha256:");
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// The names of the example in one language.
struct Names {
    lang: &'static str,
    dir: &'static str,
    shop: &'static str,
    pricing: &'static str,
    tax: &'static str,
    pricing_100: &'static str,
    pricing_200: &'static str,
    rules: &'static str,
    fee: &'static str,
}

const EN: Names = Names {
    lang: "en",
    dir: "modules",
    shop: "shop",
    pricing: "pricing",
    tax: "tax",
    pricing_100: "versions/pricing-1.0.0",
    pricing_200: "versions/pricing-2.0.0",
    rules: "rules",
    fee: "shipping_fee.rule",
};

const JA: Names = Names {
    lang: "ja",
    dir: "modules.ja",
    shop: "店",
    pricing: "価格",
    tax: "税",
    pricing_100: "版/価格-1.0.0",
    pricing_200: "版/価格-2.0.0",
    rules: "規則",
    fee: "送料.rule",
};

const PRICING: &str = "example.com/acme/pricing";

/// The example with tax and the three versions of pricing from a registry, in a store of the test's
/// own: each version of pricing with the lock `ritsu mod lock` writes (tax from the store), and the
/// module at hand, shop, locked to pricing 1.1.0.
fn registry(n: &Names) -> (TempDir, PathBuf) {
    let t = TempDir::new(&format!("mod-diff-registry-{}", n.lang));
    let d = t.path();
    let src = here().join("tests/projects").join(n.dir);
    for m in [n.shop, n.pricing, n.tax, n.pricing_100, n.pricing_200] {
        copy_dir(&src.join(m), &d.join(m));
    }
    let tax_line = "require tax oci \"example.com/acme/tax\" 1.0.0";
    replace(d, &format!("{}/ritsu.mod", n.pricing), &format!("require tax path \"../{}\"", n.tax), tax_line);
    for v in [n.pricing_100, n.pricing_200] {
        replace(d, &format!("{v}/ritsu.mod"), &format!("require tax path \"../../{}\"", n.tax), tax_line);
    }
    replace(d, &format!("{}/ritsu.mod", n.shop), &format!("require pricing path \"../{}\"", n.pricing), "require pricing oci \"example.com/acme/pricing\" 1.1.0");
    let store = d.join("store");
    let put = |m: &str| {
        store::put_directory(&Store::at(&store), &d.join(m), "2026-10-28", &[]).unwrap_or_else(|e| panic!("{}", e.en));
    };
    put(n.tax);
    for m in [n.pricing, n.pricing_100, n.pricing_200] {
        let (code, out, err) = ritsu_in(&d.join(m), &store, &["mod", "lock"]);
        assert_eq!(code, 0, "{m}: {out}{err}");
        put(m);
    }
    let (code, out, err) = ritsu_in(&d.join(n.shop), &store, &["mod", "lock"]);
    assert_eq!(code, 0, "{out}{err}");
    (t, store)
}

/// The names of the desk and the stock book in one language, and the materials of koyomi's and
/// sekisho's diffs they are made of.
struct Desk {
    lang: &'static str,
    desk: (&'static str, &'static str),
    stock: (&'static str, &'static str),
    calendars: &'static str,
    gates: &'static str,
    books: &'static str,
    /// The calendar: its name in the desk, its two versions in koyomi's materials, and the table of
    /// days each reads.
    office: &'static str,
    offices: (&'static str, &'static str),
    days: (&'static str, &'static str),
    /// The dates file, which reads the calendar.
    payment: &'static str,
    /// The gate and the rule it reads: their names, and the versions in sekisho's materials.
    gate: &'static str,
    gates_of: (&'static str, &'static str),
    rule: &'static str,
    rules_of: (&'static str, &'static str),
    book: &'static str,
    books_of: (&'static str, &'static str),
}

const DESK_EN: Desk = Desk {
    lang: "en",
    desk: ("desk-1.0.0", "desk-1.1.0"),
    stock: ("stock-1.0.0", "stock-1.1.0"),
    calendars: "calendars",
    gates: "gates",
    books: "books",
    office: "office.cal",
    offices: ("office.cal", "office_v2.cal"),
    days: ("days_off.csv", "days_off_v2.csv"),
    payment: "payment.cal",
    gate: "limits.gate",
    gates_of: ("limits.gate", "limits_v2.gate"),
    rule: "band.rule",
    rules_of: ("band.rule", "band_v2.rule"),
    book: "stock.book",
    books_of: (STOCK_EN, STOCK_EN_V2),
};

const DESK_JA: Desk = Desk {
    lang: "ja",
    desk: ("事務-1.0.0", "事務-1.1.0"),
    stock: ("在庫-1.0.0", "在庫-1.1.0"),
    calendars: "暦",
    gates: "ゲート",
    books: "帳簿",
    office: "事務所.cal",
    offices: ("事務所.cal", "事務所_二版.cal"),
    days: ("休み.csv", "休み_二版.csv"),
    payment: "支払日.cal",
    gate: "上限.gate",
    gates_of: ("上限.gate", "上限_二版.gate"),
    rule: "区分.rule",
    rules_of: ("区分.rule", "区分_二版.rule"),
    book: "在庫.book",
    books_of: (STOCK_JA, STOCK_JA_V2),
};

const STOCK_EN: &str = "book stock v1
unit piece

account shelf(sku: string) : piece
  at least 0 refused as out_of_stock
account supplier : piece outside
account customer : piece outside

transfer receive(slip: string, sku: string, count: piece)
  key slip, sku
  move count from supplier to shelf(sku)
transfer ship(order: string, sku: string, count: piece)
  key order, sku
  move count from shelf(sku) to customer
";

/// The second version: the shelf's lower bound raised (E050), the key of `receive` narrowed (E051),
/// and a transfer added.
const STOCK_EN_V2: &str = "book stock v2
unit piece

account shelf(sku: string) : piece
  at least 3 refused as out_of_stock
account supplier : piece outside
account customer : piece outside

transfer receive(slip: string, sku: string, count: piece)
  key slip
  move count from supplier to shelf(sku)
transfer ship(order: string, sku: string, count: piece)
  key order, sku
  move count from shelf(sku) to customer
transfer take_back(slip: string, sku: string, count: piece)
  key slip, sku
  move count from customer to shelf(sku)
";

const STOCK_JA: &str = "book 在庫 v1
unit 個

account 棚(sku: string) : 個
  at least 0 refused as 在庫切れ
account 仕入先 : 個 outside
account 客 : 個 outside

transfer 入荷(伝票: string, sku: string, 数: 個)
  key 伝票, sku
  move 数 from 仕入先 to 棚(sku)
transfer 出荷(注文: string, sku: string, 数: 個)
  key 注文, sku
  move 数 from 棚(sku) to 客
";

const STOCK_JA_V2: &str = "book 在庫 v2
unit 個

account 棚(sku: string) : 個
  at least 3 refused as 在庫切れ
account 仕入先 : 個 outside
account 客 : 個 outside

transfer 入荷(伝票: string, sku: string, 数: 個)
  key 伝票
  move 数 from 仕入先 to 棚(sku)
transfer 出荷(注文: string, sku: string, 数: 個)
  key 注文, sku
  move 数 from 棚(sku) to 客
transfer 返品(伝票: string, sku: string, 数: 個)
  key 伝票, sku
  move 数 from 客 to 棚(sku)
";

/// The desk's two versions and the stock book's, written in a directory of their own.
fn desks(n: &Desk) -> TempDir {
    let t = TempDir::new(&format!("mod-diff-desk-{}", n.lang));
    let koyomi = here().join("../koyomi/tests/diff");
    let sekisho = here().join("../sekisho/tests/diff");
    let read = |p: PathBuf| std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    for (i, (v, version)) in [(n.desk.0, "1.0.0"), (n.desk.1, "1.1.0")].into_iter().enumerate() {
        t.write(&format!("{v}/ritsu.mod"), format!("module example.com/acme/desk\nversion {version}\n"));
        let pick = |p: (&'static str, &'static str)| -> &'static str { if i == 0 { p.0 } else { p.1 } };
        t.write(&format!("{v}/{}/{}", n.calendars, n.office), read(koyomi.join(pick(n.offices))));
        let days = pick(n.days);
        t.write(&format!("{v}/{}/data/{days}", n.calendars), std::fs::read(koyomi.join("data").join(days)).unwrap());
        t.write(&format!("{v}/{}/{}", n.calendars, n.payment), read(koyomi.join(n.payment)));
        // the gate of the second version reads the rule of the second version, by the rule's name in the desk
        let gate = read(sekisho.join(pick(n.gates_of))).replace(n.rules_of.1, n.rule);
        t.write(&format!("{v}/{}/{}", n.gates, n.gate), gate);
        t.write(&format!("{v}/{}/{}", n.gates, n.rule), read(sekisho.join(pick(n.rules_of))));
    }
    for (v, version, book) in [(n.stock.0, "1.0.0", n.books_of.0), (n.stock.1, "1.1.0", n.books_of.1)] {
        t.write(&format!("{v}/ritsu.mod"), format!("module example.com/acme/stock\nversion {version}\n"));
        t.write(&format!("{v}/{}/{}", n.books, n.book), book);
    }
    t
}

/// pricing from the store, 1.0.0 → 1.1.0 and 1.1.0 → 2.0.0, in the three formats, in each language
/// of the example, held to golden files.
#[test]
fn pricing_from_the_store_held_to_golden_files() {
    let mut failures = Vec::new();
    for n in [EN, JA] {
        let (t, store) = registry(&n);
        let lang = n.lang;
        for (case, old, new) in [("pricing-1.1.0", "1.0.0", "1.1.0"), ("pricing-2.0.0", "1.1.0", "2.0.0")] {
            let (old, new) = (format!("{PRICING}@{old}"), format!("{PRICING}@{new}"));
            for (format, ext) in [("text", "txt"), ("markdown", "md"), ("json", "json")] {
                let (code, out, err) = ritsu_in(t.path(), &store, &["mod", "diff", &old, &new, "--format", format, "--lang", lang]);
                assert_eq!((code, err.as_str()), (1, ""), "{case} {format} {lang}:\n{out}");
                assert!(!out.contains(&*t.path().to_string_lossy()), "{case} {format} {lang} names the test's directory:\n{out}");
                golden(&format!("tests/golden/modules/diff-{case}.{lang}.{ext}"), &masked(&out), &mut failures);
            }
        }
        let (_, out, _) = ritsu_in(t.path(), &store, &["mod", "diff", &format!("{PRICING}@1.0.0"), &format!("{PRICING}@1.1.0"), "--format", "json"]);
        let v = json(&out);
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["ritsu", "old", "new", "files", "requires", "behavior", "interface", "differs"]);
        assert_eq!((&v["old"]["module"], &v["old"]["version"], &v["new"]["version"]), (&serde_json::json!(PRICING), &serde_json::json!("1.0.0"), &serde_json::json!("1.1.0")));
        assert!(v["old"]["digest"].as_str().unwrap().starts_with("sha256:"));
        let fee = format!("{}/{}", n.rules, n.fee);
        assert_eq!(v["files"]["changed"], serde_json::json!(["ritsu.mod", fee]));
        // the step of the weight: two cells of the fee differ; the other rule answers the same
        let behavior: Vec<(&str, bool)> = v["behavior"].as_array().unwrap().iter().map(|b| (b["file"].as_str().unwrap(), b["result"]["changes"].as_array().is_some_and(|c| !c.is_empty()))).collect();
        assert!(behavior.contains(&(fee.as_str(), true)) && behavior.iter().filter(|(_, d)| *d).count() == 1, "{behavior:?}");
        // eu added in each language the rule is generated in, and the number `connect` gives the value
        // after it moved: the one change that may break a user
        let changes = v["interface"][0]["changes"].as_array().unwrap();
        let breaking: Vec<&serde_json::Value> = changes.iter().filter(|c| c["may_break"] == true).collect();
        assert_eq!(breaking.len(), 1, "{changes:?}");
        let at = breaking[0]["at"].as_str().unwrap();
        assert!(at.starts_with("/connect/enums/") && at.ends_with("/number") && breaking[0]["old"] == 2 && breaking[0]["new"] == 3, "{at}");
        let added: Vec<&str> = changes.iter().filter(|c| c["change"] == "added").map(|c| c["at"].as_str().unwrap()).collect();
        assert_eq!(added.len(), 10, "{added:?}");
        assert!(added.iter().all(|a| a.contains("/enums/") && a.rsplit('/').next() == added[0].rsplit('/').next()), "{added:?}");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The desk (koyomi, sekisho and rulec at once) and the stock book, compared as directories, in the
/// three formats, in each language of the materials, held to golden files.
#[test]
fn the_desk_and_the_book_held_to_golden_files() {
    let mut failures = Vec::new();
    for n in [DESK_EN, DESK_JA] {
        let t = desks(&n);
        let lang = n.lang;
        for (case, (old, new)) in [("desk", n.desk), ("stock", n.stock)] {
            for (format, ext) in [("text", "txt"), ("markdown", "md"), ("json", "json")] {
                let (code, out, err) = ritsu_in(t.path(), t.path(), &["mod", "diff", old, new, "--format", format, "--lang", lang]);
                assert_eq!((code, err.as_str()), (1, ""), "{case} {format} {lang}:\n{out}");
                golden(&format!("tests/golden/modules/diff-{case}.{lang}.{ext}"), &out, &mut failures);
            }
        }
        // the dates file's bytes are the same; its answers are not, for it reads the calendar
        let (_, out, _) = ritsu_in(t.path(), t.path(), &["mod", "diff", n.desk.0, n.desk.1, "--format", "json"]);
        let v = json(&out);
        let payment = format!("{}/{}", n.calendars, n.payment);
        assert!(!v["files"]["changed"].as_array().unwrap().iter().any(|f| f == payment.as_str()));
        let tools: Vec<(&str, &str)> = v["behavior"].as_array().unwrap().iter().map(|b| (b["file"].as_str().unwrap(), b["tool"].as_str().unwrap())).collect();
        let mut want = vec![
            (format!("{}/{}", n.calendars, n.office), "koyomi"),
            (payment.clone(), "koyomi"),
            (format!("{}/{}", n.gates, n.rule), "rulec"),
            (format!("{}/{}", n.gates, n.gate), "sekisho"),
        ];
        want.sort();
        assert_eq!(tools, want.iter().map(|(f, t)| (f.as_str(), *t)).collect::<Vec<_>>());
        for b in v["behavior"].as_array().unwrap() {
            assert!(b["error"].is_null(), "{b}");
        }
        // the book: E050 and E051 against the old version, at the top of its interface
        let (code, out, _) = ritsu_in(t.path(), t.path(), &["mod", "diff", n.stock.0, n.stock.1, "--format", "json"]);
        let v = json(&out);
        let book = &v["interface"][0];
        let codes: Vec<&str> = book["breaks"].as_array().unwrap().iter().map(|b| b["code"].as_str().unwrap()).collect();
        assert_eq!((code, book["tool"].as_str(), codes), (1, Some("chobo"), vec!["E050", "E051"]));
        assert_eq!(book["breaks"][0]["file"], format!("{}/{}", n.books, n.book));
        assert!(v["behavior"].as_array().unwrap().is_empty());
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The behavior of each file is what the language's own diff says of the same two files: `ritsu
/// rulec diff`, `ritsu koyomi diff` and `ritsu sekisho diff` (with the rules of rulec joined, as
/// ritsu runs sekisho), as JSON, and in its text.
#[test]
fn each_language_says_its_own_diff() {
    for n in [DESK_EN, DESK_JA] {
        let t = desks(&n);
        let (_, out, _) = ritsu_in(t.path(), t.path(), &["mod", "diff", n.desk.0, n.desk.1, "--format", "json", "--lang", n.lang]);
        let (_, text, _) = ritsu_in(t.path(), t.path(), &["mod", "diff", n.desk.0, n.desk.1, "--lang", n.lang]);
        let v = json(&out);
        for b in v["behavior"].as_array().unwrap() {
            let (file, tool) = (b["file"].as_str().unwrap(), b["tool"].as_str().unwrap());
            let (old, new) = (format!("{}/{file}", n.desk.0), format!("{}/{file}", n.desk.1));
            let (code, own, err) = ritsu_in(t.path(), t.path(), &[tool, "diff", &old, &new, "--format", "json", "--lang", n.lang]);
            assert_eq!((code, err.as_str()), (1, ""), "{tool} {file}");
            assert_eq!(b["result"], json(&own), "{tool} {file}");
            let (_, own_text, _) = ritsu_in(t.path(), t.path(), &[tool, "diff", &old, &new, "--lang", n.lang]);
            let indented: String = own_text.lines().map(|l| if l.is_empty() { "\n".to_string() } else { format!("    {l}\n") }).collect();
            assert!(text.contains(&indented), "{tool} {file}:\n{text}\n---\n{indented}");
        }
    }
}

/// Two files, in the spellings of DESIGN 17.9.1: paths, and the module at hand's `@pricing/…` against
/// `@pricing@1.0.0/…` from the store. The behavior and the interface of that file alone.
#[test]
fn two_files() {
    let (t, store) = registry(&EN);
    let fee = format!("{}/{}", EN.rules, EN.fee);
    let shop = t.path().join(EN.shop);
    for (dir, old, new) in [
        (t.path().to_path_buf(), format!("{}/{fee}", EN.pricing_100), format!("{}/{fee}", EN.pricing)),
        (shop.clone(), format!("@pricing@1.0.0/{fee}"), format!("@pricing/{fee}")),
    ] {
        let (code, out, err) = ritsu_in(&dir, &store, &["mod", "diff", &old, &new, "--format", "json"]);
        assert_eq!((code, err.as_str()), (1, ""), "{old} {new}:\n{out}");
        let v = json(&out);
        assert_eq!((&v["old"]["path"], &v["new"]["path"], &v["old"]["module"]), (&serde_json::json!(old), &serde_json::json!(new), &serde_json::Value::Null));
        assert_eq!(v["files"], serde_json::json!({"added": [], "removed": [], "changed": [new], "same": 0}));
        assert_eq!((v["behavior"][0]["tool"].as_str(), v["behavior"][0]["result"]["changes"].as_array().map(Vec::len)), (Some("rulec"), Some(2)), "{out}");
        assert_eq!((v["interface"][0]["file"].as_str(), v["interface"][0]["how"].as_str()), (Some(new.as_str()), Some("api")));
    }
    // the same file on both sides: the same answers, nothing else to say
    let (code, out, _) = ritsu_in(&shop, &store, &["mod", "diff", &format!("@pricing/{fee}"), &format!("@pricing@1.1.0/{fee}")]);
    assert_eq!(code, 0, "{out}");
    assert!(out.ends_with("\nno difference in behavior or in an interface.\n"), "{out}");
}

/// A version against itself: exit 0, from the store and from the module at hand's own tree. What
/// stops a diff, exit 2: a version the store does not have (which `ritsu mod fetch` would bring), a
/// module against a file, a directory in no module, a `--budget` that is no positive integer, and a
/// walk over `--budget` when nothing else differs.
#[test]
fn the_same_version_and_what_stops_a_diff() {
    let (t, store) = registry(&EN);
    let same = |dir: &Path, args: &[&str]| {
        let (code, out, err) = ritsu_in(dir, &store, &[&["mod", "diff"][..], args].concat());
        assert_eq!((code, err.as_str()), (0, ""), "{args:?}:\n{out}");
        assert!(out.ends_with("\nno difference in behavior or in an interface.\n"), "{out}");
    };
    let v11 = format!("{PRICING}@1.1.0");
    same(t.path(), &[&v11, &v11]);
    // the module at hand (`.`) is the version the store has, lock and all
    same(&t.path().join(EN.pricing), &[&v11]);
    same(&t.path().join(EN.pricing), &[".", "."]);

    let stops = |args: &[&str], en: &str, ja: &str| {
        let all = [&["mod", "diff"][..], args].concat();
        let (code, out, err) = ritsu_in(t.path(), &store, &all);
        assert_eq!((code, out.as_str(), err.as_str()), (2, "", en), "{args:?}");
        let (code, _, err) = ritsu_in(t.path(), &store, &[&all[..], &["--lang", "ja"]].concat());
        assert_eq!((code, err.as_str()), (2, ja), "{args:?}");
    };
    stops(
        &[&format!("{PRICING}@3.0.0"), &v11],
        "error: `example.com/acme/pricing` 3.0.0 is neither in the store nor in the module's copy of it; run `ritsu mod fetch example.com/acme/pricing@3.0.0`\n",
        "エラー: `example.com/acme/pricing` 3.0.0 が、置き場にも同梱のコピーにもありません。`ritsu mod fetch example.com/acme/pricing@3.0.0` を走らせてください\n",
    );
    let fee = format!("{}/{}/{}", EN.pricing, EN.rules, EN.fee);
    stops(
        &[&v11, &fee],
        "error: `ritsu mod diff` compares two modules (`<module>@<version>`, a directory or `.`) or two files\n",
        "エラー: `ritsu mod diff` は、二つのモジュール（`<名前>@<版>`、ディレクトリ、`.`）か、二つのファイルを比べます\n",
    );
    stops(
        &["store", &v11],
        "error: `store` is in no module: there is no `ritsu.mod` there or above it\n",
        "エラー: `store` はモジュールの中にありません（そこにも上にも `ritsu.mod` がありません）\n",
    );
    stops(&[&v11, &v11, "--budget", "0"], "error: `--budget 0` is not a positive integer\n", "エラー: `--budget 0` は正の整数ではありません\n");
    stops(&[&v11, &v11, "--budget", "many"], "error: `--budget many` is not a positive integer\n", "エラー: `--budget many` は正の整数ではありません\n");

    // a gate whose walk is over the budget, against itself: nothing differs, nothing is compared
    let d = desks(&DESK_EN);
    let gate = format!("{}/{}/{}", DESK_EN.desk.0, DESK_EN.gates, DESK_EN.gate);
    let (code, out, err) = ritsu_in(d.path(), d.path(), &["mod", "diff", &gate, &gate, "--budget", "10"]);
    assert_eq!((code, err.as_str()), (2, ""), "{out}");
    assert!(out.contains(&format!("  {gate} (sekisho): not compared (the walk of `refund_order` over both versions has 12 combinations, over the budget of 10; give a larger `--budget`)\n")), "{out}");
    assert!(out.ends_with("\nthe behavior of some files could not be compared, so this does not say there is no difference.\n"), "{out}");
    // with a budget written with separators, it is compared
    let (code, out, _) = ritsu_in(d.path(), d.path(), &["mod", "diff", &gate, &gate, "--budget", "1_000_000"]);
    assert_eq!(code, 0, "{out}");
}
