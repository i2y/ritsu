//! yuen reading the suite through ritsu's ports, in English: the twin of suite.rs. The Japanese
//! tests keep the fixtures they read (the Japanese civil code and stamp duty, the Japanese
//! books and workflows); these read English ones: the copies of the eCFR, the bank holidays of
//! England and Wales, an English book, workflow, context and `.proto`. What only e-Gov's text can
//! show has no twin here: the E106 of an article a calendar does not pin (koyomi pins only
//! e-Gov's laws) and the amendment of an article a calendar pins.

mod common;

use ritsu_ports::{Item, Items, Said};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;
use yuen::names::Tool;

/// The end of every artifact the links of a fixture name, by its naming: the hash and the text.
fn ends(dir: &str) -> BTreeMap<String, (String, String)> {
    let c = common::check(dir);
    assert!(!c.has_errors(), "{dir}: {:?}", c.diags.iter().map(|d| (d.code, d.message.en.clone())).collect::<Vec<_>>());
    let m = c.model.as_ref().unwrap();
    m.artifacts.iter().map(|(n, e)| (n.text(), e.as_ref().map(|e| (e.hash.clone(), String::from_utf8_lossy(&e.bytes).to_string())).unwrap())).collect()
}

fn hash_of(ends: &BTreeMap<String, (String, String)>, naming: &str) -> String {
    ends.get(naming).unwrap_or_else(|| panic!("no end for {naming}: {:?}", ends.keys().collect::<Vec<_>>())).0.clone()
}

fn codes(c: &yuen::check::Checked) -> Vec<&'static str> {
    c.diags.iter().map(|d| d.code).collect()
}

fn path_of(t: &common::TempDir, name: &str) -> String {
    t.path().join(name).to_string_lossy().to_string()
}

// ── C.1 the ports, joined ─────────────────────────────────────────────────────

/// The binary of this crate holds no other language: for each, a project that names one of its
/// things is told so (E206) with exit 2, the language named, and to run `ritsu yuen`. The
/// English half of the rulec fixture is one file; geas's fixture is English already.
#[test]
fn the_binary_refuses_each_language_it_does_not_hold_in_english() {
    for (what, root, tool) in [
        ("tests/fixtures/rulec/fire_extinguishers.req", "tests/fixtures/rulec", "rulec"),
        ("tests/fixtures/fee_rules", "tests/fixtures/fee_rules", "rulec"),
        ("tests/fixtures/calendar_sources", "tests/fixtures/calendar_sources", "koyomi"),
        ("tests/fixtures/refunds_book", "tests/fixtures/refunds_book", "chobo"),
        ("tests/fixtures/delivery_flow", "tests/fixtures/delivery_flow", "dandori"),
        ("tests/fixtures/ordering_terms", "tests/fixtures/ordering_terms", "sakai"),
    ] {
        let r = common::yuen(Path::new("."), &["check", what, "--root", root]);
        assert_eq!(r.code, 2, "{what}: {}{}", r.stdout, r.stderr);
        assert!(r.stdout.contains("[E206]: ") && r.stdout.contains(&format!("this yuen cannot read {tool} ")), "{what}: {}", r.stdout);
        assert!(r.stdout.contains(&format!("`ritsu yuen check {what} --root {root}`")), "{what}: {}", r.stdout);
        let joined = common::run(&["check", what, "--root", root]);
        assert_eq!(joined.code, 0, "{what}: {}{}", joined.stdout, joined.stderr);
    }
}

/// A port that counts how often each file is asked.
struct Counting {
    inner: Rc<dyn Items>,
    asked: RefCell<BTreeMap<String, usize>>,
}

impl Items for Counting {
    fn items(&self, root: &Path, file: &str) -> Result<Vec<Item>, Vec<Said>> {
        *self.asked.borrow_mut().entry(file.to_string()).or_default() += 1;
        self.inner.items(root, file)
    }
}

