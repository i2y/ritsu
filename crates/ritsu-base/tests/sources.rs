//! The copies of sources (DESIGN 4.6). The examples of element names and of the text of a copy
//! are the ones rulec's, koyomi's and yuen's tests held their own functions to; the requests go
//! to a small HTTP server inside the test, and to the real e-Gov and eCFR only when
//! `RITSU_TEST_LEVEL=platforms`.

use ritsu_base::json::{self, Json};
use ritsu_base::sha256;
use ritsu_base::sources::{self, Ecfr, Egov, LawDb, SupplCache};
use ritsu_testkit::http::HttpServer;
use ritsu_testkit::level::{self, Level};
use std::path::{Path, PathBuf};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(rel)
}

const LAW: &str = "law/129AC0000000089@2026-10-01";
const CFR: &str = "law/29-CFR-1910@2026-01-01/1910.157.xml";

#[test]
fn articles_are_named_as_the_law_names_them() {
    // rulec's.
    let elm = |s: &str| sources::fragment(LawDb::Egov, s).map(|f| f.elm);
    assert_eq!(elm("第91条").as_deref(), Some("MainProvision-Article_91"));
    assert_eq!(elm("第20条の2").as_deref(), Some("MainProvision-Article_20_2"));
    assert_eq!(elm("第20条第2項").as_deref(), Some("MainProvision-Article_20-Paragraph_2"));
    assert_eq!(elm("第20条第2項第3号").as_deref(), Some("MainProvision-Article_20-Paragraph_2-Item_3"));
    assert_eq!(elm("第二十条第二項").as_deref(), Some("MainProvision-Article_20-Paragraph_2"));
    assert_eq!(elm("別表第一").as_deref(), Some("AppdxTable[1]"));
    assert_eq!(elm("別表第十二").as_deref(), Some("AppdxTable[12]"));
    assert_eq!(elm("第3条ただし書"), None);
    // koyomi's, which cites the main provisions only.
    let main = |s: &str| sources::fragment(LawDb::Egov, s).filter(|f| f.is_main_provision()).map(|f| f.elm);
    assert_eq!(main("第143条").as_deref(), Some("MainProvision-Article_143"));
    assert_eq!(main("第百四十三条第二項").as_deref(), Some("MainProvision-Article_143-Paragraph_2"));
    assert_eq!(main("第20条の2第3項第4号").as_deref(), Some("MainProvision-Article_20_2-Paragraph_3-Item_4"));
    assert_eq!(main("第千五十条").as_deref(), Some("MainProvision-Article_1050"));
    assert_eq!(main("附則第3条"), None);
    assert_eq!(main("別表第一"), None);
    assert_eq!(main("第一二条"), None);
    // yuen's: the file each article is copied into.
    let file = |s: &str| sources::fragment_file(LawDb::Egov, s);
    assert_eq!(file("第140条").as_deref(), Some("MainProvision-Article_140.xml"));
    assert_eq!(file("第143条第2項").as_deref(), Some("MainProvision-Article_143-Paragraph_2.xml"));
    assert_eq!(file("第百四十三条").as_deref(), Some("MainProvision-Article_143.xml"));
    assert_eq!(file("別表第一").as_deref(), Some("AppdxTable_1.xml"));
    assert_eq!(file("附則第3条").as_deref(), Some("SupplProvision-Article_3.xml"));
    assert_eq!(file("附則").as_deref(), Some("SupplProvision.xml"));
    assert_eq!(file("附則（令和七年三月三一日法律第一三号）第3条").as_deref(), Some("SupplProvision_令和七年三月三一日法律第一三号-Article_3.xml"));
    assert_eq!(file("140条"), None);
    assert_eq!(sources::fragment_file(LawDb::Ecfr, "§1910.157").as_deref(), Some("1910.157.xml"));
    assert_eq!(sources::fragment_file(LawDb::Ecfr, "1910.157").as_deref(), Some("1910.157.xml"));
    assert_eq!(sources::fragment_file(LawDb::Ecfr, "1910"), None);
    assert_eq!(sources::copy_dir("29 CFR 1910", "2026-01-01"), "sources/law/29-CFR-1910@2026-01-01");
    assert_eq!(sources::copy_dir("129AC0000000089", "2026-10-01"), "sources/law/129AC0000000089@2026-10-01");
}

