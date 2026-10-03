//! `koyomi source fetch | pin | outdated` (PLAN C.9, DESIGN 9), without the network: a
//! table's `url` is a `file://` URL to a newer copy, and e-Gov law API v2 is a small HTTP
//! server inside the test (`KOYOMI_EGOV`). The real Cabinet Office, GOV.UK and e-Gov are
//! asked only when `KOYOMI_NET=1`.

mod common;

use common::{TempDir, on_path};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const CSV: &str = "examples/calendars/data/syukujitsu.csv";
const JSON: &str = "examples/calendars/data/bank-holidays.json";
const LAW: &str = "examples/sources/law/129AC0000000089@2026-10-01";
const REVISION: &str = "129AC0000000089_20260624_508AC0000000045";
/// The revisions of the Civil Code that come into force after 2026-10-01 (DESIGN 1.5).
const LATER: [&str; 5] = ["2027-06-23", "2027-12-05", "2028-06-13", "2028-12-23", "2029-06-23"];

fn koyomi(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_koyomi")).args(args).current_dir(dir).env_remove("KOYOMI_LANG").output().unwrap()
}

fn text(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap()
}

fn short(b: &[u8]) -> String {
    koyomi::sha256::short(b)
}

fn no_curl() -> bool {
    if on_path("curl") {
        return false;
    }
    println!("SKIP: curl is not on the PATH; source fetch and outdated are not run");
    true
}

/// The bytes of the copy with every line of one year taken out.
fn csv_without(year: &str) -> Vec<u8> {
    let b = std::fs::read(CSV).unwrap();
    let lines: Vec<&[u8]> = b.split(|c| *c == b'\n').filter(|l| !l.starts_with(format!("{year}/").as_bytes())).collect();
    lines.join(&b'\n')
}

#[test]
fn a_table_from_the_cabinet_office_moves_on() {
    if no_curl() {
        return;
    }
    let t = TempDir::new("fetch-csv");
    let dir = t.path();
    std::fs::create_dir_all(dir.join("data")).unwrap();
    let old = csv_without("2027");
    let new = std::fs::read(CSV).unwrap();
    std::fs::write(dir.join("data/syukujitsu.csv"), &old).unwrap();
    std::fs::write(dir.join("data/new.csv"), &new).unwrap();
    let url = format!("file://{}", dir.join("data/new.csv").display());
    let cal = format!(
        "calendar 試験(test) v1\noffset +09:00\n\nsource 祝日 = file \"data/syukujitsu.csv\" url \"{url}\" sha256:{}\n  format csv shift_jis\n  covers 1955-01-01..2026-12-31\n\nclosed weekly sat, sun\nclosed 祝日\n",
        short(&old)
    );
    std::fs::write(dir.join("試験.cal"), &cal).unwrap();
    assert_eq!(code(&koyomi(dir, &["check", "試験.cal"])), 0, "the old copy passes");

    let o = koyomi(dir, &["source", "outdated", "試験.cal", "--lang", "ja"]);
    assert_eq!(code(&o), 1, "{}", text(&o));
    let out = text(&o);
    assert!(out.contains(&format!("祝日: {url} が変わりました（固定は sha256:{}、いまは sha256:{}）", short(&old), short(&new))), "{out}");
    assert!(out.contains("2027 年の 17 日が増えます: 2027-01-01 元日、2027-01-11 成人の日、"), "{out}");
    assert!(out.contains("ほか 11 日"), "{out}");
    assert!(out.contains("`covers 1955-01-01..2027-12-31`（いまは 1955-01-01..2026-12-31）"), "{out}");
    let en = text(&koyomi(dir, &["source", "outdated", "試験.cal"]));
    assert!(en.contains("17 days are added (17 in 2027): 2027-01-01 元日"), "{en}");

    let o = koyomi(dir, &["source", "fetch", "試験.cal"]);
    assert_eq!(code(&o), 0, "{}", text(&o));
    assert_eq!(std::fs::read(dir.join("data/syukujitsu.csv")).unwrap(), new, "fetch writes the bytes as they came");
    assert!(text(&o).contains(&format!("the pin is sha256:{}", short(&old))), "{}", text(&o));
    assert_eq!(code(&koyomi(dir, &["check", "試験.cal"])), 1, "the copy no longer matches its pin (E103)");

    let o = koyomi(dir, &["source", "pin", "試験.cal"]);
    assert_eq!(code(&o), 0, "{}", text(&o));
    assert!(text(&o).contains(&format!("祝日: pinned sha256:{}; the table has 1,067 rows, covers 1955-01-01..2026-12-31", short(&new))), "{}", text(&o));
    let pinned = std::fs::read_to_string(dir.join("試験.cal")).unwrap();
    assert_eq!(pinned, cal.replace(&short(&old), &short(&new)), "pin changes the 16 digits and nothing else");
    // The rows of 2027 are outside the covers written for the old table; the line outdated
    // gave fixes it.
    let o = koyomi(dir, &["check", "試験.cal"]);
    assert!(text(&o).contains("E105"), "{}", text(&o));
    std::fs::write(dir.join("試験.cal"), pinned.replace("covers 1955-01-01..2026-12-31", "covers 1955-01-01..2027-12-31")).unwrap();
    let o = koyomi(dir, &["check", "試験.cal"]);
    assert_eq!(code(&o), 0, "{}", text(&o));
    let o = koyomi(dir, &["source", "outdated", "試験.cal"]);
    assert_eq!(code(&o), 0, "{}", text(&o));
    assert!(text(&o).contains("祝日: unchanged"), "{}", text(&o));
}

