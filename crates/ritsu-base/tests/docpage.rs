//! The frame of a page (DESIGN 4.8): the light and the dark palette and how a page chooses
//! between them, the head of the page, and that it reads nothing from outside.

use ritsu_base::docpage::{self, Palette};
use ritsu_base::text::Lang;

#[test]
fn the_palette_is_light_dark_when_the_system_asks_and_as_the_page_says() {
    let css = Palette::default().set("closed", "#ecebe6", "#24272d").set("bg", "#fbfaf7", "#15171b").css();
    let lines: Vec<&str> = css.lines().collect();
    assert_eq!(lines.len(), 4, "{css}");
    assert!(lines[0].starts_with(":root { color-scheme: light dark; --bg: #fbfaf7; --fg: #1f2328;"), "{}", lines[0]);
    assert!(lines[0].ends_with("--closed: #ecebe6; }"), "a page's own colour comes after the common ones: {}", lines[0]);
    assert!(lines[1].starts_with("@media (prefers-color-scheme: dark) { :root:not([data-theme=\"light\"]) { color-scheme: dark; --bg: #15171b;"), "{}", lines[1]);
    assert!(lines[2].starts_with(":root[data-theme=\"dark\"] { color-scheme: dark; --bg: #15171b;"), "{}", lines[2]);
    assert_eq!(lines[3], ":root[data-theme=\"light\"] { color-scheme: light; }");
    for role in docpage::ROLES {
        assert!(lines[0].contains(&format!("--{role}: ")) && lines[2].contains(&format!("--{role}: ")), "{role} is in both palettes");
    }
    assert_eq!(docpage::LIGHT.len(), docpage::ROLES.len());
    assert_eq!(docpage::DARK.len(), docpage::ROLES.len());
}

#[test]
fn the_head_of_a_page() {
    let head = docpage::html_head(Lang::Ja, "koyomi 0.1.0", "支払日 <v1>", "body { margin: 0; }\n");
    assert_eq!(
        head,
        "<!doctype html>\n<html lang=\"ja\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"generator\" content=\"koyomi 0.1.0\">\n<title>支払日 &lt;v1&gt;</title>\n<style>\nbody { margin: 0; }\n</style>\n</head>\n<body>\n"
    );
    assert_eq!(docpage::esc("a&b<c>\"d'"), "a&amp;b&lt;c&gt;&quot;d&#39;");
}

#[test]
fn a_page_reads_nothing_from_outside() {
    let page = format!(
        "{}<p><a href=\"#e101\">E101</a> <img src=\"data:image/png;base64,AAAA\"></p>\n{}",
        docpage::html_head(Lang::En, "ritsu", "t", &Palette::default().css()),
        docpage::HTML_TAIL
    );
    assert!(docpage::outside_urls(&page).is_empty(), "{:?}", docpage::outside_urls(&page));
    let bad = "<script src=\"https://cdn.example/x.js\"></script><link href='//fonts.example/f.css'><style>@import \"http://x/y.css\"; .a { background: url(https://x/i.png) }</style>";
    assert_eq!(docpage::outside_urls(bad), ["https://cdn.example/x.js", "//fonts.example/f.css", "https://x/i.png", "http://x/y.css"]);
}
