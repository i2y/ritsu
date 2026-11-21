//! A number's unit is ritsu's one type of units (ritsu-units; ritsu's DESIGN 5.3): two spellings of
//! one unit are the same unit, two units of a dimension are not, and a bound of a range may carry a
//! unit, counted in the unit of its type as rulec counts one. The checks say a range with the units
//! its files write, as ritsu's other languages say a number. The rules are read through rulec's own
//! answer to ritsu's port of rules.

use dandori::check::Checked;
use dandori::model::{Range, Ty};
use std::path::PathBuf;
use std::rc::Rc;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A flow checked as if it were the file `tests/fixtures/<name>`, with rulec's port.
fn check(name: &str, src: &str) -> Checked {
    dandori::sources::with_rules(Rc::new(rulec::ports::Engine::new()), || dandori::check::check_source(src, &root().join("tests/fixtures").join(name)))
}

fn said(c: &Checked) -> Vec<(String, String, Vec<String>)> {
    c.diags.iter().map(|d| (d.code.to_string(), d.en.clone(), d.notes.iter().map(|n| n.en.clone()).collect())).collect()
}

/// The workflow of ritsu's DESIGN 1.4: the hotel's rule answers `money[JPY, incl_tax]`, and the task
/// takes `money[円, incl_tax]`. `JPY` is another spelling of `円`, so the amount goes as it is.
#[test]
fn an_amount_in_jpy_goes_where_yen_is_taken() {
    let c = check(
        "hold.flow",
        "workflow hold v1\n\nuse rule hold from \"../../examples/hotel/rules/hold_amount.rule\"\n\ninputs\n  room   : hold.room\n  nights : int  range >=1 <=30\n\ntask authorize(amount: money[円, incl_tax])\n\nflow\n  let quote = hold(room: room, nights: nights)\n  authorize(amount: quote.amount)\n",
    );
    assert!(c.model.is_some() && c.diags.is_empty(), "{:?}", said(&c));
    // and the other way round: a task's yen into an input of JPY
    let c = check("pay.flow", "workflow pay v1\n\ninputs\n  amount : money[円, incl_tax]\n\ntask charge(amount: money[JPY, incl_tax])\n\nflow\n  charge(amount: amount)\n");
    assert!(c.model.is_some() && c.diags.is_empty(), "{:?}", said(&c));
}

/// Two units of one dimension are two units: dandori computes nothing (P1), so it does not convert
/// a value where it goes, and a rule says so if one is to be converted.
#[test]
fn a_mass_in_kg_does_not_go_where_g_is_taken() {
    let c = check("weigh.flow", "workflow weigh v1\n\ninputs\n  w : mass[kg]\n\ntask ship(weight: mass[g])\n\nflow\n  ship(weight: w)\n");
    assert_eq!(
        said(&c).iter().map(|(code, en, _)| (code.as_str(), en.as_str())).collect::<Vec<_>>(),
        [("E003", "expected `mass[g]` here, but this is `mass[kg]`")]
    );
    // nor tax in where it is left out
    let c = check("tax.flow", "workflow tax v1\n\ninputs\n  a : money[円, incl_tax]\n\ntask book(amount: money[円, excl_tax])\n\nflow\n  book(amount: a)\n");
    assert_eq!(said(&c).iter().map(|(code, _, _)| code.as_str()).collect::<Vec<_>>(), ["E003"]);
}

/// A bound written with a unit is counted in the unit of the type: what goes on the wire.
#[test]
fn a_bound_with_a_unit_is_counted_in_the_type() {
    let c = check(
        "bounds.flow",
        "workflow bounds v1\n\nrecord 請求\n  額 : money[円, incl_tax]  range >=0円 <=100万円\n  重さ : mass[g]  range >=1kg <=40kg\n  割引 : rate[step 0.1%]  range <=50%\n  税抜 : money[JPY, excl_tax]  range >=100銭\n  温度 : temperature[℃]  range >=41℉\n\nflow\n  pass\n",
    );
    let m = c.model.as_ref().unwrap_or_else(|| panic!("{:?}", said(&c)));
    let rec = m.records.iter().find(|r| r.name == "請求").unwrap();
    let r = |lo: Option<i64>, hi: Option<i64>| Range { lo, hi };
    assert_eq!(rec.ranges["額"], r(Some(0), Some(1_000_000)));
    assert_eq!(rec.ranges["重さ"], r(Some(1_000), Some(40_000)));
    assert_eq!(rec.ranges["割引"], r(None, Some(500)));
    assert_eq!(rec.ranges["税抜"], r(Some(1), None));
    assert_eq!(rec.ranges["温度"], r(Some(5), None));
    // a sign of temperature is a unit, never part of a name the code dandori writes could not spell
    for src in ["workflow w v1\n\ninputs\n  温度℃ : int\n\nflow\n  pass\n", "workflow w v1\n\ninputs\n  t : int  range >=41 <=50 ℉\n\nflow\n  pass\n"] {
        let c = check("bad.flow", src);
        assert_eq!(said(&c).iter().map(|(code, _, _)| code.as_str()).collect::<Vec<_>>(), ["E001"], "{src}");
    }
    // the types keep their spelling
    assert_eq!(m.ty_name(&rec.fields.iter().find(|(n, _)| n == "割引").unwrap().1), "rate[step 0.1%]");
    assert!(matches!(&rec.fields[0].1, Ty::Num(u) if u.to_string() == "money[円, incl_tax]"));
}

/// The checks say a range as ritsu's other languages say a number (ritsu's DESIGN 6.2, 8): each end
/// with the unit its file writes, its digits in threes from five of them, a rate's steps as a
/// percentage. A place's range is said in the place's type, a rule's input in the rule's, and what
/// is given in its own; a whole number the flow writes is said as it is written and, given to a
/// place of a unit, as the number of that unit it is: `600` is 60% of `rate[step 0.1%]`. A bound
/// is still written either way (above), and what `api` and an agent read keeps the numbers that go
/// on the wire (DESIGN 1.3).
#[test]
fn the_checks_say_a_range_with_the_units_its_files_write() {
    let c = check(
        "said.flow",
        "workflow said v1\n\nuse rule urgency from \"../../examples/order/rules/urgency.rule\"\n\ninputs\n  amount   : money[円, incl_tax]  range >=0円 <=2000000円\n  discount : rate[step 0.1%]  range <=80%\n  parcel   : mass[g]  range >=500g <=50kg\n  count    : int  range >=0 <=20000\n  loose    : money[JPY, incl_tax]\n\noutputs\n  total : money[JPY, incl_tax]  range <=1000000JPY\n\ntask charge(discount: rate[step 0.1%]  range <=50%, parcel: mass[g]  range >=1kg <=30kg, count: int  range >=1 <=10000)\n\nflow\n  let a = urgency(member: false, amount: amount)\n  let b = urgency(member: false, amount: loose)\n  let c = urgency(member: false, amount: 1500000)\n  charge(discount: discount, parcel: parcel, count: count)\n  charge(discount: 600, parcel: 40000, count: 1)\n  succeed total = amount\n",
    );
    let said: Vec<_> = c.diags.iter().map(|d| (d.code, d.line, d.en.as_str(), d.ja.as_str())).collect();
    assert_eq!(
        said,
        [
            ("E014", 18, "`amount` can be outside `>=0JPY <=1_000_000JPY`, the range of `amount` of the rule `urgency`: it is `>=0円 <=2_000_000円`", "`amount` は `>=0円 <=2_000_000円` で、規則 `urgency` の `amount` の範囲 `>=0JPY <=1_000_000JPY` を外れることがあります"),
            ("W104", 19, "nothing says what range `loose` is in (the input `loose` has no range), and `amount` of the rule `urgency` takes `>=0JPY <=1_000_000JPY`", "`loose` の範囲が分かりません（入力 `loose` に範囲がありません）。規則 `urgency` の `amount` が受け取るのは `>=0JPY <=1_000_000JPY` です"),
            ("E014", 20, "`1500000` is 1_500_000JPY, outside `>=0JPY <=1_000_000JPY`, the range of `amount` of the rule `urgency`", "`1500000` は 1_500_000JPY で、規則 `urgency` の `amount` の範囲 `>=0JPY <=1_000_000JPY` の外です"),
            ("E014", 21, "`discount` can be outside `<=50%`, the range of the parameter `discount` of `charge`: it is `<=80%`", "`discount` は `<=80%` で、`charge` の引数 `discount` の範囲 `<=50%` を外れることがあります"),
            ("E014", 21, "`parcel` can be outside `>=1000g <=30_000g`, the range of the parameter `parcel` of `charge`: it is `>=500g <=50_000g`", "`parcel` は `>=500g <=50_000g` で、`charge` の引数 `parcel` の範囲 `>=1000g <=30_000g` を外れることがあります"),
            ("E014", 21, "`count` can be outside `>=1 <=10_000`, the range of the parameter `count` of `charge`: it is `>=0 <=20_000`", "`count` は `>=0 <=20_000` で、`charge` の引数 `count` の範囲 `>=1 <=10_000` を外れることがあります"),
            ("E014", 22, "`600` is 60%, outside `<=50%`, the range of the parameter `discount` of `charge`", "`600` は 60% で、`charge` の引数 `discount` の範囲 `<=50%` の外です"),
            ("E014", 22, "`40000` is 40_000g, outside `>=1000g <=30_000g`, the range of the parameter `parcel` of `charge`", "`40000` は 40_000g で、`charge` の引数 `parcel` の範囲 `>=1000g <=30_000g` の外です"),
            ("W106", 22, "`600`, written for `discount`, counts steps of 0.1%: it is 60%", "`discount` に書いた `600` は、0.1% の刻みの数として読みます。60% です"),
            ("E014", 23, "`amount` can be outside `<=1_000_000JPY`, the range of the output `total`: it is `>=0円 <=2_000_000円`", "`amount` は `>=0円 <=2_000_000円` で、出力 `total` の範囲 `<=1_000_000JPY` を外れることがあります"),
        ]
    );
    // a range no number is in, said the same way
    let c = check("empty.flow", "workflow empty v1\n\nrecord R\n  fee  : money[JPY]  range >=100JPY <=10JPY\n  rate : rate[step 0.1%]  range >=60% <=50%\n  n    : int  range >=20000 <=10000\n\nflow\n  pass\n");
    let said: Vec<_> = c.diags.iter().map(|d| (d.code, d.en.as_str(), d.ja.as_str())).collect();
    assert_eq!(
        said,
        [
            ("E003", "no number is in `>=100JPY <=10JPY`", "`>=100JPY <=10JPY` に入る数はありません"),
            ("E003", "no number is in `>=60% <=50%`", "`>=60% <=50%` に入る数はありません"),
            ("E003", "no number is in `>=20_000 <=10_000`", "`>=20_000 <=10_000` に入る数はありません"),
        ]
    );
}

