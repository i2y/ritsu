//! The READMEs, the pages of docs/ and the agent skill show what the tool does. Every diagnostic
//! on them is word for word in a golden file of tests/fixtures, which tests/check.rs holds to the
//! checker; every `chobo` command shown with its output prints that output now; every line of
//! `.book` on them is a line of a book of the examples, the tests or the diagnostics' own
//! examples; every relative link leads to a file; the pages of codes are what `chobo explain
//! --all` prints; the counts they give are the counts; and the keywords on the reference are
//! src/syntax.rs's.

mod common;
use chobo::diag::Lang;
use common::*;
use std::collections::BTreeSet;

/// The pages, each with the name the messages give it.
fn pages() -> Vec<PathBuf> {
    let mut out = vec![root().join("README.md"), root().join("README.ja.md"), root().join("skills/README.md")];
    for dir in ["docs", "skills/chobo"] {
        let mut more: Vec<PathBuf> = std::fs::read_dir(root().join(dir)).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "md")).collect();
        more.sort();
        out.extend(more);
    }
    out
}

fn shown(f: &Path) -> String {
    f.strip_prefix(root()).unwrap().display().to_string()
}

/// The fenced blocks of a Markdown page: the word after the opening fence, and the lines.
fn fenced(text: &str) -> Vec<(String, Vec<String>)> {
    let mut blocks = Vec::new();
    let mut open: Option<(String, Vec<String>)> = None;
    for line in text.lines() {
        match (line.trim_start().strip_prefix("```"), open.take()) {
            (Some(_), Some(block)) => blocks.push(block),
            (Some(info), None) => open = Some((info.trim().to_string(), Vec::new())),
            (None, Some(mut block)) => {
                block.1.push(line.to_string());
                open = Some(block);
            }
            (None, None) => {}
        }
    }
    blocks
}

#[test]
fn the_diagnostics_on_the_pages_are_what_the_checker_prints() {
    let goldens: Vec<String> = std::fs::read_dir(root().join("tests/fixtures"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .map(|p| std::fs::read_to_string(p).unwrap())
        .collect();
    let heads = ["error[", "warning[", "エラー[", "警告["];
    let mut wrong = Vec::new();
    let mut seen = BTreeSet::new();
    for f in pages() {
        for (_, lines) in fenced(&std::fs::read_to_string(&f).unwrap()) {
            if !lines.first().is_some_and(|l| heads.iter().any(|h| l.starts_with(h))) {
                continue;
            }
            seen.insert(shown(&f));
            if !goldens.iter().any(|g| g.contains(&lines.join("\n"))) {
                wrong.push(format!("{}: {}", shown(&f), lines[0]));
            }
        }
    }
    for must in ["README.md", "README.ja.md", "skills/chobo/SKILL.md"] {
        assert!(seen.contains(must), "no diagnostic on {must}: were the fences changed?");
    }
    assert!(wrong.is_empty(), "not what any golden file in tests/fixtures holds:\n{}", wrong.join("\n"));
}

/// Every line of every book there is: the examples, the books of the tests, and the books of the
/// diagnostics' own examples (`chobo explain`).
fn real_lines() -> BTreeSet<String> {
    let mut books: Vec<PathBuf> = example_books();
    books.extend(books_in("tests/books"));
    let mut fixtures: Vec<PathBuf> = std::fs::read_dir(root().join("tests/fixtures")).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "book")).collect();
    fixtures.sort();
    books.extend(fixtures);
    let mut real: BTreeSet<String> = BTreeSet::new();
    for b in books {
        real.extend(std::fs::read_to_string(&b).unwrap().lines().map(|l| l.trim().to_string()));
    }
    for e in chobo::codes::ledger() {
        real.extend(e.example.lines().map(|l| l.trim().to_string()));
        if let Some(b) = e.before {
            real.extend(b.lines().map(|l| l.trim().to_string()));
        }
    }
    real
}

