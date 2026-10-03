//! The frame of an approver's page (DESIGN 4.8): what rulec's, dandori's, koyomi's and chobo's
//! `doc` pages each wrote around their content.
//!
//! - HTML: `<!doctype html>`, the language, the viewport, a `generator` that names the tool and
//!   its version, the title, and one stylesheet inside the page. Its colours are variables: a
//!   light palette, a dark one when the reader's system asks for it (unless the page is told
//!   `data-theme="light"`), and the dark one when the page is told `data-theme="dark"`. The
//!   page reads no file from outside, script or stylesheet.
//! - The head of the page says where it came from: the source's path, the first sixteen digits
//!   of its SHA-256, and the tool and its version ([`stamp`]).
//! - Markdown: one comment at the top that says the same ([`markdown_head`]).
//!
//! What the page holds — rulec's tables and cards, dandori's diagrams and scenarios, koyomi's
//! months, chobo's balances — stays each language's.

use crate::text::{Lang, Text};
use crate::tr;

/// The colours every page has, by role. A page adds its own after these.
pub const ROLES: &[&str] = &["bg", "fg", "dim", "line", "soft", "panel", "code", "accent", "ok", "warn", "bad"];

/// The common palette, light and dark, by [`ROLES`]: chobo's and dandori's values, which are
/// GitHub's.
pub const LIGHT: &[(&str, &str)] = &[
    ("bg", "#ffffff"),
    ("fg", "#1f2328"),
    ("dim", "#59636e"),
    ("line", "#d1d9e0"),
    ("soft", "#f6f8fa"),
    ("panel", "#f6f8fa"),
    ("code", "#eff2f5"),
    ("accent", "#0969da"),
    ("ok", "#1a7f37"),
    ("warn", "#9a6700"),
    ("bad", "#cf222e"),
];

pub const DARK: &[(&str, &str)] = &[
    ("bg", "#0d1117"),
    ("fg", "#e6edf3"),
    ("dim", "#9198a1"),
    ("line", "#3d444d"),
    ("soft", "#151b23"),
    ("panel", "#151b23"),
    ("code", "#262c36"),
    ("accent", "#4493f8"),
    ("ok", "#3fb950"),
    ("warn", "#d29922"),
    ("bad", "#f85149"),
];

/// A page's palette: the common one and the page's own colours after it, light and dark.
#[derive(Clone, Debug)]
pub struct Palette {
    pub light: Vec<(String, String)>,
    pub dark: Vec<(String, String)>,
}

impl Default for Palette {
    fn default() -> Palette {
        let own = |v: &[(&str, &str)]| v.iter().map(|(k, c)| (k.to_string(), c.to_string())).collect();
        Palette { light: own(LIGHT), dark: own(DARK) }
    }
}

impl Palette {
    /// One more colour, or a different value for one the palette has.
    pub fn set(mut self, name: &str, light: &str, dark: &str) -> Palette {
        for (v, c) in [(&mut self.light, light), (&mut self.dark, dark)] {
            match v.iter_mut().find(|(k, _)| k == name) {
                Some(e) => e.1 = c.to_string(),
                None => v.push((name.to_string(), c.to_string())),
            }
        }
        self
    }

    /// The variables as CSS: `:root` with the light ones, the dark ones under
    /// `prefers-color-scheme: dark` unless the page says `data-theme="light"`, the dark ones
    /// again under `data-theme="dark"`, and `data-theme="light"` kept light.
    pub fn css(&self) -> String {
        let vars = |v: &[(String, String)]| v.iter().map(|(k, c)| format!("--{k}: {c};")).collect::<Vec<_>>().join(" ");
        let (light, dark) = (vars(&self.light), vars(&self.dark));
        format!(
            ":root {{ color-scheme: light dark; {light} }}\n\
             @media (prefers-color-scheme: dark) {{ :root:not([data-theme=\"light\"]) {{ color-scheme: dark; {dark} }} }}\n\
             :root[data-theme=\"dark\"] {{ color-scheme: dark; {dark} }}\n\
             :root[data-theme=\"light\"] {{ color-scheme: light; }}\n"
        )
    }
}

/// A text as HTML, its `&`, `<`, `>`, `"` and `'` escaped.
pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            c => o.push(c),
        }
    }
    o
}

/// The top of a page, up to and including `<body>`: the doctype, the language, the viewport,
/// the generator (`koyomi 0.1.0`), the title, and the stylesheet.
pub fn html_head(lang: Lang, generator: &str, title: &str, css: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"{}\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"generator\" content=\"{}\">\n<title>{}</title>\n<style>\n{css}</style>\n</head>\n<body>\n",
        lang.code(),
        esc(generator),
        esc(title)
    )
}

/// The end of a page.
pub const HTML_TAIL: &str = "</body>\n</html>\n";

/// Where a page came from, for its head: the source's path, the first sixteen digits of its
/// SHA-256, and the tool and its version, as a `<dl class="stamp">`.
pub fn stamp(lang: Lang, path: &str, sha16: &str, tool: &str, version: &str) -> String {
    let (from, digest, by) = match lang {
        Lang::Ja => ("元のファイル", "SHA-256", "書き出したツール"),
        Lang::En => ("Source", "SHA-256", "Written by"),
    };
    format!(
        "<dl class=\"stamp\"><dt>{from}</dt><dd><code>{}</code></dd><dt>{digest}</dt><dd><code>{}</code></dd><dt>{by}</dt><dd>{} {}</dd></dl>\n",
        esc(path),
        esc(sha16),
        esc(tool),
        esc(version)
    )
}

/// The comment at the top of a Markdown page: what wrote it, from which file, and that it is
/// not edited by hand.
pub fn markdown_head(tool: &str, version: &str, path: &str, sha16: &str) -> Text {
    tr!(
        "<!-- {tool} {version} が {path}（sha256:{sha16}）から書き出したもの。手で直しません。 -->\n",
        "<!-- Generated by {tool} {version} from {path} (sha256:{sha16}). Do not edit by hand. -->\n"
    )
}

/// What a page reads from outside: every `src="…"`, `href="…"`, `url(…)` and `@import` that
/// points at another server (`http:`, `https:`, or `//`). A page must read none.
pub fn outside_urls(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    for key in ["src=", "href=", "url(", "@import"] {
        let mut rest = html;
        while let Some(i) = rest.find(key) {
            let after = rest[i + key.len()..].trim_start_matches([' ', '"', '\'']);
            let end = after.find(['"', '\'', ')', ' ', '>', ';', '\n']).unwrap_or(after.len());
            let url = &after[..end];
            let lower = url.to_ascii_lowercase();
            if lower.starts_with("http:") || lower.starts_with("https:") || lower.starts_with("//") {
                out.push(url.to_string());
            }
            rest = &rest[i + key.len()..];
        }
    }
    out
}
