//! `rulec import csv` (§15.41): a draft of a `.rule` from a spreadsheet's CSV export. What
//! is tested is the shape and the honesty — every guess is marked, the values are copied as
//! they were, and a complete lookup table passes `check` as it comes out — not the
//! correctness of any guess, which by design is for a person.

use std::path::PathBuf;
use std::process::Command;

fn run(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")))
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The same, with the language pinned. `.cargo/config.toml` forces `RULEC_LANG=ja` for
/// everything cargo launches, so without this the English draft is never exercised.
fn run_lang(lang: &str, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")))
        .env("RULEC_LANG", lang)
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rulec-import-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn 表引きのcsvは列挙と表になり_そのままcheckを通る() {
    let d = dir("lookup");
    let csv = d.join("運賃.csv");
    std::fs::write(&csv, "\u{feff}あて先,サイズ,運賃\r\n近畿圏,S60,\"990円\"\r\n近畿圏,S80,\"1,310円\"\r\n遠隔地,S60,1200円\r\n遠隔地,S80,1800円\r\n").unwrap();
    let (c, out, e) = run(&["import", "csv", csv.to_str().unwrap()]);
    assert_eq!(c, 0, "{e}");
    assert!(out.starts_with("rule 運賃(imported) v1\n"), "{out}");
    assert!(out.contains("enum あて先の値(c1_kind) = 近畿圏(v1) | 遠隔地(v2)"), "{out}");
    assert!(out.contains("enum サイズの値(c2_kind) = S60 | S80"), "ASCII の値に別名は要らない: {out}");
    assert!(out.contains("  運賃(o1) : money[円, incl_tax]  round down(1円)  # 推定"), "{out}");
    assert!(out.contains("| 近畿圏 | S80 | 1310円 |"), "桁区切りを外して写す: {out}");
    assert!(out.contains("# 出典: "), "出典の列が用意される: {out}");
    // A draft is honest about itself: every guess is marked.
    assert!(out.matches("推定").count() >= 4, "{out}");
    // Four rows over 2 × 2 values: complete as it stands, so check passes.
    let rule = d.join("運賃.rule");
    std::fs::write(&rule, &out).unwrap();
    let (c, o, e) = run(&["fmt", rule.to_str().unwrap()]);
    assert_eq!(c, 0, "{o}{e}");
    let (c, o, e) = run(&["check", rule.to_str().unwrap()]);
    assert_eq!(c, 0, "下書きが check を通らない:\n{o}{e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn 数値の列は範囲つきの入力になり_等値で写したと断る() {
    let d = dir("numeric");
    let csv = d.join("wt.csv");
    std::fs::write(&csv, "weight,fee\n1000g,800円\n2000g,800円\n5000g,1100円\n").unwrap();
    let (c, out, e) = run(&["import", "csv", csv.to_str().unwrap(), "--name", "重さ運賃"]);
    assert_eq!(c, 0, "{e}");
    assert!(out.starts_with("rule 重さ運賃(imported) v1\n"), "{out}");
    assert!(out.contains("  weight : mass[g]  range >=1000g <=5000g  # 推定"), "{out}");
    assert!(out.contains("| 1000g | 800円 |"), "{out}");
    assert!(out.contains("等値で写した"), "閾値かもしれないと断る: {out}");
    // It parses, and check says what a person has to decide: the gaps between the values.
    let rule = d.join("wt.rule");
    std::fs::write(&rule, &out).unwrap();
    let (c, o, _) = run(&["check", rule.to_str().unwrap(), "--format", "json"]);
    assert_eq!(c, 1, "{o}");
    assert!(o.contains("\"code\":\"E101\""), "{o}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn 日付と率の列() {
    let d = dir("mixed");
    let csv = d.join("rt.csv");
    std::fs::write(&csv, "kind,date,rate\nA,2026-04-01,10%\nB,2026/05/01,12.5%\n").unwrap();
    let (c, out, e) = run(&["import", "csv", csv.to_str().unwrap()]);
    assert_eq!(c, 0, "{e}");
    assert!(out.contains("  date : date  range >=2026-04-01 <=2026-05-01"), "{out}");
    assert!(out.contains("  rate : rate[step 0.1%]  round down(0.1%)"), "小数の率は 0.1% 刻みで、丸めの刻みも同じ: {out}");
    assert!(out.contains("# 出典: rt.csv（"), "出典はファイル名だけ: {out}");
    assert!(out.contains("| B | 2026-05-01 | 12.5% |"), "日付の綴りをそろえる: {out}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn 表の形でないcsvは断る() {
    let d = dir("bad");
    let csv = d.join("one.csv");
    std::fs::write(&csv, "only\n1\n2\n").unwrap();
    let (c, _, e) = run(&["import", "csv", csv.to_str().unwrap()]);
    assert_eq!(c, 2, "{e}");
    let ragged = d.join("ragged.csv");
    std::fs::write(&ragged, "a,b,c\n1,2,3\n1,2\n").unwrap();
    let (c, _, e) = run(&["import", "csv", ragged.to_str().unwrap()]);
    assert_eq!(c, 2);
    assert!(e.contains("3 行目"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// The draft has to parse, and the language's own words are not names (E009). Two sources of
/// them: the draft's English word for its own table, which was `table`, and a column the file
/// headed with a keyword. The suite runs in Japanese (`.cargo/config.toml`), where the table
/// is `表`, so the English draft was broken for as long as it existed.
#[test]
fn キーワードと同じ名前の下書きは作らない() {
    let d = dir("keywords");
    let csv = d.join("kw.csv");
    std::fs::write(&csv, "count,source,fee\n1,near,100円\n2,far,200円\n").unwrap();
    for lang in ["en", "ja"] {
        let (c, out, e) = run_lang(lang, &["import", "csv", csv.to_str().unwrap()]);
        assert_eq!(c, 0, "{e}");
        // The keyword keeps its spelling as the alias, which is what the generated code says.
        assert!(out.contains("count_(count) :"), "{lang}: {out}");
        assert!(out.contains("source_(source) :"), "{lang}: {out}");
        // And the table header refers to the column by the name that was declared.
        assert!(out.contains("| count_ | source_ |"), "{lang}: 見出しが宣言と食い違う: {out}");
        let rule = d.join(format!("kw-{lang}.rule"));
        std::fs::write(&rule, &out).unwrap();
        let (c, o, _) = run_lang(lang, &["check", rule.to_str().unwrap(), "--format", "json"]);
        assert!(!o.contains("\"code\":\"E009\""), "{lang}: 下書きが E009 で落ちる:\n{o}");
        // E101 is what is left: the gaps between the values, which is a person's to decide.
        assert_eq!(c, 1, "{lang}: {o}");
    }
    let _ = std::fs::remove_dir_all(&d);
}
