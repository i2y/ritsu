//! `ritsu mod report` (DESIGN 17.10, 17.21; PLAN 8.3 C.5), English first and the twin with Japanese
//! names beside it. Each case is a git repository the test makes, whose commits raise only the
//! dependencies, with a store of the test's own:
//!
//! - The example's shop, which requires pricing from a registry. 1.0.0 → 1.1.0: answers that differ
//!   and no error (exit 0, and 1 with `--fail-on-change`). 1.1.0 → 2.0.0: the output the flow reads
//!   is renamed, so the flow no longer passes, a new error the report gives to pricing (exit 1).
//!   1.1.0 published again with other contents: the tag moved, said at the top.
//! - A module of the test's own that requires the desk (a calendar, a dates file, a gate and the
//!   rule it reads) from a registry, with a dates file and a gate of its own that read the desk's
//!   calendar and rule: what raising the desk changes in their answers (koyomi's and sekisho's
//!   diffs).
//! - Markdown and JSON held to golden files (`tests/golden/modules/report-*`; `RITSU_BLESS=1`
//!   writes them), the digests masked: the lock in each version carries the version of ritsu.
//!   Every `@` of the Markdown is in code.
//! - A report longer than a comment holds, cut at 60,000 characters with the fold it was in closed.
//! - Two revisions whose locks pin the same modules; and what stops a report (exit 2): no `--base`,
//!   a revision that is not one, a module in no repository.
//! - What the two locks pin, fetched from a registry the test starts (`ritsu_testkit::registry`:
//!   curl asks it on the loopback, and nothing else) into an empty store: the same report as with
//!   the store `ritsu mod fetch --rev <base>` and `ritsu mod fetch` filled, each version fetched said
//!   on stderr, and nothing asked of the registry once the store has it. A fetch that stops (an OCI
//!   manifest that is not its digest, a registry that does not answer) stops the report (exit 2),
//!   in each language.
//! - A module that changed whose difference cannot be made (what one of its versions reads cannot
//!   be fetched), beside one whose difference is made: the report says why in its place, counts it
//!   at the top, and exits 2, with `--fail-on-change` too when no difference is shown in behavior,
//!   in each language.

use ritsu_base::modules::store::{self, Store};
use ritsu_testkit::registry::{self as reg, Registry};
use ritsu_testkit::{TempDir, skip};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// `ritsu` with `args`, to run in `dir` with the store at `store` and no language asked of the
/// environment.
fn command(dir: &Path, store: &Path, args: &[&str]) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).args(args).env(store::VARIABLE, store);
    for v in ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG", "SEKISHO_LANG", "KANAME_LANG"] {
        c.env_remove(v);
    }
    c
}

/// The exit code, what was printed and what was said on stderr.
fn said(o: Output) -> (i32, String, String) {
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// `ritsu`, run in `dir` with the store at `store` and no language asked of the environment: the
/// exit code, what it printed and what it said on stderr.
fn ritsu_in(dir: &Path, store: &Path, args: &[&str]) -> (i32, String, String) {
    said(command(dir, store, args).output().expect("could not run ritsu"))
}

/// Whether git runs; a SKIP line when it does not.
fn has_git() -> bool {
    if Command::new("git").arg("--version").output().is_ok_and(|o| o.status.success()) {
        return true;
    }
    skip("git is not on the PATH; a report reads the base's lock at a git revision");
    false
}

fn git(dir: &Path, args: &[&str]) {
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=ritsu-test", "-c", "user.email=ritsu-test@example.com", "-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false", "-c", "init.defaultBranch=main"])
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .expect("could not run git");
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
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

/// The text with each digest (`sha256:` and 64 hexadecimal digits) masked, and each head of one
/// that tells two sides of one name apart (`sha256:` and 12).
fn masked(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("sha256:") {
        let after = &rest[at + 7..];
        let hex = after.chars().take_while(|c| c.is_ascii_hexdigit()).count();
        out.push_str(&rest[..at]);
        match hex {
            64 => out.push_str("sha256:<digest>"),
            12 => out.push_str("sha256:<head>"),
            _ => out.push_str("sha256:"),
        }
        rest = if hex == 64 || hex == 12 { &after[hex..] } else { after };
    }
    out.push_str(rest);
    out
}

/// The lines of the Markdown with an `@` outside code: a span in backticks, `<code>`, a block
/// between fences.
fn bare_ats(md: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut fence = false;
    for line in md.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        let mut outside = String::new();
        let mut rest = line;
        while let Some(c) = rest.chars().next() {
            if let Some(after) = rest.strip_prefix("<code>") {
                rest = after.find("</code>").map_or("", |at| &after[at + "</code>".len()..]);
            } else if c == '`' {
                let ticks = rest.len() - rest.trim_start_matches('`').len();
                let close = &rest[..ticks];
                rest = rest[ticks..].find(close).map_or("", |at| &rest[ticks + at + ticks..]);
            } else {
                outside.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
        if outside.contains('@') {
            found.push(line.to_string());
        }
    }
    found
}

/// What every Markdown report holds to: no `@` outside code, the folds closed, no more than a
/// comment holds, and no path of the machine.
fn holds(md: &str, t: &Path, what: &str) {
    assert_eq!(bare_ats(md), Vec::<String>::new(), "{what}: an `@` outside code");
    assert_eq!(md.matches("<details>").count(), md.matches("</details>").count(), "{what}: a fold left open");
    assert!(md.chars().count() <= 60_000, "{what}: {} characters", md.chars().count());
    assert!(!md.contains(&*t.to_string_lossy()), "{what} names the test's directory");
}

// ── The shop and pricing ────────────────────────────────────────────────

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
    flow: &'static str,
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
    flow: "orders/place_order.flow",
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
    flow: "注文/注文を受ける.flow",
};

const PRICING: &str = "example.com/acme/pricing";

fn put(store: &Path, dir: &Path) {
    store::put_directory(&Store::at(store), dir, "2026-10-28", &[]).unwrap_or_else(|e| panic!("{}", e.en));
}

/// The example's shop in a repository of its own, which requires pricing from a registry; tax and
/// the three versions of pricing in a store of the test's own, each version of pricing with the
/// lock `ritsu mod lock` writes. Three commits, each raising pricing alone, tagged `one` (1.0.0),
/// `two` (1.1.0) and `three` (2.0.0); the working tree at `two`.
fn shop(n: &Names) -> (TempDir, PathBuf, PathBuf) {
    let t = TempDir::new(&format!("mod-report-shop-{}", n.lang));
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
    let store = d.join("store");
    put(&store, &d.join(n.tax));
    for m in [n.pricing_100, n.pricing, n.pricing_200] {
        let (code, out, err) = ritsu_in(&d.join(m), &store, &["mod", "lock"]);
        assert_eq!(code, 0, "{m}: {out}{err}");
        put(&store, &d.join(m));
    }
    let repo = d.join(n.shop);
    replace(d, &format!("{}/ritsu.mod", n.shop), &format!("require pricing path \"../{}\"", n.pricing), &format!("require pricing oci \"{PRICING}\" 1.0.0"));
    let lock = |what: &str| {
        let (code, out, err) = ritsu_in(&repo, &store, &["mod", "lock"]);
        assert_eq!(code, 0, "{what}: {out}{err}");
    };
    lock("1.0.0");
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "pricing 1.0.0"]);
    git(&repo, &["tag", "one"]);
    for (from, to, tag) in [("1.0.0", "1.1.0", "two"), ("1.1.0", "2.0.0", "three")] {
        replace(d, &format!("{}/ritsu.mod", n.shop), &format!("\"{PRICING}\" {from}"), &format!("\"{PRICING}\" {to}"));
        lock(to);
        git(&repo, &["commit", "-q", "-a", "-m", &format!("pricing {to}")]);
        git(&repo, &["tag", tag]);
    }
    git(&repo, &["checkout", "-q", "two"]);
    (t, store, repo)
}

