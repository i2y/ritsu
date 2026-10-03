//! The copies of the sources (DESIGN 1.4, PLAN B.4): SHA-256 against FIPS 180-4, the pins of
//! the copies the tests read, the text of a copy, and what is wrong with copies and citations
//! (E101–E105, W101).

mod common;

use yurai::ast::LawDb;
use yurai::copies;
use yurai::sha256;

#[test]
fn sha256_against_fips_180_4() {
    assert_eq!(sha256::hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    assert_eq!(sha256::hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert_eq!(sha256::hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"), "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1");
    assert_eq!(sha256::hex(&vec![b'a'; 1_000_000]), "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
    assert_eq!(sha256::short(b"abc"), "ba7816bf8f01cfea");
}

#[test]
fn base64_round_trips() {
    let bytes: Vec<u8> = (0..=255u8).collect();
    assert_eq!(yurai::base64::decode(&yurai::base64::encode(&bytes)).unwrap(), bytes);
}

const LAW: &str = "tests/fixtures/period/sources/law/129AC0000000089@2026-10-01";

#[test]
fn the_pins_of_the_copies_of_the_civil_code() {
    for (art, pin) in [("140", "e880059021fbb67d"), ("141", "0575c131b9f08063"), ("142", "fc8c35a0769d3b35"), ("143", "6950bdfb988439b6")] {
        let b = std::fs::read(format!("{LAW}/MainProvision-Article_{art}.xml")).unwrap();
        assert_eq!(sha256::short(&b), pin, "第{art}条");
        assert!(copies::readable(LawDb::Egov, &format!("MainProvision-Article_{art}.xml"), &b).is_ok());
    }
    assert_eq!(copies::revision(std::path::Path::new(LAW)).as_deref(), Some("129AC0000000089_20260624_508AC0000000045"));
}

#[test]
fn the_text_of_a_copy() {
    let xml = std::fs::read_to_string(format!("{LAW}/MainProvision-Article_142.xml")).unwrap();
    let text = copies::xml_text(&xml);
    assert!(text.contains("期間の末日が日曜日、国民の祝日に関する法律（昭和二十三年法律第百七十八号）に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌日に満了する。"), "{text}");
    assert_eq!(text.lines().next(), Some("第百四十二条"));
    let xml = std::fs::read_to_string(format!("{LAW}/MainProvision-Article_143.xml")).unwrap();
    let lines = copies::article_lines(&xml);
    assert_eq!(lines[0], "（暦による期間の計算）");
    assert!(lines[1].starts_with("第百四十三条　週、月又は年によって"), "{}", lines[1]);
    assert!(lines[2].starts_with("２　週、月又は年の初めから"), "{}", lines[2]);
}

#[test]
fn a_section_of_the_cfr() {
    let path = "tests/fixtures/ecfr/sources/law/29-CFR-1910@2026-01-01/1910.157.xml";
    let b = std::fs::read(path).unwrap();
    assert_eq!(sha256::short(&b), "c2a9ce966c7e2269");
    assert!(copies::readable(LawDb::Ecfr, "1910.157.xml", &b).is_ok());
    assert!(copies::readable(LawDb::Egov, "MainProvision-Article_1.xml", &b).is_err(), "an eCFR section is not an e-Gov article");
    assert_eq!(copies::fragment_file(LawDb::Ecfr, "§1910.157").as_deref(), Some("1910.157.xml"));
    assert_eq!(copies::copy_dir("29 CFR 1910", "2026-01-01"), "sources/law/29-CFR-1910@2026-01-01");
    let r = common::yurai(std::path::Path::new("tests/fixtures/ecfr"), &["check", ".", "--root", "."]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
}

#[test]
fn the_files_articles_are_copied_into() {
    let e = |s: &str| copies::fragment_file(LawDb::Egov, s);
    assert_eq!(e("第143条第2項").as_deref(), Some("MainProvision-Article_143-Paragraph_2.xml"));
    assert_eq!(e("第百四十三条").as_deref(), Some("MainProvision-Article_143.xml"));
    assert_eq!(e("別表第一").as_deref(), Some("AppdxTable_1.xml"));
    assert_eq!(e("附則第3条").as_deref(), Some("SupplProvision-Article_3.xml"));
    assert_eq!(e("附則（令和七年三月三一日法律第一三号）第3条").as_deref(), Some("SupplProvision_令和七年三月三一日法律第一三号-Article_3.xml"));
    assert_eq!(e("142条"), None);
}

/// A change to a copy of the fixture: what it is, what it does, the code it gives.
type Case<'a> = (&'a str, &'a dyn Fn(&std::path::Path), &'a str);

/// What is wrong with a copy or a citation, each in a copy of the fixture changed one way.
#[test]
fn what_is_wrong_with_copies_and_citations() {
    let cases: &[Case] = &[
        ("the copy is gone", &|d| std::fs::remove_file(d.join("sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml")).unwrap(), "E101"),
        ("no hash on the pin", &|d| edit(d, "民法の期間.req", "第142条 sha256:fc8c35a0769d3b35", "第142条"), "E102"),
        ("no pin line", &|d| edit(d, "民法の期間.req", "  第142条 sha256:fc8c35a0769d3b35\n", ""), "E102"),
        ("the copy changed", &|d| edit(d, "sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml", "翌日", "翌々日"), "E103"),
        ("not XML", &|d| std::fs::write(d.join("sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml"), "x").unwrap(), "E104"),
        ("not the article", &|d| edit(d, "sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml", "<Article Num=\"142\">", "<Law>"), "E104"),
        ("a source not declared", &|d| edit(d, "民法の期間.req", "from @民法 第142条", "from @商法 第142条"), "E105"),
        ("a law cited whole", &|d| edit(d, "民法の期間.req", "from @民法 第142条", "from @民法"), "E105"),
        ("an article of no form", &|d| edit(d, "民法の期間.req", "from @民法 第142条", "from @民法 その他"), "E105"),
        ("pinned and cited nowhere", &|d| edit(d, "民法の期間.req", "  from @民法 第142条\n    reviewed 2026-10-03 by 法務 sha256:fc8c35a0769d3b35 -> sha256:d4f2d2a67322df17\n", ""), "W101"),
    ];
    for (what, change, code) in cases {
        let t = common::fixture("period");
        let d = t.path().join("period");
        change(&d);
        let r = common::yurai(t.path(), &["check", "period", "--root", "period"]);
        let got = common::codes(&r.stdout);
        assert!(got.iter().any(|c| c == code), "{what}: want {code}, got {got:?}\n{}", r.stdout);
    }
}

fn edit(d: &std::path::Path, rel: &str, from: &str, to: &str) {
    let p = d.join(rel);
    let s = std::fs::read_to_string(&p).unwrap();
    assert!(s.contains(from), "{rel} has no {from:?}");
    std::fs::write(&p, s.replacen(from, to, 1)).unwrap();
}

/// A `file` source: the copy pinned whole, cited whole.
#[test]
fn a_file_source() {
    let t = common::TempDir::new("filesource");
    let terms = "第1条 返金は、購入から 30 日以内に申し込む。\n";
    t.write("docs/terms.md", terms.as_bytes());
    let pin = sha256::short(terms.as_bytes());
    let src = format!(
        "requirements 返金 v1\nrole 経理\n\nsource 約款 = file \"docs/terms.md\" url \"https://example.org/terms.md\" sha256:{pin}\n\nrequirement 返金の期限(refund_window)\n  text \"返金は、購入から 30 日以内の申し込みに限る\"\n  owner 経理\n  from @約款\n  not satisfied \"例\"\n  not verified \"例\"\n"
    );
    t.write("refund.req", src.as_bytes());
    let r = common::yurai(t.path(), &["review", ".", "--root", ".", "--all", "--by", "経理", "--date", "2026-10-03"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    let r = common::yurai(t.path(), &["check", ".", "--root", "."]);
    assert_eq!(r.code, 0, "{}", r.stdout);
    let api: serde_json::Value = serde_json::from_str(&common::yurai(t.path(), &["api", ".", "--root", "."]).stdout).unwrap();
    assert_eq!(api["sources"][0]["kind"], "file");
    assert_eq!(api["sources"][0]["path"], "docs/terms.md");
    assert_eq!(api["requirements"][0]["from"][0]["sources"][0]["sha256"], pin.as_str());
    // The requirement's end writes the file's path from the root.
    let end = t.read(&format!("reviewed/{}", api["requirements"][0]["sha256"].as_str().unwrap()));
    assert!(end.contains(&format!("from file docs/terms.md sha256:{pin}\n")), "{end}");
    // Changing the file: E103, and with the pin written again, E302 on the `from`.
    t.write("docs/terms.md", "第1条 返金は、購入から 14 日以内に申し込む。\n".as_bytes());
    let r = common::yurai(t.path(), &["check", ".", "--root", "."]);
    assert_eq!(common::codes(&r.stdout), ["E103"]);
    // A file source cited with an article is E105.
    t.write("refund.req", src.replace("from @約款", "from @約款 第1条").as_bytes());
    let r = common::yurai(t.path(), &["check", ".", "--root", "."]);
    assert!(common::codes(&r.stdout).contains(&"E105".to_string()), "{}", r.stdout);
}