#[test]
fn the_book_on_the_pages_is_from_the_books() {
    let real = real_lines();
    // a line cut short with `…` stands for a real line that has its pieces in that order
    let found = |line: &str| {
        let pieces: Vec<&str> = line.split('…').map(str::trim).filter(|p| !p.is_empty()).collect();
        real.iter().any(|r| {
            let mut at = 0;
            pieces.iter().all(|p| match r[at..].find(p) {
                Some(i) => {
                    at += i + p.len();
                    true
                }
                None => false,
            })
        })
    };
    let mut wrong = Vec::new();
    let mut seen = BTreeSet::new();
    for f in pages() {
        for (info, lines) in fenced(&std::fs::read_to_string(&f).unwrap()) {
            if info != "book" {
                continue;
            }
            seen.insert(shown(&f));
            for l in lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty() && *l != "…") {
                if !found(l) {
                    wrong.push(format!("{}: {l}", shown(&f)));
                }
            }
        }
    }
    for must in ["README.md", "README.ja.md", "docs/reference.md", "skills/chobo/SKILL.md"] {
        assert!(seen.contains(must), "no book on {must}: were the fences changed?");
    }
    assert!(wrong.is_empty(), "not a line of any book:\n{}", wrong.join("\n"));
}

/// Whether `actual` shows what a page says it does: the lines of the page, in order, a line
/// that is only `…` standing for lines left out. Without `…` first, the output starts there;
/// without `…` last, it ends there.
fn shows(actual: &str, expected: &[String]) -> Result<(), String> {
    let act: Vec<&str> = actual.lines().map(|l| l.trim_end()).collect();
    let mut chunks: Vec<Vec<&str>> = vec![Vec::new()];
    for l in expected {
        if l.trim() == "…" {
            chunks.push(Vec::new());
        } else {
            chunks.last_mut().unwrap().push(l.trim_end());
        }
    }
    let open_end = chunks.last().is_some_and(|c| c.is_empty());
    let mut at = 0;
    for (ci, ch) in chunks.iter().enumerate() {
        if ch.is_empty() {
            continue;
        }
        let found = (at..=act.len().saturating_sub(ch.len())).find(|&i| act.len() >= i + ch.len() && act[i..i + ch.len()] == ch[..]);
        match found {
            Some(i) if ci == 0 && i != 0 => return Err(format!("the output does not start with {:?}", ch[0])),
            Some(i) => at = i + ch.len(),
            None => {
                let near = ch.iter().find(|l| !act.contains(l)).unwrap_or(&ch[0]);
                return Err(format!("not in the output, in this order: {near:?}"));
            }
        }
    }
    if !open_end && at != act.len() {
        return Err(format!("the output goes on after the last line shown: {:?}", act.get(at)));
    }
    Ok(())
}

/// The words of a command line, the shell's way for the simple ones the pages have.
fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            ' ' if !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[test]
fn every_command_on_the_pages_prints_what_they_show() {
    let work = TempDir::new("docs-commands");
    let mut wrong = Vec::new();
    let mut ran = 0;
    let mut compared = 0;
    for f in pages() {
        for (info, lines) in fenced(&std::fs::read_to_string(&f).unwrap()) {
            if info != "console" {
                continue;
            }
            // each `$ …` line, with the lines under it
            let mut cmds: Vec<(String, Vec<String>)> = Vec::new();
            for l in lines {
                match l.strip_prefix("$ ") {
                    Some(c) => cmds.push((c.split("  #").next().unwrap().trim().to_string(), Vec::new())),
                    None => match cmds.last_mut() {
                        Some(c) => c.1.push(l),
                        None => wrong.push(format!("{}: a console block that does not start with a command", shown(&f))),
                    },
                }
            }
            for (cmd, expected) in cmds {
                let w = words(&cmd);
                if w.first().map(String::as_str) != Some("chobo") {
                    if !expected.is_empty() {
                        wrong.push(format!("{}: `{cmd}` is shown with an output, which only a chobo command can be checked for", shown(&f)));
                    }
                    continue;
                }
                assert!(!cmd.contains('>') && !cmd.contains('|'), "{}: `{cmd}`: a command on a page is run as it is, without a redirection", shown(&f));
                let mut args: Vec<String> = w[1..].to_vec();
                // what a command writes goes to the scratch directory, not the repository
                if let Some(i) = args.iter().position(|a| a == "--out") {
                    args[i + 1] = work.path().join(format!("out-{ran}")).display().to_string();
                }
                let out = chobo().current_dir(root()).args(&args).output().unwrap();
                ran += 1;
                let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
                if expected.is_empty() {
                    if !out.status.success() {
                        wrong.push(format!("{}: `{cmd}` failed:\n{text}", shown(&f)));
                    }
                    continue;
                }
                compared += 1;
                if let Err(e) = shows(&String::from_utf8_lossy(&out.stdout), &expected) {
                    wrong.push(format!("{}: `{cmd}`: {e}", shown(&f)));
                }
            }
        }
    }
    assert!(compared >= 8, "only {compared} commands with an output on the pages");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    eprintln!("compared: {compared} commands on the pages print what they show, of {ran} run");
}