/// The shop's reports, in each language of the example, held to golden files: pricing raised to
/// 1.1.0 (the working tree), to 2.0.0 (two revisions), and 1.1.0 published again.
#[test]
fn the_shop_raising_pricing_held_to_golden_files() {
    if !has_git() {
        return;
    }
    let mut failures = Vec::new();
    for n in [EN, JA] {
        let (t, store, repo) = shop(&n);
        let lang = n.lang;
        let report = |args: &[&str], format: &str| {
            let mut all = vec!["mod", "report"];
            all.extend(args);
            all.extend(["--format", format, "--lang", lang]);
            ritsu_in(&repo, &store, &all)
        };

        // 1.0.0 → 1.1.0: the answers of the fee differ, and nothing breaks
        for (format, ext) in [("markdown", "md"), ("json", "json")] {
            let (code, out, err) = report(&["--base", "one"], format);
            assert_eq!((code, err.as_str()), (0, ""), "raise {format} {lang}:\n{out}");
            if format == "markdown" {
                holds(&out, t.path(), &format!("raise {lang}"));
            }
            golden(&format!("tests/golden/modules/report-raise.{lang}.{ext}"), &masked(&out), &mut failures);
        }
        let (code, out, _) = report(&["--base", "one"], "json");
        let v = json(&out);
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["ritsu", "base", "head", "check", "modules", "ok"]);
        assert_eq!((code, &v["base"]["rev"], &v["head"]["rev"], &v["ok"]), (0, &serde_json::json!("one"), &serde_json::Value::Null, &serde_json::json!(true)));
        let m = &v["modules"][0];
        let keys: Vec<&str> = m.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["module", "prefix", "change", "old", "new", "diagnostics", "diff", "error", "yours"]);
        assert_eq!(v["modules"].as_array().unwrap().len(), 1, "{out}");
        assert_eq!((&m["module"], &m["prefix"], &m["change"]), (&serde_json::json!(PRICING), &serde_json::json!("@pricing"), &serde_json::json!("upgraded")));
        assert_eq!((&m["old"]["version"], &m["new"]["version"]), (&serde_json::json!("1.0.0"), &serde_json::json!("1.1.0")));
        assert!(m["old"]["digest"].as_str().unwrap().starts_with("sha256:") && m["old"]["digest"] != m["new"]["digest"]);
        // the diff under the module is what `ritsu mod diff` says of the two versions, each file
        // named by the prefix and the version (`@pricing@1.0.0/rules/…`)
        let (_, alone, _) = ritsu_in(&repo, &store, &["mod", "diff", &format!("{PRICING}@1.0.0"), &format!("{PRICING}@1.1.0"), "--format", "json", "--lang", lang]);
        let alone = json(&alone.replace(&format!("{PRICING}@"), "@pricing@"));
        assert_eq!(m["diff"]["behavior"], alone["behavior"]);
        assert_eq!(m["diff"]["interface"], alone["interface"]);
        assert_eq!(m["diff"]["files"], alone["files"]);
        let fee = format!("{}/{}", n.rules, n.fee);
        assert!(m["diff"]["behavior"].as_array().unwrap().iter().any(|b| b["file"] == fee.as_str() && !b["result"]["changes"].as_array().unwrap().is_empty()));
        // with `--fail-on-change`, a difference in behavior is exit 1
        let (code, out, err) = report(&["--base", "one", "--fail-on-change"], "json");
        assert_eq!((code, err.as_str()), (1, ""), "{out}");
        assert_eq!(json(&out)["ok"], false);
        let (code, text, _) = report(&["--base", "one", "--fail-on-change"], "text");
        assert_eq!(code, 1, "{text}");
        assert!(text.contains("--fail-on-change"), "{text}");

        // 1.1.0 → 2.0.0: the flow reads an output that is no longer there, a new error of pricing
        for (format, ext) in [("markdown", "md"), ("json", "json")] {
            let (code, out, err) = report(&["--base", "two", "--head", "three"], format);
            assert_eq!((code, err.as_str()), (1, ""), "break {format} {lang}:\n{out}");
            if format == "markdown" {
                holds(&out, t.path(), &format!("break {lang}"));
            }
            golden(&format!("tests/golden/modules/report-break.{lang}.{ext}"), &masked(&out), &mut failures);
        }
        let (_, out, _) = report(&["--base", "two", "--head", "three"], "json");
        let v = json(&out);
        let new = v["check"]["new"].as_array().unwrap();
        let at = new.iter().position(|d| d["file"] == n.flow && d["severity"] == "error").unwrap_or_else(|| panic!("no new error in the flow: {out}"));
        assert_eq!(new[at]["tool"], "dandori", "{out}");
        assert_eq!(v["check"]["head"]["errors"].as_u64(), Some(new.iter().filter(|d| d["severity"] == "error").count() as u64), "{out}");
        assert_eq!(v["check"]["base"]["errors"], 0, "{out}");
        assert_eq!((&v["head"]["rev"], &v["modules"][0]["change"]), (&serde_json::json!("three"), &serde_json::json!("upgraded")));
        assert!(v["modules"][0]["diagnostics"].as_array().unwrap().contains(&serde_json::json!(at)), "{out}");

        // the same revision as the working tree: the same modules, no error
        let (code, text, err) = report(&["--base", "two"], "text");
        assert_eq!((code, err.as_str()), (0, ""), "{text}");
        let same = if lang == "en" { "the two locks pin the same modules" } else { "二つの lock は同じモジュールを指していて" };
        assert!(text.contains(same), "{text}");

        // 1.1.0 published again with another fee: the working tree's lock pins the new digest
        let again = t.path().join("again");
        copy_dir(&t.path().join(n.pricing), &again);
        let fee_file = again.join(n.rules).join(n.fee);
        let text = std::fs::read_to_string(&fee_file).unwrap();
        assert!(text.contains("2950GBPc"));
        std::fs::write(&fee_file, text.replace("2950GBPc", "3150GBPc")).unwrap();
        put(&store, &again);
        std::fs::remove_file(repo.join("ritsu.lock")).unwrap();
        let (code, out, err) = ritsu_in(&repo, &store, &["mod", "lock"]);
        assert_eq!(code, 0, "{out}{err}");
        for (format, ext) in [("markdown", "md"), ("json", "json")] {
            let (code, out, err) = report(&["--base", "two"], format);
            assert_eq!((code, err.as_str()), (0, ""), "moved {format} {lang}:\n{out}");
            if format == "markdown" {
                holds(&out, t.path(), &format!("moved {lang}"));
                // said at the top, before the check
                let warned = out.find("[!WARNING]").unwrap_or_else(|| panic!("{out}"));
                let table = out.find("| ").unwrap();
                assert!(warned < table, "{out}");
            }
            golden(&format!("tests/golden/modules/report-moved.{lang}.{ext}"), &masked(&out), &mut failures);
        }
        let (_, out, _) = report(&["--base", "two"], "json");
        let v = json(&out);
        let m = &v["modules"][0];
        assert_eq!((&m["change"], &m["old"]["version"], &m["new"]["version"]), (&serde_json::json!("moved"), &serde_json::json!("1.1.0"), &serde_json::json!("1.1.0")), "{out}");
        assert_ne!(m["old"]["digest"], m["new"]["digest"]);
        // the two sides go by one name, each told apart by the head of its digest
        let head = |d: &serde_json::Value| format!("(sha256:{})", &d.as_str().unwrap()["sha256:".len()..][..12]);
        let file = format!("{}@1.1.0/{fee}", m["prefix"].as_str().unwrap());
        let (oh, nh) = (head(&m["old"]["digest"]), head(&m["new"]["digest"]));
        let (_, md, _) = report(&["--base", "two"], "markdown");
        assert!(md.contains(&format!("`{file} {oh}` → `{file} {nh}`")), "{lang}:\n{md}");
        let fee = m["diff"]["behavior"].as_array().unwrap().iter().find(|b| b["file"] == fee.as_str()).unwrap_or_else(|| panic!("{out}"));
        assert_eq!((&fee["result"]["old"], &fee["result"]["new"]), (&serde_json::json!(format!("{file} {oh}")), &serde_json::json!(format!("{file} {nh}"))), "{out}");
        assert_eq!((&m["diff"]["old"]["digest"], &m["diff"]["new"]["digest"]), (&m["old"]["digest"], &m["new"]["digest"]), "{out}");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ── The office and the desk ─────────────────────────────────────────────

