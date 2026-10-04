//! The mutants (PLAN B.10, B.12): every `tests/mutants/<CODE>_<what>/` is a base map of
//! `tests/maps/` (named in its file `base`) with one change, and gives its code. What `sakai check`
//! prints for it, in English and in Japanese, is its golden file in `tests/golden/`.
//! `SAKAI_BLESS=1 cargo test` writes the golden files again; read the diff.
//!
//! A mutant gives its code and no other, unless its `README.md` says why: one change can break two
//! things at once (a shared kernel written on one side no longer allows the import it allowed).

mod common;

use sakai::check::{check_args, render, to_json};
use ritsu_base::text::Lang;

#[test]
fn every_mutant_gives_its_code_and_says_what_its_golden_files_say() {
    let mut failures = Vec::new();
    let mut n = 0;
    for name in common::mutants() {
        let dir = common::mutant(&name);
        let code = name.split('_').next().unwrap();
        // A mutant with a command (`build …`) is run with the binary in its directory, as `check .`
        // is rendered here: the paths come out the same.
        let (codes, texts): (Vec<String>, Vec<String>) = match common::mutant_command(&name) {
            Some(args) => {
                let en = String::from_utf8_lossy(&common::sakai_in(dir.path(), &args.iter().map(String::as_str).collect::<Vec<_>>()).stdout).to_string();
                let mut ja_args: Vec<&str> = args.iter().map(String::as_str).collect();
                ja_args.extend(["--lang", "ja"]);
                let ja = String::from_utf8_lossy(&common::sakai_in(dir.path(), &ja_args).stdout).to_string();
                (common::printed_codes(&en), vec![en, ja])
            }
            None => {
                // with every language joined, as `ritsu sakai` checks; E104 is what the binary of
                // sakai's own crate says, joining none
                let os = if code == "E104" { check_args(dir.path(), &[".".to_string()]).unwrap() } else { common::check_dir(dir.path()) };
                let codes = os.iter().flat_map(|o| o.diags.iter().map(|d| d.code.to_string())).collect();
                (codes, [Lang::En, Lang::Ja].iter().map(|&lang| os.iter().map(|o| render(o, lang)).collect()).collect())
            }
        };
        if !codes.iter().any(|c| c == code) {
            failures.push(format!("{name} does not give {code}: {codes:?}"));
        }
        let readme = std::fs::read_to_string(format!("tests/mutants/{name}/README.md")).unwrap_or_default();
        for other in codes.iter().filter(|c| *c != code) {
            if !readme.contains(other.as_str()) {
                failures.push(format!("{name} gives {other} too, and its README.md does not say why: {codes:?}"));
            }
        }
        for (text, tag) in texts.iter().zip(["en", "ja"]) {
            if let Some(f) = common::golden(&format!("tests/golden/{name}.{tag}.txt"), text) {
                failures.push(f);
            }
        }
        n += 1;
    }
    assert!(n >= 55, "{n} mutants");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The JSON of a diagnostic keeps its keys in English, in the order DESIGN 5.1 gives them.
#[test]
fn the_json_of_a_diagnostic() {
    let dir = common::mutant("E401_注文の状態に値が増えた");
    let os = check_args(dir.path(), &[".".to_string()]).unwrap();
    for lang in [Lang::En, Lang::Ja] {
        let v = to_json(&os[0], lang);
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["root", "file", "ok", "summary", "diagnostics"]);
        let d = &v["diagnostics"][0];
        let keys: Vec<&String> = d.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["code", "severity", "file", "line", "col", "message", "notes", "references", "fix"]);
        assert_eq!(d["code"], "E401");
        assert_eq!(d["file"], "ctx/請求.ctx");
        assert_eq!(d["fix"], "ORDER_STATUS_RETURNED -> refuse \"…\"");
        let r = &d["references"][1];
        assert_eq!(r["context"], "受注");
        assert_eq!(r["name"]["text"], "proto \"proto/shop/ordering/v1/order.proto\" enum OrderStatus");
        assert_eq!(v["ok"], false);
    }
}

/// No golden file is left without its mutant.
#[test]
fn every_golden_file_has_its_mutant() {
    let ms = common::mutants();
    for e in std::fs::read_dir("tests/golden").unwrap() {
        let e = e.unwrap();
        if e.path().is_dir() {
            continue;
        }
        let n = e.file_name().to_string_lossy().to_string();
        let stem = n.trim_end_matches(".en.txt").trim_end_matches(".ja.txt");
        assert!(ms.iter().any(|m| m == stem), "tests/golden/{n} has no mutant");
    }
}

/// The JSON of a diagnostic of the English twin of the mutant above.
#[test]
fn the_json_of_a_diagnostic_in_english() {
    let dir = common::mutant("E401_value_added_to_the_order_status");
    let os = check_args(dir.path(), &[".".to_string()]).unwrap();
    for lang in [Lang::En, Lang::Ja] {
        let v = to_json(&os[0], lang);
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["root", "file", "ok", "summary", "diagnostics"]);
        let d = &v["diagnostics"][0];
        let keys: Vec<&String> = d.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["code", "severity", "file", "line", "col", "message", "notes", "references", "fix"]);
        assert_eq!(d["code"], "E401");
        assert_eq!(d["file"], "ctx/billing.ctx");
        assert_eq!(d["fix"], "ORDER_STATUS_RETURNED -> refuse \"…\"");
        let r = &d["references"][1];
        assert_eq!(r["context"], "Ordering");
        assert_eq!(r["name"]["text"], "proto \"proto/shop/ordering/v1/order.proto\" enum OrderStatus");
        assert_eq!(v["ok"], false);
    }
}
