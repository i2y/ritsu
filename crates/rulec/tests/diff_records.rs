//! `rulec diff <old> <new> --fixtures` reads the records the way the **old** version reads them
//! (DESIGN §15.200).
//!
//! The records are the old version's: its system wrote them. They used to be read as the new
//! version reads them, so `observed` was held to the new version's output range, and the
//! records whose answer an amendment moved out of it — the ones most worth comparing — were
//! thrown out as "not matching the declared format". The same records pass `fixtures lint`
//! against the old version. Now the records are read at the old version's names, types, steps,
//! units, ranges and enums (or `--read-as`'s steps and units), `observed` is not read at all,
//! and an input the new version turns away is a difference of its own, not a record left out.
//!
//! Every case is written twice: first in English, with the corpus's English twins or a small
//! rule written in English, then the Japanese version beside it.

use std::path::{Path, PathBuf};
use std::process::Command;
use ritsu_testkit::TempDir;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rulec(dir: &Path, lang: &str, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", lang)
        .current_dir(dir)
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn json(s: &str) -> rulec::json::Json {
    rulec::json::parse(s.trim()).unwrap_or_else(|e| panic!("JSON として読めない: {e}\n{s}"))
}

fn int(j: &rulec::json::Json, k: &str) -> i128 {
    j.get(k).and_then(|x| x.as_int()).unwrap_or_else(|| panic!("{k} が無い: {j:?}"))
}

fn arr<'a>(j: &'a rulec::json::Json, k: &str) -> &'a [rulec::json::Json] {
    match j.get(k) {
        Some(rulec::json::Json::Arr(v)) => v,
        _ => panic!("{k} が配列でない: {j:?}"),
    }
}

/// Whether a report left no record out.
fn nothing_left_out(j: &rulec::json::Json) -> bool {
    matches!(j.get("excluded"), Some(rulec::json::Json::Obj(m)) if m.is_empty())
}

fn s<'a>(j: &'a rulec::json::Json, k: &str) -> &'a str {
    j.get(k).and_then(|x| x.as_str()).unwrap_or_else(|| panic!("{k} が無い: {j:?}"))
}

/// A rewrite matched by content. If it does not match, it does not pass silently.
fn edit(src: &str, from: &str, to: &str) -> String {
    assert!(src.contains(from), "書き換えが当たっていない: `{from}`");
    src.replace(from, to)
}

fn write(dir: &Path, name: &str, body: &str) -> String {
    std::fs::write(dir.join(name), body).unwrap();
    name.to_string()
}

/// The lines of the records the clusters of a `--format json` report name, in order.
fn lines_of(cs: &[rulec::json::Json]) -> Vec<i128> {
    let mut v: Vec<i128> = cs.iter().flat_map(|c| arr(c, "records").iter().map(|r| int(r, "line"))).collect();
    v.sort();
    v
}

/// The corpus's retirement allowance in one language: the callee pays a short service in
/// months by its table, and the part-time rule applies it. The amendment pays six months
/// instead of five; the examples follow it, and the part-time rule pins the new callee.
struct Allowance {
    lang: &'static str,
    callee: &'static str,
    rule: &'static str,
    /// The file `gen` writes the vectors' expected answers to: the records the generated code's
    /// record function would have written for them (§15.34).
    expected: &'static str,
    short: (&'static str, &'static str),
    callee_examples: [(&'static str, &'static str); 2],
    rule_examples: (&'static str, &'static str),
    /// The declaration of the monthly pay, and the same capped at 400,000 yen.
    pay: (&'static str, &'static str),
    /// The declaration of the period in office.
    tenure: &'static str,
}

const EN: Allowance = Allowance {
    lang: "en",
    callee: "retirement_pay.rule",
    rule: "part_time_retirement_pay.rule",
    expected: "part_time_retirement_pay.expected.jsonl",
    short: ("short | <10      | retirement_age, voluntary | 5 ", "short | <10      | retirement_age, voluntary | 6 "),
    callee_examples: [
        ("| retirement_age | 300_000JPY | 1_500_000JPY  |", "| retirement_age | 300_000JPY | 1_800_000JPY  |"),
        ("| voluntary      | 300_000JPY | 1_200_000JPY  |", "| voluntary      | 300_000JPY | 1_440_000JPY  |"),
    ],
    rule_examples: ("300_000JPY  | 1_500_000JPY ", "300_000JPY  | 1_800_000JPY "),
    pay: ("monthly_pay : money[JPY]  range >=100_000JPY <=500_000JPY", "monthly_pay : money[JPY]  range >=100_000JPY <=400_000JPY"),
    tenure: "tenure",
};

const JA: Allowance = Allowance {
    lang: "ja",
    callee: "退職手当.rule",
    rule: "非常勤退職手当.rule",
    expected: "part_time_allowance.expected.jsonl",
    short: ("短期 | <10      | 定年, 自己都合 | 5 ", "短期 | <10      | 定年, 自己都合 | 6 "),
    callee_examples: [
        ("| 定年     | 30万円 | 150万円  |", "| 定年     | 30万円 | 180万円  |"),
        ("| 自己都合 | 30万円 | 120万円  |", "| 自己都合 | 30万円 | 144万円  |"),
    ],
    rule_examples: ("30万円   | 150万円 ", "30万円   | 180万円 "),
    pay: ("報酬月額(monthly_pay)  : money[円]  range >=10万円 <=50万円", "報酬月額(monthly_pay)  : money[円]  range >=10万円 <=40万円"),
    tenure: "在職期間",
};

