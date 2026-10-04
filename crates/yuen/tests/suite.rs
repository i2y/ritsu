//! yuen reading the suite through ritsu's ports (PLAN C.1–C.9 as rewritten for ritsu's D.7): the
//! languages are handed to it, each file asked once; the end of a thing of a rule, a calendar, a
//! book, a spec, a workflow or a context is its definition as its language gives it (ritsu's
//! DESIGN 6.4), of a `.proto` element the text of DESIGN 3.4; a borrowed source is the rule's or
//! the calendar's own; a renamed thing is found; and `affected` follows a diff to the
//! requirements and their owners. Everything runs in this process, with every language joined as
//! `ritsu yuen` joins them, from the crate's directory.

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
/// things is refused with exit 2, the language named, and told to run `ritsu yuen`.
#[test]
fn the_binary_refuses_each_language_it_does_not_hold() {
    for (fixture, tool) in [("rulec", "rulec"), ("koyomi", "koyomi"), ("chobo", "chobo"), ("geas", "geas"), ("dandori", "dandori"), ("sakai", "sakai")] {
        let dir = format!("tests/fixtures/{fixture}");
        let r = common::yuen(Path::new("."), &["check", &dir, "--root", &dir]);
        assert_eq!(r.code, 2, "{fixture}: {}{}", r.stdout, r.stderr);
        assert!(r.stderr.contains(&format!("this yuen cannot read {tool} ")), "{fixture}: {}", r.stderr);
        assert!(r.stderr.contains(&format!("`ritsu yuen check {dir} --root {dir}`")), "{fixture}: {}", r.stderr);
        let joined = common::run(&["check", &dir, "--root", &dir]);
        assert_eq!(joined.code, 0, "{fixture}: {}{}", joined.stdout, joined.stderr);
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
fn each_file_is_asked_once() {
    let counting = Rc::new(Counting { inner: Rc::new(rulec::ports::Engine::new()), asked: RefCell::new(BTreeMap::new()) });
    let mut suite = common::suite();
    suite.items.insert("rulec".into(), counting.clone());
    let dir = "tests/fixtures/rulec".to_string();
    let c = yuen::check::check_with(std::slice::from_ref(&dir), Some(&dir), suite).unwrap();
    assert!(!c.has_errors());
    let asked = counting.asked.borrow();
    assert_eq!(asked.get("rules/印紙税の本則と軽減.rule"), Some(&1), "{asked:?}");
    assert_eq!(asked.get("rules/osha_extinguisher.rule"), Some(&1), "{asked:?}");
}

/// A port that gives one thing with no definition.
struct Empty;

impl Items for Empty {
    fn items(&self, _root: &Path, file: &str) -> Result<Vec<Item>, Vec<Said>> {
        Ok(vec![Item { naming: ritsu_base::naming::Name::file(Tool::Rulec, file).with("table", "空"), lines: (3, 3), text: String::new() }])
    }
}

/// A definition with nothing in it is no end to hold a link to (ritsu's PLAN 7.6): E203, and not
/// the hash of an empty text.
#[test]
fn an_empty_definition_is_no_end() {
    let t = common::TempDir::new("empty");
    t.write("a.rule", b"rule a v1\n");
    t.write("t.req", "requirements t v1\nrole 開発\n\nrequirement r1\n  text \"x\"\n  owner 開発\n  decided 2026-10-03 by 開発 \"y\"\n  satisfied by rulec \"a.rule\" table 空\n  not verified \"z\"\n".as_bytes());
    let mut suite = common::suite();
    suite.items.insert("rulec".into(), Rc::new(Empty));
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
    assert!(text.contains("rulec \"a.rule\" table 空 gives no definition"), "{text}");
    assert!(c.model.as_ref().unwrap().artifacts.values().all(|e| e.is_err()));
}

/// A rule or a calendar that does not pass its language's check makes no end: E203, with what its
/// language says.
#[test]
fn a_calendar_that_does_not_pass_its_check_makes_no_end() {
    let t = common::fixture("koyomi");
    let k = t.path().join("koyomi");
    std::fs::copy("../koyomi/examples/民法の期間_読み方の比較.cal", k.join("比較.cal")).unwrap();
    let req = std::fs::read_to_string(k.join("民法の期間.req")).unwrap();
    std::fs::write(k.join("民法の期間.req"), req.replacen("  verified by koyomi \"民法の期間.cal\" claim 満了日は起点より後", "  verified by koyomi \"比較.cal\" claim 満了日は起点より後", 1)).unwrap();
    let c = common::check(&k.to_string_lossy());
    let d = c.diags.iter().find(|d| d.code == "E203").unwrap_or_else(|| panic!("{:?}", codes(&c)));
    let text = d.render(ritsu_base::text::Lang::En);
    assert!(text.contains("koyomi cannot answer for koyomi \"比較.cal\" claim 満了日は起点より後"), "{text}");
    assert!(text.contains("what koyomi says: ["), "{text}");
}

// ── C.2 rulec ─────────────────────────────────────────────────────────────────

/// The things of a rule are its own (rulec's `Items`): a table, a clause, a definition, an input,
/// an output, each by its name. A name written as its alias is E202, with the name; a name the
/// rule does not have is E202. The end of a thing is its lines as `rulec fmt` writes them; a
/// rule named whole is its bytes.
#[test]
fn the_things_of_a_rule() {
    let e = ends("tests/fixtures/rulec");
    assert_eq!(hash_of(&e, "rulec \"rules/印紙税の本則と軽減.rule\" table 本則"), "21432c19a67fe72e");
    assert_eq!(hash_of(&e, "rulec \"rules/印紙税の本則と軽減.rule\" clause 非課税"), "5bc61581e0c31022");
    assert_eq!(hash_of(&e, "rulec \"rules/印紙税の本則と軽減.rule\" table 軽減"), "4ba4c2dec2c8262b");
    assert_eq!(hash_of(&e, "rulec \"rules/印紙税の本則と軽減.rule\" define 軽減期間"), "b5b30cefd66e58d6");
    assert_eq!(hash_of(&e, "rulec \"rules/osha_extinguisher.rule\" table distance"), "5681500476af8a56");
    assert_eq!(e["rulec \"rules/印紙税の本則と軽減.rule\" define 軽減期間"].1, "define 軽減期間(reduced) : bool = 作成日 <= 2027-03-31  @措置法 第91条");

    let t = common::fixture("rulec");
    let dir = path_of(&t, "rulec");
    t.write(
        "rulec/名前.req",
        "requirements 名前 v1\nrole 確認\n\nrequirement r1\n  text \"x\"\n  owner 確認\n  decided 2026-10-03 by 確認 \"y\"\n  satisfied by rulec \"rules/印紙税の本則と軽減.rule\"\n  satisfied by rulec \"rules/osha_extinguisher.rule\"\n  satisfied by rulec \"rules/印紙税の本則と軽減.rule\" output 印紙税額\n  satisfied by rulec \"rules/印紙税の本則と軽減.rule\" input 契約金額\n  satisfied by rulec \"rules/印紙税の本則と軽減.rule\" output tax\n  satisfied by rulec \"rules/印紙税の本則と軽減.rule\" table 無い表\n  not verified \"z\"\n".as_bytes(),
    );
    let c = common::check(&dir);
    let m = c.model.as_ref().unwrap();
    let file = |p: &str| m.artifacts.iter().find(|(n, _)| n.path == p && n.items.is_empty()).unwrap().1.as_ref().unwrap().hash.clone();
    assert_eq!(file("rules/印紙税の本則と軽減.rule"), "dc176eebd83f26e3");
    assert_eq!(file("rules/osha_extinguisher.rule"), "a52e955b88af88d6");
    let e202: Vec<String> = c.diags.iter().filter(|d| d.code == "E202").map(|d| d.render(ritsu_base::text::Lang::En)).collect();
    assert_eq!(e202.len(), 2, "{e202:?}");
    assert!(e202[0].contains("has no output tax") && e202[0].contains("`tax` is the alias of 印紙税額") && e202[0].contains("Did you mean: rulec \"rules/印紙税の本則と軽減.rule\" output 印紙税額"), "{}", e202[0]);
    assert!(e202[1].contains("has no table 無い表"), "{}", e202[1]);
}

/// The articles a rule pins (rulec's `Sources`): `yuen api` gives them with each artifact.
#[test]
fn the_articles_a_rule_pins() {
    let r = common::run(&["api", "tests/fixtures/rulec", "--root", "tests/fixtures/rulec"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    let pins = |text: &str| -> Vec<(String, String)> {
        let a = v["artifacts"].as_array().unwrap().iter().find(|a| a["text"] == text).unwrap();
        a["pins"].as_array().unwrap().iter().map(|p| (p["fragment"].as_str().unwrap().to_string(), p["sha256"].as_str().unwrap().to_string())).collect()
    };
    assert_eq!(pins("rulec \"rules/印紙税の本則と軽減.rule\" table 軽減"), [("別表第一".to_string(), "0ba69792e960021e".to_string()), ("第91条".to_string(), "85faf53f6f6e8196".to_string())]);
    assert_eq!(pins("rulec \"rules/osha_extinguisher.rule\" table distance"), [("§1910.157".to_string(), "c2a9ce966c7e2269".to_string())]);
}

// ── C.3 koyomi ────────────────────────────────────────────────────────────────

/// The things of a dates file (koyomi's `Items`): a date is its block, a claim its line, each its
/// own end; and a calendar's file source borrowed is the calendar's, with its pin.
#[test]
fn the_dates_and_claims_of_a_calendar() {
    let e = ends("tests/fixtures/koyomi");
    let cal = "koyomi \"民法の期間.cal\"";
    assert_eq!(hash_of(&e, &format!("{cal} date 起算日")), "15aa6c91d6aaae80");
    assert_eq!(hash_of(&e, &format!("{cal} date 満了日")), "23441fd408f07548");
    assert_eq!(hash_of(&e, &format!("{cal} date 満了日_142条")), "bba4761410179e6e");
    assert_eq!(hash_of(&e, &format!("{cal} claim 満了日は起点より後")), "5e7bb0ed51835dc7");
    assert_eq!(hash_of(&e, &format!("{cal} claim 142条の満了日は満了日以後")), "7c616f5dcc9503a8");
    assert_eq!(e[&format!("{cal} claim 142条の満了日は満了日以後")].1, "142条の満了日は満了日以後 : 満了日_142条 >= 満了日");
    assert_eq!(e[&format!("{cal} date 満了日_142条")].1, "date 満了日_142条(last_day_142) = 満了日                 @民法 第142条\nif closed + 1 day");
    let pay = "koyomi \"支払_20日締め翌月10日払い.cal\"";
    assert_eq!(hash_of(&e, &format!("{pay} date 支払日")), "feb9535fb9350ecc");
    assert_eq!(hash_of(&e, &format!("{pay} claim 営業日に払う")), "3cf8fc3fb2dda9d9");

    let r = common::run(&["api", "tests/fixtures/koyomi", "--root", "tests/fixtures/koyomi"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    let holidays = v["sources"].as_array().unwrap().iter().find(|s| s["name"] == "祝日").unwrap();
    assert_eq!(holidays["kind"], "file");
    assert_eq!(holidays["path"], "calendars/data/syukujitsu.csv");
    assert_eq!(holidays["sha256"], "cec37a743c96995c");
    assert_eq!(holidays["borrowed"]["text"], "koyomi \"calendars/東京の営業日.cal\" source 祝日");
}

/// An amendment of the 142nd article taken into the calendar (its copy and its pin written
/// again) marks the links of the requirement that reads the article, and no other: the dates
/// that do not cite it keep their ends (DESIGN 4.3).
#[test]
fn an_article_taken_into_a_calendar_marks_only_what_reads_it() {
    let t = common::fixture("koyomi");
    let k = t.path().join("koyomi");
    let copy = k.join("sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml");
    let text = std::fs::read_to_string(&copy).unwrap().replacen("その翌日に満了する", "その翌々日に満了する", 1);
    std::fs::write(&copy, &text).unwrap();
    let pin = ritsu_base::sha256::short(text.as_bytes());
    let cal = std::fs::read_to_string(k.join("民法の期間.cal")).unwrap().replace("第142条 sha256:fc8c35a0769d3b35", &format!("第142条 sha256:{pin}"));
    std::fs::write(k.join("民法の期間.cal"), cal).unwrap();
    let c = common::check(&k.to_string_lossy());
    let marked: Vec<(&str, usize)> = c.diags.iter().map(|d| (d.code, d.line.unwrap_or(0))).collect();
    assert_eq!(marked, [("E302", 37), ("E302", 40), ("E302", 42)], "{:?}", c.diags.iter().map(|d| d.message.en.clone()).collect::<Vec<_>>());
}

// ── C.4 chobo ─────────────────────────────────────────────────────────────────

/// The things of a book (chobo's `Items`): the end of a transfer is the JSON of DESIGN 3.2, the
/// same the prototype of stage A computed; two outside accounts in yen have one end.
#[test]
fn the_accounts_and_transfers_of_a_book() {
    let e = ends("tests/fixtures/chobo");
    let book = "chobo \"refunds.ja.book\"";
    assert_eq!(hash_of(&e, &format!("{book} transfer 返金")), "84e9ce254075c697");
    assert_eq!(e[&format!("{book} transfer 返金")].1.len(), 1206);
    assert_eq!(hash_of(&e, &format!("{book} transfer 売上計上")), "851ab806078168fe");
    assert_eq!(hash_of(&e, &format!("{book} account 返金できる残り")), "9f9b0d74872f62a4");
    assert_eq!(hash_of(&e, &format!("{book} account 返金済み")), "35a4ec5a2ee5eb06");
    let items = common::suite().items(Tool::Chobo, Path::new("tests/fixtures/chobo"), "refunds.ja.book").unwrap().unwrap();
    let sales = items.iter().find(|i| i.naming.text() == format!("{book} account 売上")).unwrap();
    assert_eq!(ritsu_base::sha256::short(sales.text.as_bytes()), "35a4ec5a2ee5eb06");
    let mut failures = Vec::new();
    common::golden("tests/golden/ends/chobo-返金.json", &e[&format!("{book} transfer 返金")].1, &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ── C.5 geas ──────────────────────────────────────────────────────────────────

/// The claims of a spec are geas's (`Items`, `Claims`): each by its name, a name the spec does
/// not have is E202. Scope rule 4 reads the record of the lines each claim ran: the server
/// traces to the requirements its claims check, with no link that names it; without a record it
/// does not, and E404 says why; a file no runtime reports is not traced.
#[test]
fn the_claims_of_a_spec_and_what_they_ran() {
    let e = ends("tests/fixtures/geas");
    assert_eq!(hash_of(&e, "geas \"greeter/greeter.geas\" claim \"rejects an empty name\""), "3f7c8d1b2bbcc309");
    assert!(e["geas \"greeter/greeter.geas\" claim \"rejects an empty name\""].1.starts_with("claim \"rejects an empty name\" {"));

    let t = common::fixture("geas");
    let g = t.path().join("geas");
    let req = std::fs::read_to_string(g.join("greeter.req")).unwrap();
    // a requirement only a claim checks, and no link that names the server
    t.write(
        "geas/greeter.req",
        "requirements greeter v1\nrole api \"decides what the service answers\"\n\nsource contract = file \"contract.md\" sha256:129685aad10b7b82\n\nscope file \"greeter/server.py\"\n\nrequirement rejects_an_empty_name\n  text \"GET /greet with an empty name answers 400\"\n  owner api\n  from @contract\n  not satisfied \"the claim says what the server does\"\n  verified by geas \"greeter/greeter.geas\" claim \"rejects an empty name\"\n".as_bytes(),
    );
    let dir = g.to_string_lossy().to_string();
    let c = common::check(&dir);
    assert!(!codes(&c).contains(&"E404"), "{:?}", codes(&c));
    let s = &c.model.as_ref().unwrap().scopes[0];
    assert_eq!(s.artifacts.len(), 1);
    assert!(s.untraced.is_empty());

    std::fs::remove_file(g.join("greeter/.geas/greeter.map.jsonl")).unwrap();
    let c = common::check(&dir);
    let d = c.diags.iter().find(|d| d.code == "E404").unwrap_or_else(|| panic!("{:?}", codes(&c)));
    let text = d.render(ritsu_base::text::Lang::En);
    assert!(text.contains("geas \"greeter/greeter.geas\" has no geas record; with one (`geas map`)"), "{text}");

    std::fs::write(g.join("greeter.req"), req.replace("scope file \"greeter/server.py\"", "scope file \"greeter/server_refactored.py\"")).unwrap();
    std::fs::copy("tests/fixtures/geas/greeter/.geas/greeter.map.jsonl", g.join("greeter/.geas/greeter.map.jsonl")).unwrap();
    let c = common::check(&dir);
    assert_eq!(codes(&c), ["E404"], "server_refactored.py is in no claim's record");

    std::fs::write(g.join("greeter.req"), req.replace("claim \"unknown paths are 404\"", "claim \"no such claim\"")).unwrap();
    let c = common::check(&dir);
    let d = c.diags.iter().find(|d| d.code == "E202").unwrap_or_else(|| panic!("{:?}", codes(&c)));
    assert!(d.render(ritsu_base::text::Lang::En).contains("has no claim \"no such claim\""));
}

// ── C.6 proto ─────────────────────────────────────────────────────────────────

fn proto_end(dir: &Path, file: &str, naming: &str) -> String {
    let ps = yuen::proto::load(dir, file).map_err(|(f, e)| format!("{f}: {}", e.en)).unwrap();
    let n = ritsu_base::naming::parse_one(naming).unwrap();
    yuen::proto::end_text(&ps, &n).ok().unwrap()
}

/// The elements of a `.proto` are read by yuen with ritsu's reader. The end of a method takes
/// in the messages and enums it reaches: a field added to a request changes its method and its
/// service, not the other method; a comment changes nothing. A child named outside its parent is
/// E012. The ends are golden files.
#[test]
fn the_elements_of_a_proto() {
    let e = ends("tests/fixtures/proto");
    assert_eq!(hash_of(&e, "proto \"warehouse.proto\" service StockService method Reserve"), "cadb2fc727e9af81");
    assert_eq!(hash_of(&e, "proto \"warehouse.proto\" service StockService method Release"), "2a0912a66889e515");
    let mut failures = Vec::new();
    for (name, naming) in [
        ("proto-reserve.txt", "proto \"warehouse.proto\" service StockService method Reserve"),
        ("proto-stockservice.txt", "proto \"warehouse.proto\" service StockService"),
        ("proto-stock.txt", "proto \"warehouse.proto\" enum Stock"),
        ("proto-fulfill.txt", "proto \"fulfillment.proto\" service FulfillmentService method Fulfill"),
    ] {
        common::golden(&format!("tests/golden/ends/{name}"), &proto_end(Path::new("tests/fixtures/proto"), &ritsu_base::naming::parse_one(naming).unwrap().path, naming), &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));

    let t = common::fixture("proto");
    let d = t.path().join("proto");
    let before: Vec<String> = ["service StockService method Reserve", "service StockService method Release", "service StockService"].iter().map(|x| proto_end(&d, "warehouse.proto", &format!("proto \"warehouse.proto\" {x}"))).collect();
    let src = std::fs::read_to_string(d.join("warehouse.proto")).unwrap();
    std::fs::write(d.join("warehouse.proto"), src.replace("// Let a reservation go.", "// Let go of a reservation.")).unwrap();
    let comment: Vec<String> = ["service StockService method Reserve", "service StockService method Release", "service StockService"].iter().map(|x| proto_end(&d, "warehouse.proto", &format!("proto \"warehouse.proto\" {x}"))).collect();
    assert_eq!(before, comment, "a comment changes no end");
    std::fs::write(d.join("warehouse.proto"), src.replace("  int32 quantity = 2;\n}", "  int32 quantity = 2;\n  string note = 3;\n}")).unwrap();
    let field: Vec<String> = ["service StockService method Reserve", "service StockService method Release", "service StockService"].iter().map(|x| proto_end(&d, "warehouse.proto", &format!("proto \"warehouse.proto\" {x}"))).collect();
    assert_ne!(before[0], field[0], "Reserve takes the request in");
    assert_eq!(before[1], field[1], "Release does not");
    assert_ne!(before[2], field[2], "the service takes every method in");

    t.write("proto/外.req", "requirements 外 v1\nrole 確認\n\nrequirement r1\n  text \"x\"\n  owner 確認\n  decided 2026-10-03 by 確認 \"y\"\n  satisfied by proto \"warehouse.proto\" method Release\n  not verified \"z\"\n".as_bytes());
    let c = common::check(&d.to_string_lossy());
    assert!(codes(&c).contains(&"E012"), "{:?}", codes(&c));
}

// ── C.7 borrowed sources and E107 ─────────────────────────────────────────────

/// A source borrowed from a calendar is the calendar's pins and copies (koyomi's `Sources`): the
/// requirements read from it have the ends they have read from copies of their own. An article
/// the calendar does not pin is E106, with the ones it does.
#[test]
fn a_source_borrowed_from_a_calendar() {
    let c = common::check("tests/fixtures/koyomi");
    let p = c.project.as_ref().unwrap();
    let m = c.model.as_ref().unwrap();
    let end = |name: &str| m.req_ends[p.find_req(name).unwrap()[0]].as_ref().unwrap().hash.clone();
    assert_eq!(end("起算日"), "a9ebc73907faddc8");
    assert_eq!(end("満了日"), "465b83ed8c251406");
    assert_eq!(end("満了日_142条"), "d4f2d2a67322df17");

    let t = common::fixture("koyomi");
    let k = t.path().join("koyomi");
    let req = std::fs::read_to_string(k.join("民法の期間.req")).unwrap();
    std::fs::write(k.join("民法の期間.req"), req.replacen("from @民法 第140条", "from @民法 第144条", 1)).unwrap();
    let c = common::check(&k.to_string_lossy());
    let d = c.diags.iter().find(|d| d.code == "E106").unwrap_or_else(|| panic!("{:?}", codes(&c)));
    let text = d.render(ritsu_base::text::Lang::En);
    assert!(text.contains("koyomi \"民法の期間.cal\" source 民法 does not pin 第144条"), "{text}");
    assert!(text.contains("The articles it pins: 第140条, 第141条, 第142条, 第143条."), "{text}");
}

/// A requirement that copies an article itself, and a rule that meets it pinning the same one:
/// the same text is no E107, whatever the bytes; another text is, with the diff.
#[test]
fn a_requirement_and_a_rule_that_read_one_article() {
    let c = common::check("tests/fixtures/rulec");
    assert!(!codes(&c).contains(&"E107"), "{:?}", codes(&c));
    let t = common::fixture("rulec");
    let d = t.path().join("rulec");
    let copy = d.join("sources/law/332AC0000000026@2026-04-01/MainProvision-Article_91.xml");
    let text = std::fs::read_to_string(&copy).unwrap().replacen("二百円", "三百円", 1);
    std::fs::write(&copy, &text).unwrap();
    let pin = ritsu_base::sha256::short(text.as_bytes());
    let req = std::fs::read_to_string(d.join("印紙税.req")).unwrap().replace("第91条 sha256:85faf53f6f6e8196", &format!("第91条 sha256:{pin}"));
    std::fs::write(d.join("印紙税.req"), req).unwrap();
    let c = common::check(&d.to_string_lossy());
    let e = c.diags.iter().find(|x| x.code == "E107").unwrap_or_else(|| panic!("{:?}", codes(&c)));
    let shown = e.render(ritsu_base::text::Lang::En);
    assert!(shown.contains("軽減税率 reads 措置法 第91条, and rulec \"rules/印紙税の本則と軽減.rule\" pins 第91条 with another text"), "{shown}");
    assert!(shown.contains("三百円") && shown.contains("二百円"), "{shown}");
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
fn renamed_things_are_found() {
    let (_t, c) = renamed("chobo", "refunds.ja.book", "transfer 返金(", "transfer 返金の申請(");
    let d = e202(&c);
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(d[0].contains("It looks renamed") && d[0].contains("Did you mean: chobo \"refunds.ja.book\" transfer 返金の申請"), "{}", d[0]);

    let (_t, c) = renamed("chobo", "refunds.ja.book", "返金済み", "返した額");
    let d = e202(&c);
    assert!(d[0].contains("Did you mean: chobo \"refunds.ja.book\" account 売上, chobo \"refunds.ja.book\" account 返した額"), "{}", d[0]);

    let (_t, c) = renamed("koyomi", "民法の期間.cal", "起算日", "初日");
    let d = e202(&c);
    assert!(d[0].contains("Did you mean: koyomi \"民法の期間.cal\" date 初日"), "{}", d[0]);
    assert!(d[0].contains("- date 起算日(first_day)") && d[0].contains("+ date 初日(first_day)"), "{}", d[0]);
}

// ── C.9 affected ──────────────────────────────────────────────────────────────

fn affected(args: &[&str], golden: &str, code: i32, failures: &mut Vec<String>) -> String {
    let r = common::run(args);
    assert_eq!(r.code, code, "{args:?}: {}{}", r.stdout, r.stderr);
    common::golden(&format!("tests/golden/affected/{golden}"), &r.stdout, failures);
    r.stdout
}

const G: &str = "tests/fixtures/geas";

/// The change of stage A on geas's greeter: with the records before and after it, the claim it
/// touches, the requirement that claim checks, and its owner (exit 0); in English, in Japanese,
/// and as JSON.
#[test]
fn affected_follows_a_change_through_the_claims() {
    let mut failures = Vec::new();
    let both = ["--map", "tests/fixtures/geas/greeter/greeter.geas=tests/fixtures/geas/greeter/.geas/greeter.map.jsonl", "--map", "tests/fixtures/geas/greeter/greeter.geas=tests/fixtures/geas/changes/after.map.jsonl"];
    let mut args = vec!["affected", G, "--root", G, "--diff", "tests/fixtures/geas/changes/change.diff"];
    args.extend(both);
    let en = affected(&args, "greeter.en.txt", 0, &mut failures);
    assert!(en.contains("claim \"rejects an empty name\": tests/fixtures/geas/greeter/server.py 26 (after), 26 (before)\n    checked by rejects_an_empty_name\n"), "{en}");
    let mut ja = args.clone();
    ja.extend(["--lang", "ja"]);
    affected(&ja, "greeter.ja.txt", 0, &mut failures);
    let mut json = args.clone();
    json.extend(["--format", "json"]);
    let j = affected(&json, "greeter.json", 0, &mut failures);
    let v: serde_json::Value = serde_json::from_str(&j).unwrap();
    assert_eq!(v["owners"], serde_json::json!(["api"]));
    assert_eq!(v["specs"][0]["claims"][0]["requirements"][0]["name"], "rejects_an_empty_name");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// With the record before the change only, geas cannot answer for the lines the change adds: it
/// says so (E063), the rest is answered, and the exit is 2.
#[test]
fn affected_says_what_geas_cannot_answer() {
    let r = common::run(&["affected", G, "--root", G, "--diff", "tests/fixtures/geas/changes/change.diff", "--map", "tests/fixtures/geas/greeter/greeter.geas=tests/fixtures/geas/greeter/.geas/greeter.map.jsonl"]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stdout.contains("  geas cannot answer: [E063] tests/fixtures/geas/changes/change.diff:10:"), "{}", r.stdout);
    assert!(r.stdout.contains("files that requirements name, that the diff touches:\n  file \"greeter/server.py\": met by"), "{}", r.stdout);
}

/// A change to a requirement's text, to the copy of an article a calendar pins and a requirement
/// borrows, to a rule, and to a requirement's own copy of an article a rule pins too.
#[test]
fn affected_follows_requirements_sources_and_rules() {
    let mut failures = Vec::new();
    let k = "tests/fixtures/koyomi";
    let out = affected(&["affected", k, "--root", k, "--diff", "tests/fixtures/koyomi/changes/text.diff"], "koyomi-text.txt", 0, &mut failures);
    assert!(out.contains("  満了日_142条: its text or period changes, so its links are to be looked at again\n"), "{out}");
    let out = affected(&["affected", k, "--root", k, "--diff", "tests/fixtures/koyomi/changes/copy.diff", "--lang", "ja"], "koyomi-copy.ja.txt", 0, &mut failures);
    assert!(out.contains("（民法 第142条）: 引く要件 満了日_142条。固定している成果物 koyomi \"民法の期間.cal\""), "{out}");
    let r = "tests/fixtures/rulec";
    let out = affected(&["affected", r, "--root", r, "--diff", "tests/fixtures/rulec/changes/rule.diff"], "rulec-rule.txt", 0, &mut failures);
    assert!(out.contains("  rulec \"rules/印紙税の本則と軽減.rule\": met by 本則の税額, 軽減税率\n"), "{out}");
    let out = affected(&["affected", r, "--root", r, "--diff", "tests/fixtures/rulec/changes/copy.diff"], "rulec-copy.txt", 0, &mut failures);
    assert!(out.contains("(措置法 第91条): cited by 軽減税率; pinned by rulec \"rules/印紙税の本則と軽減.rule\""), "{out}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A file of a scope no requirement leads to, changed, is out of every requirement's reach:
/// exit 1. A diff that does not read, and a `--map` of a spec no link names, are exit 2.
#[test]
fn affected_says_what_no_requirement_reaches() {
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
    let spec = g.join("greeter/greeter.ja.geas").to_string_lossy().to_string();
    let r = common::run(&["affected", &dir, "--root", &dir, "--diff", &diff, "--map", &format!("{spec}=x.jsonl")]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.contains("No link of the project names the spec"), "{}", r.stderr);
}

// ── dandori and sakai ─────────────────────────────────────────────────────────

/// The things of a workflow (dandori's `Items`, from its syntax alone) and the terms of a context
/// (sakai's): each its own end.
#[test]
fn the_tasks_of_a_workflow_and_the_terms_of_a_context() {
    let e = ends("tests/fixtures/dandori");
    let flow = "dandori \"arrange_delivery.ja.flow\"";
    assert_eq!(hash_of(&e, &format!("{flow} task 翌日便を頼む")), "3c19313a3f633021");
    assert!(e[&format!("{flow} task 翌日便を頼む")].1.starts_with("task 翌日便を頼む(注文ID: string, 宛名: string?) -> 集荷\n  http POST"));
    assert_eq!(e[&format!("{flow} record 集荷 field 追跡番号")].1, "追跡番号 : string");
    let e = ends("tests/fixtures/sakai");
    assert_eq!(hash_of(&e, "sakai \"contexts/受注.ctx\" term キャンセル"), "dbfd211b7e4cef4b");
    assert_eq!(e["sakai \"contexts/受注.ctx\" term キャンセル"].1, "キャンセル \"出荷の前に、客の申し出で注文を取り消すこと\"\nmeans enum OrderStatus value ORDER_STATUS_CANCELLED");
}
