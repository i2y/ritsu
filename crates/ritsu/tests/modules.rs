//! `ritsu check` of a module (DESIGN 17.8, 17.20, 17.21; PLAN B.7): the example `shop` checked with
//! its dependencies in English, in Japanese and as JSON; an error in a dependency, which fails the
//! check; the warnings of the dependencies, counted on a line of their own and shown one by one with
//! `--deps-warnings`; the claims of a dependency's `.geas`, which are not run; W303 for a path
//! through a dependency to a module the module does not require, and W301 for a `require` no file
//! uses (an import of a `.proto` counts for both); the contributions that add to a rule of another
//! module, each said in a note, and one that breaks the dependency's rule it adds to, which fails the
//! module and not the dependency; a tree of paths, which needs no lock; a tree with modules from a registry, held to its lock (E303, E304,
//! E305, W302), which stops when a module is not in the store or a newer ritsu is asked for (exit
//! 2); a walk that starts in no module, which passes over a module below and says so, in `ritsu
//! check` and in `ritsu run --with`; and a path given on the command line that starts with `@`, whose
//! errors are said as a path written in a file is (DESIGN 17.6). `ritsu run` of the example's flow,
//! held to golden files, and stopped by a tree that cannot be read; `ritsu run --with` the root of a
//! module, which merges the contributions of the whole tree; the page `ritsu rulec doc` draws of a
//! dependency's rule, which names the module of the rule it applies; and the page each language draws
//! of a file that names a dependency's, which names the module of that file. The seed module of the
//! repository (`modules/holidays`, DESIGN 17.18): its data files and their pins, which are koyomi's
//! example's, its check, held to golden files, and the workflow that publishes it, which runs by
//! hand only.

use ritsu_base::modules::lock::{Lock, Locked};
use ritsu_base::modules::semver::Version;
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
    let t = TempDir::new("modules");
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

/// The last line of the text, the summary, and the line of the dependencies before it.
fn last_lines(out: &str, n: usize) -> Vec<&str> {
    let lines: Vec<&str> = out.lines().collect();
    lines[lines.len().saturating_sub(n)..].to_vec()
}

fn json(out: &str) -> serde_json::Value {
    serde_json::from_str(out).unwrap_or_else(|e| panic!("{e}: {out}"))
}