#[test]
fn supplementary_provisions_are_named_by_their_amending_law() {
    let f = |s: &str| sources::fragment(LawDb::Egov, s);
    let own = f("附則第3条").unwrap();
    assert_eq!((own.elm.as_str(), own.file().as_str()), ("SupplProvision-Article_3", "SupplProvision-Article_3.xml"));
    assert_eq!(f("附則").unwrap().elm, "SupplProvision");
    let a = f("附則（令和七年三月三一日法律第一三号）第3条第2項").unwrap();
    assert_eq!(a.elm, "SupplProvision[?]-Article_3-Paragraph_2");
    assert_eq!(a.amend.as_deref(), Some("令和七年三月三一日法律第一三号"));
    assert_eq!(a.file(), "SupplProvision_令和七年三月三一日法律第一三号-Article_3-Paragraph_2.xml");
    assert_eq!(a.elm_at(244), "SupplProvision[244]-Article_3-Paragraph_2");
    let whole = f("附則(令和六年三月三〇日法律第八号)").unwrap();
    assert_eq!((whole.elm.as_str(), whole.file().as_str()), ("SupplProvision[?]", "SupplProvision_令和六年三月三〇日法律第八号.xml"));
    assert!(f("附則（）第3条").is_none());
    assert!(f("附則の3").is_none());
    // And back from the file a copy is kept in to what e-Gov is asked for.
    assert_eq!(sources::element_of_file(&a.file()), ("SupplProvision[?]-Article_3-Paragraph_2".to_string(), Some("令和七年三月三一日法律第一三号".to_string())));
    assert_eq!(sources::element_of_file("AppdxTable_1.xml"), ("AppdxTable[1]".to_string(), None));
    assert_eq!(sources::element_of_file("MainProvision-Article_142.xml"), ("MainProvision-Article_142".to_string(), None));
}

#[test]
fn numbers_as_a_law_writes_them() {
    for (s, n) in [("九", 9), ("十", 10), ("十二", 12), ("二十", 20), ("九十九", 99), ("百四十三", 143), ("千五十", 1050), ("12", 12)] {
        assert_eq!(sources::law_number(s), Some(n), "{s}");
    }
    for s in ["", "0", "一二", "十十", "百千", "x"] {
        assert_eq!(sources::law_number(s), None, "{s}");
    }
}

#[test]
fn the_text_of_a_copy() {
    // rulec's.
    let a = "<Article Num=\"1\"><Paragraph Num=\"1\"><Sentence>甲は、乙とする。</Sentence></Paragraph></Article>";
    let b = "<Article Num=\"1\">\n  <Paragraph Num=\"1\">\n    <Sentence Num=\"1\" WritingMode=\"vertical\">甲は、乙とする。</Sentence>\n  </Paragraph>\n</Article>";
    assert!(sources::same_text(a.as_bytes(), b.as_bytes()), "markup alone is not a change");
    assert!(!sources::same_text(a.as_bytes(), a.replace("乙", "丙").as_bytes()));
    let x = "<Article Num=\"1\"><ArticleTitle>第一条</ArticleTitle><Paragraph Num=\"1\"><ParagraphNum/><ParagraphSentence><Sentence>甲は、乙とする。</Sentence></ParagraphSentence></Paragraph></Article>";
    assert_eq!(sources::xml_text(x), "第一条\n甲は、乙とする。");
    let table = "<AppdxTable><AppdxTableTitle>別表第一</AppdxTableTitle><TableStruct><Table><TableRow><TableColumn>一</TableColumn><TableColumn>二百円</TableColumn></TableRow><TableRow><TableColumn>二</TableColumn><TableColumn>四百円</TableColumn></TableRow></Table></TableStruct></AppdxTable>";
    assert_eq!(sources::xml_text(table), "別表第一\n一 二百円\n二 四百円", "a row a line, its cells spaced apart");
    // koyomi's and yuen's, on the copies of the Civil Code.
    let a142 = std::fs::read_to_string(fixture(LAW).join("MainProvision-Article_142.xml")).unwrap();
    let text = sources::xml_text(&a142);
    assert_eq!(text.lines().next(), Some("第百四十二条"));
    assert!(text.contains("期間の末日が日曜日、国民の祝日に関する法律（昭和二十三年法律第百七十八号）に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌日に満了する。"), "{text}");
    let a143 = std::fs::read_to_string(fixture(LAW).join("MainProvision-Article_143.xml")).unwrap();
    let lines: Vec<String> = sources::xml_text(&a143).lines().map(String::from).collect();
    assert_eq!(lines[0], "（暦による期間の計算）");
    assert_eq!(lines[1], "第百四十三条");
    assert!(lines[2].starts_with("週、月又は年によって期間を定めたときは、その期間は、暦に従って計算する。"), "{}", lines[2]);
    let printed = sources::article_lines(&a143);
    assert_eq!(printed[0], "（暦による期間の計算）");
    assert!(printed[1].starts_with("第百四十三条\u{3000}週、月又は年によって"), "{}", printed[1]);
    assert!(printed[2].starts_with("２\u{3000}週、月又は年の初めから"), "{}", printed[2]);
    assert_eq!(sources::quote_lines(LawDb::Egov, "MainProvision-Article_143.xml", &a143), printed);
    assert_eq!(sources::quote_lines(LawDb::Egov, "AppdxTable_1.xml", table), ["別表第一", "一 二百円", "二 四百円"]);
}

