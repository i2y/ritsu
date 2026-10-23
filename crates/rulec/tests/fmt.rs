//! `rulec fmt` writes in ASCII what an IME types, and changes no word the lexer reads (§1.5,
//! §15.257). A name keeps every character it was written with: `松＋竹` and `第１案` are names,
//! and the lexer reads the `＋` and the `１` as their characters. A symbol is written in ASCII only
//! where the lexer reads it as a symbol of its own, and a full-width digit only where the lexer
//! reads no word (where a word would start, which stops it at E002).
//!
//! What is held is what the lexer reads, line by line: every `.rule` of the tree, formatted, reads
//! as it read before and keeps its comments as they are written (where a comment starts is the
//! lexer's to say, §15.267); the same file typed with an IME formats to the words of the file; and
//! the materials in `tests/fmt` — English first, the Japanese versions beside them with full-width
//! characters in their names — format to files that pass `check`.

use rulec::lex::{self, Kind};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// What the lexer reads of a line, a symbol written in full width the same as in ASCII (the lexer
/// reads them as one token). The `->` of the second and later output columns of a table's head is
/// left out: the parser reads the marks and their absence as one rule, and fmt folds them (§5.1).
fn line_words(line: &str) -> Vec<Option<Kind>> {
    let ks: Vec<Option<Kind>> = lex::read_through(line).into_iter().map(|(_, k)| k).collect();
    let row = matches!(ks.first(), Some(Some(Kind::Pipe))) || matches!(ks.get(1), Some(Some(Kind::Pipe)));
    let mut arrows = 0;
    ks.into_iter()
        .filter(|k| {
            if row && matches!(k, Some(Kind::Arrow)) {
                arrows += 1;
                return arrows == 1;
            }
            true
        })
        .collect()
}

/// Whether the lexer reads the whole line: no character it cannot read, no string without an end.
fn read_whole(line: &str) -> bool {
    lex::read_through(line).iter().all(|(_, k)| k.is_some())
}

/// The names the lexer reads on a line, in order.
fn names(line: &str) -> Vec<String> {
    lex::read_through(line)
        .into_iter()
        .filter_map(|(_, k)| match k {
            Some(Kind::Ident(n)) => Some(n),
            _ => None,
        })
        .collect()
}

/// The names on a line the lexer cannot read whole, but for one right after a character it cannot
/// read: the lexer stops at that character, and a full-width digit there, written in ASCII, starts
/// a number the word after it may be the unit of (`１０万円` is `10万円`).
fn names_read(line: &str) -> Vec<String> {
    let pieces = lex::read_through(line);
    pieces
        .iter()
        .enumerate()
        .filter_map(|(i, (r, k))| match k {
            Some(Kind::Ident(n)) if !(i > 0 && pieces[i - 1].1.is_none() && pieces[i - 1].0.end == r.start) => Some(n.clone()),
            _ => None,
        })
        .collect()
}

/// The comment of a line as the lexer reads it: from the first `#` after the last word it reads, to
/// the end of the line (§15.267).
fn comment(line: &str) -> Option<&str> {
    let end = lex::read_through(line).last().map_or(0, |(r, _)| r.end);
    line[end..].find('#').map(|h| line[end + h..].trim_end())
}

/// Whether `part` stands in `whole` in the same order.
fn in_order(part: &[String], whole: &[String]) -> bool {
    let mut it = whole.iter();
    part.iter().all(|p| it.any(|w| w == p))
}

/// The symbols a text still writes in full width where the lexer reads them as symbols, and the
/// ideographic spaces between its words.
fn left_in_full_width(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let pieces = lex::read_through(line);
        let mut end = 0;
        for (r, k) in &pieces {
            if line[end..r.start].contains('　') {
                out.push(format!("an ideographic space in `{line}`"));
            }
            let w = &line[r.clone()];
            if k.as_ref().is_some_and(|k| matches!(k, Kind::Le | Kind::Ge | Kind::Lt | Kind::Gt | Kind::Plus | Kind::Arrow | Kind::Comma)) && !w.is_ascii() {
                out.push(format!("{w} in `{line}`"));
            }
            end = r.end;
        }
    }
    out
}

