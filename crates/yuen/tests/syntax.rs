//! Words and lines (DESIGN 1, PLAN B.2): every `.req` DESIGN.md shows parses, and what is
//! wrong with words and lines is E001–E006.

use yuen::ast::*;
use yuen::parse::parse;

/// The fenced blocks of a part of DESIGN.md with no language after the fence.
fn blocks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    for l in text.lines() {
        if let Some(info) = l.strip_prefix("```") {
            match cur.take() {
                Some(b) => out.push(b),
                None if info.trim().is_empty() => cur = Some(String::new()),
                None => cur = Some("\u{0}".to_string()),
            }
            continue;
        }
        if let Some(b) = cur.as_mut() {
            b.push_str(l);
            b.push('\n');
        }
    }
    out.into_iter().filter(|b| !b.starts_with('\u{0}')).collect()
}

fn section<'a>(text: &'a str, from: &str, to: &str) -> &'a str {
    let a = text.find(from).unwrap();
    let b = a + text[a..].find(to).unwrap();
    &text[a..b]
}

#[test]
fn every_req_design_shows_parses() {
    let design = std::fs::read_to_string("DESIGN.md").unwrap();
    let mut parts = blocks(section(&design, "## 1. 言語", "## 2. 成果物の名指し方"));
    parts.extend(blocks(section(&design, "### 4.2 確かめた記録", "### 4.3 印の付け方")));
    let mut n = 0;
    let mut failures = Vec::new();
    for b in parts {
        // A whole file as it is; a part of one under a heading, and the lines of a
        // requirement under a requirement.
        let src = if b.starts_with("requirements ") {
            b.clone()
        } else if b.starts_with("  ") {
            format!("requirements 試し v1\n\nrequirement 試し(trial)\n{b}")
        } else {
            format!("requirements 試し v1\n\n{b}")
        };
        let p = parse("DESIGN.md", "DESIGN.md", &src);
        if p.file.is_none() || !p.diags.is_empty() {
            let shown: String = p.diags.iter().map(|d| d.render(yuen::i18n::Lang::En)).collect();
            failures.push(format!("{src}\n{shown}"));
        }
        n += 1;
    }
    assert!(n >= 8, "{n} blocks of .req in DESIGN.md");
    assert!(failures.is_empty(), "{}", failures.join("\n---\n"));
}

#[test]
fn the_file_of_design_1_1_reads_as_it_says() {
    let design = std::fs::read_to_string("DESIGN.md").unwrap();
    let b = blocks(section(&design, "### 1.1 ファイルの形", "### 1.2")).remove(0);
    let f = parse("x.req", "x.req", &b).file.unwrap();
    assert_eq!((f.header.name.as_str(), f.header.version), ("民法の期間", 1));
    assert_eq!(f.roles.len(), 2);
    assert!(matches!(&f.sources[0].kind, SourceKind::Borrowed { .. }));
    assert_eq!(f.scopes.len(), 1);
    assert_eq!(f.requirements.len(), 3);
    let r = &f.requirements[1];
    assert_eq!(r.name, "満了日");
    assert_eq!(r.alias.as_ref().unwrap().0, "last_day");
    let FromWhat::Cite { source, fragments, .. } = &r.from[0].what else { panic!() };
    assert_eq!(source, "民法");
    assert_eq!(fragments.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["第141条", "第143条"]);
    let rec = r.from[0].record.as_ref().unwrap().parsed.as_ref().unwrap();
    assert_eq!(rec.up, ["0575c131b9f08063", "6950bdfb988439b6"]);
    assert_eq!(rec.down.as_deref(), Some("465b83ed8c251406"));
    assert_eq!(r.waivers.len(), 1);
    assert_eq!(r.waivers[0].side, Side::Verified);
    let r = &f.requirements[2];
    assert_eq!(r.decided.len(), 1);
    assert_eq!(r.links.len(), 2);
    assert_eq!(r.links[1].naming.rest.len(), 2);
}

fn code_of(src: &str) -> Vec<&'static str> {
    parse("t.req", "t.req", src).diags.iter().map(|d| d.code).collect()
}

const HEAD: &str = "requirements 試し v1\nrole 経理\n\n";

