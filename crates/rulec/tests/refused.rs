//! The inputs the door refuses, written by `gen` and held by `rulec test` in every language
//! (§15.204).
//!
//! `gen` wrote only the walks the reference evaluator has no answer for into the file of refused
//! inputs, so `rulec test` never asked a language to refuse anything else: a door that let a
//! constraint, a range, a truth value or a date through stayed green. The file now has an input
//! for every reason a door refuses one, each with the sentence the reference evaluator says of
//! it, and `rulec test` holds every way the rule is reached to refusing each of them for that
//! reason, with that sentence.
//!
//! The materials are rules of the tree that between them have every reason, English first, with
//! the Japanese version of each beside it: two amounts and a truth value under a constraint
//! (`express_delivery_quote`, `速達の見積`), two dates under a constraint (`hotel_stay`, `宿泊料金`),
//! a walk with an enum in its elements and a contradiction (`nationwide_freight`, `全国運賃`), a
//! count over a sequence (`supplier_matching`, `納入先照合`), a sum over one
//! (`cart_shipping_fee`, `買物かごの送料`), a string (`sku_prefix_handling`, `品番の扱い`),
//! optional inputs of every kind (`parcel_cover`, `小包の補償`), a value the rule works out
//! past the range it accepts for it, once for a call (`card_fee`, `カード手数料`) and on each
//! element (`order_line_subtotals`, `明細の小計`, §15.313), ends written with `>` and `<` on
//! every kind of input (`season_ticket`, `シーズン券`), a sum and a count under the bottom of
//! their ranges (`gift_basket`, `贈り物のかご`), and the same ends on a walk (`parcel_batch`,
//! `小包のまとめ出荷`, §15.314); and the days of a koyomi date (`tests/days/settlement.rule`). Among
//! the inputs are ones with two things wrong (§15.328), which every way refuses for the one the
//! order puts first: the rules without a sequence carry the pairs of inputs to NumPy and SQL, and
//! `tests/refused_order.rs` has every pair of places.

use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, need, side_by_side, skip};
use rulec::json::Json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use ritsu_testkit::run::finish;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const ENGLISH: [&str; 12] = [
    "tests/corpus/express_delivery_quote.rule",
    "tests/date_constraint/hotel_stay.rule",
    "tests/corpus/nationwide_freight.rule",
    "tests/corpus/supplier_matching.rule",
    "tests/corpus/cart_shipping_fee.rule",
    "tests/corpus/sku_prefix_handling.rule",
    "tests/optional/parcel_cover.rule",
    "tests/corpus/card_fee.rule",
    "tests/corpus/order_line_subtotals.rule",
    "tests/corpus/season_ticket.rule",
    "tests/corpus/gift_basket.rule",
    "tests/corpus/parcel_batch.rule",
];

const JAPANESE: [&str; 12] = [
    "tests/corpus/速達の見積.rule",
    "tests/date_constraint/宿泊料金.rule",
    "tests/corpus/全国運賃.rule",
    "tests/corpus/納入先照合.rule",
    "tests/corpus/買物かごの送料.rule",
    "tests/corpus/品番の扱い.rule",
    "tests/optional/小包の補償.rule",
    "tests/corpus/カード手数料.rule",
    "tests/corpus/明細の小計.rule",
    "tests/corpus/シーズン券.rule",
    "tests/corpus/贈り物のかご.rule",
    "tests/corpus/小包のまとめ出荷.rule",
];

