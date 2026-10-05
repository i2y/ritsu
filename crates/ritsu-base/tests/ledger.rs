//! The ledger (DESIGN 4.3): written out as koyomi, yuen and sakai write theirs, letter for
//! letter. The entries here are copied from the three ledgers, and what they print is held to
//! what the three printed (`tests/golden/compat/`, taken from their `explain` before the move to
//! this crate).

use ritsu_base::ledger::{self, Entry, Ledger, Repro};
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use std::path::Path;

const HOLIDAYS: (&str, &[u8]) = ("holidays.csv", "2026-01-01,元日\n2026-05-04,みどりの日\n".as_bytes());

/// koyomi's and yuen's entries: a file that prints the code, and what is beside it.
fn e(code: &'static str, title: Text, when: Text, fix: Text, example: &'static str, related: &'static [&'static str]) -> Entry {
    Entry::new(code, title, when, fix, Repro::File { body: example, beside: &[] }, related)
}

fn with(mut e: Entry, files: &'static [(&'static str, &'static [u8])]) -> Entry {
    if let Repro::File { beside, .. } = &mut e.repro {
        *beside = files;
    }
    e
}

fn later(mut e: Entry) -> Entry {
    e.repro = Repro::Later;
    e
}

const SAKAI_BASE: &[(&str, &str)] = &[
    ("地図.ctx", "map 地図(m) v1\nuse context \"甲.ctx\"\nuse context \"乙.ctx\"\ncovers \".\"\n"),
    ("甲.ctx", "context 甲(a) v1\nowns\n  dir \"a\"\n"),
    (
        "乙.ctx",
        "context 乙(b) v1\nowns\n  dir \"b\"\n\npublished language b.v1\n  proto \"b/v1/b.proto\"\n  open host service BService\n\nterms\n  種類 \"乙が扱うものの種類\"\n    means enum Kind\n",
    ),
    ("a/a.proto", "syntax = \"proto3\";\npackage a;\nmessage A {}\n"),
    (
        "b/v1/b.proto",
        "syntax = \"proto3\";\npackage b.v1;\nenum Kind {\n  KIND_UNSPECIFIED = 0;\n  KIND_ONE = 1;\n  KIND_TWO = 2;\n}\nmessage B { Kind kind = 1; }\nmessage Plain { string id = 1; }\nservice BService { rpc Get(B) returns (B); }\n",
    ),
];

/// sakai's entries: files laid over its base map, and `sakai check .` run there.
fn s(code: &'static str, title: Text, when: Text, fix: Text, files: &'static [(&'static str, &'static str)], related: &'static [&'static str]) -> Entry {
    let mut all: Vec<(&'static str, &'static str)> = SAKAI_BASE.iter().map(|(p, b)| files.iter().find(|(q, _)| q == p).copied().unwrap_or((*p, *b))).collect();
    all.extend(files.iter().filter(|(q, _)| !SAKAI_BASE.iter().any(|(p, _)| p == q)).copied());
    Entry::new(code, title, when, fix, Repro::Dir { files: all, command: vec!["check", "."] }, related)
}