/// The example's `shop`, and its twin with Japanese names (`店`), checked as the whole module: the
/// files of `pricing` and of `tax` (through `pricing`) are checked with the module's own, and said
/// on a line of their own. The text in both languages and the JSON are held to golden files. Each
/// passes; the tree is of paths, so there is no `ritsu.lock`.
#[test]
fn the_example_module_is_checked_with_its_dependencies() {
    let empty = TempDir::new("modules-no-store");
    let mut failures = Vec::new();
    for (dir, name) in [("tests/projects/modules/shop", "shop"), ("tests/projects/modules.ja/店", "店")] {
        let d = here().join(dir);
        assert!(!d.join("ritsu.lock").exists(), "a tree of paths needs no lock");
        for lang in ["en", "ja"] {
            let (code, out, err) = ritsu_in(&d, empty.path(), &["check", "--lang", lang]);
            assert_eq!(code, 0, "{out}{err}");
            assert!(err.is_empty(), "{err}");
            golden(&format!("tests/golden/modules/{name}.{lang}.txt"), &out, &mut failures);
        }
        let (code, out, _) = ritsu_in(&d, empty.path(), &["check", "--format", "json"]);
        assert_eq!(code, 0);
        golden(&format!("tests/golden/modules/{name}.json"), &out, &mut failures);
        let v = json(&out);
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["ritsu", "root", "ok", "module", "dependencies", "files", "diagnostics", "contributions", "borders"]);
        for f in v["files"].as_array().unwrap() {
            let file = f["file"].as_str().unwrap();
            let keys: Vec<&str> = f.as_object().unwrap().keys().map(String::as_str).collect();
            assert_eq!(keys, ["tool", "file", "dependency", "ok"]);
            // the `require` names are the same in both: `pricing`, and `tax` in `pricing`
            let want = if file.starts_with("@pricing/@tax/") {
                serde_json::json!("@pricing/@tax")
            } else if file.starts_with("@pricing/") {
                serde_json::json!("@pricing")
            } else {
                serde_json::Value::Null
            };
            assert_eq!(f["dependency"], want, "{file}");
        }
    }
    // Checked from above the module, the files are written from where the run is.
    let (code, out, err) = ritsu_in(&here().join("tests/projects/modules"), empty.path(), &["check", "shop"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        out,
        "ok shop/checkout/basket.kn: 2 functions, total\nshop/orders/place_order.flow: ok\ndependencies: 2 modules, 3 files (rulec 3): all pass\nritsu check: 2 files (kaname 1, dandori 1): all pass; borders between the languages: 0 checked, 0 undecided\n"
    );
    // A part of the module: the files given, and what they reach, as before there were modules.
    let (code, out, _) = ritsu_in(&here().join("tests/projects/modules/shop"), empty.path(), &["check", "orders"]);
    assert_eq!(code, 0);
    assert_eq!(out, "orders/place_order.flow: ok\nritsu check: 1 file (dandori 1): all pass; borders between the languages: 0 checked, 0 undecided\n");
    let (_, out, _) = ritsu_in(&here().join("tests/projects/modules/shop"), empty.path(), &["check", "orders", "--format", "json"]);
    let v = json(&out);
    assert_eq!(v["files"][0]["dependency"], serde_json::Value::Null);
    assert_eq!(v["dependencies"].as_array().unwrap().len(), 2, "the dependencies of the tree are listed in a partial check too");
    assert_eq!(v["dependencies"][0]["files"], 0);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// `ritsu run` of the example's flow (DESIGN 17.8): the flow prices an order by the rules of
/// `pricing`, read as `@pricing/…` (and `tax` through `pricing`), and each scenario runs to its end
/// with the rules' answers. The text in the language of the version and the JSON are held to golden
/// files; `--with` the root of the module merges the contributions of the whole tree, as `ritsu
/// check` of the root does, and the run is the same (the example has none). A tree that cannot be
/// read stops the run as it stops `ritsu check` (exit 1), in the text and in the JSON.
#[test]
fn the_example_flow_runs_with_the_rules_of_its_dependencies() {
    let empty = TempDir::new("modules-run-no-store");
    let mut failures = Vec::new();
    let versions = [
        ("tests/projects/modules/shop", "orders/place_order.flow", [("charged", "charged"), ("not_charged", "not_charged")], "en"),
        ("tests/projects/modules.ja/店", "注文/注文を受ける.flow", [("請求が通る", "charged"), ("請求が通らない", "not_charged")], "ja"),
    ];
    for (dir, flow, scenarios, lang) in versions {
        let d = here().join(dir);
        let at = if lang == "ja" { "注文/シナリオ" } else { "orders/scenarios" };
        for (scenario, name) in scenarios {
            let sc = format!("{at}/{scenario}.json");
            let (code, out, err) = ritsu_in(&d, empty.path(), &["run", flow, "--scenario", &sc, "--lang", lang]);
            assert_eq!((code, err.as_str()), (0, ""), "{flow} {sc}: {out}");
            golden(&format!("tests/golden/modules/run-{name}.{lang}.txt"), &out, &mut failures);
            let (code, out, err) = ritsu_in(&d, empty.path(), &["run", flow, "--scenario", &sc, "--format", "json", "--lang", lang]);
            assert_eq!((code, err.as_str()), (0, ""), "{flow} {sc}");
            golden(&format!("tests/golden/modules/run-{name}.{lang}.json"), &out, &mut failures);
            let (code, with, _) = ritsu_in(&d, empty.path(), &["run", flow, "--scenario", &sc, "--format", "json", "--lang", lang, "--with", "."]);
            assert_eq!((code, with), (0, out), "--with the root: {flow} {sc}");
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // a tree that cannot be read
    let t = example();
    replace(&t, "shop/ritsu.mod", "require pricing path \"../pricing\"", "require pricing 1.1.0");
    let shop = t.path().join("shop");
    let args = ["run", "orders/place_order.flow", "--scenario", "orders/scenarios/charged.json"];
    let (code, out, err) = ritsu_in(&shop, t.path(), &args);
    assert_eq!((code, out.as_str()), (1, ""));
    assert!(err.starts_with("error[ritsu E301]: ritsu.mod:"), "{err}");
    assert!(err.ends_with("error: the modules could not be read (1 error), so the flow is not run\n"), "{err}");
    let (_, _, err) = ritsu_in(&shop, t.path(), &[&args[..], &["--lang", "ja"]].concat());
    assert!(err.ends_with("エラー: モジュールを読めません（エラー 1 件）。フローは流しません\n"), "{err}");
    let (code, out, _) = ritsu_in(&shop, t.path(), &[&args[..], &["--format", "json"]].concat());
    let v = json(&out);
    assert_eq!((code, &v["diagnostics"][0]["code"], &v["diagnostics"][0]["file"]), (1, &serde_json::json!("E301"), &serde_json::json!("ritsu.mod")));
}

/// `ritsu rulec doc` of a file of a dependency given as `@pricing/…` (DESIGN 17.8) draws the page of
/// the dependency's rule. The rule of `tax` it applies is not beside the page, so the page names it
/// as `pricing` writes it, with the module and version of `tax`, and no link. The Markdown and the
/// HTML are held to golden files, and the Japanese page says the same. `pricing`'s own page of the
/// rule is the same page, but for the path of the file it is drawn from.
#[test]
fn the_page_of_a_dependencys_rule_names_the_module_of_what_it_applies() {
    let empty = TempDir::new("modules-doc-no-store");
    let shop = here().join("tests/projects/modules/shop");
    let mut failures = Vec::new();
    let page = ["rulec", "doc", "@pricing/rules/order_total.rule"];
    let (code, md, err) = ritsu_in(&shop, empty.path(), &page);
    assert_eq!((code, err.as_str()), (0, ""));
    golden("tests/golden/modules/doc-order_total.md", &md, &mut failures);
    assert!(md.contains("\n## Applied rule vat: @tax/rules/vat_rate.rule (module example.com/acme/tax 1.0.0) (unpinned)\n"), "{md}");
    let (code, html, err) = ritsu_in(&shop, empty.path(), &[&page[..], &["--format", "html"]].concat());
    assert_eq!((code, err.as_str()), (0, ""));
    golden("tests/golden/modules/doc-order_total.html", &html, &mut failures);
    let (_, ja, _) = ritsu_in(&shop, empty.path(), &[&page[..], &["--lang", "ja"]].concat());
    assert!(ja.contains("\n## 準用 vat: @tax/rules/vat_rate.rule（モジュール example.com/acme/tax 1.0.0）（固定なし）\n"), "{ja}");
    // pricing's own page
    let (_, own, _) = ritsu_in(&here().join("tests/projects/modules/pricing"), empty.path(), &["rulec", "doc", "rules/order_total.rule"]);
    assert_eq!(own.replacen("from rules/order_total.rule", "from @pricing/rules/order_total.rule", 1), md);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The page each language draws of a file of a module that names a file of a dependency (DESIGN
/// 17.8): where the page names the files it reads, the dependency's file is written as the module
/// writes it, with the module and its version beside it and no link, in English and in Japanese, in
/// the Markdown and in the HTML. A file of the module's own is named as before. The material is each
/// language's own: `crates/<language>/tests/modules/en/app`, which requires `lib`, which requires
/// `base`.
#[test]
fn the_page_of_each_language_names_the_module_of_a_dependencys_file() {
    let empty = TempDir::new("modules-doc-languages");
    // the language, the file of `app`, and a line of its page in English and in Japanese
    let cases: [(&str, &str, &str, &str); 7] = [
        (
            "rulec",
            "rules/invoice.rule",
            "## Applied rule order: @lib/rules/order_total.rule (module example.com/acme/lib 1.0.0) (unpinned)",
            "## 準用 order: @lib/rules/order_total.rule（モジュール example.com/acme/lib 1.0.0）（固定なし）",
        ),
        (
            "dandori",
            "orders/place_order.flow",
            "<summary><code>total</code> · order_total v1 (module example.com/acme/lib 1.0.0) · <code>@lib/rules/order_total.rule</code></summary>",
            "<summary><code>total</code> · order_total v1（モジュール example.com/acme/lib 1.0.0） · <code>@lib/rules/order_total.rule</code></summary>",
        ),
        (
            "kaname",
            "checkout/basket.kn",
            "- Reads: the rule `order_total` (`@lib/rules/order_total.rule`, module example.com/acme/lib 1.0.0); the book `ledger` (`@lib/books/ledger.book`, module example.com/acme/lib 1.0.0); the module `charges` (`@lib/checkout/charges.kn`, module example.com/acme/lib 1.0.0)",
            "- 読むファイル：規則 `order_total`（`@lib/rules/order_total.rule`、モジュール example.com/acme/lib 1.0.0）、帳簿 `ledger`（`@lib/books/ledger.book`、モジュール example.com/acme/lib 1.0.0）、モジュール `charges`（`@lib/checkout/charges.kn`、モジュール example.com/acme/lib 1.0.0）",
        ),
        (
            "koyomi",
            "dates/net10.cal",
            "- Table: closures = `@lib/@base/data/closures.csv` (sha256:a9dedfd32fd6e08f, module example.com/acme/base 1.0.0, covers 2026-01-01..2027-12-31)",
            "- 表: closures = `@lib/@base/data/closures.csv`（sha256:a9dedfd32fd6e08f、モジュール example.com/acme/base 1.0.0、covers 2026-01-01..2027-12-31）",
        ),
        (
            "sekisho",
            "gates/purchases.gate",
            "<summary>The page of <code>@lib/rules/approval.rule</code> (module example.com/acme/lib 1.0.0), as <code>rulec doc</code> draws it</summary>",
            "<summary><code>rulec doc</code> が描いた <code>@lib/rules/approval.rule</code>（モジュール example.com/acme/lib 1.0.0）のページ</summary>",
        ),
        (
            "yuen",
            "billing.req",
            "The file `@lib/docs/pricing_policy.md` (module example.com/acme/lib 1.0.0), pinned at `sha256:24746859693385ce`.",
            "ファイル `@lib/docs/pricing_policy.md`（モジュール example.com/acme/lib 1.0.0）、固定は `sha256:24746859693385ce`。",
        ),
        ("sakai", "shop.ctx", "- Written in: `@lib/contexts/pricing.ctx` (module example.com/acme/lib 1.0.0)", "- 書いたファイル：`@lib/contexts/pricing.ctx`（モジュール example.com/acme/lib 1.0.0）"),
    ];
    for (language, file, en, ja) in cases {
        let app = here().join("..").join(language).join("tests/modules/en/app");
        let (code, md, err) = ritsu_in(&app, empty.path(), &[language, "doc", file]);
        assert_eq!(code, 0, "{language}: {err}");
        assert!(md.lines().any(|l| l == en), "{language}: no line `{en}` in\n{md}");
        let (code, md, err) = ritsu_in(&app, empty.path(), &[language, "doc", file, "--lang", "ja"]);
        assert_eq!(code, 0, "{language}: {err}");
        assert!(md.lines().any(|l| l == ja), "{language}: no line `{ja}` in\n{md}");
        let (code, html, err) = ritsu_in(&app, empty.path(), &[language, "doc", file, "--format", "html"]);
        assert_eq!(code, 0, "{language}: {err}");
        assert!(html.contains("module example.com/acme/lib 1.0.0") || html.contains("module example.com/acme/base 1.0.0"), "{language}: the HTML page names no module");
        assert!(!html.contains("href=\"@lib/") && !html.contains("href=\"../@lib/"), "{language}: the HTML page links to a dependency's file");
    }
    // a file of the module's own is named as before
    let (_, md, _) = ritsu_in(&here().join("../sakai/tests/modules/en/app"), empty.path(), &["sakai", "doc", "shop.ctx"]);
    assert!(md.lines().any(|l| l == "- Written in: `contexts/billing.ctx`"), "{md}");
}

/// An error in a file of a dependency is said as the module's own are, and fails the check (exit 1):
/// on the line of the dependencies, and in the module's own files that read it.
#[test]
fn an_error_in_a_dependency_fails_the_check() {
    let t = example();
    replace(&t, "pricing/rules/shipping_fee.rule", "| 2500g   | domestic | 395GBPc  |", "| 2500g   | domestic | 999GBPc  |");
    let shop = t.path().join("shop");
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.starts_with("error[rulec E107]: Example does not hold: fee should be 999GBPc but came out as 395GBPc\n  --> @pricing/rules/shipping_fee.rule:27 examples\n"), "{out}");
    assert!(out.contains("error[dandori E005]: orders/place_order.flow:7:10: could not read the rule `@pricing/rules/shipping_fee.rule`"), "{out}");
    assert_eq!(
        last_lines(&out, 2),
        [
            "dependencies: 2 modules, 3 files (rulec 3): 1 fail (1 error)",
            "ritsu check: 2 files (kaname 1, dandori 1): 1 fail (4 errors); borders between the languages: 0 checked, 0 undecided"
        ]
    );
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check", "--lang", "ja"]);
    assert_eq!(code, 1);
    assert_eq!(last_lines(&out, 2)[0], "依存: モジュール 2 個、ファイル 3 個（rulec 3）。検査を通らないもの 1 個（エラー 1 件）");
    let (_, out, _) = ritsu_in(&shop, t.path(), &["check", "--format", "json"]);
    let v = json(&out);
    assert_eq!(v["ok"], false);
    let pricing = &v["dependencies"][0];
    assert_eq!((pricing["prefix"].as_str(), pricing["ok"].as_bool(), pricing["errors"].as_i64()), (Some("@pricing"), Some(false), Some(1)));
    let e107 = v["diagnostics"].as_array().unwrap().iter().find(|d| d["code"] == "E107").unwrap();
    // the dependency right after the file, wherever the language's JSON puts its file
    let keys: Vec<&str> = e107.as_object().unwrap().keys().map(String::as_str).collect();
    let at = keys.iter().position(|k| *k == "file").unwrap();
    assert_eq!((keys[0], keys[at + 1]), ("tool", "dependency"));
    assert_eq!((e107["file"].as_str(), e107["dependency"].as_str()), (Some("@pricing/rules/shipping_fee.rule"), Some("@pricing")));
}

/// A rule of `tax` with two warnings (W111, W105): counted for `@pricing/@tax` on the line of the
/// dependencies and left out of the text and the JSON's diagnostics, unless `--deps-warnings` asks
/// for each. A warning does not fail the check.
#[test]
fn the_warnings_of_the_dependencies_are_counted_and_shown_when_asked() {
    let t = example();
    t.write(
        "tax/rules/promo.rule",
        "rule promo v1\ndescription \"A discount by the kind of a customer\"\n\nenum kind = new | regular\n\ninputs\n  kind  : kind\n  large : bool\n\noutputs\n  off : money[GBPc]  round down(1GBPc)\n\ntable off_by_kind\npolicy first\n| kind    | large | -> off : money[GBPc] |\n| new     | -     | 100GBPc              |\n| -       | true  | 200GBPc              |\n| -       | -     | 0GBPc                |\n\nexamples\n| kind    | large | -> off   |\n| new     | false | 100GBPc  |\n| regular | true  | 200GBPc  |\n| regular | false | 0GBPc    |\n",
    );
    let shop = t.path().join("shop");
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check"]);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("warning["), "{out}");
    assert!(!out.contains("ok @"), "the lines of the dependencies' files that pass are left out: {out}");
    assert_eq!(
        last_lines(&out, 2),
        [
            "dependencies: 2 modules, 4 files (rulec 4): all pass (2 warnings); warnings by dependency: @pricing/@tax 2 (`--deps-warnings` shows each)",
            "ritsu check: 2 files (kaname 1, dandori 1): all pass; borders between the languages: 0 checked, 0 undecided"
        ]
    );
    let (_, out, _) = ritsu_in(&shop, t.path(), &["check", "--lang", "ja"]);
    assert_eq!(
        last_lines(&out, 2)[0],
        "依存: モジュール 2 個、ファイル 4 個（rulec 4）。どれも検査を通りました（警告 2 件）。依存ごとの警告: @pricing/@tax 2 件（一つずつは `--deps-warnings` で出ます）"
    );
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check", "--deps-warnings"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("warning[rulec W111]: Values of type kind appear in no row\n  --> @pricing/@tax/rules/promo.rule:4\n"), "{out}");
    assert!(out.contains("warning[rulec W105]: "), "{out}");
    assert!(!out.contains("ok @"), "{out}");
    assert_eq!(last_lines(&out, 2)[0], "dependencies: 2 modules, 4 files (rulec 4): all pass (2 warnings); warnings by dependency: @pricing/@tax 2");
    for (flag, shown) in [(None, 0), (Some("--deps-warnings"), 2)] {
        let mut args = vec!["check", "--format", "json"];
        args.extend(flag);
        let (_, out, _) = ritsu_in(&shop, t.path(), &args);
        let v = json(&out);
        assert_eq!(v["ok"], true);
        assert_eq!(v["dependencies"][1]["warnings"], 2);
        let ws: Vec<&serde_json::Value> = v["diagnostics"].as_array().unwrap().iter().filter(|d| d["dependency"] == "@pricing/@tax").collect();
        assert_eq!(ws.len(), shown, "{flag:?}");
    }
    // Checked as the module at hand, `tax` says them as its own.
    let (_, out, _) = ritsu_in(&t.path().join("tax"), t.path(), &["check"]);
    assert!(out.contains("warning[rulec W105]: ") && out.ends_with("ritsu check: 2 files (rulec 2): all pass (2 warnings); borders between the languages: 0 checked, 0 undecided\n"), "{out}");
}

/// The claims of a dependency's `.geas` are not run: the program its target serves would leave a
/// file if it ran, and the file is not there. The line of the claims not run counts them; a path
/// that names only such a file has nothing to check (exit 2).
#[test]
fn the_claims_of_a_dependency_are_not_run() {
    let t = example();
    let mark = t.path().join("ran");
    t.write(
        "pricing/specs/api.geas",
        format!("# A claim on the pricing service: its program leaves a file when it runs.\ntarget api {{\n  serve \"sh -c 'touch {}' {{port}}\"\n  port auto\n}}\n\nclaim \"the service answers\" {{\n  when api.get(\"/\")\n  then status is 200\n}}\n", mark.display()),
    );
    let shop = t.path().join("shop");
    let (code, out, err) = ritsu_in(&shop, t.path(), &["check"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!mark.exists(), "the claim was run");
    assert_eq!(
        last_lines(&out, 3),
        [
            "dependencies: 2 modules, 3 files (rulec 3): all pass",
            "dependencies: 1 claim in 1 .geas file, not run: a dependency's claims are about its author's programs",
            "ritsu check: 2 files (kaname 1, dandori 1): all pass; borders between the languages: 0 checked, 0 undecided"
        ]
    );
    let (_, out, _) = ritsu_in(&shop, t.path(), &["check", "--lang", "ja"]);
    assert_eq!(last_lines(&out, 2)[0], "依存: .geas のファイル 1 個の主張 1 件は、走らせていません。依存の主張は、依存の作者のプログラムについてのものです");
    let (_, out, _) = ritsu_in(&shop, t.path(), &["check", "--format", "json"]);
    let v = json(&out);
    assert_eq!((v["dependencies"][0]["prefix"].as_str(), v["dependencies"][0]["claims_not_run"].as_i64()), (Some("@pricing"), Some(1)));
    assert!(!v["files"].as_array().unwrap().iter().any(|f| f["tool"] == "geas"), "{out}");
    // a path that names it alone
    let (code, out, err) = ritsu_in(&shop, t.path(), &["check", "@pricing/specs/api.geas"]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert_eq!(err, "error: @pricing/specs/api.geas holds only .geas files of dependencies, whose claims are not run: there is nothing to check\n");
    // a part of the dependency: its rules are checked, its claims are not run
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check", "@pricing"]);
    assert_eq!(code, 0);
    assert_eq!(
        out,
        "ok @pricing/rules/order_total.rule\nok @pricing/rules/shipping_fee.rule\ndependencies: 1 claim in 1 .geas file, not run: a dependency's claims are about its author's programs\nritsu check: 2 files (rulec 2): all pass; borders between the languages: 0 checked, 0 undecided\n"
    );
    assert!(!mark.exists(), "the claim was run");
}

/// A file of `shop` that writes `@pricing/@tax/…` reaches `tax` through `pricing`, which `shop` does
/// not require: W303, in a check of the whole module and of the file alone. Once `shop` requires
/// `tax`, the path is `@tax/…` by the fewest `@`, and nothing is said.
#[test]
fn w303_for_a_path_through_a_dependency_to_a_module_not_required() {
    let t = example();
    t.write(
        "shop/checkout/vat.kn",
        "# The VAT on a basket, by the rule of the module tax, which this module reaches through pricing.\nmodule vat v1\ndescription \"The VAT on a basket's subtotal, at the rate of its category\"\n\nuse rule vat_rate from \"@pricing/@tax/rules/vat_rate.rule\"\n\nfn vat(subtotal: money[GBPc] range >=0GBPc <=10_000GBPc, category: vat_rate.category) -> money[GBPc]\n  description \"The VAT on the subtotal\"\n  half_up(subtotal × vat_rate(category: category).rate, 1GBPc)\n\nexamples\n  vat(subtotal: 1000GBPc, category: vat_rate.category.standard) = 200GBPc\n",
    );
    let shop = t.path().join("shop");
    let w303 = "warning[ritsu W303]: checkout/vat.kn:5:1: `@pricing/@tax/rules/vat_rate.rule` reaches a module this module does not require; require it, so that a version of pricing that drops it does not break this file\n     5 | use rule vat_rate from \"@pricing/@tax/rules/vat_rate.rule\"\n";
    for args in [vec!["check"], vec!["check", "checkout/vat.kn"]] {
        let (code, out, _) = ritsu_in(&shop, t.path(), &args);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains(w303), "{args:?}: {out}");
    }
    let (_, out, _) = ritsu_in(&shop, t.path(), &["check", "--lang", "ja"]);
    assert!(
        out.contains("警告[ritsu W303]: checkout/vat.kn:5:1: `@pricing/@tax/rules/vat_rate.rule` は、このモジュールが `require` していないモジュールを指しています。pricing の新しい版が tax を使わなくなると、このファイルは壊れます。`require` に足してください\n"),
        "{out}"
    );
    let (_, out, _) = ritsu_in(&shop, t.path(), &["check", "--format", "json"]);
    let v = json(&out);
    let d = v["diagnostics"].as_array().unwrap().iter().find(|d| d["code"] == "W303").unwrap();
    assert_eq!((d["tool"].as_str(), d["file"].as_str(), d["dependency"].clone(), d["line"].as_i64()), (Some("ritsu"), Some("checkout/vat.kn"), serde_json::Value::Null, Some(5)));
    // required: the path is `@tax/…` now, and the `require` is used
    replace(&t, "shop/ritsu.mod", "require pricing path \"../pricing\"\n", "require pricing path \"../pricing\"\nrequire tax path \"../tax\"\n");
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check"]);
    assert_eq!(code, 0);
    assert!(!out.contains("W303") && !out.contains("W301"), "{out}");
}

/// A `require` that no file of the module uses is W301, at its line of `ritsu.mod`, in a check of
/// the whole module; a part of the module does not say it (what the rest names is not read).
#[test]
fn w301_for_a_require_no_file_uses() {
    let t = example();
    replace(&t, "shop/ritsu.mod", "require pricing path \"../pricing\"\n", "require pricing path \"../pricing\"\nrequire tax path \"../tax\"\n");
    let shop = t.path().join("shop");
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains(
            "warning[ritsu W301]: ritsu.mod:8:9: No file of this module uses its dependency `tax` (example.com/acme/tax): none names a file of `@tax/…`\n     8 | require tax path \"../tax\"\n  = If the module does not use it, delete the `require` (`ritsu mod remove tax`); if it is to, name a file of `@tax/…` where it does.\n"
        ),
        "{out}"
    );
    // `pricing` uses `tax`: its own `require` is not said
    assert_eq!(out.matches("W301").count(), 1, "{out}");
    assert!(out.ends_with("ritsu check: 2 files (kaname 1, dandori 1): all pass (1 warning); borders between the languages: 0 checked, 0 undecided\n"), "{out}");
    let (_, out, _) = ritsu_in(&shop, t.path(), &["check", "--lang", "ja"]);
    assert!(
        out.contains("警告[ritsu W301]: ritsu.mod:8:9: このモジュールのどのファイルも、依存 `tax`（example.com/acme/tax）を使っていません。`@tax/…` のファイルを指すものがありません\n"),
        "{out}"
    );
    assert!(out.contains("  = 使わないなら、`require` の行を消してください（`ritsu mod remove tax`）。使うのなら、使うところで `@tax/…` のファイルを指してください。\n"), "{out}");
    let (_, out, _) = ritsu_in(&shop, t.path(), &["check", "orders"]);
    assert!(!out.contains("W301"), "{out}");
    // a `require` of a dependency that no file of it uses: the dependency's warning, counted
    replace(&t, "tax/ritsu.mod", "license \"MIT OR Apache-2.0\"\n", "license \"MIT OR Apache-2.0\"\n\nrequire pricing path \"../pricing\"\n");
    let (_, out, _) = ritsu_in(&shop, t.path(), &["check"]);
    assert!(out.contains("; warnings by dependency: @tax 1 (`--deps-warnings` shows each)\n"), "{out}");
    let (_, out, _) = ritsu_in(&shop, t.path(), &["check", "--deps-warnings"]);
    assert!(out.contains("warning[ritsu W301]: @tax/ritsu.mod:7:9: No file of this module uses its dependency `pricing` (example.com/acme/pricing)"), "{out}");
}

/// A `require` that only a `.proto` of the module uses, by importing a `.proto` of the dependency by
/// the path of its package (DESIGN 17.6), is used: W301 is not said. Once the `.proto` imports
/// nothing of it, no file of the module uses it, and W301 is said. Without the `require`, the import
/// is read from the dependency's dependency, through pricing: W303, on the line of the import.
#[test]
fn w301_and_w303_count_the_imports_of_a_proto() {
    let t = example();
    replace(&t, "shop/ritsu.mod", "require pricing path \"../pricing\"\n", "require pricing path \"../pricing\"\nrequire tax path \"../tax\"\n");
    t.write("tax/proto/acme/tax/v1/rate.proto", "// A rate of VAT as the services of the Acme shop carry it, in whole percent.\nsyntax = \"proto3\";\n\npackage acme.tax.v1;\n\nmessage Rate {\n  int32 percent = 1;\n}\n");
    let receipt = "// A receipt of the shop, with the rate of VAT of the module tax, imported by the path of its package.\nsyntax = \"proto3\";\n\npackage acme.shop.v1;\n\nimport \"acme/tax/v1/rate.proto\";\n\nmessage Receipt {\n  string order_id = 1;\n  acme.tax.v1.Rate vat = 2;\n}\n";
    t.write("shop/proto/acme/shop/v1/receipt.proto", receipt);
    let shop = t.path().join("shop");
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check"]);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("W301") && !out.contains("W303"), "{out}");
    // the `.proto` imports nothing of tax
    t.write("shop/proto/acme/shop/v1/receipt.proto", receipt.replace("import \"acme/tax/v1/rate.proto\";\n\n", "").replace("  acme.tax.v1.Rate vat = 2;\n", "  int32 vat_percent = 2;\n"));
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("warning[ritsu W301]: ritsu.mod:8:9: No file of this module uses its dependency `tax` (example.com/acme/tax): none names a file of `@tax/…`\n"), "{out}");
    // the import, with no `require` of tax: what it reads is a file of pricing's dependency
    replace(&t, "shop/ritsu.mod", "require tax path \"../tax\"\n", "");
    t.write("shop/proto/acme/shop/v1/receipt.proto", receipt);
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains("warning[ritsu W303]: proto/acme/shop/v1/receipt.proto:6:1: `@pricing/@tax/proto/acme/tax/v1/rate.proto` reaches a module this module does not require; require it, so that a version of pricing that drops it does not break this file\n"),
        "{out}"
    );
    assert!(!out.contains("W301"), "{out}");
}

/// An open rule of `pricing` with a contribution of `shop`, and an open rule of `tax` with a
/// contribution of `pricing`: rulec gathers the contributions over the whole tree, so each rule is
/// checked with what adds to it (the contributions' examples run on the rules they add to), and each
/// contribution that crosses from one module to another is noted before the summary, and listed in
/// the JSON. An error rulec finds in a line of the module's contribution, while it checks the
/// dependency's rule, is the module's own.
#[test]
fn contributions_across_the_borders_of_modules_are_noted() {
    let t = example();
    t.write(
        "pricing/rules/order_discount.rule",
        "# The discount of an order: what the campaigns of the shop add, up to its subtotal.\nrule order_discount v1\ndescription \"The discount of an order: the sum of what the campaigns add, up to the subtotal\"\n\ninputs\n  subtotal : money[GBPc]  range >=0GBPc <=2_000_000GBPc\n\noutputs\n  discount : money[GBPc]  round down(1GBPc)\n\n# What the campaigns take off, from files of their own: nothing when none does.\nopen campaigns : money[GBPc]  combine sum  empty 0GBPc  range >=0GBPc <=10_000GBPc\n\ndefine discount : money[GBPc] = min(campaigns, subtotal)\n\nexamples\n| subtotal | -> discount |\n| 1000GBPc | 0GBPc       |\n",
    );
    let sale = "# A campaign of the shop, added to the rule order_discount of the module pricing.\ncontribution spring_sale v1\ndescription \"A spring sale: 100 pence off every order\"\nto \"@pricing/rules/order_discount.rule\" signature:67a223ec46dce359\n\ndefine sale_off : money[GBPc] = 100GBPc\n\ninto campaigns\n  sale_off\n\nexamples\n| subtotal | -> discount |\n| 1000GBPc | 100GBPc     |\n| 50GBPc   | 50GBPc      |\n";
    t.write("shop/rules/spring_sale.rule", sale);
    t.write(
        "tax/rules/surcharge.rule",
        "# A surcharge on an order: what the modules that use tax add, up to 10% of the subtotal.\nrule surcharge v1\ndescription \"The surcharge on an order: the sum of what is added, up to a tenth of the subtotal\"\n\ninputs\n  subtotal : money[GBPc]  range >=0GBPc <=2_000_000GBPc\n\noutputs\n  surcharge : money[GBPc]  round down(1GBPc)\n\n# What is added, from files of their own: nothing when none adds.\nopen added : money[GBPc]  combine sum  empty 0GBPc  range >=0GBPc <=10_000GBPc\n\ndefine surcharge : money[GBPc] = min(added, subtotal / 10)\n\nexamples\n| subtotal | -> surcharge |\n| 1000GBPc | 0GBPc        |\n",
    );
    t.write(
        "pricing/rules/handling.rule",
        "# The handling charge of pricing, added to the surcharge of the module tax.\ncontribution handling v1\ndescription \"A handling charge of 50 pence on every order\"\nto \"@tax/rules/surcharge.rule\" signature:dc63f201ffae8b92\n\ndefine handling : money[GBPc] = 50GBPc\n\ninto added\n  handling\n\nexamples\n| subtotal | -> surcharge |\n| 1000GBPc | 50GBPc       |\n| 100GBPc  | 10GBPc       |\n",
    );
    let shop = t.path().join("shop");
    let (code, out, err) = ritsu_in(&shop, t.path(), &["check"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        out,
        "ok rules/spring_sale.rule (contributes to @pricing/rules/order_discount.rule)\nok checkout/basket.kn: 2 functions, total\norders/place_order.flow: ok\nnote: `@pricing/rules/handling.rule`, a contribution of the dependency `@pricing` (example.com/acme/pricing 1.1.0), adds to `@pricing/@tax/rules/surcharge.rule`, a rule of the dependency `@pricing/@tax` (example.com/acme/tax 1.0.0)\nnote: `rules/spring_sale.rule`, a contribution of this module, adds to `@pricing/rules/order_discount.rule`, a rule of the dependency `@pricing` (example.com/acme/pricing 1.1.0)\ndependencies: 2 modules, 6 files (rulec 6): all pass\nritsu check: 3 files (rulec 1, kaname 1, dandori 1): all pass; borders between the languages: 0 checked, 0 undecided\n"
    );
    let (_, out, _) = ritsu_in(&shop, t.path(), &["check", "--lang", "ja"]);
    assert!(
        out.contains("備考: 依存 `@pricing`（example.com/acme/pricing 1.1.0）の寄与 `@pricing/rules/handling.rule` は、依存 `@pricing/@tax`（example.com/acme/tax 1.0.0）の規則 `@pricing/@tax/rules/surcharge.rule` に足しています\n備考: このモジュールの寄与 `rules/spring_sale.rule` は、依存 `@pricing`（example.com/acme/pricing 1.1.0）の規則 `@pricing/rules/order_discount.rule` に足しています\n"),
        "{out}"
    );
    let (_, out, _) = ritsu_in(&shop, t.path(), &["check", "--format", "json"]);
    assert_eq!(
        json(&out)["contributions"],
        serde_json::json!([
            {"file": "@pricing/rules/handling.rule", "dependency": "@pricing", "to": "@pricing/@tax/rules/surcharge.rule", "to_dependency": "@pricing/@tax"},
            {"file": "rules/spring_sale.rule", "dependency": null, "to": "@pricing/rules/order_discount.rule", "to_dependency": "@pricing"}
        ])
    );
    // `pricing` checked by itself writes `tax` its own way
    let (_, out, _) = ritsu_in(&t.path().join("pricing"), t.path(), &["check"]);
    assert!(out.contains("note: `rules/handling.rule`, a contribution of this module, adds to `@tax/rules/surcharge.rule`, a rule of the dependency `@tax` (example.com/acme/tax 1.0.0)\n"), "{out}");
    // the shape the contribution was pinned at has changed: the error is said at its line, the module's
    t.write("shop/rules/spring_sale.rule", sale.replace("signature:67a223ec46dce359", "signature:0000000000000000"));
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check", "--format", "json"]);
    assert_eq!(code, 1);
    let v = json(&out);
    let e040 = v["diagnostics"].as_array().unwrap().iter().find(|d| d["code"] == "E040").unwrap();
    assert_eq!((e040["file"].as_str(), e040["dependency"].clone()), (Some("rules/spring_sale.rule"), serde_json::Value::Null));
    assert_eq!(v["dependencies"][0]["errors"], 0);
}

/// An open rule of `pricing`'s, which the campaigns of the modules that use it add to.
const ORDER_DISCOUNT: &str = "# The discount of an order: what the campaigns of the shop add, up to its subtotal.\nrule order_discount v1\ndescription \"The discount of an order: the sum of what the campaigns add, up to the subtotal\"\n\ninputs\n  subtotal : money[GBPc]  range >=0GBPc <=2_000_000GBPc\n\noutputs\n  discount : money[GBPc]  round down(1GBPc)\n\n# What the campaigns take off, from files of their own: nothing when none does.\nopen campaigns : money[GBPc]  combine sum  empty 0GBPc  range >=0GBPc <=10_000GBPc\n\ndefine discount : money[GBPc] = min(campaigns, subtotal)\n\nexamples\n| subtotal | -> discount |\n| 1000GBPc | 0GBPc       |\n";

/// The module's campaign, added to `pricing`'s `order_discount`, with a term it does not declare:
/// rulec says so while it checks the rule the campaign adds to.
const SPRING_SALE_BROKEN: &str = "# A campaign of the shop, added to the rule order_discount of the module pricing.\ncontribution spring_sale v1\ndescription \"A spring sale: 100 pence off every order\"\nto \"@pricing/rules/order_discount.rule\" signature:67a223ec46dce359\n\ndefine sale_off : money[GBPc] = 100GBPc\n\ninto campaigns\n  sale_offf\n\nexamples\n| subtotal | -> discount |\n| 1000GBPc | 100GBPc     |\n| 50GBPc   | 50GBPc      |\n";

/// A contribution of the module that breaks the rule of `pricing` it adds to: rulec says the error
/// while it checks the rule, at the line of the contribution, and ritsu files it under the
/// contribution, the module's own (DESIGN 17.8). The rule passes on the line of the dependencies
/// (`ritsu check @pricing`, which reads no contribution of the module, passes it too), the error is
/// counted once, with the module's files, and the check fails (exit 1). The error's text does not
/// name the rule, so a note names it, as the run writes it; an error whose text names it (E040) has
/// no such note.
#[test]
fn a_contribution_that_breaks_a_dependencys_rule_fails_the_module_not_the_dependency() {
    let t = example();
    t.write("pricing/rules/order_discount.rule", ORDER_DISCOUNT);
    t.write("shop/rules/spring_sale.rule", SPRING_SALE_BROKEN);
    let shop = t.path().join("shop");
    let (code, out, err) = ritsu_in(&shop, t.path(), &["check"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.starts_with("error[rulec E067]: The term sale_offf is not declared\n  --> rules/spring_sale.rule:9\n"), "{out}");
    assert!(
        out.contains(" A term is a name this contribution decides, or a name of the rule it adds to declared above the open value.\n This line adds to `@pricing/rules/order_discount.rule`, a rule of the dependency `@pricing`, and the check of that rule found it.\n\n"),
        "{out}"
    );
    assert_eq!(
        last_lines(&out, 2),
        ["dependencies: 2 modules, 4 files (rulec 4): all pass", "ritsu check: 3 files (rulec 1, kaname 1, dandori 1): 1 fail (1 error); borders between the languages: 0 checked, 0 undecided"]
    );
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check", "--lang", "ja"]);
    assert_eq!(code, 1);
    assert!(out.contains(" この行は、依存 `@pricing` の規則 `@pricing/rules/order_discount.rule` に足すものです。その規則の検査で見つかりました。\n"), "{out}");
    assert_eq!(
        last_lines(&out, 2),
        [
            "依存: モジュール 2 個、ファイル 4 個（rulec 4）。どれも検査を通りました",
            "ritsu check: ファイル 3 個（rulec 1、kaname 1、dandori 1）。検査を通らないもの 1 個（エラー 1 件）。言語の境目: 確かめた 0 か所、決められない 0 か所"
        ]
    );
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check", "--format", "json"]);
    assert_eq!(code, 1);
    let v = json(&out);
    assert_eq!(v["ok"], false);
    assert_eq!((&v["dependencies"][0]["prefix"], &v["dependencies"][0]["ok"], &v["dependencies"][0]["errors"]), (&serde_json::json!("@pricing"), &serde_json::json!(true), &serde_json::json!(0)));
    let ok_of = |file: &str| v["files"].as_array().unwrap().iter().find(|f| f["file"] == file).map(|f| f["ok"].clone());
    assert_eq!(ok_of("@pricing/rules/order_discount.rule"), Some(serde_json::json!(true)));
    assert_eq!(ok_of("rules/spring_sale.rule"), Some(serde_json::json!(false)));
    let ds = v["diagnostics"].as_array().unwrap();
    assert_eq!(ds.len(), 1, "{out}");
    assert_eq!((ds[0]["code"].as_str(), ds[0]["file"].as_str(), &ds[0]["dependency"]), (Some("E067"), Some("rules/spring_sale.rule"), &serde_json::Value::Null));
    assert_eq!(ds[0]["notes"][1], "This line adds to `@pricing/rules/order_discount.rule`, a rule of the dependency `@pricing`, and the check of that rule found it.");
    // the dependency by itself passes: the module's contribution is not read
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check", "@pricing"]);
    assert_eq!(code, 0, "{out}");
    // an error whose text names the rule has no note
    t.write("shop/rules/spring_sale.rule", SPRING_SALE_BROKEN.replace("  sale_offf\n", "  sale_off\n").replace("signature:67a223ec46dce359", "signature:0000000000000000"));
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check"]);
    assert_eq!(code, 1);
    assert!(out.starts_with("error[rulec E040]: The shape of `@pricing/rules/order_discount.rule` this contribution relies on has changed\n"), "{out}");
    assert!(!out.contains("This line adds to"), "{out}");
    assert_eq!(last_lines(&out, 2)[0], "dependencies: 2 modules, 4 files (rulec 4): all pass");
}

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

/// The store `store` with the module under `dir` placed in it, and its lock line.
fn put(store: &Store, dir: &Path) -> Locked {
    store::put_directory(store, dir, "2026-10-28", &[]).unwrap_or_else(|e| panic!("{}", e.en)).locked()
}

fn write_lock(t: &TempDir, root: &str, lines: Vec<Locked>) {
    let mut l = Lock::new(lines);
    l.ritsu = v("0.37.0");
    t.write(&format!("{root}/ritsu.lock"), l.text());
}

/// The example with `pricing` and `tax` from a registry: placed in a store of the test's own (the
/// tools of B.3), `shop` requires `pricing` and `pricing` requires `tax` by their names. Without a
/// lock, E303; with one that pins both, the check reads them from the store and passes; a lock with
/// no line for `tax`, E304; a line nothing requires, W302 (the files are checked); a file of the
/// store changed, E305. The errors of the tree are said with no language's check run (exit 1); a
/// module that is not in the store, a lock a newer ritsu wrote and a module that asks for a newer
/// ritsu stop the run (exit 2).
#[test]
fn modules_from_a_registry_are_held_to_the_lock() {
    let t = example();
    replace(&t, "pricing/ritsu.mod", "require tax path \"../tax\"", "require tax oci \"example.com/acme/tax\" 1.0.0");
    replace(&t, "shop/ritsu.mod", "require pricing path \"../pricing\"", "require pricing oci \"example.com/acme/pricing\" 1.1.0");
    let store = Store::at(t.path().join("store"));
    let tax = put(&store, &t.path().join("tax"));
    let pricing = put(&store, &t.path().join("pricing"));
    let shop = t.path().join("shop");
    let at = t.path().join("store");
    // no lock
    let (code, out, err) = ritsu_in(&shop, &at, &["check"]);
    assert_eq!((code, err.as_str()), (1, ""), "{out}");
    assert_eq!(
        out,
        "error[ritsu E303]: ritsu.mod:7:9: there is no `ritsu.lock`: the dependency `pricing` comes from a registry, so the version chosen and its digests are pinned in `ritsu.lock`; run `ritsu mod lock`\n     7 | require pricing oci \"example.com/acme/pricing\" 1.1.0\nritsu check: the modules could not be read (1 error); no file was checked\n"
    );
    let (_, out, _) = ritsu_in(&shop, &at, &["check", "--lang", "ja"]);
    assert_eq!(last_lines(&out, 1), ["ritsu check: モジュールを読めません（エラー 1 件）。ファイルは検査していません"]);
    let (_, out, _) = ritsu_in(&shop, &at, &["check", "--format", "json"]);
    let v = json(&out);
    assert_eq!(v["module"], serde_json::json!({"name": "example.com/acme/shop", "version": "0.1.0"}));
    assert_eq!((v["ok"].as_bool(), v["dependencies"].as_array().map(Vec::len), v["files"].as_array().map(Vec::len)), (Some(false), Some(0), Some(0)));
    assert_eq!((v["diagnostics"][0]["code"].as_str(), v["diagnostics"][0]["file"].as_str()), (Some("E303"), Some("ritsu.mod")));
    // the lock pins both: read from the store
    write_lock(&t, "shop", vec![pricing.clone(), tax.clone()]);
    let (code, out, err) = ritsu_in(&shop, &at, &["check"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(last_lines(&out, 2)[0], "dependencies: 2 modules, 3 files (rulec 3): all pass");
    let (_, out, _) = ritsu_in(&shop, &at, &["check", "--format", "json"]);
    let v = json(&out);
    let p = &v["dependencies"][0];
    let keys: Vec<&str> = p.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["prefix", "name", "version", "source", "path", "digest", "files_digest", "where", "verified", "files", "ok", "errors", "warnings", "claims_not_run"]);
    assert_eq!((p["prefix"].as_str(), p["source"].as_str(), p["where"].as_str(), p["path"].clone()), (Some("@pricing"), Some("oci"), Some("store"), serde_json::Value::Null));
    assert!(p["digest"].as_str().is_some_and(|d| d.starts_with("sha256:")) && p["files_digest"].as_str().is_some_and(|d| d.starts_with("sha256:")), "{p}");
    assert_eq!(v["dependencies"][1]["prefix"], "@pricing/@tax");
    // a lock with no line for `tax`
    write_lock(&t, "shop", vec![pricing.clone()]);
    let (code, out, _) = ritsu_in(&shop, &at, &["check"]);
    assert_eq!(code, 1);
    assert!(out.starts_with("error[ritsu E304]: @pricing/ritsu.mod:7:9: `ritsu.lock` does not match `ritsu.mod`: the lock has no line for `require tax oci \"example.com/acme/tax\" 1.0.0`; run `ritsu mod lock`\n"), "{out}");
    // a line nothing requires now: a warning, and the files are checked
    write_lock(&t, "shop", vec![pricing.clone(), tax.clone()]);
    replace(&t, "shop/ritsu.mod", "require pricing oci \"example.com/acme/pricing\" 1.1.0", "require tax oci \"example.com/acme/tax\" 1.0.0");
    let (code, out, _) = ritsu_in(&shop, &at, &["check"]);
    assert_eq!(code, 1, "the flow reads `@pricing/…`, which is no dependency now: {out}");
    assert!(out.contains("warning[ritsu W302]: ritsu.lock:4: `ritsu.lock` has a line for `example.com/acme/pricing`, which nothing requires now; run `ritsu mod lock` to write it again\n"), "{out}");
    replace(&t, "shop/ritsu.mod", "require tax oci \"example.com/acme/tax\" 1.0.0", "require pricing oci \"example.com/acme/pricing\" 1.1.0");
    // a file of the store changed by hand
    let digest = match &tax.pinned {
        ritsu_base::modules::lock::Pinned::Oci { digest, .. } => digest.clone(),
        other => panic!("{other:?}"),
    };
    let f = store.files(&digest).join("rules/vat_rate.rule");
    let mut perm = std::fs::metadata(&f).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perm.set_readonly(false);
    std::fs::set_permissions(&f, perm).unwrap();
    std::fs::write(&f, std::fs::read_to_string(&f).unwrap().replace("| standard | 20%", "| standard | 21%")).unwrap();
    let (code, out, _) = ritsu_in(&shop, &at, &["check"]);
    assert_eq!(code, 1);
    assert!(out.starts_with("error[ritsu E305]: ritsu.lock:"), "{out}");
    assert!(out.contains("the files of `example.com/acme/tax` 1.0.0 in the store differ from the digest of `ritsu.lock`; run `ritsu mod clean`, then `ritsu mod fetch` to fetch them again\n"), "{out}");
    // a store without the modules: stop
    let empty = TempDir::new("modules-empty-store");
    let (code, out, err) = ritsu_in(&shop, empty.path(), &["check"]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert_eq!(err, "error: the dependency `example.com/acme/pricing` 1.1.0 is neither in the store nor in this module's copy of it; run `ritsu mod fetch`\n");
    let (code, out, err) = ritsu_in(&shop, empty.path(), &["check", "--format", "json", "--lang", "ja"]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert!(err.starts_with("エラー: "), "{err}");
    // a lock a newer ritsu wrote: stop
    let lock = t.read("shop/ritsu.lock");
    t.write("shop/ritsu.lock", lock.replacen("lock 1\n", "lock 2\n", 1));
    let (code, out, err) = ritsu_in(&shop, &at, &["check"]);
    assert_eq!((code, out.as_str()), (2, ""), "{err}");
    assert_eq!(err, "error: `ritsu.lock` is in the format `lock 2`, and this ritsu reads `lock 1` at most: a newer ritsu wrote it, so run a newer ritsu\n");
    t.write("shop/ritsu.lock", lock);
    // a module that asks for a newer ritsu: stop
    let paths = example();
    replace(&paths, "tax/ritsu.mod", "license \"MIT OR Apache-2.0\"\n", "ritsu >=99.0.0\nlicense \"MIT OR Apache-2.0\"\n");
    let (code, out, err) = ritsu_in(&paths.path().join("shop"), paths.path(), &["check"]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert_eq!(err, format!("error: `example.com/acme/tax` asks for ritsu 99.0.0 or later, and the ritsu that runs is {}; run a newer ritsu\n", env!("CARGO_PKG_VERSION")));
}

/// A directory with no `ritsu.mod` above it that holds a module: the walk passes over the module and
/// says so in one line, before the summary; with nothing else to check, the note goes with the
/// error (exit 2). The JSON lists the module passed over.
#[test]
fn a_walk_outside_a_module_passes_over_a_module_and_says_so() {
    let t = example();
    t.write("loose/rules/shipping_fee.rule", std::fs::read_to_string(here().join("tests/projects/modules/pricing/rules/shipping_fee.rule")).unwrap());
    copy_dir(&t.path().join("tax"), &t.path().join("loose/tax"));
    let (code, out, _) = ritsu_in(t.path(), t.path(), &["check", "loose"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(
        out,
        "ok loose/rules/shipping_fee.rule\nnote: `loose/tax` has a `ritsu.mod` of its own: it is another module, and was not read; run `ritsu check loose/tax` to read it\nritsu check: 1 file (rulec 1): all pass; borders between the languages: 0 checked, 0 undecided\n"
    );
    let (_, out, _) = ritsu_in(t.path(), t.path(), &["check", "loose", "--lang", "ja"]);
    assert!(out.contains("備考: `loose/tax` は自分の `ritsu.mod` を持つ別のモジュールなので、読んでいません。読むには `ritsu check loose/tax` を実行してください\n"), "{out}");
    let (_, out, _) = ritsu_in(t.path(), t.path(), &["check", "loose", "--format", "json"]);
    let v = json(&out);
    let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["ritsu", "root", "ok", "skipped_modules", "files", "diagnostics", "borders"]);
    // from the root, as the files are
    assert_eq!((v["root"].as_str(), v["skipped_modules"].clone()), (Some("loose"), serde_json::json!(["tax"])));
    std::fs::remove_file(t.path().join("loose/rules/shipping_fee.rule")).unwrap();
    let (code, out, err) = ritsu_in(t.path(), t.path(), &["check", "loose"]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert_eq!(
        err,
        "error: there is no file of ritsu's languages (.rule, .flow, .cal, .book, .geas, .req, .ctx, .gate, .kn, .proto) in loose\nnote: `loose/tax` has a `ritsu.mod` of its own: it is another module, and was not read; run `ritsu check loose/tax` to read it\n"
    );
}

/// A path given on the command line that starts with `@` is read as a path written in a file is
/// (DESIGN 17.6), from the module `ritsu check` runs in: a dependency the module does not require, a
/// path out of the module, a module nested in a dependency entered without an `@` and a file a
/// dependency does not have are said in the words of the modules (exit 2), in English and in
/// Japanese, and `ritsu rulec` says the same. Outside every module, what it says is what it said
/// before.
#[test]
fn the_four_errors_of_a_path_given_on_the_command_line() {
    let t = example();
    t.write("pricing/nested/ritsu.mod", "module example.com/acme/nested\nversion 0.1.0\n");
    let shop = t.path().join("shop");
    let errors = [
        (
            "@shop/rules/fee.rule",
            "the `@shop` of `@shop/rules/fee.rule` is no dependency: the `ritsu.mod` of the module `example.com/acme/shop` has no `require shop …`",
            "`@shop/rules/fee.rule` の `@shop` は依存ではありません。モジュール `example.com/acme/shop` の `ritsu.mod` に `require shop …` がありません",
        ),
        (
            "@pricing/../../pricing/rules/order_total.rule",
            "`@pricing/../../pricing/rules/order_total.rule` leads outside the module `example.com/acme/shop`; a file in a module names files under the module's root, and in a dependency by `@<name>/`",
            "`@pricing/../../pricing/rules/order_total.rule` は、モジュール `example.com/acme/shop` の外を指します。モジュールの中のファイルが指せるのは、モジュールのルートの下と、`@<名前>/` で入る依存の中です",
        ),
        (
            "@pricing/nested/fee.rule",
            "`@pricing/nested/fee.rule` leads into `@pricing/nested`, a directory below with a `ritsu.mod` of its own, without an `@`; require that module, and name its files by `@<name>/`",
            "`@pricing/nested/fee.rule` は、`ritsu.mod` を持つ下のディレクトリ `@pricing/nested` の中を、`@` を通らずに指しています。そのモジュールを `require` に足し、`@<名前>/` で指してください",
        ),
        (
            "@pricing/rules/fee.rule",
            "the dependency `@pricing` (`example.com/acme/pricing`, `../pricing`) has no `rules/fee.rule`",
            "依存 `@pricing`（`example.com/acme/pricing`、`../pricing`）に `rules/fee.rule` はありません",
        ),
    ];
    for (path, en, ja) in errors {
        assert_eq!(ritsu_in(&shop, t.path(), &["check", path]), (2, String::new(), format!("error: {en}\n")), "{path}");
        assert_eq!(ritsu_in(&shop, t.path(), &["check", path, "--lang", "ja"]), (2, String::new(), format!("エラー: {ja}\n")), "{path}");
        assert_eq!(ritsu_in(&shop, t.path(), &["rulec", "check", path]), (2, String::new(), format!("error: {en}\n")), "{path}");
    }
    // the file that is there
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check", "@pricing/rules/order_total.rule"]);
    assert_eq!((code, out.lines().next()), (0, Some("ok @pricing/rules/order_total.rule")));
    // outside every module, as before
    assert_eq!(ritsu_in(t.path(), t.path(), &["check", "@pricing/rules/fee.rule"]), (2, String::new(), "error: `@pricing/rules/fee.rule` is not there\n".to_string()));
}

/// `ritsu run --with` a directory does not enter another module below it, a directory with a
/// `ritsu.mod` of its own (DESIGN 17.6): its contributions are that module's. Given in no module,
/// the walk says so on stderr in a line, and the run goes on with the rest.
#[test]
fn ritsu_run_with_a_directory_does_not_enter_another_module() {
    let t = TempDir::new("modules-run-with");
    copy_dir(&here().join("tests/projects/product_line"), t.path());
    // another module's contribution, whose row would overlap the garden line's (E105) and stop the
    // run, were it read
    t.write("rules/contrib/vendor/ritsu.mod", "module example.com/acme/vendor\n");
    t.write(
        "rules/contrib/vendor/sale.rule",
        "contribution garden_sale(garden_sale) v1\ndescription \"A sale on garden supplies\"\nto \"../../product_price.rule\" signature:d5379dca48aa13d2\nafter \"../garden_kind.rule\" signature:650e995b6929bca3\n\nclause garden_sale -> markup\n  when item_kind garden\n  then 5%\n",
    );
    let store = t.path().join("store");
    let args = ["run", "pricing.flow", "--scenario", "scenarios/garden.json", "--with", "rules/contrib", "--format", "json"];
    let (code, out, err) = ritsu_in(t.path(), &store, &[&args[..], &["--lang", "en"]].concat());
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(json(&out)["trace"]["end"], serde_json::json!({"succeed": {"total": 23}}));
    assert_eq!(err, "note: `rules/contrib/vendor` has a `ritsu.mod` of its own: it is another module, and was not read; run `ritsu run … --with rules/contrib/vendor` to read it\n");
    let (code, _, err) = ritsu_in(t.path(), &store, &[&args[..], &["--lang", "ja"]].concat());
    assert_eq!(code, 0, "{err}");
    assert_eq!(err, "備考: `rules/contrib/vendor` は自分の `ritsu.mod` を持つ別のモジュールなので、読んでいません。読むには `ritsu run … --with rules/contrib/vendor` を実行してください\n");
}

/// A flow of `shop` that asks `pricing`'s `order_discount` for the discount of an order.
const DISCOUNT_FLOW: &str = "# The discount of an order, by the rule order_discount of the module pricing.\nworkflow discount v1\ndescription \"The discount of an order, by the rule order_discount of the module pricing\"\n\nuse rule discount from \"@pricing/rules/order_discount.rule\"\n\ninputs\n  subtotal : money[GBPc]  range >=0GBPc <=2_000_000GBPc\n\noutputs\n  amount : money[GBPc]\n\nflow\n  let d = discount(subtotal: subtotal)\n  succeed amount = d.discount\n";

/// The example with a flow of `shop` that asks `pricing`'s `order_discount` for the discount of an
/// order, and two campaigns that add to it: `shop`'s (100 pence off) and `pricing`'s own (50 pence
/// off), with no examples (merged together, each would answer otherwise than alone).
fn discount_example() -> TempDir {
    let t = example();
    t.write("pricing/rules/order_discount.rule", ORDER_DISCOUNT);
    // the campaigns, with no examples: merged together, each would answer otherwise than alone
    let sale = SPRING_SALE_BROKEN.replace("  sale_offf\n", "  sale_off\n");
    let sale = format!("{}\n", sale.split("\nexamples").next().unwrap());
    t.write("shop/rules/spring_sale.rule", &sale);
    t.write(
        "pricing/rules/launch.rule",
        sale.replace("# A campaign of the shop, added to the rule order_discount of the module pricing.", "# A campaign of pricing's own, added to its rule order_discount.")
            .replace("contribution spring_sale v1", "contribution launch v1")
            .replace("A spring sale: 100 pence off every order", "A launch offer: 50 pence off every order")
            .replace("\"@pricing/rules/order_discount.rule\"", "\"order_discount.rule\"")
            .replace("sale_off", "launch_off")
            .replace("= 100GBPc", "= 50GBPc"),
    );
    t.write("shop/orders/discount.flow", DISCOUNT_FLOW);
    t.write("shop/orders/scenarios/discount.json", "{\n  \"execution\": \"discount\",\n  \"now\": \"2026-10-28T10:00:00Z\",\n  \"input\": { \"subtotal\": 1000 },\n  \"answers\": []\n}\n");
    t
}

/// `ritsu run --with` the root of a module (DESIGN 17.8) merges the contributions of the whole tree
/// into the rules the flow calls, as `ritsu check` of the root does: `shop`'s campaign (100 pence
/// off) and `pricing`'s own (50 pence off) both add to `pricing`'s `order_discount`. `--with` a
/// directory below the root, or a dependency given as `@pricing`, merges what is under it alone, and
/// no `--with` merges nothing, as ever.
#[test]
fn ritsu_run_with_the_root_merges_the_contributions_of_the_whole_tree() {
    let t = discount_example();
    let shop = t.path().join("shop");
    let (code, out, err) = ritsu_in(&shop, t.path(), &["check"]);
    assert_eq!(code, 0, "{out}{err}");
    let discount = |with: &[&str]| {
        let args = [&["run", "orders/discount.flow", "--scenario", "orders/scenarios/discount.json", "--format", "json"][..], with].concat();
        let (code, out, err) = ritsu_in(&shop, t.path(), &args);
        assert_eq!(code, 0, "{with:?}: {out}{err}");
        json(&out)["events"][0]["answer"]["ok"]["discount"].clone()
    };
    assert_eq!(discount(&[]), serde_json::json!(0));
    assert_eq!(discount(&["--with", "."]), serde_json::json!(150));
    assert_eq!(discount(&["--with", "rules"]), serde_json::json!(100));
    assert_eq!(discount(&["--with", "@pricing"]), serde_json::json!(50));
}

/// In a module, a path given to `ritsu run --with` that does not start with `@` is one of the
/// module's own (DESIGN 17.8). A dependency's directory given by its path on the disk (where a
/// `require … path` leads, the store, the module's copy of the store), whose contributions would
/// not reach the rules the flow reads, stops the run (exit 2), saying the path to give with `@`;
/// given from outside the module, where `@` names no dependency, it says to run in the module. A
/// directory of no module stops it too, with no `@` path to give. The scenario is data, and is not
/// held to this.
#[test]
fn ritsu_run_with_a_dependencys_directory_by_its_path_stops() {
    let t = discount_example();
    t.write("loose/README.md", "Not a module.\n");
    let (shop, at) = (t.path().join("shop"), t.path().join("store"));
    let run = |dir: &Path, flow: &str, scenario: &str, with: &str, lang: &str| ritsu_in(dir, &at, &["run", flow, "--scenario", scenario, "--with", with, "--lang", lang]);
    let head = "is not in the module `example.com/acme/shop`, which the flow is in: a contribution given with `--with` reaches the rules the flow reads from the module's own directories, and from a dependency's only by `@<name>/`";
    for (with, named) in [("../pricing", "@pricing"), ("../pricing/rules", "@pricing/rules"), ("../pricing/rules/launch.rule", "@pricing/rules/launch.rule"), ("../tax", "@pricing/@tax")] {
        let (code, out, err) = run(&shop, "orders/discount.flow", "orders/scenarios/discount.json", with, "en");
        assert_eq!((code, out.as_str(), err), (2, "", format!("error: `{with}` {head}; give it as `{named}`\n")), "{with}");
    }
    let (code, _, err) = run(&shop, "orders/discount.flow", "orders/scenarios/discount.json", "../pricing", "ja");
    assert_eq!(
        (code, err.as_str()),
        (2, "エラー: `../pricing` は、フローのあるモジュール `example.com/acme/shop` の中にありません。`--with` で渡した `contribution` がフローの読む規則に届くのは、そのモジュールのディレクトリのものと、依存のものを `@<名前>/` で渡したときだけです。`@pricing` と書いてください\n")
    );
    // from outside the module, where `@` names no dependency
    let (code, _, err) = run(t.path(), "shop/orders/discount.flow", "shop/orders/scenarios/discount.json", "pricing", "en");
    assert_eq!(
        (code, err),
        (2, format!("error: `pricing` {head}; `@` names a dependency only in the module the run is in, so run in a directory of the module `example.com/acme/shop` and give it as `@pricing`\n"))
    );
    let (code, _, err) = run(t.path(), "shop/orders/discount.flow", "shop/orders/scenarios/discount.json", "pricing/rules", "ja");
    assert_eq!(
        (code, err.as_str()),
        (2, "エラー: `pricing/rules` は、フローのあるモジュール `example.com/acme/shop` の中にありません。`--with` で渡した `contribution` がフローの読む規則に届くのは、そのモジュールのディレクトリのものと、依存のものを `@<名前>/` で渡したときだけです。`@` で依存を指せるのは、走らせたディレクトリのあるモジュールの中だけなので、モジュール `example.com/acme/shop` のディレクトリで走らせて、`@pricing/rules` と書いてください\n")
    );
    // a directory of no module
    let (code, _, err) = run(&shop, "orders/discount.flow", "orders/scenarios/discount.json", "../loose", "en");
    assert_eq!((code, err), (2, format!("error: `../loose` {head}\n")));
    // the module's own directories, and the scenario from anywhere
    t.write("scenarios/discount.json", t.read("shop/orders/scenarios/discount.json"));
    let (code, out, err) = ritsu_in(&shop, &at, &["run", "orders/discount.flow", "--scenario", "../scenarios/discount.json", "--with", "rules", "--format", "json"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(json(&out)["events"][0]["answer"]["ok"]["discount"], serde_json::json!(100));
    // `pricing` from a registry: its files in the store, then in the module's copy of it
    replace(&t, "pricing/ritsu.mod", "require tax path \"../tax\"", "require tax oci \"example.com/acme/tax\" 1.0.0");
    replace(&t, "shop/ritsu.mod", "require pricing path \"../pricing\"", "require pricing oci \"example.com/acme/pricing\" 1.1.0");
    let store = Store::at(&at);
    let (tax, pricing) = (put(&store, &t.path().join("tax")), put(&store, &t.path().join("pricing")));
    write_lock(&t, "shop", vec![pricing.clone(), tax]);
    let digest = match &pricing.pinned {
        ritsu_base::modules::lock::Pinned::Oci { digest, .. } => digest.clone(),
        _ => unreachable!(),
    };
    let in_store = store.files(&digest).join("rules").to_string_lossy().into_owned();
    let (code, _, err) = run(&shop, "orders/discount.flow", "orders/scenarios/discount.json", &in_store, "en");
    assert_eq!((code, err), (2, format!("error: `{in_store}` {head}; give it as `@pricing/rules`\n")));
    assert_eq!(ritsu_in(&shop, &at, &["mod", "vendor"]).0, 0);
    let hex = digest.trim_start_matches("sha256:");
    let copy = format!(".ritsu/modules/sha256/{hex}/files/rules/launch.rule");
    let (code, _, err) = run(&shop, "orders/discount.flow", "orders/scenarios/discount.json", &copy, "en");
    assert_eq!((code, err), (2, format!("error: `{copy}` {head}; give it as `@pricing/rules/launch.rule`\n")));
    let (code, out, err) = ritsu_in(&shop, &at, &["run", "orders/discount.flow", "--scenario", "orders/scenarios/discount.json", "--with", "@pricing/rules/launch.rule", "--format", "json"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(json(&out)["events"][0]["answer"]["ok"]["discount"], serde_json::json!(50));
}

/// A module a `require … path` reaches, which another `require` of the tree asks of a registry, is
/// read from the path (DESIGN 17.7), with no lock to hold it to, and the check says so in a note.
#[test]
fn a_path_wins_over_a_registry_and_the_check_says_so() {
    let t = example();
    replace(&t, "shop/ritsu.mod", "require pricing path \"../pricing\"\n", "require pricing path \"../pricing\"\nrequire tax path \"../tax\"\n");
    replace(&t, "pricing/ritsu.mod", "require tax path \"../tax\"", "require tax oci \"example.com/acme/tax\" 1.0.0");
    let shop = t.path().join("shop");
    let (code, out, err) = ritsu_in(&shop, t.path(), &["check", "--lang", "en"]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    assert!(
        out.contains("note: the module example.com/acme/tax is read from the path a `require … path` gives, in place of the version from a registry another `require` of the tree asks for\n"),
        "{out}"
    );
    let (_, out, _) = ritsu_in(&shop, t.path(), &["check", "--lang", "ja"]);
    assert!(out.contains("備考: モジュール example.com/acme/tax は、木のほかの `require` がレジストリの版を求めていますが、`require … path` のパスから読んでいます\n"), "{out}");
}

/// A name that starts with `@` right under the root of a module (E307) is a finding of no file of
/// the module, and is counted with the ones that fail (DESIGN 8.3): the files all pass, and the check
/// does not. The summary says the error bare, not `all pass (1 error)`, and the JSON says `ok:
/// false` (exit 1), in English and in Japanese.
#[test]
fn e307_fails_a_check_whose_files_all_pass() {
    let t = example();
    t.write("shop/@drafts/notes.md", "Drafts of the shop's rules.\n");
    let shop = t.path().join("shop");
    let (code, out, err) = ritsu_in(&shop, t.path(), &["check"]);
    assert_eq!((code, err.as_str()), (1, ""), "{out}");
    assert_eq!(
        last_lines(&out, 3),
        [
            "error[ritsu E307]: ritsu.mod: the root of the module has `@drafts` in it; a name that starts with `@` right under the root names a dependency, so no file or directory takes one: rename it",
            "dependencies: 2 modules, 3 files (rulec 3): all pass",
            "ritsu check: 2 files (kaname 1, dandori 1): 1 error; borders between the languages: 0 checked, 0 undecided",
        ]
    );
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check", "--lang", "ja"]);
    assert_eq!(code, 1);
    assert_eq!(
        last_lines(&out, 3),
        [
            "エラー[ritsu E307]: ritsu.mod: モジュールのルートに `@drafts` があります。ルートのすぐ下の `@` で始まる名前は依存を指すので、ファイルやディレクトリには付けられません。名前を変えてください",
            "依存: モジュール 2 個、ファイル 3 個（rulec 3）。どれも検査を通りました",
            "ritsu check: ファイル 2 個（kaname 1、dandori 1）。エラー 1 件。言語の境目: 確かめた 0 か所、決められない 0 か所",
        ]
    );
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check", "--format", "json"]);
    let v = json(&out);
    assert_eq!((code, v["ok"].as_bool()), (1, Some(false)));
    assert!(v["files"].as_array().unwrap().iter().all(|f| f["ok"] == true), "{out}");
    assert_eq!((v["diagnostics"][0]["code"].as_str(), v["diagnostics"][0]["file"].as_str()), (Some("E307"), Some("ritsu.mod")));
    // renamed, the module passes
    std::fs::rename(shop.join("@drafts"), shop.join("drafts")).unwrap();
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.ends_with("ritsu check: 2 files (kaname 1, dandori 1): all pass; borders between the languages: 0 checked, 0 undecided\n"), "{out}");
}

// ── The seed module (DESIGN 17.18) ──────────────────────────────────────────────────────────

/// The root of the repository.
fn repo() -> PathBuf {
    here().join("../..")
}

/// The data files a calendar's `source … = file "<file>" url "<url>" sha256:<head>` lines pin, under
/// `dir` (its `.cal` files, walked): the file's name, the URL and the digest's head, a line each.
fn pins(dir: &Path) -> std::collections::BTreeSet<(String, String, String)> {
    let mut out = std::collections::BTreeSet::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "cal") {
                for line in std::fs::read_to_string(&p).unwrap().lines().filter(|l| l.starts_with("source ")) {
                    let quoted: Vec<&str> = line.split('"').collect();
                    let (file, url) = (quoted[1], quoted[3]);
                    let sha = line.rsplit("sha256:").next().unwrap().trim();
                    let name = file.rsplit('/').next().unwrap();
                    out.insert((name.to_string(), url.to_string(), sha.to_string()));
                }
            }
        }
    }
    out
}

/// The data files of the seed module are, byte for byte, those of koyomi's example of calendars,
/// and each calendar of the module pins its file by the URL and the digest koyomi's example pins it
/// by: one is not made new without the other.
#[test]
fn the_seed_module_holds_the_data_and_the_pins_of_koyomi_s_example() {
    let seed = repo().join("modules/holidays");
    let koyomi = repo().join("crates/koyomi/examples/calendars");
    for (ours, theirs) in [("uk/data/bank-holidays.json", "data/bank-holidays.json"), ("japan/data/syukujitsu.csv", "data/syukujitsu.csv")] {
        let (a, b) = (std::fs::read(seed.join(ours)).unwrap(), std::fs::read(koyomi.join(theirs)).unwrap());
        assert!(a == b, "modules/holidays/{ours} is not the bytes of crates/koyomi/examples/calendars/{theirs}");
    }
    let (ours, theirs) = (pins(&seed), pins(&koyomi));
    assert_eq!(ours.len(), 2, "{ours:?}");
    assert!(ours.is_subset(&theirs), "the seed module pins {ours:?}, and koyomi's example {theirs:?}");
    // five calendars, which close the holidays and nothing else: no weekend
    let mut cals = 0;
    for f in ["uk/england_and_wales.cal", "uk/scotland.cal", "uk/northern_ireland.cal", "japan/national_holidays.cal", "japan/国民の祝日.cal"] {
        let text = std::fs::read_to_string(seed.join(f)).unwrap();
        assert!(!text.contains("closed weekly"), "{f} closes a day of the week");
        cals += 1;
    }
    assert_eq!(cals, 5);
}

/// `ritsu check modules/holidays`, from the root of the repository, in English and in Japanese and
/// as JSON, held to golden files: the five calendars pass, and the module has no dependency.
#[test]
fn the_seed_module_passes_its_check() {
    let empty = TempDir::new("modules-seed");
    let mut failures = Vec::new();
    for lang in ["en", "ja"] {
        let (code, out, err) = ritsu_in(&repo(), empty.path(), &["check", "modules/holidays", "--lang", lang]);
        assert_eq!((code, err.as_str()), (0, ""), "{out}");
        golden(&format!("tests/golden/modules/holidays.{lang}.txt"), &out, &mut failures);
    }
    let (code, out, _) = ritsu_in(&repo(), empty.path(), &["check", "modules/holidays", "--format", "json"]);
    assert_eq!(code, 0, "{out}");
    golden("tests/golden/modules/holidays.json", &out, &mut failures);
    let v = json(&out);
    assert_eq!((v["module"]["name"].as_str(), v["files"].as_array().map(Vec::len)), (Some("ghcr.io/i2y/ritsu-holidays"), Some(5)));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// `.github/workflows/modules.yml` (DESIGN 17.18) starts by hand only, and pushes nothing unless
/// `publish` is ticked; its publishing job alone has the rights to push and to sign, and it installs
/// the cosign the tests install (tools.yml), at the same version and held to the same checksum.
#[test]
fn the_workflow_that_publishes_the_seed_module_runs_by_hand_only() {
    let read = |p: &str| std::fs::read_to_string(repo().join(p)).unwrap();
    // the lines that are not comments
    let wf: String = read(".github/workflows/modules.yml").lines().filter(|l| !l.trim_start().starts_with('#')).map(|l| format!("{l}\n")).collect();
    let on = wf.split_once("\non:\n").expect("modules.yml has no `on:`").1;
    let triggers: Vec<&str> = on.lines().take_while(|l| l.is_empty() || l.starts_with(' ')).filter(|l| l.starts_with("  ") && !l.starts_with("   ")).collect();
    assert_eq!(triggers, ["  workflow_dispatch:"], "modules.yml starts on more than a run by hand");
    assert!(wf.contains("      publish:\n"), "modules.yml has no input `publish`");
    let publish_input = wf.split_once("      publish:\n").unwrap().1;
    assert!(publish_input.contains("        type: boolean\n        default: false\n"), "`publish` is not a boolean that is false unless ticked");
    let (check, publish) = wf.split_once("\n  publish:\n").expect("modules.yml has no job `publish`");
    assert!(publish.contains("\n    if: ${{ inputs.publish }}\n"), "the job `publish` runs when `publish` is not ticked");
    for right in ["packages: write", "id-token: write"] {
        assert!(!check.contains(right), "modules.yml gives `{right}` outside the job `publish`");
        assert_eq!(publish.matches(right).count(), 1, "the job `publish` does not have `{right}`");
    }
    let tools = read(".github/workflows/tools.yml");
    let pinned: Vec<&str> = wf.lines().filter(|l| l.contains("/sigstore/cosign/releases/download/") || l.contains("| sha256sum -c -")).collect();
    assert_eq!(pinned.len(), 2, "{pinned:?}");
    for line in pinned {
        assert!(tools.lines().any(|t| t == line), "tools.yml installs another cosign than modules.yml's\n{line}");
    }
}
