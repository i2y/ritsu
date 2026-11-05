//! Markdown as the pages of `ritsu mod site` show it (DESIGN 17.17): what `ritsu mod diff` writes,
//! which ritsu made ([`md_html`]), and what a module's author wrote ([`readme_html`]), which ritsu
//! did not.
//!
//! A README comes from whoever published the module, so nothing in it reaches the page as HTML: every
//! character is escaped, a link leads only to `http:`, `https:`, a heading of the same README or
//! a file of the module, and an image is a link to it, which loads nothing.

use ritsu_base::docpage::esc;

// ── What ritsu wrote ────────────────────────────────────────────────────

/// A text of a page with its `` `code` `` spans made code, its `**strong**` strong, and the rest
/// escaped.
pub(super) fn inline(s: &str) -> String {
    let mut o = String::new();
    for (i, part) in s.split('`').enumerate() {
        if i % 2 == 1 {
            o.push_str(&format!("<code>{}</code>", esc(part)));
            continue;
        }
        let e = esc(part);
        let mut rest = e.as_str();
        while let Some(a) = rest.find("**") {
            let Some(z) = rest[a + 2..].find("**") else { break };
            o.push_str(&rest[..a]);
            o.push_str(&format!("<strong>{}</strong>", &rest[a + 2..a + 2 + z]));
            rest = &rest[a + 2 + z + 2..];
        }
        o.push_str(rest);
    }
    o
}

/// The cells of a row of a table, `\|` a `|` in a cell.
fn cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|');
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cur.push('|');
                chars.next();
            }
            '|' => out.push(std::mem::take(&mut cur).trim().to_string()),
            c => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out
}

/// Whether a row of a table is the line under its head.
fn is_rule(c: &[String]) -> bool {
    c.iter().all(|x| !x.is_empty() && x.chars().all(|ch| matches!(ch, '-' | ':')))
}

/// What [`md_html`] holds until a line ends it.
#[derive(Default)]
struct Held {
    para: Vec<String>,
    items: Vec<(usize, String)>,
    rows: Vec<Vec<String>>,
}

impl Held {
    fn para(&mut self, o: &mut String) {
        if !self.para.is_empty() {
            o.push_str(&format!("<p>{}</p>\n", inline(&self.para.join(" "))));
            self.para.clear();
        }
    }

    fn items(&mut self, o: &mut String) {
        if self.items.is_empty() {
            return;
        }
        o.push_str("<ul>\n");
        let mut deep = false;
        for (depth, text) in &self.items {
            if *depth > 0 && !deep {
                o.push_str("<ul>\n");
                deep = true;
            } else if *depth == 0 && deep {
                o.push_str("</ul>\n");
                deep = false;
            }
            o.push_str(&format!("<li>{}</li>\n", inline(text)));
        }
        if deep {
            o.push_str("</ul>\n");
        }
        o.push_str("</ul>\n");
        self.items.clear();
    }

    fn rows(&mut self, o: &mut String) {
        if self.rows.is_empty() {
            return;
        }
        o.push_str("<div class=\"scroll\"><table class=\"t\">\n");
        for (i, r) in self.rows.iter().enumerate() {
            let tag = if i == 0 { "th" } else { "td" };
            o.push_str("<tr>");
            for c in r {
                o.push_str(&format!("<{tag}>{}</{tag}>", inline(c)));
            }
            o.push_str("</tr>\n");
        }
        o.push_str("</table></div>\n");
        self.rows.clear();
    }

    fn all(&mut self, o: &mut String) {
        self.para(o);
        self.items(o);
        self.rows(o);
    }
}

/// The Markdown `ritsu mod diff` writes (headings, paragraphs, lists of two levels, tables, fenced
/// code, and `<details>` lines, which are HTML already, their names escaped), as HTML.
pub(super) fn md_html(md: &str) -> String {
    let mut o = String::new();
    let mut h = Held::default();
    let mut fence: Option<Vec<String>> = None;
    for line in md.lines() {
        if let Some(code) = fence.as_mut() {
            if line.trim_start().starts_with("```") {
                o.push_str(&format!("<pre><code>{}</code></pre>\n", esc(&code.join("\n"))));
                fence = None;
            } else {
                code.push(line.to_string());
            }
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            h.all(&mut o);
            fence = Some(Vec::new());
        } else if line.starts_with("<details>") || line.starts_with("</details>") {
            h.all(&mut o);
            o.push_str(line);
            o.push('\n');
        } else if trimmed.is_empty() {
            h.all(&mut o);
        } else if line.starts_with('#') {
            h.all(&mut o);
            let level = line.chars().take_while(|c| *c == '#').count().min(6);
            o.push_str(&format!("<h{level}>{}</h{level}>\n", inline(line[level..].trim())));
        } else if line.starts_with('|') {
            h.para(&mut o);
            h.items(&mut o);
            let c = cells(line);
            if !is_rule(&c) {
                h.rows.push(c);
            }
        } else if let Some(text) = trimmed.strip_prefix("- ") {
            h.para(&mut o);
            h.rows(&mut o);
            h.items.push((usize::from(line.len() - trimmed.len() >= 2), text.to_string()));
        } else {
            h.items(&mut o);
            h.rows(&mut o);
            h.para.push(trimmed.to_string());
        }
    }
    if let Some(code) = fence {
        o.push_str(&format!("<pre><code>{}</code></pre>\n", esc(&code.join("\n"))));
    }
    h.all(&mut o);
    o
}

