//! `ritsu rulec diff` where only ritsu can answer.
//!
//! - `--fixtures` over a rule whose date input takes the days a koyomi date comes to (rulec's
//!   DESIGN §15.174, §15.200). Only ritsu joins koyomi, so this is where the days are held to both
//!   versions: the records are read the way the old version reads them, and a day the new version's
//!   koyomi file does not come to is an input the new version turns away — a difference, with its
//!   reason — while a day the old version's does not come to is a record left out, which `fixtures
//!   lint` names. The old version pays on the 10th of the next month and the new one on the 15th.
//!   The English rule is rulec's `tests/days/settlement.rule`; the Japanese version is dandori's
//!   `精算.rule`.
//! - The sides of a module's (ritsu's DESIGN 17.9.1, PLAN 8.3 C.1): a dependency's file at the
//!   lock's version (`@pricing/…`) against the same file at another version of the store
//!   (`@pricing@1.0.0/…`, the store made with `put_directory`); a git revision after a dependency's
//!   file, refused (exit 2); and a commit that raises only the dependencies, whose rule files are
//!   the same before and after: read at each revision with that revision's lock, the difference of
//!   the dependency comes out in `diff`, `check --diff-base` and `replay`, and a version the store
//!   does not have says `ritsu mod fetch --rev <revision>` (exit 2).

use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::process::Command;