/// `old/` holds the corpus's two files, `new/` the amended pair (pinned again by
/// `rulec source pin`, as an author does), `capped/` the amended pair whose part-time rule also
/// caps the monthly pay at 400,000 yen, and `cap_only/` the corpus's pair with the cap alone.
/// `records.jsonl` is the expected answers `gen` writes for the corpus's version: fourteen
/// records, every one of which the amendment moves.
fn allowance(a: &Allowance) -> TempDir {
    let t = TempDir::new(&format!("diff-records-{}", a.lang));
    let d = t.path();
    let corpus = root().join("tests/corpus");
    let callee = std::fs::read_to_string(corpus.join(a.callee)).unwrap();
    let rule = std::fs::read_to_string(corpus.join(a.rule)).unwrap();
    let mut amended = edit(&callee, a.short.0, a.short.1);
    for (from, to) in a.callee_examples {
        amended = edit(&amended, from, to);
    }
    let rule_new = edit(&rule, a.rule_examples.0, a.rule_examples.1);
    for (dir, callee, rule) in [
        ("old", callee.clone(), rule.clone()),
        ("new", amended.clone(), rule_new.clone()),
        ("capped", amended.clone(), edit(&rule_new, a.pay.0, a.pay.1)),
        ("cap_only", callee.clone(), edit(&rule, a.pay.0, a.pay.1)),
    ] {
        std::fs::create_dir_all(d.join(dir)).unwrap();
        write(&d.join(dir), a.callee, &callee);
        write(&d.join(dir), a.rule, &rule);
        if dir != "old" && dir != "cap_only" {
            let (c, out, e) = rulec(d, a.lang, &["source", "pin", &format!("{dir}/{}", a.rule)]);
            assert_eq!(c, 0, "{dir}: pin が通らない: {out}{e}");
        }
        let (c, out, e) = rulec(d, a.lang, &["check", &format!("{dir}/{}", a.callee), &format!("{dir}/{}", a.rule)]);
        assert_eq!(c, 0, "{dir}: 検査を通らない: {out}{e}");
    }
    let (c, out, e) = rulec(d, a.lang, &["gen", &format!("old/{}", a.rule), "--out", "gen"]);
    assert_eq!(c, 0, "{out}{e}");
    std::fs::copy(d.join("gen/vectors").join(a.expected), d.join("records.jsonl")).unwrap();
    let first = std::fs::read_to_string(d.join("records.jsonl")).unwrap().lines().next().unwrap().to_string();
    write(d, "one.jsonl", &format!("{first}\n"));
    t
}

/// The reported defect, as it was found: the amendment moves every record's answer below the
/// new version's output range (500,000 yen against 600,000..), and every one of them used to be
/// thrown out. One record is compared now, and so are all fourteen.
#[test]
fn an_answer_moved_out_of_the_new_range_is_compared() {
    for a in [&EN, &JA] {
        let t = allowance(a);
        let d = t.path();
        let (old, new) = (format!("old/{}", a.rule), format!("new/{}", a.rule));

        let (c, out, _) = rulec(d, a.lang, &["diff", &old, &new, "--fixtures", "one.jsonl", "--format", "json"]);
        assert_eq!(c, 1, "{}: 答えが動いたのに 1 でない: {out}", a.lang);
        let j = json(&out);
        assert_eq!((int(&j, "compared"), int(&j, "matched")), (1, 0), "{}: {out}", a.lang);
        assert!(arr(&j, "refused").is_empty() && nothing_left_out(&j), "{}: {out}", a.lang);

        let (c, out, _) = rulec(d, a.lang, &["diff", &old, &new, "--fixtures", "records.jsonl", "--format", "json"]);
        assert_eq!(c, 1, "{}: {out}", a.lang);
        let j = json(&out);
        assert_eq!((int(&j, "compared"), int(&j, "matched")), (14, 0), "{}: 十四件とも照合して、十四件とも動く: {out}", a.lang);
        assert_eq!(lines_of(arr(&j, "clusters")), (1..=14).collect::<Vec<i128>>(), "{}: {out}", a.lang);

        // The text says the same, in each language.
        let (_, text, _) = rulec(d, a.lang, &["diff", &old, &new, "--fixtures", "one.jsonl"]);
        let head = if a.lang == "ja" { "照合 1 件 / 一致 0 (0.000%)" } else { "Compared 1 / matched 0 (0.000%)" };
        assert!(text.starts_with(head), "{}: {text}", a.lang);
        assert!(!text.contains("--read-as"), "{}: 読めているのに読み方を案内している: {text}", a.lang);

        // Against the version that wrote them, the records have no problem; against the new one
        // their `observed` is out of range. That is the new version's business only in `lint`.
        let (c, out, _) = rulec(d, a.lang, &["fixtures", "lint", "records.jsonl", &old, "--format", "json"]);
        assert_eq!(c, 0, "{}: {out}", a.lang);
        let (c, out, _) = rulec(d, a.lang, &["fixtures", "lint", "records.jsonl", &new, "--format", "json"]);
        assert_eq!(c, 1, "{}: {out}", a.lang);
        assert!(out.contains("\"kind\":\"bad_observed\""), "{}: {out}", a.lang);
    }
}

