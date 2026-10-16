//! The READMEs, the pages of docs/, the agent skill and the pages of chobo's site (website/chobo at
//! the root of ritsu's workspace, in both of its languages) show what the tool does. Every
//! diagnostic on them is word for word in a golden file of tests/fixtures, which tests/check.rs
//! holds to the checker, or, for what `ritsu check` says where a book meets the other languages, of
//! ritsu's tests/golden, which ritsu's tests hold to it; every `chobo` command shown with its output
//! prints that output now; every line of `.book` on them is a line of a book of the examples, the
//! tests or the diagnostics' own examples; every relative link leads to a file, and a link of the
//! site where Zensical leads it (to a page of the site, a file of it, a page of ritsu's site or of
//! another language's, or the playground on a project it lists in the page's language); the pages
//! of codes are what `chobo explain --all` prints; the counts they give are the counts; and the
//! keywords on the reference, and the ones the site colours, are src/syntax.rs's.
//!
//! What only the site shows is held to the files too: the code of the other languages is the lines
//! of one of their files, the JSON is the lines of a scenario beside a book, the text of a run or a
//! check is in a golden file, the code a page says `chobo build` writes is what it writes, and a
//! Mermaid chart is word for word on a page `chobo doc` wrote. The English site and the Japanese one
//! have the same pages, which their navs list in the same order. The site's pages of the examples and
//! its charts are held to `chobo doc` by tests/doc.rs.

mod common;
use common::*;
use ritsu_base::text::Lang;
use serde_json::Value;
use std::collections::BTreeSet;

/// The pages of chobo's site that its sync.sh copies in from docs/ (the Japanese site's codes.md from
/// docs/codes.ja.md): they are docs/'s pages, held here as they are, and the copies are not
/// committed.
const COPIED: [&str; 4] = ["reference.md", "formats.md", "targets.md", "codes.md"];

/// The pages chobo's site writes itself, in both languages: every Markdown page of website/chobo/docs
/// and docs-ja but the ones sync.sh copies in.
fn site_pages() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in ["docs", "docs-ja"] {
        let mut more: Vec<PathBuf> = std::fs::read_dir(site().join(dir))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "md") && !COPIED.iter().any(|c| p.file_name().is_some_and(|n| n == *c)))
            .collect();
        more.sort();
        out.extend(more);
    }
    assert!(out.len() > 10, "only {} pages of the site", out.len());
    out
}

/// The pages, each with the name the messages give it.
fn pages() -> Vec<PathBuf> {
    let mut out = vec![root().join("README.md"), root().join("README.ja.md"), root().join("skills/README.md")];
    for dir in ["docs", "../../skills/chobo"] {
        let mut more: Vec<PathBuf> = std::fs::read_dir(root().join(dir)).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "md")).collect();
        more.sort();
        out.extend(more);
    }
    out.extend(site_pages());
    out
}

/// A page as the messages name it: from the crate, or else from the root of the workspace.
fn shown(f: &Path) -> String {
    f.strip_prefix(root()).or_else(|_| f.strip_prefix(repo())).unwrap().display().to_string()
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

/// Every file under `dir` whose name ends with one of `tails`, by its path, leaving out what a build,
/// a package manager or a tool's cache put there, and anything behind a symbolic link.
fn files_under(dir: &Path, tails: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Ok(kind) = e.file_type() else { continue };
            if kind.is_dir() {
                if !name.starts_with('.') && !["target", "node_modules", "build", "venv", "__pycache__"].contains(&name.as_str()) {
                    todo.push(e.path());
                }
            } else if kind.is_file() && tails.iter().any(|t| name.ends_with(t)) {
                out.push(e.path());
            }
        }
    }
    out.sort();
    out
}

/// The golden files a block of text on a page can come from, each with its text: chobo's own
/// (tests/fixtures), and ritsu's (crates/ritsu/tests/golden), which hold what `ritsu check` and
/// `ritsu run` print where a book meets the other languages.
fn goldens() -> Vec<(PathBuf, String)> {
    let mut files = files_under(&root().join("tests/fixtures"), &[".txt"]);
    files.extend(files_under(&repo().join("crates/ritsu/tests/golden"), &[".txt"]));
    files.into_iter().map(|p| (p.clone(), std::fs::read_to_string(&p).unwrap())).collect()
}

