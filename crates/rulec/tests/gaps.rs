//! E101 names every part of the input no row takes, each as the row that takes it (§15.317).
//!
//! The hint of E101 was one row for one input: a table of amounts from 1 to 1,000 yen with the
//! band above 500 left out was told `| 501JPY | low |`, the output copied from the first row. With
//! that row added the next run asked for 502 yen, and so on, a yen at a time; and a table with
//! gaps in three places learned of the second only after the first was closed. The parts no row
//! takes are read off the table: they end where the table already cuts a column and where the
//! declared range ends. So E101 says them all — in words, and as rows with `?` where the answer
//! goes, which is not the tool's to choose and does not pass the check as it is.
//!
//! The materials are in `tests/gaps`, English first, with a Japanese version beside each:
//! `fee_top_band.rule`, the shape above; `handling.rule`, with an enum, a truth value and an
//! amount, short of rows in three places; `season.rule`, with a date and a number that may be
//! left out; `every_other.rule`, with more parts than one report lists; `sku.rule`, a `policy
//! first` table on a string, whose last row no cell but `-` writes; `surcharge.rule`, two tables
//! checked together; `cap.rule`, a part a `constraint` leaves only some values of;
//! `delivery_day.rule`, a date a table above writes two days of between the boundaries the table
//! below writes; and `packing_fee.rule`, an amount a table above writes one value of for each zone
//! there. What `rulec check` prints for each is its golden file in `tests/golden/gaps`
//! (`RULEC_BLESS=1 cargo test --test gaps` writes them again).

use rulec::diag::{Diag, Severity};
use rulec::i18n::{self, Lang};
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The rules of `tests/gaps`, each with its Japanese version.
const GAPS: &[(&str, &str)] = &[
    ("fee_top_band", "料金の上の帯"),
    ("handling", "取扱手数料"),
    ("season", "シーズンの料金"),
    ("every_other", "一つおきの数"),
    ("sku", "品番"),
    ("surcharge", "割増"),
    ("cap", "上限"),
    ("delivery_day", "配達日"),
    ("packing_fee", "梱包料"),
];

fn source(stem: &str) -> (String, String) {
    let rel = format!("tests/gaps/{stem}.rule");
    let src = std::fs::read_to_string(root().join(&rel)).unwrap_or_else(|_| panic!("cannot read {rel}"));
    (rel, src)
}

/// What `rulec check <rel>` prints, in `lang`.
fn printed(rel: &str, src: &str, lang: Lang) -> String {
    let lines: Vec<String> = src.lines().map(String::from).collect();
    i18n::with(lang, || {
        let r = rulec::report(src, rel);
        format!("{}{}", rulec::findings_text(&r.diags, &lines), rulec::check_tail(&r.shadow, &r.diags, rel, 0))
    })
}

fn errors(src: &str, rel: &str) -> Vec<Diag> {
    i18n::with(Lang::En, || rulec::check_source(src, rel)).into_iter().filter(|d| d.severity == Severity::Error).collect()
}

/// The E101 findings of a rule's text, in `lang`.
fn gaps(src: &str, rel: &str, lang: Lang) -> Vec<Diag> {
    i18n::with(lang, || rulec::check_source(src, rel)).into_iter().filter(|d| d.code == "E101").collect()
}

/// The one E101 of a rule's text, in `lang`.
fn gap(src: &str, rel: &str, lang: Lang) -> Diag {
    let mut ds = gaps(src, rel, lang);
    assert_eq!(ds.len(), 1, "{rel}: one E101 was expected, and there are {}", ds.len());
    ds.remove(0)
}

fn cells(line: &str) -> Vec<String> {
    line.trim().trim_matches('|').split('|').map(|c| c.trim().to_string()).collect()
}

/// The rule with the rows of each E101's `fix.text` written after the last row of the table the
/// finding names, the `?` of each output replaced by what the table's first row answers there: a
/// program doing what the hint says, and answering as someone who knows the rule would not have to.
/// None where no E101 has rows.
fn pasted(src: &str, rel: &str) -> Option<String> {
    let mut lines: Vec<String> = src.lines().map(String::from).collect();
    let mut ds = gaps(src, rel, Lang::En);
    let line_of = |d: &Diag| -> usize { d.where_.strip_prefix(&format!("{rel}:")).and_then(|t| t.split(' ').next()).and_then(|n| n.parse().ok()).unwrap_or_else(|| panic!("{rel}: where is `{}`?", d.where_)) };
    // from the bottom up, so that the lines above stay where the findings say they are
    ds.sort_by_key(|d| std::cmp::Reverse(line_of(d)));
    let mut any = false;
    for d in &ds {
        let Some(text) = d.fix.text.as_deref() else { continue };
        assert_eq!(d.fix.kind.word(), "add_row", "{rel}");
        let mut i = line_of(d);
        while !lines[i].trim_start().starts_with('|') {
            i += 1;
        }
        let head = cells(&lines[i]);
        let outputs = head.len() - head.iter().position(|c| c.starts_with("->")).unwrap_or_else(|| panic!("{rel}: no output column in `{}`", lines[i]));
        let answers = cells(&lines[i + 1])[head.len() - outputs..].to_vec();
        while i < lines.len() && lines[i].trim_start().starts_with('|') {
            i += 1;
        }
        let rows: Vec<String> = text
            .lines()
            .map(|row| {
                let cs = cells(row);
                let (ins, outs) = cs.split_at(cs.len() - outputs);
                assert!(outs.iter().all(|c| c == "?"), "{rel}: the outputs of `{row}` are not left as `?`");
                assert!(!ins.iter().any(|c| c.contains('?')), "{rel}: `{row}` leaves an input cell open");
                format!("| {} |", ins.iter().chain(&answers).cloned().collect::<Vec<_>>().join(" | "))
            })
            .collect();
        lines.splice(i..i, rows);
        any = true;
    }
    any.then(|| lines.join("\n") + "\n")
}