/// An input the old version took and the new one turns away is a difference, not a record
/// left out: counted as compared, never as matched, and listed by the reason with every record
/// by name. A version whose only change is to turn inputs away still ends with 1.
#[test]
fn an_input_the_new_version_turns_away_is_a_difference() {
    for a in [&EN, &JA] {
        let t = allowance(a);
        let d = t.path();
        let old = format!("old/{}", a.rule);
        let pay = if a.lang == "ja" { "報酬月額" } else { "monthly_pay" };

        // Six months, and the pay capped at 400,000: the eight records under the cap move, and
        // the six at 499,999 or 500,000 are turned away.
        let (c, out, _) = rulec(d, a.lang, &["diff", &old, &format!("capped/{}", a.rule), "--fixtures", "records.jsonl", "--format", "json"]);
        assert_eq!(c, 1, "{}: {out}", a.lang);
        let j = json(&out);
        assert_eq!((int(&j, "compared"), int(&j, "matched")), (14, 0), "{}: {out}", a.lang);
        assert_eq!(lines_of(arr(&j, "clusters")), [1, 2, 3, 4, 7, 10, 13, 14], "{}: {out}", a.lang);
        let refused = arr(&j, "refused");
        assert_eq!(refused.len(), 1, "{}: 理由は一つ: {out}", a.lang);
        assert_eq!((s(&refused[0], "kind"), s(&refused[0], "field"), int(&refused[0], "count")), ("input_range", pay, 6), "{}: {out}", a.lang);
        assert_eq!(lines_of(refused), [5, 6, 8, 9, 11, 12], "{}: {out}", a.lang);
        let w = refused[0].get("witness").unwrap();
        assert_eq!(int(w.get("in").unwrap(), pay), 499_999, "{}: 例は記録の値のまま: {out}", a.lang);
        assert!(w.get("ours").is_none(), "{}: 新しい版は答えていない: {out}", a.lang);
        assert_eq!(int(w.get("theirs").unwrap(), if a.lang == "ja" { "非常勤手当" } else { "part_time_allowance" }), 2_499_995, "{}: {out}", a.lang);

        let (_, text, _) = rulec(d, a.lang, &["diff", &old, &format!("capped/{}", a.rule), "--fixtures", "records.jsonl"]);
        let (head, reason) = if a.lang == "ja" {
            ("新しい版が受け付けない入力 6 件 (42.857%)", "報酬月額 が新しい版の範囲 10万円..40万円 の外です")
        } else {
            ("Not accepted by the new version 6 (42.857%)", "monthly_pay is outside the new version's range 100_000JPY..400_000JPY")
        };
        assert!(text.contains(head) && text.contains(reason), "{}: {text}", a.lang);
        let (_, md, _) = rulec(d, a.lang, &["diff", &old, &format!("capped/{}", a.rule), "--fixtures", "records.jsonl", "--format", "markdown", "--terse"]);
        let row = if a.lang == "ja" { "| 新しい版が受け付けない | 6 件 |" } else { "| Not accepted by the new version | 6 |" };
        assert!(md.contains(row) && md.contains(&format!("| {reason} | 6 |")), "{}: {md}", a.lang);
        assert!(!md.contains("499999"), "{}: --terse なのに記録の値が出ている: {md}", a.lang);

        // The cap alone: every answer under it is the same, and the run still ends with 1.
        let (c, text, _) = rulec(d, a.lang, &["diff", &old, &format!("cap_only/{}", a.rule), "--fixtures", "records.jsonl"]);
        assert_eq!(c, 1, "{}: 受け付けない入力があるのに 0: {text}", a.lang);
        let (matched, none) = if a.lang == "ja" { ("照合 14 件 / 一致 8 ", "不一致はありません") } else { ("Compared 14 / matched 8 ", "No mismatches") };
        assert!(text.starts_with(matched) && text.contains(head) && !text.contains(none), "{}: {text}", a.lang);

        // A version the records do not reach with anything new ends with 0.
        let (c, text, _) = rulec(d, a.lang, &["diff", &old, &old, "--fixtures", "records.jsonl"]);
        assert_eq!(c, 0, "{}: {text}", a.lang);
    }
}

/// A record the old version does not read is not one of its records, and is left out as
/// before, naming the version it was read as. `observed` is not read: a record without it is
/// compared, and saying it is missing is `fixtures lint`'s, against the version that wrote it.
#[test]
fn a_record_the_old_version_does_not_read_is_left_out() {
    for a in [&EN, &JA] {
        let t = allowance(a);
        let d = t.path();
        let (old, new) = (format!("old/{}", a.rule), format!("new/{}", a.rule));
        let first = std::fs::read_to_string(d.join("one.jsonl")).unwrap();
        let tenure = format!("\"{}\":1", a.tenure);
        let outside = edit(&first, &tenure, &format!("\"{}\":4", a.tenure));
        let bare = first[..first.find(",\"observed\"").unwrap()].to_string() + "}\n";
        write(d, "mixed.jsonl", &format!("{outside}{bare}"));

        let (c, out, _) = rulec(d, a.lang, &["diff", &old, &new, "--fixtures", "mixed.jsonl", "--format", "json"]);
        assert_eq!(c, 1, "{}: {out}", a.lang);
        let j = json(&out);
        assert_eq!((int(&j, "compared"), int(&j, "matched")), (1, 0), "{}: observed の無い記録も照合する: {out}", a.lang);
        assert_eq!(int(j.get("excluded").unwrap(), "bad_format"), 1, "{}: 旧版が読めない記録は外す: {out}", a.lang);
        assert!(arr(&j, "refused").is_empty(), "{}: 旧版が読めない記録を新しい版の差にしている: {out}", a.lang);
        let (_, text, _) = rulec(d, a.lang, &["diff", &old, &new, "--fixtures", "mixed.jsonl"]);
        let said = if a.lang == "ja" { "形式が旧版の宣言と食い違う記録を 1 件外しました" } else { "Excluded 1 record (not matching the old version's declared format)" };
        assert!(text.contains(said), "{}: {text}", a.lang);

        let (c, out, _) = rulec(d, a.lang, &["fixtures", "lint", "mixed.jsonl", &old, "--format", "json"]);
        assert_eq!(c, 1, "{}: {out}", a.lang);
        assert!(out.contains("\"kind\":\"bad_input\"") && out.contains("\"kind\":\"no_observed\""), "{}: {out}", a.lang);

        // Nothing the old version reads: not one record compared, and the run ends with 1,
        // pointing at the reading of an earlier version.
        write(d, "outside.jsonl", &outside);
        let (c, text, _) = rulec(d, a.lang, &["diff", &old, &new, "--fixtures", "outside.jsonl"]);
        assert_eq!(c, 1, "{}: {text}", a.lang);
        assert!(text.contains("--read-as"), "{}: {text}", a.lang);
    }
}

/// A parcel fee whose weight the records count in kilograms, and whose old version counts it in
/// grams: `--read-as` names the version that wrote the records, and they are read at its units
/// and brought to the old version's, as `replay` and `fixtures lint` read them.
struct Parcel {
    lang: &'static str,
    grams: &'static str,
    kilos: &'static str,
    raised: (&'static str, &'static str),
    records: &'static str,
    /// The old version with an input it no longer has, and the records the old system wrote
    /// with it.
    express: &'static str,
    express_records: &'static str,
    fill: &'static str,
}

