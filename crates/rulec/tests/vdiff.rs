//! The difference between two versions over the whole input space (DESIGN §15.122).
//!
//! `rulec diff` with no `--fixtures` answers a question no log can: **which inputs get a
//! different answer**, and whether there are any outside the region it names. Three things
//! are tested here, and the third is the one that matters.
//!
//! 1. A rule compared with itself reports no difference, and earns the claim that there is
//!    none outside — on every rule of the corpus it can settle.
//! 2. A planted change is found, and found where it is.
//! 3. **The two answers agree.** The same pair of versions is run against the vectors as
//!    records, and every record whose answer moved has to fall inside the region. A region
//!    that missed one would be the worst failure this command has: it would say "outside
//!    this, they answer alike" about a case that does not.

use std::path::{Path, PathBuf};
use std::process::Command;
use ritsu_testkit::TempDir;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The root of ritsu's workspace, two directories above this crate: rulec's site is there, at
/// website/rulec.
fn repo() -> PathBuf {
    root().parent().and_then(Path::parent).expect("the crate is in crates/ of the workspace").to_path_buf()
}

fn rulec(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "ja").current_dir(root()).args(args).args(if args.contains(&"--lang") { &[][..] } else { &["--lang", "ja"][..] }).output().expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

fn corpus() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(root().join("tests/corpus"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "rule"))
        .collect();
    v.sort();
    v
}

fn json(src: &str) -> rulec::json::Json {
    rulec::json::parse(src).unwrap_or_else(|e| panic!("json が読めない: {e}\n{src}"))
}

fn int(j: &rulec::json::Json, k: &str) -> i128 {
    j.get(k).and_then(|x| x.as_int()).unwrap_or_else(|| panic!("{k} が無い"))
}

fn arr<'a>(j: &'a rulec::json::Json, k: &str) -> &'a [rulec::json::Json] {
    match j.get(k) {
        Some(rulec::json::Json::Arr(v)) => v,
        _ => &[],
    }
}

fn is_true(j: &rulec::json::Json, k: &str) -> bool {
    matches!(j.get(k), Some(rulec::json::Json::Bool(true)))
}

/// A rule is the same as itself, everywhere. Nothing that decides an answer changed, so
/// this is settled without walking the space at all — and that short cut has to be right,
/// because everything else leans on it.
#[test]
fn 規則は自分自身と同じ() {
    for p in corpus() {
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        let s = p.to_string_lossy().to_string();
        let (code, out) = rulec(&["diff", &s, &s, "--format", "json"]);
        let j = json(&out);
        assert!(j.get("blocked").and_then(|x| x.as_str()).is_none(), "{name}: 自分自身と比べられない\n{out}");
        assert_eq!(int(&j, "differing"), 0, "{name}: 自分自身と比べて差が出た\n{out}");
        assert!(is_true(&j, "total"), "{name}: 自分自身と比べて外について主張できない\n{out}");
        assert_eq!(code, 0, "{name}: 差が無いのに exit が 0 でない");
    }
}

/// The region names where the change is, and the claim outside it holds.
#[test]
fn 一つの金額を動かすと_その行の届く範囲だけが出る() {
    let src = std::fs::read_to_string(root().join("tests/corpus/送料.rule")).unwrap();
    let tmp = TempDir::new("vdiff-送料");
    let dir = tmp.path().to_path_buf();
    let (a, b) = (dir.join("a.rule"), dir.join("b.rule"));
    std::fs::write(&a, &src).unwrap();
    // 遠隔地・2000g 超えの基本送料だけを上げる。examples はこの行に当たらない。
    let mutated = src.replace("| 遠隔地      | >2000g  | 1800円", "| 遠隔地      | >2000g  | 2000円");
    assert_ne!(mutated, src, "狙った行が見つからない");
    std::fs::write(&b, &mutated).unwrap();

    let (code, out) = rulec(&["diff", &a.to_string_lossy(), &b.to_string_lossy(), "--format", "json"]);
    assert_eq!(code, 1, "差があるのに exit が 1 でない\n{out}");
    let j = json(&out);
    assert!(is_true(&j, "total"), "外について主張できていない\n{out}");
    assert_eq!(arr(&j, "unknown").len(), 0);

    let texts: Vec<&str> = arr(&j, "changes").iter().filter_map(|c| c.get("text").and_then(|x| x.as_str())).collect();
    assert_eq!(texts.len(), 2, "会員で二つに割れるはず: {texts:?}");
    for t in &texts {
        assert!(t.contains("届け先"), "{t}");
        assert!(t.contains("重量"), "{t}");
        // 大口（注文金額 3万円以上）は負担率 0% なので、上げた金額は消える。記録を一件も
        // 使わずにこれが出ることが、この命令の言い分そのものである。
        assert!(t.contains("注文金額"), "×0 で消える側が領域から外れていない: {t}");
    }
    let total: i128 = arr(&j, "changes").iter().map(|c| int(c, "cells")).sum();
    assert_eq!(total, 24, "答えが違う入力の数が変わった");
}

/// A rule that folds a sequence is not a function of finitely many columns: the answer
/// depends on the whole sequence. When nothing inside the walk moved, that is still
/// settled; when something did, the command says it cannot state a region rather than
/// stating one over the summaries that would not mean what it looks like it means.
#[test]
fn 並びを畳む規則は_歩く側が変わったら比べられないと言う() {
    let src = std::fs::read_to_string(root().join("tests/corpus/全国運賃.rule")).unwrap();
    let tmp = TempDir::new("vdiff-fold");
    let dir = tmp.path().to_path_buf();
    let (a, b) = (dir.join("a.rule"), dir.join("b.rule"));
    std::fs::write(&a, &src).unwrap();
    let (as_, bs) = (a.to_string_lossy().to_string(), b.to_string_lossy().to_string());
    for (from, to) in [
        // 畳み方そのもの（打ち切ったときの答え）
        ("打ち切り  -> stop with 0円", "打ち切り  -> stop with 100円"),
        // 一件ぶんの表（どの要素がどう扱われるか）
        ("| 近畿圏   | >1000円  | 持ち越し                    |", "| 近畿圏   | >1000円  | スキップ                    |"),
    ] {
        let mutated = src.replace(from, to);
        assert_ne!(mutated, src, "狙った箇所が見つからない: {from}");
        std::fs::write(&b, &mutated).unwrap();
        if rulec(&["check", &bs]).0 != 0 {
            continue;
        }
        let (code, out) = rulec(&["diff", &as_, &bs, "--format", "json"]);
        let j = json(&out);
        assert!(j.get("blocked").and_then(|x| x.as_str()).is_some(), "歩く側が変わったのに黙って答えた\n{out}");
        assert!(!is_true(&j, "total"), "比べられないのに外について主張している");
        assert_eq!(code, 0, "領域を出していないので影響ありとは言わない");
        return;
    }
    panic!("畳む側を動かした版が一つも check を通らない");
}