/// Each file is asked once in a run, however many links name what is in it.
#[test]
fn each_file_is_asked_once_in_english() {
    let counting = Rc::new(Counting { inner: Rc::new(rulec::ports::Engine::new()), asked: RefCell::new(BTreeMap::new()) });
    let suite = common::suite_where(Some((Tool::Rulec, counting.clone())));
    let dir = "tests/fixtures/fee_rules".to_string();
    let c = yuen::check::check_with(std::slice::from_ref(&dir), Some(&dir), suite).unwrap();
    assert!(!c.has_errors());
    let asked = counting.asked.borrow();
    assert_eq!(asked.get("rules/extension_fees.rule"), Some(&1), "{asked:?}");
}

/// A port that gives one thing with no definition.
struct Empty;

impl Items for Empty {
    fn items(&self, _root: &Path, file: &str) -> Result<Vec<Item>, Vec<Said>> {
        Ok(vec![Item { naming: ritsu_base::naming::Name::file(Tool::Rulec, file).with("table", "empty"), lines: (3, 3), text: String::new() }])
    }
}

/// A definition with nothing in it is no end to hold a link to: E203, and not the hash of an
/// empty text.
#[test]
fn an_empty_definition_is_no_end_in_english() {
    let t = common::TempDir::new("empty");
    t.write("a.rule", b"rule a v1\n");
    t.write("t.req", b"requirements t v1\nrole development\n\nrequirement r1\n  text \"x\"\n  owner development\n  decided 2026-10-03 by development \"y\"\n  satisfied by rulec \"a.rule\" table empty\n  not verified \"z\"\n");
    let mut suite = common::suite_where(Some((Tool::Rulec, Rc::new(Empty))));
    // what answers for the rule's sources: a stand-in that says it pins nothing
    struct NoSources;
    impl ritsu_ports::Sources for NoSources {
        fn sources(&self, _file: &Path) -> Result<Vec<ritsu_ports::Source>, Vec<Said>> {
            Ok(vec![])
        }
    }
    suite.sources.insert("rulec".into(), Rc::new(NoSources));
    let dir = t.path().to_string_lossy().to_string();
    let c = yuen::check::check_with(std::slice::from_ref(&dir), Some(&dir), suite).unwrap();
    let d = c.diags.iter().find(|d| d.code == "E203").unwrap_or_else(|| panic!("{:?}", codes(&c)));
    let text = d.render(ritsu_base::text::Lang::En);
    assert!(text.contains("rulec \"a.rule\" table empty gives no definition"), "{text}");
    assert!(c.model.as_ref().unwrap().artifacts.values().all(|e| e.is_err()));
}

/// A rule or a calendar that does not pass its language's check makes no end: E203, with what its
/// language says.
#[test]
fn a_calendar_that_does_not_pass_its_check_makes_no_end_in_english() {
    let t = common::fixture("calendar_sources");
    let k = t.path().join("calendar_sources");
    std::fs::copy("../koyomi/examples/period_of_months_two_readings.cal", k.join("comparison.cal")).unwrap();
    let req = std::fs::read_to_string(k.join("period_of_months.req")).unwrap();
    std::fs::write(k.join("period_of_months.req"), req.replacen("  verified by koyomi \"period_of_months.cal\" claim last_day_after_origin", "  verified by koyomi \"comparison.cal\" claim the_two_readings_agree", 1)).unwrap();
    let c = common::check(&k.to_string_lossy());
    let d = c.diags.iter().find(|d| d.code == "E203").unwrap_or_else(|| panic!("{:?}", codes(&c)));
    let text = d.render(ritsu_base::text::Lang::En);
    assert!(text.contains("koyomi cannot answer for koyomi \"comparison.cal\" claim the_two_readings_agree"), "{text}");
    assert!(text.contains("what koyomi says: ["), "{text}");
}

// ── C.2 rulec ─────────────────────────────────────────────────────────────────

