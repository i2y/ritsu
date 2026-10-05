//! `yuen source fetch | pin | outdated` (DESIGN 14), without the network, in English: the twin of
//! fetch.rs, on the fixtures that cite the eCFR (`period_of_months`, `payment_policy`,
//! `calendar_sources`) with the eCFR as ritsu-testkit's small HTTP server inside the test
//! (`YUEN_ECFR`). What e-Gov's markup and its revisions show (an article rewritten by an amendment
//! of its own, or only in its markup) has no twin here; the eCFR is asked by section and says which
//! amendment changed the text.

mod common;

use common::{Ran, TempDir};
use ritsu_testkit::http::HttpServer;
use std::path::Path;

const LAW: &str = "sources/law/37-CFR-1@2026-01-01";
const SECTIONS: [&str; 4] = ["1.6", "1.7", "1.8", "1.10"];

fn no_curl() -> bool {
    if common::on_path("curl") {
        return false;
    }
    ritsu_testkit::skip("curl is not on the PATH; source fetch and outdated are not run");
    true
}

/// ritsu-testkit's small HTTP server (a body for each request target it knows, 404 for the rest,
/// which it keeps), at the place yuen is told the eCFR is.
struct Server(HttpServer);

impl Server {
    fn start() -> Server {
        Server(HttpServer::start())
    }

    fn set(&self, target: &str, body: impl Into<Vec<u8>>) {
        self.0.set(target, body);
    }

    fn env(&self) -> Vec<(&'static str, String)> {
        vec![("YUEN_EGOV", format!("{}/egov", self.0.addr)), ("YUEN_ECFR", format!("{}/ecfr", self.0.addr))]
    }

    fn missed(&self) -> Vec<String> {
        self.0.missed()
    }
}

fn yuen(dir: &Path, s: &Server, args: &[&str]) -> Ran {
    let env = s.env();
    let envs: Vec<(&str, &str)> = env.iter().map(|(k, v)| (*k, v.as_str())).collect();
    common::yuen_env(dir, args, &envs)
}

/// How a later amendment differs from the copy.
#[derive(Clone, Copy)]
enum Change {
    None,
    /// A word of the text of §1.7.
    Text,
    /// Only an attribute of §1.7's markup.
    Markup,
}

/// The eCFR's answers for the four sections of 37 CFR 1 as of the copy's date and one later
/// amendment, and the list of the versions of each.
fn serve_ecfr(s: &Server, fixture: &Path, change: Change) {
    let v = |date: &str, id: &str, substantive: bool| {
        serde_json::json!({"date": date, "amendment_date": date, "issue_date": date, "identifier": id, "name": format!("§ {id} A section."), "part": "1", "substantive": substantive, "removed": false, "subpart": "A", "title": "37", "type": "section"})
    };
    for id in SECTIONS {
        let xml = std::fs::read(fixture.join(LAW).join(format!("{id}.xml"))).unwrap();
        s.set(&format!("/ecfr/full/2026-01-01/title-37.xml?part=1&section={id}"), xml.clone());
        let later = match change {
            Change::Text if id == "1.7" => String::from_utf8(xml).unwrap().replacen("next succeeding business day which is not", "second succeeding business day which is not", 1).into_bytes(),
            Change::Markup if id == "1.7" => String::from_utf8(xml).unwrap().replacen("TYPE=\"SECTION\"", "TYPE=\"SECTION\" VOLUME=\"1\"", 1).into_bytes(),
            _ => xml,
        };
        s.set(&format!("/ecfr/full/2026-03-01/title-37.xml?part=1&section={id}"), later);
        s.set(&format!("/ecfr/versions/title-37.json?part=1&section={id}"), serde_json::json!({"content_versions": [v("2019-05-14", id, true), v("2026-03-01", id, true), v("2026-05-01", id, false)], "meta": {"title": "37"}}).to_string());
    }
}

