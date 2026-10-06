//! English snapshots of the same diagnostics that `golden.rs` pins in Japanese.
//!
//! One binary per language: the output language is a process-wide setting
//! (`rulec::i18n::set`), and cargo runs the tests of one binary in parallel
//! threads, so a binary must not mix languages. Re-bless with
//! `RULEC_BLESS=1 cargo test --test golden_en`.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rendered(rel: &str, code: &str) -> (String, String) {
    rulec::i18n::set(rulec::i18n::Lang::En);
    let src = std::fs::read_to_string(root().join(rel)).unwrap_or_else(|_| panic!("cannot read {rel}"));
    let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
    let ds = rulec::check_source(&src, rel);
    let hit: Vec<_> = ds.iter().filter(|d| d.code == code).collect();
    assert!(!hit.is_empty(), "{rel} did not produce {code}");
    let got = hit.iter().map(|d| rulec::diag::render(d, &lines)).collect::<Vec<_>>().join("\n");
    (got, src)
}

/// Kana, ideographs, and the punctuation Japanese writes with them (`〜`, `「」`, the full-width
/// parentheses): all of it is Japanese in an English rendering.
fn is_cjk(c: char) -> bool {
    ('\u{3000}'..='\u{30ff}').contains(&c)
        || ('\u{3400}'..='\u{4dbf}').contains(&c)
        || ('\u{4e00}'..='\u{9fff}').contains(&c)
        || ('\u{ff00}'..='\u{ffef}').contains(&c)
}

/// The Japanese an English rendering may say although the rule did not, and why. Everything else
/// that is Japanese has to be the rule's own (below). A word is here only because the
/// diagnostic's point is the Japanese itself.
const KEPT: &[(&str, &str)] = &[
    // E011 says a name that is not ASCII needs an alias, and shows one: kanji and kana are the
    // names it is about (the entry in the ledger keeps `重量(weight)` for the same reason).
    ("届け先", "the example name of E011's hint, `届け先(dest)`"),
];

/// An English rendering must not leak Japanese prose. Names, cell values and unit
/// symbols quoted from the rule file are Japanese by design, so a run of Japanese
/// characters is allowed exactly when it occurs in the rule source (comments and the
/// description excluded), in a rule it applies, or is a built-in prefecture name, or is one of
/// the few words in [`KEPT`], each with its reason. The units and multipliers the tool writes
/// back (`円`, `万`) are not among them: it writes them in the rule's own spelling, and an
/// amount of many digits in groups (`1_000_000JPY`).
fn assert_english(got: &str, src: &str) {
    let mut allowed: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with('#') && !l.trim_start().starts_with("description"))
        .map(|l| l.split('#').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    for (ja, _) in rulec::prelude::PREFECTURES {
        allowed.push('\n');
        allowed.push_str(ja);
    }
    // A rule applied by this one is quoted the same way: its names come from its own file.
    for l in src.lines().filter(|l| l.trim_start().starts_with("apply ")) {
        if let Some(path) = l.split('"').nth(1) {
            let p = root().join("tests/mutants").join(path);
            if let Ok(callee) = std::fs::read_to_string(&p) {
                allowed.push('\n');
                allowed.push_str(&callee.lines().map(|l| l.split('#').next().unwrap_or("")).collect::<Vec<_>>().join("\n"));
            }
        }
    }
    for (word, _) in KEPT {
        allowed.push('\n');
        allowed.push_str(word);
    }
    let mut leaks: Vec<String> = Vec::new();
    for line in got.lines() {
        // Frame lines quote the source verbatim.
        let t = line.trim_start();
        if t.starts_with('|') || t.chars().next().is_some_and(|c| c.is_ascii_digit()) && t.contains(" | ") {
            continue;
        }
        let mut run = String::new();
        for c in line.chars().chain(std::iter::once(' ')) {
            if is_cjk(c) {
                run.push(c);
            } else if !run.is_empty() {
                if !allowed.contains(run.as_str()) {
                    leaks.push(run.clone());
                }
                run.clear();
            }
        }
    }
    assert!(leaks.is_empty(), "Japanese prose in an English rendering: {leaks:?}\n{got}");
}