/// The lines of an E101's notes that say the parts in words and the rows to add: the first note
/// and the lines listed under it, and the hint with the lines under it.
fn said(d: &Diag) -> (Vec<String>, Vec<String>) {
    let mut parts = Vec::new();
    let mut rows = Vec::new();
    let mut into: Option<bool> = None;
    for n in &d.notes {
        if n.starts_with("No row matches") || n.starts_with("どの行にも当てはまらない入力") {
            into = Some(true);
            parts.push(n.clone());
        } else if n.starts_with("hint: add this row") || n.starts_with("hint: add these rows") || n.starts_with("ヒント: 次の行") {
            into = Some(false);
            rows.push(n.clone());
        } else if n.starts_with("  ") {
            match into {
                Some(true) => parts.push(n.clone()),
                Some(false) => rows.push(n.clone()),
                None => {}
            }
        } else {
            into = None;
        }
    }
    (parts, rows)
}

fn rulec(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").current_dir(root()).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// Every rule of the directory is listed here with its Japanese version, says one E101 and no
/// other error, and prints what its golden file holds: the English rule in English, the Japanese
/// one in Japanese.
#[test]
fn each_rule_says_its_gap_as_its_golden_file_holds() {
    let mut names: Vec<String> = std::fs::read_dir(root().join("tests/gaps")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    let mut listed: Vec<String> = GAPS.iter().flat_map(|(en, ja)| [format!("{en}.rule"), format!("{ja}.rule")]).collect();
    listed.sort();
    assert_eq!(names, listed, "every rule of tests/gaps is listed here, with its Japanese version");
    let mut failures = Vec::new();
    for (en, ja) in GAPS {
        for (stem, lang, tag) in [(en, Lang::En, "en"), (ja, Lang::Ja, "ja")] {
            let (rel, src) = source(stem);
            let codes: Vec<&str> = errors(&src, &rel).iter().map(|d| d.code).collect();
            if codes != ["E101"] {
                failures.push(format!("{rel}: {codes:?}, not one E101"));
                continue;
            }
            if let Err(e) = ritsu_testkit::golden::check(&root().join(format!("tests/golden/gaps/{stem}.{tag}.txt")), &printed(&rel, &src, lang)) {
                failures.push(e);
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The Japanese version is the same rule under other names: the same rows to add, cell for cell
/// where the cells hold no name, and as many parts.
#[test]
fn the_japanese_version_is_told_the_same_rows() {
    for (en, ja) in GAPS {
        let (re, se) = source(en);
        let (rj, sj) = source(ja);
        let (de, dj) = (gap(&se, &re, Lang::En), gap(&sj, &rj, Lang::Ja));
        let (pe, we) = said(&de);
        let (pj, wj) = said(&dj);
        assert_eq!(pe.len(), pj.len(), "{en} and {ja}: the parts");
        assert_eq!(we.len(), wj.len(), "{en} and {ja}: the rows");
        assert_eq!(de.fix.text.as_deref().map(|t| t.lines().count()), dj.fix.text.as_deref().map(|t| t.lines().count()), "{en} and {ja}");
        assert_eq!(de.fix.kind.word(), dj.fix.kind.word(), "{en} and {ja}");
    }
}

/// The shape the fix began with: amounts from 1 to 1,000 yen, and no row above 500. The row is
/// the band itself, from the boundary the table writes to the end of the range, which a cell
/// leaves unwritten; the words say both ends; and the example is still one amount in it. Before,
/// the row was `| 501JPY | low |`, and the run after it asked for 502.
#[test]
fn a_band_left_out_is_one_row_up_to_the_end_of_the_range() {
    let (rel, src) = source("fee_top_band");
    let d = gap(&src, &rel, Lang::En);
    assert_eq!(d.notes[0], "No row matches: amount from 501JPY to 1000JPY");
    assert_eq!(d.notes[1], "An input that matches no row: amount = 501JPY");
    assert_eq!(d.notes[2], "hint: add this row, with what it answers written in place of `?`: `| >500JPY | ? |`");
    assert!(d.notes[3].contains("no new boundary is chosen, and the rows close the whole gap as they are"), "{:?}", d.notes);
    assert!(d.notes[3].contains("What a row answers is not the tool's to decide"), "{:?}", d.notes);
    assert_eq!(d.notes.len(), 4, "{:?}", d.notes);
    assert_eq!((d.fix.kind.word(), d.fix.text.as_deref()), ("add_row", Some("| >500JPY | ? |")));
    // no output is copied from a row of the table
    assert!(!d.notes.iter().any(|n| n.contains("| low |") || n.contains("| mid |")), "{:?}", d.notes);

    let (rel, src) = source("料金の上の帯");
    let d = gap(&src, &rel, Lang::Ja);
    assert_eq!(d.notes[0], "どの行にも当てはまらない入力: 金額が 501JPY から 1000JPY");
    assert_eq!(d.notes[1], "当てはまらない例: 金額 = 501JPY");
    assert_eq!(d.notes[2], "ヒント: 次の行を足し、`?` に答えを書いてください: `| >500JPY | ? |`");
    assert_eq!(d.fix.text.as_deref(), Some("| >500JPY | ? |"));

    // one run closes it: the row with an answer written in passes, where the row of one amount
    // left 499 amounts to go
    let closed = pasted(&src, &rel).unwrap();
    assert!(errors(&closed, &rel).is_empty(), "{closed}");
}

/// A table of three columns, short of rows in three places: an enum's value no row names, one
/// value's rows for one side of a truth value, and the other side of it in the top band. Each
/// part is a row: the enum's values as their set, the truth value as its side, the amounts between
/// the boundaries the table writes, and `-` where the part is every value of the column.
#[test]
fn each_column_is_cut_where_the_table_cuts_it() {
    let (rel, src) = source("handling");
    let d = gap(&src, &rel, Lang::En);
    let (parts, rows) = said(&d);
    assert_eq!(
        parts,
        [
            "No row matches these 3 parts of the input:",
            "  amount from 101JPY to 500JPY, kind = parcel, rush = false",
            "  amount from 101JPY to 1000JPY, kind = pallet",
            "  amount from 501JPY to 1000JPY, kind one of letter, parcel, rush = true",
        ]
    );
    assert_eq!(
        rows,
        [
            "hint: add these rows, with what each answers written in place of `?`:",
            "  | >100JPY <=500JPY | parcel | false | ? | ? |",
            "  | >100JPY | pallet | - | ? | ? |",
            "  | >500JPY | letter, parcel | true | ? | ? |",
        ]
    );
    assert_eq!(d.fix.text.as_deref(), Some("| >100JPY <=500JPY | parcel | false | ? | ? |\n| >100JPY | pallet | - | ? | ? |\n| >500JPY | letter, parcel | true | ? | ? |"));

    let (rel, src) = source("取扱手数料");
    let d = gap(&src, &rel, Lang::Ja);
    let (parts, rows) = said(&d);
    assert_eq!(
        parts,
        [
            "どの行にも当てはまらない入力が 3 か所あります:",
            "  金額が 101JPY から 500JPY、種類が 小包、急ぎが false",
            "  金額が 101JPY から 1000JPY、種類が パレット",
            "  金額が 501JPY から 1000JPY、種類が 手紙, 小包 のどれか、急ぎが true",
        ]
    );
    assert_eq!(
        rows,
        ["ヒント: 次の行を足し、`?` に答えを書いてください:", "  | >100JPY <=500JPY | 小包 | false | ? | ? |", "  | >100JPY | パレット | - | ? | ? |", "  | >500JPY | 手紙, 小包 | true | ? | ? |",]
    );
}

/// Dates, and a number that may be left out: the days are cut at the day the table writes, and
/// the ages that are there are apart from the age that is not, which has its own row already.
#[test]
fn a_date_and_a_value_that_may_be_left_out_are_cut_the_same_way() {
    let (rel, src) = source("season");
    let d = gap(&src, &rel, Lang::En);
    let (parts, rows) = said(&d);
    assert_eq!(parts, ["No row matches these 2 parts of the input:", "  day from 2026-04-01 to 2026-06-30, age from 13 to 120", "  day from 2026-07-01 to 2026-09-30",]);
    assert_eq!(rows, ["hint: add these rows, with what each answers written in place of `?`:", "  | <2026-07-01 | >=13 | ? |", "  | >=2026-07-01 | - | ? |",]);

    let (rel, src) = source("シーズンの料金");
    let d = gap(&src, &rel, Lang::Ja);
    let (parts, _) = said(&d);
    assert_eq!(parts, ["どの行にも当てはまらない入力が 2 か所あります:", "  日が 2026-04-01 から 2026-06-30、年齢が 13 から 120", "  日が 2026-07-01 から 2026-09-30",]);

    // a column with no row for the value left out, and none for the values from 5 up: two parts
    // of one column, and two rows
    let (rel, src) = (String::from("tests/optional/E101_a_number_with_one_row.rule"), std::fs::read_to_string(root().join("tests/optional/E101_a_number_with_one_row.rule")).unwrap());
    let d = gap(&src, &rel, Lang::En);
    let (parts, rows) = said(&d);
    assert_eq!(parts, ["No row matches these 2 parts of the input:", "  bonus left out", "  bonus 5 or more"]);
    assert_eq!(rows.len(), 3, "{rows:?}");
    assert!(rows[1].starts_with("  | none |") && rows[2].starts_with("  | >=5 |"), "{rows:?}");
}

/// Written into the table with an answer in place of each `?`, the rows close every gap of the
/// rule in one run: of the materials here, and of every rule under `tests/mutants` and
/// `tests/optional` that says E101. Nothing but E101 is asked of those: some hold another error
/// of their own, which the rows have nothing to do with.
#[test]
fn the_rows_close_the_gap_in_one_run() {
    for (en, ja) in GAPS {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            let d = gap(&src, &rel, Lang::En);
            match *en {
                // more parts than a report lists, and tables checked together: below
                "every_other" | "surcharge" => continue,
                _ => {}
            }
            assert!(d.fix.text.is_some(), "{rel}: no rows");
            let closed = pasted(&src, &rel).unwrap();
            let left: Vec<String> = errors(&closed, &rel).iter().map(|d| format!("{} {}", d.code, d.title)).collect();
            assert!(left.is_empty(), "{rel}: {left:?}\n{closed}");
        }
    }
    let mut tried = 0;
    for dir in ["tests/mutants", "tests/optional"] {
        let mut names: Vec<String> = std::fs::read_dir(root().join(dir)).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".rule")).collect();
        names.sort();
        for name in names {
            let rel = format!("{dir}/{name}");
            let src = std::fs::read_to_string(root().join(&rel)).unwrap();
            let before = gaps(&src, &rel, Lang::En);
            if before.is_empty() {
                continue;
            }
            assert!(before.iter().all(|d| d.fix.text.is_some()), "{rel}: an E101 with no rows");
            let closed = pasted(&src, &rel).unwrap();
            let left = gaps(&closed, &rel, Lang::En);
            assert!(left.is_empty(), "{rel}: {:?}\n{closed}", left.iter().map(|d| d.notes.clone()).collect::<Vec<_>>());
            tried += 1;
        }
    }
    assert!(tried >= 20, "only {tried} rules under tests/mutants and tests/optional say E101");
}

/// A part no row takes is said by the values that occur in it (§15.325). No row takes a cap above
/// 500 yen, and a `constraint` keeps the cap under the amount, so such a cap comes only with an
/// amount above 500 yen too: the words and the row say both. Before, they said every amount
/// (`| >500JPY | - | ? |`), as though a cap of 600 yen came with an amount of 100 yen. The row
/// closes the gap all the same, since no input reaches the rest of what `-` took.
#[test]
fn a_part_is_said_by_the_values_that_occur_in_it() {
    let (rel, src) = source("cap");
    let d = gap(&src, &rel, Lang::En);
    assert_eq!(d.notes[0], "No row matches: cap from 501JPY to 1000JPY, amount from 501JPY to 1000JPY");
    assert_eq!(d.notes[2], "hint: add this row, with what it answers written in place of `?`: `| >500JPY | >500JPY | ? |`");
    assert_eq!(d.fix.text.as_deref(), Some("| >500JPY | >500JPY | ? |"));
    let closed = pasted(&src, &rel).unwrap();
    assert!(errors(&closed, &rel).is_empty(), "{closed}");

    let (rel, src) = source("上限");
    let d = gap(&src, &rel, Lang::Ja);
    assert_eq!(d.notes[0], "どの行にも当てはまらない入力: 上限が 501JPY から 1000JPY、金額が 501JPY から 1000JPY");
    assert_eq!(d.fix.text.as_deref(), Some("| >500JPY | >500JPY | ? |"));

    // A value a table above never gives with the rest of the part: the purchase route without its
    // row for goods up to a million yen that finance does not review. The table of authority has
    // finance review every purchase of goods above a million yen, so goods it does not review come
    // only up to a million yen, and the part says so. Before, it said goods finance does not review
    // at every amount (`| goods | - | false | ? | ? | ? |`).
    for (rel, lang, row, words, cells) in [
        (
            "tests/computed/purchase_route.rule",
            Lang::En,
            "| goods    | <=1_000_000JPY | false",
            "No row matches: kind = goods, amount from 1JPY to 1_000_000JPY, authority_finance = false",
            "| goods | <=1_000_000JPY | false | ? | ? | ? |",
        ),
        (
            "tests/computed/購買の経路.rule",
            Lang::Ja,
            "| 物品         | <=1_000_000円 | false",
            "どの行にも当てはまらない入力: 種類が 物品、金額が 1円 から 1_000_000円、経理を通すが false",
            "| 物品 | <=1_000_000円 | false | ? | ? | ? |",
        ),
    ] {
        let full = std::fs::read_to_string(root().join(rel)).unwrap();
        let src: String = full.lines().take_while(|l| !l.starts_with("examples")).filter(|l| !l.starts_with(row)).map(|l| format!("{l}\n")).collect();
        let d = gap(&src, rel, lang);
        assert_eq!(d.notes[0], words, "{rel}");
        assert_eq!(d.fix.text.as_deref(), Some(cells), "{rel}");
        let closed = pasted(&src, rel).unwrap();
        assert!(errors(&closed, rel).is_empty(), "{rel}\n{closed}");
    }
}

/// A row pasted and left as it is does not pass: the `?` is no value, and the check stops on the
/// cell (E063) instead of answering what nobody chose.
#[test]
fn a_row_left_with_its_question_mark_does_not_pass() {
    let (rel, src) = source("fee_top_band");
    let d = gap(&src, &rel, Lang::En);
    let with = format!("{}{}\n", src, d.fix.text.unwrap());
    let codes: Vec<&str> = errors(&with, &rel).iter().map(|d| d.code).collect();
    assert_eq!(codes, ["E063"], "{with}");
}

/// Ten parts, more than a report lists: the first eight are written out with their rows, the
/// rest are counted, and the last note says that these rows close what is listed and no more.
/// The run after them names the two that were left, and says they close it.
#[test]
fn more_parts_than_are_listed_are_counted_and_the_next_run_names_the_rest() {
    for (stem, lang, count, more, only, whole) in [
        ("every_other", Lang::En, "No row matches these 10 parts of the input:", "  and 2 more", "The rows listed close the parts listed, and the next run names what is left.", "the rows close the whole gap as they are"),
        ("一つおきの数", Lang::Ja, "どの行にも当てはまらない入力が 10 か所あります:", "  ほかに 2 か所", "ここに挙げた行で埋まるのは挙げた所だけで、残りは次の実行で出ます。", "このまま足せば穴は全部埋まります"),
    ] {
        let (rel, src) = source(stem);
        let d = gap(&src, &rel, lang);
        let (parts, rows) = said(&d);
        assert_eq!(parts[0], count);
        assert_eq!(parts.len(), 10, "{parts:?}");
        assert_eq!(parts[9], more);
        assert_eq!(rows[1..], ["  | 1 | ? |", "  | 3 | ? |", "  | 5 | ? |", "  | 7 | ? |", "  | 9 | ? |", "  | 11 | ? |", "  | 13 | ? |", "  | 15 | ? |"]);
        assert_eq!(d.fix.text.as_deref().map(|t| t.lines().count()), Some(8));
        assert!(d.notes.last().unwrap().contains(only) && !d.notes.last().unwrap().contains(whole), "{:?}", d.notes.last());

        let once = pasted(&src, &rel).unwrap();
        let d = gap(&once, &rel, lang);
        let (parts, rows) = said(&d);
        assert_eq!(parts.len(), 3, "{parts:?}");
        assert_eq!(rows[1..], ["  | 17 | ? |", "  | 19 | ? |"]);
        assert!(d.notes.last().unwrap().contains(whole), "{:?}", d.notes.last());
        let twice = pasted(&once, &rel).unwrap();
        assert!(errors(&twice, &rel).is_empty(), "{twice}");
    }
}

/// A string column holds `starts_with` and `-`, and no cell names the strings under none of the
/// prefixes. In a `policy first` table the row that takes them is the one written last with `-`,
/// and E101 gives that row and says where it goes. Under `policy unique` that row would overlap
/// the others, so there is no row to paste: the hint says why, and what a `policy first` table
/// would write. Before, the row was `| starts_with "z" | frozen |`, and the next run asked for
/// another string.
#[test]
fn where_no_cell_names_a_part_the_row_is_the_last_of_a_first_match_table() {
    let (rel, src) = source("sku");
    let d = gap(&src, &rel, Lang::En);
    assert_eq!(d.notes[0], "No row matches: code starting with none of the prefixes the table writes");
    assert_eq!(d.notes[2], "hint: add this row, with what it answers written in place of `?`: `| - | ? |`");
    assert!(d.notes[3].starts_with("Add it after the rows the table has: under `policy first` a row takes what the rows above it leave. It writes `-` in a column where no cell takes exactly the part"), "{:?}", d.notes);
    assert_eq!(d.fix.text.as_deref(), Some("| - | ? |"));

    let (rj, sj) = source("品番");
    let dj = gap(&sj, &rj, Lang::Ja);
    assert_eq!(dj.notes[0], "どの行にも当てはまらない入力: 品番が、表の書くどの前置きでも始まらない");
    assert!(dj.notes[3].starts_with("この行は、表の今の行の後ろに足してください。`policy first` の表では、上の行に当てはまらなかった入力を下の行が取ります。"), "{:?}", dj.notes);

    // the same table under `policy unique`
    let unique = src.replace("policy first\n", "");
    assert_ne!(unique, src);
    let d = gap(&unique, &rel, Lang::En);
    assert_eq!((d.fix.kind.word(), d.fix.text.as_deref()), ("none", None));
    assert!(d.notes[2].starts_with("hint: no cell takes exactly these inputs (a string column holds `starts_with` and `-` only"), "{:?}", d.notes);
    assert!(d.notes[2].ends_with("A table under `policy first` takes them with this row, written after every other row: `| - | ? |`"), "{:?}", d.notes);
    assert!(!d.notes.iter().any(|n| n.contains("close the whole gap")), "{:?}", d.notes);

    // Prefixes the table writes longer ones of, across a second column: the class under "A" and
    // not under "A1" is no cell's either, and `starts_with "A"` written last takes it. The part
    // under none of the prefixes needs no row of its own here — the three before it take it —
    // and a row no input reaches would be an error of its own (E102).
    let src = "rule codes v1\ndescription \"lanes by code and count\"\n\nenum lane = frozen | chilled | dry\n\ninputs\n  code  : string\n  units : number  range >=1 <=30\n\noutputs\n  lane : lane\n\ntable lanes\npolicy first\n| code             | units | -> lane |\n| starts_with \"A1\" | -     | frozen  |\n| starts_with \"B1\" | -     | frozen  |\n| starts_with \"A\"  | >20   | chilled |\n| starts_with \"B\"  | <=10  | chilled |\n| starts_with \"C\"  | -     | dry     |\n";
    let d = gap(src, "codes.rule", Lang::En);
    let (parts, rows) = said(&d);
    assert_eq!(
        parts,
        [
            "No row matches these 4 parts of the input:",
            "  code starting with \"A\" (but not \"A1\"), units from 1 to 10",
            "  code starting with \"A\" (but not \"A1\") or \"B\" (but not \"B1\"), units from 11 to 20",
            "  code starting with \"B\" (but not \"B1\"), units from 21 to 30",
            "  code starting with none of the prefixes the table writes",
        ]
    );
    assert_eq!(rows[1..], ["  | starts_with \"A\" | <=10 | ? |", "  | starts_with \"A\", \"B\" | >10 <=20 | ? |", "  | starts_with \"B\" | >20 | ? |", "  | - | - | ? |"]);
    let closed = pasted(src, "codes.rule").unwrap();
    let left: Vec<String> = errors(&closed, "codes.rule").iter().map(|d| format!("{} {}", d.code, d.title)).collect();
    assert!(left.is_empty(), "{left:?}\n{closed}");

    // with every count of the codes under no prefix taken by the rows before it, the last row
    // is left out
    let narrower = src.replace("| starts_with \"C\"  | -     | dry     |\n", "| starts_with \"C\"  | -     | dry     |\n| -                | >10 <=20 | dry |\n");
    let d = gap(&narrower, "codes.rule", Lang::En);
    let (_, rows) = said(&d);
    assert!(!rows.iter().any(|r| r.trim() == "| - | - | ? |"), "{rows:?}");
    let closed = pasted(&narrower, "codes.rule").unwrap();
    let left: Vec<String> = errors(&closed, "codes.rule").iter().map(|d| format!("{} {}", d.code, d.title)).collect();
    assert!(left.is_empty(), "{left:?}\n{closed}");
}

/// A boundary the table writes is written in the row the way the table writes it: under rows of
/// `<=2kg` and `>5kg` the row is `>2kg <=5kg`, not the same in grams. A boundary no cell of the
/// column writes — here the end of the range, of a column that may be left out — is written in
/// the unit the values are counted in.
#[test]
fn a_boundary_is_written_the_way_the_table_writes_it() {
    let src = "rule fee v1\ndescription \"a fee by destination and weight\"\n\nenum zone = domestic | overseas\n\ninputs\n  destination : zone\n  weight      : mass[g]  range >=1g <=10kg\n\noutputs\n  fee : money[USD]  round up(1USD)\n\ntable fee_table\npolicy unique\n| destination | weight     | -> fee |\n| domestic    | <=2kg      | 8USD   |\n| domestic    | >2kg <=5kg | 10USD  |\n| domestic    | >5kg       | 13USD  |\n| overseas    | <=2kg      | 12USD  |\n| overseas    | >5kg       | 20USD  |\n";
    let d = gap(src, "fee.rule", Lang::En);
    assert_eq!(d.notes[0], "No row matches: destination = overseas, weight from 2001g to 5000g");
    assert_eq!(d.fix.text.as_deref(), Some("| overseas | >2kg <=5kg | ? |"));
    let closed = pasted(src, "fee.rule").unwrap();
    let left: Vec<String> = errors(&closed, "fee.rule").iter().map(|d| format!("{} {}", d.code, d.title)).collect();
    assert!(left.is_empty(), "{left:?}\n{closed}");

    // the same boundary written two ways is given the way the first cell writes it
    let two = src.replace("| overseas    | >5kg       | 20USD  |", "| overseas    | >5000g     | 20USD  |").replace("| domestic    | >5kg       | 13USD  |", "| domestic    | >5000g     | 13USD  |");
    assert_eq!(gap(&two, "fee.rule", Lang::En).fix.text.as_deref(), Some("| overseas | >2kg <=5kg | ? |"));

    // an end of the range no cell writes
    let optional = "rule fee v1\ndescription \"a fee by destination and weight\"\n\nenum zone = domestic | overseas\n\ninputs\n  destination : zone\n  weight      : mass[g]?  range >=1g <=10kg\n\noutputs\n  fee : money[USD]  round up(1USD)\n\ntable fee_table\npolicy unique\n| destination | weight | -> fee |\n| domestic    | -      | 8USD   |\n| overseas    | none   | 12USD  |\n";
    let d = gap(optional, "fee.rule", Lang::En);
    assert_eq!(d.notes[0], "No row matches: destination = overseas, weight from 1g to 10_000g");
    assert_eq!(d.fix.text.as_deref(), Some("| overseas | >=1g | ? |"), "{:?}", d.notes);
}

/// W111 beside an E101 does not say the completeness check passes. Where the rows E101 asks for
/// name the values no row names, the warning would say the same thing a second time, and is left
/// out. Where the table writes the enum, its note says the rows the table lacks may be where the
/// values go; where it reads the enum and the rows asked for do not name the values, that the
/// values may come to be taken by a `-` once the table passes.
#[test]
fn a_value_no_row_names_is_not_said_twice_beside_a_gap() {
    let unused = |src: &str, rel: &str, lang: Lang| -> Vec<Diag> { i18n::with(lang, || rulec::check_source(src, rel)).into_iter().filter(|d| d.code == "W111").collect() };
    // the rows asked for name `pallet`: no W111
    for stem in ["handling", "取扱手数料"] {
        let (rel, src) = source(stem);
        assert!(unused(&src, &rel, Lang::En).is_empty(), "{rel}");
        // with the gaps closed by rows that do not name it, the warning is back as it was
        let closed = pasted(&src, &rel).unwrap();
        assert!(errors(&closed, &rel).is_empty(), "{rel}");
    }
    // the table writes the enum
    for (stem, lang, note) in [
        ("fee_top_band", Lang::En, "Table levels, which writes this type, is short of rows (E101): what the rows it lacks answer may be where those values belong."),
        ("料金の上の帯", Lang::Ja, "この型を書く表 段階の表 には、行が足りません（E101）。その値は、足りない行の答えかもしれません。"),
    ] {
        let (rel, src) = source(stem);
        let ws = unused(&src, &rel, lang);
        assert_eq!(ws.len(), 1, "{rel}");
        assert_eq!(ws[0].notes.last().map(String::as_str), Some(note), "{rel}");
    }
    // the table reads the enum, and the row asked for takes every kind
    let src = "rule fee v1\ndescription \"a fee by kind and amount\"\n\nenum kind = letter | parcel | pallet\n\ninputs\n  kind   : kind\n  amount : money[JPY]  range >=1JPY <=1_000JPY\n\noutputs\n  fee : money[JPY]  round down(1JPY)\n\ntable fees\npolicy first\n| kind   | amount   | -> fee |\n| letter | <=500JPY | 10JPY  |\n| -      | <=500JPY | 20JPY  |\n";
    assert_eq!(gap(src, "fee.rule", Lang::En).fix.text.as_deref(), Some("| - | >500JPY | ? |"));
    for (lang, notes) in [
        (Lang::En, ["Values that never appear: parcel / pallet", "Table fees, which reads this type, does not pass the completeness check (E101). Once it does, the rows for those values may simply be absorbed by a `-`."]),
        (Lang::Ja, ["現れない値: parcel / pallet", "この型を読む表 fees は、完全性の検査を通っていません（E101）。通ったあとも、その値は `-` の行に当たっているだけかもしれません。"]),
    ] {
        let ws = unused(src, "fee.rule", lang);
        assert_eq!(ws.len(), 1);
        assert_eq!(ws[0].notes, notes);
    }
    // with no gap, the note is the one it was
    let whole = format!("{src}| -      | >500JPY  | 30JPY  |\n");
    assert!(errors(&whole, "fee.rule").is_empty());
    let ws = unused(&whole, "fee.rule", Lang::En);
    assert_eq!(ws[0].notes[1], "Even though the completeness check passes, the rows for those values may simply be absorbed by a `-`.");
}

/// Tables checked together are told the parts in words and given no rows: a row may go in any
/// of them, and which is theirs to decide.
#[test]
fn tables_checked_together_are_told_the_parts_and_given_no_rows() {
    for (stem, lang, part) in [("surcharge", Lang::En, "No row matches: weight from 21kg to 50kg, fragile = false"), ("割増", Lang::Ja, "どの行にも当てはまらない入力: 重さが 21kg から 50kg、割れ物が false")] {
        let (rel, src) = source(stem);
        let d = gap(&src, &rel, lang);
        assert_eq!(d.notes[0], part);
        assert_eq!((d.fix.kind.word(), d.fix.text.as_deref()), ("none", None));
        assert!(!d.notes.iter().any(|n| n.contains("| ?")), "{:?}", d.notes);
    }
}

/// Where the definitions of what a rule reads rule the parts out, every one of them gets its
/// `never` line, and the lines close the gap with no row added. Before, the line was the one of
/// the example, and the run after it gave the next.
#[test]
fn every_part_the_definitions_rule_out_gets_its_never_line() {
    let rel = "tests/mutants/m_e101never.rule";
    let src = std::fs::read_to_string(root().join(rel)).unwrap();
    let d = gap(&src, rel, Lang::En);
    let at = d.notes.iter().position(|n| n.starts_with("hint: the combinations that do not happen come out of the tables with these lines")).unwrap_or_else(|| panic!("{:?}", d.notes));
    let lines = [&d.notes[at + 1], &d.notes[at + 2]].map(|l| l.trim().to_string());
    assert_eq!(lines, ["never has_fragile = true, none_fragile = true", "never has_fragile = false, none_fragile = false"]);
    let with = src.replace("table handling\n", &format!("{}\n{}\n\ntable handling\n", lines[0], lines[1]));
    assert_ne!(with, src);
    assert!(errors(&with, rel).is_empty(), "{with}");
    // one line leaves the other part, which is then the one line the hint gives
    let one = src.replace("table handling\n", &format!("{}\n\ntable handling\n", lines[0]));
    let d = gap(&one, rel, Lang::En);
    assert!(d.notes.iter().any(|n| n.ends_with(&format!("with this line, and no row: `{}`", lines[1]))), "{:?}", d.notes);
}

/// `--format json` carries the same: the parts and the rows in `notes`, and the rows in
/// `fix.text`, one to a line, with nothing but the rows in it.
#[test]
fn the_json_carries_the_rows_one_to_a_line() {
    let (code, out, err) = rulec(&["check", "tests/gaps/handling.rule", "--format", "json"]);
    assert_eq!(code, 1, "{out}{err}");
    let line = out.lines().find(|l| l.contains("\"code\":\"E101\"")).unwrap_or_else(|| panic!("no E101: {out}"));
    let j = rulec::json::parse(line).unwrap_or_else(|e| panic!("{e}: {line}"));
    let fix = j.get("fix").unwrap_or_else(|| panic!("no fix: {line}"));
    assert_eq!(fix.get("kind").and_then(|k| k.as_str()), Some("add_row"));
    assert_eq!(fix.get("text").and_then(|k| k.as_str()), Some("| >100JPY <=500JPY | parcel | false | ? | ? |\n| >100JPY | pallet | - | ? | ? |\n| >500JPY | letter, parcel | true | ? | ? |"));
    let notes: Vec<&str> = j.get("notes").and_then(|n| n.as_arr()).unwrap_or_else(|| panic!("no notes: {line}")).iter().filter_map(|n| n.as_str()).collect();
    assert!(notes.contains(&"  amount from 101JPY to 1000JPY, kind = pallet"), "{notes:?}");
    assert!(notes.contains(&"  | >100JPY | pallet | - | ? | ? |"), "{notes:?}");
    // the example of one input is still there, for a program that wants a request to try
    assert!(line.contains("\"witness\":{\"inputs\":{\"amount\":501,\"kind\":\"letter\",\"rush\":true}"), "{line}");
}

/// A cut of a column a table above decides, in which that table writes a few values only, is said
/// as those values (§15.331). The days table writes the 4th and the 6th of August between the 1st
/// and the 10th, the boundaries of the table below, so the column takes no other day there: the
/// rows are a day each, and they close the gap as the cut would. Before, the row was the cut,
/// `| >2026-08-01 <2026-08-10 | ? |`, which asks for an answer on days that never come, and the
/// words said every day from the 2nd to the 9th. A value the table above writes only with what a
/// row takes already is not said: 700 yen comes with the local zone alone, which a row takes.
#[test]
fn a_cut_a_table_above_writes_a_few_values_in_is_said_as_those_values() {
    for (stem, lang, parts) in [
        ("delivery_day", Lang::En, ["No row matches these 2 parts of the input:", "  day = 2026-08-04", "  day = 2026-08-06"]),
        ("配達日", Lang::Ja, ["どの行にも当てはまらない入力が 2 か所あります:", "  配達日が 2026-08-04", "  配達日が 2026-08-06"]),
    ] {
        let (rel, src) = source(stem);
        let d = gap(&src, &rel, lang);
        assert_eq!(said(&d).0, parts, "{rel}");
        assert_eq!(d.fix.text.as_deref(), Some("| 2026-08-04 | ? |\n| 2026-08-06 | ? |"), "{rel}");
    }
    let (rel, src) = source("delivery_day");
    let d = gap(&src, &rel, Lang::En);
    assert!(
        d.notes.iter().any(|n| n.starts_with("The cells are cut at the boundaries the table already writes, at the ends of the ranges, and at the values the tables above produce, the only values those columns take there: the rows close the whole gap as they are.")),
        "{:?}",
        d.notes
    );
    let (rel, src) = source("配達日");
    let d = gap(&src, &rel, Lang::Ja);
    assert!(d.notes.iter().any(|n| n.starts_with("行のセルは、表がすでに書いている境と範囲の端、それに上流の表が出す値で切ってあります。")), "{:?}", d.notes);

    // A day written by a row of a `policy first` table that the row before it takes all of never
    // comes, and is not said: the `-` row writes the 10th for every zone but the three above it.
    let (rel, src) = source("delivery_day");
    let first = src.replace("table days\n", "table days\npolicy first\n").replace("| national | 2026-08-10    |\n", "| -        | 2026-08-10    |\n| national | 2026-08-07    |\n");
    assert_eq!(first.matches("2026-08-07").count(), 1);
    let d = gap(&first, &rel, Lang::En);
    assert_eq!(d.fix.text.as_deref(), Some("| 2026-08-04 | ? |\n| 2026-08-06 | ? |"), "{:?}", d.notes);
    assert!(!d.notes.iter().any(|n| n.contains("2026-08-07")), "{:?}", d.notes);

    for (stem, lang, row) in [("packing_fee", Lang::En, "| 800JPY | national | ? |"), ("梱包料", Lang::Ja, "| 800JPY | 県外 | ? |")] {
        let (rel, src) = source(stem);
        let d = gap(&src, &rel, lang);
        assert_eq!(d.fix.text.as_deref(), Some(row), "{rel}: {:?}", d.notes);
        // one input: the example says it, and no line says the part apart from it
        assert!(said(&d).0.is_empty(), "{rel}: {:?}", d.notes);
        assert!(!d.notes.iter().any(|n| n.contains("700JPY")), "{rel}: {:?}", d.notes);
    }

    // With no row for the local zone either, each value comes with the zone it comes with, and
    // neither with the other: 700 yen is the local zone's, 800 yen the national one's.
    let (rel, src) = source("packing_fee");
    let neither = src.replace("| >500JPY <1_000JPY | local | standard |\n", "");
    assert_ne!(neither, src);
    let d = gap(&neither, &rel, Lang::En);
    assert_eq!(said(&d).0, ["No row matches these 2 parts of the input:", "  fee = 700JPY, zone = local", "  fee = 800JPY, zone = national"], "{:?}", d.notes);
    assert_eq!(d.fix.text.as_deref(), Some("| 700JPY | local | ? |\n| 800JPY | national | ? |"), "{:?}", d.notes);
    let closed = pasted(&neither, &rel).unwrap();
    assert!(errors(&closed, &rel).is_empty(), "{closed}");

    // A row above that writes a name may write any value: the cut is the row, as before.
    let named = src.replace("  zone : zone\n", "  zone : zone\n  quoted : money[JPY]  range >=0JPY <=2_000JPY\n").replace(
        "| large  | -        | 1_500JPY            |",
        "| large  | -        | quoted              |",
    );
    assert_eq!(named.matches("quoted").count(), 2);
    let d = gap(&named, &rel, Lang::En);
    assert_eq!(d.fix.text.as_deref(), Some("| >500JPY <1_000JPY | national | ? |"), "{:?}", d.notes);
    assert!(d.notes.iter().any(|n| n.contains("no new boundary is chosen, and the rows close the whole gap as they are")), "{:?}", d.notes);
    let closed = pasted(&named, &rel).unwrap();
    assert!(errors(&closed, &rel).is_empty(), "{closed}");

    // The same through `apply`: the table above is another rule's, and the table below has no row
    // from the 1st of July on, where it writes the 3rd.
    for (rel, row) in [("tests/computed/shapes/date_output.rule", "| >=2026-07-03 | late  |\n"), ("tests/computed/shapes/日付の出力.rule", "| >=2026-07-03 | 遅い    |\n")] {
        let src = std::fs::read_to_string(root().join(rel)).unwrap();
        let without = src.replace(row, "");
        assert_ne!(without, src, "{rel}");
        let d = gap(&without, rel, Lang::En);
        assert_eq!(d.fix.text.as_deref(), Some("| 2026-07-03 | ? |"), "{rel}: {:?}", d.notes);
        let closed = pasted(&without, rel).unwrap();
        assert!(errors(&closed, rel).is_empty(), "{closed}");
    }
}