#[test]
fn fetch_writes_what_the_ecfr_serves_and_keeps_a_copy_whose_text_did_not_change() {
    if no_curl() {
        return;
    }
    let t = common::fixture("period_of_months");
    let d = t.path().join("period_of_months");
    let s = Server::start();
    serve_ecfr(&s, &d, Change::None);
    let want: Vec<Vec<u8>> = SECTIONS.iter().map(|id| std::fs::read(d.join(LAW).join(format!("{id}.xml"))).unwrap()).collect();
    std::fs::remove_dir_all(d.join("sources")).unwrap();
    let r = yuen(t.path(), &s, &["source", "fetch", "period_of_months", "--root", "period_of_months"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains("cfr: fetched §1.7 (sha256:01de176ebe4740d7)"), "{}", r.stdout);
    for (id, w) in SECTIONS.iter().zip(&want) {
        assert_eq!(&std::fs::read(d.join(LAW).join(format!("{id}.xml"))).unwrap(), w, "§{id} is the bytes the eCFR served");
    }
    assert!(!d.join(LAW).join("revision.txt").exists(), "the eCFR has no revision id: the date asked for is the version");
    assert_eq!(common::yuen(t.path(), &["check", "period_of_months", "--root", "period_of_months"]).code, 0, "the copies fetched pass the check");
    // the eCFR rewrites the markup of a section no amendment touched: the copy keeps its bytes.
    let xml = String::from_utf8(want[1].clone()).unwrap().replacen("TYPE=\"SECTION\"", "TYPE=\"SECTION\" VOLUME=\"1\"", 1);
    s.set("/ecfr/full/2026-01-01/title-37.xml?part=1&section=1.7", xml.into_bytes());
    let r = yuen(t.path(), &s, &["source", "fetch", "period_of_months", "--root", "period_of_months", "--lang", "ja"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("cfr: §1.7 の本文は変わっていません（sha256:01de176ebe4740d7）"), "{}", r.stdout);
    assert_eq!(std::fs::read(d.join(LAW).join("1.7.xml")).unwrap(), want[1]);
    assert!(s.missed().is_empty(), "asked for what is not served: {:?}", s.missed());
}

#[test]
fn pin_changes_the_digits_and_nothing_else_in_english() {
    let t = common::fixture("period_of_months");
    let d = t.path().join("period_of_months");
    let req = d.join("period_of_months.req");
    let original = std::fs::read_to_string(&req).unwrap();
    for crlf in [false, true] {
        let want = if crlf { original.replace('\n', "\r\n") } else { original.clone() };
        // A pin made wrong, and the last pin line gone (the section is still cited).
        let broken = want.replace("\"§1.7\" sha256:01de176ebe4740d7", "\"§1.7\" sha256:0000000000000000").replace(&format!("  \"§1.10\" sha256:5adcbc193371fd26{}", if crlf { "\r\n" } else { "\n" }), "");
        assert_ne!(broken, want);
        std::fs::write(&req, &broken).unwrap();
        let r = common::yuen(t.path(), &["source", "pin", "period_of_months", "--root", "period_of_months"]);
        assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
        assert!(r.stdout.contains("cfr: pinned §1.7 at sha256:01de176ebe4740d7"), "{}", r.stdout);
        assert!(r.stdout.contains("cfr: added a pin line for the cited §1.10 (sha256:5adcbc193371fd26)"), "{}", r.stdout);
        assert_eq!(std::fs::read_to_string(&req).unwrap(), want, "pin gives the file back, byte for byte (CR LF: {crlf})");
        let r = common::yuen(t.path(), &["source", "pin", "period_of_months", "--root", "period_of_months"]);
        assert!(r.stdout.contains("cfr: all 4 articles already pinned"), "{}", r.stdout);
    }
    // A copy that is not there cannot be pinned, and pin says to fetch it.
    std::fs::remove_file(d.join(LAW).join("1.6.xml")).unwrap();
    std::fs::write(&req, original.replace("\"§1.6\" sha256:6bcdc27c3428886c", "\"§1.6\"")).unwrap();
    let r = common::yuen(t.path(), &["source", "pin", "period_of_months", "--root", "period_of_months", "--lang", "ja"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("cfr: §1.6 のコピーが無いので固定できません。先に yuen source fetch を走らせてください"), "{}", r.stdout);
}

#[test]
fn outdated_says_what_changed_and_whom_to_ask_in_english() {
    if no_curl() {
        return;
    }
    let t = common::fixture("period_of_months");
    let d = t.path().join("period_of_months");
    let s = Server::start();
    serve_ecfr(&s, &d, Change::None);
    let r = yuen(t.path(), &s, &["source", "outdated", "period_of_months", "--root", "period_of_months"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains("cfr: the amendment of 2026-03-01 leaves the cited sections as they are"), "{}", r.stdout);
    assert!(!r.stdout.contains("2026-05-01"), "a version that is not substantive is no amendment: {}", r.stdout);
    // A word of §1.7 changes with the amendment of 2026-03-01.
    serve_ecfr(&s, &d, Change::Text);
    let r = yuen(t.path(), &s, &["source", "outdated", "period_of_months", "--root", "period_of_months"]);
    assert_eq!(r.code, 1, "{}{}", r.stdout, r.stderr);
    let out = &r.stdout;
    assert!(out.contains("cfr: the amendment of 2026-03-01 changes §1.7"), "{out}");
    assert!(out.contains("+ (a) Whenever periods of time are specified in this part in days, calendar days are intended."), "{out}");
    assert!(out.contains("second succeeding business day which is not a Saturday"), "{out}");
    assert!(out.contains("cited by: next_business_day (owned by legal, period_of_months/period_of_months.req:"), "{out}");
    assert!(out.contains("to look at again: 2 links and 1 waiver"), "{out}");
    assert_eq!(out.matches("changes §1.7").count(), 1, "the change is said on the day it comes into force, and not again: {out}");
    let ja = yuen(t.path(), &s, &["source", "outdated", "period_of_months", "--root", "period_of_months", "--lang", "ja"]);
    assert!(ja.stdout.contains("cfr: 2026-03-01 の改正で §1.7 が変わります"), "{}", ja.stdout);
    assert!(ja.stdout.contains("引いている要件: next_business_day（持ち主 legal、period_of_months/period_of_months.req:"), "{}", ja.stdout);
    assert!(ja.stdout.contains("確かめ直すもの: リンク 2 本と見送り 1 件"), "{}", ja.stdout);
    // An amendment that only rewrote the markup is not an amendment of the text.
    serve_ecfr(&s, &d, Change::Markup);
    let r = yuen(t.path(), &s, &["source", "outdated", "period_of_months", "--root", "period_of_months"]);
    assert_eq!(r.code, 0, "{}", r.stdout);
    assert!(s.missed().is_empty(), "asked for what is not served: {:?}", s.missed());
}

#[test]
fn a_file_source_is_held_to_its_url_in_english() {
    if no_curl() {
        return;
    }
    let t = common::fixture("payment_policy");
    let d = t.path().join("payment_policy");
    let terms = std::fs::read(d.join("docs/terms.md")).unwrap();
    let served = t.write("served/terms.md", &terms);
    let url = format!("file://{}", served.display());
    let req = d.join("policy.req");
    let src = std::fs::read_to_string(&req).unwrap();
    std::fs::write(&req, src.replace("https://example.org/terms.md", &url)).unwrap();
    let r = common::yuen(t.path(), &["source", "outdated", "payment_policy", "--root", "payment_policy"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains(&format!("terms: unchanged ({url} is sha256:")), "{}", r.stdout);
    // The original moves on.
    let moved = String::from_utf8(terms.clone()).unwrap().replace("within 60 days", "within 30 days");
    std::fs::write(&served, &moved).unwrap();
    let r = common::yuen(t.path(), &["source", "outdated", "payment_policy", "--root", "payment_policy"]);
    assert_eq!(r.code, 1, "{}", r.stdout);
    assert!(r.stdout.contains(&format!("terms: {url} moved on")), "{}", r.stdout);
    assert!(r.stdout.contains("+ Clause 4. The price of an invoice received is paid within 30 days of the closing day."), "{}", r.stdout);
    assert!(r.stdout.contains("cited by: payment_policy (owned by accounting, payment_policy/policy.req:"), "{}", r.stdout);
    assert!(r.stdout.contains("read from those: payment_day v1 (owned by accounting, payment_policy/payment.req:"), "{}", r.stdout);
    assert!(r.stdout.contains("to look at again: 9 links"), "{}", r.stdout);
    // fetch takes it, pin pins it, and check then marks what the change reaches.
    let r = common::yuen(t.path(), &["source", "fetch", "payment_policy", "--root", "payment_policy"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_eq!(std::fs::read_to_string(d.join("docs/terms.md")).unwrap(), moved);
    assert!(r.stdout.contains("the pin is sha256:"), "{}", r.stdout);
    assert_eq!(common::yuen(t.path(), &["source", "pin", "payment_policy", "--root", "payment_policy"]).code, 0);
    let r = common::yuen(t.path(), &["check", "payment_policy", "--root", "payment_policy"]);
    assert_eq!(r.code, 1);
    assert_eq!(common::codes(&r.stdout).len(), 9, "the nine links outdated counted are marked: {}", r.stdout);
}

/// A borrowed source is the rule's or the calendar's to fetch and to pin: `fetch` and `pin` say
/// which command does it. `outdated` asks about it from the calendar's pins and copies, which
/// only `ritsu yuen` can read (the binary of this crate says so, E206 and exit 2).
#[test]
fn a_borrowed_source_is_its_tools_to_fetch_in_english() {
    let t = common::fixture("calendar_sources");
    let r = common::yuen(t.path(), &["source", "fetch", "calendar_sources", "--root", "calendar_sources"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.stdout.contains("bank_holidays: borrowed from a koyomi file, so yuen does not fetch it; koyomi source fetch does"), "{}", r.stdout);
    let r = common::yuen(t.path(), &["source", "pin", "calendar_sources", "--root", "calendar_sources"]);
    assert!(r.stdout.contains("koyomi source pin does"), "{}", r.stdout);
    let r = common::yuen(t.path(), &["source", "outdated", "calendar_sources", "--root", "calendar_sources"]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("[E206]: ") && r.stderr.contains("this yuen cannot read koyomi artifacts: koyomi \"calendars/tokyo_business_days.cal\" source national_holidays"), "{}", r.stderr);
    assert!(r.stderr.contains("`ritsu yuen source outdated calendar_sources --root calendar_sources`"), "{}", r.stderr);
}

#[test]
fn what_the_source_commands_refuse_in_english() {
    let p = "tests/fixtures/period_of_months";
    let run = |args: &[&str]| common::yuen(Path::new("."), args);
    assert_eq!(run(&["source"]).code, 2);
    assert_eq!(run(&["source", "frobnicate", p]).code, 2);
    assert_eq!(run(&["source", "fetch"]).code, 2);
    let r = run(&["source", "pin", "tests/mutants/E002_unknown_line"]);
    assert_eq!(r.code, 1, "an error of the words stops it");
    assert!(r.stderr.contains("error[E002]"), "{}", r.stderr);
    // Without curl, fetch says so and stops.
    let t = TempDir::new("nocurl");
    let r = common::yuen_env(Path::new("."), &["source", "outdated", p, "--root", p], &[("PATH", t.path().to_str().unwrap())]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("curl"), "{}", r.stderr);
}

#[test]
fn the_real_ecfr_when_asked_for_the_english_fixtures() {
    let platforms = ritsu_testkit::level::level() == Some(ritsu_testkit::Level::Platforms);
    if std::env::var("YUEN_NET").as_deref() != Ok("1") && !platforms {
        println!("not asked: YUEN_NET is not 1 and RITSU_TEST_LEVEL is not platforms, so the real eCFR was not asked (set YUEN_NET=1 to ask it)");
        return;
    }
    if no_curl() {
        return;
    }
    for p in ["tests/fixtures/period_of_months"] {
        let r = common::yuen(Path::new("."), &["source", "outdated", p, "--root", p]);
        println!("$ yuen source outdated {p} --root {p}\n{}{}", r.stdout, r.stderr);
        assert!(matches!(r.code, 0 | 1), "{p}: {}{}", r.stdout, r.stderr);
    }
}
