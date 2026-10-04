//! koyomi's reference interpreter, held to `KoyomiModel` (DESIGN 11.3).
//!
//! Every `.cal` of koyomi's examples and test fixtures that resolves (the two examples that break
//! a claim on purpose included: their dates are still computed): each line of `koyomi vectors` —
//! every input of the range and the inputs just outside it, or every day a calendar knows and the
//! days just outside its data — is given by the reference interpreter and by the Lean model, and
//! the two answers are compared. The rows are made while they are compared, as koyomi's own
//! `tests/targets.rs` does, and none is left out. `-- --nocapture` shows a
//! `compared koyomi <file>: <n> lines` line for each file.

mod common;

use common::{calendar, dates};
use koyomi::ast::At;
use koyomi::check::{Checked, check};
use koyomi::vectors::{self, Expect, Row};
use ritsu_model::{compare, program};
use ritsu_testkit::{Need, TempDir, ready};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn koyomi_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../koyomi")
}

/// The `.cal` files of a directory: the English ones (ASCII names) first.
fn cals(dir: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(koyomi_dir().join(dir))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "cal"))
        .collect();
    v.sort_by_key(|p| (!p.file_name().unwrap().to_string_lossy().is_ascii(), p.clone()));
    v
}

/// An answer as `KoyomiModel.answerLine` writes it.
fn answer(e: &Expect) -> String {
    match e {
        Expect::Values(vs) => format!("[{}]", vs.iter().map(|v| format!("\"{v}\"")).collect::<Vec<_>>().join(",")),
        Expect::Open(o) => format!("{{\"open\":{o}}}"),
        Expect::Error(k) => format!("{{\"error\":\"{k}\"}}"),
    }
}

fn input(r: &Row) -> String {
    format!("[{}]", r.vals.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","))
}

#[test]
fn every_line_of_every_vectors_comes_out_as_the_model_says() {
    let bin = program();
    if !ready(Need::Lean, || bin.exists(), "proofs/ is not built (lake build makes ritsu-model)") {
        return;
    }
    let _ = At::EndOfDay;
    let tmp = TempDir::new("model-koyomi");
    let files: Vec<PathBuf> = ["examples", "examples/calendars", "tests/fixtures", "tests/fixtures/calendars"].iter().flat_map(|d| cals(d)).collect();
    let (mut seen, mut lines) = (0, 0usize);
    for p in files {
        let shown = p.strip_prefix(koyomi_dir()).unwrap().display().to_string();
        let o = check(&p.to_string_lossy()).unwrap_or_else(|e| panic!("{shown}: {e}"));
        let Some(checked) = o.checked else {
            panic!("{shown} does not resolve");
        };
        let (json, rows): (Value, Box<dyn Iterator<Item = Row> + Send + '_>) = match &checked {
            Checked::Dates(m, _) => (dates(m), Box::new(vectors::DatesRows::new(m))),
            Checked::Calendar(c) => (json!({"kind": "calendar", "calendar": calendar(c)}), Box::new(vectors::calendar_rows(c))),
        };
        let file = tmp.write(&format!("{seen}.json"), json.to_string());
        let c = compare(&bin, "koyomi", &file, rows.map(|r| (input(&r), answer(&r.expect))), &|a, b| a == b).unwrap_or_else(|e| panic!("{shown}: {e}"));
        assert_eq!(c.differ, 0, "{}", c.report(&shown));
        println!("compared koyomi {shown}: {} lines", c.lines);
        seen += 1;
        lines += c.lines;
    }
    assert!(seen >= 19, "the files are fewer than they were: {seen}");
    assert!(lines >= 5_000_000, "the lines are fewer than they were: {lines}");
}
