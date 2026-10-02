//! `geas affected` on a small shop, before and after one agent-style change: the
//! claims a diff touches, the changed code no claim runs, and the codes that refuse
//! a record or a diff (E060-E064). Everything is under `tests/affected/`: the two
//! trees, a record of each, and the diffs. Nothing here runs a toolchain.
//!
//! The records were made by running, in a copy of `before/` and of `after/`,
//! `geas map shop.geas --root .` (it needs python3 and port 8126), and taking
//! `.geas/shop.map.jsonl`; a change to the record's format makes them again so.
//! The diffs are `git diff -M` between two trees written into a scratch index
//! (`git add -A` and `git write-tree`, no commit): the change, and one part of it
//! at a time, from `after/` with that part undone. The `.plain.diff` files are
//! `diff -u <file>.orig <file>`.

mod common;
use common::*;
use std::fs;

const SPEC: &str = "shop.geas";

/// A scratch holding the shop's tree on one side of the change as its root, every
/// diff beside it, and the records under the names given.
fn shop(name: &str, side: &str, records: &[(&str, &str)]) -> Scratch {
    let s = Scratch::new(name);
    let dir = root().join("tests/affected");
    for e in fs::read_dir(dir.join(side)).expect("a tree") {
        let p = e.expect("an entry").path();
        fs::copy(&p, s.path().join(p.file_name().expect("a name"))).expect("copy a file");
    }
    for e in fs::read_dir(&dir).expect("the fixtures") {
        let p = e.expect("an entry").path();
        let n = p.file_name().expect("a name").to_string_lossy().into_owned();
        if n.ends_with(".diff") {
            fs::copy(&p, s.path().join(&n)).expect("copy a diff");
        }
    }
    for (from, to) in records {
        s.write(to, &fs::read_to_string(dir.join(from)).expect("a record"));
    }
    s
}

/// The shop after the change, with the record of it where `map` writes it.
fn after(name: &str) -> Scratch {
    shop(name, "after", &[("after.map.jsonl", ".geas/shop.map.jsonl")])
}

/// Runs `affected` on a diff with more arguments; holds the exit status and the
/// stream that has the answer to the golden, the other stream empty.
fn affected(s: &Scratch, diff: &str, more: &[&str], exit: i32, golden_name: &str) -> String {
    let mut args = vec!["affected", SPEC, diff, "--root", "."];
    args.extend(more);
    let (out, err, code) = run(s.path(), &args, &[]);
    assert_eq!(code, exit, "{golden_name}: stdout {out:?}, stderr {err:?}");
    let (shown, other) = if exit == 2 { (err, out) } else { (out, err) };
    assert_eq!(other, "", "{golden_name}");
    golden(golden_name, &shown);
    shown
}

#[test]
fn the_whole_change_in_both_languages() {
    let s = after("affected-full");
    let out = affected(&s, "full.diff", &[], 1, "en/affected/full.txt");
    assert!(out.contains("every claim that starts `api`: 1, 2, 3"), "{out}");
    affected(&s, "full.diff", &["--lang", "ja"], 1, "ja/affected/full.txt");
}

#[test]
fn exact_touches_on_the_after_side() {
    let s = after("affected-touches");
    let out = affected(&s, "touches.diff", &[], 0, "en/affected/touches.txt");
    assert!(out.contains("2 - prices an item with tax") && out.contains("3 - does not price"), "{out}");
}

#[test]
fn lines_no_claim_runs() {
    let s = after("affected-unclaimed");
    affected(&s, "unclaimed.diff", &[], 1, "en/affected/unclaimed.txt");
}

#[test]
fn a_file_no_runtime_reported() {
    let s = after("affected-unreported");
    let out = affected(&s, "unreported.diff", &[], 1, "en/affected/unreported.txt");
    assert!(out.contains("legacy.py: 3 (no runtime reported this file)"), "{out}");
}

#[test]
fn a_document_outside_the_record() {
    let s = after("affected-document");
    affected(&s, "document.diff", &[], 0, "en/affected/document.txt");
}

#[test]
fn a_binary_file() {
    let s = after("affected-binary");
    affected(&s, "binary.diff", &[], 0, "en/affected/binary.txt");
}

#[test]
fn the_spec_changed() {
    let s = after("affected-spec");
    affected(&s, "spec.diff", &[], 1, "en/affected/spec.txt");
}

#[test]
fn the_baseline_changed() {
    let s = after("affected-baseline");
    affected(&s, "baseline.diff", &[], 1, "en/affected/baseline.txt");
}

#[test]
fn startup_code_touches_every_claim_that_starts_the_target() {
    let s = after("affected-startup");
    let out = affected(&s, "startup.diff", &[], 0, "en/affected/startup.txt");
    assert!(out.contains("every claim that starts `api`: 1, 2, 3"), "{out}");
}

