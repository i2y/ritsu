//! The letters of a name are those of ritsu-emit's tables (Unicode 17.0, `ident::is_letter`),
//! whichever Rust built ritsu (PLAN 7.134). The `char` of Rust 1.100 reads Unicode 18.0, which
//! makes letters of 12,833 more characters; built with it, each language's lexer used to read them
//! into names, and one built with Rust 1.99 did not. Here a name with U+0558, a letter of Unicode
//! 18.0, reads in every language as one with `¶`, a character no Unicode makes a letter: each
//! lexer's tokens and words, and sakai's names, are the same but for the character.

/// The answer of `read` for `src` with `¶` in it, and for `src` with U+0558 in its place, that
/// character written back as `¶` (and its number as U+00B6).
fn both<T: std::fmt::Debug>(read: impl Fn(&str) -> T, src: &str) -> (String, String) {
    let plain = format!("{:?}", read(src));
    let later = format!("{:?}", read(&src.replace('¶', "\u{558}"))).replace('\u{558}', "¶").replace("\\u{558}", "¶").replace("U+0558", "U+00B6");
    (plain, later)
}

#[test]
fn a_letter_of_a_later_unicode_is_no_letter_of_a_name() {
    let c = '\u{558}';
    assert!(!ritsu_emit::ident::is_letter(c) && !ritsu_emit::ident::is_letter_or_number(c));
    assert!(!kaname::lex::starts_name(c) && !kaname::lex::is_name_char(c));
    assert!(!sekisho::lex::is_name_char(c));
    assert!(!rulec::lex::is_bare_word("\u{558}a"));
    let cases: Vec<(&str, (String, String))> = vec![
        // rulec reads a name's first character as a letter, and any after it up to a delimiter
        ("rulec", both(|s| rulec::lex::lex_line(1, s), "define ¶a : number = 1")),
        ("rulec (after the first)", both(|s| rulec::lex::lex_line(1, s), "define a¶ : number = 1")),
        ("chobo", both(chobo::syntax::lex, "transfer build(permit: string, area¶: sqm)\n")),
        ("chobo (first)", both(chobo::syntax::lex, "transfer build(¶area: sqm)\n")),
        ("dandori", both(dandori::syntax::parse, "flow f\n  step a¶ = 1\n")),
        ("kaname", both(|s| kaname::lex::lex("t.kn", s), "fn f(x¶: int) -> int = x¶\n")),
        ("koyomi", both(|s| koyomi::lex::lex("t.koyomi", s), "calendar c¶\n")),
        ("sekisho", both(|s| sekisho::lex::lex("t.sekisho", s), "permit a¶\n")),
        ("yuen", both(|s| yuen::lex::lex("t.yuen", "t.yuen", s), "decided d¶\n")),
        ("sakai", both(|s| sakai::parse::parse("t.ctx", s), "context c¶\n")),
    ];
    for (lang, (plain, later)) in cases {
        assert_eq!(later, plain, "{lang} reads U+0558 as it does no other character that is no letter");
    }
}