/// The signs of the units are read as rulec reads them, with ritsu-units' characters
/// (`ritsu_units::chars`): ℃ and ℉ are a word of their own right after `[`, and with `%` the unit
/// of a bound; the full-width `％` is `%`. A `%` in a rate's brackets is no unit: the type of a rate
/// writes its step.
#[test]
fn the_signs_of_the_units_are_read_as_rulec_reads_them() {
    let c = check("signs.flow", "workflow signs v1\n\nrecord R\n  t : temperature[℉]  range >=-40℉ <=104℉\n  h : rate[step 0.1％]  range >=0％ <=100％\n\nflow\n  pass\n");
    let m = c.model.as_ref().unwrap_or_else(|| panic!("{:?}", said(&c)));
    let rec = m.records.iter().find(|r| r.name == "R").unwrap();
    assert_eq!(rec.ranges["t"], Range { lo: Some(-40), hi: Some(104) });
    assert_eq!(rec.ranges["h"], Range { lo: Some(0), hi: Some(1000) });
    // the type is written in the table's spelling
    assert_eq!(m.ty_name(&rec.fields.iter().find(|(n, _)| n == "h").unwrap().1), "rate[step 0.1%]");
    let c = check("bad.flow", "workflow bad v1\n\ninputs\n  x : rate[%]  range >=0% <=100%\n\nflow\n  pass\n");
    assert_eq!(said(&c), [("E002".to_string(), "there is no type `rate[%]`".to_string(), vec!["the type of a rate writes its step, as in `rate[step 1%]`".to_string()])]);
}

/// A spelling the table of units does not have is no type, and says why.
#[test]
fn a_unit_the_table_does_not_have_is_no_type() {
    for (ty, note) in [("mass[foo]", "`foo` is not a unit"), ("money[円, foo]", "`foo` is neither incl_tax nor excl_tax"), ("length[kg]", "`kg` is not a unit of length")] {
        let c = check("bad.flow", &format!("workflow bad v1\n\ninputs\n  x : {ty}\n\nflow\n  pass\n"));
        assert_eq!(said(&c), [("E002".to_string(), format!("there is no type `{ty}`"), vec![note.to_string()])], "{ty}");
    }
    // a word that is no dimension says what a type is, as before
    let c = check("bad.flow", "workflow bad v1\n\ninputs\n  x : weight[kg]\n\nflow\n  pass\n");
    assert_eq!(said(&c)[0].0, "E002");
    assert!(said(&c)[0].2[0].starts_with("a type is int, string, bool"), "{:?}", said(&c));
}

/// An end of a range is in the range, and is written with `>=` and `<=`. An end written with `>`
/// or `<` says so where it is written, in both languages; the same character anywhere else is the
/// character that cannot be written there.
#[test]
fn an_end_written_without_its_equals_sign_says_how_an_end_is_written() {
    for (src, col, sym) in [
        ("workflow w v1\n\ninputs\n  n : int  range >0 <10\n\nflow\n  pass\n", 18, '>'),
        ("workflow w v1\n\ninputs\n  n : int  range >=1 <10\n\nflow\n  pass\n", 22, '<'),
        ("workflow w v1\n\ninputs\n  d : date  range >2026-03-31\n\nflow\n  pass\n", 19, '>'),
        ("workflow w v1\n\ntask t() -> int range <100\n\nflow\n  pass\n", 23, '<'),
    ] {
        let c = check("ends.flow", src);
        let said: Vec<_> = c.diags.iter().map(|d| (d.code, d.col, d.en.as_str(), d.ja.as_str())).collect();
        assert_eq!(
            said,
            [(
                "E001",
                col,
                format!("the ends of a range are written with `>=` and `<=`, not `{sym}` (an end is in the range)").as_str(),
                format!("範囲の端は `>=` と `<=` で書いてください（端の値も範囲に入ります。`{sym}` は使えません）").as_str()
            )],
            "{src}"
        );
    }
    let c = check("ends.flow", "workflow w v1\n\ninputs\n  n : int\n\nflow\n  let x = n > 1\n");
    assert_eq!(c.diags.iter().map(|d| (d.code, d.en.as_str())).collect::<Vec<_>>(), [("E001", "unexpected character `>`")]);
}

// ── A number written with its unit where its type is known (DESIGN 40) ──────────────────────────

/// The flow `t` twice: each `{written|whole}` as the number written with its unit, and as the whole
/// number it counts to in the type of its place.
fn forms(t: &str) -> (String, String) {
    let (mut unit, mut whole) = (String::new(), String::new());
    let mut rest = t;
    while let Some(i) = rest.find("{{") {
        unit.push_str(&rest[..i]);
        whole.push_str(&rest[..i]);
        let end = rest[i..].find("}}").expect("a `{{…}}` closed") + i;
        let (u, w) = rest[i + 2..end].split_once('|').expect("`{{written|whole}}`");
        unit.push_str(u);
        whole.push_str(w);
        rest = &rest[end + 2..];
    }
    unit.push_str(rest);
    whole.push_str(rest);
    (unit, whole)
}

/// Every place a number with a unit goes, where its type is known: an argument of a task's call and
/// of a rule's, a field of a record whose type is known, an item of a list whose type is known (by
/// the place, or by another item), a `let` that writes the type, the `yield` of a `for` whose list's
/// type is written, the list a `for` goes through, and an output of `succeed`. `{{written|whole}}`
/// is a number with its unit and the whole number it counts to.
const PLACES: &str = "workflow units v1

use rule urgency from \"../../examples/order/rules/urgency.rule\"

inputs
  claims : list[string]
  member : bool

outputs
  fee   : rate[step 0.1%]
  total : money[JPY]

record claim
  fee    : rate[step 0.1%]
  amount : money[JPY]

task pay(fee: rate[step 0.1%], c: claim, fees: list[rate[step 0.1%]], weight: mass[g], cap: rate[step 0.1%]?) -> rate[step 0.1%]
  lambda \"arn:aws:lambda:eu-west-2:123456789012:function:pay\"
  idempotent

flow
  let f: rate[step 0.1%] = {{0.5%|5}}
  let got = pay(fee: {{0.7%|7}}, c: {fee: {{0.8%|8}}, amount: {{100万円|1000000}}}, fees: [{{0.5%|5}}, {{1%|10}}], weight: {{1.5kg|1500}}, cap: {{2%|20}})
  let u = urgency(member: member, amount: {{30000JPY|30000}})
  let c: claim = {fee: {{1.2%|12}}, amount: {{300JPY|300}}}
  let xs = [f, {{0.9%|9}}]
  let all: list[rate[step 0.1%]] = for id in claims at most 10
    yield {{0.3%|3}}
  for x in [f, {{0.4%|4}}] at most 3
    pass
  succeed fee = {{0.5%|5}}, total = {{300JPY|300}}
";

/// [`PLACES`], with the names in Japanese.
const PLACES_JA: &str = "workflow 単位 v1

use rule 急ぎ from \"../../examples/order/rules/出荷の急ぎ.rule\"

inputs
  請求の並び : list[string]
  会員       : bool

outputs
  手数料 : rate[step 0.1%]
  合計   : money[円]

record 請求
  手数料 : rate[step 0.1%]
  金額   : money[円]

task 払う(手数料: rate[step 0.1%], 件: 請求, 手数料の並び: list[rate[step 0.1%]], 重さ: mass[g], 上限: rate[step 0.1%]?) -> rate[step 0.1%]
  lambda \"arn:aws:lambda:ap-northeast-1:123456789012:function:pay\"
  idempotent