/// Every `.rule` of the workspace: the crates' tests, examples and experiments, and the site's.
fn tree() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root().join("../..")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let (p, name) = (e.path(), e.file_name().to_string_lossy().into_owned());
            if p.is_dir() {
                if !name.starts_with('.') && !matches!(name.as_str(), "target" | "node_modules" | "site") {
                    stack.push(p);
                }
            } else if name.ends_with(".rule") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Where the lines of `got` read otherwise than the lines of `want`: a line the lexer reads whole
/// is read the same, and on any other line, the names it reads are still names, in order; and every
/// line keeps its comment as it is written.
fn differences(file: &str, want: &str, got: &str) -> Vec<String> {
    let (w, g): (Vec<&str>, Vec<&str>) = (want.lines().collect(), got.lines().collect());
    if w.len() != g.len() {
        return vec![format!("{file}: {} lines became {}", w.len(), g.len())];
    }
    w.iter()
        .zip(&g)
        .enumerate()
        .filter(|(_, (a, b))| comment(a) != comment(b) || if read_whole(a) { line_words(a) != line_words(b) } else { !in_order(&names_read(a), &names(b)) })
        .map(|(i, (a, b))| format!("{file}:{}: `{a}` became `{b}`", i + 1))
        .collect()
}

#[test]
fn every_rule_of_the_tree_reads_the_same_formatted() {
    let files = tree();
    assert!(files.len() > 600, "the walk found {} files", files.len());
    let mut wrong = Vec::new();
    for f in &files {
        let src = std::fs::read_to_string(f).unwrap();
        let shown = f.strip_prefix(root().join("../..")).unwrap_or(f).display().to_string();
        wrong.extend(differences(&shown, &src, &rulec::fmt::format(&src)));
    }
    assert!(wrong.is_empty(), "fmt changed what the lexer reads:\n{}", wrong.join("\n"));
}

/// The ASCII symbols the lexer reads as tokens of their own, as an IME types them.
fn full_width(written: &str, kind: &Kind) -> Option<&'static str> {
    Some(match (written, kind) {
        ("<=", Kind::Le) => "≦",
        (">=", Kind::Ge) => "≧",
        ("<", Kind::Lt) => "＜",
        (">", Kind::Gt) => "＞",
        ("+", Kind::Plus) => "＋",
        ("->", Kind::Arrow) => "→",
        (",", Kind::Comma) => "、",
        _ => return None,
    })
}

/// A line as it is typed with an IME left on: each symbol in full width where the lexer reads the
/// line the same with it (a `+` against a name is not: `a＋b` is one name), and then the digits of
/// every number and date, which the lexer cannot read at all.
fn typed_with_an_ime(line: &str) -> String {
    if !read_whole(line) {
        return line.to_string();
    }
    let read: Vec<Option<Kind>> = lex::read_through(line).into_iter().map(|(_, k)| k).collect();
    let mut text = line.to_string();
    // from the end, so that the ranges before stay where they are
    for (r, k) in lex::read_through(line).into_iter().rev() {
        let Some(fw) = k.as_ref().and_then(|k| full_width(&line[r.clone()], k)) else { continue };
        let mut tried = text.clone();
        tried.replace_range(r, fw);
        if lex::read_through(&tried).into_iter().map(|(_, k)| k).collect::<Vec<_>>() == read {
            text = tried;
        }
    }
    let mut out = String::with_capacity(text.len());
    let digits: Vec<std::ops::Range<usize>> =
        lex::read_through(&text).into_iter().filter(|(_, k)| matches!(k, Some(Kind::Num(_) | Kind::Date(..)))).map(|(r, _)| r).collect();
    for (i, c) in text.char_indices() {
        match c {
            '0'..='9' if digits.iter().any(|r| r.contains(&i)) => out.push(char::from_u32('０' as u32 + (c as u32 - '0' as u32)).unwrap()),
            c => out.push(c),
        }
    }
    out
}