/// The names of the desk and of the office that reads it, in one language, and the materials of
/// koyomi's and sekisho's diffs they are made of.
struct Office {
    lang: &'static str,
    calendars: &'static str,
    gates: &'static str,
    /// The calendar: its name in the desk, its two versions in koyomi's materials, and the table of
    /// days each reads.
    office: &'static str,
    offices: (&'static str, &'static str),
    days: (&'static str, &'static str),
    /// The desk's dates file, which reads the calendar.
    payment: &'static str,
    /// The gate and the rule it reads: their names, and the versions in sekisho's materials.
    gate: &'static str,
    gates_of: (&'static str, &'static str),
    rule: &'static str,
    rules_of: (&'static str, &'static str),
    /// The office's own dates file and gate, made of the desk's: the line that reads the calendar
    /// or the rule, and the head and the namespace that are the office's own.
    invoice: &'static str,
    calendar_line: (&'static str, &'static str),
    dates_head: (&'static str, &'static str),
    refunds: &'static str,
    rule_line: (&'static str, &'static str),
    gate_head: (&'static str, &'static str),
    namespace: (&'static str, &'static str),
}

const OFFICE_EN: Office = Office {
    lang: "en",
    calendars: "calendars",
    gates: "gates",
    office: "office.cal",
    offices: ("office.cal", "office_v2.cal"),
    days: ("days_off.csv", "days_off_v2.csv"),
    payment: "payment.cal",
    gate: "limits.gate",
    gates_of: ("limits.gate", "limits_v2.gate"),
    rule: "band.rule",
    rules_of: ("band.rule", "band_v2.rule"),
    invoice: "invoice.cal",
    calendar_line: ("use calendar \"office.cal\"", "use calendar \"@desk/calendars/office.cal\""),
    dates_head: ("dates payment v1", "dates invoice v1"),
    refunds: "refunds.gate",
    rule_line: ("use rule band from \"band.rule\"", "use rule band from \"@desk/gates/band.rule\""),
    gate_head: ("gate limits v1", "gate refunds v1"),
    namespace: ("namespace Shop", "namespace Office"),
};

