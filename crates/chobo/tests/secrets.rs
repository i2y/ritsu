//! A key on a line another diagnostic of chobo quotes is masked there (ritsu-base's
//! `secrets::mask`), in the text and in the JSON's `excerpt`; W901 says the key once, and quotes no
//! line. The book is laid out here, its key put together from pieces.

use chobo::diag::Show;
use ritsu_base::text::Lang;

#[test]
fn a_key_on_a_quoted_line_is_masked() {
    let key = ["AIzaSyD-ritsu-fake-", "key-for-tests-000000"].concat();
    // `yen` is no unit of the book: E002 on the line that holds the key
    let src = format!("book wallet v1\nunit jpy\naccount balance(customer: string) : yen   # read with {key}\n  at least 0 refused as not_enough\n");
    let c = chobo::check::check_source(&src);
    assert!(c.diags.iter().any(|d| d.code == "W901"), "{:?}", c.diags.iter().map(|d| d.code).collect::<Vec<_>>());
    assert!(c.diags.iter().any(|d| d.is_error() && d.line == Some(3)), "{:?}", c.diags.iter().map(|d| (d.code, d.line)).collect::<Vec<_>>());
    for lang in [Lang::En, Lang::Ja] {
        let text: String = c.diags.iter().map(|d| d.shown("wallet.book", &src, lang)).collect();
        assert!(!text.contains(&key), "{text}");
        assert!(text.contains("# read with AIza…"), "{text}");
        let json: String = c.diags.iter().map(|d| d.json_in("wallet.book", &src, lang).to_string()).collect();
        assert!(!json.contains(&key), "{json}");
    }
}
