//! The pages of `ritsu mod site` (DESIGN 17.17): the list, each module, each version and each file,
//! and the stylesheets and the script they share under `_site/`.
//!
//! A module's page is the page of its newest version, and a version's page holds six tabs: what the
//! module is, what its files hold, a place to try them, what changed since the version before, its
//! versions, and its files. They are sections of one page: without a script all of them show one
//! after another, and with it the address's `#` picks one.
//!
//! A file has a page: what its language draws from it, a Markdown file as it reads, or its code for
//! any other. A file with a page of the first two kinds has its code on a page too, under `code/`,
//! and a switch on each leads to the other.

use super::changes;
use super::code;
use super::holds::{self, Held, Kind, Value};
use super::markdown::{inline, readme_html};
use super::{List, Shelf, Shown};
use crate::modules::diff::{self, ModDiff};
use ritsu_base::docpage::{self, esc};
use ritsu_base::json::Json;
use ritsu_base::modules::manifest::{self, Policy, Signature};
use ritsu_base::modules::names;
use ritsu_base::modules::store::{self, Store};
use ritsu_base::modules::digest;
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text, plural};
use ritsu_base::tr;
use ritsu_project::Joined;
use std::path::{Path, PathBuf};

/// The most characters the packed files of a version may have in the link that opens it (DESIGN
/// 17.17).
const LINK_MAX: usize = 1_000_000;

/// The directory of what the pages share.
const SHARED: &str = "_site";

/// ritsu's documents, in each language.
const DOCS_EN: &str = "https://i2y.github.io/ritsu/";
const DOCS_JA: &str = "https://i2y.github.io/ritsu/ja/";

/// The stylesheet of the frame every page has, a file's page too: it names nothing but its own `rm-`
/// classes, so a page a language drew keeps its look.
const FRAME_CSS: &str = include_str!("frame.css");
/// The stylesheet of the site's own pages, after the colors of [`colors`].
const SITE_CSS: &str = include_str!("site.css");
/// What the pages do: the theme, the tabs, the search, the copy buttons, the version's menu, trying a
/// file and opening a version in the playground.
const SITE_JS: &str = include_str!("site.js");

/// The script at the top of every page, so that the first paint has the reader's theme and tab.
const HEAD_JS: &str = "(function(){var d=document.documentElement;d.className+=\" js\";try{var t=localStorage.getItem(\"ritsu-theme\");if(t)d.setAttribute(\"data-theme\",t)}catch(e){}var l=d.getAttribute(\"data-tabs\");if(l){var h=location.hash.slice(1).split(\":\")[0],a=l.split(\" \");d.setAttribute(\"data-tab\",a.indexOf(h)>=0?h:a[0])}if(/[?&]view=try/.test(location.search))d.className+=\" rm-bare\"})();";

/// The colors of the site's pages, light and dark.
fn colors() -> docpage::Palette {
    docpage::Palette::default()
        .set("fg", "#14171a", "#e9ecef")
        .set("bg", "#ffffff", "#0e1214")
        .set("dim", "#59626d", "#a3acb6")
        .set("line", "#e2e5e9", "#2a3036")
        .set("soft", "#f4f5f7", "#161b1f")
        .set("panel", "#f4f5f7", "#161b1f")
        .set("code", "#eef0f3", "#1e2429")
        .set("accent", "#0a7268", "#4bd9c7")
        .set("accentsoft", "#dff3f0", "#10302c")
        .set("accentfg", "#ffffff", "#04201c")
        .set("warn", "#8a5200", "#f0b35a")
        .set("warnsoft", "#fbf0dc", "#2e2411")
        .set("bad", "#b3261e", "#ff8f80")
        .set("badsoft", "#fbe9e7", "#3a1a17")
        .set("ck", "#7a3e9d", "#c792ea")
        .set("cs", "#2e7d32", "#9ccc65")
        .set("cn", "#b5551d", "#f0a35e")
        .set("cp", "#6a7780", "#8d9aa1")
}

/// `s` with every byte a path of a URL does not take as it is written `%XX`: how a link names a
/// file of a module (`税/規則/付加価値税率.rule`) or a module whose name has a port in it.
pub(super) fn url_path(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/') {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

/// `../` once for each part of `rel` (a directory, `/` between its parts): from it back up.
pub(super) fn up(rel: &str) -> String {
    "../".repeat(rel.split('/').filter(|p| !p.is_empty()).count())
}

/// The directory of a file's path (`""` for one at the root).
pub(super) fn parent_of(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(d, _)| d)
}

/// The name a module's `require` line gives it on its page: the name the list says, else the last
/// part of its name, in the letters a dependency's name takes.
pub(super) fn short_name(module: &str) -> String {
    let last = module.rsplit('/').next().unwrap_or(module);
    let mut s: String = last.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
    if !s.starts_with(|c: char| c.is_ascii_lowercase()) {
        s.insert(0, 'm');
    }
    if names::dependency_ok(&s).is_ok() { s } else { "dependency".to_string() }
}

/// The start of a digest a page shows, the whole in its title.
fn digest_cell(d: &str) -> String {
    format!("<code title=\"{}\">{}</code>", esc(d), esc(&diff::digest_head(d)))
}

/// The attribute that says a text is in Japanese, on a page in English that names a file or a day
/// in it.
fn lang_attr(s: &str, page: Lang) -> &'static str {
    if page == Lang::En && !s.is_ascii() { " lang=\"ja\"" } else { "" }
}

/// A module's name, as its start and its last part.
fn split_name(name: &str) -> (&str, &str) {
    match name.rfind('/') {
        Some(i) => name.split_at(i + 1),
        None => ("", name),
    }
}

/// A tick, drawn.
const TICK: &str = "<svg class=\"tick\" width=\"16\" height=\"16\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2.4\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\"><path d=\"M5 12.5l4.5 4.5L19 7.5\"></path></svg>";

/// What `ritsu check` says of a version, run in its directory when the site is built.
pub(super) struct Check {
    /// Its own files it checked.
    files: usize,
    errors: usize,
    /// What it printed, when it does not pass.
    said: String,
}

/// `ritsu check .` in the directory of a version, and what it says; None when it could not be run
/// or printed nothing a page can read.
fn check_of(root: &Path, lang: Lang) -> Option<Check> {
    let exe = std::env::current_exe().ok()?;
    let run = |format: &str| std::process::Command::new(&exe).current_dir(root).args(["check", ".", "--format", format, "--lang", lang.code()]).output().ok();
    let o = run("json")?;
    let j = ritsu_base::json::parse(String::from_utf8_lossy(&o.stdout).trim()).ok()?;
    let ok = j.get("ok").and_then(Json::as_bool)?;
    let own = |f: &&Json| f.get("dependency").is_none_or(Json::is_null);
    let listed = j.get("files").and_then(Json::as_arr).unwrap_or(&[]);
    let files = listed.iter().filter(own).count();
    let bad = listed.iter().filter(own).filter(|f| f.get("ok").and_then(Json::as_bool) == Some(false)).count();
    if ok {
        return Some(Check { files, errors: 0, said: String::new() });
    }
    let said = run("text").map(|t| String::from_utf8_lossy(&t.stdout).into_owned() + &String::from_utf8_lossy(&t.stderr)).unwrap_or_default();
    Some(Check { files, errors: bad.max(1), said })
}

/// A version, read for its pages.
struct Read<'a> {
    shown: &'a Shown,
    holds: Vec<Held>,
    check: Option<Check>,
    /// The version before it, and what changed since.
    changes: Option<(String, ModDiff)>,
}

/// One thing the site says it checked of a version.
struct Proof {
    /// With a tick: it holds. Without: it is only said.
    ok: bool,
    html: String,
}

/// Where a page stands: from it, the site's root, the module's directory and the version's.
struct Place {
    root: String,
    module: String,
    version: String,
    /// The page's own path from the root, for the link to the same page in the other language.
    rel: String,
}

/// What writes the pages.
pub(super) struct Writer<'a> {
    pub out: PathBuf,
    pub lang: Lang,
    pub list: &'a List,
    pub playground: String,
    pub alternate: Option<(Lang, String)>,
    pub joined: Joined,
    /// How many pages were written.
    pub written: usize,
}