/// The targets of the Markdown links of a page, outside code.
fn links(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        let plain: String = line.split('`').step_by(2).collect::<Vec<_>>().join(" ");
        let mut rest = plain.as_str();
        while let Some(i) = rest.find("](") {
            let after = &rest[i + 2..];
            let Some(j) = after.find(')') else { break };
            out.push(after[..j].to_string());
            rest = &after[j + 1..];
        }
    }
    out
}

#[test]
fn every_relative_link_leads_to_a_file() {
    let mut wrong = Vec::new();
    let mut seen = 0;
    for f in pages() {
        for target in links(&std::fs::read_to_string(&f).unwrap()) {
            if target.starts_with("https://") || target.starts_with("http://") || target.starts_with('#') {
                continue;
            }
            seen += 1;
            let file = target.split('#').next().unwrap();
            if !f.parent().unwrap().join(file).exists() {
                wrong.push(format!("{}: {target}", shown(&f)));
            }
        }
    }
    assert!(seen > 20, "only {seen} relative links");
    assert!(wrong.is_empty(), "links that lead nowhere:\n{}", wrong.join("\n"));
}

#[test]
fn the_pages_of_codes_are_what_explain_prints() {
    for (page, lang) in [("docs/codes.md", Lang::En), ("docs/codes.ja.md", Lang::Ja)] {
        let text = std::fs::read_to_string(root().join(page)).unwrap();
        assert!(text == chobo::codes::markdown_all(lang), "{page} is not what `chobo explain --all --format markdown` prints; write it again with it");
    }
}

#[test]
fn the_counts_on_the_pages_are_the_counts() {
    let codes = chobo::codes::ledger().len();
    let (mut scenarios, mut together) = (0, 0);
    for p in books_in("tests/books").into_iter().chain(example_books()) {
        let (book, _) = chobo::model::load(&std::fs::read_to_string(&p).unwrap());
        let book = book.unwrap();
        let mut list = chobo::scenarios::generate(&book);
        if let Ok(text) = std::fs::read_to_string(p.with_file_name(format!("{}.more.json", stem(&p)))) {
            let more: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap();
            list.extend(more.iter().map(|v| chobo::scenario::from_json(&book, v).unwrap()));
        }
        scenarios += list.len();
        together += list.iter().filter(|s| chobo::scenario::has_together(s)).count();
    }
    for (page, says) in [
        ("README.md", format!("all {codes} codes")),
        ("README.md", format!("{scenarios} of them with the ones written by hand, {together} with callers at the same time")),
        ("README.ja.md", format!("診断のコードは全部で {codes} 種類")),
        ("README.ja.md", format!("手で書いたものを含めて {scenarios} 本、そのうち {together} 本は呼び出し元が同時に来るもの")),
        ("docs/reference.md", format!("all {codes} codes")),
    ] {
        let text = std::fs::read_to_string(root().join(page)).unwrap().split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(text.contains(&says), "{page} must say the count as it is: {says:?}");
    }
}

#[test]
fn the_reference_lists_the_keywords_of_the_language() {
    let text = std::fs::read_to_string(root().join("docs/reference.md")).unwrap();
    let table = text.split("| Where | Keywords |").nth(1).expect("the table of keywords on docs/reference.md");
    let mut listed = Vec::new();
    for row in table.lines().skip(2).take_while(|l| l.starts_with('|')) {
        let cells: Vec<&str> = row.split('|').collect();
        listed.extend(cells[2].split('`').skip(1).step_by(2).map(String::from));
    }
    let ours: Vec<String> = chobo::syntax::KEYWORDS.iter().map(|(k, _)| k.to_string()).collect();
    assert_eq!(listed, ours, "docs/reference.md must list src/syntax.rs's KEYWORDS, in their order");
}