flow
  let 率: rate[step 0.1%] = {{0.5%|5}}
  let 結果 = 払う(手数料: {{0.7%|7}}, 件: {手数料: {{0.8%|8}}, 金額: {{100万円|1000000}}}, 手数料の並び: [{{0.5%|5}}, {{1%|10}}], 重さ: {{1.5kg|1500}}, 上限: {{2%|20}})
  let 判定 = 急ぎ(会員: 会員, 金額: {{3万円|30000}})
  let 一件: 請求 = {手数料: {{1.2%|12}}, 金額: {{300円|300}}}
  let 並び = [率, {{0.9%|9}}]
  let 全部: list[rate[step 0.1%]] = for 番号 in 請求の並び at most 10
    yield {{0.3%|3}}
  for 一つ in [率, {{0.4%|4}}] at most 3
    pass
  succeed 手数料 = {{0.5%|5}}, 合計 = {{300円|300}}
";

/// A number written with its unit is the whole number of its type's unit it counts to (`0.5%` of
/// `rate[step 0.1%]` is 5, `1.5kg` of `mass[g]` is 1500, `100万円` of `money[JPY]` is 1000000), at
/// every place its type is known: the flow lowers to the same statements as the flow that writes the
/// whole numbers, and says nothing. The whole numbers on a rate counted in steps of 0.1% are W106,
/// and nothing else is.
#[test]
fn a_number_with_a_unit_goes_where_its_type_is_known() {
    for (name, t) in [("units.flow", PLACES), ("単位.flow", PLACES_JA)] {
        let (unit, whole) = forms(t);
        let cu = check(name, &unit);
        let mu = cu.model.as_ref().unwrap_or_else(|| panic!("{name}: {:?}", said(&cu)));
        assert!(cu.diags.is_empty(), "{name}: {:?}", said(&cu));
        let cw = check(name, &whole);
        let mw = cw.model.as_ref().unwrap_or_else(|| panic!("{name}: {:?}", said(&cw)));
        assert_eq!(cw.diags.iter().map(|d| (d.code, d.line)).collect::<Vec<_>>(), [22, 23, 23, 23, 23, 23, 25, 26, 28, 29, 31].map(|l| ("W106", l)), "{name}");
        assert_eq!(format!("{:?}", mu.flow), format!("{:?}", mw.flow), "{name}: the flows lower alike");
        assert!(mw.written.is_empty(), "{name}: {:?}", mw.written);
        assert_eq!(mu.written.len(), 16, "{name}: {:?}", mu.written);
    }
    // a variable whose type is known takes one again: `f` is a rate in steps of 0.1% (its own
    // `let` writes the type), so `0.6%` is 6
    let c = check("again.flow", "workflow again v1\n\nflow\n  let f: rate[step 0.1%] = 0.5%\n  let f = 0.6%\n");
    let m = c.model.as_ref().unwrap_or_else(|| panic!("{:?}", said(&c)));
    assert!(c.diags.is_empty(), "{:?}", said(&c));
    let values: Vec<String> = m.flow.iter().map(|s| format!("{:?}", s.kind)).collect();
    assert!(values[1].starts_with("Assign { name: \"f\", expr: Int(6)"), "{values:?}");
    let c = check("again.flow", "workflow 入れ直し v1\n\nflow\n  let 率: rate[step 0.1%] = 0.5%\n  let 率 = 0.6%\n");
    assert!(c.model.is_some() && c.diags.is_empty(), "{:?}", said(&c));
    // a list whose type nothing writes takes it from an item with no unit, wherever that item
    // stands: `[0.9%, f]` is a list of rates in steps of 0.1%, its first item 9
    for (name, src) in [("first.flow", "workflow first v1\n\nflow\n  let f: rate[step 0.1%] = 0.5%\n  let xs = [0.9%, f]\n"), ("先頭.flow", "workflow 先頭 v1\n\nflow\n  let 率: rate[step 0.1%] = 0.5%\n  let 並び = [0.9%, 率]\n")] {
        let c = check(name, src);
        let m = c.model.as_ref().unwrap_or_else(|| panic!("{name}: {:?}", said(&c)));
        assert!(c.diags.is_empty(), "{name}: {:?}", said(&c));
        let list = format!("{:?}", m.flow[1].kind);
        assert!(list.contains("items: [Int(9), Var {"), "{name}: {list}");
    }
}

/// The code a number with a unit builds is the code its whole number builds, byte for byte, on every
/// platform: the generators read the whole number, and the number as written is only what the
/// checks and the pages say. (The head of every file names the `.flow`'s hash, which is the hash of
/// the text: it is put the same for the two, and so is the build id it goes into.)
#[test]
fn a_number_with_a_unit_builds_the_code_its_whole_number_builds() {
    for (name, t) in [("units.flow", PLACES), ("単位.flow", PLACES_JA)] {
        // the call of the rule is a place of the check, not of the code: the flow goes without it
        let t: String = t.lines().filter(|l| !l.contains("urgency") && !l.contains("急ぎ")).map(|l| format!("{l}\n")).collect();
        let (unit, whole) = forms(&t);
        let built = |src: &str| {
            let c = dandori::check::check_source(src, &root().join("tests/fixtures").join(name));
            let mut m = c.model.unwrap_or_else(|| panic!("{name}: {:?}", c.diags));
            m.source_sha256 = "0".repeat(64);
            dandori::commands::TARGETS.map(|target| (target, dandori::commands::build(&m, target).expect("a target dandori builds for").unwrap_or_else(|d| panic!("{name}: {target}: {d:?}"))))
        };
        let (u, w) = (built(&unit), built(&whole));
        for ((target, u), (_, w)) in u.iter().zip(&w) {
            assert!(!u.is_empty(), "{name}: {target} writes nothing");
            assert_eq!(u.iter().map(|(p, _)| p).collect::<Vec<_>>(), w.iter().map(|(p, _)| p).collect::<Vec<_>>(), "{name}: {target}");
            for ((path, a), (_, b)) in u.iter().zip(w) {
                assert!(a == b, "{name}: {target}: {path} differs");
            }
        }
    }
}

/// The head of the flows of [`a_number_with_a_unit_where_nothing_says_its_unit_is_e003`].
const REFUSED: &str = "workflow refused v1\n\ninputs\n  claims : list[string]\n\nrecord claim\n  fee   : rate[step 0.1%]\n  extra : json\n\ntask pay(fee: rate[step 0.1%], amount: money[JPY], weight: mass[g], n: int, id: string, three: rate[step 3%])\n\nflow\n";
const REFUSED_JA: &str = "workflow 止まる v1\n\ninputs\n  請求の並び : list[string]\n\nrecord 請求\n  手数料 : rate[step 0.1%]\n  追加   : json\n\ntask 払う(手数料: rate[step 0.1%], 金額: money[円], 重さ: mass[g], 件数: int, 番号: string, 三刻み: rate[step 3%])\n\nflow\n";