/// The walk, over every rule of the corpus, with a change actually planted in it. Three
/// things are held at once, and the first is the one that matters.
///
/// 1. **Every record whose answer moved falls inside the region.** The same pair of
///    versions is run against the vectors as records; a region that missed one would be
///    the worst failure this command has — it would say "outside this, they answer alike"
///    about a case that does not.
/// 2. **Every witness the region reports really differs.** Each one is handed back as a
///    single record, and the record-shaped answer has to call it a mismatch.
/// 3. **The rules the walk cannot settle stay the ones that are named.** They are the ones
///    whose derived columns share an input, which is the blind spot W114 already has.
#[test]
fn 領域と記録の二つの答えが食い違わない() {
    // Where the walk gives up, and why. `預け荷物料金` has one cell whose answer is not
    // constant on it — the free allowance is per passenger — where the two versions agree
    // at every point tried and that could not be turned into a proof over the whole cell.
    // `クーポン割引` was here too: two derived columns share an input, the blind spot of
    // the sieve (§6.2), and the elimination that sees through it gave up on the whole
    // question because a rate column sat beside the money ones (§15.139).
    // The twin (tests/corpus/twins.tsv) is the same rule, so it says no more about the outside than the original does.
    let 決められない: &[&str] = &["預け荷物料金", "checked_bag_fee"];
    let (mut checked, mut withheld) = (0, Vec::new());
    for p in corpus() {
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        let src = std::fs::read_to_string(&p).unwrap();
        let tmp = TempDir::new(&format!("vdiff-{name}"));
        let dir = tmp.path().to_path_buf();
        let (a, b) = (dir.join("a.rule"), dir.join("b.rule"));
        std::fs::write(&a, &src).unwrap();
        let (as_, bs) = (a.to_string_lossy().to_string(), b.to_string_lossy().to_string());
        // 動かした金額が examples と合わなくなる規則があるので、通る行が出るまで試す。
        let mut found = false;
        for mutated in bump_each_amount(&src) {
            std::fs::write(&b, &mutated).unwrap();
            if rulec(&["check", &bs]).0 == 0 {
                found = true;
                break;
            }
        }
        if !found {
            continue;
        }

        let (_, region) = rulec(&["diff", &as_, &bs, "--format", "json"]);
        let j = json(&region);
        if j.get("blocked").and_then(|x| x.as_str()).is_some() || is_true(&j, "over_budget") {
            continue;
        }
        if int(&j, "cells") <= 1 {
            // 動かした金額が表の出力セルではなかった（`sequence` の行など）。値としては
            // 何も変わっていないので、組み合わせを当たらずに「同じ」と言うのが正しい。
            assert_eq!(int(&j, "differing"), 0, "{name}: 何も変えていないのに差が出た\n{region}");
            continue;
        }

        // (2) 出した証人を一件の記録として突き返す。動かないなら、動くと言ったのが誤り。
        for ch in arr(&j, "changes") {
            let ins: Vec<String> = arr(ch, "witness")
                .iter()
                .filter_map(|w| {
                    let (k, v) = (w.get("input")?.as_str()?, w.get("value")?.as_str()?);
                    let scalar = v.parse::<i128>().is_ok() || v == "true" || v == "false";
                    Some(format!("{}:{}", rulec::json::quote(k), if scalar { v.to_string() } else { rulec::json::quote(v) }))
                })
                .collect();
            let outs: Vec<String> = arr(ch, "outputs")
                .iter()
                .filter_map(|o| {
                    let (k, v) = (o.get("output")?.as_str()?, o.get("old")?.as_str()?);
                    let scalar = v.parse::<i128>().is_ok() || v == "true" || v == "false";
                    Some(format!("{}:{}", rulec::json::quote(k), if scalar { v.to_string() } else { rulec::json::quote(v) }))
                })
                .collect();
            if ins.is_empty() || outs.len() != arr(ch, "outputs").len() {
                continue; // 並びを渡す規則など、一行の記録に書けないもの
            }
            let one = format!("{{\"in\":{{{}}},\"observed\":{{{}}}}}\n", ins.join(","), outs.join(","));
            let f = dir.join("one.jsonl");
            std::fs::write(&f, &one).unwrap();
            let (_, r) = rulec(&["diff", &as_, &bs, "--fixtures", &f.to_string_lossy(), "--format", "json"]);
            let rj = json(&r);
            assert_eq!(int(&rj, "compared"), 1, "{name}: 証人が記録として読めない\n{one}{r}");
            assert_eq!(int(&rj, "matched"), 0, "{name}: 違うと言った証人が、記録では同じ答えだった\n{one}{r}");
        }

        // (1) ベクタを記録にして、同じ二版をもう一度比べる。
        let mut lines = String::new();
        for r in [&as_, &bs] {
            for ln in rulec(&["vectors", r]).1.lines() {
                let v = json(ln);
                let (Some(i), Some(o)) = (v.get("in"), v.get("out")) else { continue };
                lines.push_str(&format!(
                    "{{\"in\":{},\"observed\":{}}}\n",
                    rulec::json::unparse(i),
                    rulec::json::unparse(o)
                ));
            }
        }
        let f = dir.join("f.jsonl");
        std::fs::write(&f, &lines).unwrap();
        let (_, rec) = rulec(&["diff", &as_, &bs, "--fixtures", &f.to_string_lossy(), "--format", "json"]);
        let rj = json(&rec);
        let moved = int(&rj, "compared") - int(&rj, "matched");
        let cells: i128 = arr(&j, "changes").iter().map(|c| int(c, "cells")).sum();
        // 記録が動いたのに「外でも同じ」と言うのが、一番危ない壊れ方である。領域が空でも、
        // 主張を取り下げていれば嘘はついていない——そこが取り違えられないように分ける。
        if moved > 0 {
            assert!(
                cells > 0 || !is_true(&j, "total"),
                "{name}: 記録は {moved} 件動いたのに、領域は空で外についても主張している。\n領域:\n{region}\n記録:\n{rec}"
            );
        }
        if !is_true(&j, "total") {
            withheld.push(name);
        }
        checked += 1;
    }
    // 金額を一つ動かしても `examples` が生きているコーパスの本数。これを下回ったら、
    // 突き合わせがコーパスを覆わなくなっている。
    assert!(checked >= 12, "突き合わせた規則が {checked} 本しかない");
    withheld.sort();
    let mut want: Vec<String> = 決められない.iter().map(|s| (*s).to_string()).collect();
    want.sort();
    assert_eq!(withheld, want, "外について主張できない規則の顔ぶれが変わった");
}

