//! Two answers are compared as the values they are, never at either side's step (DESIGN
//! §15.199), by `diff` with records and without, by `replay` and by `verify`.
//!
//! Two defects had one shape: an answer that moved was not seen to move. `diff` with records
//! brought the old version's answer to the new version's step before comparing, so a version
//! that rounds 12.3% to 12% at a coarser step was "No mismatches" while `diff` without records
//! said it differs. `replay` held what a record says came out to what the rule can produce, and
//! left out the record whose answer the rule cannot give — the disagreement it exists to find.
//! What can still be told apart is said: a value outside what the rule can produce is counted
//! beside the headline, with the reading of a record written before a step or a unit changed.
//!
//! Every case is written twice: first in English, then the Japanese version beside it.

use std::path::Path;
use std::process::Command;
use ritsu_testkit::{Need, TempDir, ready};

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

/// A value as the JSON writes it, a number with its decimals or a string.
fn raw(j: &rulec::json::Json, k: &str) -> String {
    match j.get(k) {
        Some(rulec::json::Json::Int(n)) => n.to_string(),
        Some(rulec::json::Json::Frac(s)) | Some(rulec::json::Json::Str(s)) => s.clone(),
        other => panic!("{k}: {other:?}"),
    }
}

fn write(dir: &Path, name: &str, body: &str) {
    std::fs::write(dir.join(name), body).unwrap();
}

/// Whether `diff` without records lists the change of an output's declaration: no answer moves
/// when only its step or its unit does, and what callers receive changes all the same.
fn declares(v: &rulec::json::Json, what: &str, name: &str, old: &str, new: &str) -> bool {
    arr(v, "domain").iter().any(|d| {
        let s = |k: &str| match d.get(k) {
            Some(rulec::json::Json::Str(x)) => x.clone(),
            _ => String::new(),
        };
        s("what") == what && s("name") == name && s("old") == old && s("new") == new
    })
}

