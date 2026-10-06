//! The mutants (PLAN B.12), in English: every mutant of a Japanese name, and `E011_dir` (whose
//! files are Japanese), has a twin of an English name whose files are English, and gives the same
//! diagnostics. Both are walked by mutants.rs, which checks each against its golden file; this is
//! what pairs them (the pairs are told by the code the names start with). The Japanese mutants
//! are kept as they are.
//!
//! What only e-Gov's text can show has its twin in the eCFR (`E107`'s copy of one section with
//! another text, `E406`'s gap of one day); `E206_rulec-not-joined` is English already.

mod common;

use ritsu_base::text::Lang;

/// (the mutant of a Japanese name or Japanese files, its English twin).
const PAIRS: [(&str, &str); 74] = [
    ("E001_閉じていない文字列", "E001_unclosed_string"),
    ("E002_名前に予約語", "E002_reserved_word_as_a_name"),
    ("E002_知らない行", "E002_unknown_line"),
    ("E003_見出しが無い", "E003_no_heading"),
    ("E004_要件の中の順序", "E004_order_inside_a_requirement"),
    ("E004_記録が二つ", "E004_two_records"),
    ("E005_タブ", "E005_tab"),
    ("E005_記録がリンクの下にない", "E005_record_not_under_a_link"),
    ("E006_無い日付", "E006_no_such_date"),
    ("E006_逆さまの期間", "E006_inverted_period"),
    ("E007_同じ役割", "E007_same_role"),
    ("E007_同じ要件", "E007_same_requirement"),
    ("E008_宣言されていない役割", "E008_undeclared_role"),
    ("E008_宣言されていない要件", "E008_undeclared_requirement"),
    ("E009_版が零", "E009_version_zero"),
    ("E009_版を書かずに指す", "E009_pointed_at_without_a_version"),
    ("E010_出どころが無い", "E010_no_source"),
    ("E010_別名が無い", "E010_no_alias"),
    ("E011_dir", "E011_scope_directory"),
    ("E011_知らないツール", "E011_unknown_tool"),
    ("E012_dandoriの種類", "E012_kind_of_dandori"),
    ("E012_valueだけ", "E012_value_alone"),
    ("E012_yuenの名指し", "E012_yuen_named"),
    ("E012_借りる出典がchobo", "E012_borrowing_from_chobo"),
    ("E012_出典をリンクに", "E012_source_as_a_link"),
    ("E013_ルートの外", "E013_outside_the_root"),
    ("E013_絶対パス", "E013_absolute_path"),
    ("E101_コピーが無い", "E101_copy_missing"),
    ("E102_固定の行にハッシュが無い", "E102_pin_without_a_hash"),
    ("E102_引いた条の固定が無い", "E102_cited_article_not_pinned"),
    ("E103_固定と違う", "E103_pin_differs"),
    ("E104_XMLでない", "E104_not_xml"),
    ("E104_条でない要素", "E104_not_an_article"),
    ("E104_差分の仕様を名指す", "E104_delta_spec_named"),
    ("E105_宣言されていない出典", "E105_undeclared_source"),
    ("E105_条の形でない", "E105_not_an_article_form"),
    ("E106_宣言されていない出典を借りる", "E106_borrows_an_undeclared_source"),
    ("E107_同じ条の違う本文", "E107_same_article_different_text"),
    ("E108_仕様に無い要件", "E108_no_such_requirement"),
    ("E201_ディレクトリ", "E201_directory"),
    ("E201_成果物のファイルが無い", "E201_artifact_file_missing"),
    ("E202_名前が変わった", "E202_renamed"),
    ("E202_無いメッセージ", "E202_no_such_message"),
    ("E203_検査を通らない規則", "E203_rule_that_does_not_pass"),
    ("E205_読めないproto", "E205_unreadable_proto"),
    ("E301_確かめていない", "E301_not_looked_at"),
    ("E302_元の要件が変わった", "E302_original_requirement_changed"),
    ("E302_条が変わった", "E302_article_changed"),
    ("E303_成果物が変わった", "E303_artifact_changed"),
    ("E303_要件の文が変わった", "E303_requirement_text_changed"),
    ("E303_要素が変わった", "E303_element_changed"),
    ("E304_承認が無い", "E304_no_approval"),
    ("E304_承認のあとで要件が変わった", "E304_requirement_changed_after_approval"),
    ("E305_ハッシュの数", "E305_hash_count"),
    ("E305_矢印が無い", "E305_no_arrow"),
    ("E401_満たすものが無い", "E401_nothing_meets_it"),
    ("E402_確かめるものが無い", "E402_nothing_checks_it"),
    ("E403_出力を確かめる側に", "E403_output_on_the_checking_side"),
    ("E404_範囲のファイル", "E404_file_in_scope"),
    ("E405_循環", "E405_cycle"),
    ("E406_隙間が一日", "E406_gap_of_one_day"),
    ("E407_重なりが三日", "E407_overlap_of_three_days"),
    ("E408_期間の無い版", "E408_version_without_a_period"),
    ("E408_番号が期間の順でない", "E408_number_out_of_period_order"),
    ("E408_開いた終わりが最後でない", "E408_open_end_not_last"),
    ("E409_置き換えの始まりが一日遅い", "E409_replacement_starts_a_day_late"),
    ("W101_引かれていない固定", "W101_pin_not_cited"),
    ("W102_固定していない要件", "W102_requirement_not_pinned"),
    ("W301_中身が無い", "W301_no_content"),
    ("W401_見送りとリンク", "W401_waiver_and_link"),
    ("W402_主張の無いシナリオ", "W402_scenario_without_a_claim"),
    ("W901_文字列の鍵", "W901_key_in_a_string"),
    ("W901_コメントの鍵", "W901_key_in_a_comment"),
    ("W901_テスト用の鍵", "W901_test_secret"),
];