#[test]
fn a_govuk_table_moves_on() {
    if no_curl() {
        return;
    }
    let t = TempDir::new("fetch-json");
    let dir = t.path();
    std::fs::create_dir_all(dir.join("data")).unwrap();
    let new = std::fs::read(JSON).unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&new).unwrap();
    for (_, div) in v.as_object_mut().unwrap() {
        div["events"].as_array_mut().unwrap().retain(|e| !e["date"].as_str().unwrap().starts_with("2028"));
    }
    let old = serde_json::to_vec(&v).unwrap();
    std::fs::write(dir.join("data/bank-holidays.json"), &old).unwrap();
    std::fs::write(dir.join("data/new.json"), &new).unwrap();
    let url = format!("file://{}", dir.join("data/new.json").display());
    let cal = format!(
        "calendar test v1\n\nsource bank_holidays = file \"data/bank-holidays.json\" url \"{url}\" sha256:{}\n  format govuk \"england-and-wales\"\n  covers listed years\n\nclosed weekly sat, sun\nclosed bank_holidays\n",
        short(&old)
    );
    std::fs::write(dir.join("test.cal"), &cal).unwrap();
    assert_eq!(code(&koyomi(dir, &["check", "test.cal"])), 0);

    let o = koyomi(dir, &["source", "outdated", "test.cal"]);
    assert_eq!(code(&o), 1, "{}", text(&o));
    let out = text(&o);
    assert!(out.contains("8 days are added (8 in 2028): 2028-01-03 New Year’s Day (Substitute day)"), "{out}");
    assert!(out.contains("covers listed years moves from 2019..2027 to 2019..2028"), "{out}");
    assert_eq!(code(&koyomi(dir, &["source", "fetch", "test.cal"])), 0);
    assert_eq!(std::fs::read(dir.join("data/bank-holidays.json")).unwrap(), new);
    assert_eq!(code(&koyomi(dir, &["source", "pin", "test.cal"])), 0);
    assert_eq!(std::fs::read_to_string(dir.join("test.cal")).unwrap(), cal.replace(&short(&old), &short(&new)));
    let o = koyomi(dir, &["check", "test.cal"]);
    assert_eq!(code(&o), 0, "{}", text(&o));
    assert!(text(&o).contains("2019-01-01..2028-12-31"), "{}", text(&o));
}

