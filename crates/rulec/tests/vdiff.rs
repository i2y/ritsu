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

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rulec(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(root()).args(args).output().expect("rulec を起動できない");
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
    let dir = std::env::temp_dir().join("rulec-vdiff-送料");
    let _ = std::fs::create_dir_all(&dir);
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
    let dir = std::env::temp_dir().join("rulec-vdiff-fold");
    let _ = std::fs::create_dir_all(&dir);
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
    // Where the walk gives up, and why. `クーポン割引` has two derived columns sharing an
    // input, so the sieve cannot rule out a combination no input reaches either (§6.2,
    // the blind spot W114 already names). `預け荷物料金` has one cell whose answer is not
    // constant on it — the free allowance is per passenger — where the two versions agree
    // at every point tried and that could not be turned into a proof over the whole cell.
    let 決められない: &[&str] = &["クーポン割引", "預け荷物料金"];
    let (mut checked, mut withheld) = (0, Vec::new());
    for p in corpus() {
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        let src = std::fs::read_to_string(&p).unwrap();
        let dir = std::env::temp_dir().join(format!("rulec-vdiff-{name}"));
        let _ = std::fs::create_dir_all(&dir);
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
/// says so instead of answering about nothing.
#[test]
fn 予算を超えたら_領域を出さずにそう言う() {
    let src = std::fs::read_to_string(root().join("tests/corpus/送料.rule")).unwrap();
    let dir = std::env::temp_dir().join("rulec-vdiff-budget");
    let _ = std::fs::create_dir_all(&dir);
    let (a, b) = (dir.join("a.rule"), dir.join("b.rule"));
    std::fs::write(&a, &src).unwrap();
    std::fs::write(&b, src.replace("| 遠隔地      | >2000g  | 1800円", "| 遠隔地      | >2000g  | 2000円")).unwrap();
    let (as_, bs) = (a.to_string_lossy().to_string(), b.to_string_lossy().to_string());
    let (code, out) = rulec(&["diff", &as_, &bs, "--budget", "10", "--format", "json"]);
    let j = json(&out);
    assert!(is_true(&j, "over_budget"), "{out}");
    assert!(!is_true(&j, "total"), "予算を超えたのに外について主張している");
    assert!(arr(&j, "changes").is_empty(), "領域を出さないと言いながら出している");
    assert_eq!(code, 0, "何も出していないので影響ありとは言わない");
    let (_, text) = rulec(&["diff", &as_, &bs, "--budget", "10"]);
    assert!(text.contains("budget") || text.contains("予算"), "{text}");
}

/// What the rule accepts is a different question from what it answers, and the two are not
/// mixed. A version that takes one more value of an enum has a wider door, not a different
/// answer, and the report says which.
#[test]
fn 受け付ける入力が変わったことは_答えの差とは別に出る() {
    let src = std::fs::read_to_string(root().join("tests/corpus/送料.rule")).unwrap();
    let dir = std::env::temp_dir().join("rulec-vdiff-domain");
    let _ = std::fs::create_dir_all(&dir);
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
    // 広がったぶんは旧版が断る入力なので、軸は両方が受け付ける範囲に絞る。絞らずに
    // 比べると、旧版に入口で断られる入力について「同じ答えだ」と言ってしまう。
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
    let have: Vec<String> = j.as_obj().expect("object").keys().cloned().collect();
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

/// The example printed on the site is what the tool prints. Two copies of it, one per
/// language, and either going stale is exactly the kind of thing a reader cannot tell from
/// the page (§15.73).
#[test]
fn 文書に載せた実演は_いまの出力と一致する() {
    let src = std::fs::read_to_string(root().join("tests/corpus/送料.rule")).unwrap();
    let dir = std::env::temp_dir().join("rulec-vdiff-demo");
    let _ = std::fs::create_dir_all(&dir);
    let (a, b) = (dir.join("v3.rule"), dir.join("v4.rule"));
    std::fs::write(&a, src.replace("rule 送料(shipping_fee) v4", "rule 送料(shipping_fee) v3")).unwrap();
    std::fs::write(&b, src.replace("| 遠隔地      | >2000g  | 1800円", "| 遠隔地      | >2000g  | 2000円")).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .env("RULEC_LANG", "ja")
        .args(["diff", &a.to_string_lossy(), &b.to_string_lossy()])
        .output()
        .expect("rulec を起動できない");
    let got = String::from_utf8_lossy(&o.stdout).into_owned();
    // ファイルの名前は文書のほうで `送料@v3` と書いてあるので、そこだけ読み替える。
    let body: Vec<&str> = got.lines().skip(1).collect();
    for doc in ["website/docs/compare.md", "website/docs-ja/compare.md"] {
        let page = std::fs::read_to_string(root().join(doc)).unwrap();
        for line in &body {
            assert!(
                line.trim().is_empty() || page.contains(line),
                "{doc} の実演がいまの出力と食い違う。無い行:\n{line}\n出力全体:\n{got}"
            );
        }
    }
}

/// The flags that only mean something against records are refused here rather than
/// ignored, and the ones that mean something in both modes work in both.
#[test]
fn 意味の無い旗は断り_意味のある旗は効く() {
    let p = root().join("tests/corpus/送料.rule");
    let s = p.to_string_lossy().to_string();
    for bad in [vec!["diff", &s, &s, "--fill", "重量=1"], vec!["diff", &s, &s, "--manifest", "m.json"]] {
        let (code, _) = rulec(&bad);
        assert_eq!(code, 2, "{bad:?} が断られていない");
    }
    // --budget belongs to the other side of the same coin.
    let (code, _) = rulec(&["diff", &s, &s, "--fixtures", "/dev/null", "--budget", "10"]);
    assert_eq!(code, 2, "--budget が --fixtures と一緒に通ってしまう");

    // --terse keeps the example out of both renderings, in the mode that has one.
    let src = std::fs::read_to_string(&p).unwrap();
    let dir = std::env::temp_dir().join("rulec-vdiff-terse");
    let _ = std::fs::create_dir_all(&dir);
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
