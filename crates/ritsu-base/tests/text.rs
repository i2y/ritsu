//! The two languages (DESIGN 4.1): the width of Japanese text, the language a run prints in,
//! and the small things both languages need.

use ritsu_base::text::{self, Lang, Text};
use ritsu_base::tr;

#[test]
fn wide_and_full_width_characters_take_two_columns() {
    assert_eq!(text::width("abc"), 3);
    assert_eq!(text::width("支払日"), 6);
    assert_eq!(text::width("ｱｲｳ"), 3, "half-width katakana is one column");
    assert_eq!(text::width("ＡＢ"), 4, "full-width letters are two");
    assert_eq!(text::width("、。"), 4);
    assert_eq!(text::width("2026-10-03（土）"), 16);
    assert_eq!(text::pad("支払日", 8), "支払日  ");
    assert_eq!(text::pad("abc", 2), "abc");
}

#[test]
fn the_language_is_the_flag_then_the_tools_variable_then_ritsu_lang_then_english() {
    use Lang::*;
    assert_eq!(Lang::choose(Some("ja"), Some("en"), Some("en")), Ja);
    assert_eq!(Lang::choose(None, Some("ja"), Some("en")), Ja);
    assert_eq!(Lang::choose(None, None, Some("ja")), Ja);
    assert_eq!(Lang::choose(None, None, None), En);
    assert_eq!(Lang::choose(Some("en"), Some("ja"), Some("ja")), En);
    // Longer spellings, and a value that names neither language, which is passed over.
    assert_eq!(Lang::choose(None, Some("ja_JP.UTF-8"), None), Ja);
    assert_eq!(Lang::choose(None, Some("fr"), Some("ja")), Ja);
    assert_eq!(Lang::choose(Some("EN-us"), None, Some("ja")), En);
    // The flag wins whatever the environment says.
    assert_eq!(Lang::pick(Some("ja"), "RITSU_BASE_TEST_NO_SUCH_VAR"), Ja);
    assert_eq!(Lang::pick(Some("en"), "RITSU_BASE_TEST_NO_SUCH_VAR"), En);
    assert_eq!((Ja.code(), En.code()), ("ja", "en"));
}

#[test]
fn tr_writes_both_halves_over_the_same_arguments() {
    let n = 3;
    let t = tr!("{n} 件", "{n} items");
    assert_eq!(t, Text::new("3 件", "3 items"));
    let (ja, en) = ("三", "three");
    let t = tr!("{}件", "{} items", ja; en);
    assert_eq!((t.get(Lang::Ja), t.get(Lang::En)), ("三件", "three items"));
    assert!(Text::default().is_empty());
    assert_eq!(Text::same("x").then(&tr!("あ", "a")), Text::new("xあ", "xa"));
    assert_eq!(tr!("{{who}} が見た", "{{who}} looked").sub("who", &tr!("法務", "legal")), Text::new("法務 が見た", "legal looked"));
    assert_eq!(Text::join(&[Text::same("a"), Text::same("b")], "・", " / "), Text::new("a・b", "a / b"));
}

#[test]
fn counts_plurals_and_the_two_ways_of_printing_a_message() {
    assert_eq!(text::count(1067), "1,067");
    assert_eq!(text::count(871_596), "871,596");
    assert_eq!(text::count(0), "0");
    assert_eq!(text::plural(1, "link", "links"), "1 link");
    assert_eq!(text::plural(1067, "day", "days"), "1,067 days");
    let t = tr!("invoice_dateは月末", "invoice_date is the last day");
    assert_eq!(text::as_written(&t, Lang::En), "invoice_date is the last day");
    assert_eq!(text::as_written(&t, Lang::Ja), "invoice_dateは月末");
    assert_eq!(text::spaced(&t, Lang::En), "Invoice_date is the last day");
    assert_eq!(text::spaced(&t, Lang::Ja), "invoice_date は月末");
    assert_eq!(text::capitalize("élan"), "élan", "only an ASCII letter is raised");
}