fn crates() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).args(args);
    for v in ["RITSU_LANG", "RULEC_LANG", "KOYOMI_LANG"] {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn edit(src: &str, from: &str, to: &str) -> String {
    assert!(src.contains(from), "the rewrite does not apply: `{from}`");
    src.replace(from, to)
}

struct Settlement {
    lang: &'static str,
    rule: &'static str,
    rule_name: &'static str,
    cal: &'static str,
    cal_name: &'static str,
    /// The rule's line naming the koyomi file, and the same naming it beside the rule.
    from: (&'static str, &'static str),
    pays: (&'static str, &'static str),
    /// The rule's rows and examples on the 10th, and the same on the 15th.
    days: &'static [(&'static str, &'static str)],
    records: &'static str,
    /// What the text says of the two days the new version does not take, and of the one the old
    /// version does not.
    said: [&'static str; 3],
}

const EN: Settlement = Settlement {
    lang: "en",
    rule: "rulec/tests/days/settlement.rule",
    rule_name: "settlement.rule",
    cal: "rulec/tests/days/payment_terms.cal",
    cal_name: "payment_terms.cal",
    from: ("\"payment_terms.cal\"", "\"payment_terms.cal\""),
    pays: ("day 10 of month +1    # pays on the 10th of the next month", "day 15 of month +1    # pays on the 15th of the next month"),
    days: &[
        ("| >=2026-07-10 <=2026-12-10 |", "| >=2026-07-15 <=2026-12-15 |"),
        ("| 2026-02-10 |", "| 2026-02-15 |"),
        ("| 2026-07-10 |", "| 2026-07-15 |"),
        ("| 2027-01-10 |", "| 2027-01-15 |"),
    ],
    records: r#"{"in":{"pay_day":"2026-02-10"},"observed":{"batch":"first_half"}}
{"in":{"pay_day":"2026-07-10"},"observed":{"batch":"second_half"}}
{"in":{"pay_day":"2026-03-11"},"observed":{"batch":"first_half"}}
"#,
    said: [
        "pay_day is not a day the new version's koyomi \"payment_terms.cal\" date payment comes to",
        "pay_day is outside the new version's range 2026-02-15..2027-01-15",
        "`in.pay_day`: 2026-03-11 is not a day koyomi \"payment_terms.cal\" date payment comes to",
    ],
};

const JA: Settlement = Settlement {
    lang: "ja",
    rule: "dandori/tests/flows/rules/精算.rule",
    rule_name: "精算.rule",
    cal: "dandori/tests/flows/dates/支払条件.cal",
    cal_name: "支払条件.cal",
    from: ("\"../dates/支払条件.cal\"", "\"支払条件.cal\""),
    pays: ("day 10 of month +1    # 翌月 10 日払い", "day 15 of month +1    # 翌月 15 日払い"),
    days: &[
        ("| >=2026-07-10 <=2026-12-10 |", "| >=2026-07-15 <=2026-12-15 |"),
        ("| 2026-02-10 |", "| 2026-02-15 |"),
        ("| 2026-07-10 |", "| 2026-07-15 |"),
        ("| 2027-01-10 |", "| 2027-01-15 |"),
    ],
    records: r#"{"in":{"支払日":"2026-02-10"},"observed":{"精算の回":"前半"}}
{"in":{"支払日":"2026-07-10"},"observed":{"精算の回":"後半"}}
{"in":{"支払日":"2026-03-11"},"observed":{"精算の回":"前半"}}
"#,
    said: [
        "支払日 が、新しい版の koyomi \"支払条件.cal\" date 支払日 がとる日ではありません",
        "支払日 が新しい版の範囲 2026-02-15..2027-01-15 の外です",
        "`in.支払日`: 2026-03-11 は koyomi \"支払条件.cal\" date 支払日 がとる日ではありません",
    ],
};

#[test]
fn a_day_the_new_version_s_koyomi_file_does_not_come_to_is_a_difference() {
    for s in [&EN, &JA] {
        let t = TempDir::new(&format!("rulec-diff-days-{}", s.lang));
        let d = t.path();
        let rule = std::fs::read_to_string(crates().join(s.rule)).unwrap();
        let rule = edit(&rule, s.from.0, s.from.1);
        let cal = std::fs::read_to_string(crates().join(s.cal)).unwrap();
        let mut later = rule.clone();
        for (from, to) in s.days {
            later = edit(&later, from, to);
        }
        for (dir, rule, cal) in [("old", rule.clone(), cal.clone()), ("new", later, edit(&cal, s.pays.0, s.pays.1))] {
            std::fs::create_dir_all(d.join(dir)).unwrap();
            std::fs::write(d.join(dir).join(s.rule_name), rule).unwrap();
            std::fs::write(d.join(dir).join(s.cal_name), cal).unwrap();
            let (code, out, err) = ritsu_in(d, &["rulec", "check", &format!("{dir}/{}", s.rule_name), "--lang", s.lang]);
            assert_eq!(code, 0, "{dir}: {out}{err}");
        }
        std::fs::write(d.join("r.jsonl"), s.records).unwrap();
        let (old, new) = (format!("old/{}", s.rule_name), format!("new/{}", s.rule_name));

        let (code, out, err) = ritsu_in(d, &["rulec", "diff", &old, &new, "--fixtures", "r.jsonl", "--format", "json", "--lang", s.lang]);
        assert_eq!(code, 1, "{}: {out}{err}", s.lang);
        let j: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!((j["compared"].as_i64(), j["matched"].as_i64()), (Some(2), Some(0)), "{}: {out}", s.lang);
        assert_eq!(j["excluded"]["bad_format"].as_i64(), Some(1), "{}: the day the old version does not take is left out: {out}", s.lang);
        let mut kinds: Vec<(String, i64)> =
            j["refused"].as_array().unwrap().iter().map(|r| (r["kind"].as_str().unwrap().to_string(), r["records"][0]["line"].as_i64().unwrap())).collect();
        kinds.sort();
        assert_eq!(kinds, [("input_day".to_string(), 2), ("input_range".to_string(), 1)], "{}: {out}", s.lang);

        let (_, text, _) = ritsu_in(d, &["rulec", "diff", &old, &new, "--fixtures", "r.jsonl", "--lang", s.lang]);
        assert!(text.contains(s.said[0]) && text.contains(s.said[1]), "{}: {text}", s.lang);
        let (code, text, _) = ritsu_in(d, &["rulec", "fixtures", "lint", "r.jsonl", &old, "--lang", s.lang]);
        assert_eq!(code, 1, "{}: {text}", s.lang);
        assert!(text.contains(s.said[2]), "{}: {text}", s.lang);
    }
}

// ── The sides of a module (ritsu's DESIGN 17.9.1) ───────────────────────

use ritsu_base::modules::store::{self, Store};

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// `ritsu`, run in `dir` with the store at `store`: the exit code, what it printed and what it said
/// on stderr.
fn ritsu_with(dir: &Path, store: &Path, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).args(args).env(store::VARIABLE, store);
    for v in ["RITSU_LANG", "RULEC_LANG", "KOYOMI_LANG"] {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
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

fn put(store: &Path, dir: &Path) {
    store::put_directory(&Store::at(store), dir, "2026-10-28", &[]).unwrap_or_else(|e| panic!("{}", e.en));
}

fn json(out: &str) -> serde_json::Value {
    serde_json::from_str(out).unwrap_or_else(|e| panic!("{e}: {out}"))
}

/// The names of the example in one language: its directory, the module at hand, pricing, tax, the
/// versions of pricing, and the directory of the rules.
struct Names {
    lang: &'static str,
    dir: &'static str,
    shop: &'static str,
    pricing: &'static str,
    tax: &'static str,
    pricing_100: &'static str,
    rules: &'static str,
    fee: &'static str,
    total: &'static str,
    vat: &'static str,
}

const EN_NAMES: Names = Names {
    lang: "en",
    dir: "modules",
    shop: "shop",
    pricing: "pricing",
    tax: "tax",
    pricing_100: "versions/pricing-1.0.0",
    rules: "rules",
    fee: "shipping_fee.rule",
    total: "order_total.rule",
    vat: "vat_rate.rule",
};

const JA_NAMES: Names = Names {
    lang: "ja",
    dir: "modules.ja",
    shop: "店",
    pricing: "価格",
    tax: "税",
    pricing_100: "版/価格-1.0.0",
    rules: "規則",
    fee: "送料.rule",
    total: "注文の合計.rule",
    vat: "付加価値税率.rule",
};

/// The example with pricing and tax from a registry: tax and both versions of pricing in a store of
/// the test's own, and the lock of the module at hand written by `ritsu mod lock`.
fn from_a_registry(n: &Names) -> (TempDir, PathBuf) {
    let t = TempDir::new(&format!("rulec-diff-modules-{}", n.lang));
    let d = t.path();
    let src = here().join("tests/projects").join(n.dir);
    for m in [n.shop, n.pricing, n.tax, n.pricing_100] {
        copy_dir(&src.join(m), &d.join(m));
    }
    let tax_line = "require tax oci \"example.com/acme/tax\" 1.0.0";
    replace(d, &format!("{}/ritsu.mod", n.pricing), &format!("require tax path \"../{}\"", n.tax), tax_line);
    replace(d, &format!("{}/ritsu.mod", n.pricing_100), &format!("require tax path \"../../{}\"", n.tax), tax_line);
    replace(d, &format!("{}/ritsu.mod", n.shop), &format!("require pricing path \"../{}\"", n.pricing), "require pricing oci \"example.com/acme/pricing\" 1.1.0");
    let store = d.join("store");
    for m in [n.tax, n.pricing, n.pricing_100] {
        put(&store, &d.join(m));
    }
    let (code, out, err) = ritsu_with(&d.join(n.shop), &store, &["mod", "lock"]);
    assert_eq!(code, 0, "{out}{err}");
    (t, store)
}

/// `@pricing/…` (the lock's 1.1.0) against `@pricing@1.0.0/…` from the store: the step of the weight
/// moved and the region eu came, so the fee differs (exit 1); the order total, the same in both
/// versions, does not (exit 0); a version the store does not have, and a git revision after a
/// dependency's file, stop with what to write (exit 2).
#[test]
fn a_dependency_s_file_at_a_version_of_the_store() {
    for n in [&EN_NAMES, &JA_NAMES] {
        let (t, store) = from_a_registry(n);
        let shop = t.path().join(n.shop);
        let old = format!("@pricing@1.0.0/{}/{}", n.rules, n.fee);
        let new = format!("@pricing/{}/{}", n.rules, n.fee);
        let (code, out, err) = ritsu_with(&shop, &store, &["rulec", "diff", &old, &new, "--format", "json", "--lang", n.lang]);
        assert_eq!(code, 1, "{}: {out}{err}", n.lang);
        let j = json(&out);
        assert_eq!((j["old"].as_str(), j["new"].as_str()), (Some(old.as_str()), Some(new.as_str())), "{}: {out}", n.lang);
        assert_eq!((j["cells"].as_i64(), j["same"].as_i64(), j["differing"].as_i64()), (Some(14), Some(10), Some(4)), "{}: {out}", n.lang);
        let (code, text, err) = ritsu_with(&shop, &store, &["rulec", "diff", &old, &new, "--lang", n.lang]);
        assert_eq!(code, 1, "{}: {text}{err}", n.lang);
        let said: &[&str] = if n.lang == "en" {
            &["  an enum gained a value: region.eu\n", "  region = domestic  and  weight >=2001g <=2500g\n    fee: 695 → 395\n"]
        } else {
            &["  列挙に値が増えた: 地域.EU\n", "  地域 = 国内  かつ  重さ >=2001g <=2500g\n    料金: 695 → 395\n"]
        };
        for s in said {
            assert!(text.contains(s), "{}: {text}", n.lang);
        }
        let (old_total, new_total) = (format!("@pricing@1.0.0/{}/{}", n.rules, n.total), format!("@pricing/{}/{}", n.rules, n.total));
        let (code, out, err) = ritsu_with(&shop, &store, &["rulec", "diff", &old_total, &new_total, "--lang", n.lang]);
        assert_eq!(code, 0, "{}: {out}{err}", n.lang);
        let (code, out, err) = ritsu_with(&shop, &store, &["rulec", "diff", &format!("@pricing@9.9.9/{}/{}", n.rules, n.fee), &new, "--lang", "en"]);
        assert_eq!((code, out.as_str()), (2, ""), "{}: {err}", n.lang);
        assert_eq!(err, "error: `pricing` 9.9.9 is neither in the store nor in the module's copy of it; run `ritsu mod fetch pricing@9.9.9`\n");
        let (code, out, err) = ritsu_with(&shop, &store, &["rulec", "diff", &format!("{new}@HEAD"), &new, "--lang", "en"]);
        assert_eq!((code, out.as_str()), (2, ""), "{}: {err}", n.lang);
        assert_eq!(err, format!(
            "error: `{new}@HEAD`: a dependency's file takes no git revision, which is for the files of the repository at hand; write a version of the dependency as `@pricing@<version>/{}/{}`\n",
            n.rules, n.fee
        ));
    }
}

fn git(dir: &Path, args: &[&str]) {
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=ritsu-test", "-c", "user.email=ritsu-test@example.com", "-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .expect("could not run git");
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
}

/// A commit that raises only the dependencies: pricing, a repository of its own, requires tax from a
/// registry at 1.0.0, then at 1.1.0, whose reduced rate is 6% where 1.0.0's is 5%. The rule files
/// are the same at both revisions. Each revision is read with its own `ritsu.mod` and `ritsu.lock`
/// (ritsu's DESIGN 17.9.1), so the raise comes out: `diff` of the two revisions differs for the
/// reduced category; `replay` at the base matches what the base answered, while the working tree,
/// whose pin of the applied rule no longer holds (E040), does not pass check. A store without the
/// base's tax stops `diff` and `check --diff-base` with `ritsu mod fetch --rev <revision>` (exit 2).
#[test]
fn a_commit_that_raises_only_the_dependencies() {
    if Command::new("git").arg("--version").output().is_err() {
        ritsu_testkit::skip("git is not on the PATH; a revision is read with it");
        return;
    }
    for n in [&EN_NAMES, &JA_NAMES] {
        let t = TempDir::new(&format!("rulec-diff-raise-{}", n.lang));
        let d = t.path();
        let src = here().join("tests/projects").join(n.dir);
        copy_dir(&src.join(n.pricing), &d.join("repo"));
        copy_dir(&src.join(n.tax), &d.join("tax-1.0.0"));
        copy_dir(&src.join(n.tax), &d.join("tax-1.1.0"));
        replace(d, "tax-1.1.0/ritsu.mod", "version 1.0.0", "version 1.1.0");
        let vat = format!("tax-1.1.0/{}/{}", n.rules, n.vat);
        let reduced = if n.lang == "en" { "| reduced  | 5%" } else { "| 軽減 | 5%" };
        replace(d, &vat, reduced, &reduced.replace("5%", "6%"));
        replace(d, &vat, reduced, &reduced.replace("5%", "6%"));
        let store = d.join("store");
        put(&store, &d.join("tax-1.0.0"));
        put(&store, &d.join("tax-1.1.0"));
        let repo = d.join("repo");
        replace(d, "repo/ritsu.mod", &format!("require tax path \"../{}\"", n.tax), "require tax oci \"example.com/acme/tax\" 1.0.0");
        let (code, out, err) = ritsu_with(&repo, &store, &["mod", "lock"]);
        assert_eq!(code, 0, "{out}{err}");
        git(&repo, &["init", "-q"]);
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-q", "-m", "tax 1.0.0"]);
        replace(d, "repo/ritsu.mod", "\"example.com/acme/tax\" 1.0.0", "\"example.com/acme/tax\" 1.1.0");
        let (code, out, err) = ritsu_with(&repo, &store, &["mod", "lock"]);
        assert_eq!(code, 0, "{out}{err}");
        git(&repo, &["commit", "-q", "-a", "-m", "tax 1.1.0"]);
        let changed = Command::new("git").arg("-C").arg(&repo).args(["diff", "--name-only", "HEAD~1", "HEAD"]).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&changed.stdout), "ritsu.lock\nritsu.mod\n");

        let total = format!("{}/{}", n.rules, n.total);
        let (old, new) = (format!("{total}@HEAD~1"), format!("{total}@HEAD"));
        let (code, out, err) = ritsu_with(&repo, &store, &["rulec", "diff", &old, &new, "--format", "json", "--lang", n.lang]);
        assert_eq!(code, 1, "{}: {out}{err}", n.lang);
        let j = json(&out);
        assert!(j["differing"].as_i64().is_some_and(|k| k > 0), "{}: {out}", n.lang);
        let (code, text, _) = ritsu_with(&repo, &store, &["rulec", "diff", &old, &total, "--lang", n.lang]);
        assert_eq!(code, 1, "{}: {text}", n.lang);
        let category = if n.lang == "en" { "category = reduced" } else { "区分 = 軽減" };
        assert!(text.contains(category), "{}: {text}", n.lang);
        // the same revision on both sides: the same rule, the same tax
        let (code, text, _) = ritsu_with(&repo, &store, &["rulec", "diff", &new, &total, "--lang", n.lang]);
        assert_eq!(code, 0, "{}: {text}", n.lang);

        // the pin of the applied rule no longer holds in the working tree; the base is checked with
        // its own lock (a store without its tax stops it, below)
        let (code, out, _) = ritsu_with(&repo, &store, &["rulec", "check", &total, "--diff-base", "HEAD~1", "--lang", "en"]);
        assert_eq!(code, 1, "{}: {out}", n.lang);
        assert!(out.contains("error[E040]"), "{}: {out}", n.lang);

        // what the base answered, replayed at the base: the base's tax, so the same answer; the
        // working tree, with 1.1.0, does not pass check (E040)
        let (subtotal, category_in, total_out) = if n.lang == "en" { ("subtotal", "category", "total") } else { ("小計", "区分", "合計") };
        let reduced_word = if n.lang == "en" { "reduced" } else { "軽減" };
        let records = format!("{{\"in\":{{\"{subtotal}\":1000,\"{category_in}\":\"{reduced_word}\",\"shipping\":0}},\"observed\":{{\"{total_out}\":1050}}}}\n");
        let records = if n.lang == "en" { records } else { records.replace("\"shipping\"", "\"送料\"") };
        std::fs::write(repo.join("r.jsonl"), records).unwrap();
        let (code, out, err) = ritsu_with(&repo, &store, &["rulec", "replay", &old, "--fixtures", "r.jsonl", "--lang", "en"]);
        assert_eq!(code, 0, "{}: {out}{err}", n.lang);
        let (code, out, err) = ritsu_with(&repo, &store, &["rulec", "replay", &total, "--fixtures", "r.jsonl", "--lang", "en"]);
        assert_eq!((code, err.as_str()), (2, format!("error: `{total}` does not pass check\n").as_str()), "{}: {out}", n.lang);

        // a store without the base's tax
        let thin = d.join("thin-store");
        put(&thin, &d.join("tax-1.1.0"));
        let (code, out, err) = ritsu_with(&repo, &thin, &["rulec", "diff", &old, &total, "--lang", "en"]);
        assert_eq!((code, out.as_str()), (2, ""), "{}: {err}", n.lang);
        assert_eq!(err, "error: the dependency `example.com/acme/tax` 1.0.0 is neither in the store nor in this module's copy of it; run `ritsu mod fetch --rev HEAD~1`\n");
        let (code, _, err) = ritsu_with(&repo, &thin, &["rulec", "check", &total, "--diff-base", "HEAD~1", "--lang", "ja"]);
        assert_eq!(code, 2, "{}: {err}", n.lang);
        assert!(err.contains("`ritsu mod fetch --rev HEAD~1` を走らせてください"), "{}: {err}", n.lang);
    }
}
