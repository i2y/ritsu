//! Whether Mermaid draws a page's charts (dandori's and chobo's): every chart of a Markdown
//! page, rendered in headless Chrome by Mermaid 11 and 12 (`tools/mermaid` in the crate:
//! `npm ci --prefix tools/mermaid` puts each in `node_modules/mermaid-<major>`).

use crate::tmp::TempDir;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The majors a chart must draw in.
pub const MAJORS: [&str; 2] = ["11", "12"];

/// The charts of a Markdown page: what is between a line ```` ```mermaid ```` and a line
/// ```` ``` ````.
pub fn charts(markdown: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut open: Option<String> = None;
    for line in markdown.lines() {
        match (&mut open, line) {
            (None, "```mermaid") => open = Some(String::new()),
            (Some(c), "```") => {
                out.push(std::mem::take(c));
                open = None;
            }
            (Some(c), l) => {
                c.push_str(l);
                c.push('\n');
            }
            (None, _) => {}
        }
    }
    out
}

/// The script of each major under `node_modules` (`tools/mermaid/node_modules`), or why it is
/// not there (for a SKIP line).
pub fn scripts(node_modules: &Path) -> Result<Vec<(&'static str, PathBuf)>, String> {
    let mut out = Vec::new();
    for major in MAJORS {
        let s = node_modules.join(format!("mermaid-{major}/dist/mermaid.min.js"));
        if !s.is_file() {
            return Err(format!("{} is missing; run `npm ci --prefix tools/mermaid`", s.display()));
        }
        out.push((major, s));
    }
    Ok(out)
}

/// A JSON string, for the page that holds the charts.
fn json_string(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// What one Mermaid made of each chart, in order: `ok`, or `error: <what it said>`. None when
/// the page did not answer for every chart.
pub fn draw(chrome: &Path, script: &Path, charts: &[String]) -> Option<Vec<String>> {
    let work = TempDir::new("mermaid");
    let list = format!("[{}]", charts.iter().map(|c| json_string(c)).collect::<Vec<_>>().join(",")).replace("</", "<\\/");
    let page = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><script src=\"file://{}\"></script></head><body><pre id=\"out\">not run</pre><script>\nconst charts = {list};\nmermaid.initialize({{ startOnLoad: false, securityLevel: 'strict' }});\n(async () => {{ const out = []; for (let k = 0; k < charts.length; k++) {{ try {{ await mermaid.render('c' + k, charts[k]); out.push('ok'); }} catch (e) {{ out.push('error: ' + String(e && e.message || e).split('\\n').join(' ')); }} }} document.getElementById('out').textContent = out.join('\\n'); }})();\n</script></body></html>",
        script.display()
    );
    let at = work.write("charts.html", page);
    let dom = crate::chrome::dump_dom(chrome, &format!("file://{}", at.display()), 20000, Duration::from_secs(90));
    let body = dom.split("<pre id=\"out\">").nth(1).and_then(|r| r.split("</pre>").next())?;
    let results: Vec<String> = body.lines().map(String::from).collect();
    (results.len() == charts.len()).then_some(results)
}