const OFFICE_JA: Office = Office {
    lang: "ja",
    calendars: "暦",
    gates: "ゲート",
    office: "事務所.cal",
    offices: ("事務所.cal", "事務所_二版.cal"),
    days: ("休み.csv", "休み_二版.csv"),
    payment: "支払日.cal",
    gate: "上限.gate",
    gates_of: ("上限.gate", "上限_二版.gate"),
    rule: "区分.rule",
    rules_of: ("区分.rule", "区分_二版.rule"),
    invoice: "請求.cal",
    calendar_line: ("use calendar \"事務所.cal\"", "use calendar \"@desk/暦/事務所.cal\""),
    dates_head: ("dates 支払日(payment) v1", "dates 請求(invoice) v1"),
    refunds: "返金.gate",
    rule_line: ("use rule 区分 from \"区分.rule\"", "use rule 区分 from \"@desk/ゲート/区分.rule\""),
    gate_head: ("gate 上限(limits_ja) v1", "gate 返金(refunds_ja) v1"),
    namespace: ("namespace ShopJa", "namespace OfficeJa"),
};

/// The desk's two versions in a store of the test's own, and the office in a repository of its own:
/// a commit tagged `one` that requires the desk at 1.0.0, and the working tree raising it to 1.1.0.
fn office(n: &Office) -> (TempDir, PathBuf, PathBuf) {
    let t = TempDir::new(&format!("mod-report-office-{}", n.lang));
    let koyomi = here().join("../koyomi/tests/diff");
    let sekisho = here().join("../sekisho/tests/diff");
    let read = |p: PathBuf| std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let store = t.path().join("store");
    for (i, (v, version)) in [("desk-1.0.0", "1.0.0"), ("desk-1.1.0", "1.1.0")].into_iter().enumerate() {
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
        put(&store, &t.path().join(v));
    }
    let repo = t.path().join("office");
    let swap = |text: String, pairs: &[(&str, &str)]| {
        pairs.iter().fold(text, |s, (a, b)| {
            assert!(s.contains(a), "no `{a}`");
            s.replacen(a, b, 1)
        })
    };
    t.write("office/ritsu.mod", "module example.com/acme/office\nversion 0.1.0\n\nrequire desk oci \"example.com/acme/desk\" 1.0.0\n");
    t.write(&format!("office/{}", n.invoice), swap(read(koyomi.join(n.payment)), &[n.calendar_line, n.dates_head]));
    t.write(&format!("office/{}", n.refunds), swap(read(sekisho.join(n.gates_of.0)), &[n.rule_line, n.gate_head, n.namespace]));
    let lock = |what: &str| {
        let (code, out, err) = ritsu_in(&repo, &store, &["mod", "lock"]);
        assert_eq!(code, 0, "{what}: {out}{err}");
    };
    lock("1.0.0");
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "desk 1.0.0"]);
    git(&repo, &["tag", "one"]);
    replace(&repo, "ritsu.mod", "\"example.com/acme/desk\" 1.0.0", "\"example.com/acme/desk\" 1.1.0");
    lock("1.1.0");
    (t, store, repo)
}