#[test]
fn the_copies_their_pins_and_what_a_copy_starts_with() {
    for (art, pin) in [("140", "e880059021fbb67d"), ("141", "0575c131b9f08063"), ("142", "fc8c35a0769d3b35"), ("143", "6950bdfb988439b6")] {
        let file = format!("MainProvision-Article_{art}.xml");
        let b = std::fs::read(fixture(LAW).join(&file)).unwrap();
        assert_eq!(sha256::short(&b), pin, "第{art}条");
        assert!(sources::readable(LawDb::Egov, &file, &b).is_ok());
    }
    assert_eq!(sources::revision(&fixture(LAW)).as_deref(), Some("129AC0000000089_20260624_508AC0000000045"));
    let cfr = std::fs::read(fixture(CFR)).unwrap();
    assert_eq!(sha256::short(&cfr), "c2a9ce966c7e2269");
    assert!(sources::readable(LawDb::Ecfr, "1910.157.xml", &cfr).is_ok());
    assert!(sources::readable(LawDb::Egov, "MainProvision-Article_1.xml", &cfr).is_err(), "an eCFR section is not an e-Gov article");
    assert!(sources::readable(LawDb::Egov, "MainProvision-Article_1.xml", b"x").is_err());
    assert!(sources::readable(LawDb::Egov, "MainProvision-Article_1.xml", &[0xff, 0xfe]).is_err());
    assert_eq!(sources::root_element(LawDb::Egov, "MainProvision-Article_143-Paragraph_2.xml"), "Paragraph");
    assert_eq!(sources::root_element(LawDb::Egov, "AppdxTable_1.xml"), "AppdxTable");
    assert_eq!(sources::root_element(LawDb::Egov, "SupplProvision.xml"), "SupplProvision");
    assert_eq!(sources::root_element(LawDb::Egov, "MainProvision-Article_140.xml"), "Article");
    assert_eq!(sources::first_element("\u{feff}<?xml version=\"1.0\"?>\n<!-- c --><Article Num=\"1\">"), Some("Article"));
}

#[test]
fn the_supplementary_provisions_are_counted_in_document_order() {
    let xml = "<Law><MainProvision/><SupplProvision Extract=\"true\"><SupplProvisionLabel>附　則</SupplProvisionLabel></SupplProvision>\n<SupplProvision AmendLawNum=\"昭和四二年七月一三日法律第五六号\" Extract=\"true\"><SupplProvisionLabel>附　則</SupplProvisionLabel></SupplProvision><SupplProvision AmendLawNum=\"令和七年三月三一日法律第一三号\"></SupplProvision></Law>";
    assert_eq!(sources::suppl_ordinals(xml), vec![None, Some("昭和四二年七月一三日法律第五六号".into()), Some("令和七年三月三一日法律第一三号".into())]);
}