fn koyomi() -> Ledger {
    Ledger {
        tool: "koyomi",
        example_file: "example.cal",
        fence: "cal",
        repro_heading: tr!("再現", "Example"),
        later_text: Text::default(),
        later_markdown: Text::default(),
        entries: vec![
            with(
                e(
            "E102",
            tr!("出典が固定されていません", "A source is not pinned"),
            tr!("表の出典の行か、法令の条の固定の行に `sha256:` が無いとき。", "A table's line, or the pin line of a law's article, has no `sha256:`."),
            tr!("コピーの SHA-256 の先頭 16 桁を `sha256:` で書きます（直し方に、いまのコピーの値を書いた行が出ます）。", "Pin it with the first 16 digits of the copy's SHA-256; the fix gives the line with the copy's own."),
            "calendar t v1\n\nsource 休み = file \"holidays.csv\"\n  format csv\n  covers 2026-01-01..2026-12-31\n\nclosed 休み\n",
            &["E101", "E103"],
                ),
                &[HOLIDAYS],
            ),
            e(
            "E305",
            tr!("検査の予算を超えました（確かめていません）", "The check is over its budget (nothing was checked)"),
            tr!("入力の組み合わせの数（日付の範囲の日数と、整数の入力の範囲の大きさの積）が予算を超えるとき。一部だけ試して通すことはしません。", "The input combinations (the days of the date's range times the size of every integer input's range) are more than the budget. It never tries some and passes."),
            tr!("範囲を狭めるか、ファイルを分けるか、`--budget` で予算を上げます。", "Narrow a range, split the file, or raise the budget with `--budget`."),
            "dates t v1\n\ninputs\n  d : date  range >=0001-01-01 <=9999-12-31\n  n : int   range >=1 <=100\n\ndate x = d\n  + n days\n",
            &[],
            ),
        ],
    }
}

fn yuen() -> Ledger {
    let later_note = tr!("（再現には一式のツールが要ります。yuen がツールを読むようになったら足します。）", "(The reproduction needs the suite's tools; it is added when yuen reads them.)");
    Ledger {
        tool: "yuen",
        example_file: "example.req",
        fence: "req",
        repro_heading: tr!("再現", "Example"),
        later_text: later_note.clone(),
        later_markdown: later_note,
        entries: vec![later(e("E107", tr!("要件と成果物が、同じ条の違う本文を読んでいます", "A requirement and what meets it read different texts of one article"), tr!("要件が引く条を、それを満たす成果物も固定していて、どのコピーも要件のコピーと本文が違うとき。どちらかが古いコピーです。", "What meets a requirement pins an article the requirement cites, and none of its copies has the text of the requirement's copy: one of them is old."), tr!("本文の差分を読み、古いほうのコピーを取り直して固定し直します。", "Read the diff of the texts, then fetch and pin the older copy again."), "", &["E103"]))],
    }
}

fn sakai() -> Ledger {
    Ledger {
        tool: "sakai",
        example_file: "",
        fence: "ctx",
        repro_heading: tr!("再現", "Reproduction"),
        later_text: tr!("（sakai はこのコードをまだ出さないので、再現はありません）", "(sakai does not print this code yet; it has no reproduction)"),
        later_markdown: tr!("sakai はこのコードをまだ出さないので、再現はありません", "sakai does not print this code yet; it has no reproduction"),
        entries: vec![s(
            "E001",
            tr!("読めない字句があります", "Something cannot be read as a word of the language"),
            tr!(
                "閉じていない文字列、文字列の中の知らないエスケープ、全角の空白、名前に使えない文字があるとき。名前は文字、数字、`_` で書き、数字では始めません。",
                "A string not closed, an escape a string does not take, a full-width space, or a character a name cannot have. A name is letters, digits and `_`, and does not start with a digit."
            ),
            tr!("示された位置を直します。文字列は同じ行の `\"` で閉じ、エスケープは `\\\"` と `\\\\` だけを使います。", "Correct it where it points: close the string with `\"` on the same line, and use no escape but `\\\"` and `\\\\`."),
            &[("甲.ctx", "context 甲(a) v1\ndescription \"閉じていない\nowns\n  dir \"a\"\n")],
            &["E002"],
        )],
    }
}