/// For each rule, a sentence its file has to hold for each reason the rule's door has: by alias,
/// in the language the files are generated in.
fn expected(japanese: bool) -> Vec<(&'static str, Vec<&'static str>)> {
    if japanese {
        vec![
            ("express_quote", vec!["重さ がありません", "重さ が整数ではありません", "重さ が範囲の外です", "速達 が真偽ではありません", "制約が成り立ちません: 申告額 <= 補償額"]),
            ("hotel_stay_ja", vec!["チェックイン日 が日付ではありません", "チェックイン日 が範囲の外です", "制約が成り立ちません: チェックイン日 <= チェックアウト日"]),
            ("freight", vec!["畳み込み 採用: take_unique に二件当たりました（運賃行[0] と 運賃行[1]）", "運賃行 が並びではありません", "運賃行[1] がオブジェクトではありません", "運賃行[1].行ゾーン が列挙 ゾーン区分 の値ではありません", "運賃行[1].閾値 が範囲の外です"]),
            ("supplier_match", vec!["候補 の要素が多すぎます（上限 50）"]),
            ("cart_shipping", vec!["合計 が範囲の外です", "明細[1].金額 が整数ではありません"]),
            ("sku_handling", vec!["品番 が文字列ではありません"]),
            ("parcel_cover_ja", vec!["申告額 が範囲の外です", "発送日 が日付ではありません", "署名 が真偽ではありません"]),
            ("card_fee_ja", vec!["金額 が範囲の外です", "手数料 が範囲の外です"]),
            ("line_subtotals", vec!["明細[1].単価 が範囲の外です", "明細[1].小計 が範囲の外です"]),
            ("season_ticket_ja", vec!["価格 が範囲の外です", "割引率 が範囲の外です", "日付 が範囲の外です", "年齢 が範囲の外です", "席 が範囲の外です"]),
            ("gift_basket_ja", vec!["包装した品 が範囲の外です", "金額 が範囲の外です", "明細 の要素が多すぎます（上限 5）"]),
            ("parcel_batch_ja", vec!["小包[1].重さ が範囲の外です", "総重量 が範囲の外です", "割れ物の数 が範囲の外です", "小包 の要素が多すぎます（上限 3）"]),
        ]
    } else {
        vec![
            ("express_delivery_quote", vec!["weight is missing", "weight is not an integer", "weight is out of range", "express is not a boolean", "the constraint does not hold: declared <= cover"]),
            ("hotel_stay", vec!["check_in is not a date", "check_in is out of range", "the constraint does not hold: check_in <= check_out"]),
            ("nationwide_freight", vec!["fold verdict: two elements matched a take_unique (freight_rows[0] and freight_rows[1])", "freight_rows is not a sequence", "freight_rows[1] is not an object", "freight_rows[1].row_zone is not a value of enum zone", "freight_rows[1].threshold is out of range"]),
            ("supplier_matching", vec!["candidates has too many elements (at most 50)"]),
            ("cart_shipping_fee", vec!["total is out of range", "lines[1].amount is not an integer"]),
            ("sku_prefix_handling", vec!["sku is not a string"]),
            ("parcel_cover", vec!["declared is out of range", "ship_on is not a date", "signature is not a boolean"]),
            ("card_fee", vec!["amount is out of range", "fee is out of range"]),
            ("order_line_subtotals", vec!["lines[1].price is out of range", "lines[1].subtotal is out of range"]),
            ("season_ticket", vec!["price is out of range", "discount is out of range", "day is out of range", "age is out of range", "seat is out of range"]),
            ("gift_basket", vec!["wrapped_items is out of range", "amount is out of range", "lines has too many elements (at most 5)"]),
            ("parcel_batch", vec!["parcels[1].weight is out of range", "total is out of range", "fragile_parcels is out of range", "parcels has too many elements (at most 3)"]),
        ]
    }
}

/// For each rule, the inputs with two things wrong its file has to hold (§15.328), by why each is
/// there, with the sentence of the one the order puts first: two inputs, an input and a field of an
/// element, two fields of elements, and the fold that stops before an element outside its range.
/// The rules without a sequence carry the first two to NumPy and SQL.
fn two_faults(japanese: bool) -> Vec<(&'static str, &'static str, &'static str)> {
    if japanese {
        vec![
            ("express_quote", "重さ を範囲の外に、あとの 速達 を別の種類の値に。種類のほうを言う", "速達 が真偽ではありません"),
            ("express_quote", "重さ を int64 で持てない数に、あとの 速達 を別の種類の値に。先に読む 重さ を言う", "重さ が範囲の外です"),
            ("hotel_stay_ja", "チェックイン日 を範囲の外に、あとの チェックアウト日 を別の種類の値に。種類のほうを言う", "チェックアウト日 が日付ではありません"),
            ("cart_shipping", "区分 を別の種類の値に、明細 の要素の 金額 も別の種類の値に。先の引数の 区分 を言う", "区分 が列挙 会員区分 の値ではありません"),
            ("freight", "運賃行 の一つ目の要素の 閾値 を範囲の外に、二つ目の 行ゾーン を別の種類の値に。種類のほうを言う", "運賃行[1].行ゾーン が列挙 ゾーン区分 の値ではありません"),
            ("freight", "運賃行 の要素で fold が止まり、その次の要素の 閾値 を範囲の外に。止まったあとの要素も入口で確かめる", "運賃行[1].閾値 が範囲の外です"),
            ("freight", "運賃行 の二つの要素が take_unique で当たり、三つ目の 閾値 を範囲の外に。歩きが先に会う二件のほうを言う", "畳み込み 採用: take_unique に二件当たりました（運賃行[0] と 運賃行[1]）"),
        ]
    } else {
        vec![
            ("express_delivery_quote", "weight outside its range and express after it of another kind: the kind is said", "express is not a boolean"),
            ("express_delivery_quote", "weight past what int64 holds and express after it of another kind: weight, read first, is said", "weight is out of range"),
            ("hotel_stay", "check_in outside its range and check_out after it of another kind: the kind is said", "check_out is not a date"),
            ("cart_shipping_fee", "tier of another kind and amount of an element of lines of another kind too: tier, the argument before, is said", "tier is not a value of enum member_tier"),
            ("nationwide_freight", "threshold of the first element of freight_rows outside its range and row_zone of the second of another kind: the kind is said", "freight_rows[1].row_zone is not a value of enum zone"),
            (
                "nationwide_freight",
                "the fold stops on an element of freight_rows, and threshold of the element after it outside its range: the door looks at the elements past the stop",
                "freight_rows[1].threshold is out of range",
            ),
            (
                "nationwide_freight",
                "two elements of freight_rows taking under a take_unique, and threshold of the third outside its range: the two, which the walk meets first, are said",
                "fold verdict: two elements matched a take_unique (freight_rows[0] and freight_rows[1])",
            ),
        ]
    }
}