/// The office's report, in each language of the materials, held to golden files: the desk raised,
/// and the answers of the office's own dates file and gate that change with it.
#[test]
fn your_answers_that_change_held_to_golden_files() {
    if !has_git() {
        return;
    }
    let mut failures = Vec::new();
    for n in [OFFICE_EN, OFFICE_JA] {
        let (t, store, repo) = office(&n);
        let lang = n.lang;
        for (format, ext) in [("text", "txt"), ("markdown", "md"), ("json", "json")] {
            let (code, out, err) = ritsu_in(&repo, &store, &["mod", "report", "--base", "one", "--format", format, "--lang", lang]);
            assert_eq!((code, err.as_str()), (0, ""), "{format} {lang}:\n{out}");
            if format == "markdown" {
                holds(&out, t.path(), &format!("office {lang}"));
            }
            golden(&format!("tests/golden/modules/report-office.{lang}.{ext}"), &masked(&out), &mut failures);
        }
        let (_, out, _) = ritsu_in(&repo, &store, &["mod", "report", "--base", "one", "--format", "json", "--lang", lang]);
        let v = json(&out);
        let m = &v["modules"][0];
        assert_eq!((&m["module"], &m["change"]), (&serde_json::json!("example.com/acme/desk"), &serde_json::json!("upgraded")));
        // each file of the office's own is what its language's diff says of it under the two locks
        let yours: Vec<(&str, &str, bool)> = m["yours"].as_array().unwrap().iter().map(|b| (b["file"].as_str().unwrap(), b["tool"].as_str().unwrap(), b["error"].is_null())).collect();
        let mut want = vec![(n.invoice, "koyomi", true), (n.refunds, "sekisho", true)];
        want.sort();
        assert_eq!(yours, want, "{out}");
        for b in m["yours"].as_array().unwrap() {
            let differs = match b["tool"].as_str().unwrap() {
                "koyomi" => b["result"]["changes"].as_array().is_some_and(|c| !c.is_empty()),
                _ => b["result"]["total"]["gained"].as_u64().is_some_and(|k| k > 0),
            };
            assert!(differs, "{b}");
        }
        let (code, _, _) = ritsu_in(&repo, &store, &["mod", "report", "--base", "one", "--fail-on-change", "--lang", lang]);
        assert_eq!(code, 1);
        // the text says each of the office's files with its language's own text under it
        let (code, text, _) = ritsu_in(&repo, &store, &["mod", "report", "--base", "one", "--lang", lang]);
        assert_eq!(code, 0, "{text}");
        let head = if lang == "en" { "your answers that change:" } else { "あなたの答えのうち、変わるもの：" };
        assert!(text.contains(head) && text.contains(&format!("  {} (koyomi)", n.invoice)) && text.contains(&format!("  {} (sekisho)", n.refunds)), "{text}");
        // the two sides of a file of the office's are the same file, named with the lock each is read with
        let (base, now) = if lang == "en" { ("the lock of one", "the lock of the working tree") } else { ("one の lock", "作業ツリーの lock") };
        for f in [n.invoice, n.refunds] {
            assert!(text.contains(&format!("{f} ({base})\n")) && text.contains(&format!("{f} ({now})\n")), "{f}:\n{text}");
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A module with no file of ritsu's languages of its own, which only gathers a dependency: `ritsu
/// check` has nothing to check in it, so each check counts no diagnostic, and the dependency is
/// compared all the same.
#[test]
fn a_module_with_no_file_of_a_language() {
    if !has_git() {
        return;
    }
    let (t, store, _) = shop(&EN);
    let repo = t.path().join("gather");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(
        repo.join("ritsu.mod"),
        format!("module example.com/acme/gather\nversion 0.1.0\ndescription \"The prices of the shop, gathered for its apps\"\n\nrequire pricing oci \"{PRICING}\" 1.0.0\n"),
    )
    .unwrap();
    std::fs::write(repo.join("README.md"), "The prices of the shop, gathered.\n").unwrap();
    let lock = || {
        let (code, out, err) = ritsu_in(&repo, &store, &["mod", "lock"]);
        assert_eq!(code, 0, "{out}{err}");
    };
    lock();
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "pricing 1.0.0"]);
    git(&repo, &["tag", "one"]);
    replace(&repo, "ritsu.mod", &format!("\"{PRICING}\" 1.0.0"), &format!("\"{PRICING}\" 1.1.0"));
    lock();
    // `ritsu check` itself stops on it
    let (code, _, err) = ritsu_in(&repo, &store, &["check"]);
    assert_eq!(code, 2, "{err}");
    let (code, out, err) = ritsu_in(&repo, &store, &["mod", "report", "--base", "one", "--format", "json"]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    let v = json(&out);
    let counts = |side: &str| (v["check"][side]["errors"].as_u64(), v["check"][side]["warnings"].as_u64());
    assert_eq!((counts("base"), counts("head")), ((Some(0), Some(0)), (Some(0), Some(0))), "{out}");
    let m = &v["modules"][0];
    assert_eq!((&m["module"], &m["change"]), (&serde_json::json!(PRICING), &serde_json::json!("upgraded")), "{out}");
    let fee = format!("{}/{}", EN.rules, EN.fee);
    assert!(m["diff"]["behavior"].as_array().unwrap().iter().any(|b| b["file"] == fee.as_str() && !b["result"]["changes"].as_array().unwrap().is_empty()), "{out}");
    assert_eq!(m["yours"], serde_json::json!([]), "{out}");
    let (code, text, _) = ritsu_in(&repo, &store, &["mod", "report", "--base", "one"]);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("the check with the lock of one: 0 errors, 0 warnings\nthe check with the lock of the working tree: 0 errors, 0 warnings\n"), "{text}");
}

// ── A long report, and what stops one ───────────────────────────────────

/// A module of many rules, each of whose answers moves between its two versions: the Markdown is cut
/// at 60,000 characters, with what it was in closed and a line that names the JSON, which holds it
/// all.
#[test]
fn a_report_longer_than_a_comment_is_cut() {
    if !has_git() {
        return;
    }
    const RULES: usize = 140;
    let t = TempDir::new("mod-report-long");
    let store = t.path().join("store");
    // four bounds, each moved by two in the second version: four regions of answers that differ
    for (v, s) in [("1.0.0", 0), ("1.1.0", 2)] {
        let dir = format!("rates-{v}");
        t.write(&format!("{dir}/ritsu.mod"), format!("module example.com/acme/rates\nversion {v}\n"));
        for i in 0..RULES {
            t.write(
                &format!("{dir}/rules/rate_{i:03}.rule"),
                format!(
                    "rule rate_{i:03} v1\ndescription \"A rate of the test's own\"\n\nenum level = l0 | l1 | l2 | l3 | l4\n\ninputs\n  amount : number  range >=1 <=100\n\noutputs\n  level : level\n\ntable level\npolicy unique\n| amount | -> level |\n| <={} | l0 |\n| >={} <={} | l1 |\n| >={} <={} | l2 |\n| >={} <={} | l3 |\n| >={} | l4 |\n",
                    20 + s,
                    21 + s,
                    40 + s,
                    41 + s,
                    60 + s,
                    61 + s,
                    80 + s,
                    81 + s
                ),
            );
        }
        put(&store, &t.path().join(&dir));
    }
    let repo = t.path().join("books");
    t.write("books/ritsu.mod", "module example.com/acme/books\nversion 0.1.0\n\nrequire rates oci \"example.com/acme/rates\" 1.0.0\n");
    // a check of a module needs a file of its own
    t.write(
        "books/rules/fee.rule",
        "rule fee v1\ndescription \"A fee of the test's own\"\n\nenum level = low | high\n\ninputs\n  amount : number  range >=1 <=100\n\noutputs\n  level : level\n\ntable level\npolicy unique\n| amount | -> level |\n| <=50 | low |\n| >=51 | high |\n",
    );
    let (code, out, err) = ritsu_in(&repo, &store, &["mod", "lock"]);
    assert_eq!(code, 0, "{out}{err}");
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "rates 1.0.0"]);
    replace(&repo, "ritsu.mod", "\"example.com/acme/rates\" 1.0.0", "\"example.com/acme/rates\" 1.1.0");
    let (code, out, err) = ritsu_in(&repo, &store, &["mod", "lock"]);
    assert_eq!(code, 0, "{out}{err}");
    for lang in ["en", "ja"] {
        let (code, md, err) = ritsu_in(&repo, &store, &["mod", "report", "--base", "HEAD", "--format", "markdown", "--lang", lang]);
        assert_eq!((code, err.as_str()), (0, ""), "{lang}: {}", &md[..md.len().min(3000)]);
        holds(&md, t.path(), &format!("long {lang}"));
        let note = if lang == "en" { "_This report is cut at 60,000 characters" } else { "_この報告は 60,000 字で切りました" };
        let tail: String = md.chars().rev().take(400).collect::<Vec<_>>().into_iter().rev().collect();
        assert!(md.contains(note) && md.trim_end().ends_with("_"), "{lang}: {tail}");
        assert!(md.chars().count() > 50_000, "{lang}: {}", md.chars().count());
    }
    let (_, out, _) = ritsu_in(&repo, &store, &["mod", "report", "--base", "HEAD", "--format", "json"]);
    let v = json(&out);
    assert_eq!(v["modules"][0]["diff"]["behavior"].as_array().unwrap().len(), RULES);
}