/// A number with a unit where nothing says what whole number it is, E003: where no type is known (a
/// new variable of a `let` that writes none, what a `match` reads, a list whose other items say no
/// type, the `yield` of a `for` whose list's type is not written), where a type with no unit goes
/// (`json`, `int`, `string`), with a unit of another dimension or currency, and where it does not
/// count to a whole number of its type's unit, or to one of 64 bits.
#[test]
fn a_number_with_a_unit_where_nothing_says_its_unit_is_e003() {
    let unknown = |raw: &str| {
        (
            format!("a number with a unit, `{raw}`, goes where its type is known: an argument of a call, an output of `succeed`, or a field, an item or a variable whose type is known"),
            format!("単位の付いた数 `{raw}` は、型の分かる所（呼び出しの引数、`succeed` の出力、型の分かるフィールド・要素・変数）に書いてください"),
            Some(format!("a value travels as a whole number in the unit of its type, so where the type is not known, nothing says what whole number `{raw}` is")),
        )
    };
    let not_of = |raw: &str, ty: &str, note: Option<&str>| (format!("`{raw}` is not a value of `{ty}`"), format!("`{raw}` は `{ty}` の値ではありません"), note.map(String::from));
    let fraction = |raw: &str, ty: &str| {
        (
            format!("`{raw}`, counted in `{ty}`, is not a whole number"),
            format!("`{raw}` を `{ty}` で数えると、整数になりません"),
            Some("a value travels as a whole number in the unit of its type, so a number written with a unit must come to one too".to_string()),
        )
    };
    let call = |args: &str| format!("  pay(fee: 1%, amount: 1JPY, weight: 1g, n: 1, id: \"x\", three: 3%{args})\n");
    let call_ja = |args: &str| format!("  払う(手数料: 1%, 金額: 1円, 重さ: 1g, 件数: 1, 番号: \"x\", 三刻み: 3%{args})\n");
    let json_note = "a value of `json` has no unit, so nothing says what whole number `0.5%` is; write the whole number that is passed";
    let cases: Vec<(String, String, (String, String, Option<String>))> = vec![
        ("  let x = 0.5%\n".into(), "  let 率 = 0.5%\n".into(), unknown("0.5%")),
        ("  match 0.5%\n    _ => pass\n".into(), "  match 0.5%\n    _ => pass\n".into(), unknown("0.5%")),
        ("  let xs = [0.5%, 1%]\n".into(), "  let 並び = [0.5%, 1%]\n".into(), unknown("0.5%")),
        ("  let all = for id in claims at most 3\n    yield 0.5%\n".into(), "  let 全部 = for 番号 in 請求の並び at most 3\n    yield 0.5%\n".into(), unknown("0.5%")),
        ("  let c: claim = {fee: 0.5%, extra: 0.5%}\n".into(), "  let 件: 請求 = {手数料: 0.5%, 追加: 0.5%}\n".into(), not_of("0.5%", "json", Some(json_note))),
        (call("").replace("n: 1", "n: 5%"), call_ja("").replace("件数: 1", "件数: 5%"), not_of("5%", "int", Some("a value of `int` is written without a unit"))),
        (call("").replace("\"x\"", "5%"), call_ja("").replace("\"x\"", "5%"), not_of("5%", "string", None)),
        (call("").replace("fee: 1%", "fee: 0.5kg"), call_ja("").replace("手数料: 1%", "手数料: 0.5kg"), not_of("0.5kg", "rate[step 0.1%]", None)),
        (call("").replace("1JPY", "10USD"), call_ja("").replace("1円", "10USD"), not_of("10USD", "money[JPY]", None)),
        (call("").replace("fee: 1%", "fee: 0.55%"), call_ja("").replace("手数料: 1%", "手数料: 0.55%"), fraction("0.55%", "rate[step 0.1%]")),
        (call("").replace("1JPY", "1.5JPY"), call_ja("").replace("1円", "1.5円"), fraction("1.5JPY", "money[JPY]")),
        (call("").replace("three: 3%", "three: 5%"), call_ja("").replace("三刻み: 3%", "三刻み: 5%"), fraction("5%", "rate[step 3%]")),
        (
            call("").replace("1g", "10000000000000000kg"),
            call_ja("").replace("1g", "10000000000000000kg"),
            ("`10000000000000000kg`, counted in `mass[g]`, does not fit a whole number of 64 bits".into(), "`10000000000000000kg` を `mass[g]` で数えると、64 ビットの整数に収まりません".into(), None),
        ),
    ];
    for (body, body_ja, (en, ja, note)) in &cases {
        // the money of the Japanese flow is in yen, as its type and its numbers write it
        let ja_flow = format!("{REFUSED_JA}{body_ja}");
        let yen = |s: &String| s.replace("money[JPY]", "money[円]").replace("1.5JPY", "1.5円");
        for (src, en, ja) in [(format!("{REFUSED}{body}"), en.clone(), ja.clone()), (ja_flow, yen(en), yen(ja))] {
            let c = check("refused.flow", &src);
            let got: Vec<_> = c.diags.iter().map(|d| (d.code, d.en.clone(), d.ja.clone(), d.notes.first().map(|n| n.en.clone()))).collect();
            assert_eq!(got, [("E003", en, ja, note.clone())], "{src}");
        }
    }
    // a bound too, past 64 bits
    let c = check("bound.flow", "workflow bound v1\n\nrecord R\n  w : mass[g]  range <=10000000000000000kg\n\nflow\n  pass\n");
    assert_eq!(
        c.diags.iter().map(|d| (d.code, d.en.as_str(), d.ja.as_str())).collect::<Vec<_>>(),
        [("E003", "the bound `10000000000000000kg`, counted in `mass[g]`, does not fit a whole number of 64 bits", "範囲の端 `10000000000000000kg` を `mass[g]` で数えると、64 ビットの整数に収まりません")]
    );
}

/// W106 says a whole number written where a rate counts steps other than 1%, at each place: the
/// number is that many steps, and a reader cannot tell 5 steps from 5%. The note writes it with its
/// unit, as the place takes it (`fee: 0.5%`, `fee = 0.5%`, `0.5%`).
#[test]
fn w106_says_a_whole_number_where_a_rate_counts_steps_other_than_one_percent() {
    let (_, whole) = forms(PLACES);
    let c = check("units.flow", &whole);
    let got: Vec<_> = c.diags.iter().map(|d| (d.code, d.line, d.en.as_str(), d.notes.iter().map(|n| n.en.as_str()).collect::<Vec<_>>())).collect();
    let w = |line: usize, en: &'static str, note: &'static str| ("W106", line, en, vec![note]);
    assert_eq!(
        got,
        [
            w(22, "`5`, written for `f`, counts steps of 0.1%: it is 0.5%", "Write it `0.5%`, so that a reader sees the rate; the generated code passes the same 5"),
            w(23, "`7`, written for `fee`, counts steps of 0.1%: it is 0.7%", "Write it `fee: 0.7%`, so that a reader sees the rate; the generated code passes the same 7"),
            w(23, "`8`, written for `fee`, counts steps of 0.1%: it is 0.8%", "Write it `fee: 0.8%`, so that a reader sees the rate; the generated code passes the same 8"),
            w(23, "`5`, written as an item of `fees`, counts steps of 0.1%: it is 0.5%", "Write it `0.5%`, so that a reader sees the rate; the generated code passes the same 5"),
            w(23, "`10`, written as an item of `fees`, counts steps of 0.1%: it is 1%", "Write it `1%`, so that a reader sees the rate; the generated code passes the same 10"),
            w(23, "`20`, written for `cap`, counts steps of 0.1%: it is 2%", "Write it `cap: 2%`, so that a reader sees the rate; the generated code passes the same 20"),
            w(25, "`12`, written for `fee`, counts steps of 0.1%: it is 1.2%", "Write it `fee: 1.2%`, so that a reader sees the rate; the generated code passes the same 12"),
            w(26, "`9`, written as an item of `xs`, counts steps of 0.1%: it is 0.9%", "Write it `0.9%`, so that a reader sees the rate; the generated code passes the same 9"),
            w(28, "`3`, written as an item of `all`, counts steps of 0.1%: it is 0.3%", "Write it `0.3%`, so that a reader sees the rate; the generated code passes the same 3"),
            w(29, "`4`, written in the list `for x` goes through, counts steps of 0.1%: it is 0.4%", "Write it `0.4%`, so that a reader sees the rate; the generated code passes the same 4"),
            w(31, "`5`, written for `fee`, counts steps of 0.1%: it is 0.5%", "Write it `fee = 0.5%`, so that a reader sees the rate; the generated code passes the same 5"),
        ]
    );
    // and in Japanese, with the names in Japanese
    let (_, whole) = forms(PLACES_JA);
    let c = check("単位.flow", &whole);
    let got: Vec<_> = c.diags.iter().map(|d| (d.code, d.ja.as_str(), d.notes.iter().map(|n| n.ja.as_str()).collect::<Vec<_>>())).collect();
    let w = |ja: &'static str, note: &'static str| ("W106", ja, vec![note]);
    assert_eq!(
        got,
        [
            w("`率` に書いた `5` は、0.1% の刻みの数として読みます。0.5% です", "読む人に率が分かるよう、`0.5%` と書いてください。生成するコードが渡す数（5）は変わりません"),
            w("`手数料` に書いた `7` は、0.1% の刻みの数として読みます。0.7% です", "読む人に率が分かるよう、`手数料: 0.7%` と書いてください。生成するコードが渡す数（7）は変わりません"),
            w("`手数料` に書いた `8` は、0.1% の刻みの数として読みます。0.8% です", "読む人に率が分かるよう、`手数料: 0.8%` と書いてください。生成するコードが渡す数（8）は変わりません"),
            w("`手数料の並び` の要素に書いた `5` は、0.1% の刻みの数として読みます。0.5% です", "読む人に率が分かるよう、`0.5%` と書いてください。生成するコードが渡す数（5）は変わりません"),
            w("`手数料の並び` の要素に書いた `10` は、0.1% の刻みの数として読みます。1% です", "読む人に率が分かるよう、`1%` と書いてください。生成するコードが渡す数（10）は変わりません"),
            w("`上限` に書いた `20` は、0.1% の刻みの数として読みます。2% です", "読む人に率が分かるよう、`上限: 2%` と書いてください。生成するコードが渡す数（20）は変わりません"),
            w("`手数料` に書いた `12` は、0.1% の刻みの数として読みます。1.2% です", "読む人に率が分かるよう、`手数料: 1.2%` と書いてください。生成するコードが渡す数（12）は変わりません"),
            w("`並び` の要素に書いた `9` は、0.1% の刻みの数として読みます。0.9% です", "読む人に率が分かるよう、`0.9%` と書いてください。生成するコードが渡す数（9）は変わりません"),
            w("`全部` の要素に書いた `3` は、0.1% の刻みの数として読みます。0.3% です", "読む人に率が分かるよう、`0.3%` と書いてください。生成するコードが渡す数（3）は変わりません"),
            w("`for 一つ` が回るリストに書いた `4` は、0.1% の刻みの数として読みます。0.4% です", "読む人に率が分かるよう、`0.4%` と書いてください。生成するコードが渡す数（4）は変わりません"),
            w("`手数料` に書いた `5` は、0.1% の刻みの数として読みます。0.5% です", "読む人に率が分かるよう、`手数料 = 0.5%` と書いてください。生成するコードが渡す数（5）は変わりません"),
        ]
    );
    // the end of a range, and a rate in other steps than 0.1%
    let c = check(
        "ends.flow",
        "workflow ends v1\n\nrecord R\n  a : rate[step 0.1%]  range >=0 <=100\n  b : rate[step 0.1%]  range >=5\n  c : rate[step 3%]  range <=10\n\ntask pay(fee: rate[step 3%])\n\nflow\n  pay(fee: 2)\n",
    );
    let got: Vec<_> = c.diags.iter().map(|d| (d.line, d.en.as_str(), d.ja.as_str(), d.notes[0].en.as_str(), d.notes[0].ja.as_str())).collect();
    assert_eq!(
        got,
        [
            (4, "`100`, written as an end of the range, counts steps of 0.1%: it is 10%", "範囲の端に書いた `100` は、0.1% の刻みの数として読みます。10% です", "Write it `<=10%`, so that a reader sees the rate; the range is the same", "読む人に率が分かるよう、`<=10%` と書いてください。範囲は変わりません"),
            (5, "`5`, written as an end of the range, counts steps of 0.1%: it is 0.5%", "範囲の端に書いた `5` は、0.1% の刻みの数として読みます。0.5% です", "Write it `>=0.5%`, so that a reader sees the rate; the range is the same", "読む人に率が分かるよう、`>=0.5%` と書いてください。範囲は変わりません"),
            (6, "`10`, written as an end of the range, counts steps of 3%: it is 30%", "範囲の端に書いた `10` は、3% の刻みの数として読みます。30% です", "Write it `<=30%`, so that a reader sees the rate; the range is the same", "読む人に率が分かるよう、`<=30%` と書いてください。範囲は変わりません"),
            (11, "`2`, written for `fee`, counts steps of 3%: it is 6%", "`fee` に書いた `2` は、3% の刻みの数として読みます。6% です", "Write it `fee: 6%`, so that a reader sees the rate; the generated code passes the same 2", "読む人に率が分かるよう、`fee: 6%` と書いてください。生成するコードが渡す数（2）は変わりません"),
        ]
    );
}

