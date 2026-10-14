//! A rule applied by another (§15.69, §15.66): the callee is read beside
//! the rule, held to its pinned digest, checked on its own, and expanded into the rule under
//! the apply's name with the bound inputs substituted. What is pinned here is the shape of the
//! expansion the rest of the tool sees, and the flow from a changed callee to a new pin.

// Each test reads rulec's Japanese wording, so it sets the language first: rulec has one
// language for the whole process (src/i18n.rs), and English is the default.

use rulec::ast::Item;
use std::path::PathBuf;
use std::process::Command;
use ritsu_testkit::{Need, TempDir, need};
use ritsu_testkit::tmp::tmpdir_in;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const CALLER: &str = "tests/corpus/非常勤退職手当.rule";
const CALLEE: &str = "tests/corpus/退職手当.rule";

fn prepared(rel: &str) -> (rulec::ast::RuleFile, rulec::types::Checked) {
    let src = std::fs::read_to_string(root().join(rel)).unwrap();
    rulec::prepare(&src, &root().join(rel).to_string_lossy()).unwrap_or_else(|ds| panic!("{rel} は検査を通らない: {ds:?}"))
}

#[test]
fn 呼び先の定義は呼び出しの名前の下に展開される() {
    rulec::i18n::set(rulec::i18n::Lang::Ja);
    let (f, c) = prepared(CALLER);
    let tables: Vec<String> = f
        .items
        .iter()
        .filter_map(|it| if let Item::Table(t) = it { t.name.as_ref().map(|n| n.text.clone()) } else { None })
        .collect();
    // 支給表 and the main clause come in; the excepted 減額 does not.
    assert_eq!(tables, vec!["退職手当:支給表", "退職手当:本則"]);
    for it in &f.items {
        if let Item::Table(t) = it {
            assert_eq!(t.applied.as_deref(), Some("退職手当"), "展開した表は呼び出しの名前を持つ");
        }
    }
    // The bound inputs are this rule's names; the callee's own define is renamed.
    let schedule = f.items.iter().find_map(|it| match it {
        Item::Table(t) if t.name.as_ref().is_some_and(|n| n.text == "退職手当:支給表") => Some(t),
        _ => None,
    }).unwrap();
    let cols: Vec<&str> = schedule.inputs.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(cols, vec!["在職期間", "任期終了事由"]);
    let defines: Vec<String> = f.items.iter().filter_map(|it| if let Item::Define(d) = it { Some(d.name.text.clone()) } else { None }).collect();
    assert!(defines.contains(&"退職手当:満額".to_string()), "{defines:?}");
    assert!(!defines.iter().any(|d| d == "退職手当:減額後"), "除いた節しか読まない定義は消える: {defines:?}");
    assert!(defines.contains(&"非常勤手当".to_string()), "呼び先の出力はこの規則の定義になる: {defines:?}");
    // The enum cells are spelled in this rule's values, and a row naming only a value that
    // stands for nothing here (死亡) is gone.
    assert_eq!(schedule.rows.len(), 3);
    assert!(c.ty_of("退職手当:支給月数").is_some());
    // The apply remembers what it learned.
    let a = &f.applies[0];
    assert_eq!(a.meaning.as_deref(), Some("6812e6cdf8088ad4"));
    // 支給表, 満額, 本則 and the definition of 非常勤手当 (the callee has no `result`, so
    // its output is the clause's column itself).
    assert_eq!(a.count, 4);
    assert_eq!(a.defines, vec!["非常勤手当"]);
    assert!(a.callee.as_ref().is_some_and(|k| k.rule.text == "退職手当"));
}

#[test]
fn 呼び先の行は呼び先のとおりに当たり_trace_は呼び出しの名前で言う() {
    rulec::i18n::set(rulec::i18n::Lang::Ja);
    let (f, c) = prepared(CALLER);
    let mut inputs = std::collections::HashMap::new();
    inputs.insert("在職期間".to_string(), rulec::eval::Val::Num(rulec::num::Rat::int(3)));
    inputs.insert("任期終了事由".to_string(), rulec::eval::Val::Enum("辞職".into()));
    inputs.insert("報酬月額".to_string(), rulec::eval::Val::Num(rulec::num::Rat::int(300000)));
    let (outs, fired, _, _) = rulec::eval::run_all_traced(&f, &c, inputs);
    assert_eq!(outs[0].1, Some(rulec::eval::Val::Num(rulec::num::Rat::int(1500000))), "減額は準用しないので満額");
    assert_eq!(fired, vec!["表 退職手当:支給表 行1", "表 退職手当:本則 行1"]);
    // The label travels with the row, under the table's name in this rule.
    assert_eq!(c.label_of("退職手当:支給表", 1), Some("短期"));
}

