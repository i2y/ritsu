//! `ritsu rulec diff --fixtures` over a rule whose date input takes the days a koyomi date comes to
//! (rulec's DESIGN §15.174, §15.200). Only ritsu joins koyomi, so this is where the days are held
//! to both versions: the records are read the way the old version reads them, and a day the new
//! version's koyomi file does not come to is an input the new version turns away — a difference,
//! with its reason — while a day the old version's does not come to is a record left out, which
//! `fixtures lint` names.
//!
//! The old version pays on the 10th of the next month and the new one on the 15th. The English
//! rule is rulec's `tests/days/settlement.rule`; the Japanese version is dandori's `精算.rule`.

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