fn mutants() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir("tests/mutants").unwrap().filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    v.sort();
    v
}

/// Whether any name or content under `dir` has a character of Japanese.
fn japanese_in(dir: &std::path::Path) -> Option<String> {
    let is_cjk = |s: &str| s.chars().any(|c| matches!(c as u32, 0x3000..=0x9FFF | 0xFF00..=0xFFEF));
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if is_cjk(&p.file_name().unwrap().to_string_lossy()) {
            return Some(p.display().to_string());
        }
        if p.is_dir() {
            if let Some(f) = japanese_in(&p) {
                return Some(f);
            }
        } else if is_cjk(&String::from_utf8_lossy(&std::fs::read(&p).unwrap())) {
            return Some(p.display().to_string());
        }
    }
    None
}

fn codes_of(name: &str) -> Vec<&'static str> {
    let path = format!("tests/mutants/{name}");
    let c = yuen::check::check_with(std::slice::from_ref(&path), Some(&path), common::suite()).unwrap_or_else(|r| panic!("{path} is refused: {}", r.0.en));
    c.diags.iter().map(|d| d.code).collect()
}

/// Every Japanese mutant has an English one of the same code, with English files, and the two give
/// the same codes in the same order.
#[test]
fn every_japanese_mutant_has_an_english_one() {
    let names = mutants();
    let mut failures = Vec::new();
    for (ja, en) in PAIRS {
        for n in [ja, en] {
            if !names.iter().any(|m| m == n) {
                failures.push(format!("{n} is not in tests/mutants"));
            }
        }
        if ja.split('_').next() != en.split('_').next() {
            failures.push(format!("{ja} and {en} are not of one code"));
        }
        if ja == en || !en.is_ascii() {
            failures.push(format!("{en} is not a name in English"));
        }
        if let Some(f) = japanese_in(&std::path::Path::new("tests/mutants").join(en)) {
            failures.push(format!("{en} has Japanese in {f}"));
        }
        if names.iter().any(|m| m == ja) && names.iter().any(|m| m == en) {
            let (a, b) = (codes_of(ja), codes_of(en));
            if a != b {
                failures.push(format!("{ja} gives {a:?} and {en} gives {b:?}"));
            }
        }
    }
    // every mutant of Japanese names or files is in a pair: a new one needs its twin
    for n in &names {
        let japanese = !n.is_ascii() || japanese_in(&std::path::Path::new("tests/mutants").join(n)).is_some();
        if japanese && !PAIRS.iter().any(|(ja, _)| ja == n) {
            failures.push(format!("{n} is Japanese and has no English twin"));
        }
    }
    assert_eq!(PAIRS.iter().filter(|(ja, _)| !ja.is_ascii()).count(), 73, "the 73 mutants of Japanese names are all kept");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The JSON of a diagnostic keeps its keys in English, and carries its diff: the twin of the test
/// of the same name in mutants.rs.
#[test]
fn the_json_of_a_diagnostic_in_english() {
    let path = "tests/mutants/E302_article_changed".to_string();
    let c = yuen::check::check_with(std::slice::from_ref(&path), Some(&path), common::suite()).unwrap();
    for lang in [Lang::En, Lang::Ja] {
        let v = yuen::check::to_json(&c, &path, lang);
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["root", "ok", "summary", "diagnostics"]);
        let d = &v["diagnostics"][0];
        let keys: Vec<&String> = d.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["code", "severity", "file", "line", "col", "message", "notes", "diff", "chain", "candidates", "fix"]);
        assert_eq!(d["code"], "E302");
        assert_eq!(d["file"], "period_of_months.req");
        assert!(d["diff"].as_array().unwrap().iter().any(|l| l["op"] == "+" && l["text"].as_str().unwrap().contains("second succeeding")));
        assert_eq!(v["ok"], false);
    }
}