/// The pin of an apply is the callee's meaning (§15.230): a pin that is not the callee's stops at
/// E040, whose fix is the heading pinned at the meaning; `rulec source pin` writes it, keeping the
/// comment, from a `sha256:` pin too. A comment added to the callee moves nothing; a row does.
#[test]
fn 固定と違う呼び先は_e040_で止まり_pin_が見出しを書き換える() {
    rulec::i18n::set(rulec::i18n::Lang::Ja);
    let tmp = TempDir::new("apply-pin");
    let dir = tmp.path().to_path_buf();
    let callee = std::fs::read_to_string(root().join(CALLEE)).unwrap();
    std::fs::write(dir.join("退職手当.rule"), &callee).unwrap();
    let m = rulec::link::pin::meaning(&callee);
    let caller0 = std::fs::read_to_string(root().join(CALLER)).unwrap();
    let pinned = caller0.lines().find(|l| l.starts_with("apply ")).and_then(|l| l.split_whitespace().find(|w| w.starts_with("meaning:") || w.starts_with("sha256:"))).unwrap().to_string();
    let caller = caller0.replace(&pinned, "sha256:0000000000000000  # 固定");
    let path = dir.join("非常勤退職手当.rule");
    std::fs::write(&path, &caller).unwrap();
    let p = path.to_string_lossy().into_owned();
    let ds = rulec::report(&caller, &p).diags;
    let e040: Vec<_> = ds.iter().filter(|d| d.code == "E040").collect();
    assert_eq!(e040.len(), 1, "{ds:?}");
    assert_eq!(e040[0].fix.text.as_deref(), Some(format!("apply 退職手当(retirement) = \"退職手当.rule\" meaning:{m}").as_str()));
    // The check goes on past E040, so the rest of the rule is still judged.
    assert!(ds.iter().all(|d| d.code == "E040" || d.severity != rulec::diag::Severity::Error), "{ds:?}");
    // gen refuses; diff may go on.
    assert!(rulec::prepare(&caller, &p).is_err());
    assert!(rulec::prepare_lenient(&caller, &p).is_ok());
    // pin rewrites the heading's pin as the meaning and keeps the comment.
    let parsed = rulec::parse::parse(&caller, &p);
    let (text, o) = rulec::sources::pin(parsed.file.as_ref().unwrap(), &p, &caller).unwrap();
    assert!(o.changed);
    assert!(text.contains(&format!("meaning:{m}  # 固定")), "{text}");
    assert!(rulec::report(&text, &p).diags.iter().all(|d| d.code != "E040"));
    // A comment in the callee moves no meaning; a row does.
    std::fs::write(dir.join("退職手当.rule"), format!("# 改正の前の版\n{callee}")).unwrap();
    assert!(rulec::report(&text, &p).diags.iter().all(|d| d.code != "E040"), "コメントは意味を動かさない");
    let row = "長期 | >=25     | 定年, 自己都合 | 40                           |";
    assert!(callee.contains(row));
    std::fs::write(dir.join("退職手当.rule"), callee.replace(row, "長期 | >=25     | 定年, 自己都合 | 41                           |")).unwrap();
    let ds = rulec::report(&text, &p).diags;
    assert_eq!(ds.iter().filter(|d| d.severity == rulec::diag::Severity::Error).map(|d| d.code).collect::<Vec<_>>(), ["E040"], "行を変えると意味が動く");
}

#[test]
fn 呼び先が無ければ_e044_で止まる() {
    rulec::i18n::set(rulec::i18n::Lang::Ja);
    let src = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=1 <=3\n\noutputs\n  y(y) : number  round down(1)\n\napply 呼(c) = \"無い.rule\" sha256:0000000000000000\n  a = n\n  x -> y\n";
    let ds = rulec::report(src, "t.rule").diags;
    assert_eq!(ds.iter().map(|d| d.code).collect::<Vec<_>>(), vec!["E044"]);
}