#[test]
fn a_laws_number_and_a_revision_in_words() {
    let s = sources::law_num_text("508AC0000000012");
    assert_eq!((s.ja.as_str(), s.en.as_str()), ("令和8年法律第12号", "Act No. 12 of 2026"));
    assert_eq!(sources::law_num_text("430AC0000000007").ja, "平成30年法律第7号");
    assert_eq!(sources::law_num_text("508CO0000000012").en, "508CO0000000012", "not an act");
    let w = sources::revision_words("332AC0000000026_20260401_508AC0000000012").unwrap();
    assert_eq!(w.ja, "2026-04-01 施行、令和8年法律第12号による改正後");
    assert_eq!(w.en, "in force from 2026-04-01, as amended by Act No. 12 of 2026");
    assert!(sources::revision_words("nonsense").is_none());
}

#[test]
fn a_diff_of_two_copies() {
    let a = "<Article><Sentence>平成二十六年四月一日から令和六年三月三十一日までの間</Sentence><Sentence>二百円</Sentence></Article>";
    let b = "<Article><Sentence>平成二十六年四月一日から令和九年三月三十一日までの間</Sentence><Sentence>二百円</Sentence></Article>";
    let (ta, tb) = (sources::xml_text(a), sources::xml_text(b));
    let d: Vec<String> = sources::text_diff(&ta, &tb, 8).into_iter().map(|t| t.en).collect();
    assert_eq!(d, ["- 平成二十六年四月一日から令和六年三月三十一日までの間", "+ 平成二十六年四月一日から令和九年三月三十一日までの間"]);
    assert!(sources::text_diff(&ta, &ta, 8).is_empty());
    let d = sources::text_diff("a\nb\nc", "x\ny\nz", 2);
    assert_eq!(d.iter().map(|t| t.en.as_str()).collect::<Vec<_>>(), ["- a", "- b", "- … (1 more lines)", "+ x", "+ y", "+ … (1 more lines)"]);
    assert_eq!(d[2].ja, "- …（あと 1 行）");
}

#[test]
fn a_pin_written_into_a_line() {
    // Only the digits change; a line without a pin gets one before its comment.
    assert_eq!(sources::pinned("  第142条 sha256:fc8c35a0769d3b35", "54a319e4148c24c7"), "  第142条 sha256:54a319e4148c24c7");
    assert_eq!(sources::pinned("  第142条   # 満了日", "54a319e4148c24c7"), "  第142条 sha256:54a319e4148c24c7   # 満了日");
    assert_eq!(sources::pinned("  第142条\r", "54a319e4148c24c7"), "  第142条 sha256:54a319e4148c24c7\r", "the line ending stays");
    assert_eq!(
        sources::pinned("source 約款 = file \"a#b sha256:x.md\" sha256:0000", "1234567890abcdef"),
        "source 約款 = file \"a#b sha256:x.md\" sha256:1234567890abcdef",
        "a `#` or a `sha256:` in a string is not the line's"
    );
    assert_eq!(sources::pinned("x \"a\\\"b\" # c", "ff"), "x \"a\\\"b\" sha256:ff # c");
    // The line a diagnostic offers, fixed.
    assert_eq!(sources::fixed_pin_line("  第143条 sha256:0000000000000000  ", "6950bdfb988439b6"), "  第143条 sha256:6950bdfb988439b6");
    assert_eq!(sources::fixed_pin_line("  第142条", "fc8c35a0769d3b35"), "  第142条 sha256:fc8c35a0769d3b35");
    // rulec's, which keeps a comment two spaces after the pin.
    assert_eq!(sources::pinned_spaced("  第91条 sha256:aaaa   # 本則", "bbbb"), "  第91条 sha256:bbbb  # 本則");
    assert_eq!(sources::pinned_spaced("  第91条", "bbbb"), "  第91条 sha256:bbbb");
}

#[test]
fn base64_both_ways() {
    let bytes: Vec<u8> = (0..=255u8).collect();
    assert_eq!(sources::base64_decode(&sources::base64_encode(&bytes)).unwrap(), bytes);
    assert_eq!(sources::base64_encode(b"ab"), "YWI=");
    assert_eq!(sources::base64_decode("YWJj\nYQ==").unwrap(), b"abca", "a line break is skipped");
    assert!(sources::base64_decode("YQ").is_none(), "the standard form has its padding");
    assert!(sources::base64_decode("Y=Q=").is_none());
    assert!(sources::base64_decode("YW-_").is_none(), "the standard alphabet");
    assert_eq!(sources::base64_decode_lenient("YQ").unwrap(), b"a");
    assert_eq!(sources::base64_decode_lenient("-_8").unwrap(), sources::base64_decode("+/8=").unwrap());
}