#[test]
fn a_rename() {
    let s = after("affected-rename");
    affected(&s, "rename.diff", &[], 0, "en/affected/rename.txt");
}

#[test]
fn an_added_file() {
    let s = after("affected-added");
    affected(&s, "added.diff", &[], 0, "en/affected/added.txt");
}

/// With only a record of the code after the change, which claims ran a deleted
/// file is not known, and that needs a person.
#[test]
fn a_deleted_file() {
    let s = after("affected-deleted");
    affected(&s, "deleted.diff", &[], 1, "en/affected/deleted.txt");
}

/// A removed line, with only a record of the code after the change, touches the
/// claims that run the code around where it was.
#[test]
fn a_removal_attributed_near() {
    let s = after("affected-near");
    let out = affected(&s, "app.plain.diff", &[], 1, "en/affected/near.txt");
    assert!(out.contains("removed 18 (next to lines they run)"), "{out}");
}

/// With a record of each side, every removed line is looked up exactly, deleted
/// files included.
#[test]
fn two_records_and_exact_removals() {
    let s = shop(
        "affected-two",
        "after",
        &[("after.map.jsonl", ".geas/shop.map.jsonl"), ("before.map.jsonl", "before.map.jsonl")],
    );
    let out = affected(&s, "full.diff", &["--map", "before.map.jsonl", "--map", ".geas/shop.map.jsonl"], 1, "en/affected/two-records.txt");
    assert!(out.contains("app.py: removed 6-7,18"), "{out}");
    // the order of the two records does not matter
    let (again, _, _) = run(s.path(), &["affected", SPEC, "full.diff", "--root", ".", "--map", ".geas/shop.map.jsonl", "--map", "before.map.jsonl"], &[]);
    assert_eq!(again.lines().skip(1).collect::<Vec<_>>(), out.lines().skip(1).collect::<Vec<_>>());
}

/// A plain diff is held to the disk: here the disk is the code after the change.
#[test]
fn a_plain_diff_with_the_disk_after_the_change() {
    let s = after("affected-plain-after");
    let plain = affected(&s, "app.plain.diff", &[], 1, "en/affected/plain-after.txt");
    // app.py's lines are those of the git diff of the whole change
    let (git, _, _) = run(s.path(), &["affected", SPEC, "full.diff", "--root", "."], &[]);
    let app = |text: &str| -> Vec<String> { text.lines().filter(|l| l.starts_with("      app.py")).map(String::from).collect() };
    assert_eq!(app(&plain), app(&git));
}

/// Here the disk is the code before the change, so the record of it answers.
#[test]
fn a_plain_diff_with_the_disk_before_the_change() {
    let s = shop("affected-plain-before", "before", &[("before.map.jsonl", ".geas/shop.map.jsonl")]);
    let out = affected(&s, "trim.plain.diff", &[], 0, "en/affected/plain-before.txt");
    assert!(out.contains("(the code before the change)") && out.contains("sign.py: removed 6"), "{out}");
}

#[test]
fn e064_a_plain_diff_that_fits_neither_side() {
    let s = shop("affected-neither", "before", &[("before.map.jsonl", ".geas/shop.map.jsonl")]);
    affected(&s, "neither.plain.diff", &[], 2, "en/affected/E064-neither.txt");
    affected(&s, "neither.plain.diff", &["--lang", "ja"], 2, "ja/affected/E064-neither.txt");
}

#[test]
fn e064_a_diff_that_does_not_read() {
    let s = after("affected-not-a-diff");
    s.write("broken.diff", "diff --git a/app.py b/app.py\nindex 63d46ed..570f870 100644\n--- a/app.py\n+++ b/app.py\n@@ -3,8 +3,8 @@\n import sys\n-import old\n");
    affected(&s, "broken.diff", &[], 2, "en/affected/E064-broken.txt");
    affected(&s, "broken.diff", &["--lang", "ja"], 2, "ja/affected/E064-broken.txt");
}

/// A file on disk that is not what the record holds, and that the diff does not
/// touch: the record is of other code.
#[test]
fn e062_an_untouched_file_changed_on_disk() {
    let s = after("affected-stale");
    s.write("sign.py", &format!("{}print(\"Closed on Sundays\")\n", s.read("sign.py")));
    let out = affected(&s, "full.diff", &[], 2, "en/affected/E062.txt");
    assert!(out.contains("sign.py: the record has 37ed622"), "{out}");
    affected(&s, "full.diff", &["--lang", "ja"], 2, "ja/affected/E062.txt");
}