/// A callee whose table produces one of its own enums is applied (§15.230): the enum comes in as
/// `<apply>:<enum>`, a clause of this rule tests a value of it through `<apply>:<name>`, and an
/// output of it is read in this rule's values with `with`, from the callee's value to this rule's.
/// The English rules, and their Japanese versions.
#[test]
fn 呼び先の列挙を出す表も準用でき_出力は_with_でこの規則の値に読む() {
    let callee_en = "rule grade v1\n\nenum level = low | high\n\ninputs\n  score : number  range >=0 <=100\n\noutputs\n  band : level\n\ntable level_of\npolicy unique\n| score | -> band |\n| <50   | low     |\n| >=50  | high    |\n";
    let caller_en = "rule review v1\n\nenum verdict_kind = needs_work | fine\n\ninputs\n  points : number  range >=0 <=100\n\noutputs\n  verdict : verdict_kind\n  bonus   : number  round down(1)\n\napply grade = \"grade.rule\" meaning:{m}\n  score = points\n  band -> verdict with low -> needs_work, high -> fine\n\nclause high_bonus -> bonus\n  when grade:band high\n  then 10\n\nclause low_bonus -> bonus\n  when grade:band low\n  then 0\n\nexamples\n| points | -> verdict | bonus |\n| 40     | needs_work | 0     |\n| 70     | fine       | 10    |\n";
    let callee_ja = "rule 評価(grade) v1\n\nenum 水準(level) = 低い(low) | 高い(high)\n\ninputs\n  点数(score) : number  range >=0 <=100\n\noutputs\n  段(band) : 水準\n\ntable 水準の表(level_of)\npolicy unique\n| 点数 | -> 段 |\n| <50  | 低い  |\n| >=50 | 高い  |\n";
    let caller_ja = "rule 審査(review) v1\n\nenum 判定(verdict_kind) = 要指導(needs_work) | 良(fine)\n\ninputs\n  得点(points) : number  range >=0 <=100\n\noutputs\n  判定結果(verdict) : 判定\n  加点(bonus)       : number  round down(1)\n\napply 評価(grade) = \"評価.rule\" meaning:{m}\n  点数 = 得点\n  段 -> 判定結果 with 低い -> 要指導, 高い -> 良\n\nclause 高い加点(high_bonus) -> 加点\n  when 評価:段 高い\n  then 10\n\nclause 低い加点(low_bonus) -> 加点\n  when 評価:段 低い\n  then 0\n\nexamples\n| 得点 | -> 判定結果 | 加点 |\n| 40   | 要指導      | 0    |\n| 70   | 良          | 10   |\n";
    let tmp = TempDir::new("apply-enum");
    let dir = tmp.path().to_path_buf();
    for (callee, callee_name, caller, name, lang) in [
        (callee_en, "grade.rule", caller_en, "review.rule", rulec::i18n::Lang::En),
        (callee_ja, "評価.rule", caller_ja, "審査.rule", rulec::i18n::Lang::Ja),
    ] {
        std::fs::write(dir.join(callee_name), callee).unwrap();
        let caller = caller.replace("{m}", &rulec::link::pin::meaning(callee));
        let path = dir.join(name);
        std::fs::write(&path, &caller).unwrap();
        let p = path.to_string_lossy().into_owned();
        let ds = rulec::i18n::with(lang, || rulec::report(&caller, &p).diags);
        let errors: Vec<_> = ds.iter().filter(|d| d.severity == rulec::diag::Severity::Error).map(|d| (d.code, d.title.clone())).collect();
        assert!(errors.is_empty(), "{name}: {errors:?}");
        // The examples ran: a score of 40 is read as this rule's needs_work, and the clause that
        // tests the callee's low gives no bonus.
        let (f, c) = rulec::prepare(&caller, &p).unwrap();
        let points = if lang == rulec::i18n::Lang::En { "points" } else { "得点" };
        let mut inputs = std::collections::HashMap::new();
        inputs.insert(points.to_string(), rulec::eval::Val::Num(rulec::num::Rat::int(70)));
        let (outs, _, _) = rulec::eval::run_all(&f, &c, inputs);
        let want = if lang == rulec::i18n::Lang::En { "fine" } else { "良" };
        assert_eq!(outs[0].1, Some(rulec::eval::Val::Enum(want.into())), "{name}");
        assert_eq!(outs[1].1, Some(rulec::eval::Val::Num(rulec::num::Rat::int(10))), "{name}");
    }
}

