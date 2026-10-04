//! The READMEs, the examples' READMEs and the agent skill show what geas does, and these tests
//! hold them to it:
//!
//! - every claims file on them (a block fenced `geas`) parses; one that starts as a claims file
//!   of the examples is that file as it is, and one with `…` lines standing for lines left out is
//!   an excerpt of a claims file in the repository;
//! - every console block that shows what a command printed, and every text block that starts the
//!   way geas's output does, is lines of a golden file as they are, `$ ` lines included, with `…`
//!   standing for lines left out (tests/readme.rs records the stories the READMEs tell);
//! - every code they name is one geas has, and the count of codes they give is the count;
//! - every geas command and option they name is in `geas --help`;
//! - every relative link leads to a file.

mod common;
use common::*;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The pages: the READMEs, the skill's, and each example's.
fn pages() -> Vec<PathBuf> {
    let mut out = vec![root().join("README.md"), root().join("README.ja.md"), root().join("skills/README.md")];
    out.extend(files_in(&root().join("../../skills/geas"), "md"));
    for dir in files_in(&root().join("examples"), "") {
        if dir.join("README.md").is_file() {
            out.push(dir.join("README.md"));
        }
    }
    out
}

/// The entries of a directory with that extension (any, for ""), sorted.
fn files_in(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|e| e.expect("an entry").path())
        .filter(|p| ext.is_empty() || p.extension().is_some_and(|x| x == ext))
        .collect();
    out.sort();
    out
}

fn shown(p: &Path) -> String {
    p.strip_prefix(root()).unwrap_or(p).display().to_string()
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
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
    assert!(open.is_none(), "a fence is left open");
    blocks
}

/// Whether `want` is lines of `have`, in order: each run of lines between `…` lines is a run of
/// lines of `have` as they are, and each run comes after the one before. Trailing blanks are not
/// compared.
fn excerpt_of(want: &[String], have: &str) -> bool {
    let have: Vec<&str> = have.lines().map(str::trim_end).collect();
    let mut runs: Vec<Vec<&str>> = vec![Vec::new()];
    for l in want {
        if l.trim() == "…" {
            runs.push(Vec::new());
        } else {
            runs.last_mut().expect("a run").push(l.trim_end());
        }
    }
    let mut at = 0;
    for run in runs.iter().filter(|r| !r.is_empty()) {
        match (at..=have.len().saturating_sub(run.len())).find(|&i| have.len() >= i + run.len() && have[i..i + run.len()] == run[..]) {
            Some(i) => at = i + run.len(),
            None => return false,
        }
    }
    true
}

/// Every file under a directory, recursively, with that extension.
fn walk(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    for p in files_in(dir, "") {
        if p.is_dir() {
            walk(&p, ext, out);
        } else if p.extension().is_some_and(|x| x == ext) {
            out.push(p);
        }
    }
}