/// What stops a report, exit 2 and nothing printed: no `--base`, a revision that is not one, a module
/// in no repository. (What stops a fetch of what the locks pin is in `what_a_report_reads_is_fetched`.)
#[test]
fn what_stops_a_report() {
    if !has_git() {
        return;
    }
    let (t, store, repo) = shop(&EN);
    let stops = |dir: &Path, store: &Path, args: &[&str]| {
        let (code, out, err) = ritsu_in(dir, store, args);
        assert_eq!((code, out.as_str()), (2, ""), "{args:?}: {err}");
        err
    };
    assert_eq!(stops(&repo, &store, &["mod", "report"]), "error: `--base <revision>` is required: the revision to compare with\n");
    assert_eq!(stops(&repo, &store, &["mod", "report", "--lang", "ja"]), "エラー: `--base <git の版>`（比べる前の版）を付けてください\n");
    assert_eq!(stops(&repo, &store, &["mod", "report", "--base", "nowhere"]), "error: the git revision `nowhere` cannot be resolved\n");
    assert_eq!(stops(&repo, &store, &["mod", "report", "--base", "one", "--budget", "0"]), "error: `--budget 0` is not a positive integer\n");
    // a module in no repository
    let alone = t.path().join("alone");
    copy_dir(&repo, &alone);
    std::fs::remove_dir_all(alone.join(".git")).unwrap();
    let err = stops(&alone, &store, &["mod", "report", "--base", "one"]);
    assert!(err.starts_with("error: `") && err.ends_with("` is in no git repository\n"), "{err}");
}

// ── What a report reads, fetched ────────────────────────────────────────

/// The example's shop on a registry the test starts, in a repository of its own: tax 1.0.0 and
/// pricing 1.0.0 and 1.1.0 pushed (each `require` from the registry, a `ritsu >=` line under each
/// version), and two commits of shop, tagged `one` (pricing 1.0.0) and `two` (pricing 1.1.0, the
/// working tree). The pushes and the locks use the store `pushed/`, and every run a Docker config of
/// the test's own, with nothing in it.
struct OnRegistry {
    t: TempDir,
    r: Registry,
}

impl OnRegistry {
    fn new(what: &str) -> OnRegistry {
        let r = Registry::start();
        let t = TempDir::new(what);
        let host = r.host.clone();
        let from = here().join("tests/projects/modules");
        for (src, dst) in [("tax", "tax"), ("pricing", "pricing"), ("shop", "shop"), ("versions/pricing-1.0.0", "pricing-1.0.0")] {
            copy_dir(&from.join(src), &t.path().join(dst));
            let file = t.path().join(dst).join("ritsu.mod");
            let text = std::fs::read_to_string(&file)
                .unwrap()
                .replace("example.com/acme/", &format!("{host}/acme/"))
                .replace("require tax path \"../tax\"", &format!("require tax oci \"{host}/acme/tax\" 1.0.0"))
                .replace("require tax path \"../../tax\"", &format!("require tax oci \"{host}/acme/tax\" 1.0.0"))
                .replace("require pricing path \"../pricing\"", &format!("require pricing oci \"{host}/acme/pricing\" 1.0.0"));
            let mut out = String::new();
            for line in text.split_inclusive('\n') {
                out.push_str(line);
                if line.starts_with("version ") {
                    out.push_str("ritsu >=0.36.0\n");
                }
            }
            std::fs::write(&file, out).unwrap();
        }
        std::fs::create_dir_all(t.path().join("docker")).unwrap();
        let w = OnRegistry { t, r };
        let pushed = w.path("pushed");
        let step = |dir: &str, step: &str| {
            let (code, out, err) = w.run(dir, &pushed, &["mod", step]);
            assert_eq!(code, 0, "ritsu mod {step} in {dir}:\n{out}{err}");
        };
        for (dir, s) in [("tax", "push"), ("pricing-1.0.0", "lock"), ("pricing-1.0.0", "fetch"), ("pricing-1.0.0", "push"), ("pricing", "lock"), ("pricing", "push"), ("shop", "lock")] {
            step(dir, s);
        }
        let repo = w.path("shop");
        git(&repo, &["init", "-q"]);
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-q", "-m", "pricing 1.0.0"]);
        git(&repo, &["tag", "one"]);
        replace(&repo, "ritsu.mod", &format!("\"{host}/acme/pricing\" 1.0.0"), &format!("\"{host}/acme/pricing\" 1.1.0"));
        step("shop", "lock");
        git(&repo, &["commit", "-q", "-a", "-m", "pricing 1.1.0"]);
        git(&repo, &["tag", "two"]);
        w
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.t.path().join(rel)
    }

    /// ritsu, run in the directory `dir` of the test with the store at `store`.
    fn run(&self, dir: &str, store: &Path, args: &[&str]) -> (i32, String, String) {
        let mut c = command(&self.path(dir), store, args);
        c.env("DOCKER_CONFIG", self.path("docker"));
        said(c.output().expect("could not run ritsu"))
    }