const PARCEL_EN: Parcel = Parcel {
    lang: "en",
    grams: "rule parcel_fee v1

enum zone = domestic | overseas

inputs
  zone   : zone
  weight : mass[g]  range >=1000g <=30000g

outputs
  fee : money[USD]  round down(1USD)

table fees
policy unique
| zone     | weight  | -> fee : money[USD] |
| domestic | <=5000g | 5USD                |
| domestic | >5000g  | 9USD                |
| overseas | -       | 20USD               |
",
    kilos: "rule parcel_fee v0

enum zone = domestic | overseas

inputs
  zone   : zone
  weight : mass[kg]  range >=1kg <=30kg

outputs
  fee : money[USD]  round down(1USD)

table fees
policy unique
| zone     | weight | -> fee : money[USD] |
| domestic | <=5kg  | 5USD                |
| domestic | >5kg   | 9USD                |
| overseas | -      | 20USD               |
",
    raised: ("| domestic | >5000g  | 9USD ", "| domestic | >5000g  | 10USD"),
    records: r#"{"in":{"zone":"domestic","weight":3},"observed":{"fee":5}}
{"in":{"zone":"domestic","weight":8},"observed":{"fee":9}}
{"in":{"zone":"overseas","weight":2},"observed":{"fee":20}}
"#,
    express: "rule parcel_fee v2

enum zone = domestic | overseas

inputs
  zone    : zone
  weight  : mass[g]  range >=1000g <=30000g
  express : bool

outputs
  fee : money[USD]  round down(1USD)

table fees
policy unique
| zone     | weight  | express | -> fee : money[USD] |
| domestic | <=5000g | false   | 5USD                |
| domestic | <=5000g | true    | 8USD                |
| domestic | >5000g  | -       | 9USD                |
| overseas | -       | -       | 20USD               |
",
    express_records: r#"{"in":{"zone":"domestic","weight":3000,"express":true},"observed":{"fee":8}}
{"in":{"zone":"overseas","weight":2000,"express":false},"observed":{"fee":20}}
"#,
    fill: "express",
};

const PARCEL_JA: Parcel = Parcel {
    lang: "ja",
    grams: "rule 小包料金(parcel_fee) v1

enum 地域(zone) = 国内(domestic) | 海外(overseas)

inputs
  地域(zone)   : 地域
  重量(weight) : mass[g]  range >=1000g <=30000g

outputs
  料金(fee) : money[円]  round down(1円)

table 料金表(fees)
policy unique
| 地域 | 重量    | -> 料金 : money[円] |
| 国内 | <=5000g | 500円               |
| 国内 | >5000g  | 900円               |
| 海外 | -       | 2000円              |
",
    kilos: "rule 小包料金(parcel_fee) v0

enum 地域(zone) = 国内(domestic) | 海外(overseas)

inputs
  地域(zone)   : 地域
  重量(weight) : mass[kg]  range >=1kg <=30kg

outputs
  料金(fee) : money[円]  round down(1円)

table 料金表(fees)
policy unique
| 地域 | 重量  | -> 料金 : money[円] |
| 国内 | <=5kg | 500円               |
| 国内 | >5kg  | 900円               |
| 海外 | -     | 2000円              |
",
    raised: ("| 国内 | >5000g  | 900円 ", "| 国内 | >5000g  | 1000円"),
    records: r#"{"in":{"地域":"国内","重量":3},"observed":{"料金":500}}
{"in":{"地域":"国内","重量":8},"observed":{"料金":900}}
{"in":{"地域":"海外","重量":2},"observed":{"料金":2000}}
"#,
    express: "rule 小包料金(parcel_fee) v2

enum 地域(zone) = 国内(domestic) | 海外(overseas)

inputs
  地域(zone)    : 地域
  重量(weight)  : mass[g]  range >=1000g <=30000g
  速達(express) : bool

outputs
  料金(fee) : money[円]  round down(1円)

table 料金表(fees)
policy unique
| 地域 | 重量    | 速達  | -> 料金 : money[円] |
| 国内 | <=5000g | false | 500円               |
| 国内 | <=5000g | true  | 800円               |
| 国内 | >5000g  | -     | 900円               |
| 海外 | -       | -     | 2000円              |
",
    express_records: r#"{"in":{"地域":"国内","重量":3000,"速達":true},"observed":{"料金":800}}
{"in":{"地域":"海外","重量":2000,"速達":false},"observed":{"料金":2000}}
"#,
    fill: "速達",
};

fn parcel(p: &Parcel) -> TempDir {
    let t = TempDir::new(&format!("diff-records-parcel-{}", p.lang));
    let d = t.path();
    write(d, "grams.rule", p.grams);
    write(d, "raised.rule", &edit(p.grams, p.raised.0, p.raised.1));
    write(d, "kilos.rule", p.kilos);
    write(d, "express.rule", p.express);
    write(d, "kg.jsonl", p.records);
    write(d, "express.jsonl", p.express_records);
    for r in ["grams.rule", "raised.rule", "kilos.rule", "express.rule"] {
        let (c, out, e) = rulec(d, p.lang, &["check", r]);
        assert_eq!(c, 0, "{r}: {out}{e}");
    }
    t
}

