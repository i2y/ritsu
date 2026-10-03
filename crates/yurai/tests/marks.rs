//! Marks (DESIGN 4.3, PLAN B.7): the five changes of PLAN B.7 made to a copy of the `period`
//! fixture, the marks each gives, and what `check` prints for them in both languages, against
//! the golden files in `tests/golden/marks/`.

mod common;

use common::{change_142, change_cal, change_text, edit};
use std::path::Path;

fn drop_records(d: &Path) {
    edit(d, "民法の期間.req", "    reviewed 2026-10-03 by 開発 sha256:a9ebc73907faddc8 -> sha256:c9b94eecde23e6b5\n", "");
    edit(d, "民法の期間.req", "    approved 2026-10-03 by 法務 sha256:465b83ed8c251406\n", "");
}

fn without_reviewed(d: &Path) {
    std::fs::remove_dir_all(d.join("reviewed")).unwrap();
    change_142(d);
}

/// Each change: its name, what it does, and the codes it gives, in order.
#[allow(clippy::type_complexity)]
fn cases() -> Vec<(&'static str, fn(&Path), Vec<&'static str>)> {
    vec![
        ("1_142条の本文", change_142, vec!["E302", "E302", "E304"]),
        ("2_満了日の文", change_text, vec!["E303", "E302", "E304"]),
        ("3_calの一行", change_cal, vec!["E303", "E303", "E303"]),
        ("4_記録と承認を消す", drop_records, vec!["E301", "E304"]),
        ("5_reviewedを消して1", without_reviewed, vec!["E302", "W301", "E302", "W301", "E304"]),
    ]
}

#[test]
fn the_five_changes() {
    let mut failures = Vec::new();
    for (name, change, want) in cases() {
        let t = common::fixture("period");
        change(&t.path().join("period"));
        let en = common::yurai(t.path(), &["check", "period", "--root", "period"]);
        let ja = common::yurai(t.path(), &["check", "period", "--root", "period", "--lang", "ja"]);
        assert_eq!(en.code, 1, "{name}");
        let got = common::codes(&en.stdout);
        if got != want {
            failures.push(format!("{name}: want {want:?}, got {got:?}\n{}", en.stdout));
        }
        common::golden(&format!("tests/golden/marks/{name}.en.txt"), &en.stdout, &mut failures);
        common::golden(&format!("tests/golden/marks/{name}.ja.txt"), &ja.stdout, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Where a diff is: the first mark of a change shows it, the others say it is the same.
#[test]
fn a_diff_is_shown_once() {
    let t = common::fixture("period");
    change_142(&t.path().join("period"));
    let r = common::yurai(t.path(), &["check", "period", "--root", "period"]);
    let blocks: Vec<&str> = r.stdout.split("\nerror[").collect();
    assert!(blocks[0].contains("+ 期間の末日が日曜日") && blocks[0].contains("翌々日"), "{}", blocks[0]);
    assert!(!blocks[1].contains("@@"), "{}", blocks[1]);
    let t = common::fixture("period");
    change_cal(&t.path().join("period"));
    let r = common::yurai(t.path(), &["check", "period", "--root", "period"]);
    assert_eq!(r.stdout.matches("@@ -").count(), 1, "{}", r.stdout);
    assert_eq!(r.stdout.matches("The same change as at period/民法の期間.req:21").count(), 2, "{}", r.stdout);
    // Without reviewed/, no diff at all.
    let t = common::fixture("period");
    without_reviewed(&t.path().join("period"));
    let r = common::yurai(t.path(), &["check", "period", "--root", "period"]);
    assert!(!r.stdout.contains("@@"), "{}", r.stdout);
}

/// A content that is not text, or over 1 MiB, is not diffed; its size and hash are said.
#[test]
fn what_is_not_text() {
    let t = common::TempDir::new("binary");
    t.write("a.bin", &[0u8, 159, 146, 150]);
    t.write(
        "t.req",
        b"requirements t v1\nrole \xe7\xb5\x8c\xe7\x90\x86\n\nrequirement r1\n  text \"x\"\n  owner \xe7\xb5\x8c\xe7\x90\x86\n  decided 2026-10-03 by \xe7\xb5\x8c\xe7\x90\x86 \"y\"\n  satisfied by file \"a.bin\"\n  not verified \"z\"\n",
    );
    let r = common::yurai(t.path(), &["review", ".", "--root", ".", "--all", "--by", "経理", "--date", "2026-10-03"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    // reviewed/ keeps the requirement's end, which is text, and not the binary file.
    let kept: Vec<String> = std::fs::read_dir(t.path().join("reviewed")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    assert_eq!(kept.len(), 1, "{kept:?}");
    t.write("a.bin", &[0u8, 159, 146, 151]);
    let r = common::yurai(t.path(), &["check", ".", "--root", "."]);
    assert_eq!(common::codes(&r.stdout), ["E303", "W301"], "{}", r.stdout);
}
