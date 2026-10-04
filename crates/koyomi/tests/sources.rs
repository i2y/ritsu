//! Sources and tables of holidays (PLAN B.5, DESIGN 1.5).

use koyomi::check::{Options, check_text};
use koyomi::holidays::{read_csv, read_govuk};
use ritsu_base::sha256;
use std::io::Write;
use std::process::{Command, Stdio};

const CSV: &str = "examples/calendars/data/syukujitsu.csv";
const JSON: &str = "examples/calendars/data/bank-holidays.json";
const LAW: &str = "examples/sources/law/129AC0000000089@2026-10-01";

#[test]
fn sha256_is_fips_180_4() {
    assert_eq!(sha256::hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    assert_eq!(sha256::hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert_eq!(
        sha256::hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
    assert_eq!(sha256::hex(&vec![b'a'; 1_000_000]), "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
}

#[test]
fn the_copies_are_the_ones_pinned() {
    // Taken on 2026-10-02 and never again (PLAN B.5): DESIGN's numbers come from these bytes.
    assert_eq!(sha256::hex(&std::fs::read(CSV).unwrap()), "cec37a743c96995cdb9cb52b685c9003634682a9b0e1a640a6b9b96881fe964a");
    assert_eq!(sha256::hex(&std::fs::read(JSON).unwrap()), "538b3482c28b85ecd2db606a0d5ae6ad17248900b6498700ce0a48d26a3ecde6");
}

#[test]
fn the_cabinet_office_table() {
    let rows = read_csv(&std::fs::read(CSV).unwrap(), true).unwrap();
    assert_eq!(rows.len(), 1067);
    let years: std::collections::BTreeSet<i32> = rows.iter().map(|r| r.day.year()).collect();
    assert_eq!(years, (1955..=2027).collect());
    assert_eq!((rows[0].day.to_string(), rows[0].name.as_str()), ("1955-01-01".into(), "元日"));
    let last = rows.last().unwrap();
    assert_eq!((last.day.to_string(), last.name.as_str()), ("2027-11-23".into(), "勤労感謝の日"));
    assert_eq!(rows.iter().filter(|r| r.name == "休日").count(), 116);
}

#[test]
fn govuk_bank_holidays() {
    let rows = read_govuk(&std::fs::read(JSON).unwrap(), "england-and-wales").unwrap();
    assert_eq!(rows.len(), 83);
    assert_eq!(rows[0].day.year(), 2019);
    assert_eq!(rows.last().unwrap().day.year(), 2028);
    assert_eq!(rows.iter().filter(|r| r.day.weekday() >= 5).count(), 0, "no bank holiday falls on a weekend; GOV.UK lists the substitute");
    assert!(rows.iter().any(|r| r.name == "Boxing Day (Substitute day)"));
    assert!(read_govuk(&std::fs::read(JSON).unwrap(), "wales").is_err());
}

#[test]
fn shift_jis_agrees_with_cp932() {
    if !ritsu_testkit::need(ritsu_testkit::Need::Python) {
        return;
    }
    if !ritsu_testkit::tools::runs("python3", &["--version"]) {
        ritsu_testkit::skip("python3 not found; the Shift_JIS table is not compared with Python's cp932");
        return;
    }
    let py = "python3";
    // The whole CSV.
    let bytes = std::fs::read(CSV).unwrap();
    let ours = koyomi::sjis::decode(&bytes).unwrap();
    let out = Command::new(py)
        .args(["-c", "import sys; sys.stdout.buffer.write(open(sys.argv[1], 'rb').read().decode('cp932').encode('utf-8'))", CSV])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8(out.stdout).unwrap(), ours, "the CSV reads the same as Python's cp932 reads it");
    // Every pointer of the table: its two bytes, as koyomi reads them and as cp932 does.
    let mut lines = String::new();
    let mut n = 0;
    for p in 0..koyomi::sjis_table::POINTERS {
        if (8836..=10715).contains(&p) {
            continue;
        }
        if let Some(c) = koyomi::sjis::pointer_char(p) {
            let (a, b) = koyomi::sjis::pointer_bytes(p);
            assert_eq!(koyomi::sjis::decode(&[a, b]).unwrap(), c.to_string());
            lines.push_str(&format!("{a:02x}{b:02x} {:x}\n", c as u32));
            n += 1;
        }
    }
    assert_eq!(n, koyomi::sjis_table::ASSIGNED);
    assert_eq!(n, 7724);
    let script = "import sys\nbad = 0\nfor l in sys.stdin:\n    h, c = l.split()\n    if bytes.fromhex(h).decode('cp932') != chr(int(c, 16)):\n        bad += 1\n        print(h, c)\nprint('differ', bad)\n";
    let mut child = Command::new(py).args(["-c", script]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(lines.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.trim_end().ends_with("differ 0"), "pointers where koyomi and cp932 disagree:\n{text}");
}

#[test]
fn bytes_that_are_not_shift_jis() {
    let b = std::fs::read("tests/fixtures/data/sjis-broken.csv").unwrap();
    let e = koyomi::sjis::decode(&b).unwrap_err();
    assert_eq!(e.bytes, vec![0x81, 0x7f]);
    let e = read_csv(&b, true).unwrap_err();
    assert_eq!(e.line, Some(2));
}

/// A calendar of the fixtures' tables, checked: the codes it gives.
fn calendar_codes(source_line: &str, format: &str, covers: &str) -> Vec<&'static str> {
    let src = format!("calendar t v1\n\n{source_line}\n  {format}\n  {covers}\n\nclosed 休み\n");
    let o = check_text("tests/fixtures/t.cal", &src, &Options::default());
    o.diags.iter().map(|d| d.code).collect()
}

#[test]
fn what_is_wrong_with_a_table() {
    let ok = "source 休み = file \"data/holidays.csv\" sha256:56ebcd2f1e91e0a1";
    assert_eq!(calendar_codes(ok, "format csv", "covers 2026-01-01..2026-12-31"), Vec::<&str>::new());
    assert_eq!(calendar_codes("source 休み = file \"data/none.csv\" sha256:56ebcd2f1e91e0a1", "format csv", "covers 2026-01-01..2026-12-31"), vec!["E101"]);
    assert_eq!(calendar_codes("source 休み = file \"data/holidays.csv\"", "format csv", "covers 2026-01-01..2026-12-31"), vec!["E102"]);
    assert_eq!(calendar_codes("source 休み = file \"data/holidays.csv\" sha256:0123456789abcdef", "format csv", "covers 2026-01-01..2026-12-31"), vec!["E103"]);
    assert_eq!(calendar_codes("source 休み = file \"data/broken.csv\" sha256:72fa28860be08f0a", "format csv", "covers 2026-01-01..2026-12-31"), vec!["E104"]);
    assert_eq!(calendar_codes("source 休み = file \"data/sjis-broken.csv\" sha256:5b9c45915a5110af", "format csv shift_jis", "covers 2026-01-01..2026-12-31"), vec!["E104"]);
    assert_eq!(calendar_codes(ok, "format csv", "covers 2026-01-01..2026-03-31"), vec!["E105"]);
    assert_eq!(calendar_codes("source 休み = file \"data/gap.csv\" sha256:bec0e9a9279b388d", "format csv", "covers listed years"), vec!["E106"]);
    // E102's fix is the line with the copy's own pin.
    let o = check_text("tests/fixtures/t.cal", "calendar t v1\n\nsource 休み = file \"data/holidays.csv\"\n  format csv\n  covers 2026-01-01..2026-12-31\n\nclosed 休み\n", &Options::default());
    assert_eq!(o.diags[0].fixed_line(), Some("source 休み = file \"data/holidays.csv\" sha256:56ebcd2f1e91e0a1"));
}

#[test]
fn the_law_copies() {
    let pins = [("140", "e880059021fbb67d"), ("141", "0575c131b9f08063"), ("142", "fc8c35a0769d3b35"), ("143", "6950bdfb988439b6")];
    for (a, pin) in pins {
        let b = std::fs::read(format!("{LAW}/MainProvision-Article_{a}.xml")).unwrap();
        assert_eq!(sha256::short(&b), pin, "article {a}");
    }
    assert_eq!(std::fs::read_to_string(format!("{LAW}/revision.txt")).unwrap().trim(), "129AC0000000089_20260624_508AC0000000045");
    let text = ritsu_base::sources::xml_text(&std::fs::read_to_string(format!("{LAW}/MainProvision-Article_143.xml")).unwrap());
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "（暦による期間の計算）");
    assert_eq!(lines[1], "第百四十三条");
    assert!(lines[2].starts_with("週、月又は年によって期間を定めたときは、その期間は、暦に従って計算する。"), "{text}");
}

#[test]
fn fragments_are_named_as_the_law_names_them() {
    use koyomi::sources::elm;
    assert_eq!(elm("第143条").as_deref(), Some("MainProvision-Article_143"));
    assert_eq!(elm("第百四十三条第二項").as_deref(), Some("MainProvision-Article_143-Paragraph_2"));
    assert_eq!(elm("第20条の2第3項第4号").as_deref(), Some("MainProvision-Article_20_2-Paragraph_3-Item_4"));
    assert_eq!(elm("第千五十条").as_deref(), Some("MainProvision-Article_1050"));
    assert_eq!(elm("附則第3条"), None);
    assert_eq!(elm("別表第一"), None);
    assert_eq!(elm("第一二条"), None);
}

/// A dates file beside the law copies, checked: the codes it gives.
fn law_codes(source: &str, cite: &str) -> Vec<&'static str> {
    let src = format!("dates t v1\n{source}\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate x = d    {cite}\n  + 1 day\n");
    let o = check_text("examples/t.cal", &src, &Options::default());
    o.diags.iter().map(|d| d.code).collect()
}

#[test]
fn citing_a_law() {
    let src = "source 民法 = law \"129AC0000000089\" asof 2026-10-01\n  第143条 sha256:6950bdfb988439b6";
    assert_eq!(law_codes(src, "@民法 第143条"), Vec::<&str>::new());
    // Cited and not pinned, cited whole, not a fragment, not declared.
    assert_eq!(law_codes(src, "@民法 第143条, 第142条"), vec!["E111"]);
    assert_eq!(law_codes(src, "@民法"), vec!["W102", "E111"]);
    assert_eq!(law_codes(src, "@民法 附則第3条"), vec!["W102", "E111"]);
    assert_eq!(law_codes(src, "@刑法 第1条"), vec!["W102", "E111"]);
    // Pinned and not cited.
    assert_eq!(law_codes(src, ""), vec!["W102"]);
    // No copy, no pin, the wrong pin.
    assert_eq!(law_codes("source 民法 = law \"129AC0000000089\" asof 2026-10-01\n  第1条 sha256:6950bdfb988439b6", "@民法 第1条"), vec!["E101"]);
    assert_eq!(law_codes("source 民法 = law \"129AC0000000089\" asof 2026-10-01\n  第143条", "@民法 第143条"), vec!["E102"]);
    assert_eq!(law_codes("source 民法 = law \"129AC0000000089\" asof 2026-10-01\n  第143条 sha256:0000000000000000", "@民法 第143条"), vec!["E103"]);
    // E111's fix is the pin line, with the copy's digest.
    let o = check_text("examples/t.cal", &format!("dates t v1\n{src}\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate x = d    @民法 第143条, 第142条\n  + 1 day\n"), &Options::default());
    assert_eq!(o.diags[0].fixed_line(), Some("  第142条 sha256:fc8c35a0769d3b35"));
}

// ── In English: tables of holidays in English, and a law cited by English names ────────────────

#[test]
fn an_english_csv_table() {
    let b = "Date,Holiday\r\n2026/12/28,\"Boxing Day, substitute\"\r\n2026/1/1,New Year's Day\r\n2026/12/25,Christmas Day\r\n";
    let rows = read_csv(b.as_bytes(), false).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!((rows[0].day.to_string(), rows[0].name.as_str()), ("2026-01-01".into(), "New Year's Day"));
    let last = rows.last().unwrap();
    assert_eq!((last.day.to_string(), last.name.as_str()), ("2026-12-28".into(), "Boxing Day, substitute"));
    assert_eq!(last.line, 2, "the line of the copy it came from");
}

/// What `bytes_that_are_not_shift_jis` shows, for a table read as UTF-8: one saved as
/// Windows-1252, its apostrophe the byte 0x92, is not read, and the place is said.
#[test]
fn bytes_that_are_not_utf8() {
    let b = std::fs::read("tests/fixtures/data/utf8-broken.csv").unwrap();
    let e = read_csv(&b, false).unwrap_err();
    assert_eq!(e.line, Some(2));
    assert!(e.why.en.contains("are not UTF-8"), "{}", e.why.en);
}

/// A calendar of the English tables of the fixtures, checked: the codes it gives.
fn calendar_codes_en(source_line: &str, format: &str, covers: &str) -> Vec<&'static str> {
    let src = format!("calendar t v1\n\n{source_line}\n  {format}\n  {covers}\n\nclosed holidays\n");
    let o = check_text("tests/fixtures/t.cal", &src, &Options::default());
    o.diags.iter().map(|d| d.code).collect()
}