#[test]
fn raw_urls_and_queries() {
    let commit = "https://raw.githubusercontent.com/o/r/a1b2c3d4e5f60718293a4b5c6d7e8f9012345678/docs/t.md";
    let got = sources::github_raw(commit).unwrap();
    assert_eq!((got.0.as_str(), got.1.as_str(), got.3.as_str()), ("o", "r", "docs/t.md"));
    assert!(sources::github_raw("https://raw.githubusercontent.com/o/r/a1b2c3d/t.md").is_some());
    for not in ["https://raw.githubusercontent.com/o/r/main/t.md", "https://raw.githubusercontent.com/o/r/v1.2.0/t.md", "https://raw.githubusercontent.com/o/r/abc/t.md", "https://raw.githubusercontent.com/o/r/a1b2c3d", "https://example.com/o/r/a1b2c3d/t.md"] {
        assert!(sources::github_raw(not).is_none(), "{not}");
    }
    assert!(sources::raw_on_a_branch("https://raw.githubusercontent.com/o/r/main/t.md"));
    assert!(!sources::raw_on_a_branch(commit));
    assert_eq!(sources::urlq("docs/料金表.md"), "docs/%E6%96%99%E9%87%91%E8%A1%A8.md");
    assert_eq!(sources::urlq("a b/c.md"), "a%20b/c.md");
    assert_eq!(sources::cfr_id("29 CFR 1910").unwrap(), ("29".to_string(), "1910".to_string()));
    assert!(sources::cfr_id("29 USC 1910").is_err());
}

fn no_curl() -> bool {
    if ritsu_testkit::tools::on_path("curl").is_some() {
        return false;
    }
    ritsu_testkit::skip("curl is not on the PATH; the requests to e-Gov and the eCFR are not run");
    true
}

/// What e-Gov's `law_data` answers: the XML in base64, and the revision.
fn law_data(xml: &[u8], rev: &str) -> String {
    Json::obj([
        ("law_info", Json::obj([("law_id", Json::str("129AC0000000089"))])),
        ("revision_info", Json::obj([("law_revision_id", Json::str(rev))])),
        ("law_full_text", Json::str(sources::base64_encode(xml))),
    ])
    .compact()
}

