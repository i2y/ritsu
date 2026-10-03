//! Words and lines (PLAN B.3): every `.cal` DESIGN.md shows is read without an error, and
//! what cannot be read gets E001–E006 at the right place.

use koyomi::parse::parse;

/// The fenced blocks of DESIGN.md.
fn design_blocks() -> Vec<String> {
    let text = std::fs::read_to_string("DESIGN.md").unwrap();
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    for l in text.lines() {
        if l.starts_with("```") {
            match cur.take() {
                Some(b) => out.push(b),
                None => cur = Some(String::new()),
            }
            continue;
        }
        if let Some(b) = cur.as_mut() {
            b.push_str(l);
            b.push('\n');
        }
    }
    out
}

/// A listing with line numbers in front (DESIGN 4.3), without them.
fn unnumber(b: &str) -> String {
    b.lines()
        .map(|l| {
            let t = l.trim_start();
            let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
            let rest = &t[digits.len()..];
            rest.strip_prefix("  ").unwrap_or(rest.trim_start()).to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

const INPUTS: &str = "inputs\n  受領日(received) : date  range >=2026-01-01 <=2026-12-31\n  起点(origin) : date  range >=2026-01-01 <=2026-12-31\n  月数(month_count) : int  range >=1 <=12\n\n";

/// The `.cal` a block of DESIGN.md is, made whole where it shows only a part.
fn as_file(b: &str) -> Option<String> {
    let first = b.lines().find(|l| !l.trim().is_empty())?.trim_start();
    if first.chars().next()?.is_ascii_digit() && (first.contains("  dates ") || first.contains("  calendar ")) {
        return Some(unnumber(b));
    }
    if first.starts_with("calendar ") || first.starts_with("dates ") {
        return Some(b.to_string());
    }
    if first.starts_with("source ") && first.contains("= file") {
        return Some(format!("calendar t v1\n\n{b}\nclosed 祝日\n"));
    }
    if first.starts_with("source ") || first.starts_with("date ") {
        // The source lines go before the inputs, the dates after them.
        let at = if first.starts_with("date ") { 0 } else { b.find("\ndate ").map(|i| i + 1).unwrap_or(b.len()) };
        return Some(format!("dates t v1\n{}\n{INPUTS}{}", &b[..at], &b[at..]));
    }
    if first.starts_with("claims") {
        return Some(format!("dates t v1\n\n{INPUTS}{b}"));
    }
    if first.starts_with("inputs") {
        return Some(format!("dates t v1\n\n{b}"));
    }
    None
}

#[test]
fn every_cal_in_design_is_read() {
    let mut n = 0;
    let mut failures = Vec::new();
    for b in design_blocks() {
        let Some(f) = as_file(&b) else { continue };
        n += 1;
        let p = parse("design.cal", &f);
        if !p.diags.is_empty() {
            let text: String = p.diags.iter().map(|d| d.render(ritsu_base::text::Lang::En)).collect();
            failures.push(format!("--- block\n{f}--- says\n{text}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // 1.1 (two), 1.3, 1.5 (two), 1.7, 1.10, 4.3 (two).
    assert!(n >= 9, "found {n} .cal blocks in DESIGN.md");
}

fn codes(src: &str) -> Vec<(&'static str, usize, usize)> {
    parse("t.cal", src).diags.iter().map(|d| (d.code, d.line.unwrap_or(0), d.col.unwrap_or(0))).collect()
}

#[test]
fn what_cannot_be_read() {
    // E001: an unclosed string, a character that is not part of the language, a broken pin,
    // a name that starts with digits in ASCII.
    assert_eq!(codes("dates t v1\ndescription \"open\n"), vec![("E001", 2, 13)]);
    assert_eq!(codes("calendar t v1\nclosed weekly sat; sun\n")[0].0, "E001");
    assert_eq!(codes("calendar t v1\nsource a = file \"a.csv\" sha256:XYZ\n  format csv\n  covers listed years\n")[0].0, "E001");
    assert_eq!(codes("dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-12-31\n\ndate x = d\n  +30days\n")[0].0, "E001");
    assert_eq!(codes("dates t v1\n\ninputs\n  d : date  range >=2026-1-1 <=2026-12-31\n")[0].0, "E001");
    // E002: a word where it does not belong.
    assert_eq!(codes("dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-12-31\n\ndate x = d\n  roll sideways\n"), vec![("E002", 7, 8)]);
    assert_eq!(codes("dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-12-31\n\ndate x = d\n  if closed if closed + 1 day\n")[0].0, "E002");
    // E003: the first line.
    assert_eq!(codes("# a comment\n\ninputs\n"), vec![("E003", 3, 1)]);
    assert_eq!(codes(""), vec![("E003", 1, 1)]);
    // E004: order, twice, and the wrong kind of file.
    assert_eq!(codes("dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-12-31\ndescription \"late\"\n")[0], ("E004", 5, 1));
    assert_eq!(codes("calendar t v1\noffset +09:00\noffset +09:00\n")[0], ("E004", 3, 1));
    assert_eq!(codes("dates t v1\noffset +09:00\n")[0].0, "E004");
    assert_eq!(codes("dates t v1\nsource s = file \"a.csv\"\n  format csv\n  covers listed years\n")[0].0, "E004");
    // E005: a tab, a line out of line, an indented line under nothing.
    assert_eq!(codes("dates t v1\n\ninputs\n\td : date  range >=2026-01-01 <=2026-12-31\n")[0].0, "E005");
    assert_eq!(codes("dates t v1\n\ninputs\n  a : date  range >=2026-01-01 <=2026-12-31\n   b : int  range >=1 <=2\n")[0], ("E005", 5, 4));
    assert_eq!(codes("dates t v1\n  description \"x\"\n")[0].0, "E005");
    // E006: no such date, month and day, time.
    assert_eq!(codes("dates t v1\n\ninputs\n  d : date  range >=2026-02-01 <=2026-02-30\n"), vec![("E006", 4, 34)]);
    assert_eq!(codes("calendar t v1\nclosed every 13-01 \"x\"\n")[0].0, "E006");
    assert_eq!(codes("dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-12-31\n\ndate x = d\n  at 25:00\n")[0].0, "E006");
}

#[test]
fn what_is_read() {
    // A name can start with digits when it goes on in Japanese.
    let p = parse("t.cal", "dates t v1\n\ninputs\n  d : date  range >=2026-01-01 <=2026-12-31\n\ndate x = d\n  + 40 business days\n\nclaims\n  40営業日以内 : x <= d + 40 business days\n");
    assert!(p.diags.is_empty());
    assert_eq!(p.file.unwrap().claims[0].name, "40営業日以内");
    // Every closure of every year, across the new year.
    let p = parse("t.cal", "calendar t v1\nclosed every 12-29..01-03 \"年末年始\"\nclosed every 02-29\n");
    assert!(p.diags.is_empty());
    // The time zone name is read as it is, for E107 to refuse.
    let p = parse("t.cal", "calendar t v1\noffset Asia/Tokyo\n");
    assert!(p.diags.is_empty());
    assert_eq!(p.file.unwrap().offset.unwrap().0, "Asia/Tokyo");
    // A comment after a string that has a `#` in it.
    let p = parse("t.cal", "calendar t v1\ndescription \"#1 の休み\" # 説明\n");
    assert!(p.diags.is_empty());
    assert_eq!(p.file.unwrap().description.unwrap(), "#1 の休み");
}

/// DESIGN 1.2's table of words and `src/kw.rs` say the same words, row by row.
#[test]
fn the_words_of_design_are_the_words_of_kw() {
    let text = std::fs::read_to_string("DESIGN.md").unwrap();
    let start = text.find("| 位置 | 語 |").unwrap();
    let table: Vec<&str> = text[start..].lines().skip(2).take_while(|l| l.starts_with('|')).collect();
    let words = |cell: &str| -> Vec<String> {
        let mut v = Vec::new();
        let mut rest = cell;
        while let Some(a) = rest.find('`') {
            let b = rest[a + 1..].find('`').unwrap() + a + 1;
            // `listed years` is two words.
            v.extend(rest[a + 1..b].split_whitespace().map(|w| w.to_string()));
            rest = &rest[b + 1..];
        }
        v
    };
    let rows: Vec<Vec<String>> = table.iter().map(|l| words(l.split('|').nth(2).unwrap())).collect();
    // The last row of DESIGN's table is the symbols, which are not words.
    assert_eq!(rows.len(), koyomi::kw::TABLE.len() + 1);
    for (row, (_, ws)) in rows.iter().zip(koyomi::kw::TABLE) {
        let mut a: Vec<&str> = row.iter().map(|s| s.as_str()).collect();
        let mut b: Vec<&str> = ws.to_vec();
        a.sort();
        b.sort();
        assert_eq!(a, b, "a row of DESIGN 1.2");
    }
    for (_, ws) in koyomi::kw::TABLE {
        for w in *ws {
            let w = w.trim_end_matches(':');
            if w.chars().all(|c| c.is_ascii_alphabetic() || c == '_' || c.is_ascii_digit()) {
                assert!(koyomi::kw::is_reserved(w), "{w} is reserved");
            }
        }
    }
}

/// The files DESIGN 1.1 and 4.3 show are the example files, byte for byte.
#[test]
fn the_listings_of_design_are_the_examples() {
    let blocks = design_blocks();
    let find = |head: &str| blocks.iter().find(|b| b.trim_start().starts_with(head)).cloned().unwrap_or_else(|| panic!("DESIGN shows {head}"));
    assert_eq!(find("calendar 東京の営業日(tokyo) v1"), std::fs::read_to_string("examples/calendars/東京の営業日.cal").unwrap());
    assert_eq!(find("dates 支払条件(payment_terms) v1"), std::fs::read_to_string("examples/支払_20日締め翌月10日払い.cal").unwrap());
    let numbered = blocks.iter().find(|b| b.contains("  dates 月末締め翌々月末払い(eom_two_months) v1")).unwrap();
    assert_eq!(unnumber(numbered), std::fs::read_to_string("examples/支払_月末締め翌々月末払い.cal").unwrap());
}