/// A rate kept at 0.1% steps, and the version that keeps it at 1%: 12.3% becomes 12%, and 2%
/// stays 2% — written 20 before and 2 after, the same value at another step.
struct Rates {
    lang: &'static str,
    fine: &'static str,
    coarse: &'static str,
    records: &'static str,
    out: &'static str,
    /// The record `diff`'s witness and `diff` without records, as each writes the move.
    said: [&'static str; 2],
}

const RATES_EN: Rates = Rates {
    lang: "en",
    fine: "rule rate_demo v1

enum kind = general | construction

inputs
  kind : kind

outputs
  rate : rate[step 0.1%]  round down(0.1%)

table rates
policy unique
| kind         | -> rate : rate[step 0.1%] |
| general      | 12.3%                     |
| construction | 2%                        |
",
    coarse: "rule rate_demo v2

enum kind = general | construction

inputs
  kind : kind

outputs
  rate : rate[step 1%]  round down(1%)

table rates
policy unique
| kind         | -> rate : rate[step 1%] |
| general      | 12%                     |
| construction | 2%                      |
",
    records: r#"{"in":{"kind":"general"},"observed":{"rate":123}}
{"in":{"kind":"construction"},"observed":{"rate":20}}
"#,
    out: "rate",
    said: ["rule rate=12% / old version rate=12.3%", "rate: 12.3% → 12%"],
};

const RATES_JA: Rates = Rates {
    lang: "ja",
    fine: "rule 料率(rate_demo) v1

enum 区分(kind) = 一般(general) | 建設(construction)

inputs
  区分(kind) : 区分

outputs
  料率(rate) : rate[step 0.1%]  round down(0.1%)

table 料率表(rates)
policy unique
| 区分 | -> 料率 : rate[step 0.1%] |
| 一般 | 12.3%                     |
| 建設 | 2%                        |
",
    coarse: "rule 料率(rate_demo) v2

enum 区分(kind) = 一般(general) | 建設(construction)

inputs
  区分(kind) : 区分

outputs
  料率(rate) : rate[step 1%]  round down(1%)

table 料率表(rates)
policy unique
| 区分 | -> 料率 : rate[step 1%] |
| 一般 | 12%                     |
| 建設 | 2%                      |
",
    records: r#"{"in":{"区分":"一般"},"observed":{"料率":123}}
{"in":{"区分":"建設"},"observed":{"料率":20}}
"#,
    out: "料率",
    said: ["規則 料率=12% / 旧版 料率=12.3%", "料率: 12.3% → 12%"],
};

fn rates(r: &Rates) -> TempDir {
    let t = TempDir::new(&format!("compare-values-{}", r.lang));
    let d = t.path();
    write(d, "fine.rule", r.fine);
    write(d, "coarse.rule", r.coarse);
    write(d, "r.jsonl", r.records);
    for f in ["fine.rule", "coarse.rule"] {
        let (c, out, e) = rulec(d, r.lang, &["check", f]);
        assert_eq!(c, 0, "{f}: {out}{e}");
    }
    t
}

/// A version that keeps a rate at a coarser step moves the answer it rounds and not the one it
/// does not, and the two diffs say so alike: the record whose answer moved is the input where
/// the walk finds a difference. The move is written unrounded, each answer at its own step.
#[test]
fn a_coarser_step_moves_the_answer_it_rounds_in_both_diffs() {
    for r in [&RATES_EN, &RATES_JA] {
        let t = rates(r);
        let d = t.path();
        let (c, out, e) = rulec(d, r.lang, &["diff", "fine.rule", "coarse.rule", "--fixtures", "r.jsonl", "--format", "json"]);
        assert_eq!(c, 1, "{}: 動いた答えがあるのに 0: {out}{e}", r.lang);
        let j = json(&out);
        assert_eq!((int(&j, "compared"), int(&j, "matched")), (2, 1), "{}: {out}", r.lang);
        let cl = &arr(&j, "clusters")[0];
        let delta = cl.get("delta").unwrap().get(r.out).unwrap();
        assert_eq!((raw(delta, "min"), raw(delta, "total")), ("-0.3".to_string(), "-0.3".to_string()), "{}: 差は新しい版の刻みで丸めずに: {out}", r.lang);
        let w = cl.get("witness").unwrap();
        assert_eq!((raw(w.get("ours").unwrap(), r.out), raw(w.get("theirs").unwrap(), r.out)), ("12".to_string(), "123".to_string()), "{}: 二つの答えはそれぞれの版の刻みで: {out}", r.lang);
        assert!(matches!(cl.get("suspect_rounding"), Some(rulec::json::Json::Bool(true))), "{}: 刻みより細かい差は丸め方の違いの疑い: {out}", r.lang);

        let (c, region, _) = rulec(d, r.lang, &["diff", "fine.rule", "coarse.rule", "--format", "json"]);
        assert_eq!(c, 1, "{}: {region}", r.lang);
        let v = json(&region);
        assert_eq!((int(&v, "same"), int(&v, "differing")), (1, 1), "{}: 2% は刻みが変わっても同じ値: {region}", r.lang);
        assert!(declares(&v, "output_step", r.out, "0.1%", "1%"), "{}: 出力の刻みが変わったことを言わない: {region}", r.lang);
        let o = &arr(&arr(&v, "changes")[0], "outputs")[0];
        assert_eq!((raw(o, "old"), raw(o, "new")), ("123".to_string(), "12".to_string()), "{}: {region}", r.lang);
        // The input where the walk finds the difference is the record that moved.
        let witness = &arr(&arr(&v, "changes")[0], "witness")[0];
        let moved = arr(cl, "records")[0].get("line").unwrap().as_int().unwrap();
        let first = r.records.lines().nth(moved as usize - 1).unwrap();
        assert!(first.contains(&format!("\"{}\"", raw(witness, "value"))), "{}: 二つの diff が別の入力を言う: {first} / {region}", r.lang);

        let (_, text, _) = rulec(d, r.lang, &["diff", "fine.rule", "coarse.rule", "--fixtures", "r.jsonl"]);
        assert!(text.contains(r.said[0]), "{}: {text}", r.lang);
        let (_, text, _) = rulec(d, r.lang, &["diff", "fine.rule", "coarse.rule"]);
        assert!(text.contains(r.said[1]), "{}: {text}", r.lang);
    }
}

/// An amount the new version counts in another unit is the same amount: yen and sen, dollars
/// and cents. Only the one that changed moves, by the cents it changed, in either diff, and each
/// answer is written with its own version's unit beside it.
struct Units {
    lang: &'static str,
    whole: &'static str,
    cents: &'static str,
    records: &'static str,
    out: &'static str,
    /// The record `diff`'s witness and `diff` without records.
    said: [&'static str; 2],
}

const UNITS_EN: Units = Units {
    lang: "en",
    whole: "rule fee_demo v1

enum zone = domestic | overseas

inputs
  zone : zone

outputs
  fee : money[USD]  round down(1USD)

table fees
policy unique
| zone     | -> fee : money[USD] |
| domestic | 5USD                |
| overseas | 20USD               |
",
    cents: "rule fee_demo v2

enum zone = domestic | overseas

inputs
  zone : zone

outputs
  fee : money[USDc]  round down(1USDc)

table fees
policy unique
| zone     | -> fee : money[USDc] |
| domestic | 500USDc              |
| overseas | 2050USDc             |
",
    records: r#"{"in":{"zone":"domestic"}}
{"in":{"zone":"overseas"}}
"#,
    out: "fee",
    said: ["rule fee=2050USDc / old version fee=20USD", "fee: 20USD → 2050USDc"],
};

const UNITS_JA: Units = Units {
    lang: "ja",
    whole: "rule 手数料(fee_demo) v1

enum 地域(zone) = 国内(domestic) | 海外(overseas)

inputs
  地域(zone) : 地域

outputs
  手数料(fee) : money[円]  round down(1円)

table 手数料表(fees)
policy unique
| 地域 | -> 手数料 : money[円] |
| 国内 | 500円                 |
| 海外 | 2000円                |
",
    cents: "rule 手数料(fee_demo) v2

enum 地域(zone) = 国内(domestic) | 海外(overseas)

inputs
  地域(zone) : 地域

outputs
  手数料(fee) : money[銭]  round down(1銭)

table 手数料表(fees)
policy unique
| 地域 | -> 手数料 : money[銭] |
| 国内 | 50000銭               |
| 海外 | 200050銭              |
",
    records: r#"{"in":{"地域":"国内"}}
{"in":{"地域":"海外"}}
"#,
    out: "手数料",
    said: ["規則 手数料=200050銭 / 旧版 手数料=2000円", "手数料: 2000円 → 200050銭"],
};

#[test]
fn an_amount_in_another_unit_is_the_same_amount() {
    for u in [&UNITS_EN, &UNITS_JA] {
        let t = TempDir::new(&format!("compare-units-{}", u.lang));
        let d = t.path();
        write(d, "whole.rule", u.whole);
        write(d, "cents.rule", u.cents);
        write(d, "r.jsonl", u.records);
        for f in ["whole.rule", "cents.rule"] {
            let (c, out, e) = rulec(d, u.lang, &["check", f]);
            assert_eq!(c, 0, "{f}: {out}{e}");
        }
        let (c, out, e) = rulec(d, u.lang, &["diff", "whole.rule", "cents.rule", "--fixtures", "r.jsonl", "--format", "json"]);
        assert_eq!(c, 1, "{}: {out}{e}", u.lang);
        let j = json(&out);
        assert_eq!((int(&j, "compared"), int(&j, "matched")), (2, 1), "{}: 単位を替えただけの答えは同じ: {out}", u.lang);
        let cl = &arr(&j, "clusters")[0];
        assert_eq!(raw(cl.get("delta").unwrap().get(u.out).unwrap(), "total"), "50", "{}: 差は新しい版の単位で: {out}", u.lang);
        let (_, region, _) = rulec(d, u.lang, &["diff", "whole.rule", "cents.rule", "--format", "json"]);
        let v = json(&region);
        assert_eq!((int(&v, "same"), int(&v, "differing")), (1, 1), "{}: {region}", u.lang);
        let (old, new) = if u.lang == "ja" { ("money[円]", "money[銭]") } else { ("money[USD]", "money[USDc]") };
        assert!(declares(&v, "output_type", u.out, old, new), "{}: 出力の型が変わったことを言わない: {region}", u.lang);
        let (_, text, _) = rulec(d, u.lang, &["diff", "whole.rule", "cents.rule", "--fixtures", "r.jsonl"]);
        assert!(text.contains(u.said[0]), "{}: 二つの答えにそれぞれの単位が無い: {text}", u.lang);
        let (_, text, _) = rulec(d, u.lang, &["diff", "whole.rule", "cents.rule"]);
        assert!(text.contains(u.said[1]), "{}: {text}", u.lang);
    }
}

/// A difference no decimal writes is written as the fraction it is, never rounded: a limit the
/// old version kept in ℉ and the new one in ℃, where 42℉ is 50/9 ℃ and the new answer is 6℃.
/// The two that only spell one temperature (5℉ and -15℃) are the same.
struct Temps {
    lang: &'static str,
    old: &'static str,
    out: &'static str,
    said: [&'static str; 2],
}

const TEMPS_EN: Temps = Temps {
    lang: "en",
    old: "rule cold_chain v1

enum goods = chilled | frozen

inputs
  goods : goods

outputs
  limit : temperature[℉]  round down(1℉)

table limits
policy unique
| goods   | -> limit : temperature[℉] |
| chilled | 42℉                       |
| frozen  | 5℉                        |
",
    out: "limit",
    said: ["rule limit=6℃ / old version limit=42℉", "limit: 42℉ → 6℃"],
};

const TEMPS_JA: Temps = Temps {
    lang: "ja",
    old: "rule 保冷(cold_chain) v1

enum 品目(goods) = 冷蔵(chilled) | 冷凍(frozen)

inputs
  品目(goods) : 品目

outputs
  上限温度(limit) : temperature[℉]  round down(1℉)

table 上限表(limits)
policy unique
| 品目 | -> 上限温度 : temperature[℉] |
| 冷蔵 | 42℉                         |
| 冷凍 | 5℉                          |
",
    out: "上限温度",
    said: ["規則 上限温度=6℃ / 旧版 上限温度=42℉", "上限温度: 42℉ → 6℃"],
};

#[test]
fn a_difference_no_decimal_writes_is_a_fraction() {
    for t in [&TEMPS_EN, &TEMPS_JA] {
        let tmp = TempDir::new(&format!("compare-temps-{}", t.lang));
        let d = tmp.path();
        let new = t.old.replace("℉", "℃").replace("42℃", "6℃").replace("| 5℃ ", "| -15℃").replace(" v1\n", " v2\n");
        write(d, "f.rule", t.old);
        write(d, "c.rule", &new);
        for f in ["f.rule", "c.rule"] {
            let (c, out, e) = rulec(d, t.lang, &["check", f]);
            assert_eq!(c, 0, "{f}: {out}{e}\n{new}");
        }
        let goods = if t.lang == "ja" { ("品目", ["冷蔵", "冷凍"]) } else { ("goods", ["chilled", "frozen"]) };
        let records: String = goods.1.iter().map(|g| format!("{{\"in\":{{\"{}\":\"{g}\"}}}}\n", goods.0)).collect();
        write(d, "r.jsonl", &records);
        let (c, out, e) = rulec(d, t.lang, &["diff", "f.rule", "c.rule", "--fixtures", "r.jsonl", "--format", "json"]);
        assert_eq!(c, 1, "{}: {out}{e}", t.lang);
        let j = json(&out);
        assert_eq!((int(&j, "compared"), int(&j, "matched")), (2, 1), "{}: 5℉ と -15℃ は同じ温度: {out}", t.lang);
        let cl = &arr(&j, "clusters")[0];
        assert_eq!(raw(cl.get("delta").unwrap().get(t.out).unwrap(), "total"), "4/9", "{}: {out}", t.lang);
        let w = cl.get("witness").unwrap();
        assert_eq!((raw(w.get("ours").unwrap(), t.out), raw(w.get("theirs").unwrap(), t.out)), ("6".to_string(), "42".to_string()), "{}: {out}", t.lang);
        let (_, text, _) = rulec(d, t.lang, &["diff", "f.rule", "c.rule", "--fixtures", "r.jsonl"]);
        assert!(text.contains(t.said[0]) && text.contains("+4/9"), "{}: {text}", t.lang);
        let (c, region, _) = rulec(d, t.lang, &["diff", "f.rule", "c.rule"]);
        assert_eq!(c, 1, "{}: {region}", t.lang);
        assert!(region.contains(t.said[1]), "{}: {region}", t.lang);
        let (_, region, _) = rulec(d, t.lang, &["diff", "f.rule", "c.rule", "--format", "json"]);
        let v = json(&region);
        assert_eq!((int(&v, "same"), int(&v, "differing")), (1, 1), "{}: {region}", t.lang);
        assert!(declares(&v, "output_type", t.out, "temperature[℉]", "temperature[℃]"), "{}: {region}", t.lang);
    }
}

/// What a record says came out is read as the value it is and compared: a value the rule cannot
/// produce, and one finer than the output's step, are mismatches, not problems of the record.
/// `fixtures lint` and `replay` leave out the same records — only those whose form is wrong.
struct Allowance {
    lang: &'static str,
    rule: &'static str,
    callee: &'static str,
    records: &'static str,
    out: &'static str,
    /// The line `replay` writes of the value outside, and the note of `fixtures lint`.
    said: [&'static str; 2],
}

const ALLOWANCE_EN: Allowance = Allowance {
    lang: "en",
    rule: "part_time_retirement_pay.rule",
    callee: "retirement_pay.rule",
    records: r#"{"in":{"tenure":1,"end_reason":"term_end","monthly_pay":100000},"observed":{"part_time_allowance":499999}}
{"in":{"tenure":1,"end_reason":"term_end","monthly_pay":100000},"observed":{"part_time_allowance":500000.5}}
{"in":{"tenure":1,"end_reason":"term_end","monthly_pay":100000},"observed":{"part_time_allowance":600000}}
{"in":{"tenure":1,"end_reason":"retired","monthly_pay":100000},"observed":{"part_time_allowance":500000}}
"#,
    out: "part_time_allowance",
    said: [
        "Records whose observed value is outside what the rule can produce: 1 (part_time_allowance: 1); counted as mismatches",
        "`observed.part_time_allowance`: outside what the rule can produce, 500_000JPY..20_000_000JPY",
    ],
};

const ALLOWANCE_JA: Allowance = Allowance {
    lang: "ja",
    rule: "非常勤退職手当.rule",
    callee: "退職手当.rule",
    records: r#"{"in":{"在職期間":1,"任期終了事由":"任期満了","報酬月額":100000},"observed":{"非常勤手当":499999}}
{"in":{"在職期間":1,"任期終了事由":"任期満了","報酬月額":100000},"observed":{"非常勤手当":500000.5}}
{"in":{"在職期間":1,"任期終了事由":"任期満了","報酬月額":100000},"observed":{"非常勤手当":600000}}
{"in":{"在職期間":1,"任期終了事由":"定年","報酬月額":100000},"observed":{"非常勤手当":500000}}
"#,
    out: "非常勤手当",
    said: [
        "観測の値が規則の取りうる範囲の外にある記録 1 件（非常勤手当 1 件）。不一致に数えています",
        "`observed.非常勤手当`: 規則が取りうる範囲 50万円..2000万円 の外です",
    ],
};

#[test]
fn replay_compares_what_the_rule_cannot_produce() {
    for a in [&ALLOWANCE_EN, &ALLOWANCE_JA] {
        let t = TempDir::new(&format!("compare-replay-{}", a.lang));
        let d = t.path();
        let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
        for f in [a.rule, a.callee] {
            std::fs::copy(corpus.join(f), d.join(f)).unwrap();
        }
        write(d, "r.jsonl", a.records);
        let (c, out, e) = rulec(d, a.lang, &["replay", a.rule, "--fixtures", "r.jsonl", "--format", "json"]);
        assert_eq!(c, 1, "{}: {out}{e}", a.lang);
        let j = json(&out);
        // The three answers differ from the rule's 500,000: below what it can produce, finer than
        // its yen, and plainly more. The fourth record is not one of the rule's: its end reason
        // is no value of the enum, and that is a problem of its form.
        assert_eq!((int(&j, "compared"), int(&j, "matched")), (3, 0), "{}: {out}", a.lang);
        assert_eq!(int(j.get("excluded").unwrap(), "bad_format"), 1, "{}: {out}", a.lang);
        assert_eq!(int(j.get("out_of_reach").unwrap(), a.out), 1, "{}: {out}", a.lang);
        // The three fire the same rows, so they are one cluster: 1, -0.5 and -100,000 yen, the
        // half yen kept rather than cut.
        let cl = arr(&j, "clusters");
        assert_eq!(cl.len(), 1, "{}: {out}", a.lang);
        let delta = cl[0].get("delta").unwrap().get(a.out).unwrap();
        assert_eq!((raw(delta, "min"), raw(delta, "max"), raw(delta, "total")), ("-100000".to_string(), "1".to_string(), "-99999.5".to_string()), "{}: {out}", a.lang);
        let (_, text, _) = rulec(d, a.lang, &["replay", a.rule, "--fixtures", "r.jsonl"]);
        assert!(text.contains(a.said[0]) && text.contains("--read-as"), "{}: {text}", a.lang);

        // `fixtures lint` reports the one record replay leaves out, and only that one; the value
        // outside what the rule can produce is a note.
        let (c, lint, _) = rulec(d, a.lang, &["fixtures", "lint", "r.jsonl", a.rule, "--format", "json"]);
        assert_eq!(c, 1, "{}: {lint}", a.lang);
        let l = json(&lint);
        let problems: i128 = arr(&l, "problems").iter().map(|p| int(p, "count")).sum();
        assert_eq!(problems, int(j.get("excluded").unwrap(), "bad_format"), "{}: lint と replay が外す記録が違う: {lint}", a.lang);
        assert_eq!(int(&arr(&l, "out_of_reach")[0], "count"), 1, "{}: {lint}", a.lang);
        let (_, text, _) = rulec(d, a.lang, &["fixtures", "lint", "r.jsonl", a.rule]);
        assert!(text.contains(a.said[1]), "{}: {text}", a.lang);
    }
}

/// An answer finer than the rule's step is the implementation computing past it: a mismatch
/// below the output's grid, flagged as a suspected rounding difference, with the difference
/// written unrounded. Records written at another step look otherwise — whole numbers of the
/// rule's step, outside what it can produce — and the run says how to read them.
#[test]
fn an_answer_finer_than_the_step_and_a_record_at_another_step() {
    for r in [&RATES_EN, &RATES_JA] {
        let t = rates(r);
        let d = t.path();
        let key = if r.lang == "ja" { ("区分", "一般") } else { ("kind", "general") };
        write(d, "finer.jsonl", &format!("{{\"in\":{{\"{}\":\"{}\"}},\"observed\":{{\"{}\":123.4}}}}\n", key.0, key.1, r.out));
        let (c, out, e) = rulec(d, r.lang, &["replay", "fine.rule", "--fixtures", "finer.jsonl", "--format", "json"]);
        assert_eq!(c, 1, "{}: {out}{e}", r.lang);
        let j = json(&out);
        assert_eq!((int(&j, "compared"), int(&j, "matched")), (1, 0), "{}: 小数の観測値も照合する: {out}", r.lang);
        let cl = &arr(&j, "clusters")[0];
        assert_eq!(raw(cl.get("delta").unwrap().get(r.out).unwrap(), "total"), "-0.4", "{}: {out}", r.lang);
        assert_eq!(raw(cl.get("witness").unwrap().get("theirs").unwrap(), r.out), "123.4", "{}: {out}", r.lang);
        assert!(matches!(cl.get("suspect_rounding"), Some(rulec::json::Json::Bool(true))), "{}: {out}", r.lang);
        let (c, lint, _) = rulec(d, r.lang, &["fixtures", "lint", "finer.jsonl", "fine.rule", "--format", "json"]);
        assert_eq!(c, 0, "{}: 小数の観測値は形式の問題ではない: {lint}", r.lang);
        let (_, text, _) = rulec(d, r.lang, &["replay", "fine.rule", "--fixtures", "finer.jsonl"]);
        assert!(text.contains("12.34%"), "{}: 観測値は丸めずに出る: {text}", r.lang);

        // The records of the coarse version, read by the fine one: 12 is 1.2% there, and 2 is
        // 0.2% — outside what it can produce. Compared, counted, and the reading named. Read as
        // the version that wrote them, 2% agrees, and 12% is what the coarse version answered
        // where the fine one answers 12.3%: one mismatch, and nothing outside.
        write(d, "coarse.jsonl", &r.records.replace(":123}", ":12}").replace(":20}", ":2}"));
        let (c, out, _) = rulec(d, r.lang, &["replay", "fine.rule", "--fixtures", "coarse.jsonl", "--format", "json"]);
        assert_eq!(c, 1, "{}: {out}", r.lang);
        let j = json(&out);
        assert_eq!((int(&j, "compared"), int(&j, "matched"), int(j.get("out_of_reach").unwrap(), r.out)), (2, 0, 2), "{}: {out}", r.lang);
        let (_, text, _) = rulec(d, r.lang, &["replay", "fine.rule", "--fixtures", "coarse.jsonl"]);
        assert!(text.contains("--read-as"), "{}: 読み方を案内していない: {text}", r.lang);
        let (c, out, e) = rulec(d, r.lang, &["replay", "fine.rule", "--fixtures", "coarse.jsonl", "--read-as", "coarse.rule", "--format", "json"]);
        assert_eq!(c, 1, "{}: {out}{e}", r.lang);
        let j = json(&out);
        assert_eq!((int(&j, "compared"), int(&j, "matched")), (2, 1), "{}: {out}", r.lang);
        assert!(matches!(j.get("out_of_reach"), Some(rulec::json::Json::Obj(m)) if m.is_empty()), "{}: {out}", r.lang);
    }
}

/// `verify` reads an implementation's answers the same way: a decimal finer than the step and a
/// value the rule cannot produce are compared exactly and counted as mismatches.
#[test]
fn verify_reads_the_answers_as_values() {
    if !ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 が無いので飛ばした") {
        return;
    }
    for r in [&RATES_EN, &RATES_JA] {
        let t = rates(r);
        let d = t.path();
        let (key, general) = if r.lang == "ja" { ("区分", "一般") } else { ("kind", "general") };
        let adapter = format!(
            "import json, sys\nsys.stdin.readline()\nprint(json.dumps({{\"ok\": True, \"impl\": \"legacy@test\"}}), flush=True)\nfor line in sys.stdin:\n    line = line.strip()\n    if not line:\n        continue\n    req = json.loads(line)\n    out = 123.4 if req[\"in\"][\"{key}\"] == \"{general}\" else 999\n    print(json.dumps({{\"id\": req[\"id\"], \"out\": {{\"{}\": out}}}}, ensure_ascii=False), flush=True)\n",
            r.out
        );
        write(d, "adapter.py", &adapter);
        let (c, out, e) = rulec(d, r.lang, &["verify", "fine.rule", "--format", "json", "--adapter", "python3", "adapter.py"]);
        assert_eq!(c, 1, "{}: {out}{e}", r.lang);
        let j = json(&out);
        assert_eq!(int(&j, "matched"), 0, "{}: {out}", r.lang);
        let total: Vec<String> = arr(&j, "clusters").iter().map(|cl| raw(cl.get("delta").unwrap().get(r.out).unwrap(), "total")).collect();
        assert!(total.iter().any(|t| t.ends_with(".4")), "{}: 小数の答えの差が丸めずに出る: {total:?}\n{out}", r.lang);
        assert!(int(j.get("out_of_reach").unwrap(), r.out) >= 1, "{}: {out}", r.lang);
        let (_, text, _) = rulec(d, r.lang, &["verify", "fine.rule", "--adapter", "python3", "adapter.py"]);
        let said = if r.lang == "ja" { "現行の答えが規則の取りうる範囲の外だったもの" } else { "Answers of the legacy implementation outside what the rule can produce" };
        assert!(text.contains(said) && !text.contains("--read-as"), "{}: {text}", r.lang);
    }
}