#[test]
fn every_claims_file_on_the_pages_parses_or_is_an_excerpt() {
    let mut specs = Vec::new();
    walk(&root().join("examples"), "geas", &mut specs);
    let examples: Vec<String> = specs.iter().map(|p| read(p)).collect();
    walk(&root().join("tests"), "geas", &mut specs);
    let specs: Vec<String> = specs.iter().map(|p| read(p)).collect();
    let s = Scratch::new("docs-geas");
    let mut wrong = Vec::new();
    let (mut parsed, mut excerpts) = (0, 0);
    for page in pages() {
        for (n, (info, lines)) in fenced(&read(&page)).into_iter().enumerate() {
            if info != "geas" {
                continue;
            }
            if lines.iter().any(|l| l.trim() == "…") {
                excerpts += 1;
                if !specs.iter().any(|spec| excerpt_of(&lines, spec)) {
                    wrong.push(format!("{}: block {n} is not an excerpt of any claims file in the repository", shown(&page)));
                }
                continue;
            }
            // a whole claims file of the examples, quoted, is the file as it is now
            let text = lines.join("\n") + "\n";
            if let Some(example) = examples.iter().find(|e| e.lines().next() == lines.first().map(String::as_str)) {
                if *example != text {
                    wrong.push(format!("{}: block {n} starts as a claims file of the examples, and is not that file", shown(&page)));
                }
            }
            // drift reads the spec, then stops at E050, before running anything, when there is
            // no baseline: a spec that gives E050 and nothing else parses
            parsed += 1;
            let file = format!("b{parsed}.geas");
            s.write(&file, &text);
            let (out, _, code) = run(s.path(), &["drift", "--json", &file], &[]);
            let codes: Vec<String> =
                json(&out).get("diagnostics").arr().iter().map(|d| d.get("code").str().to_string()).collect();
            if code != 2 || codes != ["E050"] {
                wrong.push(format!("{}: block {n} does not parse: {codes:?}", shown(&page)));
            }
        }
    }
    assert!(parsed >= 10 && excerpts >= 2, "only {parsed} claims files and {excerpts} excerpts on the pages: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    eprintln!("compared: {parsed} claims files on the pages parse, and {excerpts} excerpts are from the repository's");
}

/// The first words of what geas prints, which make a text block an output to hold to a golden.
const OUTPUT_STARTS: &[&str] = &[
    "ok ", "not ok ", "error[", "warning[", "エラー[", "警告[", "claim \"", "主張 \"", "drift:", "ドリフト:", "diff:",
    "差分:", "map:", "記録:", "= ", "the screen:", "画面:", "{\"", "$ ",
];

fn goldens() -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    walk(&root().join("tests/golden"), "txt", &mut files);
    walk(&root().join("tests/golden"), "jsonl", &mut files);
    walk(&root().join("tests/golden"), "json", &mut files);
    files.into_iter().map(|p| {
        let text = read(&p);
        (p, text)
    }).collect()
}

