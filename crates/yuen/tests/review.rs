//! `yuen review` (DESIGN 4.2, 4.4, PLAN B.10).

mod common;

use common::{TempDir, yuen};

const REQ: &str = "period/民法の期間.req";

#[test]
fn review_clears_the_marks_and_changes_nothing_but_the_records() {
    for (name, change) in [("142", common::change_142 as fn(&std::path::Path)), ("text", common::change_text), ("cal", common::change_cal)] {
        let t = common::fixture("period");
        let d = t.path().join("period");
        change(&d);
        let before = std::fs::read_to_string(d.join("民法の期間.req")).unwrap();
        let old: std::collections::BTreeSet<String> = std::fs::read_dir(d.join("reviewed")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
        let r = yuen(t.path(), &["review", "period", "--root", "period", "--all", "--by", "法務", "--date", "2026-10-04"]);
        assert_eq!(r.code, 0, "{name}: {}{}", r.stdout, r.stderr);
        let after = std::fs::read_to_string(d.join("民法の期間.req")).unwrap();
        // Only record lines changed, and every changed one carries the new date.
        let (b, a): (Vec<&str>, Vec<&str>) = (before.lines().collect(), after.lines().collect());
        assert_eq!(b.len(), a.len(), "{name}: no line added or removed");
        for (x, y) in b.iter().zip(&a) {
            if x != y {
                assert!(x.trim_start().starts_with("reviewed ") || x.trim_start().starts_with("approved "), "{name}: {x:?} changed");
                assert!(y.contains(" 2026-10-04 by 法務 "), "{name}: {y:?}");
            }
        }
        let c = yuen(t.path(), &["check", "period", "--root", "period"]);
        assert_eq!(c.code, 0, "{name}: {}", c.stdout);
        // reviewed/: every file is named by its hash; the new contents came, the old ones went.
        let now: std::collections::BTreeSet<String> = std::fs::read_dir(d.join("reviewed")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
        for f in &now {
            let b = std::fs::read(d.join("reviewed").join(f)).unwrap();
            assert_eq!(&ritsu_base::sha256::short(&b), f, "{name}: reviewed/{f}");
        }
        assert_ne!(now, old, "{name}");
        for f in old.difference(&now) {
            assert!(!after.contains(f.as_str()), "{name}: reviewed/{f} went, and a record points at it");
        }
        for h in yuen::marks::hashes_in(&after) {
            if after.contains(&format!("-> sha256:{h}")) || after.lines().any(|l| l.trim_start().starts_with("approved ") && l.contains(&h)) {
                assert!(now.contains(&h) || h == "c9b94eecde23e6b5" || h == "553a87c58fb264e0" || name == "cal", "{name}: reviewed/{h} is missing");
            }
        }
    }
}

#[test]
fn review_writes_the_records_a_person_made() {
    // The fixture's records, written again from nothing: the same lines.
    let t = common::fixture("period");
    let d = t.path().join("period");
    let want = std::fs::read_to_string(d.join("民法の期間.req")).unwrap();
    let bare: String = want.lines().filter(|l| !l.trim_start().starts_with("reviewed ") && !l.trim_start().starts_with("approved ")).map(|l| format!("{l}\n")).collect();
    std::fs::write(d.join("民法の期間.req"), &bare).unwrap();
    std::fs::remove_dir_all(d.join("reviewed")).unwrap();
    // The links to the .cal by 開発, the rest by 法務, as the fixture was made.
    let ats: Vec<String> = bare.lines().enumerate().filter(|(_, l)| l.contains("satisfied by file")).map(|(i, _)| format!("{REQ}:{}", i + 1)).collect();
    let mut args = vec!["review", "period", "--root", "period", "--by", "開発", "--date", "2026-10-03"];
    for a in &ats {
        args.push("--at");
        args.push(a);
    }
    let r = yuen(t.path(), &args);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    let r = yuen(t.path(), &["review", "period", "--root", "period", "--all", "--by", "法務", "--date", "2026-10-03"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert_eq!(std::fs::read_to_string(d.join("民法の期間.req")).unwrap(), want);
    let kept: Vec<String> = std::fs::read_dir(d.join("reviewed")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    let mut fixture: Vec<String> = std::fs::read_dir("tests/fixtures/period/reviewed").unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    let mut kept = kept;
    kept.sort();
    fixture.sort();
    assert_eq!(kept, fixture);
}

#[test]
fn crlf_stays_crlf() {
    let t = common::fixture("period");
    let d = t.path().join("period");
    let lf = std::fs::read_to_string(d.join("民法の期間.req")).unwrap();
    std::fs::write(d.join("民法の期間.req"), lf.replace('\n', "\r\n")).unwrap();
    common::change_142(&d);
    // A link with no record, too: its new line is added with CR LF.
    let crlf = std::fs::read_to_string(d.join("民法の期間.req")).unwrap();
    let crlf = crlf.replace("    reviewed 2026-10-03 by 開発 sha256:a9ebc73907faddc8 -> sha256:c9b94eecde23e6b5\r\n", "");
    std::fs::write(d.join("民法の期間.req"), &crlf).unwrap();
    let r = yuen(t.path(), &["review", "period", "--root", "period", "--all", "--by", "法務", "--date", "2026-10-04"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    let after = std::fs::read(d.join("民法の期間.req")).unwrap();
    let after = String::from_utf8(after).unwrap();
    assert!(!after.replace("\r\n", "").contains('\n'), "every line ends with CR LF");
    assert_eq!(after.matches("\r\n").count(), crlf.matches("\r\n").count() + 1);
    assert_eq!(yuen(t.path(), &["check", "period", "--root", "period"]).code, 0);
}

#[test]
fn what_review_does_not_write() {
    let t = common::fixture("period");
    let d = t.path().join("period");
    let before = std::fs::read_to_string(d.join("民法の期間.req")).unwrap();
    // A link with no mark, chosen by its line: nothing written, and it says so.
    let r = yuen(t.path(), &["review", "period", "--root", "period", "--at", &format!("{REQ}:21"), "--by", "法務", "--date", "2026-10-04"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("not written, since nothing is marked"), "{}", r.stdout);
    assert_eq!(std::fs::read_to_string(d.join("民法の期間.req")).unwrap(), before);
    // A line that has no link: exit 2.
    let r = yuen(t.path(), &["review", "period", "--root", "period", "--at", &format!("{REQ}:16"), "--by", "法務"]);
    assert_eq!(r.code, 2, "{}", r.stderr);
    // A role not declared: E008, exit 1, nothing written.
    common::change_142(&d);
    let changed = std::fs::read_to_string(d.join("民法の期間.req")).unwrap();
    let r = yuen(t.path(), &["review", "period", "--root", "period", "--all", "--by", "法務部"]);
    assert_eq!(r.code, 1);
    assert!(r.stdout.contains("error[E008]"), "{}", r.stdout);
    assert_eq!(std::fs::read_to_string(d.join("民法の期間.req")).unwrap(), changed);
    // No choice: exit 2.
    assert_eq!(yuen(t.path(), &["review", "period", "--by", "法務"]).code, 2);
    // No role: exit 2.
    assert_eq!(yuen(t.path(), &["review", "period", "--all"]).code, 2);
    // Errors of the words: exit 1, nothing written.
    std::fs::write(d.join("民法の期間.req"), changed.replace("requirements 民法の期間 v1", "requirements 民法の期間")).unwrap();
    let r = yuen(t.path(), &["review", "period", "--root", "period", "--all", "--by", "法務"]);
    assert_eq!(r.code, 1, "{}", r.stdout);
}

#[test]
fn a_link_whose_ends_cannot_be_made_is_not_written() {
    let t = common::fixture("period");
    let d = t.path().join("period");
    common::change_text(&d);
    std::fs::remove_file(d.join("民法の期間.cal")).unwrap();
    let r = yuen(t.path(), &["review", "period", "--root", "period", "--all", "--by", "法務", "--date", "2026-10-04"]);
    assert_eq!(r.code, 1, "{}", r.stdout);
    assert!(r.stdout.contains("not written, since the ends of the link cannot be read"), "{}", r.stdout);
    // What could be written was: the `from` and the waiver of 満了日.
    assert!(std::fs::read_to_string(d.join("民法の期間.req")).unwrap().contains("2026-10-04 by 法務"));
}

#[test]
fn a_comment_after_a_record_stays_and_the_last_line_gets_its_record() {
    let t = TempDir::new("review-edge");
    t.write("a.txt", b"a\n");
    t.write(
        "t.req",
        "requirements t v1\nrole 経理\n\nrequirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"y\"\n  satisfied by file \"a.txt\"\n    reviewed 2026-10-01 by 経理 sha256:0000000000000000 -> sha256:0000000000000000  # 前の記録\n  not verified \"z\"".as_bytes(),
    );
    let r = yuen(t.path(), &["review", ".", "--root", ".", "--all", "--by", "経理", "--date", "2026-10-03"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    let after = t.read("t.req");
    assert!(after.contains("-> sha256:87428fc522803d31  # 前の記録\n"), "{after}");
    assert!(after.ends_with("  not verified \"z\"\n    approved 2026-10-03 by 経理 sha256:fbdfb71af500ce5f"), "{after}");
    assert_eq!(yuen(t.path(), &["check", ".", "--root", "."]).code, 0);
}