#[test]
fn what_is_wrong_with_words_and_lines() {
    let cases: &[(&str, &str)] = &[
        // E001: words.
        ("requirement r1\n  text \"open\n", "E001"),
        ("requirement r1\n  text \"a\\nb\"\n", "E001"),
        ("requirement r1(Pay)\n", "E001"),
        ("requirement 30days\n", "E001"),
        ("requirement r1\n  in force 2026-1-1..\n", "E001"),
        ("requirement r1 v01\n", "E001"),
        ("requirement r1\n  from @民法 第1条\n    reviewed 2026-10-03 by 経理 sha256:ABC -> sha256:abc\n", "E001"),
        // E002: a word where it does not belong.
        ("requirement r1\n  because \"x\"\n", "E002"),
        ("role text\n", "E002"),
        ("requirement r1\n  owner\n", "E002"),
        ("source 民法 = law \"x\"\n", "E002"),
        ("requirement r1\n  in force ..\n", "E002"),
        ("requirement r1\n  satisfied by\n", "E002"),
        ("hello\n", "E002"),
        // E004: order and count.
        ("requirement r1\n  owner 経理\n  text \"x\"\n", "E004"),
        ("requirement r1\n  text \"x\"\n  text \"y\"\n", "E004"),
        ("requirement r1\n  satisfied by file \"a\"\n    reviewed 2026-10-03 by 経理 sha256:0000000000000000 -> sha256:0000000000000000\n    reviewed 2026-10-03 by 経理 sha256:0000000000000000 -> sha256:0000000000000000\n", "E004"),
        ("description \"x\"\n", "E004"),
        // E005: indentation.
        ("requirement r1\n\ttext \"x\"\n", "E005"),
        ("requirement r1\n  text \"x\"\n   owner 経理\n", "E005"),
        ("  text \"x\"\n", "E005"),
        ("requirement r1\n  text \"x\"\n    reviewed 2026-10-03 by 経理 sha256:0000000000000000 -> sha256:0000000000000000\n", "E005"),
        // E006: dates.
        ("requirement r1\n  in force 2026-02-30..\n", "E006"),
        ("requirement r1\n  in force 2027-01-01..2026-01-01\n", "E006"),
    ];
    for (body, code) in cases {
        let src = format!("{HEAD}{body}");
        let got = code_of(&src);
        assert_eq!(got.first(), Some(code), "{src:?} gave {got:?}");
    }
    assert_eq!(code_of("role 経理\n"), ["E003"]);
    assert_eq!(code_of(""), ["E003"]);
    assert_eq!(code_of("requirements 試し\n"), ["E002"]);
}

#[test]
fn a_broken_record_is_said_later_and_does_not_stop_the_words() {
    // The parser keeps what is wrong with a record for stage 6 (E305): the file parses.
    let src = format!("{HEAD}requirement r1\n  text \"x\"\n  satisfied by file \"a\"\n    reviewed 2026-10-03 by 経理 sha256:0000000000000000\n  not verified \"y\"\n    reviewed 2026-10-03 by 経理 sha256:0000000000000000 -> sha256:0000000000000000\n");
    let p = parse("t.req", "t.req", &src);
    assert!(p.diags.is_empty());
    let r = &p.file.unwrap().requirements[0];
    assert!(r.links[0].record.as_ref().unwrap().parsed.is_err());
    assert!(r.waivers[0].record.as_ref().unwrap().parsed.is_err(), "a waiver's record is `approved`");
}

#[test]
fn periods_aliases_versions_and_comments() {
    let src = format!(
        "{HEAD}requirement 支払日(payment_day) v2  # the second\n  text \"20 日締め # 翌月 10 日払い\"\n  in force ..2027-03-31\n  owner 経理\n  replaces 古い支払日 v3\n  from 方針\n"
    );
    let f = parse("t.req", "t.req", &src).file.unwrap();
    let r = &f.requirements[0];
    assert_eq!(r.version.unwrap().0, 2);
    assert_eq!(r.text.as_ref().unwrap().0, "20 日締め # 翌月 10 日払い");
    let p = r.in_force.unwrap().0;
    assert_eq!((p.from, p.to.map(|d| d.to_string())), (None, Some("2027-03-31".into())));
    assert_eq!(r.replaces[0].version, Some(3));
    assert!(matches!(&r.from[0].what, FromWhat::Req(rr) if rr.name == "方針" && rr.version.is_none()));
}

#[test]
fn crlf_reads_as_lf() {
    let src = format!("{HEAD}requirement r1\n  text \"x\"\n  owner 経理\n").replace('\n', "\r\n");
    let p = parse("t.req", "t.req", &src);
    assert!(p.diags.is_empty());
    assert_eq!(p.file.unwrap().requirements[0].text.as_ref().unwrap().0, "x");
}