#[test]
fn every_output_on_the_pages_is_in_a_golden_file() {
    let goldens = goldens();
    let mut wrong = Vec::new();
    let mut held = 0;
    let mut seen = BTreeSet::new();
    for page in pages() {
        for (info, lines) in fenced(&read(&page)) {
            let first = lines.iter().map(|l| l.trim()).find(|l| !l.is_empty()).unwrap_or("");
            let output = match info.as_str() {
                // a console block with a line that is not a command shows what one printed
                "console" => lines.iter().any(|l| !l.starts_with("$ ") && !l.trim().is_empty()),
                "text" => OUTPUT_STARTS.iter().any(|s| first.starts_with(s)),
                _ => false,
            };
            if !output {
                continue;
            }
            held += 1;
            seen.insert(shown(&page));
            if !goldens.iter().any(|(_, g)| excerpt_of(&lines, g)) {
                wrong.push(format!("{}: not lines of any golden file:\n{}", shown(&page), lines.join("\n")));
            }
        }
    }
    for must in ["README.md", "README.ja.md", "../../skills/geas/SKILL.md", "../../skills/geas/codes.md"] {
        assert!(seen.contains(must), "no output on {must}: were the fences changed?");
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
    eprintln!("compared: {held} outputs on the pages are lines of golden files");
}

/// The codes geas has, as `geas explain --all --json` lists them.
fn codes() -> Vec<String> {
    let s = Scratch::new("docs-codes");
    let (out, _, code) = run(s.path(), &["explain", "--all", "--json"], &[]);
    assert_eq!(code, 0);
    out.lines().map(|l| json(l).get("code").str().to_string()).collect()
}

#[test]
fn every_code_on_the_pages_is_one_geas_has() {
    let codes = codes();
    let mut wrong = BTreeSet::new();
    for page in pages() {
        let text = read(&page);
        let chars: Vec<char> = text.chars().collect();
        for i in 0..chars.len().saturating_sub(3) {
            let word_start = i == 0 || !chars[i - 1].is_ascii_alphanumeric();
            if word_start && (chars[i] == 'E' || chars[i] == 'W') && chars[i + 1..i + 4].iter().all(|c| c.is_ascii_digit()) {
                let after = chars.get(i + 4).copied().unwrap_or(' ');
                if after.is_ascii_alphanumeric() {
                    continue;
                }
                let code: String = chars[i..i + 4].iter().collect();
                if !codes.contains(&code) {
                    wrong.insert(format!("{}: {code}", shown(&page)));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "codes geas does not have:\n{}", wrong.into_iter().collect::<Vec<_>>().join("\n"));
    // the counts the READMEs give
    let n = codes.len();
    for (page, says) in [("README.md", format!("there are {n} codes")), ("README.ja.md", format!("診断のコードは全部で {n} 種類"))] {
        let text = read(&root().join(page)).split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(text.contains(&says), "{page} must give the count of codes as it is: {says:?}");
    }
}

/// What `geas --help` names: the commands, and the options.
fn help() -> (BTreeSet<String>, BTreeSet<String>) {
    let s = Scratch::new("docs-help");
    let (out, _, _) = run(s.path(), &["--help"], &[]);
    let commands = out.lines().filter_map(|l| l.strip_prefix("  geas ")).map(|l| l.split_whitespace().next().expect("a command").to_string()).collect();
    let mut options = BTreeSet::new();
    for w in out.split(|c: char| c.is_whitespace() || c == ',' || c == '|' || c == '(' || c == ')' || c == '`') {
        if w.starts_with('-') && w.len() > 1 {
            options.insert(w.trim_end_matches(['.', ';', ':']).to_string());
        }
    }
    (commands, options)
}

/// The geas command lines on a page: inline code that starts with `geas `, the `$ ` lines of
/// blocks that run geas (`$ geas …`, `$ … | geas …`), and the lines of shell blocks that do.
fn command_lines(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut fence: Option<String> = None;
    for line in text.lines() {
        if let Some(info) = line.trim_start().strip_prefix("```") {
            fence = if fence.is_some() { None } else { Some(info.trim().to_string()) };
            continue;
        }
        if let Some(info) = &fence {
            let l = line.trim_start();
            let l = match l.strip_prefix("$ ") {
                Some(rest) => rest,
                None if info == "sh" => l,
                None => continue,
            };
            let l = l.split("  #").next().expect("a line");
            if let Some(i) = l.find("geas ").filter(|&i| i == 0 || l[..i].ends_with("| ")) {
                out.push(l[i..].to_string());
            }
            continue;
        }
        for (k, span) in line.split('`').enumerate() {
            if k % 2 == 1 && span.starts_with("geas ") {
                out.push(span.to_string());
            }
        }
    }
    out
}

#[test]
fn every_command_and_option_on_the_pages_is_in_the_help() {
    let (commands, options) = help();
    let mut wrong = BTreeSet::new();
    let mut seen = 0;
    for page in pages() {
        for line in command_lines(&read(&page)) {
            seen += 1;
            let words: Vec<&str> = line.split_whitespace().collect();
            if let Some(cmd) = words.get(1).filter(|w| !w.starts_with('-') && !w.starts_with('<') && !w.starts_with('[')) {
                if !commands.contains(*cmd) {
                    wrong.insert(format!("{}: `{line}`: no command `{cmd}`", shown(&page)));
                }
            }
            for w in &words[1..] {
                let w = w.trim_matches(|c| c == '[' || c == ']' || c == '(' || c == ')');
                if !w.starts_with('-') || w == "-" {
                    continue;
                }
                let known = options.contains(w) || (w.starts_with("-j") && w[2..].chars().all(|c| c.is_ascii_digit()));
                if !known {
                    wrong.insert(format!("{}: `{line}`: no option `{w}`", shown(&page)));
                }
            }
        }
        // an option named alone, in inline code; the pages name one of another program's, Node's
        // `--require`, which geas puts on `NODE_OPTIONS`
        for (k, span) in read(&page).split('`').enumerate() {
            if k % 2 == 1 && span.starts_with("--") {
                let w = span.split_whitespace().next().expect("a word");
                if !options.contains(w) && w != "--require" {
                    wrong.insert(format!("{}: `{span}`: no option `{w}`", shown(&page)));
                }
            }
        }
    }
    assert!(seen > 50, "only {seen} geas command lines on the pages");
    assert!(wrong.is_empty(), "{}", wrong.into_iter().collect::<Vec<_>>().join("\n"));
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
    for page in pages() {
        for target in links(&read(&page)) {
            if target.starts_with("https://") || target.starts_with("http://") || target.starts_with('#') {
                continue;
            }
            seen += 1;
            let file = target.split('#').next().expect("a path");
            if !page.parent().expect("a directory").join(file).exists() {
                wrong.push(format!("{}: {target}", shown(&page)));
            }
        }
    }
    assert!(seen > 40, "only {seen} relative links");
    assert!(wrong.is_empty(), "links that lead nowhere:\n{}", wrong.join("\n"));
}