#[test]
fn read_as_reads_the_records_at_the_version_that_wrote_them() {
    for p in [&PARCEL_EN, &PARCEL_JA] {
        let t = parcel(p);
        let d = t.path();
        // Read as the old version, 3 is 3 g: outside its range, and every record is left out.
        let (c, text, _) = rulec(d, p.lang, &["diff", "grams.rule", "raised.rule", "--fixtures", "kg.jsonl"]);
        assert_eq!(c, 1, "{}: {text}", p.lang);
        assert!(text.contains("--read-as"), "{}: 読み方を案内していない: {text}", p.lang);
        // Read as the version that wrote them, 3 is 3 kg: all three compared, the one over 5 kg
        // moved.
        let (c, out, e) = rulec(d, p.lang, &["diff", "grams.rule", "raised.rule", "--fixtures", "kg.jsonl", "--read-as", "kilos.rule", "--format", "json"]);
        assert_eq!(c, 1, "{}: {out}{e}", p.lang);
        let j = json(&out);
        assert_eq!((int(&j, "compared"), int(&j, "matched")), (3, 2), "{}: {out}", p.lang);
        assert_eq!(lines_of(arr(&j, "clusters")), [2], "{}: {out}", p.lang);
        let w = arr(&j, "clusters")[0].get("witness").unwrap().get("in").unwrap().clone();
        assert_eq!(int(&w, if p.lang == "ja" { "重量" } else { "weight" }), 8000, "{}: 記録の 8 kg を旧版の 8000 g で見せる: {out}", p.lang);
        // The same records, `replay`ed against the old version, read the same way.
        let (c, out, e) = rulec(d, p.lang, &["replay", "grams.rule", "--fixtures", "kg.jsonl", "--read-as", "kilos.rule", "--format", "json"]);
        assert_eq!(c, 0, "{}: {out}{e}", p.lang);
        assert_eq!((int(&json(&out), "compared"), int(&json(&out), "matched")), (3, 3), "{}: {out}", p.lang);
    }
}

/// An input only the new version takes is not in the old version's records: the default from
/// the manifest or `--fill` gives it, read at the new version's type, and every record answered
/// with it is a filled record. Without one, the records are left out for the missing field, as
/// before. An input only the old version has is in the records and simply not passed on.
#[test]
fn an_input_only_one_version_has() {
    for p in [&PARCEL_EN, &PARCEL_JA] {
        let t = parcel(p);
        let d = t.path();
        write(d, "g.jsonl", &p.records.replace(":3}", ":3000}").replace(":8}", ":8000}").replace(":2}", ":2000}"));
        let (c, out, _) = rulec(d, p.lang, &["diff", "grams.rule", "express.rule", "--fixtures", "g.jsonl", "--format", "json"]);
        assert_eq!(c, 1, "{}: {out}", p.lang);
        let j = json(&out);
        assert_eq!(int(&j, "compared"), 0, "{}: {out}", p.lang);
        assert_eq!(int(j.get("excluded").unwrap(), "missing_field"), 3, "{}: {out}", p.lang);

        let fill = format!("{}=true", p.fill);
        let (c, out, e) = rulec(d, p.lang, &["diff", "grams.rule", "express.rule", "--fixtures", "g.jsonl", "--fill", &fill, "--format", "json"]);
        assert_eq!(c, 1, "{}: {out}{e}", p.lang);
        let j = json(&out);
        let filled = j.get("filled").unwrap();
        assert_eq!((int(filled, "count"), int(filled.get("by_field").unwrap(), p.fill)), (3, 3), "{}: {out}", p.lang);
        assert_eq!(s(filled.get("defaults").unwrap(), p.fill), "true", "{}: {out}", p.lang);
        // Only the parcel at 3 kg is answered by the express row.
        assert_eq!(lines_of(arr(&j, "clusters")), [1], "{}: {out}", p.lang);
        write(d, "m.json", &format!("{{\"rule\":\"{}\",\"fills\":{{\"{}\":true}}}}", if p.lang == "ja" { "小包料金" } else { "parcel_fee" }, p.fill));
        let (c, out2, e) = rulec(d, p.lang, &["diff", "grams.rule", "express.rule", "--fixtures", "g.jsonl", "--manifest", "m.json", "--format", "json"]);
        assert_eq!(c, 1, "{}: {e}", p.lang);
        assert_eq!(out2, out, "{}: マニフェストと --fill で答えが違う", p.lang);

        // The other way round: the records carry an input the new version no longer takes.
        let (c, out, e) = rulec(d, p.lang, &["diff", "express.rule", "grams.rule", "--fixtures", "express.jsonl", "--format", "json"]);
        assert_eq!(c, 1, "{}: {out}{e}", p.lang);
        let j = json(&out);
        assert_eq!((int(&j, "compared"), int(&j, "matched")), (2, 1), "{}: {out}", p.lang);
        assert!(!out.contains("bad_format"), "{}: 新しい版に無い入力を形式の誤りにしている: {out}", p.lang);
    }
}

/// The reasons the new version turns an input away, each its own line and its own `kind`: a value
/// its enum no longer has, a weight that is no whole number of its new unit, a weight past its
/// new range, and a combination its new constraint rules out.
struct Quote {
    lang: &'static str,
    old: &'static str,
    new: &'static str,
    records: &'static str,
    /// The old version with the constraint, and the reason a record breaking it is reported.
    constrained: (&'static str, &'static str),
}

const QUOTE_EN: Quote = Quote {
    lang: "en",
    old: "rule quote v1

enum zone = domestic | overseas | remote

inputs
  zone     : zone
  weight   : mass[g]  range >=1g <=30000g
  declared : money[USD]  range >=0USD <=5000USD  contract_only
  cover    : money[USD]  range >=0USD <=5000USD  contract_only

outputs
  fee : money[USD]  round down(1USD)

table fees
policy unique
| zone     | weight  | -> fee : money[USD] |
| domestic | <=2000g | 5USD                |
| domestic | >2000g  | 9USD                |
| overseas | -       | 20USD               |
| remote   | -       | 30USD               |
",
    new: "rule quote v2

enum zone = domestic | overseas

inputs
  zone     : zone
  weight   : mass[kg]  range >=1kg <=20kg
  declared : money[USD]  range >=0USD <=5000USD
  cover    : money[USD]  range >=0USD <=5000USD

constraint declared <= cover

outputs
  fee : money[USD]  round down(1USD)

table fees
policy unique
| zone     | weight | -> fee : money[USD] |
| domestic | <=2kg  | 5USD                |
| domestic | >2kg   | 9USD                |
| overseas | -      | 20USD               |
",
    records: r#"{"in":{"zone":"domestic","weight":1000,"declared":100,"cover":200},"observed":{"fee":5}}
{"in":{"zone":"remote","weight":1000,"declared":100,"cover":200},"observed":{"fee":30}}
{"in":{"zone":"domestic","weight":1500,"declared":100,"cover":200},"observed":{"fee":5}}
{"in":{"zone":"domestic","weight":25000,"declared":100,"cover":200},"observed":{"fee":9}}
{"in":{"zone":"domestic","weight":3000,"declared":300,"cover":200},"observed":{"fee":9}}
{"in":{"zone":"overseas","weight":2000,"declared":0,"cover":0},"observed":{"fee":20}}
"#,
    constrained: ("  cover    : money[USD]  range >=0USD <=5000USD  contract_only\n", "  cover    : money[USD]  range >=0USD <=5000USD  contract_only\n\nconstraint declared <= cover\n"),
};