/// The space is small enough to walk. The budget is there for the rule that is not, and it
/// says so instead of answering about nothing. What changed in the declarations is known without
/// the walk, and is said over the budget too: an output's range that moved (§15.271).
#[test]
fn 予算を超えたら_領域を出さずにそう言う() {
    let src = std::fs::read_to_string(root().join("tests/corpus/送料.rule")).unwrap();
    let tmp = TempDir::new("vdiff-budget");
    let dir = tmp.path().to_path_buf();
    let (a, b) = (dir.join("a.rule"), dir.join("b.rule"));
    std::fs::write(&a, &src).unwrap();
    // 遠隔地のほかの 2kg 超を 1100円 から 1300円 に。送料の上限の 1800円 は動かないので、
    // 出力の範囲は変わらない
    let raised = src.replace("| not: 遠隔地 | >2000g  | 1100円", "| not: 遠隔地 | >2000g  | 1300円");
    assert_ne!(raised, src);
    std::fs::write(&b, &raised).unwrap();
    let (as_, bs) = (a.to_string_lossy().to_string(), b.to_string_lossy().to_string());
    let (code, out) = rulec(&["diff", &as_, &bs, "--budget", "10", "--format", "json"]);
    let j = json(&out);
    assert!(is_true(&j, "over_budget"), "{out}");
    assert!(!is_true(&j, "total"), "予算を超えたのに外について主張している");
    assert!(arr(&j, "changes").is_empty(), "領域を出さないと言いながら出している");
    assert!(arr(&j, "domain").is_empty(), "変わっていない宣言を出している: {out}");
    assert_eq!(code, 0, "何も出していないので影響ありとは言わない");
    let (_, text) = rulec(&["diff", &as_, &bs, "--budget", "10"]);
    assert!(text.contains("budget") || text.contains("予算"), "{text}");

    // 遠隔地の 2kg 超を 1800円 から 2000円 にすると、送料の範囲が 0..1800 から 0..2000 に動く。
    // 歩かなくても分かることなので、予算を超えても宣言の変化として出し、影響ありと言う
    std::fs::write(&b, src.replace("| 遠隔地      | >2000g  | 1800円", "| 遠隔地      | >2000g  | 2000円")).unwrap();
    let (code, out) = rulec(&["diff", &as_, &bs, "--budget", "10", "--format", "json"]);
    let j = json(&out);
    assert!(is_true(&j, "over_budget"), "{out}");
    assert!(arr(&j, "changes").is_empty(), "領域を出さないと言いながら出している");
    let moved: Vec<(&str, &str, &str)> = arr(&j, "domain")
        .iter()
        .map(|d| (d.get("what").and_then(|x| x.as_str()).unwrap_or(""), d.get("old").and_then(|x| x.as_str()).unwrap_or(""), d.get("new").and_then(|x| x.as_str()).unwrap_or("")))
        .collect();
    assert_eq!(moved, [("output_range", "0..1800", "0..2000")], "{out}");
    assert_eq!(code, 1, "出力の範囲が動いたのに影響なしと言っている");
}

/// What the rule accepts is a different question from what it answers, and the two are not
/// mixed. A version that takes one more value of an enum has a wider door, not a different
/// answer, and the report says which.
#[test]
fn 受け付ける入力が変わったことは_答えの差とは別に出る() {
    let src = std::fs::read_to_string(root().join("tests/corpus/送料.rule")).unwrap();
    let tmp = TempDir::new("vdiff-domain");
    let dir = tmp.path().to_path_buf();
    let (a, b) = (dir.join("a.rule"), dir.join("b.rule"));
    std::fs::write(&a, &src).unwrap();
    let widened = src.replace("range >=1g <=40kg", "range >=1g <=60kg");
    assert_ne!(widened, src);
    std::fs::write(&b, &widened).unwrap();
    let (as_, bs) = (a.to_string_lossy().to_string(), b.to_string_lossy().to_string());
    let (_, out) = rulec(&["diff", &as_, &bs, "--format", "json"]);
    let j = json(&out);
    let kinds: Vec<&str> = arr(&j, "domain").iter().filter_map(|d| d.get("what").and_then(|x| x.as_str())).collect();
    assert!(kinds.contains(&"input_range"), "範囲が広がったことが出ていない: {out}");
    // 広がったぶんは旧版が受け付けない入力なので、軸は両方が受け付ける範囲に絞る。絞らずに
    // 比べると、旧版が入口で受け付けない入力について「同じ答えだ」と言ってしまう。
    assert!(arr(&j, "changes").is_empty(), "受け付ける範囲の違いを答えの差に混ぜている: {out}");
    let (_, text) = rulec(&["diff", &as_, &bs]);
    assert!(
        text.contains("両方") || text.contains("both"),
        "主張が「両方の版が受け付ける入力では」になっていない:\n{text}"
    );

    // 入力そのものが増えたら、同じ入力を両方に渡せないので比べられないと言う。
    let more = src.replace("  会員(member)    : 会員区分", "  会員(member)    : 会員区分\n  荷姿(shape)     : bool  contract_only");
    assert_ne!(more, src);
    std::fs::write(&b, &more).unwrap();
    if rulec(&["check", &bs]).0 == 0 {
        let (_, out) = rulec(&["diff", &as_, &bs, "--format", "json"]);
        let j = json(&out);
        assert!(j.get("blocked").and_then(|x| x.as_str()).is_some(), "入力が増えたのに比べたと言っている: {out}");
        assert!(!is_true(&j, "total"));
        let kinds: Vec<&str> = arr(&j, "domain").iter().filter_map(|d| d.get("what").and_then(|x| x.as_str())).collect();
        assert!(kinds.contains(&"input_added"), "増えた入力が出ていない: {out}");
    }
}