#[test]
fn the_diagnostics_on_the_pages_are_what_the_checker_prints() {
    let goldens = goldens();
    let heads = ["error[", "warning[", "エラー[", "警告["];
    let mut wrong = Vec::new();
    let mut seen = BTreeSet::new();
    for f in pages() {
        for (_, lines) in fenced(&std::fs::read_to_string(&f).unwrap()) {
            if !lines.first().is_some_and(|l| heads.iter().any(|h| l.starts_with(h))) {
                continue;
            }
            seen.insert(shown(&f));
            if !goldens.iter().any(|(_, g)| g.contains(&lines.join("\n"))) {
                wrong.push(format!("{}: {}", shown(&f), lines[0]));
            }
        }
    }
    for must in [
        "README.md",
        "README.ja.md",
        "../../skills/chobo/SKILL.md",
        "website/chobo/docs/checks.md",
        "website/chobo/docs/together.md",
        "website/chobo/docs-ja/checks.md",
        "website/chobo/docs-ja/together.md",
    ] {
        assert!(seen.contains(must), "no diagnostic on {must}: were the fences changed?");
    }
    assert!(wrong.is_empty(), "not what any golden file holds:\n{}", wrong.join("\n"));
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

/// Whether a line on a page is the real line `real`: a line cut short with `…` stands for a real
/// line that has its pieces in that order, and a line without it is the whole line. The first piece
/// starts the real line unless `…` comes before it, and the last ends it unless `…` comes after it,
/// so that a line cut short without saying so is not taken for the line.
fn line_is(shown: &str, real: &str) -> bool {
    let (shown, real) = (shown.trim(), real.trim());
    let pieces: Vec<&str> = shown.split('…').map(str::trim).filter(|p| !p.is_empty()).collect();
    let mut at = 0;
    for (n, p) in pieces.iter().enumerate() {
        let i = if n + 1 == pieces.len() && !shown.ends_with('…') {
            match real[at..].strip_suffix(p) {
                Some(head) => head.len(),
                None => return false,
            }
        } else {
            match real[at..].find(p) {
                Some(i) => i,
                None => return false,
            }
        };
        if n == 0 && i != 0 && !shown.starts_with('…') {
            return false;
        }
        at += i + p.len();
    }
    true
}

#[test]
fn the_book_on_the_pages_is_from_the_books() {
    let real = real_lines();
    let found = |line: &str| real.iter().any(|r| line_is(line, r));
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
    for must in ["README.md", "README.ja.md", "docs/reference.md", "../../skills/chobo/SKILL.md", "website/chobo/docs/tour.md", "website/chobo/docs-ja/tour.md"] {
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

/// Whether the lines of a block are in `text`, wherever they start: the runs of lines between the
/// lines that are only `…`, each as it is and whole, one after another.
fn excerpt_of(text: &str, lines: &[String]) -> bool {
    let mut shown = vec![String::from("…")];
    shown.extend(lines.iter().cloned());
    shown.push(String::from("…"));
    shows(text, &shown).is_ok()
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

/// The relative links of the READMEs, docs/ and the skill lead to a file, from the page's own
/// directory, as GitHub reads them.
#[test]
fn every_relative_link_leads_to_a_file() {
    let mut wrong = Vec::new();
    let mut seen = 0;
    for f in pages().into_iter().filter(|f| !f.starts_with(site())) {
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

// ── where a link of the site leads ────────────────────────────────────────

/// A path with `.` and `..` worked out: `/ritsu/chobo/ja/../../ja/playground/` is
/// `/ritsu/ja/playground/`.
fn normal(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for p in path.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    let mut out = format!("/{}", parts.join("/"));
    if path.ends_with('/') && out != "/" {
        out.push('/');
    }
    out
}

/// The page a path of ritsu's published site is made from: ritsu's own pages at /ritsu/ and
/// /ritsu/ja/, and a language's site, a directory of website/ with a zensical.toml, at
/// /ritsu/<language>/ and /ritsu/<language>/ja/. A page chobo's sync.sh copies in counts, though a
/// tree that was never built has none.
fn source_of(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("/ritsu/").or((url == "/ritsu").then_some(""))?;
    let mut parts: Vec<&str> = rest.split('/').filter(|p| !p.is_empty()).collect();
    let mut base = repo().join("website");
    if let Some(first) = parts.first().copied()
        && base.join(first).join("zensical.toml").is_file()
    {
        base = base.join(first);
        parts.remove(0);
    }
    let docs = if parts.first() == Some(&"ja") {
        parts.remove(0);
        "docs-ja"
    } else {
        "docs"
    };
    let d = base.join(docs);
    let rest = parts.join("/");
    let candidates = if rest.is_empty() { vec![d.join("index.md")] } else { vec![d.join(format!("{rest}.md")), d.join(&rest).join("index.md"), d.join(&rest).join("index.html"), d.join(&rest)] };
    candidates.into_iter().find(|p| p.is_file() || copied(p))
}

/// Whether a page is one chobo's sync.sh copies into its site.
fn copied(p: &Path) -> bool {
    p.parent().is_some_and(|d| d == site().join("docs") || d == site().join("docs-ja")) && COPIED.iter().any(|c| p.file_name().is_some_and(|n| n == *c))
}

/// The text of a page of chobo's site, or of the page of docs/ sync.sh copies to it.
fn text_of(p: &Path) -> String {
    if copied(p) {
        let name = p.file_name().unwrap().to_string_lossy();
        let from = if name == "codes.md" && p.starts_with(site().join("docs-ja")) { "codes.ja.md".to_string() } else { name.to_string() };
        return std::fs::read_to_string(root().join("docs").join(from)).unwrap();
    }
    std::fs::read_to_string(p).unwrap_or_default()
}

/// The anchors of a page's headings, as the site makes them (`toc.slugify`, in lower case): the
/// words kept, in any script, the spaces turned into `-`, the rest dropped; or the id a heading
/// gives itself (`{ #id }`).
fn anchors(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
            continue;
        }
        let Some(heading) = line.strip_prefix('#').filter(|_| !fence) else { continue };
        let heading = heading.trim_start_matches('#').trim();
        if let Some((_, id)) = heading.rsplit_once("{ #") {
            out.push(id.trim_end_matches(['}', ' ']).to_string());
            continue;
        }
        let slug: String = heading
            .to_lowercase()
            .chars()
            .filter_map(|c| if c == ' ' { Some('-') } else if c.is_alphanumeric() || c == '-' || c == '_' { Some(c) } else { None })
            .collect();
        out.push(slug);
    }
    out
}

/// Whether a link opens ritsu's playground on a project it lists for `lang` (`en` or `ja`), and on
/// a file of the project when it names one, as projects.json lists them.
fn opens_a_project(fragment: &str, lang: &str) -> bool {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(repo().join("website/docs/playground/projects.json")).unwrap()).unwrap();
    let parts: Vec<(&str, &str)> = fragment.split('&').map(|kv| kv.split_once('=').unwrap_or((kv, ""))).collect();
    let part = |key: &str| parts.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);
    let Some(name) = part("project") else { return false };
    v["projects"].as_array().unwrap().iter().any(|p| {
        p["name"] == name && p["lang"].as_str().is_none_or(|l| l == lang) && part("file").is_none_or(|f| p["files"].as_array().unwrap().iter().any(|x| x[0] == f))
    })
}

/// The data a page `chobo doc --format html` wrote steps through.
fn page_data(page: &str) -> Value {
    let json = page.split("<script type=\"application/json\" id=\"chobo-data\">").nth(1).and_then(|r| r.split("</script>").next()).unwrap_or("null");
    serde_json::from_str(&json.replace("<\\/", "</")).unwrap_or(Value::Null)
}

/// Where a relative link of a page of chobo's site leads, as Zensical reads it
/// (`use_directory_urls`): a link to a `.md` names a page of the site from the page's own file, and
/// its `#anchor` a heading of it; a link to another file of the site's docs (an image, a page
/// `chobo doc` drew, with the scenario, the step and the way it came out that it opens) names that
/// file; and any other is a path of the published tree, read from the directory of the page's file,
/// under /ritsu/, which names a page of ritsu's site or of a language's, or the playground in the
/// page's language on a project it lists in that language. The Japanese site's images and
/// stylesheet are the English site's, which sync.sh copies into docs-ja and are not committed, so a
/// Japanese page's link to one names the file of docs/.
fn site_link(page: &Path, target: &str) -> Result<(), String> {
    let (file, fragment) = target.split_once('#').map_or((target, None), |(f, a)| (f, Some(a)));
    if file.is_empty() {
        return Ok(());
    }
    let dir = page.parent().unwrap();
    let ja = dir.ends_with("docs-ja");
    if file.ends_with(".md") {
        let to = PathBuf::from(normal(&dir.join(file).display().to_string()));
        if !to.is_file() && !copied(&to) {
            return Err(format!("{file} is no page of the site"));
        }
        return match fragment {
            Some(a) if !anchors(&text_of(&to)).iter().any(|h| h == a) => Err(format!("no heading of {file} is anchored #{a}")),
            _ => Ok(()),
        };
    }
    let to = match dir.join(file) {
        to if !to.is_file() && ja && (file.starts_with("images/") || file.starts_with("stylesheets/")) => site().join("docs").join(file),
        to => to,
    };
    if to.is_file() {
        if let Some(a) = fragment.filter(|a| a.starts_with("scenario=")) {
            let data = page_data(&std::fs::read_to_string(&to).unwrap());
            let n = |k: &str| a.split('&').find_map(|kv| kv.strip_prefix(&format!("{k}="))).and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
            let (s, step, o) = (n("scenario"), n("step"), n("outcome"));
            let steps = data["scenarios"].get(s.wrapping_sub(1)).and_then(|x| x["outcomes"].get(o.wrapping_sub(1))).and_then(Value::as_array).map(Vec::len);
            if !steps.is_some_and(|len| s > 0 && o > 0 && step <= len) {
                return Err(format!("{file} has no scenario {s} that comes out a way {o} with a step {step}"));
            }
        }
        return Ok(());
    }
    let at = if ja { "/ritsu/chobo/ja/" } else { "/ritsu/chobo/" };
    let url = normal(&format!("{at}{file}"));
    if url == "/ritsu/playground/" || url == "/ritsu/ja/playground/" {
        if (url == "/ritsu/ja/playground/") != ja {
            return Err(format!("{url} is the playground in another language than the page's"));
        }
        let lang = if ja { "ja" } else { "en" };
        return match fragment {
            Some(f) if opens_a_project(f, lang) => Ok(()),
            _ => Err(format!("{url}#{} opens no project the playground lists in {lang}", fragment.unwrap_or(""))),
        };
    }
    match source_of(&url) {
        Some(_) => Ok(()),
        None => Err(format!("{url} is no page of ritsu's site")),
    }
}

#[test]
fn every_link_of_the_site_leads_where_zensical_leads_it() {
    let mut wrong = Vec::new();
    let mut seen = 0;
    for f in site_pages() {
        for target in links(&std::fs::read_to_string(&f).unwrap()) {
            if target.starts_with("https://") || target.starts_with("http://") {
                continue;
            }
            seen += 1;
            if let Err(e) = site_link(&f, &target) {
                wrong.push(format!("{}: `{target}`: {e}", shown(&f)));
            }
        }
    }
    assert!(seen > 50, "only {seen} relative links on the site's pages");
    assert!(wrong.is_empty(), "links of the site that lead nowhere:\n{}", wrong.join("\n"));
    eprintln!("compared: {seen} links of the site's pages with where they lead");
}

/// The pages a nav of chobo's site lists, in its order: its `.md` targets (the playground, which is
/// ritsu's page and not one of the site's, left out).
fn nav_pages(config: &str) -> Vec<String> {
    let text = std::fs::read_to_string(site().join(config)).unwrap();
    let nav = text.split("nav = [").nth(1).unwrap_or_else(|| panic!("website/chobo/{config} has no nav")).split("\n]").next().unwrap();
    nav.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.rsplit_once("= \""))
        .map(|(_, t)| t.split('"').next().unwrap().to_string())
        .filter(|t| t.ends_with(".md"))
        .collect()
}

/// The Markdown pages of a docs directory of chobo's site, by name: the ones written there, and the
/// ones sync.sh copies in, whether or not the tree has been synced.
fn pages_of(dir: &str) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = std::fs::read_dir(site().join(dir)).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".md")).collect();
    out.extend(COPIED.iter().map(|c| c.to_string()));
    out
}