/// W106 is not said where a whole number reads as it is: a rate in steps of 1% (the number is the
/// percentage), money and a quantity (the type names the unit the number counts), `int`, 0 (0% in
/// any steps), the ends of their ranges, and a number written with its unit.
#[test]
fn w106_is_not_said_where_a_whole_number_reads_as_it_is() {
    let quiet = "workflow quiet v1\n\nrecord R\n  a : rate[step 0.1%]  range >=0 <=100%\n  b : rate[step 1%]  range <=50\n  c : money[JPY]  range <=1000\n  d : mass[g]  range <=1500\n  e : rate[step 0.1%]  range >=0% <=10%\n\ntask pay(fee: rate[step 0.1%], flat: rate[step 1%], amount: money[JPY], weight: mass[g], n: int)\n\nflow\n  pay(fee: 0, flat: 5, amount: 300, weight: 1500, n: 7)\n  pay(fee: 0.5%, flat: 5%, amount: 300JPY, weight: 1.5kg, n: 7)\n";
    let quiet_ja = "workflow 静か v1\n\nrecord 記録\n  甲 : rate[step 0.1%]  range >=0 <=100%\n  乙 : rate[step 1%]  range <=50\n  丙 : money[円]  range <=1000\n  丁 : mass[g]  range <=1500\n  戊 : rate[step 0.1%]  range >=0% <=10%\n\ntask 払う(手数料: rate[step 0.1%], 一律: rate[step 1%], 金額: money[円], 重さ: mass[g], 件数: int)\n\nflow\n  払う(手数料: 0, 一律: 5, 金額: 300, 重さ: 1500, 件数: 7)\n  払う(手数料: 0.5%, 一律: 5%, 金額: 300円, 重さ: 1.5kg, 件数: 7)\n";
    for (name, src) in [("quiet.flow", quiet), ("静か.flow", quiet_ja)] {
        let c = check(name, src);
        assert!(c.model.is_some() && c.diags.is_empty(), "{name}: {:?}", said(&c));
    }
}

/// What the checks, `api` and the page say of a number written with its unit, they say as written:
/// E014 of an argument of a task and of a rule and of an output (`60%` is outside `<=50%`), W904 of
/// a secret, the steps of the run E010 shows, the outputs of a `succeed` in `api`, and the lines of
/// the page.
#[test]
fn a_number_with_a_unit_is_said_as_it_is_written() {
    let c = check(
        "quoted.flow",
        "workflow quoted v1\n\nuse rule urgency from \"../../examples/order/rules/urgency.rule\"\n\ninputs\n  member : bool\n\noutputs\n  total : money[JPY]  range <=1000000JPY\n\ntask charge(fee: rate[step 0.1%]  range <=50%)\n\nflow\n  let a = urgency(member: member, amount: 150万円)\n  charge(fee: 60%)\n  succeed total = 2000000JPY\n",
    );
    let got: Vec<_> = c.diags.iter().map(|d| (d.code, d.en.as_str(), d.ja.as_str())).collect();
    assert_eq!(
        got,
        [
            ("E014", "`150万円` is outside `>=0JPY <=1_000_000JPY`, the range of `amount` of the rule `urgency`", "`150万円` は規則 `urgency` の `amount` の範囲 `>=0JPY <=1_000_000JPY` の外です"),
            ("E014", "`60%` is outside `<=50%`, the range of the parameter `fee` of `charge`", "`60%` は `charge` の引数 `fee` の範囲 `<=50%` の外です"),
            ("E014", "`2000000JPY` is outside `<=1_000_000JPY`, the range of the output `total`", "`2000000JPY` は出力 `total` の範囲 `<=1_000_000JPY` の外です"),
        ]
    );
    let c = check(
        "引用.flow",
        "workflow 引用 v1\n\nuse rule 急ぎ from \"../../examples/order/rules/出荷の急ぎ.rule\"\n\ninputs\n  会員 : bool\n\noutputs\n  合計 : money[円]  range <=100万円\n\ntask 請求する(手数料: rate[step 0.1%]  range <=50%)\n\nflow\n  let 判定 = 急ぎ(会員: 会員, 金額: 150万円)\n  請求する(手数料: 60%)\n  succeed 合計 = 200万円\n",
    );
    let got: Vec<_> = c.diags.iter().map(|d| (d.code, d.ja.as_str())).collect();
    assert_eq!(
        got,
        [
            ("E014", "`150万円` は規則 `急ぎ` の `金額` の範囲 `>=0円 <=1_000_000円` の外です"),
            ("E014", "`60%` は `請求する` の引数 `手数料` の範囲 `<=50%` の外です"),
            ("E014", "`200万円` は出力 `合計` の範囲 `<=1_000_000円` の外です"),
        ]
    );
    // a secret written out
    let secret = "workflow secret v1\n\nrecord slip\n  name   : string\n  salary : money[JPY]  secret\n  bonus  : rate[step 0.1%]  secret\n\ntask send(slip: slip, extra: money[JPY]  secret)\n  lambda \"arn:aws:lambda:eu-west-2:123456789012:function:send\"\n  idempotent\n\nflow\n  send(slip: {name: \"a\", salary: 300000JPY, bonus: 2.5%}, extra: 5000JPY)\n";
    let c = check("secret.flow", secret);
    let got: Vec<_> = c.diags.iter().map(|d| (d.code, d.en.as_str(), d.ja.as_str())).collect();
    assert_eq!(
        got,
        [
            ("W904", "the secret `300000JPY` is kept in the history of the workflow, as the argument `slip` of `send`", "秘密の値 `300000JPY` が、`send` の引数 `slip` として、ワークフローの履歴に残ります"),
            ("W904", "the secret `2.5%` is kept in the history of the workflow, as the argument `slip` of `send`", "秘密の値 `2.5%` が、`send` の引数 `slip` として、ワークフローの履歴に残ります"),
            ("W904", "the secret `5000JPY` is kept in the history of the workflow, as the argument `extra` of `send`", "秘密の値 `5000JPY` が、`send` の引数 `extra` として、ワークフローの履歴に残ります"),
        ]
    );
    // the run E010 shows, as written
    let seen = "workflow seen v1\n\nenum tier = basic | gold\n\ntask pick(fee: rate[step 0.1%]) -> tier\n\nflow\n  let f: rate[step 0.1%] = 0.5%\n  for x in [f, 0.4%] at most 3\n    pass\n  let t = pick(fee: f)\n  let f = 0.7%\n  match t\n    basic => pass\n";
    let c = check("seen.flow", seen);
    let [d] = &c.diags[..] else { panic!("{:?}", said(&c)) };
    assert_eq!(d.code, "E010");
    assert_eq!(d.path.iter().map(|s| (s.line, s.en.as_str(), s.ja.as_str())).collect::<Vec<_>>(), [
        (8, "f = 0.5%", "f = 0.5%"),
        (9, "for x in [f, 0.4%] (at most 3)", "for x in [f, 0.4%]（3 個まで）"),
        (11, "t = pick(…)", "t = pick(…)"),
        (12, "f = 0.7%", "f = 0.7%"),
    ]);
    // `api` and the page
    let t: String = PLACES.lines().filter(|l| !l.contains("urgency")).map(|l| format!("{l}\n")).collect();
    let (unit, _) = forms(&t);
    let dir = ritsu_testkit::TempDir::new("units-written");
    let file = dir.path().join("units.flow");
    std::fs::write(&file, &unit).unwrap();
    let (src, d) = dandori::check::drawable(&file).unwrap();
    let m = d.model.as_ref().unwrap_or_else(|| panic!("{:?}", d.diags));
    let api = dandori::api::json(m);
    let outputs = api.get("succeeds").and_then(|s| s.as_arr()).and_then(|s| s.first()).and_then(|s| s.get("outputs")).cloned().unwrap();
    assert_eq!([outputs.get("fee").and_then(|v| v.as_str()), outputs.get("total").and_then(|v| v.as_str())], [Some("0.5%"), Some("300JPY")], "{outputs:?}");
    let page = dandori::doc::markdown(&dandori::doc::Input { m, src: &src, file: "units.flow", facts: &d.facts, diags: &d.diags, lang: dandori::diag::Lang::En });
    for line in ["let f = 0.5%", "for x in [f, 0.4%] at most 3", "yield 0.3%", "succeed fee = 0.5%, total = 300JPY"] {
        assert!(page.contains(line), "the page has no `{line}`:\n{page}");
    }
}