/// The JSON keys are the contract (docs/formats.md), so they are fixed here.
#[test]
fn json_の鍵は約束である() {
    let p = root().join("tests/corpus/送料.rule");
    let s = p.to_string_lossy().to_string();
    let (_, out) = rulec(&["diff", &s, &s, "--format", "json"]);
    let j = json(&out);
    let want = [
        "rule", "old", "new", "old_version", "new_version", "over_budget", "total", "cells", "feasible", "same",
        "differing", "unsettled", "unrealized", "domain", "changes", "unknown",
    ];
    let have: Vec<String> = rulec::json::members(&j).expect("object").into_keys().map(str::to_string).collect();
    for k in want {
        assert!(have.contains(&k.to_string()), "鍵 {k} が無い: {have:?}");
    }
}

/// Every version of the rule with the last amount of one table row bumped, one row at a
/// time. The caller takes the first that still passes `check`: a planted change that broke
/// the rule's own `examples` would be testing the wrong thing.
fn bump_each_amount(src: &str) -> Vec<String> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    for (i, ln) in lines.iter().enumerate() {
        let t = ln.trim();
        if !t.starts_with('|') || t.contains("->") {
            continue;
        }
        let Some(last) = t.trim_matches('|').split('|').next_back().map(|x| x.trim().to_string()) else { continue };
        let digits: String = last.chars().take_while(|c| c.is_ascii_digit() || *c == '_').collect();
        if digits.is_empty() {
            continue;
        }
        let unit = &last[digits.len()..];
        let Ok(n) = digits.replace('_', "").parse::<i128>() else { continue };
        let bumped = format!("{}{}", n + if n >= 100 { 100 } else { 7 }, unit);
        let mut cells: Vec<String> = ln.split('|').map(|x| x.to_string()).collect();
        let Some(k) = cells.iter().rposition(|c| c.trim() == last) else { continue };
        cells[k] = cells[k].replace(&last, &bumped);
        let mut all: Vec<String> = lines.iter().map(|x| (*x).to_string()).collect();
        all[i] = cells.join("|");
        out.push(all.join("\n"));
    }
    out
}

/// The example printed on the site is what the tool prints. Four copies of it — two pages
/// in two languages — and any of them going stale is exactly the kind of thing a reader
/// cannot tell from the page (§15.73). Each page is held to the output **in its own
/// language**: an English page showing Japanese output is its own kind of stale.
#[test]
fn 文書に載せた実演は_いまの出力と一致する() {
    let tmp = TempDir::new("vdiff-demo");
    let dir = tmp.path().to_path_buf();

    // Each language's pages show a rule written in it: the shipping fee on the Japanese pages,
    // and on the English ones `parcel_rate`, a tariff written in English whose example inputs
    // name no prefecture (its fuel rate for north_america goes from 5% to 6%).
    for (lang, pages, rule, old_name, new_name, old_cell, new_cell) in [
        (
            "ja",
            ["website/rulec/docs-ja/compare.md", "website/rulec/docs-ja/scenarios.md"],
            "tests/corpus/送料.rule",
            ("rule 送料(shipping_fee) v4", "rule 送料(shipping_fee) v3"),
            ("rule 送料(shipping_fee) v4", "rule 送料(shipping_fee) v4"),
            "| 遠隔地      | >2000g  | 1800円",
            "| 遠隔地      | >2000g  | 2000円",
        ),
        (
            "en",
            ["website/rulec/docs/compare.md", "website/rulec/docs/scenarios.md"],
            "tests/corpus/parcel_rate.rule",
            ("rule parcel_rate v1", "rule parcel_rate v3"),
            ("rule parcel_rate v1", "rule parcel_rate v4"),
            "| north_america | 5%                      |",
            "| north_america | 6%                      |",
        ),
    ] {
        let src = std::fs::read_to_string(root().join(rule)).unwrap();
        assert!(src.contains(old_name.0) && src.contains(old_cell), "{rule} has no `{}` or `{old_cell}` to change", old_name.0);
        let (a, b) = (dir.join(format!("{lang}-v3.rule")), dir.join(format!("{lang}-v4.rule")));
        std::fs::write(&a, src.replace(old_name.0, old_name.1)).unwrap();
        std::fs::write(&b, src.replace(new_name.0, new_name.1).replace(old_cell, new_cell)).unwrap();
        let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
            .current_dir(root())
            .env("RULEC_LANG", lang)
            .args(["diff", &a.to_string_lossy(), &b.to_string_lossy()])
            .output()
            .expect("rulec を起動できない");
        let got = String::from_utf8_lossy(&o.stdout).into_owned();
        // 一行目はファイルの名前を含むので飛ばす。文書は `送料@v3` と書いている。
        let body: Vec<&str> = got.lines().skip(1).collect();
        for doc in pages {
            let page = std::fs::read_to_string(repo().join(doc)).unwrap();
            for line in &body {
                assert!(
                    line.trim().is_empty() || page.contains(line),
                    "{doc} の実演がいまの出力と食い違う。無い行:\n{line}\n出力全体:\n{got}"
                );
            }
        }
    }
}