#[test]
fn pin_changes_only_the_digits() {
    let t = TempDir::new("pin");
    let dir = t.path();
    std::fs::create_dir_all(dir.join("data")).unwrap();
    std::fs::write(dir.join("data/h.csv"), "2026-01-01,元日\r\n2026-05-04,みどりの日\r\n").unwrap();
    let h = short(&std::fs::read(dir.join("data/h.csv")).unwrap());
    // A source line with no pin and a comment after it, in a file with CR LF line endings.
    let cal = "calendar 試験(test) v1\r\n\r\nsource 休日 = file \"data/h.csv\"   # 手で書いた表\r\n  format csv\r\n  covers 2026-01-01..2026-12-31\r\n\r\nclosed 休日\r\n";
    std::fs::write(dir.join("試験.cal"), cal).unwrap();
    let o = koyomi(dir, &["source", "pin", "試験.cal"]);
    assert_eq!(code(&o), 0, "{}", text(&o));
    let after = std::fs::read(dir.join("試験.cal")).unwrap();
    assert_eq!(String::from_utf8(after).unwrap(), cal.replace("\"data/h.csv\"   #", &format!("\"data/h.csv\" sha256:{h}   #")));
    // Run again: nothing to change, and the file keeps its bytes.
    let o = koyomi(dir, &["source", "pin", "試験.cal", "--lang", "ja"]);
    assert!(text(&o).contains(&format!("休日: sha256:{h} で固定済みです。表は 2 行、covers 2026-01-01..2026-12-31")), "{}", text(&o));
    assert_eq!(code(&koyomi(dir, &["check", "試験.cal"])), 0);
    // A pin that is wrong is rewritten in place.
    let wrong = String::from_utf8(std::fs::read(dir.join("試験.cal")).unwrap()).unwrap().replace(&h, "0123456789abcdef");
    std::fs::write(dir.join("試験.cal"), &wrong).unwrap();
    assert_eq!(code(&koyomi(dir, &["source", "pin", "試験.cal"])), 0);
    assert_eq!(String::from_utf8(std::fs::read(dir.join("試験.cal")).unwrap()).unwrap(), wrong.replace("0123456789abcdef", &h));
}

// ── e-Gov, served from inside the test ───────────────────────────────────────

