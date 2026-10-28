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
//!   a revision that is not one, a version the store does not have.

use ritsu_base::modules::store::{self, Store};
use ritsu_testkit::{TempDir, skip};
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
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
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

/// What stops a report, exit 2 and nothing printed: no `--base`, a revision that is not one, a
/// version the store does not have, a module in no repository.
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
    // a store without the base's pricing
    let thin = t.path().join("thin-store");
    put(&thin, &t.path().join(EN.tax));
    put(&thin, &t.path().join(EN.pricing));
    let err = stops(&repo, &thin, &["mod", "report", "--base", "one"]);
    assert!(err.contains("`example.com/acme/pricing` 1.0.0") && err.contains("`ritsu mod fetch --rev one`"), "{err}");
    // a module in no repository
    let alone = t.path().join("alone");
    copy_dir(&repo, &alone);
    std::fs::remove_dir_all(alone.join(".git")).unwrap();
    let err = stops(&alone, &store, &["mod", "report", "--base", "one"]);
    assert!(err.starts_with("error: `") && err.ends_with("` is in no git repository\n"), "{err}");
}