/// The flags that only mean something against records are refused here rather than
/// ignored, and the ones that mean something in both modes work in both.
#[test]
fn 意味の無い旗はエラーにし_意味のある旗は効く() {
    let p = root().join("tests/corpus/送料.rule");
    let s = p.to_string_lossy().to_string();
    for bad in [vec!["diff", &s, &s, "--fill", "重量=1"], vec!["diff", &s, &s, "--manifest", "m.json"]] {
        let (code, _) = rulec(&bad);
        assert_eq!(code, 2, "{bad:?} がエラーになっていない");
    }
    // --budget belongs to the other side of the same coin.
    let (code, _) = rulec(&["diff", &s, &s, "--fixtures", "/dev/null", "--budget", "10"]);
    assert_eq!(code, 2, "--budget が --fixtures と一緒に通ってしまう");

    // --terse keeps the example out of both renderings, in the mode that has one.
    let src = std::fs::read_to_string(&p).unwrap();
    let tmp = TempDir::new("vdiff-terse");
    let dir = tmp.path().to_path_buf();
    let (a, b) = (dir.join("a.rule"), dir.join("b.rule"));
    std::fs::write(&a, &src).unwrap();
    std::fs::write(&b, src.replace("| 遠隔地      | >2000g  | 1800円", "| 遠隔地      | >2000g  | 2000円")).unwrap();
    let (as_, bs) = (a.to_string_lossy().to_string(), b.to_string_lossy().to_string());
    let (_, full) = rulec(&["diff", &as_, &bs]);
    let (_, terse) = rulec(&["diff", &as_, &bs, "--terse"]);
    // `重量 >=2001g` は領域そのものなので、消えるのは「例」の行のほうである。
    assert!(full.contains("会員=一般"), "入力例が出ていない:\n{full}");
    assert!(!terse.contains("会員=一般"), "--terse なのに入力例が出ている:\n{terse}");
    assert!(terse.contains("重量 >=2001g"), "--terse で領域まで消えている:\n{terse}");
    let (_, md) = rulec(&["diff", &as_, &bs, "--format", "markdown", "--terse"]);
    assert!(md.starts_with("### "), "markdown の見出しが無い");
    assert!(!md.contains("会員=一般"), "--terse なのに markdown に入力例がある:\n{md}");
    let (_, md_full) = rulec(&["diff", &as_, &bs, "--format", "markdown"]);
    assert!(md_full.contains("会員=一般"), "markdown に入力例が無い:\n{md_full}");
}

/// The short cut — "nothing that decides an answer changed, so do not walk" — is the one
/// place this command can lie outright, and it did: its first version fingerprinted a row
/// by what it **answers**, so moving a threshold left every output cell where it was and
/// the command said "every input gets the same answer" about a rule that now charges a
/// different amount. Each thing that decides an answer without being written in an output
/// cell gets a case here.
#[test]
fn 答えを決めるものを変えたら_短絡してはいけない() {
    let src = std::fs::read_to_string(root().join("tests/corpus/送料.rule")).unwrap();
    let tmp = TempDir::new("vdiff-decides");
    let dir = tmp.path().to_path_buf();
    let (a, b) = (dir.join("a.rule"), dir.join("b.rule"));
    std::fs::write(&a, &src).unwrap();
    let (as_, bs) = (a.to_string_lossy().to_string(), b.to_string_lossy().to_string());

    for (what, from, to, expect) in [
        // 行の条件。出力セルは一つも動かない。
        (
            "閾値",
            vec![
                ("| 遠隔地      | <=2000g | 1200円", "| 遠隔地      | <=1500g | 1200円"),
                ("| 遠隔地      | >2000g  | 1800円", "| 遠隔地      | >1500g  | 1800円"),
            ],
            "重量 >=1501g <=2000g",
            "届け先 = 遠隔地",
        ),
        // 群。行にも出力にも現れないのに、行が何に当たるかを変える。
        (
            "群",
            vec![("group 遠隔地(remote) = 北海道, 沖縄県", "group 遠隔地(remote) = 北海道, 沖縄県, 青森県")],
            "青森県",
            "送料",
        ),
    ] {
        let mut mutated = src.clone();
        for (f, t) in &from {
            assert!(mutated.contains(f), "{what}: 狙った行が見つからない: {f}");
            mutated = mutated.replace(f, t);
        }
        std::fs::write(&b, &mutated).unwrap();
        assert_eq!(rulec(&["check", &bs]).0, 0, "{what}: 変えた版が check を通らない");

        let (code, out) = rulec(&["diff", &as_, &bs, "--format", "json"]);
        let j = json(&out);
        assert!(int(&j, "cells") > 1, "{what} を変えたのに短絡した\n{out}");
        assert_eq!(code, 1, "{what} を変えたのに影響なしと言っている\n{out}");
        assert!(int(&j, "differing") > 0, "{what} を変えたのに差が出ていない\n{out}");
        let texts: Vec<&str> =
            arr(&j, "changes").iter().filter_map(|c| c.get("text").and_then(|x| x.as_str())).collect();
        assert!(texts.iter().any(|t| t.contains(to)), "{what}: 領域に {to} が無い: {texts:?}");
        assert!(
            texts.iter().any(|t| t.contains(expect)) || out.contains(expect),
            "{what}: 領域に {expect} が無い: {texts:?}"
        );
    }
}

/// A rule that only looks different — the same tests written in another order — is the same
/// rule, and the short cut is allowed to say so without walking anything.
#[test]
fn 書き方だけ変えたら_短絡してよい() {
    let src = std::fs::read_to_string(root().join("tests/corpus/送料.rule")).unwrap();
    let tmp = TempDir::new("vdiff-same");
    let dir = tmp.path().to_path_buf();
    let (a, b) = (dir.join("a.rule"), dir.join("b.rule"));
    std::fs::write(&a, &src).unwrap();
    let (as_, bs) = (a.to_string_lossy().to_string(), b.to_string_lossy().to_string());
    // 群の中身の順だけ入れ替える。集合としては同じもの。
    let mutated = src.replace("group 遠隔地(remote) = 北海道, 沖縄県", "group 遠隔地(remote) = 沖縄県, 北海道");
    assert_ne!(mutated, src);
    std::fs::write(&b, &mutated).unwrap();
    if rulec(&["check", &bs]).0 != 0 {
        return;
    }
    let (code, out) = rulec(&["diff", &as_, &bs, "--format", "json"]);
    let j = json(&out);
    assert_eq!(int(&j, "cells"), 1, "順を入れ替えただけで空を当たっている\n{out}");
    assert_eq!(code, 0);
    assert!(is_true(&j, "total"));
}

/// A rule whose computed columns share an input: the walk gives each of them an axis of its
/// own, so cells that no caller can reach look like cells the walk owes an answer for, and a
/// point that only the **intersection** of two targets holds is one it cannot climb to.
///
/// Both are decided by eliminating variables (§15.129), and what they cost while they were
/// not is the headline claim: 38 of the cells of this pair came back unrealized, and
/// "outside the reported region the two answer alike" was withheld because of them.
#[test]
fn 入力を共有する導出の規則でも_外について言い切れる() {
    let src = std::fs::read_to_string(root().join("tests/corpus/クーポン併用.rule")).unwrap();
    let tmp = TempDir::new("vdiff-stack");
    let dir = tmp.path().to_path_buf();
    let old = dir.join("old.rule");
    let new = dir.join("new.rule");
    std::fs::write(&old, &src).unwrap();
    // One output moved, which is what makes there be anything to report at all.
    std::fs::write(&new, src.replace("| >=3980円 | 対象  ", "| >=3980円 | 対象外")).unwrap();
    let (code, out) =
        rulec(&["diff", &old.to_string_lossy(), &new.to_string_lossy(), "--format", "json"]);
    let j = json(&out);
    assert_eq!(int(&j, "unrealized"), 0, "実現できなかった升目が残っている\n{out}");
    assert!(is_true(&j, "total"), "外について言い切れていない\n{out}");
    assert!(int(&j, "differing") > 0, "差があるはず\n{out}");
    assert_eq!(code, 1, "差があるので exit は 1");
}