// ── A number a case holds an input of its machine to (DESIGN 40) ────────────────────────────────

/// A rule whose machine holds four numbers: a rate in steps of 0.1%, one in steps of 1%, an amount
/// and a count. Its table tells each apart at one value, so its values of `fee` are 0%, 0.1%, 0.5%,
/// 0.6% and 10% (a value for each end and each stretch between them, as rulec writes them).
const HELD_RULE: &str = "rule fees v1
description \"A claim that is charged a fee until it is closed\"

enum st = open | done
enum ev = charge | close

inputs
  state : st
  event : ev
  fee   : rate[step 0.1%]  range >=0% <=10%
  flat  : rate[step 1%]  range >=0% <=10%
  cap   : money[JPY]  range >=0JPY <=10000JPY
  items : number  range >=0 <=10

outputs
  moved   : st
  flagged : bool

table t
policy first
| state | event  | fee   | flat | cap     | items | -> moved | flagged |
| open  | charge | >0.5% | -    | -       | -     | open     | true    |
| open  | charge | -     | >5%  | -       | -     | open     | true    |
| open  | charge | -     | -    | >300JPY | -     | open     | true    |
| open  | charge | -     | -    | -       | >3    | open     | true    |
| open  | charge | -     | -    | -       | -     | open     | false   |
| open  | close  | -     | -    | -       | -     | done     | false   |
| done  | -      | -     | -    | -       | -     | state    | false   |

machine life over t
  carry   state -> moved
  held    fee, flat, cap, items
  initial open
  final   done
";

/// [`HELD_RULE`], with the names in Japanese.
const HELD_RULE_JA: &str = "rule 手数料(fees) v1
description \"締めるまで手数料を取る請求\"

enum 状態(st) = 受付(open) | 済み(done)
enum 出来事(ev) = 課金(charge) | 締め(close)

inputs
  状態(state)   : 状態
  出来事(event) : 出来事
  手数料(fee)   : rate[step 0.1%]  range >=0% <=10%
  一律(flat)    : rate[step 1%]  range >=0% <=10%
  上限(cap)     : money[円]  range >=0円 <=10000円
  件数(items)   : number  range >=0 <=10

outputs
  移った(moved) : 状態
  目印(flagged) : bool

table 表(t)
policy first
| 状態 | 出来事 | 手数料 | 一律 | 上限   | 件数 | -> 移った | 目印  |
| 受付 | 課金   | >0.5%  | -    | -      | -    | 受付      | true  |
| 受付 | 課金   | -      | >5%  | -      | -    | 受付      | true  |
| 受付 | 課金   | -      | -    | >300円 | -    | 受付      | true  |
| 受付 | 課金   | -      | -    | -      | >3   | 受付      | true  |
| 受付 | 課金   | -      | -    | -      | -    | 受付      | false |
| 受付 | 締め   | -      | -    | -      | -    | 済み      | false |
| 済み | -      | -      | -    | -      | -    | 状態      | false |

machine 流れ(life) over 表
  carry   状態 -> 移った
  held    手数料, 一律, 上限, 件数
  initial 受付
  final   済み
";

/// A case of [`HELD_RULE`]'s machine, which holds its four inputs; `{{written|whole}}` as in
/// [`forms`].
const HELD: &str = "workflow fees v1

use rule fees from \"fees.rule\"

record Claim
  id    : string
  state : fees.st

inputs
  id : string

task open_claim(id: string) -> Claim
  lambda \"arn:aws:lambda:eu-west-2:123456789012:function:open\"
  starts fees.life then charge
  key

task close_claim(id: string) -> Claim
  lambda \"arn:aws:lambda:eu-west-2:123456789012:function:close\"
  sends close
  idempotent

case c : Claim follows fees.life
  held fee = {{0.5%|5}}
  held flat = {{5%|5}}
  held cap = {{300JPY|300}}
  held items = 3

flow
  c <- open_claim(id: id)
  c <- close_claim(id: id)
    on failure => fail NotClosed \"the claim stays open\" leaving c
  succeed
";

/// [`HELD`], with the names in Japanese, of [`HELD_RULE_JA`].
const HELD_JA: &str = "workflow 請求の手数料 v1

use rule 手数料 from \"手数料.rule\"

record 請求
  番号 : string
  状態 : 手数料.状態

inputs
  番号 : string

task 開く(番号: string) -> 請求
  lambda \"arn:aws:lambda:ap-northeast-1:123456789012:function:open\"
  starts 手数料.流れ then 課金
  key

task 締める(番号: string) -> 請求
  lambda \"arn:aws:lambda:ap-northeast-1:123456789012:function:close\"
  sends 締め
  idempotent

case 件 : 請求 follows 手数料.流れ
  held 手数料 = {{0.5%|5}}
  held 一律 = {{5%|5}}
  held 上限 = {{300円|300}}
  held 件数 = 3

flow
  件 <- 開く(番号: 番号)
  件 <- 締める(番号: 番号)
    on failure => fail 締められない \"請求は開いたまま\" leaving 件
  succeed
";

