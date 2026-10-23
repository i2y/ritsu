//! `rulec import csv` (§15.41): a draft of a `.rule` from a spreadsheet's CSV export. What
//! is tested is the shape and the honesty — every guess is marked, the values are copied as
//! they were, and a complete lookup table passes `check` as it comes out — not the
//! correctness of any guess, which by design is for a person.

use std::path::PathBuf;
use std::process::Command;
use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, ready, side_by_side, skip};
use rulec::json::Json;

fn run(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "ja")
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")))
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The same, with the language pinned. The suite runs rulec with `RULEC_LANG=ja`, so
/// without this the English draft is never exercised.
fn run_lang(lang: &str, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")))
        .env("RULEC_LANG", lang)
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// Whether the draft has a table row with these cells, however it is padded.
fn has_row(out: &str, cells: &[&str]) -> bool {
    out.lines().any(|l| {
        let t = l.trim();
        t.starts_with('|') && t.trim_matches('|').split('|').map(str::trim).collect::<Vec<_>>() == cells
    })
}

fn dir(tag: &str) -> (TempDir, PathBuf) {
    let t = TempDir::new(&format!("import-{tag}"));
    let d = t.path().to_path_buf();
    (t, d)
}

#[test]
fn 表引きのcsvは列挙と表になり_そのままcheckを通る() {
    let (_tmp, d) = dir("lookup");
    let csv = d.join("運賃.csv");
    std::fs::write(&csv, "\u{feff}あて先,サイズ,運賃\r\n近畿圏,S60,\"990円\"\r\n近畿圏,S80,\"1,310円\"\r\n遠隔地,S60,1200円\r\n遠隔地,S80,1800円\r\n").unwrap();
    let (c, out, e) = run(&["import", "csv", csv.to_str().unwrap()]);
    assert_eq!(c, 0, "{e}");
    assert!(out.starts_with("rule 運賃(imported) v1\n"), "{out}");
    assert!(out.contains("enum あて先の値(c1_kind) = 近畿圏(v1) | 遠隔地(v2)"), "{out}");
    assert!(out.contains("enum サイズの値(c2_kind) = S60 | S80"), "ASCII の値に別名は要らない: {out}");
    assert!(out.contains("  運賃(o1) : money[円, incl_tax]  round down(1円)  # 推定"), "{out}");
    assert!(has_row(&out, &["近畿圏", "S80", "1310円"]), "桁区切りを外して転記する: {out}");
    assert!(out.contains("# 出典: "), "出典の列が用意される: {out}");
    // A draft is honest about itself: every guess is marked.
    assert!(out.matches("推定").count() >= 4, "{out}");
    // Four rows over 2 × 2 values: complete as it stands, so check passes.
    let rule = d.join("運賃.rule");
    std::fs::write(&rule, &out).unwrap();
    // Written the way fmt leaves it, Japanese columns and all.
    let (c, o, e) = run(&["fmt", "--check", rule.to_str().unwrap()]);
    assert_eq!(c, 0, "下書きが fmt のとおりになっていない:\n{o}{e}");
    let (c, o, e) = run(&["check", rule.to_str().unwrap()]);
    assert_eq!(c, 0, "下書きが check を通らない:\n{o}{e}");
}

/// A sheet that writes its amounts in JPY gets a draft in JPY: the currency is spelled as the
/// sheet spells it, in the type as in the rounding, and nothing in the draft is Japanese.
#[test]
fn an_amount_in_jpy_stays_in_jpy() {
    let (_tmp, d) = dir("jpy");
    let csv = d.join("fees.csv");
    std::fs::write(&csv, "weight,fee\n1000g,800JPY\n2000g,800JPY\n5000g,1100JPY\n").unwrap();
    let (c, out, e) = run(&["import", "csv", csv.to_str().unwrap(), "--name", "parcel_fee", "--lang", "en"]);
    assert_eq!(c, 0, "{e}");
    assert!(out.contains("fee : money[JPY, incl_tax]  round down(1JPY)"), "{out}");
    assert!(!out.contains('円'), "a JPY sheet writes no 円: {out}");
    // and a sheet in 円 still writes 円
    let csv = d.join("fees_ja.csv");
    std::fs::write(&csv, "weight,fee\n1000g,800円\n2000g,800円\n").unwrap();
    let (c, out, e) = run(&["import", "csv", csv.to_str().unwrap(), "--name", "parcel_fee_ja"]);
    assert_eq!(c, 0, "{e}");
    assert!(out.contains("money[円, incl_tax]  round down(1円)"), "{out}");
}

#[test]
fn 数値の列は範囲つきの入力になり_等値として転記したと注記する() {
    let (_tmp, d) = dir("numeric");
    let csv = d.join("wt.csv");
    std::fs::write(&csv, "weight,fee\n1000g,800円\n2000g,800円\n5000g,1100円\n").unwrap();
    let (c, out, e) = run(&["import", "csv", csv.to_str().unwrap(), "--name", "重さ運賃"]);
    assert_eq!(c, 0, "{e}");
    assert!(out.starts_with("rule 重さ運賃(imported) v1\n"), "{out}");
    assert!(out.contains("  weight : mass[g]  range >=1000g <=5000g  # 推定"), "{out}");
    assert!(has_row(&out, &["1000g", "800円"]), "{out}");
    assert!(out.contains("等値として転記した"), "閾値かもしれないと注記する: {out}");
    // It parses, and check says what a person has to decide: the gaps between the values.
    let rule = d.join("wt.rule");
    std::fs::write(&rule, &out).unwrap();
    let (c, o, _) = run(&["check", rule.to_str().unwrap(), "--format", "json"]);
    assert_eq!(c, 1, "{o}");
    assert!(o.contains("\"code\":\"E101\""), "{o}");
}

#[test]
fn 日付と率の列() {
    let (_tmp, d) = dir("mixed");
    let csv = d.join("rt.csv");
    std::fs::write(&csv, "kind,date,rate\nA,2026-04-01,10%\nB,2026/05/01,12.5%\n").unwrap();
    let (c, out, e) = run(&["import", "csv", csv.to_str().unwrap()]);
    assert_eq!(c, 0, "{e}");
    assert!(out.contains("  date : date  range >=2026-04-01 <=2026-05-01"), "{out}");
    assert!(out.contains("  rate : rate[step 0.1%]  round down(0.1%)"), "小数の率は 0.1% 刻みで、丸めの刻みも同じ: {out}");
    assert!(out.contains("# 出典: rt.csv（"), "出典はファイル名だけ: {out}");
    assert!(has_row(&out, &["B", "2026-05-01", "12.5%"]), "日付の綴りをそろえる: {out}");
}

#[test]
fn 表の形でないcsvはエラーにする() {
    let (_tmp, d) = dir("bad");
    let csv = d.join("one.csv");
    std::fs::write(&csv, "only\n1\n2\n").unwrap();
    let (c, _, e) = run(&["import", "csv", csv.to_str().unwrap()]);
    assert_eq!(c, 2, "{e}");
    let ragged = d.join("ragged.csv");
    std::fs::write(&ragged, "a,b,c\n1,2,3\n1,2\n").unwrap();
    let (c, _, e) = run(&["import", "csv", ragged.to_str().unwrap()]);
    assert_eq!(c, 2);
    assert!(e.contains("3 行目"), "{e}");
}

/// The draft has to parse, and the language's own words are not names (E009). Two sources of
/// them: the draft's English word for its own table, which was `table`, and a column the file
/// headed with a keyword. The suite runs in Japanese (`RULEC_LANG=ja`), where the table
/// is `表`, so the English draft was broken for as long as it existed.
#[test]
fn キーワードと同じ名前の下書きは作らない() {
    let (_tmp, d) = dir("keywords");
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
}

/// The materials of `tests/import/`, English first and the Japanese versions beside them: for each
/// kind of word the lexer does not read as a name, a sheet that writes one in a header or in a
/// cell. Each such word is named by its alias (`c2`, `v1`, set apart by a number from a name the
/// sheet already gives: `c2_2`, `v1_2`), the word is kept in the `# 推定` note, and `check` reads
/// the draft back: every one of them is a complete table and passes (§15.259, §15.266).
const UNREAD: [(&str, &[(&str, &str)]); 18] = [
    ("header_with_a_blank_and_brackets", &[("Weight (kg)", "c1")]),
    ("空白と括弧のある見出し", &[("重量 (kg)", "c1")]),
    ("header_with_a_separator", &[("Zone/Area", "c1")]),
    ("区切りのある見出し", &[("地域・区分", "c1")]),
    ("names_that_start_with_a_digit", &[("2nd Factor", "c1"), ("2FA", "v1")]),
    ("数字で始まる名前", &[("2段階目", "c1"), ("2段階認証", "v1")]),
    ("values_with_a_hyphen_or_a_dot", &[("re-quote", "v1"), ("v1.2", "v1"), ("No.5", "v2")]),
    ("ハイフンと点のある値", &[("再-見積", "v1"), ("第1.2版", "v1"), ("第5.0版", "v2")]),
    ("values_with_a_blank", &[("on hold", "v1")]),
    ("空白のある値", &[("保留 中", "v1")]),
    ("numbers_and_dates_among_words", &[("1200JPY", "v2"), ("2026-04-01", "v3")]),
    ("語の中の数と日付", &[("1200円", "v2"), ("2026-04-01", "v3")]),
    ("values_that_compare", &[("<=2000g", "v1"), (">2000g", "v2")]),
    ("比べる値", &[("<=2000g", "v1"), (">2000g", "v2")]),
    ("a_name_an_alias_would_take", &[("Zone/Area", "c2_2"), ("v2.0", "v1_2")]),
    ("別名と同じ名前", &[("地域・区分", "c2_2"), ("v2.0", "v1_2")]),
    ("headers_left_empty_or_broken", &[("", "c1"), ("Delivery zone", "c2")]),
    ("空の見出しと改行のある見出し", &[("", "c1"), ("配送の 地域", "c2")]),
];

/// The materials of `tests/import/` whose words the lexer reads as names, but which the code
/// generated for some language cannot take as they are written, or spells as another name — what
/// `check` says with W121, or stops at with E034 — English first and the Japanese versions beside
/// them, with how many of the trailing columns are outputs (two outputs are the fields of a
/// `NamedTuple` in Python), and the declaration each word gets: an alias the generated code calls it
/// by (`Zone(zone)`, `North(v2)`), the name alone where that is what it calls it (`import_`), or the
/// alias of its kind as the name of a second column headed alike (`c2`). The third field is `en`
/// where only the English draft spells the word so: an enum's own name, which the Japanese draft
/// writes `<header>の値` with the alias of its kind beside it already. The word and what the
/// generated code calls it are kept in a `# 推定` note, and `check` reads the draft back without a
/// word (§15.266).
const RESPELLED: [(&str, usize, &[(&str, &str, &str)]); 11] = [
    ("headers_that_start_with_a_capital", 1, &[("Zone", "Zone(zone)", ""), ("Weight", "Weight(weight)", ""), ("Fee", "Fee(fee)", "")]),
    ("大文字で始まる見出し", 1, &[("SKU", "SKU(sku)", "")]),
    ("headers_that_are_keywords_of_a_language", 1, &[("type", "type(type_)", ""), ("class", "class(class_)", ""), ("import", "import_", "")]),
    ("outputs_that_start_with_an_underscore", 2, &[("_fee", "_fee(o1)", ""), ("_tax", "_tax(o2)", "")]),
    (
        "an_output_longer_than_63_bytes",
        1,
        &[("the_fee_for_a_parcel_carried_to_the_far_north_of_the_country_by_road", "the_fee_for_a_parcel_carried_to_the_far_north_of_the_country_by_road(o1)", "")],
    ),
    ("headers_spelled_alike", 1, &[("shipZone", "shipZone(c2)", ""), ("shipZone_values", "shipZone_values(c2_kind)", "en")]),
    ("headers_that_differ_in_case", 1, &[("Zone", "Zone(c1)", ""), ("zone_values", "zone_values(c2_kind)", "en")]),
    ("values_spelled_alike", 1, &[("North", "North(v2)", ""), ("goldplus", "goldplus(v2)", ""), ("low", "low(v2)", "")]),
    ("大文字と小文字だけ違う値", 1, &[("s60", "s60(v2)", "")]),
    ("headers_written_twice", 1, &[("zone", "c2", "")]),
    ("同じ見出しが二つ", 1, &[("地域", "c2", "")]),
];

#[test]
fn a_draft_is_read_back_by_check_whatever_the_sheet_writes() {
    let (_tmp, d) = dir("read-back");
    let mut materials: Vec<String> = std::fs::read_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/import"))
        .unwrap()
        .map(|e| e.unwrap().path().file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    materials.sort();
    let mut listed: Vec<String> = UNREAD.iter().map(|(m, _)| m.to_string()).chain(RESPELLED.iter().map(|(m, _, _)| m.to_string())).collect();
    listed.sort();
    assert_eq!(materials, listed, "every material is listed with the words it writes");
    for (m, words) in UNREAD {
        let csv = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/import/{m}.csv"));
        for lang in ["en", "ja"] {
            let (c, out, e) = run_lang(lang, &["import", "csv", csv.to_str().unwrap()]);
            assert_eq!(c, 0, "{m} {lang}: {e}");
            for (word, name) in words {
                // the name takes the word's place: a column of the table's head, or a value of an enum
                let named = out.lines().any(|l| l.trim().starts_with('|') && l.split('|').any(|cell| cell.trim() == *name || cell.trim().starts_with(&format!("{name} :"))));
                assert!(named, "{m} {lang}: {name} is not in the table:\n{out}");
                // and the word is kept in the note, on a comment
                let kept = if word.is_empty() {
                    if lang == "ja" { format!("見出しが空なので、仮に {name} と名付けた") } else { format!("the header is empty, so the column is named {name} for now") }
                } else if lang == "ja" {
                    format!("「{word}」")
                } else {
                    format!("\"{word}\"")
                };
                assert!(out.lines().any(|l| l.split_once('#').is_some_and(|(_, c)| c.contains(&kept) && c.contains(*name))), "{m} {lang}: {kept} is not kept beside {name}:\n{out}");
            }
            let rule = d.join(format!("{m}.{lang}.rule"));
            std::fs::write(&rule, &out).unwrap();
            let (c, o, e) = run_lang(lang, &["check", rule.to_str().unwrap(), "--format", "json"]);
            assert_eq!((c, o.as_str()), (0, ""), "{m} {lang}: check does not read the draft back:\n{out}\n{e}");
            let (c, o, _) = run_lang(lang, &["fmt", "--check", rule.to_str().unwrap()]);
            assert_eq!(c, 0, "{m} {lang}: {o}");
        }
    }
}

/// Whether the draft declares `decl`: a column on its line, an enum on its, or a value of one.
fn declares(out: &str, decl: &str) -> bool {
    out.lines().any(|l| {
        let code = l.split_once('#').map_or(l, |(code, _)| code).trim();
        code.starts_with(&format!("{decl} :"))
            || code.strip_prefix("enum ").is_some_and(|e| e.starts_with(&format!("{decl} =")) || e.split_once('=').is_some_and(|(_, vs)| vs.split('|').any(|v| v.trim() == decl)))
    })
}

/// What the generated code calls what `decl` declares: the alias, else the name.
fn called(decl: &str) -> &str {
    decl.split_once('(').map_or(decl, |(_, a)| a.trim_end_matches(')'))
}

/// A name the code generated for some language cannot take as the sheet wrote it, or spells as
/// another name's, is spelled the way `check` says nothing of (§15.266): the materials of
/// `RESPELLED`, in both languages.
#[test]
fn a_name_the_generated_code_cannot_take_as_written_is_spelled_so_that_it_can() {
    let (_tmp, d) = dir("respelled");
    for (m, outputs, words) in RESPELLED {
        let csv = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/import/{m}.csv"));
        let n = outputs.to_string();
        for lang in ["en", "ja"] {
            let (c, out, e) = run_lang(lang, &["import", "csv", csv.to_str().unwrap(), "--outputs", &n]);
            assert_eq!(c, 0, "{m} {lang}: {e}");
            for (word, decl, _) in words.iter().filter(|(_, _, only)| only.is_empty() || *only == lang) {
                assert!(declares(&out, decl), "{m} {lang}: {word} is not declared as {decl}:\n{out}");
                let ident = called(decl);
                assert!(out.lines().any(|l| l.split_once('#').is_some_and(|(_, c)| c.contains(*word) && c.contains(ident))), "{m} {lang}: the note does not keep {word} and {ident}:\n{out}");
            }
            let rule = d.join(format!("{m}.{lang}.rule"));
            std::fs::write(&rule, &out).unwrap();
            let (c, o, e) = run_lang(lang, &["check", rule.to_str().unwrap(), "--format", "json"]);
            assert_eq!((c, o.as_str()), (0, ""), "{m} {lang}: check does not read the draft back without a word:\n{out}\n{e}");
            let (c, o, _) = run_lang(lang, &["fmt", "--check", rule.to_str().unwrap()]);
            assert_eq!(c, 0, "{m} {lang}: {o}");
        }
    }
}

/// The rule's name, from the file or from `--name`, is read the same way, and the file's name inside
/// the draft's description string is written so that the string ends where it should.
#[test]
fn a_rule_name_not_read_as_a_name_is_named_for_now() {
    let (_tmp, d) = dir("rule-name");
    let csv = d.join("fees \"2026\".csv");
    std::fs::copy(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/import/values_that_compare.csv"), &csv).unwrap();
    for (lang, note) in [
        ("en", "rule imported v1  # guess: the rule's name \"fees \"2026\"\" is not read as a name, so the rule is named imported for now\n"),
        ("ja", "rule imported v1  # 推定: 規則の名前「fees \"2026\"」は名前として読めないので、仮に imported と名付けた\n"),
    ] {
        let (c, out, e) = run_lang(lang, &["import", "csv", csv.to_str().unwrap()]);
        assert_eq!(c, 0, "{e}");
        assert!(out.starts_with(note), "{lang}: {out}");
        assert!(out.lines().nth(1).unwrap().starts_with("description \"") && out.contains("fees '2026'.csv"), "{lang}: {out}");
        let rule = d.join(format!("named-{lang}.rule"));
        std::fs::write(&rule, &out).unwrap();
        let (c, o, _) = run_lang(lang, &["check", rule.to_str().unwrap(), "--format", "json"]);
        assert_eq!((c, o.as_str()), (0, ""), "{lang}:\n{out}");
    }
    // `--name` too, and a name the lexer reads stays as it was
    let (_, out, _) = run_lang("en", &["import", "csv", csv.to_str().unwrap(), "--name", "parcel fee"]);
    assert!(out.starts_with("rule imported v1  # guess: the rule's name \"parcel fee\" is not read as a name"), "{out}");
    let (_, out, _) = run_lang("en", &["import", "csv", csv.to_str().unwrap(), "--name", "parcel_fee"]);
    assert!(out.starts_with("rule parcel_fee v1\n"), "{out}");
    // unless the generated code cannot take it as written (§15.266): `sum` is a word of the language,
    // and the generated code could not take its spelling as the alias, which hides a word some
    // languages have already; and an enum whose type the rule's name takes is spelled so that it
    // does not.
    for (name, said) in [
        ("sum", "rule sum_ v1  # guess: the code generated for some languages cannot take the rule's name \"sum\" as it is, or makes it one identifier with another name, so the generated code names the rule sum_ for now\n"),
        ("Zone_values", "enum Zone_values(c1_kind) = north | south  # guess: the values seen in this column, as an enum; add what is missing, and rename the aliases; the code generated for some languages cannot take the enum's name \"Zone_values\" as it is, or makes it one identifier with another name, so the generated code names the enum c1_kind for now\n"),
    ] {
        let (c, out, e) = run_lang("en", &["import", "csv", csv.to_str().unwrap(), "--name", name]);
        assert_eq!(c, 0, "{e}");
        assert!(out.contains(said), "{name}: {out}");
        let rule = d.join(format!("{name}.rule"));
        std::fs::write(&rule, &out).unwrap();
        let (c, o, _) = run_lang("en", &["check", rule.to_str().unwrap(), "--format", "json"]);
        assert_eq!((c, o.as_str()), (0, ""), "{name}:\n{out}");
    }
}

fn have_python() -> bool {
    Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success())
}

/// `rulec` in `cwd`, with what it writes for a while under `tmp`.
fn rulec_in(cwd: &std::path::Path, tmp: &std::path::Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmp).current_dir(cwd).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// What a draft `check` says nothing of, every language takes: the code generated from the draft of
/// every material here, English and Japanese, builds and answers as the reference evaluator does in
/// every way `rulec test` runs it (§15.266). Each draft is under a name of its own, since a sheet
/// named in Japanese gives its rule the alias `imported`, and the drafts are dealt into as many
/// directories as there are cores, up to four, run side by side.
#[test]
fn the_code_generated_from_every_draft_answers_alike_in_every_language() {
    if !ready(Need::Python, have_python, "python3 is not here") {
        return;
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let t = TempDir::new("import-runs");
    let n = std::thread::available_parallelism().map_or(1, |n| n.get()).clamp(1, 4);
    let dirs: Vec<PathBuf> = (0..n).map(|i| t.path().join(format!("d{i}"))).collect();
    let mut drafts: Vec<(String, String)> = Vec::new();
    let materials = UNREAD.iter().map(|(m, _)| (*m, 1)).chain(RESPELLED.iter().map(|(m, o, _)| (*m, *o)));
    for (m, outputs) in materials {
        for lang in ["en", "ja"] {
            let name = format!("draft_{}_{lang}", drafts.len());
            let csv = root.join(format!("tests/import/{m}.csv"));
            let (c, out, e) = run_lang(lang, &["import", "csv", csv.to_str().unwrap(), "--outputs", &outputs.to_string(), "--name", &name]);
            assert_eq!(c, 0, "{m} {lang}: {e}");
            let dir = &dirs[drafts.len() % n];
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join(format!("{name}.rule")), out).unwrap();
            drafts.push((name, format!("{m} ({lang})")));
        }
    }
    side_by_side(&dirs, |dir| {
        let tmp = tmpdir_in(dir);
        let mut files: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).filter(|f| f.ends_with(".rule")).collect();
        files.sort();
        let mut args: Vec<&str> = vec!["gen"];
        args.extend(files.iter().map(String::as_str));
        args.extend(["--out", "out"]);
        let (c, said, e) = rulec_in(dir, &tmp, &args);
        assert_eq!(c, 0, "gen: {said}{e}");
        let (_, said, e) = rulec_in(dir, &tmp, &["test", "out", "--format", "json"]);
        let j = rulec::json::parse(said.lines().next().unwrap_or_else(|| panic!("no result\n{e}"))).unwrap_or_else(|x| panic!("{x}: {said}"));
        for why in j.get("skipped").and_then(Json::as_arr).unwrap_or_default() {
            skip(&format!("rulec test: {}", why.as_str().unwrap_or_default()));
        }
        let results = j.get("results").and_then(Json::as_arr).unwrap_or_default();
        let field = |r: &Json, k: &str| r.get(k).and_then(Json::as_str).unwrap_or_default().to_string();
        let of = |rule: &str| drafts.iter().find(|(n, _)| n == rule).map_or(rule.to_string(), |(_, m)| m.clone());
        let broken: Vec<String> = results
            .iter()
            .filter(|r| r.get("ok").and_then(|v| v.as_bool()) != Some(true))
            .map(|r| format!("{} {}/{}: {}", of(&field(r, "rule")), field(r, "lang"), field(r, "via"), field(r, "error")))
            .collect();
        assert!(broken.is_empty(), "these did not run or did not agree:\n{}", broken.join("\n"));
        let ran: Vec<String> = results.iter().filter(|r| r.get("ran").and_then(|v| v.as_bool()) == Some(true)).map(|r| field(r, "lang")).collect();
        assert!(ran.iter().any(|l| l == "python"), "Python did not run: {ran:?}");
    });
}