/// A product on one value is a matter of its divisors (§15.291). The new version pays a bonus of 5
/// rather than 1 where `qty × price` is exactly 80,779,853,376, which is 123,456 × 654,321 and which
/// no move of one input along a line lands on: that cell came back unrealized, and the one change
/// there is was not reported. The two cells beside it, with one input at 1,000,000, hold no input at
/// all, since 1,000,000 does not divide the value, and they withheld the claim about the rest.
#[test]
fn a_product_on_one_value_is_built_from_its_divisors_and_its_empty_neighbours_are_left_out() {
    let rule = |version: &str, bonus: &str| {
        format!(
            "rule t(t) {version}

inputs
  qty(qty)     : number  range >=1 <=1000000
  price(price) : number  range >=1 <=1000000

define raw_total(raw_total) : number = qty * price

outputs
  bonus(bonus) : number  round down(1)

table bonuses(bonuses)
policy first
| raw_total   | -> bonus |
| 80779853376 | {bonus}  |
| -           | 0        |
"
        )
    };
    let tmp = TempDir::new("vdiff-product");
    let (old, new) = (tmp.path().join("old.rule"), tmp.path().join("new.rule"));
    std::fs::write(&old, rule("v1", "1")).unwrap();
    std::fs::write(&new, rule("v2", "5")).unwrap();
    let (code, out) = rulec(&["diff", &old.to_string_lossy(), &new.to_string_lossy(), "--format", "json"]);
    let j = json(&out);
    assert_eq!(int(&j, "differing"), 1, "{out}");
    assert_eq!(int(&j, "unrealized"), 0, "{out}");
    assert!(is_true(&j, "total"), "{out}");
    assert_eq!(code, 1, "{out}");
}

/// A range put on an input of an ordered enum is a change of what the version takes, as a
/// number's range is (§15.251). The corpus's campaign stops at gold; a version of it whose
/// campaign takes every tier was said to take every input the other does, with "every input gets
/// the same answer", and the run passed. It says the range moved, by the values at its ends, and
/// compares only what both take. English first, then the Japanese version.
#[test]
fn a_range_put_on_an_enum_input_is_a_change_of_what_the_version_takes() {
    for (rule, range, name, wide, narrow) in [
        ("tests/corpus/member_tier_shipping.rule", "  range >=basic <=gold", "campaign", "basic..platinum", "basic..gold"),
        ("tests/corpus/会員段階の送料.rule", "  range >=一般 <=ゴールド", "キャンペーン", "一般..プラチナ", "一般..ゴールド"),
    ] {
        let src = std::fs::read_to_string(root().join(rule)).unwrap();
        assert!(src.contains(range), "{rule}");
        let tmp = TempDir::new("vdiff-enum-range");
        let (a, b) = (tmp.path().join("wide.rule"), tmp.path().join("narrow.rule"));
        std::fs::write(&a, src.replace(range, "")).unwrap();
        std::fs::write(&b, &src).unwrap();
        let (a, b) = (a.to_string_lossy().to_string(), b.to_string_lossy().to_string());
        for (from, to, old, new) in [(&a, &b, wide, narrow), (&b, &a, narrow, wide)] {
            let (c, out) = rulec(&["diff", from, to, "--format", "json"]);
            assert_eq!(c, 1, "{rule}: {out}");
            let j = json(&out);
            let moved: Vec<(&str, &str, &str, &str)> = arr(&j, "domain")
                .iter()
                .map(|d| {
                    let s = |k: &str| d.get(k).and_then(|x| x.as_str()).unwrap_or("");
                    (s("what"), s("name"), s("old"), s("new"))
                })
                .collect();
            assert_eq!(moved, [("input_range", name, old, new)], "{rule}: {out}");
            assert!(arr(&j, "changes").is_empty() && int(&j, "differing") == 0, "{rule}: {out}");
            let (_, text) = rulec(&["diff", from, to]);
            assert!(text.contains(&format!("{name}  {old} → {new}")) && text.contains("両方の版が受け付ける入力"), "{rule}:\n{text}");
        }
    }
}

/// A rule that walks one sequence and tests nothing in a table, in English and in Japanese: the
/// fees of the lots of a sale, added up.
const LOTS: [(&str, [&str; 8]); 2] = [
    (
        "rule lots v1\n\
         description \"What the lots of a sale add up to\"\n\n\
         elements lots\n\
         \x20 lot : number  range >=1 <=99\n\
         \x20 big : bool\n\
         \x20 fee : number  range >=0 <=100\n\n\
         sum total_fee over lots of fee  range >=0 <=1000\n\
         count bigs over lots where big  range >=0 <=20\n\n\
         outputs\n\
         \x20 total : number  round down(1)\n\n\
         define total : number = total_fee\n",
        ["lots", "lot", "fee", "total", "  lot : number  range >=1 <=99\n", "  fee : number  range >=0 <=100\n", "= total_fee\n", "bigs"],
    ),
    (
        "rule 出品のまとめ(lots) v1\n\
         description \"出品の手数料の合計\"\n\n\
         elements 出品(lots)\n\
         \x20 番号(lot) : number  range >=1 <=99\n\
         \x20 大型(big) : bool\n\
         \x20 手数料(fee) : number  range >=0 <=100\n\n\
         sum 手数料の合計(total_fee) over 出品 of 手数料  range >=0 <=1000\n\
         count 大型の数(bigs) over 出品 where 大型  range >=0 <=20\n\n\
         outputs\n\
         \x20 合計(total) : number  round down(1)\n\n\
         define 合計 : number = 手数料の合計\n",
        ["出品", "番号", "手数料", "合計", "  番号(lot) : number  range >=1 <=99\n", "  手数料(fee) : number  range >=0 <=100\n", "= 手数料の合計\n", "大型の数"],
    ),
];