fn base64(b: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                s.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

/// What the server changes in an article as of a later date.
#[derive(Clone, Copy, PartialEq)]
enum Change {
    None,
    /// One character of the text of 第143条, from that date on.
    Text(&'static str),
    /// Only an attribute of 第143条's markup, from that date on.
    Markup(&'static str),
}

/// e-Gov law API v2's `law_data` and `law_revisions` for the Civil Code, from the copies.
struct Egov {
    addr: String,
    stop: Arc<AtomicBool>,
    change: Arc<Mutex<Change>>,
    requests: Arc<Mutex<Vec<String>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

fn respond(target: &str, change: Change) -> (u16, String) {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let q: std::collections::HashMap<&str, &str> = query.split('&').filter_map(|kv| kv.split_once('=')).collect();
    if path == "/law_revisions/129AC0000000089" {
        let mut revs = vec![serde_json::json!({"law_revision_id": REVISION, "amendment_enforcement_date": "2026-06-24"})];
        for d in LATER {
            revs.push(serde_json::json!({"law_revision_id": format!("129AC0000000089_{}_TEST", d.replace('-', "")), "amendment_enforcement_date": d}));
        }
        revs.push(serde_json::json!({"law_revision_id": "129AC0000000089_UNKNOWN", "amendment_enforcement_date": null}));
        return (200, serde_json::json!({"law_info": {"law_id": "129AC0000000089"}, "revisions": revs}).to_string());
    }
    if path == "/law_data/129AC0000000089" && q.get("law_full_text_format") == Some(&"xml") {
        let (Some(asof), Some(elm)) = (q.get("asof"), q.get("elm")) else { return (400, "{}".into()) };
        let Ok(xml) = std::fs::read_to_string(format!("{LAW}/{elm}.xml")) else { return (404, "{}".into()) };
        let xml = match change {
            Change::Text(from) if elm.ends_with("_143") && *asof >= from => xml.replace("暦に従って計算する", "暦に従つて計算する"),
            Change::Markup(from) if elm.ends_with("_143") && *asof >= from => xml.replace("WritingMode=\"vertical\"", "WritingMode=\"horizontal\""),
            _ => xml,
        };
        let rev = if *asof == "2026-10-01" { REVISION.to_string() } else { format!("129AC0000000089_{}_TEST", asof.replace('-', "")) };
        return (200, serde_json::json!({"law_info": {"law_id": "129AC0000000089"}, "revision_info": {"law_revision_id": rev}, "law_full_text": base64(xml.as_bytes())}).to_string());
    }
    (404, "{}".into())
}

impl Egov {
    fn start() -> Egov {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("http://{}", l.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let change = Arc::new(Mutex::new(Change::None));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let (s, c, r) = (Arc::clone(&stop), Arc::clone(&change), Arc::clone(&requests));
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
                r.lock().unwrap().push(target.clone());
                let (status, body) = respond(&target, *c.lock().unwrap());
                let reason = if status == 200 { "OK" } else { "Not Found" };
                let _ = write!(conn, "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            }
        });
        Egov { addr, stop, change, requests, thread: Some(thread) }
    }

    fn set(&self, c: Change) {
        *self.change.lock().unwrap() = c;
    }
}

impl Drop for Egov {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr.trim_start_matches("http://"));
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

const PERIOD: &str = "dates 期間(period) v1
source 民法 = law \"129AC0000000089\" asof 2026-10-01
  第140条 sha256:e880059021fbb67d
  第143条 sha256:6950bdfb988439b6

inputs
  起点(origin)      : date  range >=2026-01-01 <=2026-12-31
  月数(month_count) : int   range >=1 <=12

date 起算日(first_day) = 起点            @民法 第140条
  + 1 day
date 満了日(last_day) = 起算日           @民法 第141条, 第143条
  + 月数 months else start_of_next_month
  - 1 day
";

fn egov_run(dir: &Path, server: &Egov, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_koyomi")).args(args).current_dir(dir).env_remove("KOYOMI_LANG").env("KOYOMI_EGOV", &server.addr).output().unwrap()
}

#[test]
fn a_law_from_egov() {
    if no_curl() {
        return;
    }
    let server = Egov::start();
    let t = TempDir::new("egov");
    let dir = t.path();
    std::fs::write(dir.join("期間.cal"), PERIOD).unwrap();
    // 第141条 is cited and not pinned: check says so (E111), and there is no copy yet.
    let o = koyomi(dir, &["check", "期間.cal"]);
    assert!(text(&o).contains("E101"), "{}", text(&o));

    let o = egov_run(dir, &server, &["source", "fetch", "期間.cal"]);
    assert_eq!(code(&o), 0, "{}", text(&o));
    let copies = dir.join("sources/law/129AC0000000089@2026-10-01");
    for a in ["140", "141", "143"] {
        let f = format!("MainProvision-Article_{a}.xml");
        assert_eq!(std::fs::read(copies.join(&f)).unwrap(), std::fs::read(format!("{LAW}/{f}")).unwrap(), "fetch writes {f} as e-Gov gave it");
    }
    assert_eq!(std::fs::read_to_string(copies.join("revision.txt")).unwrap(), format!("{REVISION}\n"));
    assert!(server.requests.lock().unwrap().iter().any(|r| r == "/law_data/129AC0000000089?asof=2026-10-01&elm=MainProvision-Article_143&law_full_text_format=xml"));
    let o = koyomi(dir, &["check", "期間.cal"]);
    assert!(text(&o).contains("E111") && text(&o).contains("第141条"), "{}", text(&o));

    // pin adds the line for 第141条 after the last pin of the source, and keeps the rest.
    let o = koyomi(dir, &["source", "pin", "期間.cal"]);
    assert_eq!(code(&o), 0, "{}", text(&o));
    assert!(text(&o).contains("民法: added a pin line for the cited 第141条 (sha256:0575c131b9f08063)"), "{}", text(&o));
    let pinned = std::fs::read_to_string(dir.join("期間.cal")).unwrap();
    assert_eq!(pinned, PERIOD.replace("  第143条 sha256:6950bdfb988439b6\n", "  第143条 sha256:6950bdfb988439b6\n  第141条 sha256:0575c131b9f08063\n"));
    let o = koyomi(dir, &["check", "期間.cal"]);
    assert_eq!(code(&o), 0, "{}", text(&o));

    // No later revision changes the text: exit 0, one line a revision.
    let o = egov_run(dir, &server, &["source", "outdated", "期間.cal"]);
    assert_eq!(code(&o), 0, "{}", text(&o));
    assert_eq!(text(&o).matches("leaves the cited articles as they are").count(), 5, "{}", text(&o));
    assert!(text(&o).contains("民法: the revision in force from 2027-06-23 (129AC0000000089_20270623_TEST) leaves the cited articles as they are"), "{}", text(&o));

    // One character of 第143条 changes from 2028-06-13: that revision and that article.
    server.set(Change::Text("2028-06-13"));
    let o = egov_run(dir, &server, &["source", "outdated", "期間.cal", "--lang", "ja"]);
    assert_eq!(code(&o), 1, "{}", text(&o));
    let out = text(&o);
    assert!(out.contains("民法: 2028-06-13 施行の版（129AC0000000089_20280613_TEST）で 第143条 が変わります"), "{out}");
    assert!(out.contains("第143条: - 週、月又は年によって期間を定めたときは、その期間は、暦に従って計算する。"), "{out}");
    assert!(out.contains("第143条: + 週、月又は年によって期間を定めたときは、その期間は、暦に従つて計算する。"), "{out}");
    assert_eq!(out.matches("引いている条は変わりません").count(), 4, "the change is said once, on the day it comes into force:\n{out}");

    // An attribute alone is not an amendment: the text is what is compared.
    server.set(Change::Markup("2027-12-05"));
    let o = egov_run(dir, &server, &["source", "outdated", "期間.cal"]);
    assert_eq!(code(&o), 0, "{}", text(&o));
    // And fetch keeps the bytes of a copy whose text did not change, so the pin stays true.
    let before = std::fs::read(copies.join("MainProvision-Article_143.xml")).unwrap();
    server.set(Change::Markup("2026-01-01"));
    let o = egov_run(dir, &server, &["source", "fetch", "期間.cal"]);
    assert!(text(&o).contains("the text of 第143条 is unchanged"), "{}", text(&o));
    assert_eq!(std::fs::read(copies.join("MainProvision-Article_143.xml")).unwrap(), before);
}

#[test]
fn check_reads_no_network() {
    // check, eval, gen, vectors and api never call curl: with an empty PATH they still work.
    let dir = std::env::current_dir().unwrap();
    for args in [
        vec!["check", "examples/calendars", "examples/net30.cal"],
        vec!["api", "examples/民法の期間.cal"],
        vec!["eval", "examples/net30.cal", "invoice_date=2026-03-04"],
    ] {
        let o = Command::new(env!("CARGO_BIN_EXE_koyomi")).args(&args).current_dir(&dir).env("PATH", "").output().unwrap();
        assert_eq!(code(&o), 0, "{args:?}: {}", text(&o));
    }
}

#[test]
fn the_real_sources_when_asked() {
    if std::env::var("KOYOMI_NET").as_deref() != Ok("1") {
        println!("not asked: KOYOMI_NET is not 1, so the real Cabinet Office, GOV.UK and e-Gov were not asked (set KOYOMI_NET=1 to ask them)");
        return;
    }
    if no_curl() {
        return;
    }
    let dir = std::env::current_dir().unwrap();
    for f in ["examples/calendars/東京の営業日.cal", "examples/calendars/england_and_wales.cal"] {
        let o = koyomi(&dir, &["source", "outdated", f]);
        println!("$ koyomi source outdated {f}\n{}", text(&o));
        assert!(matches!(code(&o), 0 | 1), "{f}: {}", text(&o));
    }
    // The Civil Code: none of the five later revisions changes 140–143 (checked on
    // 2026-10-02). If this fails, the Code was amended, and DESIGN 1.5 is to be rewritten.
    let o = koyomi(&dir, &["source", "outdated", "examples/民法の期間.cal"]);
    println!("$ koyomi source outdated examples/民法の期間.cal\n{}", text(&o));
    assert_eq!(code(&o), 0, "{}", text(&o));
    assert_eq!(text(&o).matches("leaves the cited articles as they are").count(), 5, "{}", text(&o));
}