#[test]
fn what_is_wrong_with_a_table_in_english() {
    let ok = "source holidays = file \"data/holidays.en.csv\" sha256:86cb32774217a66f";
    assert_eq!(calendar_codes_en(ok, "format csv", "covers 2026-01-01..2026-12-31"), Vec::<&str>::new());
    assert_eq!(calendar_codes_en("source holidays = file \"data/none.csv\" sha256:86cb32774217a66f", "format csv", "covers 2026-01-01..2026-12-31"), vec!["E101"]);
    assert_eq!(calendar_codes_en("source holidays = file \"data/holidays.en.csv\"", "format csv", "covers 2026-01-01..2026-12-31"), vec!["E102"]);
    assert_eq!(calendar_codes_en("source holidays = file \"data/holidays.en.csv\" sha256:0123456789abcdef", "format csv", "covers 2026-01-01..2026-12-31"), vec!["E103"]);
    assert_eq!(calendar_codes_en("source holidays = file \"data/broken.en.csv\" sha256:3ae3e9825beae644", "format csv", "covers 2026-01-01..2026-12-31"), vec!["E104"]);
    assert_eq!(calendar_codes_en("source holidays = file \"data/utf8-broken.csv\" sha256:1ceb9da7f664e1c9", "format csv", "covers 2026-01-01..2026-12-31"), vec!["E104"]);
    assert_eq!(calendar_codes_en("source holidays = file \"data/sjis-broken.csv\" sha256:5b9c45915a5110af", "format csv shift_jis", "covers 2026-01-01..2026-12-31"), vec!["E104"]);
    assert_eq!(calendar_codes_en(ok, "format csv", "covers 2026-01-01..2026-03-31"), vec!["E105"]);
    assert_eq!(calendar_codes_en("source holidays = file \"data/gap.en.csv\" sha256:cf519bbe72d28ea7", "format csv", "covers listed years"), vec!["E106"]);
    // E102's fix is the line with the copy's own pin.
    let o = check_text("tests/fixtures/t.cal", "calendar t v1\n\nsource holidays = file \"data/holidays.en.csv\"\n  format csv\n  covers 2026-01-01..2026-12-31\n\nclosed holidays\n", &Options::default());
    assert_eq!(o.diags[0].fixed_line(), Some("source holidays = file \"data/holidays.en.csv\" sha256:86cb32774217a66f"));
}

