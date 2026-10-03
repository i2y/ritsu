//! `chobo check --diff-base`: a git repository in a temporary directory, a book committed,
//! then changed in the ways the data in a database cannot follow.

mod common;
use common::*;
use std::process::Command;

fn git(dir: &std::path::Path, args: &[&str]) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=chobo-test", "-c", "user.email=chobo-test@example.com", "-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .is_ok_and(|o| o.status.success())
}

#[test]
fn changes_since_a_revision() {
    if Command::new("git").arg("--version").output().is_err() {
        skip("git is not on the PATH; --diff-base needs it");
        return;
    }
    let tmp = TempDir::new("diffbase");
    let dir = tmp.path();
    assert!(git(dir, &["init", "-q"]));
    let before = std::fs::read_to_string(root().join("tests/fixtures/変更.before.book")).unwrap();
    let after = std::fs::read_to_string(root().join("tests/fixtures/変更.book")).unwrap();
    let book = dir.join("変更.book");
    std::fs::write(&book, &before).unwrap();
    assert!(git(dir, &["add", "."]));
    assert!(git(dir, &["commit", "-q", "-m", "first"]));

    // unchanged: nothing to say
    let out = chobo().args(["check", book.to_str().unwrap(), "--diff-base", "HEAD"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(!text.contains("E05") && !text.contains("W107"), "{text}");

    std::fs::write(&book, &after).unwrap();
    let out = chobo().args(["check", book.to_str().unwrap(), "--diff-base", "HEAD"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    for code in ["[E050]", "[E051]", "[W107]"] {
        assert!(text.contains(code), "{code}:\n{text}");
    }
    // as JSON too
    let out = chobo().args(["check", book.to_str().unwrap(), "--diff-base", "HEAD", "--format", "json"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let codes: Vec<String> = v["files"][0]["diagnostics"].as_array().unwrap().iter().map(|d| d["code"].as_str().unwrap().to_string()).collect();
    assert!(codes.contains(&"E050".to_string()) && codes.contains(&"E051".to_string()) && codes.contains(&"W107".to_string()));

    // a book the revision does not have: a note, and nothing compared
    let other = dir.join("新しい.book");
    std::fs::write(&other, &after).unwrap();
    let out = chobo().args(["check", other.to_str().unwrap(), "--diff-base", "HEAD"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("HEAD has no such file"));

    // a revision that is not there is a mistake in the call
    let out = chobo().args(["check", book.to_str().unwrap(), "--diff-base", "no-such-rev"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}