/// A callee may apply a rule in turn (§15.230): it is expanded as its own check expands it, what it
/// brought in comes in under both names, and the answer is the one each level gives. A rule may also
/// bind a callee's input to a field of the elements it walks, which applies the callee to each
/// element.
#[test]
fn 準用の準用と_要素のフィールドで読む準用() {
    rulec::i18n::set(rulec::i18n::Lang::Ja);
    let tmp = TempDir::new("apply-levels");
    let dir = tmp.path().to_path_buf();
    let c = "rule unit v1\n\ninputs\n  x : number  range >=0 <=10\n\noutputs\n  y : number  round down(1)\n\ntable step_of\npolicy unique\n| x  | -> y |\n| <5 | 1    |\n| >=5 | 2   |\n";
    std::fs::write(dir.join("unit.rule"), c).unwrap();
    let b = format!("rule tens v1\n\ninputs\n  n : number  range >=0 <=10\n\noutputs\n  z : number  round down(1)\n\napply inner = \"unit.rule\" meaning:{}\n  x = n\n  y -> base\n\ndefine z : number = base * 10\n", rulec::link::pin::meaning(c));
    std::fs::write(dir.join("tens.rule"), &b).unwrap();
    let a = format!("rule hundreds v1\n\ninputs\n  k : number  range >=0 <=10\n\noutputs\n  w : number  round down(1)\n\napply outer = \"tens.rule\" meaning:{}\n  n = k\n  z -> w\n\nexamples\n| k | -> w |\n| 3 | 10   |\n| 7 | 20   |\n", rulec::link::pin::meaning(&b));
    let path = dir.join("hundreds.rule");
    std::fs::write(&path, &a).unwrap();
    let p = path.to_string_lossy().into_owned();
    let ds = rulec::report(&a, &p).diags;
    assert!(ds.iter().all(|d| d.severity != rulec::diag::Severity::Error), "{:?}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>());
    let (f, _) = rulec::prepare(&a, &p).unwrap();
    let tables: Vec<String> = f.items.iter().filter_map(|it| if let Item::Table(t) = it { t.name.as_ref().map(|n| n.text.clone()) } else { None }).collect();
    assert_eq!(tables, ["outer:inner:step_of"], "the callee's callee comes in under both names");
    // A field of the elements this rule walks, bound to the callee's input: the callee answers for
    // each parcel, and the sum adds the answers up.
    let fee = "rule parcel_fee v1\n\ninputs\n  kg : number  range >=0 <=50\n\noutputs\n  fee : money[USD]  round down(1USD)\n\ntable fee_of\npolicy unique\n| kg   | -> fee : money[USD] |\n| <=10 | 5USD                |\n| >10  | 12USD               |\n";
    std::fs::write(dir.join("parcel_fee.rule"), fee).unwrap();
    let order = format!("rule shipment v1\n\nelements parcels\n  weight : number  range >=0 <=50\n\noutputs\n  charge : money[USD]  round down(1USD)\n\napply per = \"parcel_fee.rule\" meaning:{}\n  kg = weight\n  fee -> parcel_fee\n\nsum parcels_total over parcels of parcel_fee  range >=0USD <=1000USD\n\ndefine charge : money[USD] = parcels_total\n\nsequence two\n| weight |\n| 5      |\n| 20     |\n\nexamples\n| parcels | -> charge |\n| two     | 17USD     |\n", rulec::link::pin::meaning(fee));
    let path = dir.join("shipment.rule");
    std::fs::write(&path, &order).unwrap();
    let p = path.to_string_lossy().into_owned();
    let ds = rulec::report(&order, &p).diags;
    assert!(ds.iter().all(|d| d.severity != rulec::diag::Severity::Error), "{:?}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>());
}

#[test]
fn 整形は呼び出しの本体を二字下げる() {
    rulec::i18n::set(rulec::i18n::Lang::Ja);
    let src = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=1 <=3\n\noutputs\n  y(y) : number  round down(1)\n\napply 呼(c) = \"呼び先.rule\" sha256:0000000000000000\n    a   =  n\nexcept 表\n  x  ->   y\n";
    let out = rulec::fmt::format(src);
    assert!(out.contains("\n  a = n\n  except 表\n  x -> y\n"), "{out}");
}

/// A callee whose table column carries the output's name — the usual shape of a one-table
/// rule — applied twice under unaliased `->` names: two versions of a rule bundled, with a
/// date table choosing between them (§15.71). The expansion used to give the renamed output
/// the callee's own prefixed alias, the very identifier the callee's output column gets, so
/// the generated TypeScript, JavaScript, Go and Swift declared one name twice. The callee
/// rounds `half_up`, whose Python helper is `_round_half`: the SQL translator read that name
/// as a mode and stopped `gen` for every language. `上乗せ` reads a rounded callee output
/// in arithmetic, which needs that output's range, and adds a definition that is one bare
/// literal, which Go and Swift typed as a plain integer.
#[test]
fn 呼び先の出力列が出力と同名でも_付け替えた名前の識別子は衝突しない() {
    rulec::i18n::set(rulec::i18n::Lang::Ja);
    let tmp = TempDir::new("bundle");
    let d = tmp.path().to_path_buf();
    // 旧: a table whose output column carries the output's name, rounded `half_up` (the
    // Python helper `_round_half`). 新: a rate arithmetic held in 1/20000 円, rounded
    // `half_down` — a callee output that has to come back to whole 円 before this rule
    // reads it, through a helper the four compiled languages did not know by that name.
    let old = "rule 旧税額(old_tax) v1\n\ninputs\n  金額(amount) : money[円]  range >=0円 <=100万円\n\noutputs\n  税額(tax) : money[円]  round half_up(1円)\n\ntable 税額表(rates)\npolicy unique\n| 金額     | -> 税額(tax) : money[円] |\n| <=10万円 | 200円 |\n| >10万円  | 400円 |\n".to_string();
    let new = "rule 新税額(new_tax) v1\n\ninputs\n  金額(amount) : money[円]  range >=0円 <=100万円\n  率(rate)     : rate[step 0.01%]  range >=0% <=20%\n\noutputs\n  税額(tax) : money[円]  round half_down(1円)\n\ndefine 税額(tax) : money[円] = 金額 × 率 ÷ 2\n".to_string();
    std::fs::write(d.join("旧税額.rule"), &old).unwrap();
    std::fs::write(d.join("新税額.rule"), &new).unwrap();
    let bundle = format!(
        "rule 束(bundle) v1\n\ninputs\n  金額(amount) : money[円]  range >=0円 <=100万円\n  作成日(made) : date       range >=2024-04-01 <=2030-12-31\n\noutputs\n  税額(tax) : money[円]  round down(1円)\n  上乗せ(extra) : money[円]  round half_up(1円)\n\napply 旧(old) = \"旧税額.rule\" sha256:{}\n  金額 = 金額\n  税額 -> 旧税額\n\napply 新(new) = \"新税額.rule\" sha256:{}\n  金額 = 金額\n  率 = 10.28%\n  税額 -> 新税額\n\ndefine 定数(fixed) : money[円] = 100円\ndefine 上乗せ(extra) : money[円] = 新税額 × 3 ÷ 2 + 定数\n\ntable 選択(pick)\npolicy unique\n| 作成日       | -> 税額 |\n| <=2027-03-31 | 旧税額  |\n| >=2027-04-01 | 新税額  |\n\nexamples\n| 金額    | 作成日     | -> 税額 | 上乗せ |\n| 5万円   | 2027-03-31 | 200円   | 3955円 |\n| 5万円   | 2027-04-01 | 2570円  | 3955円 |\n| 30001円 | 2027-04-01 | 1542円  | 2413円 |\n",
        rulec::sha256::short(old.as_bytes()),
        rulec::sha256::short(new.as_bytes())
    );
    let path = d.join("束.rule");
    std::fs::write(&path, &bundle).unwrap();
    let (f, _) = rulec::prepare(&bundle, &path.to_string_lossy()).unwrap_or_else(|ds| panic!("{ds:?}"));
    // Every identifier the generated code writes for an item is distinct.
    let ident = |n: &rulec::ast::Name| n.ascii.clone().unwrap_or_else(|| n.text.clone());
    let mut idents: Vec<String> = Vec::new();
    for it in &f.items {
        match it {
            Item::Define(x) => idents.push(ident(&x.name)),
            Item::Derived(x) => idents.push(ident(&x.name)),
            Item::Agg(x) => idents.push(ident(&x.name)),
            Item::Table(t) => idents.extend(t.outputs.iter().map(|o| ident(&o.name))),
            // A fold's answer named after `->` is a value the code writes (§15.219).
            Item::Fold(x) => idents.extend(x.answer.iter().map(|a| ident(&a.name))),
        }
    }
    let mut distinct = idents.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), idents.len(), "識別子が重なっている: {idents:?}");
    assert!(idents.contains(&"旧税額".to_string()), "別名の無い付け替え先は名前そのものが識別子になる: {idents:?}");
    // And the generated code answers like the evaluator wherever a toolchain is installed.
    if !need(Need::Python) {
        return;
    }
    let generated = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "ja").current_dir(&d).args(["gen", "束.rule", "--out", "out"]).output().unwrap();
    assert!(generated.status.success(), "{}", String::from_utf8_lossy(&generated.stderr));
    let test = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "ja").env("TMPDIR", tmpdir_in(&d.join("out"))).current_dir(&d).args(["test", "out"]).output().unwrap();
    let text = String::from_utf8_lossy(&test.stdout).into_owned() + &String::from_utf8_lossy(&test.stderr);
    assert!(test.status.success() && !text.contains("FAIL"), "{text}");
}

