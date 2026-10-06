//! The page as one HTML file (ritsu's DESIGN 4.8): ritsu's frame of a page (`docpage`: the head
//! and the palette, light and dark), a list of what the page holds, and the blocks. It reads
//! nothing from outside. The page of a rule, a dates file or a calendar opens over the page, in a
//! frame of its own whose script cannot reach the page's, as the pages of `dandori doc` open the
//! pages of their rules: the pages are held in the page as JSON, and a small script opens one.

use super::{Block, Embedded, Mark, Page};
use ritsu_base::docpage::{HTML_TAIL, Palette, esc, html_head};
use ritsu_base::json::Json;
use ritsu_base::text::Lang;

const CSS: &str = r#"* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--fg); font: 15px/1.6 system-ui, -apple-system, "Segoe UI", "Hiragino Sans", "Noto Sans JP", sans-serif; }
main { max-width: 1180px; margin: 0 auto; padding: 24px 16px 64px; }
h1 { font-size: 26px; margin: 8px 0 6px; }
h2 { font-size: 21px; margin: 44px 0 10px; padding-top: 14px; border-top: 1px solid var(--line); }
h3 { font-size: 17px; margin: 30px 0 8px; }
h4 { font-size: 14px; margin: 20px 0 6px; color: var(--dim); }
p, li { max-width: 88ch; }
p.desc { color: var(--dim); margin: 0 0 12px; }
a { color: var(--accent); }
code { font: .88em ui-monospace, SFMono-Regular, Menlo, Consolas, "Liberation Mono", monospace; background: var(--code); padding: 1px 4px; border-radius: 4px; overflow-wrap: anywhere; }
nav.toc { margin: 16px 0 8px; padding: 10px 14px; background: var(--panel); border: 1px solid var(--line); border-radius: 8px; font-size: 14px; }
nav.toc ul { margin: 0; padding-left: 18px; }
nav.toc li { max-width: none; }
.note { margin: 12px 0; padding: 8px 12px; border-left: 4px solid var(--accent); background: var(--soft); border-radius: 0 6px 6px 0; }
.scroll { overflow-x: auto; margin: 8px 0 16px; }
table { border-collapse: collapse; font-size: 13px; }
th, td { border: 1px solid var(--line); padding: 5px 9px; text-align: left; vertical-align: top; white-space: nowrap; }
th { background: var(--soft); }
td:last-child { white-space: normal; min-width: 18ch; }
tr.allow td:first-child { box-shadow: inset 4px 0 0 var(--ok); }
tr.deny td:first-child { box-shadow: inset 4px 0 0 var(--bad); }
details { margin: 8px 0 16px; padding: 6px 12px; border: 1px solid var(--line); border-radius: 8px; background: var(--panel); }
details > summary { cursor: pointer; font-weight: 600; }
details[open] > summary { margin-bottom: 6px; }
.side { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 440px), 1fr)); gap: 12px; margin: 8px 0 20px; }
figure.code { margin: 0; min-width: 0; }
figure.code figcaption { font-size: 12px; color: var(--dim); margin: 0 0 4px; }
pre { margin: 0 0 12px; padding: 10px 12px; background: var(--soft); border: 1px solid var(--line); border-radius: 8px; overflow-x: auto; font: 12.5px/1.5 ui-monospace, SFMono-Regular, Menlo, Consolas, "Liberation Mono", monospace; }
figure.code pre { margin: 0; }
pre code { background: none; padding: 0; font: inherit; }
button.open { font: inherit; font-size: 14px; padding: 5px 12px; border: 1px solid var(--accent); border-radius: 6px; background: var(--bg); color: var(--accent); cursor: pointer; }
button.open:hover, button.open:focus-visible { background: var(--soft); }
.sheet { position: fixed; inset: 0; z-index: 10; display: flex; flex-direction: column; background: var(--bg); }
.sheet[hidden] { display: none; }
.sheet .bar { display: flex; align-items: center; gap: 10px; padding: 8px 14px; border-bottom: 1px solid var(--line); background: var(--panel); font-size: 13px; }
.sheet .what { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.sheet button.close { font: inherit; font-size: 16px; line-height: 1; padding: 3px 9px; border: 1px solid var(--line); border-radius: 6px; background: var(--bg); color: var(--fg); cursor: pointer; }
.sheet button.close:hover { border-color: var(--accent); }
.sheet iframe { flex: 1; width: 100%; border: 0; }
body.sheet-open { overflow: hidden; }
:target { scroll-margin-top: 8px; }
@media print { .sheet, button.open { display: none; } }
"#;

/// Opens the page of a rule, a dates file or a calendar over the page: a button's `data-page`, or
/// `#page=<n>` (from 1) when the page loads; Escape and the close button close it.
const JS: &str = r#"(function () {
  var pages = JSON.parse(document.getElementById('sk-pages').textContent);
  var sheet = document.getElementById('sk-sheet');
  var frame = sheet.querySelector('iframe');
  var opener = null;
  function code(t) { var c = document.createElement('code'); c.textContent = t; return c; }
  function open(k) {
    var p = pages[k];
    if (!p) return;
    if (sheet.hidden) opener = document.activeElement;
    sheet.querySelector('.what').replaceChildren(code(p.file), ' · ' + p.tool + ' doc');
    if (p.page !== null) {
      frame.srcdoc = p.page;
    } else {
      var pre = document.createElement('pre');
      pre.textContent = p.error;
      pre.setAttribute('style', 'white-space:pre-wrap;font:13px ui-monospace,monospace;padding:16px');
      frame.srcdoc = '<!doctype html><meta charset="utf-8">' + pre.outerHTML;
    }
    sheet.hidden = false;
    document.body.classList.add('sheet-open');
    sheet.querySelector('.close').focus();
    try { history.replaceState(null, '', '#page=' + (k + 1)); } catch (e) {}
  }
  function close() {
    if (sheet.hidden) return;
    sheet.hidden = true;
    frame.srcdoc = '';
    document.body.classList.remove('sheet-open');
    try { history.replaceState(null, '', location.pathname + location.search); } catch (e) {}
    if (opener && opener.focus) opener.focus();
  }
  document.addEventListener('click', function (ev) {
    var b = ev.target.closest && ev.target.closest('button[data-page]');
    if (b) open(+b.dataset.page);
  });
  sheet.querySelector('.close').addEventListener('click', close);
  document.addEventListener('keydown', function (ev) { if (ev.key === 'Escape') close(); });
  var m = /^#page=(\d+)$/.exec(location.hash);
  if (m) open(+m[1] - 1);
})();
"#;

