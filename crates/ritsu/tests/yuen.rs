//! `ritsu yuen` (DESIGN 8.6): yuen's command, with every language it reads joined through the
//! ports — the things of a rule, a calendar, a book, a spec, a workflow or a context, the sources a
//! rule or a calendar pins, and the records of a spec's claims. What only this binary can run is
//! here: the binary, with the environment a test sets (an e-Gov of the test's own).

use ritsu_testkit::TempDir;
use ritsu_testkit::http::HttpServer;
use std::path::{Path, PathBuf};
use std::process::Command;

fn yuen_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../yuen")
}

/// `ritsu`, run in `dir`, with no language asked of the environment, and these variables.
fn ritsu_in(dir: &Path, args: &[&str], envs: &[(&str, &str)]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).env_remove("RITSU_LANG").env_remove("YUEN_LANG").args(args);
    for (k, v) in envs {
        c.env(k, v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// Every test project of yuen that names the things of another language passes `ritsu yuen
/// check`, which the binary of yuen's own crate refuses.
#[test]
fn ritsu_yuen_reads_every_language() {
    for name in ["rulec", "koyomi", "chobo", "geas", "proto", "dandori", "sakai"] {
        let dir = format!("tests/fixtures/{name}");
        let (code, out, err) = ritsu_in(&yuen_dir(), &["yuen", "check", &dir, "--root", &dir], &[]);
        assert_eq!(code, 0, "{name}: {out}{err}");
        assert!(out.starts_with(&format!("{dir}: ok — ")), "{name}: {out}");
    }
    let (code, out, err) = ritsu_in(&yuen_dir(), &["yuen", "check", "tests/fixtures/koyomi", "--root", "tests/fixtures/koyomi", "--lang", "ja"], &[]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("範囲の date 3 個は、どれも要件に辿れます"), "{out}");
    // the words after `yuen` are yuen's
    let (code, out, _) = ritsu_in(&yuen_dir(), &["yuen", "--version"], &[]);
    assert!(code == 0 && out.starts_with("yuen ") && out.lines().count() == 1, "{out}");
}

/// `ritsu yuen affected` follows the change of geas's greeter through the records of its claims.
#[test]
fn ritsu_yuen_follows_a_change_to_the_requirements() {
    let g = "tests/fixtures/geas";
    let (code, out, err) = ritsu_in(
        &yuen_dir(),
        &[
            "yuen",
            "affected",
            g,
            "--root",
            g,
            "--diff",
            "tests/fixtures/geas/changes/change.diff",
            "--map",
            "tests/fixtures/geas/greeter/greeter.geas=tests/fixtures/geas/greeter/.geas/greeter.map.jsonl",
            "--map",
            "tests/fixtures/geas/greeter/greeter.geas=tests/fixtures/geas/changes/after.map.jsonl",
        ],
        &[],
    );
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("  claim \"rejects an empty name\": ") && out.contains("    checked by rejects_an_empty_name\n"), "{out}");
    assert!(out.ends_with("4 requirements touched; ask api\n"), "{out}");
}

const LAW: &str = "sources/law/129AC0000000089@2026-10-01";
const REVISION: &str = "129AC0000000089_20260624_508AC0000000045";
const LATER: [&str; 2] = ["2027-06-23", "2028-06-13"];

fn law_data(xml: &[u8], rev: &str) -> String {
    format!(
        "{{\"law_info\": {{\"law_id\": \"129AC0000000089\"}}, \"revision_info\": {{\"law_revision_id\": \"{rev}\"}}, \"law_full_text\": \"{}\"}}",
        ritsu_base::sources::base64_encode(xml)
    )
}

/// A source borrowed from a calendar is asked about as yuen's own are, from the calendar's pins
/// and copies (yuen's DESIGN 14): an amendment of the 142nd article says the requirement that
/// cites it, the calendar that pins it, and that the calendar is where it is fetched again.
#[test]
fn ritsu_yuen_asks_whether_a_borrowed_source_moved_on() {
    if ritsu_testkit::tools::on_path("curl").is_none() {
        ritsu_testkit::skip("curl is not on the PATH; source outdated is not run");
        return;
    }
    let t = TempDir::new("borrowed");
    ritsu_testkit::tmp::copy_dir(&yuen_dir().join("tests/fixtures/koyomi"), &t.path().join("koyomi"));
    let k = t.path().join("koyomi");
    // the payment terms borrow a file source whose url is on the network: not this test's
    std::fs::remove_file(k.join("支払条件.req")).unwrap();
    let s = HttpServer::start();
    let mut revs = vec![format!("{{\"law_revision_id\": \"{REVISION}\", \"amendment_enforcement_date\": \"2026-06-24\"}}")];
    for d in LATER {
        revs.push(format!("{{\"law_revision_id\": \"129AC0000000089_{}_TEST\", \"amendment_enforcement_date\": \"{d}\"}}", d.replace('-', "")));
    }
    s.set("/egov/law_revisions/129AC0000000089", format!("{{\"law_info\": {{\"law_id\": \"129AC0000000089\"}}, \"revisions\": [{}]}}", revs.join(", ")));
    for a in ["140", "141", "142", "143"] {
        let xml = std::fs::read(k.join(LAW).join(format!("MainProvision-Article_{a}.xml"))).unwrap();
        for d in LATER {
            let text = String::from_utf8(xml.clone()).unwrap();
            let text = if a == "142" && d == "2028-06-13" { text.replace("その翌日に満了する", "その翌々日に満了する") } else { text };
            let rev = format!("129AC0000000089_{}_TEST", d.replace('-', ""));
            s.set(&format!("/egov/law_data/129AC0000000089?asof={d}&elm=MainProvision-Article_{a}&law_full_text_format=xml"), law_data(text.as_bytes(), &rev));
        }
    }
    let egov = format!("{}/egov", s.addr);
    let (code, out, err) = ritsu_in(t.path(), &["yuen", "source", "outdated", "koyomi", "--root", "koyomi"], &[("YUEN_EGOV", &egov)]);
    println!("$ ritsu yuen source outdated koyomi --root koyomi\n{out}");
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("民法: the revision in force from 2027-06-23 (129AC0000000089_20270623_TEST) leaves the cited articles as they are\n"), "{out}");
    assert!(out.contains("民法: the revision in force from 2028-06-13 (129AC0000000089_20280613_TEST) changes 第142条\n"), "{out}");
    assert!(out.contains("  cited by: 満了日_142条 (owned by 法務, koyomi/民法の期間.req:33)\n"), "{out}");
    assert!(out.contains("  pinned by: koyomi \"民法の期間.cal\"\n"), "{out}");
    assert!(out.contains("  in koyomi \"民法の期間.cal\" source 民法, move asof to 2028-06-13, then run koyomi source fetch and koyomi source pin; yuen check then marks these\n"), "{out}");
    assert!(s.missed().is_empty(), "asked for what is not served: {:?}", s.missed());
}