const QUOTE_JA: Quote = Quote {
    lang: "ja",
    old: "rule 見積(quote) v1

enum 地域(zone) = 国内(domestic) | 海外(overseas) | 離島(remote)

inputs
  地域(zone)       : 地域
  重量(weight)     : mass[g]  range >=1g <=30000g
  申告額(declared) : money[円]  range >=0円 <=50万円  contract_only
  補償額(cover)    : money[円]  range >=0円 <=50万円  contract_only

outputs
  料金(fee) : money[円]  round down(1円)

table 料金表(fees)
policy unique
| 地域 | 重量    | -> 料金 : money[円] |
| 国内 | <=2000g | 500円               |
| 国内 | >2000g  | 900円               |
| 海外 | -       | 2000円              |
| 離島 | -       | 3000円              |
",
    new: "rule 見積(quote) v2

enum 地域(zone) = 国内(domestic) | 海外(overseas)

inputs
  地域(zone)       : 地域
  重量(weight)     : mass[kg]  range >=1kg <=20kg
  申告額(declared) : money[円]  range >=0円 <=50万円
  補償額(cover)    : money[円]  range >=0円 <=50万円

constraint 申告額 <= 補償額

outputs
  料金(fee) : money[円]  round down(1円)

table 料金表(fees)
policy unique
| 地域 | 重量  | -> 料金 : money[円] |
| 国内 | <=2kg | 500円               |
| 国内 | >2kg  | 900円               |
| 海外 | -     | 2000円              |
",
    records: r#"{"in":{"地域":"国内","重量":1000,"申告額":10000,"補償額":20000},"observed":{"料金":500}}
{"in":{"地域":"離島","重量":1000,"申告額":10000,"補償額":20000},"observed":{"料金":3000}}
{"in":{"地域":"国内","重量":1500,"申告額":10000,"補償額":20000},"observed":{"料金":500}}
{"in":{"地域":"国内","重量":25000,"申告額":10000,"補償額":20000},"observed":{"料金":900}}
{"in":{"地域":"国内","重量":3000,"申告額":30000,"補償額":20000},"observed":{"料金":900}}
{"in":{"地域":"海外","重量":2000,"申告額":0,"補償額":0},"observed":{"料金":2000}}
"#,
    constrained: ("  補償額(cover)    : money[円]  range >=0円 <=50万円  contract_only\n", "  補償額(cover)    : money[円]  range >=0円 <=50万円  contract_only\n\nconstraint 申告額 <= 補償額\n"),
};

#[test]
fn each_reason_the_new_version_turns_an_input_away() {
    for q in [&QUOTE_EN, &QUOTE_JA] {
        let t = TempDir::new(&format!("diff-records-quote-{}", q.lang));
        let d = t.path();
        write(d, "old.rule", q.old);
        write(d, "new.rule", q.new);
        write(d, "constrained.rule", &edit(q.old, q.constrained.0, q.constrained.1));
        write(d, "r.jsonl", q.records);
        for r in ["old.rule", "new.rule", "constrained.rule"] {
            let (c, out, e) = rulec(d, q.lang, &["check", r]);
            assert_eq!(c, 0, "{r}: {out}{e}");
        }
        let (c, out, e) = rulec(d, q.lang, &["diff", "old.rule", "new.rule", "--fixtures", "r.jsonl", "--format", "json"]);
        assert_eq!(c, 1, "{}: {out}{e}", q.lang);
        let j = json(&out);
        assert_eq!((int(&j, "compared"), int(&j, "matched")), (6, 2), "{}: {out}", q.lang);
        assert!(arr(&j, "clusters").is_empty(), "{}: 受け付けた入力の答えは同じ: {out}", q.lang);
        let mut got: Vec<(String, Vec<i128>)> = arr(&j, "refused").iter().map(|r| (s(r, "kind").to_string(), lines_of(std::slice::from_ref(r)))).collect();
        got.sort();
        let want: Vec<(String, Vec<i128>)> =
            [("constraint", 5), ("enum_value", 2), ("input_range", 4), ("input_step", 3)].iter().map(|(k, l)| (k.to_string(), vec![*l as i128])).collect();
        assert_eq!(got, want, "{}: {out}", q.lang);
        let (_, text, _) = rulec(d, q.lang, &["diff", "old.rule", "new.rule", "--fixtures", "r.jsonl"]);
        let reasons: &[&str] = if q.lang == "ja" {
            &["地域 = 離島 は、新しい版の列挙 地域 の値ではありません", "重量 の値が、新しい版の mass[kg] の刻みに載りません", "重量 が新しい版の範囲 1kg..20kg の外です", "新しい版の制約 `申告額 <= 補償額` が成り立ちません"]
        } else {
            &["zone = remote is not a value of the new version's enum zone", "weight does not sit on the step of the new version's mass[kg]", "weight is outside the new version's range 1kg..20kg", "the new version's constraint `declared <= cover` does not hold"]
        };
        for r in reasons {
            assert!(text.contains(r), "{}: `{r}` が無い:\n{text}", q.lang);
        }

        // A constraint the old version already has: the record that breaks it is not one the old
        // version takes. `fixtures lint` says so, and `diff` and `replay` leave it out.
        let (c, out, _) = rulec(d, q.lang, &["fixtures", "lint", "r.jsonl", "constrained.rule"]);
        assert_eq!(c, 1, "{}: {out}", q.lang);
        let said = if q.lang == "ja" { "`in`: 制約 `申告額 <= 補償額` が成り立ちません" } else { "`in`: the constraint `declared <= cover` does not hold" };
        assert!(out.contains(said), "{}: {out}", q.lang);
        let (_, out, _) = rulec(d, q.lang, &["diff", "constrained.rule", "new.rule", "--fixtures", "r.jsonl", "--format", "json"]);
        let j = json(&out);
        assert_eq!(int(j.get("excluded").unwrap(), "bad_format"), 1, "{}: {out}", q.lang);
        assert!(!arr(&j, "refused").iter().any(|r| s(r, "kind") == "constraint"), "{}: 両方の版が拒む組み合わせを差にしている: {out}", q.lang);
    }
}

