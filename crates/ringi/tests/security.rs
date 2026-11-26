//! Bounds and characters of the names and labels of a request: bidirectional embedding,
//! override and isolate controls are refused, and a name is at most 64 characters.

mod common;

use ritsu_base::text::Lang;

fn source(first: &str, label: &str) -> String {
    format!("{first} \"Example\"\n\nform\n  amount : money[JPY] \"{label}\"  range >=1JPY <=100_000JPY\n\nroute\n  check  manager\n\nwho\n  manager = manager of requester\n\nticket\n  self approval  not allowed\n")
}

#[test]
fn bidirectional_controls_in_labels_and_names_are_refused() {
    let o = common::check_text("example.ringi", &source("request example v1", "Amount\u{202e}"));
    assert!(o.diags.iter().any(|d| d.code == "E002" && d.message.en.contains("changes the direction of text")), "{:?}", o.diags);
    assert!(o.diags.iter().any(|d| d.code == "E002" && d.message.ja.contains("文の向きを変える制御文字")), "{:?}", o.diags);
    // in a name the lexer refuses the same controls before the declarations are read
    let o = common::check_text("example.ringi", &source("request exa\u{2066}mple v1", "Amount"));
    assert!(o.diags.iter().any(|d| d.is_error()), "the name is taken");
}

#[test]
fn a_name_longer_than_sixty_four_characters_is_refused() {
    let name = "x".repeat(65);
    let o = common::check_text("example.ringi", &source(&format!("request {name} v1"), "Amount"));
    assert!(o.diags.iter().any(|d| d.code == "E002" && d.message.en.contains("64 characters at most")), "{}", common::shown(&o, Lang::En));
    assert!(o.diags.iter().any(|d| d.code == "E002" && d.message.ja.contains("64 字まで")), "{}", common::shown(&o, Lang::Ja));
}
