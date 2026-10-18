//! A key written in a `.kn` (W901, ritsu's DESIGN 16.3; DESIGN 12): said once for each place it is
//! at, in a comment or in a string, by its kind, its prefix and its length, never the key nor its
//! line; said of a module that does not read too; not said where the line marks it a value for
//! tests. A key on a line another diagnostic quotes is masked there. The keys are put together from
//! pieces here, so that this file holds none.

mod common;

use kaname::check::Run;
use ritsu_base::text::Lang;

fn key() -> String {
    ["AIzaSyD-ritsu-fake-", "key-for-tests-000000"].concat()
}

fn check(src: &str) -> kaname::check::Outcome {
    Run::new(common::joined()).text("maps.kn", src)
}

#[test]
fn a_key_in_a_comment_or_a_string_is_said_by_its_kind_and_prefix() {
    let key = key();
    let src = format!("module maps v1\n\n# the maps API key of the staging site: {key}\nfn url(q: string) -> string\n  concat(\"https://maps.example.com/v1/find?q=\", q, \"&key={key}\")\n");
    let o = check(&src);
    let at: Vec<(&str, usize, usize)> = o.diags.iter().map(|d| (d.code, d.line.unwrap_or(0), d.col.unwrap_or(0))).collect();
    // one for each place: the comment, and the string, where `"` ends the key as gitleaks' rule for
    // Google ends one (a key followed by `&` in a URL is not one: ritsu's DESIGN 16.3)
    assert_eq!(at, [("W901", 3, 41), ("W901", 5, 58)]);
    for lang in [Lang::En, Lang::Ja] {
        let text = kaname::check::render(&o, lang);
        assert!(!text.contains(&key), "{text}");
        assert!(!text.contains(" | "), "a line is quoted: {text}");
        assert!(text.contains("AIza…"), "{text}");
    }
    assert!(kaname::check::render(&o, Lang::En).contains("A Google API key is written here (AIza…, 39 characters)"));
    assert!(kaname::check::render(&o, Lang::Ja).contains("Google の API キーがここに書かれています（AIza…、39 文字）"));
    // a warning: the module still checks
    assert!(o.checked.is_some());
}

#[test]
fn a_key_in_a_module_that_does_not_read_is_said_too() {
    let key = key();
    let o = check(&format!("module broken v1\n\nfn f(x: number range >=0 <=9) -> number\n  x +\n# {key}\n"));
    let codes: Vec<&str> = o.diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, ["E001", "W901"], "{codes:?}");
}

#[test]
fn a_value_for_tests_is_not_said() {
    let key = key();
    let o = check(&format!("module maps v1\n\nfn same(code: string) -> bool\n  code = \"{key}\"   # ritsu: test secret\n"));
    assert!(o.diags.is_empty(), "{:?}", o.diags.iter().map(|d| d.code).collect::<Vec<_>>());
}

#[test]
fn a_key_on_a_line_another_diagnostic_quotes_is_masked() {
    let key = key();
    // `==` is no comparison of kaname: E001 quotes the line that holds the key
    let o = check(&format!("module maps v1\n\nfn same(code: string) -> bool\n  code == \"{key}\"\n"));
    let codes: Vec<&str> = o.diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, ["E001", "W901"], "{codes:?}");
    for lang in [Lang::En, Lang::Ja] {
        let text = kaname::check::render(&o, lang);
        assert!(!text.contains(&key), "{text}");
        assert!(text.contains("code == \"AIza…\""), "{text}");
    }
}