fn golden(name: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/compat").join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

#[test]
fn explain_prints_what_the_three_printed() {
    let mut failures = Vec::new();
    for (ledger, code, tool) in [(koyomi(), "E102", "koyomi"), (koyomi(), "E305", "koyomi"), (yuen(), "E107", "yuen"), (sakai(), "E001", "sakai")] {
        let e = ledger.find(code).unwrap();
        for lang in [Lang::En, Lang::Ja] {
            let name = format!("{tool}-explain-{code}.{}.txt", lang.code());
            let got = ledger.render_text(e, lang);
            if got != golden(&name) {
                failures.push(format!("{name}:\n{}", ritsu_testkit::golden::line_diff(&golden(&name), &got)));
            }
            let name = format!("{tool}-markdown-{code}.{}.md", lang.code());
            let got = ledger.render_markdown_one(e, lang);
            if got != golden(&name) {
                failures.push(format!("{name}:\n{}", ritsu_testkit::golden::line_diff(&golden(&name), &got)));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_markdown_page_has_an_anchor_for_every_code_and_links_its_related_codes() {
    let k = koyomi();
    let md = k.render_markdown(Lang::En);
    assert!(md.starts_with("# Diagnostic codes\n\nWritten by `koyomi explain --all --format markdown`; do not edit.\n\n<a id=\"e102\"></a>\n"), "{md}");
    assert_eq!(md.matches("<a id=\"").count(), k.entries.len());
    assert!(md.contains("See also: [E101](#e101), [E103](#e103)\n"), "{md}");
    let ja = k.render_markdown(Lang::Ja);
    assert!(ja.starts_with("# 診断のコード\n\n`koyomi explain --all --format markdown --lang ja` の出力です。手で直しません。\n\n"), "{ja}");
    assert!(ja.contains("関連: [E101](#e101), [E103](#e103)\n"));
    assert!(k.find("e305").is_some(), "a code is found in either case");
    assert!(k.find("E999").is_none());
    assert!(ledger::duplicates(&k).is_empty());
}

#[test]
fn an_entry_for_a_program() {
    let k = koyomi();
    let j = k.to_json(k.find("E102").unwrap(), Lang::En);
    let keys: Vec<&str> = j.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["code", "severity", "title", "when", "fix", "repro", "related"]);
    let files = j.get("repro").and_then(|r| r.get("files")).and_then(|f| f.as_arr()).unwrap();
    assert_eq!(files.iter().filter_map(|f| f.get("path").and_then(|p| p.as_str())).collect::<Vec<_>>(), ["example.cal", "holidays.csv"]);
    let y = yuen();
    assert!(y.to_json(y.find("E107").unwrap(), Lang::Ja).get("repro").unwrap().is_null());
    let s = sakai();
    let j = s.to_json(s.find("E001").unwrap(), Lang::En);
    assert_eq!(j.get("repro").and_then(|r| r.get("command")).map(|c| c.compact()), Some(r#"["check","."]"#.to_string()));
}

#[test]
fn every_reproduction_is_laid_out_and_run() {
    let dir = ritsu_testkit::TempDir::new("ledger");
    // A run that reads what was laid out: koyomi's E102 has its file and the table beside it.
    let k = koyomi();
    let failures = ledger::check_every(&k, dir.path(), |e, d| {
        assert!(d.join("example.cal").is_file(), "{}", e.code);
        if e.code == "E102" {
            assert_eq!(std::fs::read_to_string(d.join("holidays.csv")).unwrap(), "2026-01-01,元日\n2026-05-04,みどりの日\n");
            vec!["E102".to_string()]
        } else {
            vec!["E999".to_string()]
        }
    });
    assert_eq!(failures, vec![r#"E305: the reproduction printed ["E999"]"#.to_string()]);
    // A later entry is passed over; a directory of files is laid out whole.
    assert!(ledger::check_every(&yuen(), dir.path(), |_, _| panic!("a later entry is not run")).is_empty());
    let s = sakai();
    let failures = ledger::check_every(&s, dir.path(), |_, d| {
        let names = ["地図.ctx", "甲.ctx", "乙.ctx", "a/a.proto", "b/v1/b.proto"];
        assert!(names.iter().all(|n| d.join(n).is_file()), "{}", d.display());
        assert!(std::fs::read_to_string(d.join("甲.ctx")).unwrap().contains("閉じていない"), "the entry's file is laid over the base");
        vec!["E001".to_string()]
    });
    assert!(failures.is_empty(), "{failures:?}");
    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none(), "the directories are removed");
}

/// An entry with a reproduction in English names and one in Japanese names: `explain` shows the one
/// of the language it is asked in (text, Markdown and JSON), an entry with one reproduction shows
/// it in both, and `check_every` runs each of the two.
#[test]
fn an_entry_shows_the_reproduction_of_the_language_it_is_asked_in() {
    let both = e("E001", tr!("読めない字句があります", "Something cannot be read"), tr!("いつ", "When"), tr!("直し方", "Fix"), "dates 例 v1\n", &[])
        .english(Repro::File { body: "dates example v1\n", beside: &[("holidays.csv", b"2026-01-01,New Year's Day\n")] });
    let one = e("E002", tr!("ほかの字句です", "Another thing"), tr!("いつ", "When"), tr!("直し方", "Fix"), "dates t v1\n", &[]);
    let ledger = Ledger {
        tool: "koyomi",
        example_file: "example.cal",
        fence: "cal",
        repro_heading: tr!("再現", "Example"),
        later_text: Text::default(),
        later_markdown: Text::default(),
        entries: vec![both, one],
    };
    let (both, one) = (ledger.find("E001").unwrap(), ledger.find("E002").unwrap());
    let en = ledger.render_text(both, Lang::En);
    assert!(en.contains("    dates example v1\n") && en.contains("beside it: holidays.csv") && !en.contains('例'), "{en}");
    let ja = ledger.render_text(both, Lang::Ja);
    assert!(ja.contains("    dates 例 v1\n") && !ja.contains("holidays.csv") && !ja.contains("example v1"), "{ja}");
    assert!(ledger.render_markdown_one(both, Lang::En).contains("```cal\ndates example v1\n```"));
    assert!(ledger.render_markdown_one(both, Lang::Ja).contains("```cal\ndates 例 v1\n```"));
    let files = |lang| {
        let j = ledger.to_json(both, lang);
        let files = j.get("repro").and_then(|r| r.get("files")).and_then(|f| f.as_arr()).unwrap().to_vec();
        files.iter().filter_map(|f| f.get("text").and_then(|t| t.as_str()).map(String::from)).collect::<Vec<_>>()
    };
    assert_eq!(files(Lang::En)[0], "dates example v1\n");
    assert_eq!(files(Lang::Ja), ["dates 例 v1\n"]);
    // an entry with one reproduction shows it in both languages, as before
    assert_eq!(ledger.render_text(one, Lang::En).matches("dates t v1").count(), 1);
    assert_eq!(ledger.render_text(one, Lang::Ja).matches("dates t v1").count(), 1);
    // check_every runs the English reproduction and then the Japanese one, each laid out whole
    let dir = ritsu_testkit::TempDir::new("ledger-two");
    let mut seen = Vec::new();
    let failures = ledger::check_every(&ledger, dir.path(), |entry, d| {
        let body = std::fs::read_to_string(d.join("example.cal")).unwrap();
        seen.push((entry.code, body, d.join("holidays.csv").is_file()));
        vec![entry.code.to_string()]
    });
    assert!(failures.is_empty(), "{failures:?}");
    assert_eq!(
        seen,
        [("E001", "dates example v1\n".to_string(), true), ("E001", "dates 例 v1\n".to_string(), false), ("E002", "dates t v1\n".to_string(), false)]
    );
    // a reproduction that prints another code is told with the language it is in
    let failures = ledger::check_every(&ledger, dir.path(), |entry, _| {
        let english = matches!(&entry.repro, Repro::File { body, .. } if body.contains("example"));
        vec![if english || entry.code == "E002" { entry.code.to_string() } else { "E999".to_string() }]
    });
    assert_eq!(failures, vec![r#"E001-ja: the reproduction printed ["E999"]"#.to_string()]);
}