#[test]
fn requests_to_e_gov_and_the_ecfr() {
    if no_curl() {
        return;
    }
    let s = HttpServer::start();
    let egov = Egov { base: format!("{}/egov", s.addr) };
    let a142 = std::fs::read(fixture(LAW).join("MainProvision-Article_142.xml")).unwrap();
    s.set("/egov/law_data/129AC0000000089?asof=2026-10-01&elm=MainProvision-Article_142&law_full_text_format=xml", law_data(&a142, "129AC0000000089_20260624_508AC0000000045"));
    let (xml, rev) = egov.law_data("129AC0000000089", "2026-10-01", Some("MainProvision-Article_142")).unwrap();
    assert_eq!((xml, rev.as_str()), (a142.clone(), "129AC0000000089_20260624_508AC0000000045"));
    // An amending law's supplementary provisions: the whole law is read once for their
    // position, then the element is asked for at it.
    let whole = "<Law><SupplProvision/><SupplProvision AmendLawNum=\"令和七年三月三一日法律第一三号\"></SupplProvision></Law>";
    s.set("/egov/law_data/129AC0000000089?asof=2026-10-01&law_full_text_format=xml", law_data(whole.as_bytes(), "r"));
    let article = "<SupplProvision AmendLawNum=\"令和七年三月三一日法律第一三号\"><Article Num=\"3\"><Sentence>附則の条</Sentence></Article></SupplProvision>";
    s.set("/egov/law_data/129AC0000000089?asof=2026-10-01&elm=SupplProvision%5B2%5D-Article_3&law_full_text_format=xml", law_data(article.as_bytes(), "r"));
    let mut cache = SupplCache::new();
    let f = sources::fragment(LawDb::Egov, "附則（令和七年三月三一日法律第一三号）第3条").unwrap();
    let (xml, _) = egov.element("129AC0000000089", "2026-10-01", "2026-10-01", &f.elm, f.amend.as_deref(), &mut cache).unwrap();
    assert_eq!(sources::xml_text(&String::from_utf8(xml).unwrap()), "附則の条");
    let (xml, _) = egov.element("129AC0000000089", "2026-10-01", "2026-10-01", &f.elm, f.amend.as_deref(), &mut cache).unwrap();
    assert!(!xml.is_empty());
    assert_eq!(s.asked().iter().filter(|t| !t.contains("elm=")).count(), 1, "the whole law is read once: {:?}", s.asked());
    assert!(egov.suppl_index("129AC0000000089", "2026-10-01", "平成元年法律第一号", &mut cache).is_err());
    // The revisions after a date, one a day; a revision with no date is passed over.
    let revs = json::parse(r#"{"revisions":[{"law_revision_id":"a","amendment_enforcement_date":"2026-06-24"},{"law_revision_id":"b","amendment_enforcement_date":"2027-06-23"},{"law_revision_id":"c","amendment_enforcement_date":"2027-06-23"},{"law_revision_id":"d","amendment_enforcement_date":"2028-01-01"},{"law_revision_id":"e","amendment_enforcement_date":null}]}"#).unwrap();
    s.set("/egov/law_revisions/129AC0000000089", revs.compact());
    assert_eq!(
        egov.later_revisions("129AC0000000089", "2026-10-01").unwrap(),
        vec![("2027-06-23".to_string(), "b".to_string()), ("2028-01-01".to_string(), "d".to_string())]
    );
    // The eCFR: a section as served, and the substantive amendments after a date.
    let ecfr = Ecfr { base: format!("{}/ecfr", s.addr) };
    let cfr = std::fs::read(fixture(CFR)).unwrap();
    s.set("/ecfr/full/2026-01-01/title-29.xml?part=1910&section=1910.157", cfr.clone());
    assert_eq!(ecfr.section("29 CFR 1910", "2026-01-01", "1910.157").unwrap(), cfr);
    s.set(
        "/ecfr/versions/title-29.json?part=1910&section=1910.157",
        r#"{"content_versions":[{"amendment_date":"2019-05-14","substantive":true},{"amendment_date":"2026-03-01","substantive":true},{"amendment_date":"2026-05-01","substantive":false}],"meta":{"title":"29"}}"#,
    );
    assert_eq!(ecfr.amendments("29 CFR 1910", "2026-01-01", &["1910.157".to_string()]).unwrap(), vec![("2026-03-01".to_string(), "1910.157".to_string())]);
    assert!(s.missed().is_empty(), "asked for what is not served: {:?}", s.missed());
    // A file:// URL is tried once, and what curl said is kept.
    let e = sources::curl("file:///no/such/file/for/ritsu").unwrap_err();
    assert!(e.en.starts_with("cannot fetch file:///no/such/file/for/ritsu (tried 1 times):"), "{}", e.en);
}

#[test]
fn the_real_e_gov_and_ecfr_when_the_level_is_platforms() {
    if level::level() != Some(Level::Platforms) {
        println!("not asked: RITSU_TEST_LEVEL is not platforms, so the real e-Gov and eCFR were not asked");
        return;
    }
    if no_curl() {
        return;
    }
    let egov = Egov { base: sources::EGOV.to_string() };
    let (xml, rev) = egov.law_data("129AC0000000089", "2026-10-01", Some("MainProvision-Article_142")).unwrap();
    println!("e-Gov served 第142条 as of 2026-10-01 from {rev}");
    let ours = std::fs::read(fixture(LAW).join("MainProvision-Article_142.xml")).unwrap();
    assert!(sources::same_text(&xml, &ours), "the copy's text is e-Gov's");
    let ecfr = Ecfr { base: sources::ECFR.to_string() };
    let cfr = ecfr.section("29 CFR 1910", "2026-01-01", "1910.157").unwrap();
    assert!(sources::same_text(&cfr, &std::fs::read(fixture(CFR)).unwrap()), "the copy's text is the eCFR's");
}