/// A new file the diff does not add makes the record stale: `git diff` leaves out a
/// file git does not track, and its lines would go unreported.
#[test]
fn e062_a_new_file_missing_from_the_diff() {
    let s = shop("affected-untracked", "before", &[("before.map.jsonl", ".geas/shop.map.jsonl")]);
    s.write("discount.py", "def discounted(price):\n    return price * 90 // 100\n");
    let out = affected(&s, "trim.plain.diff", &[], 2, "en/affected/E062-untracked.txt");
    assert!(out.contains("1 file(s) differ") && out.contains("discount.py: on disk and not in the record"), "{out}");
}

#[test]
fn e063_only_a_record_of_the_code_before() {
    let s = shop("affected-before-only", "after", &[("before.map.jsonl", ".geas/shop.map.jsonl")]);
    affected(&s, "full.diff", &[], 2, "en/affected/E063.txt");
    affected(&s, "full.diff", &["--lang", "ja"], 2, "ja/affected/E063.txt");
}

#[test]
fn e060_no_record() {
    let s = shop("affected-no-record", "after", &[]);
    affected(&s, "full.diff", &[], 2, "en/affected/E060.txt");
    affected(&s, "full.diff", &["--lang", "ja"], 2, "ja/affected/E060.txt");
    affected(&s, "full.diff", &["--map", "kept.jsonl"], 2, "en/affected/E060-map.txt");
}

#[test]
fn e061_a_record_of_another_spec() {
    let s = after("affected-other-spec");
    let rec = s.read(".geas/shop.map.jsonl").replacen("\"spec\":\"shop.geas\"", "\"spec\":\"tools/shop.geas\"", 1);
    s.write(".geas/shop.map.jsonl", &rec);
    affected(&s, "full.diff", &[], 2, "en/affected/E061-other-spec.txt");
    affected(&s, "full.diff", &["--lang", "ja"], 2, "ja/affected/E061-other-spec.txt");
    s.write(".geas/shop.map.jsonl", "{\"geas_map\":2}\n");
    affected(&s, "full.diff", &[], 2, "en/affected/E061-format.txt");
    s.write(".geas/shop.map.jsonl", &format!("{}\nnot json\n", rec.lines().next().expect("a header").replace("tools/", "")));
    affected(&s, "full.diff", &[], 2, "en/affected/E061-not-json.txt");
}

#[test]
fn the_diff_from_stdin() {
    let s = after("affected-stdin");
    let diff = s.read("touches.diff");
    let mut child = std::process::Command::new(geas())
        .args(["affected", SPEC, "-", "--root", "."])
        .current_dir(s.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("start geas");
    use std::io::Write as _;
    child.stdin.take().expect("piped").write_all(diff.as_bytes()).expect("write the diff");
    let out = child.wait_with_output().expect("geas ends");
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).expect("UTF-8");
    assert!(text.starts_with("diff: <stdin> · "), "{text}");
    golden("en/affected/stdin.txt", &text);
}

#[test]
fn as_json() {
    let s = shop(
        "affected-json",
        "after",
        &[("after.map.jsonl", ".geas/shop.map.jsonl"), ("before.map.jsonl", "before.map.jsonl")],
    );
    let out = affected(&s, "full.diff", &["--json", "--map", "before.map.jsonl", "--map", ".geas/shop.map.jsonl"], 1, "en/affected/full.json");
    let v = json(&out);
    assert_eq!(v.get("ok"), &Json::Bool(false));
    let sides: Vec<&str> = v.get("records").arr().iter().map(|r| r.get("side").str()).collect();
    assert_eq!(sides, ["before", "after"]);
    assert_eq!(v.get("deleted").arr()[0].get("known"), &Json::Bool(true));
    // a refused record answers as a diagnostic, in JSON too
    let (out, _, code) = run(s.path(), &["affected", SPEC, "full.diff", "--root", ".", "--json", "--map", "nope.jsonl"], &[]);
    assert_eq!(code, 2);
    assert_eq!(json(&out).get("diagnostics").arr()[0].get("code").str(), "E060");
}

#[test]
fn arguments_affected_does_not_take() {
    let s = after("affected-args");
    for (name, args) in [
        ("affected-one-argument", vec!["affected", SPEC]),
        ("affected-three-maps", vec!["affected", SPEC, "full.diff", "--map", "a", "--map", "b", "--map", "c"]),
        ("affected-out", vec!["affected", SPEC, "full.diff", "--out", "x"]),
    ] {
        let (out, err, code) = run(s.path(), &args, &[]);
        assert_eq!((out.as_str(), code), ("", 2), "{name}");
        golden(&format!("en/errors/E080-{name}.txt"), &err);
    }
}