/// How long a run of rulec may take: `rulec test` talks to the servers of the generated code,
/// and waits for their answers without a limit of its own. Past it, rulec is stopped with every
/// process it started, and the test fails saying what it ran and what it printed until then.
const RULEC_LIMIT: std::time::Duration = std::time::Duration::from_secs(20 * 60);

fn rulec(lang: &str, tmp: Option<&Path>, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rulec"));
    c.env("RULEC_LANG", lang).current_dir(root()).args(args).args(["--lang", lang]);
    if let Some(t) = tmp {
        c.env("TMPDIR", t);
    }
    let o = finish(&mut c, RULEC_LIMIT, &format!("rulec {}", args.join(" ")));
    (o.code.unwrap_or(-1), o.stdout, o.stderr)
}

/// `rulec gen` of a set of rules into one directory, in a language.
fn generate(rules: &[&str], lang: &str) -> (TempDir, PathBuf) {
    let t = TempDir::new("refused");
    let out = t.path().join("out");
    let mut args = vec!["gen"];
    args.extend(rules.iter().copied());
    let o = out.to_string_lossy().into_owned();
    args.extend(["--out", o.as_str()]);
    args.extend(rulec::targets::UP_TO_0_40);
    let (code, said, e) = rulec(lang, None, &args);
    assert_eq!(code, 0, "{said}{e}");
    (t, out)
}

/// Why each line of the refused file of `alias` under `out` is there, with its sentence.
fn whys(out: &Path, alias: &str) -> Vec<(String, String)> {
    let body = std::fs::read_to_string(out.join("vectors").join(format!("{alias}.refused.jsonl"))).unwrap_or_else(|e| panic!("{alias}: {e}"));
    body.lines()
        .map(|l| {
            let j = rulec::json::parse(l).unwrap_or_else(|x| panic!("{alias}: {x}: {l}"));
            let s = |k: &str| j.get(k).and_then(|v| v.as_str()).unwrap_or_else(|| panic!("{alias}: no {k}: {l}")).to_string();
            (s("why"), s("error"))
        })
        .collect()
}

/// The refused files under `out`, by alias: each line as (refused, error).
fn refused(out: &Path) -> BTreeMap<String, Vec<(String, String)>> {
    let mut all = BTreeMap::new();
    for e in std::fs::read_dir(out.join("vectors")).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(alias) = name.strip_suffix(".refused.jsonl") else { continue };
        let body = std::fs::read_to_string(e.path()).unwrap();
        let lines = body
            .lines()
            .map(|l| {
                let j = rulec::json::parse(l).unwrap_or_else(|x| panic!("{alias}: {x}: {l}"));
                let s = |k: &str| j.get(k).and_then(|v| v.as_str()).unwrap_or_else(|| panic!("{alias}: no {k}: {l}")).to_string();
                assert!(j.get("in").is_some() && j.get("why").is_some(), "{alias}: {l}");
                (s("refused"), s("error"))
            })
            .collect();
        all.insert(alias.to_string(), lines);
    }
    all
}

/// `rulec test` over a generated directory, as JSON, with a SKIP line for whatever it did not run.
fn rulec_test(out: &Path) -> (i32, Json) {
    let (code, said, e) = rulec("en", Some(&tmpdir_in(out)), &["test", &out.to_string_lossy(), "--format", "json"]);
    let j = rulec::json::parse(said.lines().next().unwrap_or_else(|| panic!("no result\n{e}"))).unwrap_or_else(|x| panic!("{x}: {said}"));
    for why in j.get("skipped").and_then(|v| v.as_arr()).unwrap_or_default() {
        skip(&format!("rulec test: {}", why.as_str().unwrap_or_default()));
    }
    (code, j)
}