impl Writer<'_> {
    fn say(&self, t: Text) -> String {
        t.get(self.lang).to_string()
    }

    fn title(&self) -> String {
        self.list.title.clone().unwrap_or_else(|| self.say(tr!("モジュール", "Modules")))
    }

    /// A file at `rel` (from the site's root), its directories made.
    fn write(&mut self, rel: &str, body: &str) -> Result<(), Text> {
        let p = self.out.join(rel);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| {
                let d = d.display();
                tr!("{d} を作れません（{e}）", "cannot make {d} ({e})")
            })?;
        }
        std::fs::write(&p, body).map_err(|e| {
            let p = p.display();
            tr!("{p} に書けません（{e}）", "cannot write {p} ({e})")
        })?;
        if rel.ends_with(".html") {
            self.written += 1;
        }
        Ok(())
    }

    /// The band at the top of every page.
    fn bar(&self, root: &str, rel: &str) -> String {
        let docs = if self.lang == Lang::Ja { DOCS_JA } else { DOCS_EN };
        let other = match &self.alternate {
            Some((l, base)) => {
                let (word, code) = if *l == Lang::Ja { ("日本語", "ja") } else { ("English", "en") };
                let page = if rel == "index.html" { "index.html".to_string() } else { url_path(rel) };
                format!("<a class=\"rm-link\" href=\"{}{}/{}\" lang=\"{code}\" hreflang=\"{code}\">{word}</a>", esc(root), esc(base.trim_end_matches('/')), esc(&page))
            }
            None => String::new(),
        };
        let placeholder = self.say(tr!("モジュール、規則、カレンダーを探す…", "Search modules, rules, calendars…"));
        let label = self.say(tr!("モジュール、規則、カレンダーを探す", "Search modules, rules and calendars"));
        let theme = self.say(tr!("明るい表示と暗い表示を切り替える", "Switch between light and dark"));
        // a list with a name of its own goes by it: it may not be ritsu's
        let brand = match &self.list.title {
            Some(title) => format!("<span class=\"rm-own\" title=\"{0}\">{0}</span>", esc(title)),
            None => "<span class=\"rm-word\">ritsu</span><span class=\"rm-sub\">modules</span>".to_string(),
        };
        format!(
            "<div class=\"rm-bar\" role=\"banner\"><div class=\"rm-barin\">\n<a class=\"rm-brand\" href=\"{root}index.html\">{brand}</a>\n<form class=\"rm-search\" action=\"{root}index.html\" method=\"get\" role=\"search\"><label class=\"rm-vh\" for=\"rm-q\">{}</label><input id=\"rm-q\" name=\"q\" type=\"search\" placeholder=\"{}\" autocomplete=\"off\"></form>\n<nav class=\"rm-nav\" aria-label=\"ritsu\"><a class=\"rm-link\" href=\"{}\">{}</a><a class=\"rm-link\" href=\"{}\">{}</a>{other}<button class=\"rm-theme\" type=\"button\" aria-label=\"{}\"><svg width=\"18\" height=\"18\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.8\" aria-hidden=\"true\"><circle cx=\"12\" cy=\"12\" r=\"9\"></circle><path d=\"M12 3a9 9 0 0 1 0 18z\" fill=\"currentColor\" stroke=\"none\"></path></svg></button></nav>\n</div></div>\n",
            esc(&label),
            esc(&placeholder),
            esc(docs),
            esc(&self.say(tr!("ドキュメント", "Docs"))),
            esc(&self.playground),
            esc(&self.say(tr!("ブラウザで試す", "Playground"))),
            esc(&theme)
        )
    }

    /// The end of a page's content.
    fn footer(&self, more: Option<&str>) -> String {
        let v = env!("CARGO_PKG_VERSION");
        let made = self.say(tr!("`ritsu mod site` {v} が書きました。ほかのサーバーからは何も読み込みません。", "Built by `ritsu mod site` {v}. These pages load nothing from other servers."));
        let more = more.map(|m| format!(" {}", inline(m))).unwrap_or_default();
        format!("<footer class=\"wrap foot\">{}{more}</footer>\n", inline(&made))
    }

    /// A page of the site: the top, the band, and the body, which holds `<main>`.
    fn page(&self, title: &str, description: Option<&str>, place: &Place, tabs: Option<&str>, body: &str) -> String {
        let root = &place.root;
        let tabs_attr = tabs.map(|t| format!(" data-tabs=\"{t}\"")).unwrap_or_default();
        let desc = description.map(|d| format!("<meta name=\"description\" content=\"{}\">\n", esc(d))).unwrap_or_default();
        let mut o = format!(
            "<!doctype html>\n<html lang=\"{}\"{tabs_attr}>\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"generator\" content=\"ritsu mod site {}\">\n<title>{}</title>\n{desc}<link rel=\"stylesheet\" href=\"{root}{SHARED}/frame.css\">\n<link rel=\"stylesheet\" href=\"{root}{SHARED}/site.css\">\n<script>{HEAD_JS}</script>\n<script src=\"{root}{SHARED}/site.js\" defer></script>\n</head>\n<body>\n",
            self.lang.code(),
            env!("CARGO_PKG_VERSION"),
            esc(title)
        );
        o.push_str(&self.bar(root, &place.rel));
        o.push_str(body);
        o.push_str(docpage::HTML_TAIL);
        o
    }

    /// A block with a command or some lines to copy.
    fn step(&self, label: &str, code: &str) -> String {
        format!("<div class=\"step\"><div class=\"stephead\"><p>{}</p>{}</div>\n<pre class=\"code\"><code>{}</code></pre></div>\n", inline(label), self.copy_button(), esc(code))
    }

    fn copy_button(&self) -> String {
        let (c, d, f) = (self.say(tr!("コピー", "Copy")), self.say(tr!("コピーしました", "Copied")), self.say(tr!("コピーできませんでした", "Copy failed")));
        format!("<button class=\"copy\" type=\"button\" data-copy=\"{0}\" data-copied=\"{1}\" data-failed=\"{2}\">{0}</button>", esc(&c), esc(&d), esc(&f))
    }

    // ── Everything ──────────────────────────────────────────────────────

    /// Every page.
    pub(super) fn all(&mut self, shelves: &[Shelf]) -> Result<(), Text> {
        // published from a branch, a site goes through Jekyll unless this file is at its root, and
        // Jekyll leaves out what starts with `_`
        self.write(".nojekyll", "")?;
        self.write(&format!("{SHARED}/frame.css"), FRAME_CSS)?;
        self.write(&format!("{SHARED}/site.css"), &format!("{}{SITE_CSS}", colors().css()))?;
        self.write(&format!("{SHARED}/site.js"), SITE_JS)?;
        // each version read, and the pages of its files written, oldest first
        let mut read: Vec<Vec<Read<'_>>> = Vec::new();
        for s in shelves {
            let mut versions: Vec<Read<'_>> = Vec::new();
            let mut before: Option<&Shown> = None;
            for v in s.versions.iter().filter(|v| v.module.is_some()) {
                versions.push(self.read_version(s, v, before)?);
                before = Some(v);
            }
            read.push(versions);
        }
        let index = self.index(shelves, &read);
        self.write("index.html", &index)?;
        for (s, versions) in shelves.iter().zip(&read) {
            let rel = format!("{}/index.html", s.name);
            let place = Place { root: up(&s.name), module: "./".to_string(), version: versions.last().map(|r| format!("./{}/", url_path(&r.shown.version))).unwrap_or_default(), rel: rel.clone() };
            let page = self.module_page(s, versions, versions.len().checked_sub(1), &place);
            self.write(&rel, &page)?;
            for i in 0..versions.len() {
                let v = &versions[i].shown.version;
                let dir = format!("{}/{v}", s.name);
                let rel = format!("{dir}/index.html");
                let place = Place { root: up(&dir), module: "../".to_string(), version: "./".to_string(), rel: rel.clone() };
                let page = self.module_page(s, versions, Some(i), &place);
                self.write(&rel, &page)?;
            }
        }
        Ok(())
    }

    /// A version read: the pages of its files written, its `module.json`, what its files hold, what
    /// `ritsu check` says of it, and what changed since the version before.
    fn read_version<'s>(&mut self, s: &Shelf, v: &'s Shown, before: Option<&Shown>) -> Result<Read<'s>, Text> {
        let m = v.module.as_ref().expect("a version with its files");
        let dir = format!("{}/{}", s.name, v.version);
        let mut holds: Vec<Held> = Vec::new();
        for (rel, bytes) in &m.files {
            let mut h = holds::held(m, rel, self.lang, &self.joined);
            let tool = ritsu_project::project::kind_of(rel).filter(|t| has_doc(*t));
            let doc = tool.map(|t| doc_of(&m.root, t, rel, self.lang));
            if let (Some(h), Some(Ok(html))) = (h.as_mut(), doc.as_ref()) {
                h.has_page = true;
                h.can_try = html.contains("<section id=\"try\"");
            }
            let page = self.file_page(s, v, rel, bytes, doc);
            self.write(&format!("{dir}/files/{rel}.html"), &page)?;
            if has_code_page(rel, bytes) {
                let page = self.code_page(s, v, rel, &String::from_utf8_lossy(bytes));
                self.write(&format!("{dir}/code/{rel}.html"), &page)?;
            }
            holds.extend(h);
        }
        holds.sort_by(|a, b| (a.kind, &a.rel).cmp(&(b.kind, &b.rel)));
        if !v.paths {
            let json = module_json(m, &v.deps)?;
            self.write(&format!("{dir}/module.json"), &json)?;
        }
        let changes = before.and_then(|p| p.module.as_ref().map(|pm| (p.version.clone(), diff::modules(pm, m, None, self.lang, &self.joined))));
        Ok(Read { shown: v, holds, check: check_of(&m.root, self.lang), changes })
    }

    /// The list's `signature` lines that cover the module `name`.
    fn lines_for(&self, name: &str) -> Vec<&Signature> {
        self.list.signatures.iter().filter(|s| names::covers(&s.prefix, name)).collect()
    }

    /// Who the list's lines say signed a module, in a few words: `i2y/ritsu` for a workflow of a
    /// repository, when the line's pattern says one.
    fn signer(&self, line: &Signature) -> Option<String> {
        let Policy::Matching { pattern, issuer } = &line.policy else { return None };
        if issuer != "https://token.actions.githubusercontent.com" {
            return None;
        }
        let rest = pattern.strip_prefix("^https://github[.]com/")?;
        let rest = rest.trim_end_matches('/');
        let plain = !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/'));
        plain.then(|| rest.to_string())
    }

    /// What is said of how a version was checked, in a few words each.
    fn proofs(&self, s: &Shelf, r: &Read<'_>) -> Vec<Proof> {
        let mut out = Vec::new();
        let v = r.shown;
        if !v.published {
            out.push(Proof { ok: false, html: esc(&self.say(tr!("まだ公開していません", "Not published yet"))) });
        } else if s.git.is_some() {
            out.push(Proof { ok: true, html: esc(&self.say(tr!("git のコミットとダイジェストで固定", "Held by its git commit and digests"))) });
        } else {
            let lines = self.lines_for(&s.name);
            match lines.first() {
                None => out.push(Proof { ok: false, html: esc(&self.say(tr!("一覧の `signature` の行が覆っていません", "No signature line of the list covers it")).replace('`', "")) }),
                Some(l) => {
                    let said = match self.signer(l) {
                        Some(who) => tr!("{who} の署名を確かめました", "Signed by {who}"),
                        None => tr!("署名を確かめました", "Signature checked"),
                    };
                    out.push(Proof { ok: true, html: esc(&self.say(said)) });
                }
            }
            out.push(Proof { ok: true, html: esc(&self.say(tr!("ダイジェストが一致", "Digests match"))) });
        }
        if let Some(c) = &r.check {
            let n = c.files;
            if c.errors == 0 {
                if n > 0 {
                    out.push(Proof { ok: true, html: inline(&self.say(tr!("`ritsu check` を通ります", "Passes `ritsu check`"))) });
                }
            } else {
                let e = c.errors;
                out.push(Proof { ok: false, html: format!("<span class=\"warn\">{}</span>", inline(&self.say(tr!("`ritsu check` でエラーが {e} 件", "`ritsu check` finds {e} with errors")))) });
            }
        }
        let pinned = holds::pinned(&r.holds);
        if !pinned.is_empty() {
            let n = pinned.len();
            let w = plural(n, "data source", "data sources");
            out.push(Proof { ok: true, html: esc(&self.say(tr!("データ {n} 件を SHA-256 で固定", "{w} pinned by SHA-256"))) });
        }
        out
    }

    // ── The list ────────────────────────────────────────────────────────

    fn index(&self, shelves: &[Shelf], read: &[Vec<Read<'_>>]) -> String {
        let title = self.title();
        let description = self.list.description.clone().unwrap_or_else(|| self.say(tr!("読んで、試して、確かめてから依存できる規則、カレンダー、ワークフロー。", "Rules, calendars and workflows you can read, try and check before you depend on them.")));
        let mut b = String::from("<main>\n<div class=\"wrap body\"><div class=\"cols\">\n<section class=\"primary\" aria-labelledby=\"list-title\">\n");
        b.push_str(&format!("<h1 id=\"list-title\" class=\"listtitle\">{}</h1>\n<p class=\"lead dim\">{}</p>\n", esc(&title), esc(&description)));
        // the filters, by what the modules hold
        let mut kinds: Vec<(Kind, usize)> = Vec::new();
        for versions in read {
            if let Some(r) = versions.last() {
                for (k, _) in holds::tally(&r.holds) {
                    match kinds.iter_mut().find(|(x, _)| *x == k) {
                        Some((_, n)) => *n += 1,
                        None => kinds.push((k, 1)),
                    }
                }
            }
        }
        kinds.sort();
        if kinds.len() > 1 {
            b.push_str(&format!("<div class=\"filters\" role=\"group\" aria-label=\"{}\">\n", esc(&self.say(tr!("中身で絞り込む", "Filter by what a module holds")))));
            b.push_str(&format!("<button class=\"fil on\" type=\"button\" data-kind=\"\" aria-pressed=\"true\">{}<span class=\"count\">{}</span></button>\n", esc(&self.say(tr!("すべて", "All"))), shelves.len()));
            for (k, n) in &kinds {
                b.push_str(&format!("<button class=\"fil\" type=\"button\" data-kind=\"{}\" aria-pressed=\"false\">{}<span class=\"count\">{n}</span></button>\n", k.word(), esc(&self.say(k.many()))));
            }
            b.push_str("</div>\n");
        }
        let nothing = self.say(tr!("「%s」に当てはまるものはありません。", "Nothing matches “%s”."));
        let reads = self.say(tr!("モジュールの名前と説明、中の規則やカレンダーの名前を探します。", "Search reads the names of modules, what they say about themselves, and the names of the rules and calendars they hold."));
        b.push_str(&format!("<div class=\"empty\" hidden data-text=\"{}\"><p class=\"emptyhead\"></p><p class=\"dim\">{}</p></div>\n", esc(&nothing), esc(&reads)));
        b.push_str("<div class=\"rows\">\n");
        for (s, versions) in shelves.iter().zip(read) {
            b.push_str(&self.row(s, versions));
        }
        b.push_str("</div>\n</section>\n");
        // what the list is
        b.push_str("<aside class=\"side\">\n");
        b.push_str(&format!("<h2>{}</h2>\n<p class=\"small dim\">{}</p>\n", esc(&self.say(tr!("モジュールの置き場所", "Where modules live"))), inline(&self.say(tr!(
            "モジュールは、作者が公開したところにあります。OCI のレジストリか、git のリポジトリのタグです。このサイトはビルドのときに版を一つずつ取り出し、ダイジェストと署名を確かめて、ファイルからこのページを描きます。`ritsu mod add` も同じところから取り出すので、このサイトが止まっても困りません。",
            "Modules stay where their authors publish them: an OCI registry or the tags of a git repository. This site fetches each version when it is built, checks its digests and its signature, and draws these pages from the files. `ritsu mod add` fetches from the same place, so nothing depends on this site."
        )))));
        if let Some(url) = &self.list.contribute {
            b.push_str(&format!("<h2>{}</h2>\n", esc(&self.say(tr!("モジュールを載せる", "List your module")))));
            b.push_str(&self.step(&self.say(tr!("1. 署名を付けて、自分のレジストリに公開します。", "1. Publish it to your registry, signed in your workflow.")), "ritsu mod push"));
            let lines = "module \"ghcr.io/you/your-module\"\nsignature \"ghcr.io/you/your-module\" by matching \"^https://github[.]com/you/\" issuer \"https://token.actions.githubusercontent.com\"";
            let (u, go) = (esc(url), self.say(tr!("一覧のファイルを開く", "Open the list file")));
            b.push_str(&self.step(&self.say(tr!("2. 次の行を一覧に足すプルリクエストを送ります。CI がモジュールを取り出して確かめます。", "2. Send a pull request with these lines for the list. The check fetches your module and verifies it.")), lines));
            b.push_str(&format!("<p class=\"small\"><a href=\"{u}\">{}</a></p>\n", esc(&go)));
        }
        if !self.list.signatures.is_empty() {
            b.push_str(&format!(
                "<details class=\"small\"><summary>{}</summary>\n<p class=\"dim\">{}</p>\n<pre class=\"code\"><code>{}</code></pre>\n</details>\n",
                esc(&self.say(tr!("一覧の署名の決まり", "The list's signature rules"))),
                inline(&self.say(tr!(
                    "このサイトは、載せたモジュールの署名を次の行で確かめました。使う側の `ritsu.mod` に同じ行を書くと、ritsu は取り出すときに同じように署名を確かめます。",
                    "The site checked the signatures of the modules it shows against these lines. The same lines in a `ritsu.mod` check them the same way when they are fetched."
                ))),
                self.list.signatures.iter().map(|s| esc(&s.text())).collect::<Vec<_>>().join("\n")
            ));
        }
        b.push_str("</aside>\n</div></div>\n");
        b.push_str(&self.footer(None));
        b.push_str("</main>\n");
        let place = Place { root: "./".to_string(), module: String::new(), version: String::new(), rel: "index.html".to_string() };
        self.page(&title, Some(&description), &place, None, &b)
    }

    /// One module on the list.
    fn row(&self, s: &Shelf, versions: &[Read<'_>]) -> String {
        let newest = versions.last();
        let manifest = newest.and_then(|r| r.shown.module.as_ref()).map(|m| &m.manifest);
        let description = manifest.and_then(|m| m.description.as_ref()).map(|d| d.value.clone()).unwrap_or_default();
        let (prefix, last) = split_name(&s.name);
        let tally = newest.map(|r| holds::tally(&r.holds)).unwrap_or_default();
        let kinds: Vec<&str> = tally.iter().map(|(k, _)| k.word()).collect();
        let items: Vec<&Held> = newest.map(|r| r.holds.iter().collect()).unwrap_or_default();
        let searched: String = format!("{} {description} {}", s.name, items.iter().map(|h| h.name.as_str()).collect::<Vec<_>>().join(" ")).to_lowercase();
        let mut o = format!("<article class=\"row\" data-kinds=\"{}\" data-text=\"{}\">\n<div class=\"rowmain\">\n<div class=\"rowhead\"><h2>", kinds.join(" "), esc(&searched));
        let linked = newest.is_some() || !s.versions.is_empty();
        o.push_str(&format!("<span class=\"prefix\">{}</span>", esc(prefix)));
        if linked {
            o.push_str(&format!("<a href=\"./{}/index.html\">{}</a>", url_path(&s.name), esc(last)));
        } else {
            o.push_str(&esc(last));
        }
        o.push_str("</h2>");
        match newest {
            Some(r) => {
                o.push_str(&format!("<span class=\"ver\">{}</span>", esc(&r.shown.version)));
                if !r.shown.published {
                    o.push_str(&format!("<span class=\"tag warnish\">{}</span>", esc(&self.say(tr!("未公開", "unpublished")))));
                }
            }
            None => o.push_str("<span class=\"ver\">—</span>"),
        }
        o.push_str("</div>\n");
        if !description.is_empty() {
            o.push_str(&format!("<p class=\"desc\">{}</p>\n", esc(&description)));
        }
        if !tally.is_empty() {
            let said = Text::join(&tally.iter().map(|(k, n)| k.counted(*n)).collect::<Vec<_>>(), "、", ", ");
            o.push_str(&format!("<div class=\"items\"><span class=\"summary\">{}</span>", esc(&self.say(said))));
            for h in items.iter().take(8) {
                o.push_str(&format!("<span class=\"item\" data-name=\"{}\"{}>{}</span>", esc(&h.name.to_lowercase()), lang_attr(&h.name, self.lang), esc(&h.name)));
            }
            if items.len() > 8 {
                let more = items.len() - 8;
                o.push_str(&format!("<span class=\"summary\">{}</span>", esc(&self.say(tr!("ほか {more}", "and {more} more")))));
            }
            o.push_str("</div>\n");
        }
        if let Some((r, (_, d))) = newest.and_then(|r| r.changes.as_ref().map(|c| (r, c))) {
            let line = self.say(changes::latest_line(d, &r.shown.version));
            o.push_str(&format!("<p class=\"latest\"><span class=\"dim\">{}</span> <a href=\"./{}/index.html#changes\">{}</a></p>\n", esc(&self.say(tr!("最新の変更：", "Latest change:"))), url_path(&s.name), esc(&line)));
            if let Some((reason, more)) = changes::reason_of(d) {
                let more = if more > 0 { format!(" <span class=\"dim\">{}</span>", esc(&self.say(tr!("ほか {more} 件", "and {more} more")))) } else { String::new() };
                o.push_str(&format!("<p class=\"latest reason\"><span class=\"dim\">{}</span> {}{more}</p>\n", esc(&self.say(tr!("理由：", "Why:"))), esc(&reason)));
            }
        }
        o.push_str("</div>\n<ul class=\"proofs side\">\n");
        if s.failed() {
            o.push_str(&format!("<li><span class=\"bad\">{}</span></li>\n", esc(&self.say(tr!("確かめられない版があります", "a version could not be verified")))));
        }
        if let Some(r) = newest {
            for p in self.proofs(s, r) {
                o.push_str(&format!("<li>{}<span>{}</span></li>\n", if p.ok { TICK } else { "" }, p.html));
            }
            if let Some(d) = &r.shown.published_on {
                o.push_str(&format!("<li class=\"dim\"><span>{}</span></li>\n", esc(&self.say(tr!("公開 {d}", "Published {d}")))));
            }
        }
        if let Some(l) = manifest.and_then(|m| m.license.as_ref()) {
            o.push_str(&format!("<li class=\"license\">{}</li>\n", esc(&l.value)));
        }
        o.push_str("</ul>\n</article>\n");
        o
    }

    // ── A module, at a version ──────────────────────────────────────────

    /// The page of a module at its version `at` (an index of `versions`): None for a module with no
    /// version that could be read.
    fn module_page(&self, s: &Shelf, versions: &[Read<'_>], at: Option<usize>, place: &Place) -> String {
        let (prefix, last) = split_name(&s.name);
        let r = at.map(|i| &versions[i]);
        let manifest = r.and_then(|r| r.shown.module.as_ref()).map(|m| &m.manifest);
        let description = manifest.and_then(|m| m.description.as_ref()).map(|d| d.value.clone());
        let newest = versions.last().map(|n| n.shown.version.as_str());
        // the tabs this page has
        let can_try = r.is_some_and(|r| r.holds.iter().any(|h| h.can_try));
        let mut tabs: Vec<(&str, Text, Option<usize>)> = Vec::new();
        if let Some(r) = r {
            let m = r.shown.module.as_ref().expect("a version with its files");
            tabs.push(("overview", tr!("概要", "Overview"), None));
            tabs.push(("contents", tr!("中身", "Contents"), Some(r.holds.len())));
            if can_try {
                tabs.push(("try", tr!("試す", "Try"), None));
            }
            tabs.push(("changes", tr!("変更点", "Changes"), None));
            tabs.push(("versions", tr!("版", "Versions"), Some(s.versions.len())));
            tabs.push(("files", tr!("ファイル", "Files"), Some(m.files.len())));
        }
        let mut b = String::from("<main>\n<div class=\"head\"><div class=\"wrap\">\n");
        let crumbs: Vec<String> = prefix.trim_end_matches('/').split('/').filter(|p| !p.is_empty()).map(esc).collect();
        let modules = self.say(tr!("モジュール", "Modules"));
        b.push_str(&format!("<p class=\"crumbs\"><a href=\"{}index.html\">{}</a>{}</p>\n", place.root, esc(&modules), crumbs.iter().map(|c| format!(" / {c}")).collect::<String>()));
        b.push_str(&format!("<div class=\"title\"><h1>{}</h1>", esc(last)));
        if let Some(r) = r {
            b.push_str(&self.version_menu(s, versions, &r.shown.version, place, "version"));
            if !r.shown.published {
                b.push_str(&format!("<span class=\"tag warnish\">{}</span>", esc(&self.say(tr!("未公開", "unpublished")))));
            }
            if Some(r.shown.version.as_str()) != newest && let Some(n) = newest {
                b.push_str(&format!("<a class=\"tag\" href=\"{}index.html\">{}</a>", place.module, esc(&self.say(tr!("新しい版 {n} があります", "A newer version, {n}, is out")))));
            }
        }
        b.push_str("</div>\n");
        if let Some(d) = &description {
            b.push_str(&format!("<p class=\"lead\">{}</p>\n", esc(d)));
        }
        b.push_str("<ul class=\"proofs inline\">\n");
        if let Some(r) = r {
            for p in self.proofs(s, r) {
                b.push_str(&format!("<li>{}<span>{}</span></li>\n", if p.ok { TICK } else { "" }, p.html));
            }
            if let Some(d) = &r.shown.published_on {
                b.push_str(&format!("<li class=\"dim\"><span>{}</span></li>\n", esc(&self.say(tr!("公開 {d}", "Published {d}")))));
            }
            if let Some(need) = manifest.and_then(|m| m.ritsu.as_ref()) {
                let need = &need.value;
                b.push_str(&format!("<li class=\"dim\"><span>{}</span></li>\n", esc(&self.say(tr!("ritsu {need} 以降が要ります", "Needs ritsu {need} or later")))));
            }
            if let Some(u) = manifest.and_then(|m| m.source.as_ref()).filter(|u| is_web(&u.value)) {
                b.push_str(&format!("<li><a href=\"{}\" rel=\"nofollow noopener\">{}</a></li>\n", esc(&u.value), esc(&self.say(tr!("ソース", "Source")))));
            }
        } else if s.failed() {
            b.push_str(&format!("<li><span class=\"bad\">{}</span></li>\n", esc(&self.say(tr!("確かめられない版があります", "a version could not be verified")))));
        }
        b.push_str("</ul>\n");
        if !s.registry {
            b.push_str(&format!(
                "<p class=\"warn small\">{}</p>\n",
                esc(&self.say(tr!(
                    "このモジュールは、まだレジストリに送り出していません。ページは作業ツリーのファイルから作りました。",
                    "This module is not on a registry yet: the pages are made from the files of its working tree."
                )))
            ));
        }
        if !tabs.is_empty() {
            b.push_str(&format!("<nav class=\"tabs\" aria-label=\"{}\">\n", esc(&self.say(tr!("モジュールの中の節", "Module sections")))));
            for (id, label, n) in &tabs {
                let count = n.map(|n| format!("<span class=\"count\">{n}</span>")).unwrap_or_default();
                b.push_str(&format!("<a class=\"tab\" href=\"#{id}\" data-tab=\"{id}\">{}{count}</a>\n", esc(&self.say(label.clone()))));
            }
            b.push_str("</nav>\n");
        }
        b.push_str("</div></div>\n<div class=\"wrap body\"><div class=\"cols\">\n<div class=\"primary\">\n");
        if let Some(r) = r {
            b.push_str(&self.overview(s, r, place));
            b.push_str(&self.contents(r, place));
            if can_try {
                b.push_str(&self.try_pane(r, place));
            }
            b.push_str(&self.changes_pane(s, versions, r, place));
        }
        b.push_str(&self.versions_pane(s, versions, r, place));
        if let Some(r) = r {
            b.push_str(&self.files_pane(r, place));
        }
        b.push_str("</div>\n");
        if let Some(r) = r {
            b.push_str("<aside class=\"side\">\n");
            b.push_str(&self.use_it(s, r, place));
            b.push_str(&self.upgrade(s, r));
            b.push_str("</aside>\n");
        }
        b.push_str("</div></div>\n");
        b.push_str(&self.footer(None));
        b.push_str("</main>\n");
        let title = match r {
            Some(r) => format!("{} {}", s.name, r.shown.version),
            None => s.name.clone(),
        };
        let ids: Vec<&str> = tabs.iter().map(|(id, ..)| *id).collect();
        let tab_list = ids.join(" ");
        self.page(&title, description.as_deref(), place, if tabs.is_empty() { None } else { Some(&tab_list) }, &b)
    }

    /// The menu of a module's versions, newest first: each option says where its page is.
    fn version_menu(&self, s: &Shelf, versions: &[Read<'_>], current: &str, place: &Place, id: &str) -> String {
        let newest = versions.last().map(|n| n.shown.version.as_str());
        let mut o = format!("<label class=\"vermenu\"><span class=\"rm-vh\">{}</span><select class=\"ctl mono\" id=\"{id}\" data-go=\"\">", esc(&self.say(tr!("版", "Version"))));
        for r in versions.iter().rev() {
            let v = &r.shown.version;
            let note = if Some(v.as_str()) == newest { self.say(tr!("（最新）", " (latest)")) } else { String::new() };
            let note2 = if r.shown.published { String::new() } else { self.say(tr!("（未公開）", " (not published)")) };
            let selected = if v == current { " selected" } else { "" };
            o.push_str(&format!("<option value=\"{}{}/index.html\"{selected}>{}{}{}</option>", place.module, url_path(v), esc(v), esc(&note), esc(&note2)));
        }
        o.push_str("</select></label>");
        let _ = s;
        o
    }

    /// A link to a file's page from a version's page, given where the page stands.
    fn file_link(&self, place: &Place, rel: &str) -> String {
        format!("{}files/{}.html", place.version, url_path(rel))
    }

    /// A link to a file's code from a version's page: its own page, or the page of a file that has
    /// no other page than its code.
    fn code_link(&self, place: &Place, m: &diff::Module, rel: &str) -> String {
        if m.files.get(rel).is_some_and(|b| has_code_page(rel, b)) { format!("{}code/{}.html", place.version, url_path(rel)) } else { self.file_link(place, rel) }
    }

    // ── The tabs ────────────────────────────────────────────────────────

    fn overview(&self, s: &Shelf, r: &Read<'_>, place: &Place) -> String {
        let m = r.shown.module.as_ref().expect("a version with its files");
        let mut o = String::from("<section class=\"pane\" id=\"overview\">\n");
        if !r.holds.is_empty() {
            o.push_str(&format!("<h2>{}</h2>\n", esc(&self.say(tr!("入っているもの", "What it holds")))));
            let has_known = r.holds.iter().any(|h| h.known.is_some());
            o.push_str("<div class=\"scroll\"><table class=\"t holds\">\n<thead><tr>");
            o.push_str(&format!("<th scope=\"col\">{}</th><th scope=\"col\">{}</th>", esc(&self.say(tr!("名前", "Name"))), esc(&self.say(tr!("説明", "What it is")))));
            if has_known {
                o.push_str(&format!("<th scope=\"col\">{}</th>", esc(&self.say(tr!("データの範囲", "Known from the data")))));
            }
            o.push_str(&format!("<th scope=\"col\"><span class=\"rm-vh\">{}</span></th>", esc(&self.say(tr!("試す、コードを読む", "Try it, or read its code")))));
            o.push_str("</tr></thead>\n<tbody>\n");
            for h in &r.holds {
                let name = if h.has_page {
                    format!("<a href=\"{}\">{}</a>", esc(&self.file_link(place, &h.rel)), esc(&h.name))
                } else {
                    esc(&h.name)
                };
                let what = h.description.clone().unwrap_or_else(|| self.say(h.kind.one()));
                o.push_str(&format!("<tr><td class=\"mono strong nowrap\"{}>{name}</td><td>{}</td>", lang_attr(&h.name, self.lang), esc(&what)));
                if has_known {
                    let k = h.known.as_ref().map(|(a, z)| self.say(tr!("{a} 〜 {z}", "{a} to {z}"))).unwrap_or_default();
                    o.push_str(&format!("<td class=\"mono small nowrap\">{}</td>", esc(&k)));
                }
                let try_it = if h.can_try { format!("<a class=\"trylink\" href=\"#try:{}\">{}</a>", url_path(&h.rel), esc(&self.say(tr!("試す", "Try")))) } else { String::new() };
                o.push_str(&format!("<td class=\"right nowrap rowlinks\">{try_it}<a class=\"codelink\" href=\"{}\">{}</a></td>", esc(&self.code_link(place, m, &h.rel)), esc(&self.say(tr!("コード", "Code")))));
                o.push_str("</tr>\n");
            }
            o.push_str("</tbody>\n</table></div>\n");
        }
        // the README, as its author wrote it
        let (first, second) = if self.lang == Lang::Ja { ("README.ja.md", "README.md") } else { ("README.md", "README.en.md") };
        if let Some(bytes) = m.files.get(first).or_else(|| m.files.get(second)).or_else(|| m.files.get("README.md"))
            && let Ok(text) = std::str::from_utf8(bytes)
        {
            let (_, last) = split_name(&s.name);
            let links = |p: &str| m.files.contains_key(p).then(|| self.file_link(place, p));
            o.push_str(&format!("<div class=\"readme\">\n{}</div>\n", readme_html(text, last, &links)));
        } else if r.holds.is_empty() {
            o.push_str(&format!("<p class=\"dim\">{}</p>\n", esc(&self.say(tr!("このモジュールに README はありません。", "This module has no README.")))));
        }
        o.push_str("</section>\n");
        o
    }

    fn contents(&self, r: &Read<'_>, place: &Place) -> String {
        let m = r.shown.module.as_ref().expect("a version with its files");
        let mut o = String::from("<section class=\"pane\" id=\"contents\">\n");
        let tally = holds::tally(&r.holds);
        if tally.is_empty() {
            o.push_str(&format!("<h2>{}</h2>\n<p class=\"dim\">{}</p>\n", esc(&self.say(tr!("中身", "Contents"))), esc(&self.say(tr!("ritsu の言語のファイルはありません。", "No file in a language of ritsu.")))));
        } else {
            let said = Text::join(&tally.iter().map(|(k, n)| k.counted(*n)).collect::<Vec<_>>(), "、", ", ");
            o.push_str(&format!("<h2>{}</h2>\n<p class=\"dim\">{}</p>\n", esc(&self.say(said)), esc(&self.say(tr!("ページは、この版のファイルからそれぞれの言語が描きました。", "Each page is drawn by its language from the file at this version.")))));
        }
        for h in &r.holds {
            o.push_str("<article class=\"card\">\n<div class=\"cardhead\">");
            o.push_str(&format!("<h3 class=\"mono\"{}>{}</h3>", lang_attr(&h.name, self.lang), esc(&h.name)));
            let path = if h.has_page { format!("<a href=\"{}\">{}</a>", esc(&self.file_link(place, &h.rel)), esc(&h.rel)) } else { esc(&h.rel) };
            o.push_str(&format!("<span class=\"path\"{}>{path}</span><span class=\"tag\">{}</span></div>\n", lang_attr(&h.rel, self.lang), esc(&self.say(h.kind.one()))));
            if let Some(d) = &h.description {
                o.push_str(&format!("<p>{}</p>\n", esc(d)));
            }
            if !h.facts.is_empty() {
                o.push_str("<dl class=\"facts\">\n");
                for (k, v) in &h.facts {
                    let shown = match v {
                        Value::Text(t) => esc(t),
                        Value::Code(c) => format!("<code>{}</code>", esc(c)),
                        Value::Names(ns) => ns.iter().map(|n| format!("<code>{}</code>", esc(n))).collect::<Vec<_>>().join(" "),
                        Value::Link(u) if is_web(u) => format!("<a href=\"{0}\" rel=\"nofollow noopener\">{0}</a>", esc(u)),
                        Value::Link(u) => esc(u),
                    };
                    o.push_str(&format!("<dt>{}</dt><dd>{shown}</dd>\n", esc(k.get(self.lang))));
                }
                o.push_str("</dl>\n");
            }
            o.push_str("<p class=\"small cardlinks\">");
            if h.can_try {
                o.push_str(&format!("<a class=\"trylink\" href=\"#try:{}\">{}</a>", url_path(&h.rel), esc(&self.say(tr!("試す", "Try it")))));
            }
            o.push_str(&format!("<a class=\"codelink\" href=\"{}\">{}</a></p>\n", esc(&self.code_link(place, m, &h.rel)), esc(&self.say(tr!("コードを読む", "Read the code")))));
            o.push_str("</article>\n");
        }
        o.push_str("</section>\n");
        o
    }

    fn try_pane(&self, r: &Read<'_>, place: &Place) -> String {
        let mut o = format!("<section class=\"pane\" id=\"try\">\n<h2 class=\"rm-vh\">{}</h2>\n", esc(&self.say(tr!("試す", "Try"))));
        let files: Vec<&Held> = r.holds.iter().filter(|h| h.can_try).collect();
        // one file has nothing to choose
        let hidden = if files.len() == 1 { " hidden" } else { "" };
        o.push_str(&format!("<label class=\"tryfile\"{hidden}><span class=\"small dim\">{}</span><select class=\"ctl mono\" id=\"try-file\">", esc(&self.say(tr!("ファイル", "File")))));
        for h in &files {
            o.push_str(&format!("<option value=\"{}\" data-rel=\"{}\">{}</option>", esc(&self.file_link(place, &h.rel)), esc(&url_path(&h.rel)), esc(&h.name)));
        }
        o.push_str("</select></label>\n");
        o.push_str(&format!("<iframe class=\"tryframe\" id=\"try-frame\" title=\"{}\" hidden></iframe>\n", esc(&self.say(tr!("ファイルを試す", "Try the file")))));
        o.push_str("<noscript><ul>\n");
        for h in &files {
            o.push_str(&format!("<li><a href=\"{}#try\">{}</a></li>\n", esc(&self.file_link(place, &h.rel)), esc(&h.name)));
        }
        o.push_str("</ul></noscript>\n</section>\n");
        o
    }

    fn changes_pane(&self, s: &Shelf, versions: &[Read<'_>], r: &Read<'_>, place: &Place) -> String {
        let mut o = String::from("<section class=\"pane\" id=\"changes\">\n");
        match &r.changes {
            Some((from, d)) => {
                let to = format!(
                    "<div class=\"to\"><span class=\"small dim\">{}</span>{}</div>",
                    esc(&self.say(tr!("比べる先", "To"))),
                    self.version_menu(s, versions, &r.shown.version, place, "version-to")
                );
                let module = r.shown.module.as_ref();
                let code = |rel: &str| module.filter(|m| m.files.contains_key(rel)).map(|m| self.code_link(place, m, rel));
                o.push_str(&changes::pane(d, from, &to, self.lang, &code));
            }
            None => {
                let v = &r.shown.version;
                o.push_str(&format!("<h2>{}</h2>\n", esc(&self.say(tr!("{v} はこのサイトでいちばん古い版です", "{v} is the oldest version here")))));
                o.push_str(&format!("<p class=\"dim prose\">{}</p>\n", esc(&self.say(tr!(
                    "新しい版が公開されると、このタブに、どの入力の答えが変わるか、どの日が休みに変わるか、どの日を新しく知るかが出ます。",
                    "When a new version is published, this tab says which inputs get a different answer, which days become closed or open, and which days the calendar newly knows."
                )))));
            }
        }
        o.push_str("</section>\n");
        o
    }

    fn versions_pane(&self, s: &Shelf, versions: &[Read<'_>], r: Option<&Read<'_>>, place: &Place) -> String {
        let n = s.versions.len();
        let w = plural(n, "version", "versions");
        let mut o = format!("<section class=\"pane\" id=\"versions\">\n<h2>{}</h2>\n", esc(&self.say(tr!("版 {n} 個", "{w}"))));
        o.push_str("<div class=\"scroll\"><table class=\"t\">\n<thead><tr>");
        for h in [tr!("版", "Version"), tr!("公開", "Published"), tr!("ダイジェスト", "Digest"), tr!("ファイル", "Files"), tr!("確かめたこと", "Checked")] {
            o.push_str(&format!("<th scope=\"col\">{}</th>", esc(&self.say(h))));
        }
        o.push_str("</tr></thead>\n<tbody>\n");
        let newest = versions.last().map(|x| x.shown.version.as_str());
        for v in s.versions.iter().rev() {
            let read = versions.iter().find(|x| x.shown.version == v.version);
            let current = r.is_some_and(|x| x.shown.version == v.version);
            let mut name = if v.module.is_some() { format!("<a href=\"{}{}/index.html\">{}</a>", place.module, url_path(&v.version), esc(&v.version)) } else { esc(&v.version) };
            if Some(v.version.as_str()) == newest {
                name.push_str(&format!(" <span class=\"dim\">{}</span>", esc(&self.say(tr!("最新", "latest")))));
            }
            if current {
                name = format!("<strong>{name}</strong>");
            }
            let date = v.published_on.as_deref().map(esc).unwrap_or_else(|| "—".to_string());
            let digest = v.digest.as_deref().map(digest_cell).unwrap_or_else(|| "—".to_string());
            let files = v.module.as_ref().map(|m| ritsu_base::text::count(m.files.len() as u64)).unwrap_or_else(|| "—".to_string());
            let state = match (&v.failed, v.published) {
                (Some(t), _) => format!("<span class=\"bad\">{}</span>", inline(&self.say(tr!("確かめられません。{}", "not verified: {}", t.ja; t.en)))),
                (None, false) => format!("<span class=\"warn\">{}</span>", esc(&self.say(tr!("まだ公開していません", "not published yet")))),
                (None, true) if s.git.is_some() => esc(&self.say(tr!("コミットとダイジェスト", "its commit and digests"))),
                (None, true) if self.lines_for(&s.name).is_empty() => esc(&self.say(tr!("ダイジェスト", "its digests"))),
                (None, true) => esc(&self.say(tr!("ダイジェストと署名", "its digests and its signature"))),
            };
            let check = read.and_then(|x| x.check.as_ref()).map(|c| {
                if c.errors == 0 {
                    self.say(tr!("、`ritsu check` を通過", "; passes `ritsu check`"))
                } else {
                    self.say(tr!("、`ritsu check` でエラー", "; `ritsu check` finds errors"))
                }
            });
            let check = check.filter(|_| v.failed.is_none()).map(|c| inline(&c)).unwrap_or_default();
            o.push_str(&format!("<tr><td class=\"mono nowrap\">{name}</td><td class=\"mono small nowrap\">{date}</td><td class=\"small\">{digest}</td><td>{files}</td><td>{state}{check}</td></tr>\n"));
        }
        o.push_str("</tbody>\n</table></div>\n</section>\n");
        o
    }

    fn files_pane(&self, r: &Read<'_>, place: &Place) -> String {
        let m = r.shown.module.as_ref().expect("a version with its files");
        let mut o = String::from("<section class=\"pane\" id=\"files\">\n");
        // the files of each kind, then what the module says of itself, then the rest
        let about = |rel: &str| {
            let name = rel.rsplit('/').next().unwrap_or(rel).to_ascii_uppercase();
            rel == manifest::FILE || rel == ritsu_base::modules::lock::FILE || ["README", "LICENSE", "LICENCE", "NOTICE", "COPYING", "THIRD_PARTY", "CHANGELOG"].iter().any(|p| name.starts_with(p))
        };
        let mut groups: Vec<(String, Vec<&String>)> = Vec::new();
        for (k, _) in holds::tally(&r.holds) {
            let files: Vec<&String> = r.holds.iter().filter(|h| h.kind == k).map(|h| &h.rel).collect();
            groups.push((self.say(k.many()), files));
        }
        let held: Vec<&String> = r.holds.iter().map(|h| &h.rel).collect();
        let rest: Vec<&String> = m.files.keys().filter(|f| !held.contains(f) && !about(f) && !diff::is_record(f)).collect();
        if !rest.is_empty() {
            groups.push((self.say(tr!("データとそのほかのファイル", "Data and other files")), rest));
        }
        let records: Vec<&String> = m.files.keys().filter(|f| diff::is_record(f)).collect();
        if !records.is_empty() {
            groups.push((self.say(tr!("確認の記録が残した中身", "What the records of who looked keep")), records));
        }
        let own: Vec<&String> = m.files.keys().filter(|f| !held.contains(f) && about(f)).collect();
        if !own.is_empty() {
            groups.push((self.say(tr!("モジュールについて", "About the module")), own));
        }
        for (title, files) in groups {
            o.push_str(&format!("<h2 class=\"h3\">{}</h2>\n<div class=\"scroll\"><table class=\"t files\">\n<tbody>\n", esc(&title)));
            for f in files {
                let size = m.files.get(f).map_or(0, Vec::len);
                let n = ritsu_base::text::count(size as u64);
                o.push_str(&format!(
                    "<tr><td class=\"mono small\"{}><a href=\"{}\">{}</a></td><td class=\"right dim small nowrap\">{}</td></tr>\n",
                    lang_attr(f, self.lang),
                    esc(&if is_markdown(f) { self.file_link(place, f) } else { self.code_link(place, m, f) }),
                    esc(f),
                    esc(&self.say(tr!("{n} バイト", "{n} B")))
                ));
            }
            o.push_str("</tbody>\n</table></div>\n");
        }
        o.push_str("</section>\n");
        o
    }

    // ── The side of a version's page ────────────────────────────────────

    /// The steps that use a module, the button that opens it, and what was checked.
    fn use_it(&self, s: &Shelf, r: &Read<'_>, place: &Place) -> String {
        let v = r.shown;
        let m = v.module.as_ref().expect("a version with its files");
        let short = s.short.clone().unwrap_or_else(|| short_name(&s.name));
        let name = &s.name;
        let version = &v.version;
        let mut o = String::from("<div class=\"only-use\">\n");
        o.push_str(&format!("<h2>{}</h2>\n", esc(&self.say(tr!("使い方", "Use it")))));
        let mut n = 1;
        if v.published {
            let add = match &s.git {
                Some((url, dir)) => format!("ritsu mod add {short} {url}@{version}{}", dir.as_ref().map(|d| format!(" --dir {d}")).unwrap_or_default()),
                None => format!("ritsu mod add {short} {name}@{version}"),
            };
            o.push_str(&self.step(&self.say(tr!("{n}. 依存に足します", "{n}. Require it")), &add));
            n += 1;
            if s.git.is_none() {
                let lines = self.lines_for(name);
                if !lines.is_empty() {
                    let text: Vec<String> = lines.iter().map(|l| l.text()).collect();
                    o.push_str(&self.step(&self.say(tr!("{n}. 署名を確かめる行を `ritsu.mod` に足します", "{n}. Check its signature, in `ritsu.mod`")), &text.join("\n")));
                    n += 1;
                }
            }
            o.push_str(&self.step(&self.say(tr!("{n}. 取り出します", "{n}. Fetch it")), "ritsu mod fetch"));
            n += 1;
        } else {
            o.push_str(&format!("<p class=\"small\">{}</p>\n", inline(&self.say(tr!("公開したあとは `ritsu mod add {short} {name}@<版>` で足せます。", "Once it is published, `ritsu mod add {short} {name}@<version>` requires it.")))));
        }
        if let Some((label, code)) = self.use_snippet(&short, r) {
            o.push_str(&self.step(&format!("{n}. {}", self.say(label)), &code));
        }
        o.push_str(&self.button(v, place));
        // what was checked
        o.push_str(&format!("<h2 class=\"later\">{}</h2>\n", esc(&self.say(tr!("確かめたこと", "What was checked")))));
        let built = env!("CARGO_PKG_VERSION");
        o.push_str(&format!("<p class=\"small dim\">{}</p>\n<dl class=\"checked\">\n", esc(&self.say(tr!("このサイトをビルドしたときに ritsu {built} が確かめました。", "By ritsu {built}, when this site was built.")))));
        let item = |k: Text, html: String| format!("<dt>{}</dt><dd>{html}</dd>\n", esc(k.get(self.lang)));
        if v.published {
            if let Some((url, dir)) = &s.git {
                let (u, c) = (esc(url), v.commit.as_deref().unwrap_or("—"));
                let at = dir.as_ref().map(|d| format!(" ({})", esc(d))).unwrap_or_default();
                o.push_str(&item(tr!("出どころ", "Where it comes from"), format!("{}<br><code>{u}</code>{at}<br><code>{}</code>", esc(&self.say(tr!("git のリポジトリから取ります。署名はなく、lock がコミットとダイジェストで固定します。", "From a git repository: no signature, held by its commit and digests."))), esc(c))));
            } else {
                let lines = self.lines_for(name);
                let html = match lines.first() {
                    None => esc(&self.say(tr!("一覧の signature の行が覆っていないので、署名は確かめていません。", "No signature line of the list covers it, so no signature was checked."))),
                    Some(l) => {
                        let said = match (self.signer(l), &l.policy) {
                            (Some(who), _) => self.say(tr!("{who} の GitHub Actions のワークフローの署名です。cosign で確かめました。", "Signed by a GitHub Actions workflow of {who}. Checked with cosign.")),
                            (None, Policy::Key { .. }) => self.say(tr!("鍵の署名です。cosign で確かめました。", "Signed with a key. Checked with cosign.")),
                            (None, _) => self.say(tr!("決まった相手の署名です。cosign で確かめました。", "Signed by the identity below. Checked with cosign.")),
                        };
                        let rule = lines.iter().map(|x| esc(&x.text())).collect::<Vec<_>>().join("\n");
                        format!("{}\n<details><summary>{}</summary><pre class=\"code\"><code>{rule}</code></pre></details>", esc(&said), esc(&self.say(tr!("確かめた決まり", "The rule it was checked against"))))
                    }
                };
                o.push_str(&item(tr!("署名", "Signature"), html));
            }
            if let Some(d) = &v.digest {
                o.push_str(&item(tr!("ダイジェスト", "Digest"), digest_cell(d)));
            }
        }
        if let Some(c) = &r.check {
            let f = c.files;
            let html = if c.errors == 0 {
                let w = plural(f, "file", "files");
                inline(&self.say(tr!("`ritsu check .`：ファイル {f} 個がすべて通りました。", "`ritsu check .`: all {w} pass.")))
            } else {
                format!("<span class=\"warn\">{}</span><details><summary>{}</summary><pre class=\"code\"><code>{}</code></pre></details>", inline(&self.say(tr!("`ritsu check .` がエラーを出しました。", "`ritsu check .` found errors."))), esc(&self.say(tr!("言語の出力", "What it said"))), esc(c.said.trim()))
            };
            o.push_str(&item(tr!("検査", "Check"), html));
        }
        let pinned = holds::pinned(&r.holds);
        if !pinned.is_empty() {
            let n = pinned.len();
            let w = plural(n, "source", "sources");
            let mut html = format!("{}<ul class=\"plain\">", esc(&self.say(tr!("{n} 件を SHA-256 で固定しています。", "{w}, pinned by SHA-256."))));
            for p in &pinned {
                // a file of a dependency is on that module's page, not on this one's
                let file = if p.file.starts_with('@') {
                    format!("<code{}>{}</code>", lang_attr(&p.file, self.lang), esc(&p.file))
                } else {
                    format!("<a href=\"{}\"{}>{}</a>", esc(&self.file_link(place, &p.file)), lang_attr(&p.file, self.lang), esc(&p.file))
                };
                let url = p.url.as_ref().filter(|u| is_web(u)).map(|u| format!("<br><a class=\"small\" href=\"{0}\" rel=\"nofollow noopener\">{0}</a>", esc(u))).unwrap_or_default();
                html.push_str(&format!("<li>{file}{url}</li>"));
            }
            html.push_str("</ul>");
            o.push_str(&item(tr!("データ", "Data"), html));
        }
        match m.manifest.license.as_ref() {
            Some(l) => o.push_str(&item(tr!("ライセンス", "License"), format!("<code>{}</code>", esc(&l.value)))),
            None => o.push_str(&item(tr!("ライセンス", "License"), esc(&self.say(tr!("`ritsu.mod` に license の行がありません", "No license line in `ritsu.mod`")).replace('`', "")))),
        }
        if let Some(d) = &v.published_on {
            let how = if s.git.is_some() { self.say(tr!("タグのコミットの日付（UTC）", "The date of the tag's commit (UTC)")) } else { self.say(tr!("署名が透明性ログに入った日（UTC）", "The day its signature went into the transparency log (UTC)")) };
            o.push_str(&item(tr!("公開", "Published"), format!("<code>{}</code><br><span class=\"small dim\">{}</span>", esc(d), esc(&how))));
        }
        o.push_str("</dl>\n</div>\n");
        o
    }

    /// The step that says how a file names the module, for what it holds.
    fn use_snippet(&self, short: &str, r: &Read<'_>) -> Option<(Text, String)> {
        // a module of more than one kind of thing to call: each of them, as a caller names them
        let has = |k: Kind| r.holds.iter().any(|h| h.kind == k);
        if has(Kind::Rule) && (has(Kind::Calendar) || has(Kind::Dates) || has(Kind::Book)) || has(Kind::Dates) && has(Kind::Book) {
            let mut lines = String::new();
            for (kind, word) in [(Kind::Rule, "rule"), (Kind::Dates, "dates"), (Kind::Book, "book")] {
                for h in r.holds.iter().filter(|h| h.kind == kind) {
                    lines.push_str(&format!("use {word} {} from \"@{short}/{}\"\n", h.name, h.rel));
                }
            }
            return Some((tr!("ワークフローや kaname のモジュールから呼びます", "Call them from a workflow or a kaname module"), lines));
        }
        if let Some(c) = r.holds.iter().find(|h| h.kind == Kind::Calendar) {
            let (name, rel) = (&c.name, &c.rel);
            return Some((
                tr!("自分のカレンダーから読み、自分の休みを足します", "Read a calendar from your own, and close what your business closes"),
                format!("calendar office v1\ndescription \"Closed on Saturdays, Sundays and the days of {name}\"\nuse calendar \"@{short}/{rel}\"\n\nclosed weekly sat, sun\n"),
            ));
        }
        if let Some(c) = r.holds.iter().find(|h| h.kind == Kind::Rule) {
            let (name, rel) = (&c.name, &c.rel);
            return Some((tr!("ワークフローや kaname のモジュールから呼びます", "Call a rule from a workflow or a kaname module"), format!("use rule {name} from \"@{short}/{rel}\"\n")));
        }
        None
    }

    /// The steps before moving a project to a version, beside the tab of what it changes.
    fn upgrade(&self, s: &Shelf, r: &Read<'_>) -> String {
        if r.changes.is_none() {
            return String::new();
        }
        let short = s.short.clone().unwrap_or_else(|| short_name(&s.name));
        let version = &r.shown.version;
        let mut o = String::from("<div class=\"only-changes\">\n");
        o.push_str(&format!("<h2>{}</h2>\n", esc(&self.say(tr!("上げる前に", "Before you upgrade")))));
        o.push_str(&self.step(&self.say(tr!("1. プロジェクトをこの版に上げます", "1. Move your project to this version")), &format!("ritsu mod get {short}@{version}\nritsu mod fetch")));
        o.push_str(&self.step(&self.say(tr!("2. 自分のファイルが壊れないかを確かめます", "2. Let your own files say whether it breaks them")), "ritsu check ."));
        o.push_str(&self.step(&self.say(tr!("3. プルリクエストのために、自分のファイルに何が変わるかを見ます", "3. See what it changes for your files, for the pull request")), "ritsu mod report --base main --format markdown"));
        o.push_str(&format!("<p class=\"small dim\">{}</p>\n", inline(&self.say(tr!(
            "このページが言うのは、モジュールが何を変えるかです。自分のプロジェクトが壊れるかは、自分の `ritsu check` が言います。新しい値は、自分の表の外に出ることがあります。",
            "This page says what the module changes. Whether that breaks your project is for your own check to say: a new value can reach a table of yours that does not read it."
        )))));
        o.push_str("</div>\n");
        o
    }

    /// The button that opens a version in the playground; or why it has none.
    fn button(&self, v: &Shown, place: &Place) -> String {
        if v.paths {
            return format!(
                "<p class=\"small warn\">{}</p>\n",
                inline(&self.say(tr!(
                    "この作業ツリーは `require … path` でディレクトリのモジュールを読むので、ブラウザで試すページでは開けません。そのディレクトリは、このモジュールのファイルに入っていないからです。",
                    "This working tree requires a module by a path, so the playground cannot open it: the directory is not among the module's files."
                )))
            );
        }
        let label = self.say(tr!("ブラウザで試すページで開く", "Open in the playground"));
        let big = self.say(tr!("大きすぎて、ブラウザで試すページでは開けません", "Too big to open in the playground"));
        let failed = self.say(tr!("ブラウザで試すページに渡せませんでした", "Could not hand the files to the playground"));
        let note = self.say(tr!(
            "ファイルはリンクの `#` のあとに入れて運びます。ブラウザはそこをサーバーに送りません。",
            "The files go in the link, after its `#`, which a browser does not send to a server."
        ));
        format!(
            "<p><button type=\"button\" class=\"primarybtn\" id=\"open\" data-module=\"{}module.json\" data-playground=\"{}\" data-max=\"{LINK_MAX}\" data-big=\"{}\" data-failed=\"{}\">{}</button></p>\n<p class=\"small dim\">{}</p>\n",
            esc(&place.version),
            esc(&self.playground),
            esc(&big),
            esc(&failed),
            esc(&label),
            inline(&note)
        )
    }

    // ── A file ──────────────────────────────────────────────────────────

    /// The page of one file: the page its language's `doc` draws, in the frame of the site; or its
    /// source, with what its language said when it drew none.
    fn file_page(&self, s: &Shelf, v: &Shown, rel: &str, bytes: &[u8], doc: Option<Result<String, String>>) -> String {
        let m = v.module.as_ref().expect("a version with its files");
        let in_files = format!("files/{}", parent_of(rel));
        let page_rel = format!("{}/{}/files/{rel}.html", s.name, v.version);
        let root = up(&format!("{}/{}/{in_files}", s.name, v.version));
        let crumbs = self.file_crumbs(s, v, rel, false);
        let mut said = String::new();
        if let Some(d) = doc {
            match d {
                Ok(html) => {
                    let accent = docpage::Palette { light: vec![("accent".into(), "#0a7268".into())], dark: vec![("accent".into(), "#4bd9c7".into())] };
                    let head = format!("<link rel=\"stylesheet\" href=\"{root}{SHARED}/frame.css\">\n<style>{}</style>\n<script>{HEAD_JS}</script>\n<script src=\"{root}{SHARED}/site.js\" defer></script>\n</head>", accent.css());
                    return html.replacen("</head>", &head, 1).replacen("<body>\n", &format!("<body>\n{}{crumbs}", self.bar(&root, &page_rel)), 1);
                }
                Err(e) => said = e,
            }
        }
        let name = rel.rsplit('/').next().unwrap_or(rel);
        let mut b = format!("<main>\n{crumbs}<div class=\"wrap body filepage\">\n<h1 class=\"mono filetitle\"{}>{}</h1>\n", lang_attr(rel, self.lang), esc(rel));
        if !said.is_empty() {
            b.push_str(&format!(
                "<p class=\"bad\">{}</p>\n<pre class=\"code\"><code>{}</code></pre>\n",
                esc(&self.say(tr!("このファイルのページを作れませんでした。言語の出力：", "The page of this file could not be made. Its language said:"))),
                esc(&said)
            ));
        }
        match std::str::from_utf8(bytes) {
            Ok(text) if name.to_ascii_lowercase().ends_with(".md") => {
                let links = |p: &str| {
                    let joined = match parent_of(rel) {
                        "" => p.to_string(),
                        d => format!("{d}/{p}"),
                    };
                    m.files.contains_key(&joined).then(|| {
                        let back = up(parent_of(rel));
                        format!("{back}{}.html", url_path(&joined[parent_of(rel).len().min(0)..]))
                    })
                };
                b.push_str(&format!("<div class=\"readme\">\n{}</div>\n", readme_html(text, "", &links)));
            }
            Ok(text) => b.push_str(&self.code_block(rel, text)),
            Err(_) => {
                let (n, d) = (ritsu_base::text::count(bytes.len() as u64), digest::of(bytes));
                b.push_str(&format!(
                    "<p>{}</p>\n",
                    inline(&self.say(tr!(
                        "UTF-8 のテキストではないファイルです（{n} バイト、`{d}`）。ブラウザで試すページには、バイトのまま渡します。",
                        "This file is not UTF-8 text ({n} bytes, `{d}`). The playground is given its bytes as they are."
                    )))
                ));
            }
        }
        b.push_str("</div>\n");
        b.push_str(&self.footer(None));
        b.push_str("</main>\n");
        let place = Place { root, module: String::new(), version: String::new(), rel: page_rel };
        self.page(&format!("{rel} · {} {}", s.name, v.version), None, &place, None, &b)
    }

    /// The code of a file whose page is something else: what its language drew, or its Markdown as
    /// it reads.
    fn code_page(&self, s: &Shelf, v: &Shown, rel: &str, text: &str) -> String {
        let root = up(&format!("{}/{}/code/{}", s.name, v.version, parent_of(rel)));
        let mut b = format!("<main>\n{}<div class=\"wrap body filepage\">\n<h1 class=\"mono filetitle\"{}>{}</h1>\n", self.file_crumbs(s, v, rel, true), lang_attr(rel, self.lang), esc(rel));
        b.push_str(&self.code_block(rel, text));
        b.push_str("</div>\n");
        b.push_str(&self.footer(None));
        b.push_str("</main>\n");
        let place = Place { root, module: String::new(), version: String::new(), rel: format!("{}/{}/code/{rel}.html", s.name, v.version) };
        self.page(&format!("{rel} · {} {}", s.name, v.version), None, &place, None, &b)
    }

    /// A file's lines, and over them how many there are, how long the file is, whose language it is
    /// and the button that copies it.
    fn code_block(&self, rel: &str, text: &str) -> String {
        let (pre, n) = code::block(rel, text, lang_attr(text, self.lang));
        let bytes = ritsu_base::text::count(text.len() as u64);
        let mut about = vec![self.say(Text::new(format!("{} 行", ritsu_base::text::count(n as u64)), plural(n, "line", "lines"))), self.say(tr!("{bytes} バイト", "{bytes} B"))];
        about.extend(ritsu_project::project::kind_of(rel).filter(|t| has_doc(*t) || *t == Tool::Geas).map(|t| t.word().to_string()));
        format!("<div class=\"stephead\"><p class=\"small dim\">{}</p>{}</div>\n{pre}", esc(&about.join(" · ")), self.copy_button())
    }

    /// The bar over a file's page or its code: the way back, the switch between the two when the
    /// file has both, and the other files of the version, which open as this one is shown.
    fn file_crumbs(&self, s: &Shelf, v: &Shown, rel: &str, in_code: bool) -> String {
        let m = v.module.as_ref().expect("a version with its files");
        let here = format!("{}/{}", if in_code { "code" } else { "files" }, parent_of(rel));
        let to_version = up(&here);
        let root = up(&format!("{}/{}/{here}", s.name, v.version));
        let options: String = m
            .files
            .iter()
            .map(|(f, bytes)| {
                let selected = if f == rel { " selected" } else { "" };
                let to = if in_code && has_code_page(f, bytes) { "code" } else { "files" };
                format!("<option value=\"{to_version}{to}/{}.html\"{selected}>{}</option>", esc(&url_path(f)), esc(f))
            })
            .collect();
        let switch = if m.files.get(rel).is_some_and(|b| has_code_page(rel, b)) {
            let link = |to: &str, on: bool, label: Text| format!("<a href=\"{to_version}{to}/{}.html\"{}>{}</a>", esc(&url_path(rel)), if on { " aria-current=\"page\"" } else { "" }, esc(&self.say(label)));
            format!(
                "<span class=\"rm-switch\" role=\"group\" aria-label=\"{}\">{}{}</span>",
                esc(&self.say(tr!("このファイルの見方", "How to read this file"))),
                link("files", !in_code, tr!("ページ", "Page")),
                link("code", in_code, tr!("コード", "Code"))
            )
        } else {
            String::new()
        };
        format!(
            "<nav class=\"rm-crumbs\" aria-label=\"{}\"><div class=\"rm-crumbsin\"><span class=\"rm-trail\"><a href=\"{root}index.html\">{}</a> / <a href=\"{to_version}../index.html\">{}</a> / <a href=\"{to_version}index.html\">{}</a></span>{switch}<label class=\"rm-pick\"><span class=\"rm-vh\">{}</span><select class=\"rm-select\" data-go=\"\">{options}</select></label></div></nav>\n",
            esc(&self.say(tr!("このファイルの場所", "Where this file is"))),
            esc(&self.say(tr!("モジュール", "Modules"))),
            esc(&s.name),
            esc(&v.version),
            esc(&self.say(tr!("ほかのファイル", "Another file")))
        )
    }
}

fn is_markdown(rel: &str) -> bool {
    rel.rsplit('/').next().unwrap_or(rel).to_ascii_lowercase().ends_with(".md")
}

/// Whether a file's code has a page of its own: the page of the file itself is what its language
/// drew, or its Markdown as it reads. Any other file of text is shown as its code on its page.
fn has_code_page(rel: &str, bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).is_ok() && (is_markdown(rel) || ritsu_project::project::kind_of(rel).is_some_and(has_doc))
}

/// Whether a link a module's files give leads to the web, and so may be a link on a page.
fn is_web(u: &str) -> bool {
    let l = u.to_ascii_lowercase();
    l.starts_with("https://") || l.starts_with("http://")
}

/// Whether a language draws a page of a file with `doc` (DESIGN 4.8).
fn has_doc(t: Tool) -> bool {
    matches!(t, Tool::Rulec | Tool::Dandori | Tool::Koyomi | Tool::Chobo | Tool::Yuen | Tool::Sakai | Tool::Sekisho | Tool::Kaname)
}

/// The HTML page `ritsu <language> doc <rel> --format html` prints, run in `root`; or what it said
/// when it drew none.
fn doc_of(root: &Path, t: Tool, rel: &str, lang: Lang) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut c = std::process::Command::new(exe);
    c.current_dir(root).args([t.word(), "doc", rel, "--format", "html", "--lang", lang.code()]);
    if t == Tool::Sekisho {
        c.args(["--root", "."]);
    }
    let o = c.output().map_err(|e| e.to_string())?;
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    if o.status.success() && out.trim_start().to_ascii_lowercase().starts_with("<!doctype html>") {
        Ok(out)
    } else {
        Err(format!("{out}{}", String::from_utf8_lossy(&o.stderr)))
    }
}

/// `module.json` (DESIGN 17.17): the files of the version, and the modules its lock pins in the form
/// of a module's copy of the store, `[[path, text], …]`, a file that is not UTF-8 as `[path,
/// {"base64": "…"}]`.
fn module_json(m: &diff::Module, deps: &[(Store, String)]) -> Result<String, Text> {
    let mut items: Vec<Json> = m.files.iter().map(|(p, b)| item(p, b)).collect();
    for (from, d) in deps {
        let hex = d.strip_prefix("sha256:").unwrap_or(d);
        let at = format!("{}/sha256/{hex}", store::VENDOR);
        let unreadable = || tr!("置き場の項目 {d} を読めません", "cannot read the entry {d} of the store");
        let entry = from.entry(d);
        let manifest_bytes = std::fs::read(entry.join("manifest.json")).map_err(|_| unreadable())?;
        let module = from.module_bytes(d).ok_or_else(unreadable)?;
        let record = std::fs::read(entry.join("record.json")).map_err(|_| unreadable())?;
        items.push(item(&format!("{at}/manifest.json"), &manifest_bytes));
        items.push(item(&format!("{at}/{}", manifest::FILE), &module));
        items.push(item(&format!("{at}/record.json"), &record));
        for (p, b) in digest::files_in(&from.files(d))? {
            items.push(item(&format!("{at}/files/{p}"), &b));
        }
    }
    Ok(Json::arr(items).compact() + "\n")
}

/// One file of `module.json`.
fn item(path: &str, bytes: &[u8]) -> Json {
    match std::str::from_utf8(bytes) {
        Ok(t) => Json::arr([Json::str(path), Json::str(t)]),
        Err(_) => Json::arr([Json::str(path), Json::obj([("base64", Json::str(ritsu_base::sources::base64_encode(bytes)))])]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_paths_as_links_take_them() {
        assert_eq!(url_path("税/a b.rule"), "%E7%A8%8E/a%20b.rule");
        assert_eq!(url_path("127.0.0.1:5000/acme/tax"), "127.0.0.1%3A5000/acme/tax");
        assert_eq!(up("a/b/c"), "../../../");
        assert_eq!(parent_of("rules/a.rule"), "rules");
        assert_eq!(parent_of("a.rule"), "");
        assert_eq!(short_name("ghcr.io/i2y/ritsu-holidays"), "ritsu_holidays");
        assert_eq!(short_name("example.com/acme/tax"), "tax");
        assert_eq!(split_name("ghcr.io/i2y/ritsu-holidays"), ("ghcr.io/i2y/", "ritsu-holidays"));
    }

    #[test]
    fn who_signed_it_in_a_few_words() {
        let line = |pattern: &str, issuer: &str| Signature { prefix: "x".into(), policy: Policy::Matching { pattern: pattern.into(), issuer: issuer.into() }, line: 1 };
        let list = List { title: None, description: None, playground: None, contribute: None, entries: Vec::new(), signatures: Vec::new(), dir: PathBuf::new() };
        let w = Writer { out: PathBuf::new(), lang: Lang::En, list: &list, playground: String::new(), alternate: None, joined: Joined::new(), written: 0 };
        let actions = "https://token.actions.githubusercontent.com";
        assert_eq!(w.signer(&line("^https://github[.]com/i2y/ritsu/", actions)).as_deref(), Some("i2y/ritsu"));
        assert_eq!(w.signer(&line("^https://github[.]com/i2y/", actions)).as_deref(), Some("i2y"));
        assert_eq!(w.signer(&line("^https://github[.]com/.*", actions)), None);
        assert_eq!(w.signer(&line("^https://github[.]com/i2y/", "https://example.com")), None);
    }
}