/// What `domain` says of the two files, as (what, name, old, new).
fn domain_of(j: &rulec::json::Json) -> Vec<(String, String, String, String)> {
    arr(j, "domain")
        .iter()
        .map(|d| {
            let s = |k: &str| d.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            (s("what"), s("name"), s("old"), s("new"))
        })
        .collect()
}

/// A field of an element is held at the door as an input is, so a version that changes one takes
/// other sequences (§15.314): its range, its type, whether it is `unique`, and whether it is there.
/// `diff` said nothing of any of them, and called two versions with another range on a field "the
/// same answer for every input", passing. Each is said in `domain`, the field named with its
/// sequence as the records hold it (`lots[].fee`); the run does not pass; and two versions with
/// other fields are not compared point by point, as two with other inputs are not. What is
/// compared is the sequences both take: no element past the lower top of a range, and no value of
/// a `unique` field twice — a sequence that repeats one is refused by the version that holds it,
/// and was shown as an answer that moved (`total: 0 → -`). Every cell is built: the total at its
/// top beside every number of big lots takes elements of two kinds.
#[test]
fn a_change_to_a_field_of_an_element_is_a_change_of_what_the_version_takes() {
    for (src, [seq, lot, fee, _, lot_line, fee_line, _, _]) in LOTS {
        let tmp = TempDir::new("vdiff-fields");
        let write = |name: &str, text: String| {
            let p = tmp.path().join(name);
            std::fs::write(&p, text).unwrap();
            p.to_string_lossy().into_owned()
        };
        assert!(src.contains(lot_line) && src.contains(fee_line), "{seq}");
        let v1 = write("v1.rule", src.to_string());
        let narrow = write("narrow.rule", src.replace(fee_line, &fee_line.replace("<=100", "<=50")));
        let once = write("once.rule", src.replace(lot_line, &lot_line.replace('\n', "  unique\n")));
        let text_lot = write("text.rule", src.replace(lot_line, &lot_line.replace("number  range >=1 <=99", "string")));
        let without = write("without.rule", src.replace(lot_line, ""));
        let d = |what: &str, field: &str, old: &str, new: &str| (what.to_string(), format!("{seq}[].{field}"), old.to_string(), new.to_string());
        for (from, to, want, compared) in [
            (&v1, &narrow, vec![d("field_range", fee, "0..100", "0..50")], true),
            (&narrow, &v1, vec![d("field_range", fee, "0..50", "0..100")], true),
            (&v1, &once, vec![d("field_unique_added", lot, "", "")], true),
            (&once, &v1, vec![d("field_unique_removed", lot, "", "")], true),
            (&v1, &text_lot, vec![d("field_type", lot, "number", "string"), d("field_range", lot, "1..99", "")], true),
            (&v1, &without, vec![d("field_removed", lot, "", "")], false),
            (&without, &v1, vec![d("field_added", lot, "", "")], false),
        ] {
            let (c, out) = rulec(&["diff", from, to, "--format", "json"]);
            assert_eq!(c, 1, "{seq}: {from} → {to}: {out}");
            let j = json(&out);
            assert_eq!(domain_of(&j), want, "{seq}: {from} → {to}: {out}");
            assert!(arr(&j, "changes").is_empty() && int(&j, "differing") == 0, "{seq}: {out}");
            let (_, text) = rulec(&["diff", from, to]);
            if compared {
                assert!(j.get("blocked").is_none_or(|b| matches!(b, rulec::json::Json::Null)), "{seq}: {out}");
                assert_eq!((int(&j, "cells"), int(&j, "same"), int(&j, "unrealized")), (9, 9, 0), "{seq}: {from} → {to}: {out}");
                assert!(text.contains("両方の版が受け付ける入力では、どれも答えは同じです"), "{seq}:\n{text}");
            } else {
                assert!(j.get("blocked").and_then(|b| b.as_str()).is_some_and(|b| b.contains("二つの版が取る入力が違う")), "{seq}: {out}");
            }
            assert!(text.contains(&format!("{seq}[].")), "{seq}:\n{text}");
        }
    }
    // The gift basket of the corpus with the price of a line held to 100 to 40,000 yen: the range
    // is said, and of four records the old version takes, the two with a line outside it are
    // counted as ones the new version does not take, the field named the same way.
    for (rule, range, narrow, name) in [
        ("tests/corpus/gift_basket.rule", "range >=0JPY <=50_000JPY", "range >=100JPY <=40_000JPY", "lines[].price"),
        ("tests/corpus/贈り物のかご.rule", "range >=0円 <=50000円", "range >=100円 <=40000円", "明細[].価格"),
    ] {
        let src = std::fs::read_to_string(root().join(rule)).unwrap();
        assert_eq!(src.matches(range).count(), 1, "{rule}");
        let tmp = TempDir::new("vdiff-field-records");
        let (a, b, r) = (tmp.path().join("old.rule"), tmp.path().join("new.rule"), tmp.path().join("records.jsonl"));
        std::fs::write(&a, &src).unwrap();
        std::fs::write(&b, src.replace(range, narrow)).unwrap();
        let (seq, field) = name.split_once("[].").unwrap();
        let (wrapped, free, card) = if seq == "lines" { ("wrapped", "free_shipping", "card") } else { ("包装", "送料無料", "カード") };
        let record = |lines: &[(i64, bool)], free_v: bool, card_v: bool| {
            let ls: Vec<String> = lines.iter().map(|(p, w)| format!(r#"{{"{field}":{p},"{wrapped}":{w}}}"#)).collect();
            format!(r#"{{"in":{{"{seq}":[{}]}},"observed":{{"{free}":{free_v},"{card}":{card_v}}}}}"#, ls.join(","))
        };
        let records = [
            record(&[(600, true)], false, false),
            record(&[(50, true), (700, false)], false, false),
            record(&[(45000, true), (700, true)], true, true),
            record(&[(20000, true), (20000, true)], true, true),
        ];
        std::fs::write(&r, records.join("\n") + "\n").unwrap();
        let (a, b, r) = (a.to_string_lossy().to_string(), b.to_string_lossy().to_string(), r.to_string_lossy().to_string());
        let (c, out) = rulec(&["diff", &a, &b, "--format", "json"]);
        assert_eq!(c, 1, "{rule}: {out}");
        assert_eq!(domain_of(&json(&out)), [("field_range".to_string(), name.to_string(), "0..50000".to_string(), "100..40000".to_string())], "{rule}: {out}");
        let (c, out) = rulec(&["diff", &a, &b, "--fixtures", &r, "--format", "json"]);
        assert_eq!(c, 1, "{rule}: {out}");
        let j = json(&out);
        assert_eq!((int(&j, "compared"), int(&j, "matched")), (4, 2), "{rule}: {out}");
        let refused: Vec<(&str, &str, i128)> = arr(&j, "refused")
            .iter()
            .map(|x| (x.get("kind").and_then(|v| v.as_str()).unwrap_or(""), x.get("field").and_then(|v| v.as_str()).unwrap_or(""), x.get("count").and_then(|v| v.as_int()).unwrap_or(0)))
            .collect();
        assert_eq!(refused, [("input_range", name, 2)], "{rule}: {out}");
    }
}

/// A rule whose answer reads what a walk comes to, with no table that tests it, had no column to
/// cut its inputs by, and its one cell was every sequence there is, tried on the empty one. A
/// version that adds one to the total answers every sequence differently, and `diff` said "nothing
/// that decides an answer changed: every input gets the same answer" in text, though its JSON
/// counted the cell as differing; a version that adds the count of the big lots agrees on the
/// empty sequence, and was counted as proved the same; and a version that moves the number a
/// total is compared with passed as the same. What a walk comes to is a column of the comparison
/// whether a table tests it or an expression reads it: the first differs in every cell, the second
/// where there is a big lot, with a sequence that shows it, and the third between the two numbers.
#[test]
fn a_walk_no_table_tests_is_not_called_the_same_on_one_sequence() {
    let unchanged = "答えを決めるものは何も変わっていません";
    for (src, [seq, _, _, total, _, _, define, bigs]) in LOTS {
        let tmp = TempDir::new("vdiff-walk-only");
        let write = |name: &str, text: String| {
            let p = tmp.path().join(name);
            std::fs::write(&p, text).unwrap();
            p.to_string_lossy().into_owned()
        };
        assert!(src.ends_with(define), "{seq}");
        let v1 = write("v1.rule", src.to_string());
        let plus_one = write("plus_one.rule", src.replace(define, &define.replace('\n', " + 1\n")));
        let plus_bigs = write("plus_bigs.rule", src.replace(define, &define.replace('\n', &format!(" + {bigs}\n"))));

        let (c, out) = rulec(&["diff", &v1, &plus_one, "--format", "json"]);
        assert_eq!(c, 1, "{seq}: {out}");
        let j = json(&out);
        assert_eq!((int(&j, "cells"), int(&j, "same"), int(&j, "differing"), int(&j, "unsettled"), int(&j, "unrealized")), (9, 0, 9, 0, 0), "{seq}: {out}");
        let (c, text) = rulec(&["diff", &v1, &plus_one]);
        assert_eq!(c, 1, "{seq}:\n{text}");
        assert!(!text.contains(unchanged) && text.contains(&format!("{total}: 0 → 1")), "{seq}:\n{text}");

        let (c, out) = rulec(&["diff", &v1, &plus_bigs, "--format", "json"]);
        assert_eq!(c, 1, "{seq}: {out}");
        let j = json(&out);
        // the six cells with a big lot differ; with none, the two at an end of the total are the
        // same and the one between is left unsettled
        assert_eq!((int(&j, "same"), int(&j, "differing"), int(&j, "unsettled"), int(&j, "unrealized")), (2, 6, 1, 0), "{seq}: {out}");
        assert!(!is_true(&j, "total") && arr(&j, "unknown").len() == 1, "{seq}: {out}");
        let (_, text) = rulec(&["diff", &v1, &plus_bigs]);
        assert!(!text.contains(unchanged) && text.contains(&format!("{bigs} >=1 <=20")) && text.contains(&format!("{total}: 0 → 1")), "{seq}:\n{text}");

        // the version compared with itself is still the same, and passes
        let (c, text) = rulec(&["diff", &v1, &v1]);
        assert_eq!(c, 0, "{seq}:\n{text}");
        assert!(text.contains(unchanged), "{seq}:\n{text}");
    }
    // Whether a sale is a big one, by what the fees of its lots add up to, and a version that
    // moves the number from 500 to 600: a sale of 501 to 600 is big in one and not in the other,
    // and the sequence shown adds up to a total between.
    for (src, total_fee, big_sale) in [
        (
            "rule sale v1\n\nelements lots\n  fee : number  range >=0 <=100\n\nsum total_fee over lots of fee  range >=0 <=1000\n\n\
             outputs\n  big_sale : bool\n\ndefine big_sale : bool = total_fee > 500\n",
            "total_fee",
            "big_sale",
        ),
        (
            "rule 大口の判定(sale) v1\n\nelements 出品(lots)\n  手数料(fee) : number  range >=0 <=100\n\n\
             sum 手数料の合計(total_fee) over 出品 of 手数料  range >=0 <=1000\n\n\
             outputs\n  大口(big_sale) : bool\n\ndefine 大口 : bool = 手数料の合計 > 500\n",
            "手数料の合計",
            "大口",
        ),
    ] {
        let tmp = TempDir::new("vdiff-walk-compared");
        let (a, b) = (tmp.path().join("v1.rule"), tmp.path().join("v2.rule"));
        assert!(src.ends_with("> 500\n"), "{total_fee}");
        std::fs::write(&a, src).unwrap();
        std::fs::write(&b, src.replace("> 500\n", "> 600\n")).unwrap();
        let (a, b) = (a.to_string_lossy().to_string(), b.to_string_lossy().to_string());
        let (c, out) = rulec(&["diff", &a, &b, "--format", "json"]);
        assert_eq!(c, 1, "{total_fee}: {out}");
        let j = json(&out);
        assert!(!is_true(&j, "total") && int(&j, "differing") == 2 && int(&j, "unrealized") == 0, "{total_fee}: {out}");
        let (c, text) = rulec(&["diff", &a, &b]);
        assert_eq!(c, 1, "{total_fee}:\n{text}");
        assert!(!text.contains(unchanged) && text.contains(&format!("{total_fee} >=501 <=600")) && text.contains(&format!("{big_sale}: true → false")), "{total_fee}:\n{text}");
    }
}