/// The things of a rule are its own (rulec's `Items`): a table, a clause, a definition, an input,
/// an output, each by its name. A name written as its alias is E202, with the name; a name the
/// rule does not have is E202. The end of a thing is its lines as `rulec fmt` writes them; a
/// rule named whole is its bytes.
#[test]
fn the_things_of_a_rule_in_english() {
    let e = ends("tests/fixtures/fee_rules");
    let rule = "rulec \"rules/extension_fees.rule\"";
    assert_eq!(hash_of(&e, &format!("{rule} table standard_fee")), "9c75e7072bde2938");
    assert_eq!(hash_of(&e, &format!("{rule} clause no_extension")), "2391acd18f47abcb");
    assert_eq!(hash_of(&e, &format!("{rule} table small_entity_fee")), "5fd98a7705c641eb");
    assert_eq!(hash_of(&e, &format!("{rule} define reduced_period")), "611179d6b0c376c4");
    assert_eq!(e[&format!("{rule} define reduced_period")].1, "define reduced_period(reduced) : bool = made <= 2027-03-31  @cfr \"§1.27\"");

    let t = common::fixture("fee_rules");
    let dir = path_of(&t, "fee_rules");
    t.write(
        "fee_rules/names.req",
        b"requirements names v1\nrole checker\n\nrequirement r1\n  text \"x\"\n  owner checker\n  decided 2026-10-03 by checker \"y\"\n  satisfied by rulec \"rules/extension_fees.rule\"\n  satisfied by rulec \"rules/extension_fees.rule\" output extension_fee\n  satisfied by rulec \"rules/extension_fees.rule\" input months\n  satisfied by rulec \"rules/extension_fees.rule\" output fee\n  satisfied by rulec \"rules/extension_fees.rule\" table no_such_table\n  not verified \"z\"\n",
    );
    let c = common::check(&dir);
    let m = c.model.as_ref().unwrap();
    let file = |p: &str| m.artifacts.iter().find(|(n, _)| n.path == p && n.items.is_empty()).unwrap().1.as_ref().unwrap().hash.clone();
    assert_eq!(file("rules/extension_fees.rule"), "a6276fdf51f978ba");
    let e202: Vec<String> = c.diags.iter().filter(|d| d.code == "E202").map(|d| d.render(ritsu_base::text::Lang::En)).collect();
    assert_eq!(e202.len(), 2, "{e202:?}");
    assert!(e202[0].contains("has no output fee") && e202[0].contains("`fee` is the alias of extension_fee") && e202[0].contains("Did you mean: rulec \"rules/extension_fees.rule\" output extension_fee"), "{}", e202[0]);
    assert!(e202[1].contains("has no table no_such_table"), "{}", e202[1]);
}