/// A callee with the shapes the corpus's one applied rule does not have.
///
/// `apply.rs` sat at 71.9% of its lines with the corpus alone (§15.90), and the gap was one
/// shape: the callee it applies has no `derive`, no `constraint`, no `not` cell, no date or
/// boolean input, and every binding is a name. Each of those is a branch of the expansion
/// that had never run — and binding a callee's input to a **literal** turned out not to work
/// at all, though both the checker's type table and the expansion's partial evaluation had
/// an arm for it.
const PAIR: (&str, &str) = ("tests/apply_fixtures/適用.rule", "tests/apply_fixtures/規程.rule");

#[test]
fn リテラルで束縛した入力は列ごと畳まれる() {
    rulec::i18n::set(rulec::i18n::Lang::Ja);
    let (f, _) = prepared(PAIR.0);
    // 基準日 is bound to a date and 特例 to `false`, so neither is a column of the expanded
    // table any more: the rows that could not match them are gone, and the rest lost the
    // column. `not 前二つ | - | true` cannot hold with 特例 = false, so it is dropped.
    let judge = f
        .items
        .iter()
        .find_map(|it| match it {
            Item::Table(t) if t.name.as_ref().is_some_and(|n| n.text == "規程:判定") => Some(t),
            _ => None,
        })
        .expect("規程:判定 が展開されていない");
    let cols: Vec<&str> = judge.inputs.iter().map(|(n, _)| n.as_str()).collect();
    assert!(!cols.contains(&"基準日"), "日付で束縛した列が残っている: {cols:?}");
    assert!(!cols.contains(&"特例"), "真偽で束縛した列が残っている: {cols:?}");
    assert!(cols.contains(&"型"), "読み替えた列が名前で入っていない: {cols:?}");
    // Four rows in the callee; the one that needs 特例 = true cannot happen here.
    assert_eq!(judge.rows.len(), 3, "定数で消える行が消えていない");

    // The callee's `derive` and its `constraint` come in under the apply's name.
    assert!(
        f.items.iter().any(|it| matches!(it, Item::Derived(d) if d.name.text == "規程:幅")),
        "呼び先の derive が展開されていない"
    );
    assert!(
        f.constraints.iter().any(|k| k.names().any(|n| n == "小")),
        "呼び先の constraint が呼び出し側の名前で入っていない: {:?}",
        f.constraints.iter().map(|k| (&k.left, &k.right)).collect::<Vec<_>>()
    );
}