// ── What a module's author wrote ────────────────────────────────────────

/// The id of a heading of a README. It starts with `r-` so that it cannot be the id of a section of
/// the page, and it keeps letters of every script: a heading in Japanese has an id too.
fn slug(text: &str) -> String {
    let mut o = String::from("r-");
    for c in text.chars() {
        if c.is_alphanumeric() {
            o.extend(c.to_lowercase());
        } else if (c == ' ' || c == '-' || c == '_') && !o.ends_with('-') {
            o.push('-');
        }
    }
    o.trim_end_matches('-').to_string()
}

/// The link a README's `target` becomes: `http:` and `https:` as they are, `#heading` to the heading's
/// id, and a path of the module's files through `file` (what it says of a path it does not know is
/// nothing, and the text is then only text). Nothing else is a link.
fn link_of(target: &str, file: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let t = target.trim();
    if t.is_empty() {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Some(t.to_string());
    }
    if let Some(h) = t.strip_prefix('#') {
        return Some(format!("#{}", slug(h).as_str()));
    }
    if t.contains(':') || t.starts_with("//") {
        return None;
    }
    let (path, tail) = match t.find('#') {
        Some(i) => t.split_at(i),
        None => (t, ""),
    };
    let path = path.strip_prefix("./").unwrap_or(path).trim_end_matches('/');
    file(path).map(|u| format!("{u}{tail}"))
}

/// One link of a README as HTML: outside the module, it says so to the browser.
fn anchor(href: &str, text: &str) -> String {
    let outside = href.starts_with("http://") || href.starts_with("https://");
    let rel = if outside { " rel=\"nofollow noopener\"" } else { "" };
    format!("<a href=\"{}\"{rel}>{text}</a>", esc(href))
}

/// The text of a README's line as HTML: code spans, `**strong**`, `*emphasis*`, `[text](target)`,
/// `![text](target)` (a link), `<https://…>`, and backslashes. Every other character is escaped.
fn span(s: &str, file: &dyn Fn(&str) -> Option<String>) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut o = String::new();
    let mut i = 0;
    let text = |a: &[char]| a.iter().collect::<String>();
    let find = |from: usize, want: char| (from..chars.len()).find(|&k| chars[k] == want);
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' if i + 1 < chars.len() && chars[i + 1].is_ascii_punctuation() => {
                o.push_str(&esc(&chars[i + 1].to_string()));
                i += 2;
            }
            '`' => {
                let run = chars[i..].iter().take_while(|x| **x == '`').count();
                let mut k = i + run;
                let mut end = None;
                while k < chars.len() {
                    if chars[k] == '`' {
                        let r = chars[k..].iter().take_while(|x| **x == '`').count();
                        if r == run {
                            end = Some(k);
                            break;
                        }
                        k += r;
                    } else {
                        k += 1;
                    }
                }
                match end {
                    Some(e) => {
                        let inner = text(&chars[i + run..e]);
                        o.push_str(&format!("<code>{}</code>", esc(inner.trim())));
                        i = e + run;
                    }
                    None => {
                        o.push_str(&esc(&text(&chars[i..i + run])));
                        i += run;
                    }
                }
            }
            '<' => {
                // an autolink, and nothing else that looks like HTML
                let auto = find(i + 1, '>').map(|e| (e, text(&chars[i + 1..e]))).filter(|(_, u)| {
                    let l = u.to_ascii_lowercase();
                    (l.starts_with("http://") || l.starts_with("https://")) && !u.contains(char::is_whitespace)
                });
                match auto {
                    Some((e, u)) => {
                        o.push_str(&anchor(&u, &esc(&u)));
                        i = e + 1;
                    }
                    None => {
                        o.push_str("&lt;");
                        i += 1;
                    }
                }
            }
            '!' | '[' => {
                let at = if c == '!' { i + 1 } else { i };
                let label = (chars.get(at) == Some(&'[')).then(|| find(at + 1, ']')).flatten();
                let target = label.filter(|e| chars.get(e + 1) == Some(&'(')).and_then(|e| find(e + 2, ')').map(|z| (e, z)));
                match target {
                    Some((e, z)) => {
                        let inner = span(&text(&chars[at + 1..e]), file);
                        // a title after the target is not kept
                        let t = text(&chars[e + 2..z]);
                        let t = t.split_whitespace().next().unwrap_or("").to_string();
                        match link_of(&t, file) {
                            Some(href) => o.push_str(&anchor(&href, &inner)),
                            None => o.push_str(&inner),
                        }
                        i = z + 1;
                    }
                    None => {
                        o.push_str(&esc(&c.to_string()));
                        i += 1;
                    }
                }
            }
            '*' | '_' => {
                let run = chars[i..].iter().take_while(|x| **x == c).count().min(2);
                let open = i + run;
                let boundary = i == 0 || !chars[i - 1].is_alphanumeric();
                let close = (open + 1..chars.len()).find(|&k| chars[k] == c && chars[k..].iter().take_while(|x| **x == c).count() >= run && !chars[k - 1].is_whitespace());
                match close {
                    Some(z) if boundary && open < chars.len() && !chars[open].is_whitespace() => {
                        let tag = if run == 2 { "strong" } else { "em" };
                        o.push_str(&format!("<{tag}>{}</{tag}>", span(&text(&chars[open..z]), file)));
                        i = z + run;
                    }
                    _ => {
                        o.push_str(&esc(&c.to_string()));
                        i += 1;
                    }
                }
            }
            c => {
                o.push_str(&esc(&c.to_string()));
                i += 1;
            }
        }
    }
    o
}