    /// The digest of the OCI manifest of the version `tag` of `repository` in the registry.
    fn digest(&self, repository: &str, tag: &str) -> String {
        self.r.tag(repository, tag).unwrap_or_else(|| panic!("the registry has no {repository}:{tag}"))
    }
}

/// Whether the store at `store` holds an entry, or a line of its index.
fn holds_any(store: &Path) -> bool {
    store.join("sha256").exists() || store.join("refs").exists()
}

/// What the two locks pin, fetched into an empty store by the report itself: the base's (pricing
/// 1.0.0 and tax), then the head's (pricing 1.1.0), each said on stderr, a line each. The report is
/// the one a store filled by `ritsu mod fetch --rev one` and `ritsu mod fetch` gives (as JSON and as
/// Markdown); with the store full, the registry is asked nothing and nothing is said. What stops a
/// fetch stops the report (exit 2), nothing put in the store: an OCI manifest that is not its
/// digest, and a registry that does not answer.
#[test]
fn what_a_report_reads_is_fetched() {
    if !has_git() {
        return;
    }
    let w = OnRegistry::new("mod-report-fetch");
    let host = w.r.host.clone();
    let (d10, d11, tax) = (w.digest("acme/pricing", "1.0.0"), w.digest("acme/pricing", "1.1.0"), w.digest("acme/tax", "1.0.0"));
    let fetched = |name: &str, version: &str, d: &str| format!("fetched {host}/acme/{name} {version} ({d})\n");
    let three = [fetched("pricing", "1.0.0", &d10), fetched("tax", "1.0.0", &tax), fetched("pricing", "1.1.0", &d11)].concat();

    let report = ["mod", "report", "--base", "one", "--format", "json"];
    let empty = w.path("store-empty");
    let (code, first, err) = w.run("shop", &empty, &report);
    assert_eq!((code, err.as_str()), (0, three.as_str()), "{first}");
    let v = json(&first);
    assert_eq!((&v["modules"][0]["module"], &v["modules"][0]["change"]), (&serde_json::json!(format!("{host}/acme/pricing")), &serde_json::json!("upgraded")), "{first}");
    w.r.clear_log();
    assert_eq!(w.run("shop", &empty, &report), (0, first.clone(), String::new()));
    // a store `ritsu mod fetch --rev one` and `ritsu mod fetch` filled: the same report, the
    // registry asked nothing
    let full = w.path("store-full");
    for args in [&["mod", "fetch", "--rev", "one"][..], &["mod", "fetch"]] {
        let (code, out, err) = w.run("shop", &full, args);
        assert_eq!(code, 0, "{args:?}: {out}{err}");
    }
    w.r.clear_log();
    assert_eq!(w.run("shop", &full, &report), (0, first, String::new()));
    let markdown = ["mod", "report", "--base", "one", "--format", "markdown", "--lang", "ja"];
    let (_, from_full, _) = w.run("shop", &full, &markdown);
    assert_eq!(w.r.log().len(), 0);
    let (code, out, err) = w.run("shop", &w.path("store-markdown"), &markdown);
    assert_eq!((code, out), (0, from_full));
    assert!(err.starts_with(&format!("{host}/acme/pricing 1.0.0 を取り出しました（{d10}）\n")), "{err}");

    // other bytes for tax's OCI manifest: the base's lock cannot be fetched, and nothing is put
    let manifest = w.r.get("acme/tax", &tax).unwrap();
    let other = String::from_utf8(manifest.clone()).unwrap().replace("Acme", "Acne");
    assert_ne!(other.as_bytes(), &manifest[..]);
    w.r.serve_instead(&tax, other.as_bytes());
    let x = format!("sha256:{}", reg::sha256_hex(other.as_bytes()));
    for (lang, want) in [
        ("en", format!("error: cannot fetch the modules of the lock of `one`: the bytes fetched as the OCI manifest of `{host}/acme/tax` 1.0.0 have the digest {x}, not the {tax} asked for\n")),
        ("ja", format!("エラー: `one` の lock のモジュールを取り出せません。`{host}/acme/tax` 1.0.0 の OCI のマニフェストとして受け取ったバイトのダイジェストは {x} で、求めた {tax} ではありません\n")),
    ] {
        let store = w.path(&format!("store-other-{lang}"));
        assert_eq!(w.run("shop", &store, &["mod", "report", "--base", "one", "--lang", lang]), (2, String::new(), want));
        assert!(!holds_any(&store), "{lang}");
    }
    w.r.serve_instead(&tax, &manifest);

    // a registry that does not answer: what curl said, after the three tries (two and four seconds
    // apart; the two languages at once)
    let OnRegistry { t, r } = w;
    drop(r);
    let url = format!("http://{host}/v2/acme/pricing/manifests/{d10}");
    let runs: Vec<_> = ["en", "ja"]
        .iter()
        .map(|lang| {
            let store = t.path().join(format!("store-down-{lang}"));
            let mut c = command(&t.path().join("shop"), &store, &["mod", "report", "--base", "one", "--lang", lang]);
            c.env("DOCKER_CONFIG", t.path().join("docker")).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
            (store, c.spawn().expect("could not run ritsu"))
        })
        .collect();
    for ((store, child), (head, tail)) in runs.into_iter().zip([
        (format!("error: cannot fetch the modules of the lock of `one`: cannot reach `GET {url}` (curl exited with 7: "), ")\n"),
        (format!("エラー: `one` の lock のモジュールを取り出せません。`GET {url}` に届きません（curl の終了コード 7: "), "）\n"),
    ]) {
        let (code, out, err) = said(child.wait_with_output().unwrap());
        assert_eq!((code, out.as_str()), (2, ""), "{err}");
        assert!(err.starts_with(&head) && err.ends_with(tail) && err.lines().count() == 1, "{err}");
        assert!(!holds_any(&store));
    }
}