/// The flows `srcs`, each checked as `held.flow` beside the rule `rule` (its file's name and text) in
/// one directory, with rulec's port, and the code each builds for every platform, with the `.flow`'s
/// hash put the same (as [`a_number_with_a_unit_builds_the_code_its_whole_number_builds`] does).
fn held_flows(rule: (&str, &str), srcs: &[&str]) -> Vec<(Checked, Vec<(&'static str, dandori::commands::Built)>)> {
    let dir = ritsu_testkit::TempDir::new("units-held");
    std::fs::write(dir.path().join(rule.0), rule.1).unwrap();
    dandori::sources::with_rules(Rc::new(rulec::ports::Engine::new()), || {
        srcs.iter()
            .map(|src| {
                let c = dandori::check::check_source(src, &dir.path().join("held.flow"));
                let built = match c.model.clone() {
                    Some(mut m) => {
                        m.source_sha256 = "0".repeat(64);
                        dandori::commands::TARGETS.iter().map(|t| (*t, dandori::commands::build(&m, t).expect("a target dandori builds for"))).collect()
                    }
                    None => Vec::new(),
                };
                (c, built)
            })
            .collect()
    })
}

/// A number `held` gives an input of a case's machine is a number of the input's type, as a value of
/// a call is: with its unit (`held fee = 0.5%`, `held cap = 300JPY`) or as the whole number it counts
/// to (`5`, `300`), it is the same value of the machine's table, the flow lowers alike and builds the
/// same code on every platform, and `api` gives it as written. Of the whole numbers, W106 says the
/// one on the rate in steps of 0.1% alone; 0 there is 0% and says nothing.
#[test]
fn a_number_a_case_holds_counts_in_the_unit_of_its_input() {
    let w106 = [
        (
            ("fees.rule", HELD_RULE),
            HELD,
            [("fee", "0.5%"), ("flat", "5%"), ("cap", "300JPY"), ("items", "3")],
            [("fee", "5"), ("flat", "5"), ("cap", "300"), ("items", "3")],
            ("`5`, written for `fee`, counts steps of 0.1%: it is 0.5%", "Write it `held fee = 0.5%`, so that a reader sees the rate; the case holds the same value"),
        ),
        (
            ("手数料.rule", HELD_RULE_JA),
            HELD_JA,
            [("手数料", "0.5%"), ("一律", "5%"), ("上限", "300円"), ("件数", "3")],
            [("手数料", "5"), ("一律", "5"), ("上限", "300"), ("件数", "3")],
            ("`手数料` に書いた `5` は、0.1% の刻みの数として読みます。0.5% です", "読む人に率が分かるよう、`held 手数料 = 0.5%` と書いてください。案件が持つ値は変わりません"),
        ),
    ];
    for (rule, t, as_written, as_whole, (said_w106, note)) in w106 {
        let (unit, whole) = forms(t);
        let zero = whole.replace("= 5\n", "= 0\n");
        assert_ne!(zero, whole);
        let [(cu, bu), (cw, bw), (cz, _)]: [_; 3] = held_flows(rule, &[&unit, &whole, &zero]).try_into().unwrap_or_else(|_| unreachable!());
        let mu = cu.model.as_ref().unwrap_or_else(|| panic!("{}: {:?}", rule.0, said(&cu)));
        let mw = cw.model.as_ref().unwrap_or_else(|| panic!("{}: {:?}", rule.0, said(&cw)));
        assert!(cu.diags.is_empty(), "{}: {:?}", rule.0, said(&cu));
        let got: Vec<_> = cw.diags.iter().map(|d| (d.code, d.line, d.col, d.en.as_str(), d.ja.as_str(), d.notes.iter().map(|n| (n.en.as_str(), n.ja.as_str())).collect::<Vec<_>>())).collect();
        let [(code, line, col, en, ja, notes)] = &got[..] else { panic!("{}: {got:?}", rule.0) };
        assert_eq!((*code, *line, *col), ("W106", 23, 14), "{}", rule.0);
        assert!(*en == said_w106 || *ja == said_w106, "{}: {en} / {ja}", rule.0);
        assert!(notes.iter().any(|(e, j)| *e == note || *j == note), "{}: {notes:?}", rule.0);
        assert!(cz.model.is_some() && cz.diags.is_empty(), "{}: {:?}", rule.0, said(&cz));
        // the same values of the table
        assert_eq!(mu.cases[0].held.len(), 4, "{}", rule.0);
        assert_eq!(mu.cases[0].held, mw.cases[0].held, "{}", rule.0);
        let pairs = |v: &[(String, String)]| v.iter().map(|(i, x)| (i.clone(), x.clone())).collect::<Vec<_>>();
        let want = |v: [(&str, &str); 4]| v.map(|(i, x)| (i.to_string(), x.to_string())).to_vec();
        assert_eq!(pairs(&mu.cases[0].held_values), want(as_written), "{}", rule.0);
        assert_eq!(pairs(&mw.cases[0].held_values), want(as_whole), "{}", rule.0);
        let api = dandori::api::json(mu);
        let held = api.get("cases").and_then(|c| c.as_arr()).and_then(|c| c.first()).and_then(|c| c.get("held")).cloned().unwrap();
        let held: Vec<_> = held.as_arr().unwrap().iter().map(|h| (h.get("input").and_then(|v| v.as_str()).unwrap().to_string(), h.get("value").and_then(|v| v.as_str()).unwrap().to_string())).collect();
        assert_eq!(held, want(as_written), "{}", rule.0);
        // the same flow, and the same code
        assert_eq!(format!("{:?}", mu.flow), format!("{:?}", mw.flow), "{}", rule.0);
        assert_eq!(bu.len(), 7, "{}", rule.0);
        for ((target, u), (_, w)) in bu.iter().zip(&bw) {
            let (Ok(u), Ok(w)) = (u, w) else { panic!("{}: {target}: {u:?} / {w:?}", rule.0) };
            assert!(!u.is_empty(), "{}: {target} writes nothing", rule.0);
            assert_eq!(u.iter().map(|(p, _)| p).collect::<Vec<_>>(), w.iter().map(|(p, _)| p).collect::<Vec<_>>(), "{}: {target}", rule.0);
            for ((path, a), (_, b)) in u.iter().zip(w) {
                assert!(a == b, "{}: {target}: {path} differs", rule.0);
            }
        }
    }
}

/// A number `held` gives an input is any value of the set a coordinate of the machine's table stands
/// for, not only the coordinate itself, the ends of the set as well: the port carries each
/// coordinate's ends (ritsu's DESIGN 3.2). The table cuts `fee` at 0.5% and the range ends at 0% and
/// 10%, so `0.1%` stands for 0.1% to 0.4%, and `0.3%`, `0.4%` and the whole number `3` (W106, as
/// anywhere) are the same value of the table as `0.1%`, lower to the same flow and build the same
/// code; `0%` and `0.5%`, just outside the set, are values of their own. `api` gives each as written.
#[test]
fn a_number_a_case_holds_is_any_value_of_the_set_a_coordinate_stands_for() {
    let w106 = |lang: &str, n: &str, rate: &str| match lang {
        "en" => format!("`{n}`, written for `fee`, counts steps of 0.1%: it is {rate}"),
        _ => format!("`手数料` に書いた `{n}` は、0.1% の刻みの数として読みます。{rate} です"),
    };
    // the line that holds an input, the coordinate, the values of its set, and the values just outside
    let sets: [(&str, &str, &[&str], &[&str]); 4] = [
        ("held fee = 0.5%", "0.1%", &["0.3%", "0.4%", "3"], &["0%", "0.5%"]),
        ("held fee = 0.5%", "0.6%", &["0.7%", "9.9%", "99"], &["0.5%", "10%"]),
        ("held cap = 300JPY", "301JPY", &["301JPY", "5000JPY", "9999JPY"], &["300JPY", "10000JPY"]),
        ("held items = 3", "4", &["5", "9"], &["3", "10"]),
    ];
    let (en_flow, _) = forms(HELD);
    let (ja_flow, _) = forms(HELD_JA);
    let ja_line = |l: &str| l.replace("held fee", "held 手数料").replace("held cap", "held 上限").replace("held items", "held 件数").replace("JPY", "円");
    for (line, coord, inside, outside) in sets {
        let at = |v: &str| line.split_once(" = ").map(|(h, _)| format!("{h} = {v}")).unwrap();
        for (rule, flow, lang) in [(("fees.rule", HELD_RULE), en_flow.clone(), "en"), (("手数料.rule", HELD_RULE_JA), ja_flow.clone(), "ja")] {
            let write = |v: &str| if lang == "en" { flow.replace(line, &at(v)) } else { flow.replace(&ja_line(line), &ja_line(&at(v))) };
            let values: Vec<&str> = [coord].into_iter().chain(inside.iter().copied()).chain(outside.iter().copied()).collect();
            let all: Vec<String> = values.iter().map(|&v| write(v)).collect();
            for (&v, f) in values.iter().zip(&all) {
                assert!(*f != flow || at(v) == line, "{lang}: `{}` is not in the flow", at(v));
            }
            let srcs: Vec<&str> = all.iter().map(String::as_str).collect();
            let got = held_flows(rule, &srcs);
            let model = |i: usize| got[i].0.model.as_ref().unwrap_or_else(|| panic!("{lang} {}: {:?}", all[i], said(&got[i].0)));
            let (base, base_code) = (model(0), &got[0].1);
            assert!(got[0].0.diags.is_empty(), "{lang} {coord}: {:?}", said(&got[0].0));
            for (k, &v) in inside.iter().enumerate() {
                let i = 1 + k;
                let m = model(i);
                // a whole number on the rate in steps of 0.1% is W106, as anywhere, and nothing else
                let want: Vec<String> = if v.chars().all(|c| c.is_ascii_digit()) && line.contains("fee") { vec![w106(lang, v, &format!("{}%", v.parse::<f64>().unwrap() / 10.0))] } else { vec![] };
                assert_eq!(got[i].0.diags.iter().map(|d| if lang == "en" { d.en.clone() } else { d.ja.clone() }).collect::<Vec<_>>(), want, "{lang} {v}");
                assert_eq!(m.cases[0].held, base.cases[0].held, "{lang} {v} is the table's {coord}");
                assert_eq!(format!("{:?}", m.flow), format!("{:?}", base.flow), "{lang} {v}");
                for ((target, a), (_, b)) in got[i].1.iter().zip(base_code) {
                    let (Ok(a), Ok(b)) = (a, b) else { panic!("{lang} {v}: {target}") };
                    assert!(a.iter().map(|(p, _)| p).eq(b.iter().map(|(p, _)| p)) && a.iter().zip(b).all(|((_, x), (_, y))| x == y), "{lang} {v}: {target} differs");
                }
                let written = m.cases[0].held_values.iter().find(|(i, _)| line.contains(&format!("held {i} ")) || ja_line(line).contains(&format!("held {i} "))).map(|(_, x)| x.clone());
                let shown = if lang == "en" { v.to_string() } else { ja_line(v) };
                assert_eq!(written.as_deref(), Some(shown.as_str()), "{lang} {v}");
            }
            for (k, &v) in outside.iter().enumerate() {
                let i = 1 + inside.len() + k;
                let m = model(i);
                assert!(got[i].0.diags.is_empty(), "{lang} {v}: {:?}", said(&got[i].0));
                assert_ne!(m.cases[0].held, base.cases[0].held, "{lang} {v} is no value of {coord}'s set");
            }
        }
    }
}

/// A machine that holds two counts, each bounded on one side only: `low` by `<=10`, `high` by
/// `>=0`, both cut at 3.
const ONE_SIDED_RULE: &str = "rule limits v1
description \"A claim whose counts are bounded on one side\"

enum st = waiting | done
enum ev = add | close

inputs
  state : st
  event : ev
  low   : number  range <=10
  high  : number  range >=0

outputs
  moved   : st
  flagged : bool

table t
policy first
| state   | event | low | high | -> moved | flagged |
| waiting | add   | >3  | -    | waiting  | true    |
| waiting | add   | -   | >3   | waiting  | true    |
| waiting | add   | -   | -    | waiting  | false   |
| waiting | close | -   | -    | done     | false   |
| done    | -     | -   | -    | state    | false   |

machine life over t
  carry   state -> moved
  held    low, high
  initial waiting
  final   done
";

/// [`ONE_SIDED_RULE`], with the names in Japanese.
const ONE_SIDED_RULE_JA: &str = "rule 限度(limits) v1
description \"片側だけに限りのある数を持つ請求\"

enum 状態(st) = 待ち(waiting) | 済み(done)
enum 出来事(ev) = 追加(add) | 締め(close)

inputs
  状態(state)   : 状態
  出来事(event) : 出来事
  残り(low)     : number  range <=10
  回数(high)    : number  range >=0

outputs
  移った(moved) : 状態
  目印(flagged) : bool

table 表(t)
policy first
| 状態 | 出来事 | 残り | 回数 | -> 移った | 目印  |
| 待ち | 追加   | >3   | -    | 待ち      | true  |
| 待ち | 追加   | -    | >3   | 待ち      | true  |
| 待ち | 追加   | -    | -    | 待ち      | false |
| 待ち | 締め   | -    | -    | 済み      | false |
| 済み | -      | -    | -    | 状態      | false |

machine 流れ(life) over 表
  carry   状態 -> 移った
  held    残り, 回数
  initial 待ち
  final   済み
";

/// A case of [`ONE_SIDED_RULE`]'s machine, holding each count to a coordinate of its own.
const ONE_SIDED: &str = "workflow limits v1

use rule limits from \"limits.rule\"

record Claim
  id    : string
  state : limits.st

inputs
  id : string

task open_claim(id: string) -> Claim
  lambda \"arn:aws:lambda:eu-west-2:123456789012:function:open\"
  starts limits.life then add
  key

task close_claim(id: string) -> Claim
  lambda \"arn:aws:lambda:eu-west-2:123456789012:function:close\"
  sends close
  idempotent

case c : Claim follows limits.life
  held low = 2
  held high = 4

flow
  c <- open_claim(id: id)
  c <- close_claim(id: id)
    on failure => fail NotClosed \"the claim stays open\" leaving c
  succeed
";

/// [`ONE_SIDED`], with the names in Japanese, of [`ONE_SIDED_RULE_JA`].
const ONE_SIDED_JA: &str = "workflow 請求の限度 v1

use rule 限度 from \"限度.rule\"

record 請求
  番号 : string
  状態 : 限度.状態

inputs
  番号 : string

task 開く(番号: string) -> 請求
  lambda \"arn:aws:lambda:ap-northeast-1:123456789012:function:open\"
  starts 限度.流れ then 追加
  key

task 締める(番号: string) -> 請求
  lambda \"arn:aws:lambda:ap-northeast-1:123456789012:function:close\"
  sends 締め
  idempotent

case 件 : 請求 follows 限度.流れ
  held 残り = 2
  held 回数 = 4

flow
  件 <- 開く(番号: 番号)
  件 <- 締める(番号: 番号)
    on failure => fail 締められない \"請求は開いたまま\" leaving 件
  succeed
";

/// Where an input's range leaves a side open, the set at that side runs on without an end: `low`'s
/// `2` stands for every number up to 2, and `high`'s `4` for every number from 4, so `held low = 0`
/// and `held high = 1000000` are the same values of the table as `2` and `4`. Past the side the
/// range closes, E003 says the one end there is.
#[test]
fn a_number_a_case_holds_where_the_range_is_open_on_one_side() {
    let cases = [
        (("limits.rule", ONE_SIDED_RULE), ONE_SIDED, ("held low = ", "held high = "), "`11` is not a value of `low` (its values are 10 or less)"),
        (("限度.rule", ONE_SIDED_RULE_JA), ONE_SIDED_JA, ("held 残り = ", "held 回数 = "), "`11` は `残り` の値ではありません（値は 10 以下）"),
    ];
    for (rule, flow, (low, high), e003) in cases {
        let srcs = [flow.to_string(), flow.replace(&format!("{low}2\n"), &format!("{low}0\n")), flow.replace(&format!("{high}4\n"), &format!("{high}1000000\n")), flow.replace(&format!("{low}2\n"), &format!("{low}11\n"))];
        assert!(srcs[1..].iter().all(|s| s != flow), "{}", rule.0);
        let got = held_flows(rule, &srcs.iter().map(String::as_str).collect::<Vec<_>>());
        let held = |i: usize| got[i].0.model.as_ref().map(|m| m.cases[0].held.clone()).unwrap_or_else(|| panic!("{} {i}: {:?}", rule.0, said(&got[i].0)));
        for i in 0..3 {
            assert!(got[i].0.diags.is_empty(), "{} {i}: {:?}", rule.0, said(&got[i].0));
            assert_eq!(held(i), held(0), "{}: {}", rule.0, srcs[i]);
        }
        let errs: Vec<(&str, String)> = got[3].0.diags.iter().map(|d| (d.code, if rule.0 == "limits.rule" { d.en.clone() } else { d.ja.clone() })).collect();
        assert_eq!(errs, [("E003", e003.to_string())], "{}", rule.0);
    }
}

/// A number `held` gives an input that is no value of it, E003, as at a call: a unit of another
/// dimension or currency, one that does not count to a whole number of the input's unit, and a unit on
/// a count; and a number in none of the sets the machine's table tells the input apart by, past an end
/// of the input's range, which the error says the values run between (a whole number there is W106 as
/// well).
#[test]
fn a_number_a_case_holds_that_is_no_value_of_its_input_is_e003() {
    let table = "(its values run from 0% to 10%)";
    let table_ja = "（値は 0% から 10% まで）";
    let cases: [(&str, &str, &str, Vec<(&str, String, String)>); 9] = [
        ("held fee = 0.5%", "held fee = 0.5kg", "held 手数料 = 0.5kg", vec![("E003", "`0.5kg` is not a value of `rate[step 0.1%]`".into(), "`0.5kg` は `rate[step 0.1%]` の値ではありません".into())]),
        (
            "held fee = 0.5%",
            "held fee = 0.55%",
            "held 手数料 = 0.55%",
            vec![("E003", "`0.55%`, counted in `rate[step 0.1%]`, is not a whole number".into(), "`0.55%` を `rate[step 0.1%]` で数えると、整数になりません".into())],
        ),
        ("held cap = 300JPY", "held cap = 10USD", "held 上限 = 10USD", vec![("E003", "`10USD` is not a value of `money[JPY]`".into(), "`10USD` は `money[円]` の値ではありません".into())]),
        ("held items = 3", "held items = 3%", "held 件数 = 3%", vec![("E003", "`3%` is not a value of `int`".into(), "`3%` は `int` の値ではありません".into())]),
        ("held fee = 0.5%", "held fee = 10.1%", "held 手数料 = 10.1%", vec![("E003", format!("`10.1%` is not a value of `fee` {table}"), format!("`10.1%` は `手数料` の値ではありません{table_ja}"))]),
        ("held fee = 0.5%", "held fee = 12%", "held 手数料 = 12%", vec![("E003", format!("`12%` is not a value of `fee` {table}"), format!("`12%` は `手数料` の値ではありません{table_ja}"))]),
        (
            "held fee = 0.5%",
            "held fee = 101",
            "held 手数料 = 101",
            vec![
                ("E003", format!("`101` is not a value of `fee` {table}"), format!("`101` は `手数料` の値ではありません{table_ja}")),
                ("W106", "`101`, written for `fee`, counts steps of 0.1%: it is 10.1%".into(), "`手数料` に書いた `101` は、0.1% の刻みの数として読みます。10.1% です".into()),
            ],
        ),
        (
            "held cap = 300JPY",
            "held cap = 10001JPY",
            "held 上限 = 10001円",
            vec![("E003", "`10001JPY` is not a value of `cap` (its values run from 0JPY to 10000JPY)".into(), "`10001円` は `上限` の値ではありません（値は 0円 から 10000円 まで）".into())],
        ),
        ("held items = 3", "held items = 11", "held 件数 = 11", vec![("E003", "`11` is not a value of `items` (its values run from 0 to 10)".into(), "`11` は `件数` の値ではありません（値は 0 から 10 まで）".into())]),
    ];
    let (en_flow, _) = forms(HELD);
    let (ja_flow, _) = forms(HELD_JA);
    let ja_line = |l: &str| l.replace("held fee", "held 手数料").replace("held cap", "held 上限").replace("held items", "held 件数").replace("300JPY", "300円");
    for (from, to, to_ja, want) in cases {
        let en = en_flow.replace(from, to);
        let ja = ja_flow.replace(&ja_line(from), to_ja);
        assert_ne!(en, en_flow, "{to}");
        assert_ne!(ja, ja_flow, "{to_ja}");
        let [(c, _)]: [_; 1] = held_flows(("fees.rule", HELD_RULE), &[&en]).try_into().unwrap_or_else(|_| unreachable!());
        assert_eq!(c.diags.iter().map(|d| (d.code, d.en.clone())).collect::<Vec<_>>(), want.iter().map(|(code, en, _)| (*code, en.clone())).collect::<Vec<_>>(), "{to}");
        let [(c, _)]: [_; 1] = held_flows(("手数料.rule", HELD_RULE_JA), &[&ja]).try_into().unwrap_or_else(|_| unreachable!());
        assert_eq!(c.diags.iter().map(|d| (d.code, d.ja.clone())).collect::<Vec<_>>(), want.iter().map(|(code, _, ja)| (*code, ja.clone())).collect::<Vec<_>>(), "{to_ja}");
    }
}