/// A machine's records are read the same way. A case is played by the new version call by call,
/// and a call it turns away refuses the case there; the record-level report names the records.
struct Orders {
    lang: &'static str,
    rule: &'static str,
    cap: (&'static str, &'static str),
    log: &'static str,
    field: &'static str,
}

const ORDERS_EN: Orders = Orders {
    lang: "en",
    rule: "order_lifecycle.rule",
    cap: ("range >=0JPY <=1_000_000JPY", "range >=0JPY <=4_000JPY"),
    log: r#"{"tag":"order:1","in":{"state":"received","event":"pay","amount_paid":3000},"observed":{"next_state":"paid","refund":0,"accepted":true}}
{"tag":"order:1","in":{"state":"paid","event":"ship","amount_paid":3000},"observed":{"next_state":"shipped","refund":0,"accepted":true}}
{"tag":"order:2","in":{"state":"received","event":"pay","amount_paid":5000},"observed":{"next_state":"paid","refund":0,"accepted":true}}
{"tag":"order:1","in":{"state":"shipped","event":"cancel","amount_paid":3000},"observed":{"next_state":"shipped","refund":0,"accepted":false}}
{"tag":"order:2","in":{"state":"paid","event":"cancel","amount_paid":5000},"observed":{"next_state":"cancelled","refund":5000,"accepted":true}}
"#,
    field: "amount_paid",
};

const ORDERS_JA: Orders = Orders {
    lang: "ja",
    rule: "注文の状態.rule",
    cap: ("range >=0円 <=100万円", "range >=0円 <=4000円"),
    log: r#"{"tag":"order:1","in":{"状態":"受付","出来事":"入金","支払額":3000},"observed":{"次の状態":"入金済","返金額":0,"受理":true}}
{"tag":"order:1","in":{"状態":"入金済","出来事":"出荷","支払額":3000},"observed":{"次の状態":"出荷済","返金額":0,"受理":true}}
{"tag":"order:2","in":{"状態":"受付","出来事":"入金","支払額":5000},"observed":{"次の状態":"入金済","返金額":0,"受理":true}}
{"tag":"order:1","in":{"状態":"出荷済","出来事":"取消依頼","支払額":3000},"observed":{"次の状態":"出荷済","返金額":0,"受理":false}}
{"tag":"order:2","in":{"状態":"入金済","出来事":"取消依頼","支払額":5000},"observed":{"次の状態":"取消","返金額":5000,"受理":true}}
"#,
    field: "支払額",
};

#[test]
fn a_case_the_new_version_turns_away_is_refused_where_it_does() {
    for o in [&ORDERS_EN, &ORDERS_JA] {
        let t = TempDir::new(&format!("diff-records-orders-{}", o.lang));
        let d = t.path();
        let src = std::fs::read_to_string(root().join("tests/corpus").join(o.rule)).unwrap();
        write(d, "old.rule", &src);
        write(d, "new.rule", &edit(&src, o.cap.0, o.cap.1));
        write(d, "log.jsonl", o.log);
        let (c, out, e) = rulec(d, o.lang, &["check", "new.rule"]);
        assert_eq!(c, 0, "{out}{e}");
        let (c, out, e) = rulec(d, o.lang, &["diff", "old.rule", "new.rule", "--fixtures", "log.jsonl", "--format", "json"]);
        assert_eq!(c, 1, "{}: {out}{e}", o.lang);
        let j = json(&out);
        assert_eq!((int(&j, "compared"), int(&j, "matched")), (5, 3), "{}: {out}", o.lang);
        let refused = arr(&j, "refused");
        assert_eq!(refused.len(), 1, "{}: {out}", o.lang);
        assert_eq!((s(&refused[0], "kind"), s(&refused[0], "field")), ("input_range", o.field), "{}: {out}", o.lang);
        assert_eq!(lines_of(refused), [3, 5], "{}: {out}", o.lang);
        let cs = j.get("cases").unwrap();
        assert_eq!((int(cs, "total"), int(cs, "followed")), (2, 1), "{}: {out}", o.lang);
        let at: Vec<(String, i128)> = arr(cs, "refused").iter().map(|x| (s(x, "tag").to_string(), int(x, "line"))).collect();
        assert_eq!(at, [("order:2".to_string(), 3)], "{}: order:2 は最初の呼び出しで受け付けられない: {out}", o.lang);
        assert!(arr(cs, "diverged").is_empty(), "{}: {out}", o.lang);
    }
}