/// Two modules that changed, one of which cannot be compared. Shop requires tax itself, above the
/// version pricing requires: `three` pins pricing 1.0.0 and tax 1.1.0, `four` (the working tree)
/// pricing 1.1.0 and tax 1.2.0, so neither lock pins tax 1.0.0, which each version of pricing pins
/// in its own lock. With other bytes for tax 1.0.0's OCI manifest, what pricing 1.0.0 reads cannot
/// be fetched: pricing's difference is the error in its place, tax's is made, the top counts the
/// module not compared, and the report exits 2, with `--fail-on-change` too (no difference is shown
/// in behavior: tax 1.2.0 answers as 1.1.0 does), nothing of tax 1.0.0 put in the store.
#[test]
fn a_module_that_cannot_be_compared_makes_exit_2() {
    if !has_git() {
        return;
    }
    let w = OnRegistry::new("mod-report-not-compared");
    let host = w.r.host.clone();
    let pushed = w.path("pushed");
    for v in ["1.1.0", "1.2.0"] {
        let dir = format!("tax-{v}");
        copy_dir(&w.path("tax"), &w.path(&dir));
        replace(&w.path(&dir), "ritsu.mod", "version 1.0.0", &format!("version {v}"));
        let (code, out, err) = w.run(&dir, &pushed, &["mod", "push"]);
        assert_eq!(code, 0, "{out}{err}");
    }
    let repo = w.path("shop");
    let pricing = |v: &str| format!("\"{host}/acme/pricing\" {v}");
    let tax = |v: &str| format!("require tax oci \"{host}/acme/tax\" {v}");
    for (from, to, tag) in [
        (pricing("1.1.0"), format!("{}\n{}", pricing("1.0.0"), tax("1.1.0")), "three"),
        (format!("{}\n{}", pricing("1.0.0"), tax("1.1.0")), format!("{}\n{}", pricing("1.1.0"), tax("1.2.0")), "four"),
    ] {
        replace(&repo, "ritsu.mod", &from, &to);
        let (code, out, err) = w.run("shop", &pushed, &["mod", "lock"]);
        assert_eq!(code, 0, "{out}{err}");
        git(&repo, &["commit", "-q", "-a", "-m", tag]);
        git(&repo, &["tag", tag]);
    }
    let digest = |name: &str, v: &str| w.digest(&format!("acme/{name}"), v);
    let (d10, d11, t10, t11, t12) = (digest("pricing", "1.0.0"), digest("pricing", "1.1.0"), digest("tax", "1.0.0"), digest("tax", "1.1.0"), digest("tax", "1.2.0"));
    let manifest = w.r.get("acme/tax", &t10).unwrap();
    let other = String::from_utf8(manifest).unwrap().replace("Acme", "Acne");
    w.r.serve_instead(&t10, other.as_bytes());
    let x = format!("sha256:{}", reg::sha256_hex(other.as_bytes()));

    let store = w.path("store");
    let (code, out, err) = w.run("shop", &store, &["mod", "report", "--base", "three", "--format", "json"]);
    let fetched = |name: &str, v: &str, d: &str| format!("fetched {host}/acme/{name} {v} ({d})\n");
    let four = [fetched("pricing", "1.0.0", &d10), fetched("tax", "1.1.0", &t11), fetched("pricing", "1.1.0", &d11), fetched("tax", "1.2.0", &t12)].concat();
    assert_eq!((code, err), (2, four), "{out}");
    let v = json(&out);
    let why = format!("cannot fetch the modules `@pricing@1.0.0` reads: the bytes fetched as the OCI manifest of `{host}/acme/tax` 1.0.0 have the digest {x}, not the {t10} asked for");
    let modules = v["modules"].as_array().unwrap();
    assert_eq!(modules.len(), 2, "{out}");
    assert_eq!((&modules[0]["module"], &modules[0]["diff"], &modules[0]["error"]), (&serde_json::json!(format!("{host}/acme/pricing")), &serde_json::Value::Null, &serde_json::json!(why)));
    assert_eq!((&modules[1]["module"], &modules[1]["change"], &modules[1]["error"]), (&serde_json::json!(format!("{host}/acme/tax")), &serde_json::json!("upgraded"), &serde_json::Value::Null));
    assert_eq!(modules[1]["diff"]["files"]["changed"], serde_json::json!(["ritsu.mod"]));
    assert_eq!((&v["check"]["head"]["errors"], &v["ok"]), (&serde_json::json!(0), &serde_json::json!(false)));
    assert!(!store.join("sha256").join(t10.trim_start_matches("sha256:")).exists());
    // no difference shown in behavior: --fail-on-change leaves it 2
    assert_eq!(w.run("shop", &store, &["mod", "report", "--base", "three", "--fail-on-change"]).0, 2);
    // the text, in each language: the count at the top, the reason in pricing's place, the last line
    for (lang, count, reason, last) in [
        ("en", "modules not compared: 1\n", format!("\nnot compared: {why}\n"), "\nthe check with the lock of the working tree has no error; 1 module could not be compared, so this does not say behavior stays the same, which makes exit 2.\n".to_string()),
        (
            "ja",
            "比べられなかったモジュール：1 個\n",
            format!("\n比べられません：`@pricing@1.0.0` が読むモジュールを取り出せません。`{host}/acme/tax` 1.0.0 の OCI のマニフェストとして受け取ったバイトのダイジェストは {x} で、求めた {t10} ではありません\n"),
            "\n作業ツリーの lock での検査にエラーはありません。比べられなかったモジュールが 1 個あり、振る舞いが変わらないとは言えないので、exit 2 です。\n".to_string(),
        ),
    ] {
        let (code, out, err) = w.run("shop", &store, &["mod", "report", "--base", "three", "--lang", lang]);
        assert_eq!((code, err.as_str()), (2, ""), "{out}");
        assert!(out.contains(count) && out.contains(&reason) && out.ends_with(&last), "{out}");
        let (code, md, _) = w.run("shop", &store, &["mod", "report", "--base", "three", "--lang", lang, "--format", "markdown"]);
        assert_eq!(code, 2, "{md}");
        assert!(md.contains(&format!("|\n\n**{}**{}", if lang == "ja" { "比べられなかったモジュール" } else { "Modules not compared" }, if lang == "ja" { "：1 個\n" } else { ": 1\n" })), "{md}");
    }
}