/// `gen` writes an input for every reason the door of each rule has, with the sentence the
/// reference evaluator says of it, in the language the files are generated in; every line is
/// refused by the reference evaluator, and the walk's contradiction keeps its place at the head.
#[test]
fn gen_writes_an_input_for_every_reason_the_door_has() {
    for (rules, lang) in [(&ENGLISH, "en"), (&JAPANESE, "ja")] {
        let (_t, out) = generate(rules, lang);
        let got = refused(&out);
        for (alias, want) in expected(lang == "ja") {
            let lines = got.get(alias).unwrap_or_else(|| panic!("{alias}: no file of refused inputs: {:?}", got.keys()));
            let errors: Vec<&str> = lines.iter().map(|(_, e)| e.as_str()).collect();
            for w in want {
                assert!(errors.contains(&w), "{alias} [{lang}]: no input refused with `{w}`:\n{errors:#?}");
            }
            for (kind, e) in lines {
                assert!(kind == "input" || kind == "contradiction", "{alias}: {kind}");
                assert_eq!(kind == "contradiction", e.contains("take_unique"), "{alias}: {kind} {e}");
            }
        }
        for (alias, why, said) in two_faults(lang == "ja") {
            let lines = whys(&out, alias);
            assert!(lines.iter().any(|(w, e)| w == why && e == said), "{alias} [{lang}]: no input `{why}` refused with `{said}`:\n{lines:#?}");
        }
        let freight = if lang == "ja" { "freight" } else { "nationwide_freight" };
        assert_eq!(got[freight][0].0, "contradiction", "{freight}: {:?}", got[freight]);
        println!("refused [{lang}]: {}", got.iter().map(|(a, l)| format!("{a} {}", l.len())).collect::<Vec<_>>().join(", "));
    }
}

/// Every way the rules are reached refuses every input of the files, each with the kind and the
/// sentence the file gives, and the vectors keep their answers. The English rules and the Japanese
/// versions run side by side.
#[test]
fn every_way_refuses_each_one_for_the_reason_the_evaluator_gives() {
    // Every language there is a toolchain for, the tools level's (ritsu's DESIGN 10.2).
    if !need(Need::Python) {
        return;
    }
    side_by_side(&[(&ENGLISH, "en"), (&JAPANESE, "ja")], |&(rules, lang)| {
        let (_t, out) = generate(rules, lang);
        let counts: BTreeMap<String, usize> = refused(&out).into_iter().map(|(a, l)| (a, l.len())).collect();
        let (code, j) = rulec_test(&out);
        assert_eq!(code, 0, "{j:?}");
        let mut ways: BTreeMap<String, usize> = BTreeMap::new();
        for r in j.get("results").and_then(|v| v.as_arr()).unwrap_or_default() {
            let alias = r.get("rule").and_then(|v| v.as_str()).unwrap_or_default();
            let Some(n) = counts.get(alias) else { continue };
            if r.get("ran").and_then(|v| v.as_bool()) != Some(true) || r.get("via").and_then(|v| v.as_str()) == Some("proof") {
                continue;
            }
            assert_eq!(r.get("ok").and_then(|v| v.as_bool()), Some(true), "{alias} [{lang}]: {r:?}");
            assert_eq!(r.get("refused").and_then(|v| v.as_int()), Some(*n as i128), "{alias} [{lang}]: not every input was put: {r:?}");
            let way = format!("{}/{}", r.get("lang").and_then(|v| v.as_str()).unwrap_or_default(), r.get("via").and_then(|v| v.as_str()).unwrap_or_default());
            *ways.entry(way).or_default() += 1;
        }
        for way in ["python/runner", "python/mcp", "numpy/runner", "typescript/runner", "typescript/mcp", "rust/runner"] {
            assert!(ways.contains_key(way), "[{lang}] {way} did not run: {ways:?}");
        }
        println!("refused [{lang}]: {} ways: {}", ways.len(), ways.keys().cloned().collect::<Vec<_>>().join(", "));
    });
}