/// What carries across and what does not: a weight the old version counts in kilograms is the
/// new version's grams, a `null` is no value of an input that is no longer optional, and an
/// enum's value is no value of an input that has become a boolean.
struct Kinds {
    lang: &'static str,
    old: &'static str,
    new: &'static str,
    /// The new version, with the input that became a boolean left an enum.
    keep: (&'static str, &'static str),
    records: &'static str,
    fields: (&'static str, &'static str),
    reasons: [&'static str; 2],
}

const KINDS_EN: Kinds = Kinds {
    lang: "en",
    old: "rule kinds v1

enum zone = domestic | overseas

inputs
  zone    : zone
  weight  : mass[kg]  range >=1kg <=30kg
  coupon  : money[USD]?  contract_only
  insured : zone  contract_only

outputs
  fee : money[USD]  round down(1USD)

table fees
policy unique
| zone     | weight | -> fee : money[USD] |
| domestic | <=5kg  | 5USD                |
| domestic | >5kg   | 9USD                |
| overseas | -      | 20USD               |
",
    new: "rule kinds v2

enum zone = domestic | overseas

inputs
  zone    : zone
  weight  : mass[g]  range >=1000g <=30000g
  coupon  : money[USD]  range >=0USD <=50USD  contract_only
  insured : bool  contract_only

outputs
  fee : money[USD]  round down(1USD)

table fees
policy unique
| zone     | weight  | -> fee : money[USD] |
| domestic | <=5000g | 5USD                |
| domestic | >5000g  | 9USD                |
| overseas | -       | 20USD               |
",
    keep: ("  insured : bool  contract_only", "  insured : zone  contract_only"),
    records: r#"{"in":{"zone":"domestic","weight":3,"coupon":null,"insured":"domestic"}}
{"in":{"zone":"domestic","weight":3,"coupon":5,"insured":"domestic"}}
"#,
    fields: ("coupon", "insured"),
    reasons: ["coupon is none (null), and the new version's coupon is not optional", "insured is zone in the old version and bool in the new one, and a value does not carry across"],
};

const KINDS_JA: Kinds = Kinds {
    lang: "ja",
    old: "rule 種類(kinds) v1

enum 地域(zone) = 国内(domestic) | 海外(overseas)

inputs
  地域(zone)       : 地域
  重量(weight)     : mass[kg]  range >=1kg <=30kg
  クーポン(coupon) : money[円]?  contract_only
  保険(insured)    : 地域  contract_only

outputs
  料金(fee) : money[円]  round down(1円)

table 料金表(fees)
policy unique
| 地域 | 重量  | -> 料金 : money[円] |
| 国内 | <=5kg | 500円               |
| 国内 | >5kg  | 900円               |
| 海外 | -     | 2000円              |
",
    new: "rule 種類(kinds) v2

enum 地域(zone) = 国内(domestic) | 海外(overseas)

inputs
  地域(zone)       : 地域
  重量(weight)     : mass[g]  range >=1000g <=30000g
  クーポン(coupon) : money[円]  range >=0円 <=5000円  contract_only
  保険(insured)    : bool  contract_only

outputs
  料金(fee) : money[円]  round down(1円)

table 料金表(fees)
policy unique
| 地域 | 重量    | -> 料金 : money[円] |
| 国内 | <=5000g | 500円               |
| 国内 | >5000g  | 900円               |
| 海外 | -       | 2000円              |
",
    keep: ("  保険(insured)    : bool  contract_only", "  保険(insured)    : 地域  contract_only"),
    records: r#"{"in":{"地域":"国内","重量":3,"クーポン":null,"保険":"国内"}}
{"in":{"地域":"国内","重量":3,"クーポン":500,"保険":"国内"}}
"#,
    fields: ("クーポン", "保険"),
    reasons: ["クーポン が none（null）ですが、新しい版の クーポン は optional ではありません", "保険 は旧版では 地域、新しい版では bool で、値を移せません"],
};

#[test]
fn what_carries_into_the_new_version_s_types() {
    for k in [&KINDS_EN, &KINDS_JA] {
        let t = TempDir::new(&format!("diff-records-kinds-{}", k.lang));
        let d = t.path();
        write(d, "old.rule", k.old);
        write(d, "new.rule", k.new);
        write(d, "keep.rule", &edit(k.new, k.keep.0, k.keep.1));
        write(d, "r.jsonl", k.records);
        for r in ["old.rule", "new.rule", "keep.rule"] {
            let (c, out, e) = rulec(d, k.lang, &["check", r]);
            assert_eq!(c, 0, "{r}: {out}{e}");
        }
        let (c, out, e) = rulec(d, k.lang, &["diff", "old.rule", "new.rule", "--fixtures", "r.jsonl", "--format", "json"]);
        assert_eq!(c, 1, "{}: {out}{e}", k.lang);
        let j = json(&out);
        assert_eq!((int(&j, "compared"), int(&j, "matched")), (2, 0), "{}: {out}", k.lang);
        let mut got: Vec<(String, String, Vec<i128>)> =
            arr(&j, "refused").iter().map(|r| (s(r, "kind").to_string(), s(r, "field").to_string(), lines_of(std::slice::from_ref(r)))).collect();
        got.sort();
        let mut want = vec![("input_type".to_string(), k.fields.0.to_string(), vec![1]), ("input_type".to_string(), k.fields.1.to_string(), vec![2])];
        want.sort();
        assert_eq!(got, want, "{}: {out}", k.lang);
        let (_, text, _) = rulec(d, k.lang, &["diff", "old.rule", "new.rule", "--fixtures", "r.jsonl"]);
        for r in k.reasons {
            assert!(text.contains(r), "{}: `{r}` が無い:\n{text}", k.lang);
        }
        // With the enum kept, 3 kg is the new version's 3000 g: taken, and answered alike.
        let (c, out, e) = rulec(d, k.lang, &["diff", "old.rule", "keep.rule", "--fixtures", "r.jsonl", "--format", "json"]);
        assert_eq!(c, 1, "{}: {out}{e}", k.lang);
        let j = json(&out);
        assert_eq!((int(&j, "compared"), int(&j, "matched")), (2, 1), "{}: {out}", k.lang);
        assert_eq!(lines_of(arr(&j, "refused")), [1], "{}: {out}", k.lang);
    }
}