/// What `citing_a_law` shows, with English names: the source and the dates are named in English,
/// and the articles as e-Gov names them (`第143条`), since only e-Gov's laws can be cited.
#[test]
fn citing_a_law_by_an_english_name() {
    let src = "source civil_code = law \"129AC0000000089\" asof 2026-10-01\n  第143条 sha256:6950bdfb988439b6";
    assert_eq!(law_codes(src, "@civil_code 第143条"), Vec::<&str>::new());
    assert_eq!(law_codes(src, "@civil_code 第143条, 第142条"), vec!["E111"]);
    assert_eq!(law_codes(src, "@civil_code"), vec!["W102", "E111"]);
    assert_eq!(law_codes(src, "@penal_code 第1条"), vec!["W102", "E111"]);
    assert_eq!(law_codes(src, ""), vec!["W102"]);
    assert_eq!(law_codes("source civil_code = law \"129AC0000000089\" asof 2026-10-01\n  第143条", "@civil_code 第143条"), vec!["E102"]);
    let o = check_text("examples/t.cal", &format!("dates t v1\n{src}\n\ninputs\n  d : date  range >=2026-01-01 <=2026-01-31\n\ndate x = d    @civil_code 第143条, 第142条\n  + 1 day\n"), &Options::default());
    assert_eq!(o.diags[0].fixed_line(), Some("  第142条 sha256:fc8c35a0769d3b35"));
}