/// How far a line is indented, a tab as four spaces.
fn indent_of(line: &str) -> usize {
    let mut n = 0;
    for c in line.chars() {
        match c {
            ' ' => n += 1,
            '\t' => n += 4,
            _ => break,
        }
    }
    n
}

/// The item a line starts, if it starts one: whether its list is numbered, and its text.
fn item_of(line: &str) -> Option<(bool, &str)> {
    let t = line.trim_start();
    for m in ["- ", "* ", "+ "] {
        if let Some(r) = t.strip_prefix(m) {
            return Some((false, r));
        }
    }
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && digits <= 9 {
        let r = &t[digits..];
        if let Some(r) = r.strip_prefix(". ").or_else(|| r.strip_prefix(") ")) {
            return Some((true, r));
        }
    }
    None
}

/// Whether a line is a line of `-`, `*` or `_` across, which is a rule.
fn is_thematic(line: &str) -> bool {
    let t: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    t.len() >= 3 && (t.chars().all(|c| c == '-') || t.chars().all(|c| c == '*') || t.chars().all(|c| c == '_'))
}

/// The level and the text of a heading line.
fn heading_of(line: &str) -> Option<(usize, &str)> {
    let t = line.trim_start();
    let level = t.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let r = &t[level..];
    if !r.is_empty() && !r.starts_with(' ') {
        return None;
    }
    Some((level, r.trim().trim_end_matches('#').trim()))
}

/// Whether a line starts a block of its own, which ends the paragraph before it.
fn starts_block(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~") || t.starts_with('>') || heading_of(line).is_some() || item_of(line).is_some() || is_thematic(line)
}

/// A list: the lines from `start` that belong to it, written as HTML, and the line after it.
fn list(lines: &[&str], start: usize, file: &dyn Fn(&str) -> Option<String>, o: &mut String) -> usize {
    let base = indent_of(lines[start]);
    let (numbered, _) = item_of(lines[start]).unwrap_or((false, ""));
    let tag = if numbered { "ol" } else { "ul" };
    o.push_str(&format!("<{tag}>\n"));
    let mut i = start;
    while i < lines.len() {
        let line = lines[i];
        let Some((num, first)) = item_of(line).filter(|_| indent_of(line) == base) else { break };
        if num != numbered {
            break;
        }
        let mut text = vec![first.trim().to_string()];
        let mut inner = String::new();
        i += 1;
        while i < lines.len() {
            let l = lines[i];
            if l.trim().is_empty() {
                // a blank line ends the item unless more of the list follows
                let next = lines[i + 1..].iter().find(|x| !x.trim().is_empty());
                match next {
                    Some(n) if indent_of(n) > base || (indent_of(n) == base && item_of(n).is_some_and(|(m, _)| m == numbered)) => {
                        i += 1;
                        continue;
                    }
                    _ => break,
                }
            }
            if indent_of(l) <= base {
                if item_of(l).is_some() || starts_block(l) || text.is_empty() {
                    break;
                }
                // a line of the item's text, with nothing to indent it
                text.push(l.trim().to_string());
                i += 1;
                continue;
            }
            if item_of(l).is_some() {
                i = list(lines, i, file, &mut inner);
                continue;
            }
            text.push(l.trim().to_string());
            i += 1;
        }
        o.push_str(&format!("<li>{}{inner}</li>\n", span(&text.join(" "), file)));
    }
    o.push_str(&format!("</{tag}>\n"));
    i
}