#[test]
fn every_rule_typed_with_an_ime_formats_to_its_own_words() {
    let mut wrong = Vec::new();
    let mut typed = 0;
    for f in tree() {
        let src = std::fs::read_to_string(&f).unwrap();
        let ime: String = src.lines().map(|l| format!("{}\n", typed_with_an_ime(l))).collect();
        if ime != src {
            typed += 1;
        }
        let shown = f.strip_prefix(root().join("../..")).unwrap_or(&f).display().to_string();
        let got = rulec::fmt::format(&ime);
        // what the lexer read of the file as written, which the IME's spelling holds line by line
        let (w, g): (Vec<&str>, Vec<&str>) = (src.lines().collect(), got.lines().collect());
        assert_eq!(w.len(), g.len(), "{shown}");
        for (i, (a, b)) in w.iter().zip(&g).enumerate() {
            if read_whole(a) && line_words(a) != line_words(b) {
                wrong.push(format!("{shown}:{}: `{a}` typed and formatted is `{b}`", i + 1));
            }
        }
    }
    assert!(typed > 600, "only {typed} files had a symbol or a number to type");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

fn material(name: &str) -> String {
    std::fs::read_to_string(root().join("tests/fmt").join(name)).unwrap()
}

/// Every word the lexer read as written stays a word of the same kind, every name as written, and
/// the formatted file passes check, as the file typed in ASCII would.
fn formats_and_passes(name: &str) -> String {
    let src = material(name);
    let got = rulec::fmt::format(&src);
    let d = differences(name, &src, &got);
    assert!(d.is_empty(), "{}", d.join("\n"));
    assert_eq!(rulec::fmt::format(&got), got, "{name}: fmt is not idempotent");
    let diags = rulec::check_source(&got, name);
    assert!(!rulec::has_error(&diags), "{name} formatted does not pass check:\n{got}\n{diags:?}");
    got
}

/// The symbols typed with an IME, the digits in ASCII: the file passes check as it is written, and
/// formatted it reads the same words, in ASCII. The Japanese version's names hold a `＋` and a
/// full-width digit, which are the names' characters.
#[test]
fn symbols_typed_with_an_ime_are_written_in_ascii() {
    for name in ["symbols_typed_with_an_ime.rule", "IMEで打った記号.rule"] {
        let src = material(name);
        assert!(!rulec::has_error(&rulec::check_source(&src, name)), "{name} as written");
        assert!(src.lines().all(read_whole), "{name}: the lexer reads every line");
        let got = formats_and_passes(name);
        let read = |t: &str| t.lines().map(line_words).collect::<Vec<_>>();
        assert_eq!(read(&src), read(&got), "{name}");
        assert!(!left_in_full_width(&src).is_empty(), "{name} is typed with an IME");
        let left = left_in_full_width(&got);
        assert!(left.is_empty(), "{name}: {}", left.join("\n"));
        assert!(got.contains("| -> chosen : plan |") || got.contains("| -> 選んだ案 : 案 |"), "{got}");
        assert!(got.contains("<=1000USD") || got.contains("<=10万円"), "{got}");
    }
    let ja = formats_and_passes("IMEで打った記号.rule");
    assert!(ja.contains("enum 案(plan) = 第１案(first_plan) | 第２案(second_plan) | 松＋竹(combined_plan)"), "{ja}");
    assert!(ja.contains("| 東, 西 | -       | >10  | 松＋竹           |"), "{ja}");
    assert!(ja.contains("= 金額 + 1000円"), "the plus between two words is the operator: {ja}");
}

/// The digits typed in full width too: the lexer cannot read a number so written (E002), and fmt
/// writes it in ASCII. A digit inside a name is the name's (`第１案`), and stays.
#[test]
fn digits_typed_with_an_ime_are_written_in_ascii_outside_a_name() {
    for name in ["digits_typed_with_an_ime.rule", "IMEで打った数字.rule"] {
        let src = material(name);
        let as_written = rulec::check_source(&src, name);
        assert!(as_written.iter().any(|d| d.code == "E002"), "{name}: a full-width number is not read");
        let got = formats_and_passes(name);
        assert!(got.contains("range >=1 <=100"), "{got}");
        let left = left_in_full_width(&got);
        assert!(left.is_empty(), "{name}: {}", left.join("\n"));
    }
    let ja = formats_and_passes("IMEで打った数字.rule");
    assert!(ja.contains("enum 案(plan) = 第１案(first_plan) | 第２案(second_plan) | 松＋竹(combined_plan)"), "{ja}");
    assert!(ja.contains("range >=0円 <=10万円"), "{ja}");
    assert!(ja.contains("| 北   | 5000円 | 1    | 第１案      |"), "{ja}");
}

/// A symbol whose ASCII spelling would join its neighbour into another token is set apart from it
/// by a space, and what the lexer reads as one word is not cut: a `＋` in a name, and a `，` in a
/// number written with a thousands separator (E049, which says how to write it).
#[test]
fn a_symbol_is_written_in_ascii_only_as_the_same_word() {
    let cases = [
        // the two words the lexer reads stay two
        ("| a -＞ b |", "| a - > b |"),
        ("| ＜=1 |", "| < =1 |"),
        ("| ＞=1 |", "| > =1 |"),
        ("| -≧1 |", "| - >=1 |"),
        // one word stays one
        ("| 松＋竹 |", "| 松＋竹 |"),
        ("| a＋b |", "| a＋b |"),
        ("| 1，000 |", "| 1，000 |"),
        ("| 第１案 |", "| 第１案 |"),
        // and a symbol between two words is written in ASCII
        ("| a ＋ b |", "| a + b |"),
        ("| ≦１０ |", "| <=10 |"),
        ("| 1 ，000 |", "| 1, 000 |"),
        ("| 東、西 |", "| 東, 西 |"),
    ];
    for (written, want) in cases {
        let got = rulec::fmt::format(written);
        assert_eq!(got.trim_end(), want, "{written}");
        let d = differences(written, written, &got);
        assert!(d.is_empty(), "{}", d.join("\n"));
    }
}

/// A string and a comment are left as they are written, a `|` inside a string included.
#[test]
fn a_string_and_a_comment_are_left_alone() {
    let src = "| starts_with \"a|b\" | ≦１ |  # 行1→行2 ・ そのまま\n";
    let got = rulec::fmt::format(src);
    assert_eq!(got, "| starts_with \"a|b\" | <=1 |  # 行1→行2 ・ そのまま\n");
}

/// The material with the padding of its rows squeezed out: one blank before each bar the lexer
/// reads after the first, as a table typed without lining it up.
fn squeezed(src: &str) -> String {
    let mut out = String::new();
    for line in src.lines() {
        let mut end = 0;
        for (r, k) in lex::read_through(line) {
            let gap = &line[end..r.start];
            out.push_str(if end > 0 && matches!(k, Some(Kind::Pipe)) && !gap.is_empty() { " " } else { gap });
            out.push_str(&line[r.clone()]);
            end = r.end;
        }
        out.push_str(&line[end..]);
        out.push('\n');
    }
    out
}

/// Where a row ends and its comment starts is the lexer's to say (§1.5, §15.267): a comment stays as
/// it is written, the `|` in it included, at the end of a table's head, of a row with a label and of a
/// row of `examples`; and so does a line under `machine`, its comment and its strings, a `#` and a
/// blank before a comma in them included. The materials are written formatted: their rows typed
/// without lining them up format to them, and they pass check as they do.
#[test]
fn a_comment_and_a_string_stay_as_written() {
    for name in ["comments_and_strings_kept_as_written.rule", "書いたまま残るコメントと文字列.rule"] {
        let src = material(name);
        assert!(src.lines().any(|l| comment(l).is_some_and(|c| c.contains('|'))), "{name} has a comment with a bar");
        assert_eq!(formats_and_passes(name), src, "{name} is written formatted");
        let typed = squeezed(&src);
        assert_ne!(typed, src, "{name}: its rows are lined up");
        let d = differences(name, &src, &typed);
        assert!(d.is_empty(), "{}", d.join("\n"));
        assert_eq!(rulec::fmt::format(&typed), src, "{name} typed without lining up the rows");
        let codes = |t: &str| rulec::check_source(t, name).iter().map(|d| d.code).collect::<Vec<_>>();
        assert_eq!(codes(&typed), codes(&src), "{name}: check answers the same");
    }
}

/// A `|` the lexer reads in a comment or a string cuts no cell and makes no row, an `@` in a string
/// starts no citation, and a `#` in a string starts no comment (§15.267). Each line reads the words it
/// read and keeps its comment, and formatted again it stays.
#[test]
fn a_row_is_cut_where_the_lexer_cuts_it() {
    let cases = [
        ("| a |  # x|y", "| a |  # x|y"),
        ("| a | -> b : bool |  # old|-> c", "| a | -> b : bool |  # old|-> c"),
        ("r1 | a |  # x|y", "r1 | a |  # x|y"),
        ("| \"a|b\" |  # c|d", "| \"a|b\" |  # c|d"),
        ("| a |  # it's \"x|y", "| a |  # it's \"x|y"),
        ("| a |  @act 第20条  # x|y", "| a |  @act 第20条  # x|y"),
        ("| a |  @osha \"x|y\"", "| a |  @osha \"x|y\""),
        ("| a |  @osha \"x#y\"", "| a |  @osha \"x#y\""),
        // the rest of a row after a `#` is its comment (§1.5)
        ("| a | # b | c |", "| a |  # b | c |"),
        // no row: the bar is in a comment or in a string
        ("examples#x|y", "examples#x|y"),
        ("description\"a|b\"", "description\"a|b\""),
        ("r1#x | a |", "r1#x | a |"),
        // a row whose label is a string, with the bar in it
        ("\"a|b\" | c |", "\"a|b\" | c |"),
    ];
    for (written, want) in cases {
        let got = rulec::fmt::format(written);
        assert_eq!(got.trim_end(), want, "{written}");
        let d = differences(written, written, &got);
        assert!(d.is_empty(), "{}", d.join("\n"));
        assert_eq!(rulec::fmt::format(&got), got, "{written}: fmt is not idempotent");
    }
    // a cell between the last bar and a citation stays, and the row is closed as one without a
    // citation is
    let got = rulec::fmt::format("| a | b @x");
    assert_eq!(got, "| a | b |  @x\n");
    assert_eq!(names(&got), ["a", "b", "x"]);
}

/// Under `machine`, the comment starts where the lexer reads it, and a comma goes against the word
/// before it where the lexer reads a comma, when the line reads the same words so (§15.267): a `#`
/// and a blank before a comma in a string stay, and so does `1 ,000`, which against the `1` would be
/// one number written with a separator (E049).
#[test]
fn a_line_under_machine_keeps_its_strings() {
    for (written, want) in [
        ("once note starts_with \"paid #\"  # once|twice", "once    note starts_with \"paid #\"  # once|twice"),
        ("once note starts_with \"paid , at\"", "once    note starts_with \"paid , at\""),
        ("final 配達済 , 取消", "final   配達済, 取消"),
        ("always x 1 ,000", "always  x 1 ,000"),
    ] {
        let src = format!("machine m over t\n  {written}\n");
        let got = rulec::fmt::format(&src);
        assert_eq!(got, format!("machine m over t\n  {want}\n"), "{written}");
        let d = differences(written, &src, &got);
        assert!(d.is_empty(), "{}", d.join("\n"));
        assert_eq!(rulec::fmt::format(&got), got, "{written}: fmt is not idempotent");
    }
}