/// Text with `code` in backquotes, as HTML.
pub fn inline(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(t) = rest.find('`') {
        let after = &rest[t + 1..];
        let Some(j) = after.find('`') else { break };
        out.push_str(&esc(&rest[..t]));
        out.push_str(&format!("<code>{}</code>", esc(&after[..j])));
        rest = &after[j + 1..];
    }
    out.push_str(&esc(rest));
    out
}

/// What names the page of a rule, a dates file or a calendar: the summary of its fold in the
/// Markdown, the button that opens it in the HTML. As HTML.
pub fn embedded_label(lang: Lang, e: &Embedded) -> String {
    let file = format!("<code>{}</code>", esc(&e.file));
    let tool = format!("<code>{} doc</code>", e.tool);
    match lang {
        Lang::En => format!("The page of {file}, as {tool} draws it"),
        Lang::Ja => format!("{tool} が描いた {file} のページ"),
    }
}

/// The text of a `<script type="application/json">`: a `<` is written `\u003c`, which a JSON
/// string reads as `<`, so nothing the pages hold ends the element or begins a comment in it.
fn json_in_script(j: &Json) -> String {
    j.compact().replace('<', concat!("\\", "u003c"))
}

pub fn write(p: &Page) -> String {
    let version = env!("CARGO_PKG_VERSION");
    let css = format!("{}{CSS}", Palette::default().css());
    let mut out = html_head(p.lang, &format!("sekisho {version}"), &p.title, &css);
    out.push_str("<main>\n<header>\n");
    out.push_str(&format!("<h1>{}</h1>\n", esc(&p.title)));
    if let Some(d) = &p.description {
        out.push_str(&format!("<p class=\"desc\">{}</p>\n", esc(d)));
    }
    out.push_str("</header>\n");
    // what comes before the first section is the head of the page: the list of what it holds goes
    // after it
    let first = p.blocks.iter().position(|b| matches!(b, Block::Heading { level: 2, .. })).unwrap_or(p.blocks.len());
    for b in &p.blocks[..first] {
        block(&mut out, p, b);
    }
    out.push_str(&toc(p));
    for b in &p.blocks[first..] {
        block(&mut out, p, b);
    }
    if !p.embedded.is_empty() {
        let close = match p.lang {
            Lang::En => "Close",
            Lang::Ja => "閉じる",
        };
        out.push_str(&format!(
            "<div class=\"sheet\" id=\"sk-sheet\" role=\"dialog\" aria-modal=\"true\" aria-label=\"doc\" hidden><div class=\"bar\"><span class=\"what\"></span><button class=\"close\" type=\"button\" title=\"{close}\" aria-label=\"{close}\">×</button></div><iframe sandbox=\"allow-scripts\" title=\"doc\"></iframe></div>\n"
        ));
        let data = Json::arr(p.embedded.iter().map(|e| {
            let (page, error) = match &e.page {
                Ok(t) => (Json::str(t.clone()), Json::Null),
                Err(why) => (Json::Null, Json::str(match p.lang {
                    Lang::En => format!("{} could not draw the page of {}.\n\n{why}", e.tool, e.file),
                    Lang::Ja => format!("{} は、{} のページを描けませんでした。\n\n{why}", e.tool, e.file),
                })),
            };
            Json::obj([("tool", Json::str(e.tool)), ("file", Json::str(e.file.clone())), ("page", page), ("error", error)])
        }));
        out.push_str(&format!("<script type=\"application/json\" id=\"sk-pages\">{}</script>\n<script>\n{JS}</script>\n", json_in_script(&data)));
    }
    out.push_str("</main>\n");
    out.push_str(HTML_TAIL);
    out
}