/// The articles a rule pins (rulec's `Sources`): `yuen api` gives them with each artifact.
#[test]
fn the_articles_a_rule_pins_in_english() {
    let r = common::run(&["api", "tests/fixtures/fee_rules", "--root", "tests/fixtures/fee_rules"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    let pins = |text: &str| -> Vec<(String, String)> {
        let a = v["artifacts"].as_array().unwrap().iter().find(|a| a["text"] == text).unwrap();
        a["pins"].as_array().unwrap().iter().map(|p| (p["fragment"].as_str().unwrap().to_string(), p["sha256"].as_str().unwrap().to_string())).collect()
    };
    // the pins of the file the thing is in: both sections, whichever table is asked
    let both = [("§1.17".to_string(), "9b48f2c46284140a".to_string()), ("§1.27".to_string(), "fea1289ebe564f77".to_string())];
    assert_eq!(pins("rulec \"rules/extension_fees.rule\" table small_entity_fee"), both);
    assert_eq!(pins("rulec \"rules/extension_fees.rule\" table standard_fee"), both);
}

// ── C.3 koyomi ────────────────────────────────────────────────────────────────

/// The things of a dates file (koyomi's `Items`): a date is its block, a claim its line, each its
/// own end; and a calendar's file source borrowed is the calendar's, with its pin.
#[test]
fn the_dates_and_claims_of_a_calendar_in_english() {
    let e = ends("tests/fixtures/calendar_sources");
    let cal = "koyomi \"period_of_months.cal\"";
    assert_eq!(hash_of(&e, &format!("{cal} date first_day")), "f7fbde9442fad208");
    assert_eq!(hash_of(&e, &format!("{cal} date last_day")), "e4d5741e15aeb403");
    assert_eq!(hash_of(&e, &format!("{cal} date last_day_moved")), "da1536d15d1da2b6");
    assert_eq!(hash_of(&e, &format!("{cal} claim last_day_after_origin")), "d88a046cef583e8a");
    assert_eq!(hash_of(&e, &format!("{cal} claim moved_on_or_after_last_day")), "82aece4919f854bb");
    assert_eq!(e[&format!("{cal} claim moved_on_or_after_last_day")].1, "moved_on_or_after_last_day  : last_day_moved >= last_day");
    assert_eq!(e[&format!("{cal} date last_day_moved")].1, "date last_day_moved = last_day\nif closed + 1 day");
    let pay = "koyomi \"payment_20th_close_next_10th.cal\"";
    assert_eq!(hash_of(&e, &format!("{pay} date payment")), "77727589d8318079");
    assert_eq!(hash_of(&e, &format!("{pay} claim paid_on_a_business_day")), "a30c094c89e283df");

    let r = common::run(&["api", "tests/fixtures/calendar_sources", "--root", "tests/fixtures/calendar_sources"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    let bank = v["sources"].as_array().unwrap().iter().find(|s| s["name"] == "bank_holidays").unwrap();
    assert_eq!(bank["kind"], "file");
    assert_eq!(bank["path"], "calendars/data/bank-holidays.json");
    assert_eq!(bank["sha256"], "538b3482c28b85ec");
    assert_eq!(bank["borrowed"]["text"], "koyomi \"calendars/england_and_wales.cal\" source bank_holidays");
}

/// A file taken into a calendar, changed (its copy and its pin written again), marks the links of
/// the requirement that reads it, and no other: the dates that do not cite it keep their ends
/// (DESIGN 4.3). The twin of the amendment of an article, which only e-Gov's laws can show.
#[test]
fn a_file_taken_into_a_calendar_marks_only_what_reads_it_in_english() {
    let t = common::fixture("calendar_sources");
    let k = t.path().join("calendar_sources");
    let data = k.join("calendars/data/bank-holidays.json");
    let text = std::fs::read_to_string(&data).unwrap().replacen("\"title\":\"Early May bank holiday\",\"date\":\"2026-05-04\"", "\"title\":\"Early May bank holiday\",\"date\":\"2026-05-05\"", 1);
    std::fs::write(&data, &text).unwrap();
    let pin = ritsu_base::sha256::short(text.as_bytes());
    let cal = std::fs::read_to_string(k.join("calendars/england_and_wales.cal")).unwrap().replace("sha256:538b3482c28b85ec", &format!("sha256:{pin}"));
    std::fs::write(k.join("calendars/england_and_wales.cal"), cal).unwrap();
    let c = common::check(&k.to_string_lossy());
    let marked: Vec<(&str, usize)> = c.diags.iter().map(|d| (d.code, d.line.unwrap_or(0))).collect();
    assert_eq!(marked, [("E302", 35), ("E302", 38), ("E302", 40)], "{:?}", c.diags.iter().map(|d| d.message.en.clone()).collect::<Vec<_>>());
}

// ── C.4 chobo ─────────────────────────────────────────────────────────────────

/// The things of a book (chobo's `Items`): the end of a transfer is the JSON of DESIGN 3.2; two
/// outside accounts in dollars have one end.
#[test]
fn the_accounts_and_transfers_of_a_book_in_english() {
    let e = ends("tests/fixtures/refunds_book");
    let book = "chobo \"refunds.book\"";
    assert_eq!(hash_of(&e, &format!("{book} transfer refund")), "d4eb1e0e94a3b78f");
    assert_eq!(e[&format!("{book} transfer refund")].1.len(), 1176);
    assert_eq!(hash_of(&e, &format!("{book} transfer sale")), "699a4feaa65a1dfe");
    assert_eq!(hash_of(&e, &format!("{book} account refundable")), "49d9b3dd2d4bf83b");
    assert_eq!(hash_of(&e, &format!("{book} account refunded")), "8f80f74b076ccea8");
    let items = common::suite().index.items(Tool::Chobo, Path::new("tests/fixtures/refunds_book"), "refunds.book").unwrap().unwrap();
    let sales = items.iter().find(|i| i.naming.text() == format!("{book} account sales")).unwrap();
    assert_eq!(ritsu_base::sha256::short(sales.text.as_bytes()), "8f80f74b076ccea8");
    let mut failures = Vec::new();
    common::golden("tests/golden/ends/chobo-refund.json", &e[&format!("{book} transfer refund")].1, &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// An operation of a transfer (`transfer refund operation post`, ritsu's DESIGN 6.2) is a thing of
/// the book too: its end is its transfer's, with the operation and the names of the reasons it can
/// be refused with. A link to it is read like any other: not looked at yet is E301, an operation
/// the transfer does not have is E202 with the ones it has.
#[test]
fn the_operations_of_a_transfer_in_english() {
    let book = "chobo \"refunds.book\"";
    let items = common::suite().index.items(Tool::Chobo, Path::new("tests/fixtures/refunds_book"), "refunds.book").unwrap().unwrap();
    let ops: Vec<String> = items.iter().filter(|i| i.kind() == "operation").map(|i| i.naming.text()).collect();
    assert_eq!(ops, [format!("{book} transfer sale operation do"), format!("{book} transfer refund operation hold"), format!("{book} transfer refund operation post"), format!("{book} transfer refund operation void")]);
    let text = |n: &str| items.iter().find(|i| i.naming.text() == n).unwrap().text.clone();
    let (refund, post) = (text(&format!("{book} transfer refund")), text(&format!("{book} transfer refund operation post")));
    // the transfer's definition, with the operation and its refusals beside it
    let without = |t: &str, keys: &[&str]| -> Vec<(String, ritsu_base::json::Json)> {
        let j = ritsu_base::json::parse(t).unwrap();
        j.as_obj().unwrap().iter().filter(|(k, _)| !keys.contains(&k.as_str())).cloned().collect()
    };
    assert_eq!(without(&post, &["operation", "refusals"]), without(&refund, &[]));
    let j = ritsu_base::json::parse(&post).unwrap();
    assert_eq!(j.get("operation").and_then(|o| o.as_str()), Some("post"));
    assert!(j.get("refusals").and_then(|r| r.as_arr()).is_some(), "{post}");
    assert_ne!(refund, post);
    let t = common::fixture("refunds_book");
    let d = t.path().join("refunds_book");
    common::edit(&d, "refund.req", "  satisfied by chobo \"refunds.book\" transfer refund\n", "  satisfied by chobo \"refunds.book\" transfer refund operation post\n");
    let c = common::check(&d.to_string_lossy());
    assert_eq!(codes(&c), ["E303"], "the link names the operation now, whose end is not the transfer's: {:?}", c.diags.iter().map(|x| x.message.en.clone()).collect::<Vec<_>>());
    common::edit(&d, "refund.req", "transfer refund operation post\n", "transfer refund operation do\n");
    let c = common::check(&d.to_string_lossy());
    let d202: Vec<String> = c.diags.iter().filter(|x| x.code == "E202").map(|x| x.render(ritsu_base::text::Lang::En)).collect();
    assert!(d202.len() == 1 && d202[0].contains("refunds.book has no operation do") && d202[0].contains("transfer refund operation hold"), "{d202:?}");
}

// ── C.6 proto ─────────────────────────────────────────────────────────────────

fn proto_end(dir: &Path, file: &str, naming: &str) -> String {
    let ps = yuen::proto::load(dir, file).map_err(|(f, e)| format!("{f}: {}", e.en)).unwrap();
    let n = ritsu_base::naming::parse_one(naming).unwrap();
    yuen::proto::end_text(&ps, &n).ok().unwrap()
}

/// The elements of a `.proto` are read by yuen with ritsu's reader: the twin of the test of the
/// same name, on the fixture `warehouse_proto`, whose files are the same text as the fixture
/// `proto`'s, so the ends are the golden files the Japanese test has. The end of a method takes
/// in the messages and enums it reaches: a field added to a request changes its method and its
/// service, not the other method; a comment changes nothing. A child named outside its parent is
/// E012.
#[test]
fn the_elements_of_a_proto_in_english() {
    let e = ends("tests/fixtures/warehouse_proto");
    assert_eq!(hash_of(&e, "proto \"warehouse.proto\" service StockService method Reserve"), "cadb2fc727e9af81");
    assert_eq!(hash_of(&e, "proto \"warehouse.proto\" service StockService method Release"), "2a0912a66889e515");
    let mut failures = Vec::new();
    for (name, naming) in [
        ("proto-reserve.txt", "proto \"warehouse.proto\" service StockService method Reserve"),
        ("proto-stockservice.txt", "proto \"warehouse.proto\" service StockService"),
        ("proto-stock.txt", "proto \"warehouse.proto\" enum Stock"),
        ("proto-fulfill.txt", "proto \"fulfillment.proto\" service FulfillmentService method Fulfill"),
    ] {
        common::golden(&format!("tests/golden/ends/{name}"), &proto_end(Path::new("tests/fixtures/warehouse_proto"), &ritsu_base::naming::parse_one(naming).unwrap().path, naming), &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));

    let t = common::fixture("warehouse_proto");
    let d = t.path().join("warehouse_proto");
    let ends_of = |d: &Path| -> Vec<String> { ["service StockService method Reserve", "service StockService method Release", "service StockService"].iter().map(|x| proto_end(d, "warehouse.proto", &format!("proto \"warehouse.proto\" {x}"))).collect() };
    let before = ends_of(&d);
    let src = std::fs::read_to_string(d.join("warehouse.proto")).unwrap();
    std::fs::write(d.join("warehouse.proto"), src.replace("// Let a reservation go.", "// Let go of a reservation.")).unwrap();
    assert_eq!(before, ends_of(&d), "a comment changes no end");
    std::fs::write(d.join("warehouse.proto"), src.replace("  int32 quantity = 2;\n}", "  int32 quantity = 2;\n  string note = 3;\n}")).unwrap();
    let field = ends_of(&d);
    assert_ne!(before[0], field[0], "Reserve takes the request in");
    assert_eq!(before[1], field[1], "Release does not");
    assert_ne!(before[2], field[2], "the service takes every method in");

    t.write("warehouse_proto/outside.req", b"requirements outside v1\nrole checker\n\nrequirement r1\n  text \"x\"\n  owner checker\n  decided 2026-10-03 by checker \"y\"\n  satisfied by proto \"warehouse.proto\" method Release\n  not verified \"z\"\n");
    let c = common::check(&d.to_string_lossy());
    assert!(codes(&c).contains(&"E012"), "{:?}", codes(&c));
}

// ── C.7 borrowed sources and E107 ─────────────────────────────────────────────

/// A source borrowed from a calendar is the calendar's pins and copies (koyomi's `Sources`): the
/// requirements read from it have the ends they have read from copies of their own. (An article
/// the calendar does not pin, E106, is only e-Gov's: koyomi pins no eCFR section.)
#[test]
fn a_source_borrowed_from_a_calendar_in_english() {
    let c = common::check("tests/fixtures/calendar_sources");
    let p = c.project.as_ref().unwrap();
    let m = c.model.as_ref().unwrap();
    let end = |name: &str| m.req_ends[p.find_req(name).unwrap()[0]].as_ref().unwrap().hash.clone();
    assert_eq!(end("first_day"), "29836b1424129486");
    assert_eq!(end("last_day"), "534cb97e1ca8a94c");
    assert_eq!(end("last_day_moved"), "edc4cc0bb4910f57");
    assert_eq!(end("payment_day"), "dd7089d2bc2f76e0");
    // read from the file the calendar pins, the requirement's end is what its own `file` source gives
    let own = "text An end that falls on a day closed in England and Wales moves to the day after\nfrom file calendars/data/bank-holidays.json sha256:538b3482c28b85ec\nin force 2026-10-01..\n";
    assert_eq!(ritsu_base::sha256::short(own.as_bytes()), "edc4cc0bb4910f57");
}

/// A requirement that copies a section itself, and a rule that meets it pinning the same one: the
/// same text is no E107, whatever the bytes; another text is, with the diff.
#[test]
fn a_requirement_and_a_rule_that_read_one_article_in_english() {
    let c = common::check("tests/fixtures/fee_rules");
    assert!(!codes(&c).contains(&"E107"), "{:?}", codes(&c));
    let t = common::fixture("fee_rules");
    let d = t.path().join("fee_rules");
    let copy = d.join("sources/law/37-CFR-1@2026-01-01/1.17.xml");
    let text = std::fs::read_to_string(&copy).unwrap().replacen("235.00", "240.00", 1);
    std::fs::write(&copy, &text).unwrap();
    let pin = ritsu_base::sha256::short(text.as_bytes());
    let req = std::fs::read_to_string(d.join("extension_fees.req")).unwrap().replace("\"§1.17\" sha256:9b48f2c46284140a", &format!("\"§1.17\" sha256:{pin}"));
    std::fs::write(d.join("extension_fees.req"), req).unwrap();
    let c = common::check(&d.to_string_lossy());
    let e = c.diags.iter().find(|x| x.code == "E107").unwrap_or_else(|| panic!("{:?}", codes(&c)));
    let shown = e.render(ritsu_base::text::Lang::En);
    assert!(shown.contains("standard_fee reads cfr §1.17, and rulec \"rules/extension_fees.rule\" pins §1.17 with another text"), "{shown}");
    assert!(shown.contains("240.00") && shown.contains("235.00"), "{shown}");
}

// ── C.8 renamed things ────────────────────────────────────────────────────────

fn renamed(fixture: &str, file: &str, from: &str, to: &str) -> (common::TempDir, yuen::check::Checked) {
    let t = common::fixture(fixture);
    let d = t.path().join(fixture);
    let src = std::fs::read_to_string(d.join(file)).unwrap();
    assert!(src.contains(from), "{file} has no {from:?}");
    std::fs::write(d.join(file), src.replace(from, to)).unwrap();
    let c = common::check(&d.to_string_lossy());
    (t, c)
}

fn e202(c: &yuen::check::Checked) -> Vec<String> {
    c.diags.iter().filter(|d| d.code == "E202").map(|d| d.render(ritsu_base::text::Lang::En)).collect()
}

/// A thing whose definition is the same under another name is said to be renamed (DESIGN 4.5):
/// a book's transfer, whose end leaves its name out; two outside accounts are both candidates. A
/// date's definition holds its name, so the candidates are the dates no link names, and with one
/// the diff from what was looked at.
#[test]
fn renamed_things_are_found_in_english() {
    let (_t, c) = renamed("refunds_book", "refunds.book", "transfer refund(", "transfer refund_request(");
    let d = e202(&c);
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(d[0].contains("It looks renamed") && d[0].contains("Did you mean: chobo \"refunds.book\" transfer refund_request"), "{}", d[0]);

    let (_t, c) = renamed("refunds_book", "refunds.book", "refunded", "paid_back");
    let d = e202(&c);
    assert!(d[0].contains("Did you mean: chobo \"refunds.book\" account sales, chobo \"refunds.book\" account paid_back"), "{}", d[0]);

    let (_t, c) = renamed("calendar_sources", "period_of_months.cal", "first_day", "start_day");
    let d = e202(&c);
    assert!(d[0].contains("Did you mean: koyomi \"period_of_months.cal\" date start_day"), "{}", d[0]);
    assert!(d[0].contains("- date first_day") && d[0].contains("+ date start_day"), "{}", d[0]);
}

// ── C.9 affected ──────────────────────────────────────────────────────────────

fn affected(args: &[&str], golden: &str, code: i32, failures: &mut Vec<String>) -> String {
    let r = common::run(args);
    assert_eq!(r.code, code, "{args:?}: {}{}", r.stdout, r.stderr);
    common::golden(&format!("tests/golden/affected/{golden}"), &r.stdout, failures);
    r.stdout
}

/// A change to a requirement's text, to the copy of a file a calendar pins and a requirement
/// borrows, to a rule, and to a requirement's own copy of a section a rule pins too.
#[test]
fn affected_follows_requirements_sources_and_rules_in_english() {
    let mut failures = Vec::new();
    let k = "tests/fixtures/calendar_sources";
    let out = affected(&["affected", k, "--root", k, "--diff", "tests/fixtures/calendar_sources/changes/text.diff"], "calendar-text.txt", 0, &mut failures);
    assert!(out.contains("  last_day_moved: its text or period changes, so its links are to be looked at again\n"), "{out}");
    let out = affected(&["affected", k, "--root", k, "--diff", "tests/fixtures/calendar_sources/changes/copy.diff"], "calendar-copy.txt", 0, &mut failures);
    assert!(out.contains("calendars/data/bank-holidays.json (bank_holidays): cited by last_day_moved\n"), "{out}");
    let out = affected(&["affected", k, "--root", k, "--diff", "tests/fixtures/calendar_sources/changes/copy.diff", "--lang", "ja"], "calendar-copy.ja.txt", 0, &mut failures);
    assert!(out.contains("（bank_holidays）: 引く要件 last_day_moved"), "{out}");
    let r = "tests/fixtures/fee_rules";
    let out = affected(&["affected", r, "--root", r, "--diff", "tests/fixtures/fee_rules/changes/rule.diff"], "fee-rule.txt", 0, &mut failures);
    assert!(out.contains("  rulec \"rules/extension_fees.rule\": met by standard_fee, small_entity_fee\n"), "{out}");
    let out = affected(&["affected", r, "--root", r, "--diff", "tests/fixtures/fee_rules/changes/copy.diff"], "fee-copy.txt", 0, &mut failures);
    assert!(out.contains("(cfr §1.17): cited by standard_fee; pinned by rulec \"rules/extension_fees.rule\""), "{out}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A file of a scope no requirement leads to, changed, is out of every requirement's reach:
/// exit 1. A diff that does not read, and a `--map` of a spec no link names, are exit 2.
#[test]
fn affected_says_what_no_requirement_reaches_in_english() {
    let t = common::fixture("geas");
    let g = t.path().join("geas");
    let req = std::fs::read_to_string(g.join("greeter.req")).unwrap();
    std::fs::write(g.join("greeter.req"), req.replace("scope file \"greeter/server.py\"", "scope file \"greeter/\"")).unwrap();
    std::fs::write(g.join("readme.diff"), "--- a/greeter/notes.txt\n+++ b/greeter/notes.txt\n@@ -1 +1 @@\n-a\n+b\n").unwrap();
    std::fs::write(g.join("greeter/notes.txt"), "b\n").unwrap();
    let dir = g.to_string_lossy().to_string();
    let diff = g.join("readme.diff").to_string_lossy().to_string();
    let r = common::run(&["affected", &dir, "--root", &dir, "--diff", &diff]);
    assert_eq!(r.code, 1, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains("changes no requirement reaches:\n") && r.stdout.contains("greeter/notes.txt (in the scope file \"greeter\", and no requirement leads to it)"), "{}", r.stdout);

    std::fs::write(g.join("bad.diff"), "@@ -1 +1 @@\n-a\n+b\n").unwrap();
    let bad = g.join("bad.diff").to_string_lossy().to_string();
    let r = common::run(&["affected", &dir, "--root", &dir, "--diff", &bad]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.contains("The diff cannot be read"), "{}", r.stderr);
    // a spec that is the greeter's own, copied under a name no link of the project names
    std::fs::copy(g.join("greeter/greeter.geas"), g.join("greeter/other.geas")).unwrap();
    let spec = g.join("greeter/other.geas").to_string_lossy().to_string();
    let r = common::run(&["affected", &dir, "--root", &dir, "--diff", &diff, "--map", &format!("{spec}=x.jsonl")]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.contains("No link of the project names the spec"), "{}", r.stderr);
}

// ── dandori and sakai ─────────────────────────────────────────────────────────

/// The things of a workflow (dandori's `Items`, from its syntax alone) and the terms of a context
/// (sakai's): each its own end.
#[test]
fn the_tasks_of_a_workflow_and_the_terms_of_a_context_in_english() {
    let e = ends("tests/fixtures/delivery_flow");
    let flow = "dandori \"arrange_delivery.flow\"";
    assert_eq!(hash_of(&e, &format!("{flow} task book_next_day")), "06fdb81a96fb18d4");
    assert!(e[&format!("{flow} task book_next_day")].1.starts_with("task book_next_day(order_id: string, recipient: string?) -> Booking\n  http POST"));
    assert_eq!(e[&format!("{flow} record Booking field tracking_number")].1, "tracking_number : string");
    let e = ends("tests/fixtures/ordering_terms");
    assert_eq!(hash_of(&e, "sakai \"contexts/ordering.ctx\" term cancel"), "4687c56296431a6b");
    assert_eq!(e["sakai \"contexts/ordering.ctx\" term cancel"].1, "cancel \"Cancelling an order before it ships, at the customer's request\"\nmeans enum OrderStatus value ORDER_STATUS_CANCELLED");
}