fn snapshot(name: &str, (got, src): (String, String)) {
    assert_english(&got, &src);
    let dir = root().join("tests/golden/en");
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{name}.txt"));
    if std::env::var("RULEC_BLESS").is_ok() || !Path::new(&p).exists() {
        std::fs::write(&p, &got).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&p).unwrap();
    if want != got {
        panic!(
            "wording changed: {name}\n--- pinned ---\n{want}\n--- now ---\n{got}\n\
             If intended, re-bless with RULEC_BLESS=1 cargo test --test golden_en."
        );
    }
}

macro_rules! golden {
    ($fname:ident, $name:literal, $file:literal, $code:literal) => {
        #[test]
        fn $fname() {
            snapshot($name, rendered($file, $code));
        }
    };
}

golden!(e101_completeness, "E101", "tests/mutants/m_e101.rule", "E101");
golden!(e101_date_gap, "E101-date", "tests/mutants/m_e101d.rule", "E101");
golden!(e101_fractional_step, "E101-frac", "tests/mutants/m_e101frac.rule", "E101");
golden!(e102_unreachable, "E102", "tests/mutants/m_e102.rule", "E102");
golden!(e102_upstream_value, "E102-b", "tests/mutants/m_e102b.rule", "E102");
golden!(e102_upstream_pair, "E102-c", "tests/mutants/m_e102c.rule", "E102");
golden!(e103_unit, "E103", "tests/mutants/m_e103.rule", "E103");
golden!(e103_bare_decimal, "E103-bare", "tests/mutants/m_e103bare.rule", "E103");
golden!(e104_rounding_lookup, "E104-b", "tests/mutants/m_e104.rule", "E104");
golden!(e104_rounding_fraction, "E104-a", "tests/mutants/m_e104b.rule", "E104");
golden!(e105_overlap, "E105", "tests/mutants/m_e105.rule", "E105");
golden!(e106_grid, "E106", "tests/mutants/m_e106.rule", "E106");
golden!(e107_examples, "E107", "tests/mutants/m_e107.rule", "E107");
golden!(e108_overflow, "E108", "tests/mutants/m_e108.rule", "E108");
golden!(e111_missing_expected, "E111", "tests/mutants/m_e111.rule", "E111");
golden!(e112_derived_range, "E112", "tests/mutants/m_e112.rule", "E112");
golden!(e113_atom, "E113", "tests/mutants/m_e113.rule", "E113");
golden!(w114_unverified_overlap, "W114", "tests/mutants/m_w114.rule", "W114");
golden!(w111_unused, "W111", "tests/mutants/m_w111.rule", "W111");
golden!(w105_needs_review, "W105", "tests/mutants/m_w105.rule", "W105");
golden!(w105_shipping, "W105-b", "tests/corpus/送料.rule", "W105");
golden!(w105_order, "W105-c", "tests/corpus/適用順序.rule", "W105");
// The same two over their twins (tests/corpus/twins.tsv): English throughout, the source lines too.
golden!(w105_shipping_twin, "W105-b-twin", "tests/corpus/member_shipping_fee.rule", "W105");
golden!(w105_order_twin, "W105-c-twin", "tests/corpus/coupon_order.rule", "W105");
golden!(e010_range_notation, "E010", "tests/mutants/m_e010.rule", "E010");
golden!(e011_alias, "E011", "tests/mutants/m_e011.rule", "E011");

// Labels and tables that share an output (§15.66).
golden!(e034_duplicate_label, "E034", "tests/mutants/m_e034.rule", "E034");
golden!(e035_missing_target, "E035", "tests/mutants/m_e035.rule", "E035");
golden!(w117_idle_exception, "W117", "tests/mutants/m_w117.rule", "W117");
golden!(e046_clause_shape, "E046", "tests/mutants/m_e046.rule", "E046");
golden!(e037_unpinned, "E037", "tests/mutants/m_e037.rule", "E037");
golden!(e038_copy_changed, "E038", "tests/mutants/m_e038.rule", "E038");
golden!(e039_no_copy, "E039", "tests/mutants/m_e039.rule", "E039");
golden!(w119_uncited_pin, "W119", "tests/mutants/m_w119.rule", "W119");

