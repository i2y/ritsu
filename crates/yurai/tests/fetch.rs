//! `yurai source fetch | pin | outdated` (PLAN C.12, DESIGN 14), without the network: e-Gov
//! law API v2 and the eCFR are a small HTTP server inside the test (`YURAI_EGOV`,
//! `YURAI_ECFR`), serving the copies of the test material; a `file` source's `url` is a
//! `file://` URL. The real e-Gov and eCFR are asked only when `YURAI_NET=1`.

mod common;

use common::{Ran, TempDir};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const LAW: &str = "sources/law/129AC0000000089@2026-10-01";
const REVISION: &str = "129AC0000000089_20260624_508AC0000000045";
/// The revisions of the Civil Code that come into force after 2026-10-01 (koyomi's DESIGN 1.5).
const LATER: [&str; 5] = ["2027-06-23", "2027-12-05", "2028-06-13", "2028-12-23", "2029-06-23"];
const ARTICLES: [&str; 4] = ["140", "141", "142", "143"];
const CFR: &str = "sources/law/29-CFR-1910@2026-01-01/1910.157.xml";

fn no_curl() -> bool {
    if common::on_path("curl") {
        return false;
    }
    println!("SKIP: curl is not on the PATH; source fetch and outdated are not run");
    true
}

/// A small HTTP server: a body for each request target it knows, 404 for the rest (and
/// those are kept, so a test can say what it asked for that was not there).
struct Server {
    addr: String,
    stop: Arc<AtomicBool>,
    routes: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    misses: Arc<Mutex<Vec<String>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Server {
    fn start() -> Server {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("http://{}", l.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let routes: Arc<Mutex<BTreeMap<String, Vec<u8>>>> = Arc::new(Mutex::new(BTreeMap::new()));
        let misses = Arc::new(Mutex::new(Vec::new()));
        let (s, r, m) = (Arc::clone(&stop), Arc::clone(&routes), Arc::clone(&misses));
        let thread = std::thread::spawn(move || {
            for conn in l.incoming() {
                if s.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(mut conn) = conn else { continue };
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    match conn.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    }
                }
                let head = String::from_utf8_lossy(&buf).to_string();
                let target = head.split_whitespace().nth(1).unwrap_or("/").to_string();
                let body = r.lock().unwrap().get(&target).cloned();
                let _ = match body {
                    Some(b) => {
                        let _ = write!(conn, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", b.len());
                        conn.write_all(&b)
                    }
                    None => {
                        m.lock().unwrap().push(target);
                        write!(conn, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    }
                };
            }
        });
        Server { addr, stop, routes, misses, thread: Some(thread) }
    }

    fn set(&self, target: &str, body: impl Into<Vec<u8>>) {
        self.routes.lock().unwrap().insert(target.to_string(), body.into());
    }

    fn env(&self) -> Vec<(&'static str, String)> {
        vec![("YURAI_EGOV", format!("{}/egov", self.addr)), ("YURAI_ECFR", format!("{}/ecfr", self.addr))]
    }

    fn missed(&self) -> Vec<String> {
        self.misses.lock().unwrap().clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr.trim_start_matches("http://"));
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn yurai(dir: &Path, s: &Server, args: &[&str]) -> Ran {
    let env = s.env();
    let envs: Vec<(&str, &str)> = env.iter().map(|(k, v)| (*k, v.as_str())).collect();
    common::yurai_env(dir, args, &envs)
}

fn law_data(xml: &[u8], rev: &str) -> Vec<u8> {
    serde_json::json!({"law_info": {"law_id": "129AC0000000089"}, "revision_info": {"law_revision_id": rev}, "law_full_text": yurai::base64::encode(xml)}).to_string().into_bytes()
}

/// How one later revision differs from the copy.
#[derive(Clone, Copy)]
enum Change {
    None,
    /// One character of the text of 第142条, from that date on.
    Text(&'static str),
    /// Only an attribute of 第142条's markup, from that date on.
    Markup(&'static str),
}

/// e-Gov's answers for the Civil Code as of the copy's date and the five later revisions.
fn serve_egov(s: &Server, fixture: &Path, change: Change) {
    let mut revs = vec![serde_json::json!({"law_revision_id": REVISION, "amendment_enforcement_date": "2026-06-24"})];
    for d in LATER {
        revs.push(serde_json::json!({"law_revision_id": format!("129AC0000000089_{}_TEST", d.replace('-', "")), "amendment_enforcement_date": d}));
    }
    revs.push(serde_json::json!({"law_revision_id": "129AC0000000089_UNKNOWN", "amendment_enforcement_date": null}));
    s.set("/egov/law_revisions/129AC0000000089", serde_json::json!({"law_info": {"law_id": "129AC0000000089"}, "revisions": revs}).to_string());
    for a in ARTICLES {
        let xml = std::fs::read(fixture.join(LAW).join(format!("MainProvision-Article_{a}.xml"))).unwrap();
        s.set(&format!("/egov/law_data/129AC0000000089?asof=2026-10-01&elm=MainProvision-Article_{a}&law_full_text_format=xml"), law_data(&xml, REVISION));
        for d in LATER {
            let text = String::from_utf8(xml.clone()).unwrap();
            let text = match change {
                Change::Text(from) if a == "142" && d >= from => text.replace("その翌日に満了する", "その翌々日に満了する"),
                Change::Markup(from) if a == "142" && d >= from => text.replace("Num=\"142\"", "Num=\"142\" Delete=\"false\""),
                _ => text,
            };
            let rev = format!("129AC0000000089_{}_TEST", d.replace('-', ""));
            s.set(&format!("/egov/law_data/129AC0000000089?asof={d}&elm=MainProvision-Article_{a}&law_full_text_format=xml"), law_data(text.as_bytes(), &rev));
        }
    }
}

/// The eCFR's answers for 29 CFR 1910.157 as of the copy's date and one later amendment.
fn serve_ecfr(s: &Server, fixture: &Path, changed: bool) {
    let xml = std::fs::read(fixture.join(CFR)).unwrap();
    s.set("/ecfr/full/2026-01-01/title-29.xml?part=1910&section=1910.157", xml.clone());
    let later = if changed { String::from_utf8(xml.clone()).unwrap().replacen("75 feet", "50 feet", 1).into_bytes() } else { xml };
    s.set("/ecfr/full/2026-03-01/title-29.xml?part=1910&section=1910.157", later);
    let v = |date: &str, substantive: bool| {
        serde_json::json!({"date": date, "amendment_date": date, "issue_date": date, "identifier": "1910.157", "name": "§ 1910.157 Portable fire extinguishers.", "part": "1910", "substantive": substantive, "removed": false, "subpart": "L", "title": "29", "type": "section"})
    };
    s.set("/ecfr/versions/title-29.json?part=1910&section=1910.157", serde_json::json!({"content_versions": [v("2019-05-14", true), v("2026-03-01", true), v("2026-05-01", false)], "meta": {"title": "29"}}).to_string());
}

#[test]
fn fetch_writes_what_e_gov_serves_and_keeps_a_copy_whose_text_did_not_change() {
    if no_curl() {
        return;
    }
    let t = common::fixture("period");
    let d = t.path().join("period");
    let s = Server::start();
    serve_egov(&s, &d, Change::None);
    let want: Vec<Vec<u8>> = ARTICLES.iter().map(|a| std::fs::read(d.join(LAW).join(format!("MainProvision-Article_{a}.xml"))).unwrap()).collect();
    std::fs::remove_dir_all(d.join("sources")).unwrap();
    let r = yurai(t.path(), &s, &["source", "fetch", "period", "--root", "period"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains("民法: fetched 第142条 (sha256:fc8c35a0769d3b35)"), "{}", r.stdout);
    for (a, w) in ARTICLES.iter().zip(&want) {
        assert_eq!(&std::fs::read(d.join(LAW).join(format!("MainProvision-Article_{a}.xml"))).unwrap(), w, "第{a}条 is the bytes e-Gov served");
    }
    assert_eq!(std::fs::read_to_string(d.join(LAW).join("revision.txt")).unwrap(), format!("{REVISION}\n"));
    assert_eq!(common::yurai(t.path(), &["check", "period", "--root", "period"]).code, 0, "the copies fetched pass the check");
    // e-Gov rewrites the markup of an article no amendment touched: the copy keeps its bytes.
    let xml = String::from_utf8(want[2].clone()).unwrap().replace("Num=\"142\"", "Num=\"142\" Delete=\"false\"");
    s.set("/egov/law_data/129AC0000000089?asof=2026-10-01&elm=MainProvision-Article_142&law_full_text_format=xml", law_data(xml.as_bytes(), REVISION));
    let r = yurai(t.path(), &s, &["source", "fetch", "period", "--root", "period", "--lang", "ja"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("民法: 第142条 の本文は変わっていません（sha256:fc8c35a0769d3b35）"), "{}", r.stdout);
    assert_eq!(std::fs::read(d.join(LAW).join("MainProvision-Article_142.xml")).unwrap(), want[2]);
    assert!(s.missed().is_empty(), "asked for what is not served: {:?}", s.missed());
}

#[test]
fn pin_changes_the_digits_and_nothing_else() {
    let t = common::fixture("period");
    let d = t.path().join("period");
    let req = d.join("民法の期間.req");
    let original = std::fs::read_to_string(&req).unwrap();
    for crlf in [false, true] {
        let want = if crlf { original.replace('\n', "\r\n") } else { original.clone() };
        // A pin made wrong, and the last pin line gone (the article is still cited).
        let broken = want.replace("第142条 sha256:fc8c35a0769d3b35", "第142条 sha256:0000000000000000").replace(&format!("  第143条 sha256:6950bdfb988439b6{}", if crlf { "\r\n" } else { "\n" }), "");
        assert_ne!(broken, want);
        std::fs::write(&req, &broken).unwrap();
        let r = common::yurai(t.path(), &["source", "pin", "period", "--root", "period"]);
        assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
        assert!(r.stdout.contains("民法: pinned 第142条 at sha256:fc8c35a0769d3b35"), "{}", r.stdout);
        assert!(r.stdout.contains("民法: added a pin line for the cited 第143条 (sha256:6950bdfb988439b6)"), "{}", r.stdout);
        assert_eq!(std::fs::read_to_string(&req).unwrap(), want, "pin gives the file back, byte for byte (CR LF: {crlf})");
        let r = common::yurai(t.path(), &["source", "pin", "period", "--root", "period"]);
        assert!(r.stdout.contains("民法: all 4 articles already pinned"), "{}", r.stdout);
    }
    // A copy that is not there cannot be pinned, and pin says to fetch it.
    std::fs::remove_file(d.join(LAW).join("MainProvision-Article_140.xml")).unwrap();
    std::fs::write(&req, original.replace("第140条 sha256:e880059021fbb67d", "第140条")).unwrap();
    let r = common::yurai(t.path(), &["source", "pin", "period", "--root", "period", "--lang", "ja"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("民法: 第140条 の写しが無いので固定できません。先に yurai source fetch を走らせます"), "{}", r.stdout);
}

#[test]
fn outdated_says_what_changed_and_whom_to_ask() {
    if no_curl() {
        return;
    }
    let t = common::fixture("period");
    let d = t.path().join("period");
    let s = Server::start();
    serve_egov(&s, &d, Change::None);
    let r = yurai(t.path(), &s, &["source", "outdated", "period", "--root", "period"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert_eq!(r.stdout.matches("leaves the cited articles as they are").count(), 5, "{}", r.stdout);
    // One character of 第142条 changes from the revision of 2028-06-13 on: said once, that day.
    serve_egov(&s, &d, Change::Text("2028-06-13"));
    let r = yurai(t.path(), &s, &["source", "outdated", "period", "--root", "period"]);
    assert_eq!(r.code, 1, "{}{}", r.stdout, r.stderr);
    let out = &r.stdout;
    assert!(out.contains("民法: the revision in force from 2028-06-13 (129AC0000000089_20280613_TEST) changes 第142条"), "{out}");
    assert!(out.contains("+ 期間の末日が日曜日、国民の祝日に関する法律（昭和二十三年法律第百七十八号）に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌々日に満了する。"), "{out}");
    assert!(out.contains("cited by: 満了日_142条 (owned by 法務, period/民法の期間.req:37)"), "{out}");
    assert!(out.contains("to look at again: 2 links and 1 waiver"), "{out}");
    assert_eq!(out.matches("changes 第142条").count(), 1, "the change is said on the day it comes into force, and not again: {out}");
    assert_eq!(out.matches("leaves the cited articles as they are").count(), 4, "{out}");
    let ja = yurai(t.path(), &s, &["source", "outdated", "period", "--root", "period", "--lang", "ja"]);
    assert!(ja.stdout.contains("民法: 2028-06-13 施行の版（129AC0000000089_20280613_TEST）で 第142条 が変わります"), "{}", ja.stdout);
    assert!(ja.stdout.contains("引いている要件: 満了日_142条（持ち主 法務、period/民法の期間.req:37）"), "{}", ja.stdout);
    assert!(ja.stdout.contains("確かめ直すもの: リンク 2 本と見送り 1 件"), "{}", ja.stdout);
    // A revision that only rewrote the markup is not an amendment.
    serve_egov(&s, &d, Change::Markup("2027-12-05"));
    let r = yurai(t.path(), &s, &["source", "outdated", "period", "--root", "period"]);
    assert_eq!(r.code, 0, "{}", r.stdout);
    assert!(s.missed().is_empty(), "asked for what is not served: {:?}", s.missed());
}

#[test]
fn the_ecfr_is_asked_by_section() {
    if no_curl() {
        return;
    }
    let t = common::fixture("ecfr");
    let d = t.path().join("ecfr");
    let s = Server::start();
    serve_ecfr(&s, &d, false);
    let want = std::fs::read(d.join(CFR)).unwrap();
    std::fs::remove_dir_all(d.join("sources")).unwrap();
    let r = yurai(t.path(), &s, &["source", "fetch", "ecfr", "--root", "ecfr"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert_eq!(std::fs::read(d.join(CFR)).unwrap(), want, "the section is the bytes the eCFR served");
    assert!(!d.join("sources/law/29-CFR-1910@2026-01-01/revision.txt").exists(), "the eCFR has no revision id: the date asked for is the version");
    let r = yurai(t.path(), &s, &["source", "outdated", "ecfr", "--root", "ecfr"]);
    assert_eq!(r.code, 0, "{}", r.stdout);
    assert!(r.stdout.contains("osha: the amendment of 2026-03-01 leaves the cited sections as they are"), "{}", r.stdout);
    assert!(!r.stdout.contains("2026-05-01"), "a version that is not substantive is no amendment: {}", r.stdout);
    serve_ecfr(&s, &d, true);
    let r = yurai(t.path(), &s, &["source", "outdated", "ecfr", "--root", "ecfr"]);
    assert_eq!(r.code, 1, "{}", r.stdout);
    assert!(r.stdout.contains("osha: the amendment of 2026-03-01 changes §1910.157"), "{}", r.stdout);
    assert!(r.stdout.contains("cited by: extinguisher_distance (owned by safety, ecfr/osha.req:9)"), "{}", r.stdout);
    assert!(r.stdout.contains("to look at again: 1 link and 2 waivers"), "{}", r.stdout);
    assert!(s.missed().is_empty(), "asked for what is not served: {:?}", s.missed());
}

#[test]
fn a_file_source_is_held_to_its_url() {
    if no_curl() {
        return;
    }
    let t = common::fixture("payment");
    let d = t.path().join("payment");
    let terms = std::fs::read(d.join("docs/約款.md")).unwrap();
    let served = t.write("served/terms.md", &terms);
    let url = format!("file://{}", served.display());
    let req = d.join("方針.req");
    let src = std::fs::read_to_string(&req).unwrap();
    std::fs::write(&req, src.replace("https://example.org/terms.md", &url)).unwrap();
    let r = common::yurai(t.path(), &["source", "outdated", "payment", "--root", "payment"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains(&format!("約款: unchanged ({url} is sha256:")), "{}", r.stdout);
    // The original moves on.
    let moved = String::from_utf8(terms.clone()).unwrap().replace("60 日以内", "30 日以内");
    std::fs::write(&served, &moved).unwrap();
    let r = common::yurai(t.path(), &["source", "outdated", "payment", "--root", "payment"]);
    assert_eq!(r.code, 1, "{}", r.stdout);
    assert!(r.stdout.contains(&format!("約款: {url} moved on")), "{}", r.stdout);
    assert!(r.stdout.contains("+ 第 4 条　受け取った請求書の代金は、締め日から 30 日以内に支払う。"), "{}", r.stdout);
    assert!(r.stdout.contains("cited by: 支払の方針 (owned by 経理, payment/方針.req:9)"), "{}", r.stdout);
    assert!(r.stdout.contains("read from those: 支払日 v1 (owned by 経理, payment/支払.req:16), 支払日 v2 (owned by 経理, payment/支払.req:27)"), "{}", r.stdout);
    assert!(r.stdout.contains("to look at again: 9 links"), "{}", r.stdout);
    // fetch takes it, pin pins it, and check then marks what the change reaches.
    let r = common::yurai(t.path(), &["source", "fetch", "payment", "--root", "payment"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_eq!(std::fs::read_to_string(d.join("docs/約款.md")).unwrap(), moved);
    assert!(r.stdout.contains("the pin is sha256:"), "{}", r.stdout);
    assert_eq!(common::yurai(t.path(), &["source", "pin", "payment", "--root", "payment"]).code, 0);
    let r = common::yurai(t.path(), &["check", "payment", "--root", "payment"]);
    assert_eq!(r.code, 1);
    assert_eq!(common::codes(&r.stdout).len(), 9, "the nine links outdated counted are marked: {}", r.stdout);
}

#[test]
fn a_borrowed_source_is_its_tools_to_fetch() {
    let t = TempDir::new("borrowed");
    t.write(
        "t.req",
        "requirements t v1\nrole 法務\n\nsource 民法 = koyomi \"民法の期間.cal\" source 民法\n\nrequirement r1\n  text \"x\"\n  owner 法務\n  from @民法 第142条\n  not satisfied \"y\"\n  not verified \"z\"\n".as_bytes(),
    );
    let r = common::yurai(t.path(), &["source", "fetch", ".", "--root", "."]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.stdout.contains("民法: borrowed from a koyomi file, so yurai does not fetch it; koyomi source fetch does"), "{}", r.stdout);
    let r = common::yurai(t.path(), &["source", "pin", ".", "--root", "."]);
    assert!(r.stdout.contains("koyomi source pin does"), "{}", r.stdout);
    let r = common::yurai(t.path(), &["source", "outdated", ".", "--root", "."]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("yurai cannot read borrowed sources yet"), "{}", r.stderr);
}

#[test]
fn what_the_source_commands_refuse() {
    let p = "tests/fixtures/period";
    let run = |args: &[&str]| common::yurai(Path::new("."), args);
    assert_eq!(run(&["source"]).code, 2);
    assert_eq!(run(&["source", "frobnicate", p]).code, 2);
    assert_eq!(run(&["source", "fetch"]).code, 2);
    let r = run(&["source", "pin", "tests/mutants/E002_知らない行"]);
    assert_eq!(r.code, 1, "an error of the words stops it");
    assert!(r.stderr.contains("error[E002]"), "{}", r.stderr);
    // Without curl, fetch says so and stops.
    let t = TempDir::new("nocurl");
    let r = common::yurai_env(Path::new("."), &["source", "outdated", p, "--root", p], &[("PATH", t.path().to_str().unwrap())]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("curl"), "{}", r.stderr);
}

#[test]
fn the_real_e_gov_and_ecfr_when_asked() {
    if std::env::var("YURAI_NET").as_deref() != Ok("1") {
        println!("not asked: YURAI_NET is not 1, so the real e-Gov and eCFR were not asked (set YURAI_NET=1 to ask them)");
        return;
    }
    if no_curl() {
        return;
    }
    for p in ["tests/fixtures/period", "tests/fixtures/ecfr"] {
        let r = common::yurai(Path::new("."), &["source", "outdated", p, "--root", p]);
        println!("$ yurai source outdated {p} --root {p}\n{}{}", r.stdout, r.stderr);
        assert!(matches!(r.code, 0 | 1), "{p}: {}{}", r.stdout, r.stderr);
    }
}