/// A page in one language and not in the other would leave a reader of the other without it. The
/// two sites have the same pages, each written in its own language (the reference, which sync.sh
/// copies in, aside); the two navs list every page, the same pages in the same order; and each home
/// page leads to every page written for the site.
#[test]
fn the_two_sites_have_the_same_pages_and_the_navs_list_them() {
    let (en, ja) = (pages_of("docs"), pages_of("docs-ja"));
    assert_eq!(en, ja, "website/chobo/docs and website/chobo/docs-ja hold other pages");
    let (nav_en, nav_ja) = (nav_pages("zensical.toml"), nav_pages("zensical.ja.toml"));
    assert_eq!(nav_en, nav_ja, "the two navs of chobo's site list other pages, or the same in another order");
    let listed: BTreeSet<String> = nav_en.iter().cloned().collect();
    assert_eq!(listed.len(), nav_en.len(), "the nav lists a page twice: {nav_en:?}");
    assert_eq!(listed, en, "the nav lists other pages than website/chobo/docs holds");
    for (index, nav) in [("docs/index.md", &nav_en), ("docs-ja/index.md", &nav_ja)] {
        let leads: BTreeSet<String> = links(&std::fs::read_to_string(site().join(index)).unwrap()).into_iter().map(|t| t.split('#').next().unwrap().to_string()).collect();
        for p in nav.iter().filter(|p| *p != "index.md" && !COPIED.contains(&p.as_str())) {
            assert!(leads.contains(p), "website/chobo/{index} does not lead to {p}, which its nav lists");
        }
    }
}