/// The days of a koyomi date are a reason of their own: `gen`, with koyomi joined as `ritsu rulec`
/// joins it, writes a day the date does not come to, and every way refuses it.
#[test]
fn a_day_koyomi_does_not_come_to_is_written_and_refused() {
    let path = root().join("tests/days/settlement.rule");
    let t = TempDir::new("refused-days");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let port: rulec::days::Port = Arc::new(koyomi::ports::Engine);
    let code = rulec::days::with(Some(port), || rulec::codegen::generate(&[path.to_str().unwrap()], t.path().to_str().unwrap(), &rulec::targets::Asked::up_to_0_40(), false, false, &mut out, &mut err));
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    let got = refused(t.path());
    let errors: Vec<&str> = got["settlement"].iter().map(|(_, e)| e.as_str()).collect();
    assert!(errors.contains(&"pay_day is not a day payment of payment_terms.cal comes to"), "{errors:#?}");
    if !need(Need::Python) {
        return;
    }
    let (code, j) = rulec_test(t.path());
    assert_eq!(code, 0, "{j:?}");
    let mut ran = 0;
    for r in j.get("results").and_then(|v| v.as_arr()).unwrap_or_default() {
        if r.get("rule").and_then(|v| v.as_str()) != Some("settlement") || r.get("ran").and_then(|v| v.as_bool()) != Some(true) {
            continue;
        }
        assert_eq!(r.get("ok").and_then(|v| v.as_bool()), Some(true), "{r:?}");
        assert_eq!(r.get("refused").and_then(|v| v.as_int()), Some(errors.len() as i128), "{r:?}");
        ran += 1;
    }
    assert!(ran >= 2, "{j:?}");
}

/// A door that lets an input through, or that refuses it for another reason, is caught and named:
/// the Python module with the range of `weight` taken out answers the input past it, and a Ruby
/// module that says another sentence of that range disagrees with the line it has to print.
#[test]
fn a_door_that_answers_or_says_another_sentence_is_caught() {
    if !need(Need::Python) {
        return;
    }
    let (_t, out) = generate(&ENGLISH[..1], "en");
    // The Python module answers a weight past its range.
    let py = out.join("python/express_delivery_quote.py");
    let src = std::fs::read_to_string(&py).unwrap();
    // The guard of the range the rule declares, and not the one of what int64 holds, which reads
    // the argument before any range is looked at (§15.328).
    let all: Vec<&str> = src.lines().collect();
    let guard = (1..all.len())
        .find(|&i| all[i].contains("raise RuleInputError(\"weight is out of range\"") && !all[i - 1].contains("9223372036854775807"))
        .expect("the range guard of weight");
    let broken: Vec<&str> = src.lines().enumerate().filter(|(i, _)| *i + 1 != guard && *i != guard).map(|(_, l)| l).collect();
    std::fs::write(&py, broken.join("\n") + "\n").unwrap();
    // The Ruby module says something else of a weight past its range.
    let rb = out.join("ruby/express_delivery_quote.rb");
    let have_ruby = Command::new("ruby").arg("--version").output().is_ok_and(|o| o.status.success());
    if have_ruby {
        let src = std::fs::read_to_string(&rb).unwrap();
        assert!(src.contains("\"weight is out of range\""), "the range guard of weight in Ruby");
        std::fs::write(&rb, src.replace("\"weight is out of range\"", "\"weight is wrong\"")).unwrap();
    } else {
        skip("ruby is not here; the Ruby half is not run");
    }
    let (code, j) = rulec_test(&out);
    assert_eq!(code, 1, "{j:?}");
    let failed = |lang: &str, via: &str| -> Json {
        j.get("results")
            .and_then(|v| v.as_arr())
            .unwrap_or_default()
            .iter()
            .find(|r| r.get("lang").and_then(|v| v.as_str()) == Some(lang) && r.get("via").and_then(|v| v.as_str()) == Some(via))
            .cloned()
            .unwrap_or_else(|| panic!("{lang}/{via} did not run: {j:?}"))
    };
    let p = failed("python", "runner");
    assert_eq!(p.get("ok").and_then(|v| v.as_bool()), Some(false), "{p:?}");
    let said = rulec::json::unparse(&p);
    assert!(said.contains("answered an input the reference evaluator refuses"), "{said}");
    if have_ruby {
        let r = failed("ruby", "runner");
        let d = r.get("first_diff").unwrap_or_else(|| panic!("{r:?}"));
        assert!(d.get("generated").and_then(|v| v.as_str()).is_some_and(|g| g.contains("weight is wrong")), "{r:?}");
        assert!(d.get("expected").and_then(|v| v.as_str()).is_some_and(|e| e.contains("weight is out of range")), "{r:?}");
    }
    // Every other language still refuses each input for its reason.
    assert_eq!(failed("go", "runner").get("ok").and_then(|v| v.as_bool()), Some(true));
}