// A rule applied by another (§15.69).
golden!(e040_callee_changed, "E040", "tests/mutants/m_e040.rule", "E040");
golden!(e041_unbound_input, "E041", "tests/mutants/m_e041.rule", "E041");
golden!(e042_unmapped_value, "E042", "tests/mutants/m_e042.rule", "E042");
golden!(e043_outside_range, "E043", "tests/mutants/m_e043.rule", "E043");
golden!(e044_not_applicable, "E044", "tests/mutants/m_e044.rule", "E044");
golden!(w118_unused_table, "W118", "tests/mutants/m_w118.rule", "W118");

// A rule that is one step of a state machine (§15.148).
golden!(e124_final_exits, "E124", "tests/mutants/m_e124.rule", "E124");
golden!(e125_cannot_finish, "E125", "tests/mutants/m_e125.rule", "E125");
golden!(e126_never_broken, "E126", "tests/mutants/m_e126.rule", "E126");
golden!(e127_once_broken, "E127", "tests/mutants/m_e127.rule", "E127");
golden!(e107_scenario, "E107-scenario", "tests/mutants/m_e126.rule", "E107");
golden!(w125_unreached_state, "W125", "tests/mutants/m_w125.rule", "W125");

// ── The pages and the amounts an English reader sees ─────────────────────────────────────────

/// What a command prints, in a language, as the person who typed it sees it.
fn run(args: &[&str], lang: &str) -> String {
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", lang)
        .current_dir(root())
        .args(args)
        .args(["--lang", lang])
        .output()
        .expect("cannot start rulec");
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// Every `--help` page in English is written in English, but for the words of `KEPT`: the examples
/// use the English names of the corpus (`rules/member_shipping_fee.rule`, as `tests/corpus/twins.tsv`
/// pairs it with `送料.rule`) and the placeholders are English (`<field=value>`). The Japanese pages
/// are as they were.
#[test]
fn english_help_pages_have_no_japanese_but_what_is_kept() {
    let top = run(&["--help"], "en");
    let mut pages = vec![("rulec".to_string(), top.clone())];
    for l in top.lines() {
        if let Some(cmd) = l.strip_prefix("  rulec ").and_then(|t| t.split_whitespace().next()) {
            pages.push((cmd.to_string(), run(&[cmd, "--help"], "en")));
        }
    }
    assert!(pages.len() > 20, "the commands of --help were not found: {}", pages.len());
    let mut leaks: Vec<String> = Vec::new();
    for (cmd, page) in &pages {
        let mut run = String::new();
        for c in page.chars().chain(std::iter::once(' ')) {
            if is_cjk(c) {
                run.push(c);
            } else if !run.is_empty() {
                if !KEPT.iter().any(|(w, _)| *w == run) {
                    leaks.push(format!("{cmd}: {run}"));
                }
                run.clear();
            }
        }
    }
    assert!(leaks.is_empty(), "Japanese in an English --help page: {leaks:?}");
    // And the Japanese pages still show the Japanese examples.
    let ja = run(&["verify", "--help"], "ja");
    assert!(ja.contains("rules/送料.rule"), "{ja}");
    let en = run(&["verify", "--help"], "en");
    assert!(en.contains("rules/member_shipping_fee.rule") && !en.contains("送料"), "{en}");
}

/// A range and a big amount are written in English as the rule's source writes them: `to` between
/// the ends, and the digits in groups of three joined by `_` (`10_000_000JPY`), where Japanese
/// writes `〜` and `1000万JPY`. The Japanese page is as it was.
#[test]
fn english_doc_writes_ranges_and_big_amounts_in_english() {
    let file = "tests/corpus/member_shipping_fee.rule";
    let en = run(&["doc", file], "en");
    assert!(en.contains("| weight | mass[g] | 1g to 40_000g |"), "{en}");
    assert!(en.contains("| total | money[JPY, incl_tax] | 0JPY to 10_000_000JPY |"), "{en}");
    assert!(!en.contains('万') && !en.contains('億') && !en.contains('〜'), "{en}");
    let ja = run(&["doc", file], "ja");
    assert!(ja.contains("| weight | mass[g] | 1g 〜 4万g |") && ja.contains("0JPY 〜 1000万JPY"), "{ja}");
}