// ── what only the site shows ──────────────────────────────────────────────

/// The languages whose code a page of the site shows, by the word after the fence, which is the
/// extension of their files.
const OTHERS: [&str; 7] = ["rule", "flow", "cal", "gate", "req", "ctx", "geas"];

/// Whether `shown`, the lines of a block, are lines of `file`, each after the one before it.
fn lines_in_order(shown: &[&str], file: &[String]) -> bool {
    let mut at = 0;
    shown.iter().all(|l| match file[at..].iter().position(|r| line_is(l, r)) {
        Some(i) => {
            at += i + 1;
            true
        }
        None => false,
    })
}

/// The trimmed lines of a block, without the empty ones and the lines that are only `…`.
fn shown_lines(lines: &[String]) -> Vec<&str> {
    lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty() && *l != "…").collect()
}

/// The code of another language on the site (a workflow that runs the book's operations, a gate
/// that guards them, a requirement they satisfy) is the lines, in order, of one file of that
/// language in ritsu's workspace.
#[test]
fn the_code_of_the_other_languages_on_the_site_is_from_their_files() {
    let tails: Vec<String> = OTHERS.iter().map(|e| format!(".{e}")).collect();
    let tails: Vec<&str> = tails.iter().map(String::as_str).collect();
    let files: Vec<(PathBuf, Vec<String>)> = files_under(&repo(), &tails)
        .into_iter()
        .map(|p| {
            let lines = std::fs::read_to_string(&p).unwrap_or_default().lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
            (p, lines)
        })
        .collect();
    let mut wrong = Vec::new();
    let mut seen = 0;
    for f in site_pages() {
        for (info, lines) in fenced(&std::fs::read_to_string(&f).unwrap()) {
            if !OTHERS.contains(&info.as_str()) {
                continue;
            }
            seen += 1;
            let them = shown_lines(&lines);
            let of = |p: &Path| p.extension().is_some_and(|x| x == info.as_str());
            if !files.iter().any(|(p, file)| of(p) && lines_in_order(&them, file)) {
                let lone = them.iter().find(|l| !files.iter().any(|(p, file)| of(p) && file.iter().any(|r| line_is(l, r))));
                wrong.push(format!("{}: a ```{info} block that is the lines of no one .{info} file, in order{}", shown(&f), lone.map_or(String::new(), |l| format!("; `{l}` is a line of none"))));
            }
        }
    }
    // a workflow on the home page and on With the other languages, a gate and a requirement there
    assert!(seen >= 4, "only {seen} blocks of the other languages' code on the site");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The JSON on the site is the lines, in order, of one JSON file beside chobo's books: a scenario
/// written by hand, or what a test holds of one.
#[test]
fn the_json_on_the_site_is_from_the_files() {
    let mut files = files_under(&root().join("examples"), &[".json"]);
    files.extend(files_under(&root().join("tests/books"), &[".json"]));
    let files: Vec<Vec<String>> = files.iter().map(|p| std::fs::read_to_string(p).unwrap().lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()).collect();
    let mut wrong = Vec::new();
    let mut seen = 0;
    for f in site_pages() {
        for (info, lines) in fenced(&std::fs::read_to_string(&f).unwrap()) {
            if info != "json" {
                continue;
            }
            seen += 1;
            let them = shown_lines(&lines);
            if !files.iter().any(|file| lines_in_order(&them, file)) {
                wrong.push(format!("{}: a ```json block that is the lines of no JSON file beside the books: {:?}", shown(&f), them.first()));
            }
        }
    }
    assert!(seen >= 1, "no JSON on the site's pages: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The text the site quotes of a tool (a diagnostic, what `ritsu run` printed) is in one golden
/// file, as it is: the runs of lines between the lines that are only `…`, each whole, one after
/// another.
#[test]
fn the_text_on_the_site_is_in_a_golden_file() {
    let goldens = goldens();
    let mut wrong = Vec::new();
    let mut seen = 0;
    for f in site_pages() {
        for (info, lines) in fenced(&std::fs::read_to_string(&f).unwrap()) {
            if info != "text" {
                continue;
            }
            seen += 1;
            if !goldens.iter().any(|(_, g)| excerpt_of(g, &lines)) {
                wrong.push(format!("{}: a ```text block in no golden file: {:?}", shown(&f), lines.first()));
            }
        }
    }
    assert!(seen >= 8, "only {seen} blocks of text on the site's pages");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The code a page of the site says `chobo build` writes, under `<!-- chobo build <book> --target
/// <target> -->` on the line before its fence, is what that build writes: the runs of its lines
/// between the lines that are only `…`, each whole, in one of the files it writes.
#[test]
fn the_code_chobo_builds_on_the_site_is_what_it_writes() {
    let work = TempDir::new("docs-builds");
    let mut wrong = Vec::new();
    let mut seen = 0;
    for f in site_pages() {
        let text = std::fs::read_to_string(&f).unwrap();
        let mut said: Option<Vec<String>> = None;
        let mut block: Option<(Vec<String>, Vec<String>)> = None;
        for line in text.lines() {
            if let Some((args, mut lines)) = block.take() {
                if line.trim_start().starts_with("```") {
                    // the build the comment names, into a directory of its own
                    let out = work.path().join(format!("build-{seen}"));
                    let mut a = args.clone();
                    a.extend(["--out".to_string(), out.display().to_string()]);
                    let o = chobo().current_dir(root()).args(&a).output().unwrap();
                    seen += 1;
                    if !o.status.success() {
                        wrong.push(format!("{}: `chobo {}` failed: {}", shown(&f), args.join(" "), String::from_utf8_lossy(&o.stderr)));
                        continue;
                    }
                    let written: Vec<String> = files_under(&out, &[""]).iter().map(|p| std::fs::read_to_string(p).unwrap()).collect();
                    if !written.iter().any(|w| excerpt_of(w, &lines)) {
                        wrong.push(format!("{}: a block under `chobo {}` that is in no file it writes: {:?}", shown(&f), args.join(" "), lines.first()));
                    }
                } else {
                    lines.push(line.to_string());
                    block = Some((args, lines));
                }
                continue;
            }
            if line.trim_start().starts_with("```") {
                if let Some(args) = said.take() {
                    block = Some((args, Vec::new()));
                }
                continue;
            }
            if let Some(cmd) = line.trim().strip_prefix("<!-- chobo build ").and_then(|r| r.strip_suffix("-->")) {
                let mut args = vec!["build".to_string()];
                args.extend(words(cmd.trim()));
                said = Some(args);
            } else if !line.trim().is_empty() {
                said = None;
            }
        }
    }
    // the SQL of a hold and the types of a client's answers, on Build for a database
    assert!(seen >= 2, "only {seen} blocks of built code on the site's pages: were the comments changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// A Mermaid chart on the site is word for word on a page `chobo doc` wrote: beside an example, or a
/// golden file of tests/doc, which tests/doc.rs holds to it.
#[test]
fn the_charts_on_the_site_are_what_doc_draws() {
    let mut files = files_under(&root().join("examples"), &[".md"]);
    files.extend(files_under(&root().join("tests/doc"), &[".md"]));
    let drawn: Vec<String> = files.iter().map(|p| std::fs::read_to_string(p).unwrap()).collect();
    let mut wrong = Vec::new();
    let mut seen = 0;
    for f in site_pages() {
        for (info, lines) in fenced(&std::fs::read_to_string(&f).unwrap()) {
            if info != "mermaid" {
                continue;
            }
            seen += 1;
            let chart = format!("```mermaid\n{}\n```", lines.join("\n"));
            if !drawn.iter().any(|d| d.contains(&chart)) {
                wrong.push(format!("{}: a chart no page of `chobo doc` has: {:?}", shown(&f), lines.first()));
            }
        }
    }
    assert!(seen >= 1, "no Mermaid chart on the site's pages: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
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
        (root().join("README.md"), format!("all {codes} codes")),
        (root().join("README.md"), format!("{scenarios} of them with the ones written by hand, {together} with callers at the same time")),
        (root().join("README.ja.md"), format!("診断のコードは全部で {codes} 種類")),
        (root().join("README.ja.md"), format!("手で書いたものを含めて {scenarios} 本、そのうち {together} 本は呼び出し元が同時に来るもの")),
        (root().join("docs/reference.md"), format!("all {codes} codes")),
        (site().join("docs/checks.md"), format!("all {codes} codes")),
        (site().join("docs/assurance.md"), format!("{scenarios} of them with the ones written by hand, {together} with callers at the same time")),
        (site().join("docs/assurance.md"), format!("the {scenarios} scenarios above and the {codes} codes")),
        (site().join("docs-ja/checks.md"), format!("全部で {codes} 種類のコード")),
        (site().join("docs-ja/assurance.md"), format!("手で書いたものを含めて {scenarios} 本、そのうち {together} 本は呼び出し元が同時に来るもの")),
        (site().join("docs-ja/assurance.md"), format!("上の {scenarios} 本のシナリオと {codes} 種類のコード")),
    ] {
        let text = std::fs::read_to_string(&page).unwrap().split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(text.contains(&says), "{} must say the count as it is: {says:?}", shown(&page));
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

/// The site colours a ```book fence with website/chobo/tools/booklexer.py, whose `KEYWORDS` are
/// src/syntax.rs's, word for word and in their order.
#[test]
fn the_site_highlights_the_keywords_of_the_language() {
    let text = std::fs::read_to_string(site().join("tools/booklexer.py")).unwrap();
    let from = text.find("KEYWORDS = (").expect("booklexer.py has its KEYWORDS") + "KEYWORDS = (".len();
    let body = &text[from..from + text[from..].find(')').unwrap()];
    let lexer: Vec<String> = body.split('"').skip(1).step_by(2).map(String::from).collect();
    let ours: Vec<String> = chobo::syntax::KEYWORDS.iter().map(|(k, _)| k.to_string()).collect();
    assert_eq!(lexer, ours, "website/chobo/tools/booklexer.py's KEYWORDS must be src/syntax.rs's, word for word and in their order");
}

/// Each README leads to the site in its own language.
#[test]
fn the_readmes_lead_to_the_site() {
    for (readme, url) in [("README.md", "https://i2y.github.io/ritsu/chobo/"), ("README.ja.md", "https://i2y.github.io/ritsu/chobo/ja/")] {
        assert!(std::fs::read_to_string(root().join(readme)).unwrap().contains(&format!("<{url}>")), "{readme} does not lead to {url}");
    }
}