/// The list of what the page holds: each section, and the actions under the first.
fn toc(p: &Page) -> String {
    let label = match p.lang {
        Lang::En => "Contents",
        Lang::Ja => "目次",
    };
    let mut sections: Vec<(&str, &str, Vec<(&str, &str)>)> = Vec::new();
    for b in &p.blocks {
        let Block::Heading { level, text, id } = b else { continue };
        match level {
            2 => sections.push((id, text, Vec::new())),
            3 => {
                if let Some(last) = sections.last_mut()
                    && last.0 == "actions"
                {
                    last.2.push((id, text));
                }
            }
            _ => {}
        }
    }
    let mut out = format!("<nav class=\"toc\" aria-label=\"{label}\">\n<ul>\n");
    for (id, text, under) in sections {
        out.push_str(&format!("<li><a href=\"#{}\">{}</a>", esc(id), inline(text)));
        if !under.is_empty() {
            out.push_str("\n<ul>\n");
            for (id, text) in under {
                out.push_str(&format!("<li><a href=\"#{}\">{}</a></li>\n", esc(id), inline(text)));
            }
            out.push_str("</ul>\n");
        }
        out.push_str("</li>\n");
    }
    out.push_str("</ul>\n</nav>\n");
    out
}

fn block(out: &mut String, p: &Page, b: &Block) {
    match b {
        Block::Heading { level, text, id } => out.push_str(&format!("<h{level} id=\"{}\">{}</h{level}>\n", esc(id), inline(text))),
        Block::Para(text) => out.push_str(&format!("<p>{}</p>\n", inline(text))),
        Block::Note(text) => out.push_str(&format!("<p class=\"note\">{}</p>\n", inline(text))),
        Block::List(items) => {
            out.push_str("<ul>\n");
            for i in items {
                out.push_str(&format!("<li>{}</li>\n", inline(i)));
            }
            out.push_str("</ul>\n");
        }
        Block::Table { head, rows, marks } => {
            out.push_str("<div class=\"scroll\"><table>\n<thead><tr>");
            for h in head {
                out.push_str(&format!("<th>{}</th>", inline(h)));
            }
            out.push_str("</tr></thead>\n<tbody>\n");
            for (r, m) in rows.iter().zip(marks) {
                let class = match m {
                    Mark::Allow => " class=\"allow\"",
                    Mark::Deny => " class=\"deny\"",
                    Mark::Plain => "",
                };
                out.push_str(&format!("<tr{class}>"));
                for c in r {
                    out.push_str(&format!("<td>{}</td>", inline(c)));
                }
                out.push_str("</tr>\n");
            }
            out.push_str("</tbody>\n</table></div>\n");
        }
        Block::Fold { summary, body } => {
            out.push_str(&format!("<details>\n<summary>{}</summary>\n", inline(summary)));
            for x in body {
                block(out, p, x);
            }
            out.push_str("</details>\n");
        }
        Block::Code { lang, caption, text } => {
            if caption.is_empty() {
                out.push_str(&format!("<pre><code class=\"language-{lang}\">{}</code></pre>\n", esc(text)));
            } else {
                out.push_str(&format!("<figure class=\"code\"><figcaption><code>{}</code></figcaption><pre><code class=\"language-{lang}\">{}</code></pre></figure>\n", esc(caption), esc(text)));
            }
        }
        Block::Side(xs) => {
            out.push_str("<div class=\"side\">\n");
            for x in xs {
                block(out, p, x);
            }
            out.push_str("</div>\n");
        }
        Block::Embed(k) => {
            let e = &p.embedded[*k];
            let open = match p.lang {
                Lang::En => format!("Open the page of <code>{}</code> (<code>{} doc</code>)", esc(&e.file), e.tool),
                Lang::Ja => format!("<code>{}</code> のページを開く（<code>{} doc</code>）", esc(&e.file), e.tool),
            };
            out.push_str(&format!("<p><button class=\"open\" type=\"button\" data-page=\"{k}\">{open}</button></p>\n"));
        }
    }
}