/// A word that names nothing and is no value of the callee's type is still E041: the reading
/// as a literal must not swallow a misspelled name.
#[test]
fn 名前でもリテラルでもない語はE041のまま() {
    rulec::i18n::set(rulec::i18n::Lang::Ja);
    let src = std::fs::read_to_string(root().join(PAIR.0)).unwrap();
    let broken = src.replace("  特例 = false", "  特例 = 幽霊");
    let ds = rulec::check_source(&broken, &root().join(PAIR.0).to_string_lossy());
    let codes: Vec<&str> = ds.iter().map(|d| d.code).collect();
    assert!(codes.contains(&"E041"), "打ち間違いが E041 で止まらない: {codes:?}");
}

#[test]
fn 呼び先の形が揃った規則も生成物は評価器と全言語で一致する() {
    rulec::i18n::set(rulec::i18n::Lang::Ja);
    if !need(Need::Python) {
        return;
    }
    let tmp = TempDir::new("apply-run");
    let dir = tmp.path().to_path_buf();
    let out = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "ja")
        .current_dir(root())
        .args(["gen", PAIR.0, "--out", &dir.to_string_lossy()])
        .output()
        .expect("rulec を起動できない");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    let out = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "ja")
        .env("TMPDIR", tmpdir_in(&dir))
        .current_dir(root())
        .args(["test", &dir.to_string_lossy()])
        .output()
        .expect("rulec を起動できない");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(!text.lines().any(|l| l.starts_with("FAIL")), "一致しない言語がある:\n{text}");
    assert!(out.status.success(), "{text}");
}