/// A README as HTML, its first heading left out when `title` says the same (the page says it
/// already), its headings one level down so that its `#` sits under the page's. `file` gives the
/// link to a file of the module from its path; `at` the part of a heading's id that keeps it apart
/// from the ids of other pages' READMEs is not needed, since each page holds one README.
pub(super) fn readme_html(md: &str, title: &str, file: &dyn Fn(&str) -> Option<String>) -> String {
    let lines: Vec<&str> = md.lines().collect();
    let mut o = String::new();
    let mut i = 0;
    let mut first_heading = true;
    let mut used: Vec<String> = Vec::new();
    while i < lines.len() {
        let line = lines[i];
        let t = line.trim_start();
        if t.is_empty() {
            i += 1;
        } else if t.starts_with("```") || t.starts_with("~~~") {
            let mark = if t.starts_with("```") { "```" } else { "~~~" };
            let mut code = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with(mark) {
                code.push(lines[i]);
                i += 1;
            }
            i += 1;
            o.push_str(&format!("<pre class=\"code\"><code>{}</code></pre>\n", esc(&code.join("\n"))));
        } else if let Some((level, text)) = heading_of(line) {
            i += 1;
            let was_first = std::mem::take(&mut first_heading);
            if was_first && level == 1 && text.eq_ignore_ascii_case(title) {
                continue;
            }
            let mut id = slug(text);
            if used.contains(&id) {
                let n = used.iter().filter(|u| u.starts_with(&id)).count();
                id = format!("{id}-{n}");
            }
            used.push(id.clone());
            let h = (level + 1).min(6);
            o.push_str(&format!("<h{h} id=\"{}\">{}</h{h}>\n", esc(&id), span(text, file)));
        } else if is_thematic(line) {
            i += 1;
            o.push_str("<hr>\n");
        } else if t.starts_with('>') {
            let mut quoted = Vec::new();
            while i < lines.len() && lines[i].trim_start().starts_with('>') {
                let l = lines[i].trim_start().trim_start_matches('>');
                quoted.push(l.strip_prefix(' ').unwrap_or(l).to_string());
                i += 1;
            }
            let text = quoted.join("\n");
            o.push_str(&format!("<blockquote>\n{}</blockquote>\n", readme_html(&text, "", file)));
        } else if item_of(line).is_some() {
            i = list(&lines, i, file, &mut o);
        } else if line.contains('|') && i + 1 < lines.len() && lines[i + 1].contains('-') && lines[i + 1].contains('|') && is_rule(&cells(lines[i + 1])) {
            let head = cells(line);
            o.push_str("<div class=\"scroll\"><table class=\"t\">\n<thead><tr>");
            for c in &head {
                o.push_str(&format!("<th scope=\"col\">{}</th>", span(c, file)));
            }
            o.push_str("</tr></thead>\n<tbody>\n");
            i += 2;
            while i < lines.len() && lines[i].contains('|') && !lines[i].trim().is_empty() {
                o.push_str("<tr>");
                for c in cells(lines[i]) {
                    o.push_str(&format!("<td>{}</td>", span(&c, file)));
                }
                o.push_str("</tr>\n");
                i += 1;
            }
            o.push_str("</tbody>\n</table></div>\n");
        } else {
            let mut para = vec![t.trim_end().to_string()];
            i += 1;
            while i < lines.len() && !lines[i].trim().is_empty() && !starts_block(lines[i]) {
                para.push(lines[i].trim().to_string());
                i += 1;
            }
            o.push_str(&format!("<p>{}</p>\n", span(&para.join(" "), file)));
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_markdown_of_a_diff_as_html() {
        let md = "- Files: 2 changed\n  - one `a|b`\n\n| Change | File |\n|---|---|\n| changed | `x\\|y` |\n\n<details><summary><code>r.rule</code> (rulec): differ</summary>\n\n**Version diff**\n\nweight >=2g\n\n</details>\n";
        let h = md_html(md);
        assert!(h.contains("<ul>\n<li>Files: 2 changed</li>\n<ul>\n<li>one <code>a|b</code></li>\n</ul>\n</ul>\n"), "{h}");
        assert!(h.contains("<tr><th>Change</th><th>File</th></tr>\n<tr><td>changed</td><td><code>x|y</code></td></tr>"), "{h}");
        assert!(h.contains("<details><summary><code>r.rule</code> (rulec): differ</summary>\n<p><strong>Version diff</strong></p>\n<p>weight &gt;=2g</p>\n</details>"), "{h}");
    }

    fn no_files(_: &str) -> Option<String> {
        None
    }

    fn known(p: &str) -> Option<String> {
        (p == "uk/a.cal").then(|| "files/uk/a.cal.html".to_string())
    }

    /// Nothing an author writes reaches the page as HTML, and only a link that leads to `http:`,
    /// `https:`, a heading or a file of the module is a link.
    #[test]
    fn nothing_of_a_readme_is_html() {
        let cases = [
            ("<script>alert(1)</script>", "<p>&lt;script&gt;alert(1)&lt;/script&gt;</p>\n"),
            ("<img src=x onerror=alert(1)>", "<p>&lt;img src=x onerror=alert(1)&gt;</p>\n"),
            ("[a](javascript:alert(1))", "<p>a)</p>\n"),
            ("[a](JAVASCRIPT:alert(1))", "<p>a)</p>\n"),
            ("[a](data:text/html,x)", "<p>a</p>\n"),
            ("[a](//evil.example/)", "<p>a</p>\n"),
            ("![a](https://example.com/x.png)", "<p><a href=\"https://example.com/x.png\" rel=\"nofollow noopener\">a</a></p>\n"),
            ("[a\"onclick=\"x](https://example.com/\"onclick=\"x)", "<p><a href=\"https://example.com/&quot;onclick=&quot;x\" rel=\"nofollow noopener\">a&quot;onclick=&quot;x</a></p>\n"),
            ("`<b>`", "<p><code>&lt;b&gt;</code></p>\n"),
            ("<https://example.com/a>", "<p><a href=\"https://example.com/a\" rel=\"nofollow noopener\">https://example.com/a</a></p>\n"),
            ("<javascript:alert(1)>", "<p>&lt;javascript:alert(1)&gt;</p>\n"),
        ];
        for (md, want) in cases {
            assert_eq!(readme_html(md, "", &no_files), want, "{md}");
        }
        let code = readme_html("```html\n<script>x</script>\n```\n", "", &no_files);
        assert_eq!(code, "<pre class=\"code\"><code>&lt;script&gt;x&lt;/script&gt;</code></pre>\n");
    }

    #[test]
    fn a_readme_as_html() {
        let md = "# holidays\n\nThe days, *each* **pinned**.\nA second line.\n\n## Use it now\n\n- one\n- two\n  - deep\n\n1. first\n2. second\n\n| File | What |\n|---|---|\n| `uk/a.cal` | [a](uk/a.cal) and [b](uk/b.cal) |\n\n> A note\n\n---\n\nSee [below](#use-it-now).\n";
        let h = readme_html(md, "holidays", &known);
        assert!(!h.contains(">holidays<"), "{h}");
        assert!(h.contains("<p>The days, <em>each</em> <strong>pinned</strong>. A second line.</p>"), "{h}");
        assert!(h.contains("<h3 id=\"r-use-it-now\">Use it now</h3>"), "{h}");
        assert!(h.contains("<ul>\n<li>one</li>\n<li>two<ul>\n<li>deep</li>\n</ul>\n</li>\n</ul>\n"), "{h}");
        assert!(h.contains("<ol>\n<li>first</li>\n<li>second</li>\n</ol>\n"), "{h}");
        assert!(h.contains("<td><code>uk/a.cal</code></td><td><a href=\"files/uk/a.cal.html\">a</a> and b</td>"), "{h}");
        assert!(h.contains("<blockquote>\n<p>A note</p>\n</blockquote>"), "{h}");
        assert!(h.contains("<hr>"), "{h}");
        assert!(h.contains("See <a href=\"#r-use-it-now\">below</a>."), "{h}");
    }

    #[test]
    fn a_heading_in_japanese_has_an_id_and_the_same_heading_twice_has_two() {
        let h = readme_html("## 使い方\n\n## 使い方\n", "", &no_files);
        assert!(h.contains("<h3 id=\"r-使い方\">使い方</h3>\n<h3 id=\"r-使い方-1\">使い方</h3>"), "{h}");
    }
}
