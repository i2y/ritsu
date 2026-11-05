//! What a module's requirements say moved between two versions (DESIGN 17.9.5).
//!
//! A `.req` has no interface: it answers nothing, and no file of a user reads a dependency's
//! requirements (`ritsu check` holds a dependency's `.req` as a project of their own). What it has is
//! why the answers are what they are: the text of each requirement, when it is in force, what was
//! decided, what it is read from and what meets it, and who looked at each link. So `ritsu mod diff`
//! says those, requirement by requirement, from what `yuen api` prints for the side's `.req` files as
//! one project.
//!
//! A requirement moved when it has no counterpart, or when what it says or what it stands on differs:
//! its text, its period, its owner, its decisions, or the hash of an end. The line it is written at,
//! the state of its links and the records of who looked do not move it: a person looking again at
//! the same two ends changes nothing a reader asks about. A record says a person looked at the two
//! ends, and that they have not moved since. It does not say the words are right, and the output says
//! so.

use super::diff::{At, How, change_word, colon, html};
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_project::Joined;
use std::path::{Path, PathBuf};

/// A value on each side, None on a side that has none.
pub type Pair = (Option<String>, Option<String>);

/// A decision: when, by which role, and why, in the words of the `.req`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decided {
    pub date: String,
    pub by: String,
    pub why: String,
}

/// What a requirement is read from: a source's fragment (`@osha §1903.15`) or another requirement.
#[derive(Clone, Debug)]
pub struct Cited {
    pub what: String,
    /// The law's ID or the document's path, for a source.
    pub id: Option<String>,
    /// The date of a law's copy on each side.
    pub asof: Pair,
    /// Whether it changed, within a requirement both versions have: None when it is the same, and
    /// for a requirement one side has.
    pub how: Option<How>,
    /// The lines of what its text says differently, from the copies the records keep (empty when
    /// a side has no copy): `' '`, `'-'` and `'+'`, and `'@'` between two stretches.
    pub lines: Vec<(char, String)>,
    /// How many lines past those were left out.
    pub more: usize,
}

/// What meets or checks a requirement.
#[derive(Clone, Debug)]
pub struct Linked {
    /// `satisfied` or `verified`, as `.req` and `yuen api` write them.
    pub role: String,
    /// As the `.req` writes it: `rulec "fee.rule" table rates`.
    pub artifact: String,
    /// Its file, from the root of the module.
    pub path: Option<String>,
    /// As [`Cited::how`].
    pub how: Option<How>,
}

impl Linked {
    /// How the output heads it.
    pub fn label(&self) -> Text {
        role_label(&self.role)
    }
}

fn role_label(role: &str) -> Text {
    match role {
        "verified" => tr!("確かめるもの", "checked by"),
        _ => tr!("満たすもの", "met by"),
    }
}

/// A link a requirement says is not there, and why.
#[derive(Clone, Debug)]
pub struct Waived {
    pub role: String,
    pub why: String,
}

impl Waived {
    /// How the output heads it.
    pub fn label(&self) -> Text {
        match self.role.as_str() {
            "verified" => tr!("確かめないこと", "not checked"),
            _ => tr!("満たさないこと", "not met"),
        }
    }
}

/// One requirement that moved between the two versions.
#[derive(Clone, Debug)]
pub struct Why {
    /// The `.req` it is in, from the root of the module.
    pub file: String,
    /// The line it starts at in the `.req` of the newer side that has it.
    pub line: Option<i128>,
    /// Its name, with `v<n>` when it has versions.
    pub name: String,
    pub how: How,
    pub text: Pair,
    pub in_force: Pair,
    pub owner: Pair,
    /// The decisions the new side writes and the old one does not.
    pub decided: Vec<Decided>,
    pub from: Vec<Cited>,
    pub links: Vec<Linked>,
    pub waived: Vec<Waived>,
    /// Who looked, and when: the records of the new side that still hold.
    pub looked: Vec<(String, String)>,
    /// How many links of the new side have no record that holds for them as they are.
    pub unsettled: usize,
}

// ── Reading ─────────────────────────────────────────────────────────────

/// What `yuen api` prints for a side's `.req` files, as one project: None when the side has none,
/// Err, saying so, for files yuen cannot read at all (a link no one has looked at is still read: it
/// is in the JSON as a state).
pub(super) fn project(at: &At, files: &[PathBuf], joined: &Joined) -> Result<Option<Json>, Text> {
    if files.is_empty() {
        return Ok(None);
    }
    let label = &at.label;
    let cannot = || {
        tr!(
            "`{label}` の要件のファイルを読めないので、要件を比べられません。中身のダイジェストだけを比べました",
            "the requirements files of `{label}` cannot be read, so the requirements are not compared; only their digests are"
        )
    };
    let mut args = vec!["api".to_string()];
    args.extend(files.iter().map(|p| p.to_string_lossy().into_owned()));
    let (mut out, mut err) = (Vec::new(), Vec::new());
    if at.with(|| yuen::run::run(&args, joined.yuen(), &mut out, &mut err)) != 0 {
        return Err(cannot());
    }
    ritsu_base::json::parse(String::from_utf8_lossy(&out).trim()).map(Some).map_err(|_| cannot())
}

/// One side: where its files are read, and what yuen says of its `.req` files.
pub(super) struct Side<'a> {
    pub at: &'a At,
    pub api: Option<&'a Json>,
}

impl Side<'_> {
    fn requirements(&self) -> &[Json] {
        self.api.map_or(&[], |j| list(j, "requirements"))
    }

    /// The source `name` of the `.req` `file`.
    fn source(&self, file: &str, name: &str) -> Option<&Json> {
        list(self.api?, "sources").iter().find(|s| word(s, "file") == Some(file) && word(s, "name") == Some(name))
    }

    /// The text a record keeps of an end, beside the `.req` `file`: a law's copy as its text.
    fn copy(&self, file: &str, sha: &str, law: bool) -> Option<String> {
        let dir = Path::new(file).parent().map_or_else(|| self.at.root.clone(), |p| self.at.root.join(p));
        let bytes = self.at.with(|| ritsu_base::fs::read(dir.join("reviewed").join(sha))).ok()?;
        let text = String::from_utf8(bytes).ok()?;
        Some(if law { ritsu_base::sources::xml_text(&text) } else { text })
    }
}

fn list<'a>(j: &'a Json, key: &str) -> &'a [Json] {
    j.get(key).and_then(Json::as_arr).unwrap_or(&[])
}

fn word<'a>(j: &'a Json, key: &str) -> Option<&'a str> {
    j.get(key).and_then(Json::as_str)
}

fn owned(j: &Json, key: &str) -> Option<String> {
    word(j, key).map(str::to_string)
}

/// What a requirement says and stands on: all of it but where it is written, the state of its links
/// and the records of who looked.
fn bare(j: &Json) -> Json {
    match j {
        Json::Obj(pairs) => Json::Obj(pairs.iter().filter(|(k, _)| !matches!(k.as_str(), "line" | "file" | "status" | "reviewed" | "approved")).map(|(k, v)| (k.clone(), bare(v))).collect()),
        Json::Arr(xs) => Json::Arr(xs.iter().map(bare).collect()),
        other => other.clone(),
    }
}

/// The name a requirement is told by: `v<n>` follows it when its name has versions.
fn shown_name(all: &[Json], r: &Json) -> String {
    let name = word(r, "name").unwrap_or_default();
    let of_name = all.iter().filter(|x| word(x, "name") == Some(name)).count();
    match r.get("version").and_then(Json::as_int) {
        Some(v) if of_name > 1 || v > 1 => format!("{name} v{v}"),
        _ => name.to_string(),
    }
}

/// `2025-01-16..`, or `2025-01-16..2026-01-15`.
fn period(r: &Json) -> Option<String> {
    let p = r.get("in_force").filter(|p| !p.is_null())?;
    Some(format!("{}..{}", word(p, "from").unwrap_or_default(), word(p, "to").unwrap_or_default()))
}

/// One thing a requirement is read from: how it is told, its end's hash, and its source's name.
struct Cite {
    what: String,
    sha: Option<String>,
    source: Option<String>,
}

fn cites(r: &Json) -> Vec<Cite> {
    let mut out = Vec::new();
    for f in list(r, "from") {
        for s in list(f, "sources") {
            let name = word(s, "source").unwrap_or_default();
            let what = word(s, "fragment").map_or_else(|| format!("@{name}"), |fr| format!("@{name} {fr}"));
            out.push(Cite { what, sha: owned(s, "sha256"), source: Some(name.to_string()) });
        }
        if let Some(q) = f.get("requirement").filter(|q| !q.is_null()) {
            out.push(Cite { what: word(q, "name").unwrap_or_default().to_string(), sha: owned(q, "sha256"), source: None });
        }
    }
    out
}

/// How a thing of a requirement both sides have changed, from the hashes of its end.
fn how_of(both: bool, old: Option<Option<&str>>, new: Option<&str>) -> Option<How> {
    match old {
        _ if !both => None,
        None => Some(How::Added),
        Some(o) if o != new => Some(How::Changed),
        Some(_) => None,
    }
}

fn from_of(o: &Side<'_>, n: &Side<'_>, old: Option<&Json>, new: &Json) -> Vec<Cited> {
    let file = word(new, "file").unwrap_or_default();
    let before = old.map(cites).unwrap_or_default();
    let mut out = Vec::new();
    let source_of = |side: &Side<'_>, r: Option<&Json>, c: &Cite| {
        let (Some(r), Some(name)) = (r, &c.source) else { return None };
        side.source(word(r, "file").unwrap_or_default(), name).cloned()
    };
    for c in cites(new) {
        let then = before.iter().find(|b| b.what == c.what);
        let (src_o, src_n) = (then.and_then(|b| source_of(o, old, b)), source_of(n, Some(new), &c));
        let how = how_of(old.is_some(), then.map(|b| b.sha.as_deref()), c.sha.as_deref());
        let law = src_n.as_ref().is_some_and(|s| word(s, "kind") == Some("law"));
        let (mut lines, mut more) = (Vec::new(), 0);
        if how == Some(How::Changed)
            && let (Some(a), Some(b), Some(r)) = (then.and_then(|b| b.sha.as_deref()), c.sha.as_deref(), old)
            && let (Some(x), Some(y)) = (o.copy(word(r, "file").unwrap_or_default(), a, law), n.copy(file, b, law))
        {
            let (ls, m) = yuen::diff::unified(&x, &y);
            lines = ls.into_iter().map(|l| (l.op, l.text)).collect();
            more = m;
        }
        out.push(Cited {
            what: c.what,
            id: src_n.as_ref().and_then(|s| owned(s, "id").or_else(|| owned(s, "path"))),
            asof: (src_o.as_ref().and_then(|s| owned(s, "asof")), src_n.as_ref().and_then(|s| owned(s, "asof"))),
            how,
            lines,
            more,
        });
    }
    let now = cites(new);
    for b in before.iter().filter(|b| !now.iter().any(|c| c.what == b.what)) {
        let src = source_of(o, old, b);
        out.push(Cited {
            what: b.what.clone(),
            id: src.as_ref().and_then(|s| owned(s, "id").or_else(|| owned(s, "path"))),
            asof: (src.as_ref().and_then(|s| owned(s, "asof")), None),
            how: Some(How::Removed),
            lines: Vec::new(),
            more: 0,
        });
    }
    out
}

/// A requirement's links: its role, the artifact as written, its file and its end's hash.
fn links(r: &Json) -> Vec<(String, String, Option<String>, Option<String>)> {
    list(r, "links")
        .iter()
        .filter_map(|l| {
            let a = l.get("artifact").filter(|a| !a.is_null())?;
            Some((owned(l, "role").unwrap_or_default(), owned(a, "text").unwrap_or_default(), owned(a, "path"), owned(l, "sha256")))
        })
        .collect()
}

fn links_of(old: Option<&Json>, new: &Json) -> Vec<Linked> {
    let before = old.map(links).unwrap_or_default();
    let now = links(new);
    let mut out = Vec::new();
    for (role, artifact, path, sha) in &now {
        let then = before.iter().find(|b| &b.0 == role && &b.1 == artifact);
        let how = how_of(old.is_some(), then.map(|b| b.3.as_deref()), sha.as_deref());
        out.push(Linked { role: role.clone(), artifact: artifact.clone(), path: path.clone(), how });
    }
    for (role, artifact, path, _) in before.iter().filter(|b| !now.iter().any(|x| x.0 == b.0 && x.1 == b.1)) {
        out.push(Linked { role: role.clone(), artifact: artifact.clone(), path: path.clone(), how: Some(How::Removed) });
    }
    out
}

fn decided(r: &Json) -> Vec<Decided> {
    list(r, "decided").iter().map(|d| Decided { date: word(d, "date").unwrap_or_default().to_string(), by: word(d, "by").unwrap_or_default().to_string(), why: word(d, "why").unwrap_or_default().to_string() }).collect()
}

/// Who looked at the links of `r` as they are, each once, and how many links no record holds for.
fn looked(r: &Json) -> (Vec<(String, String)>, usize) {
    let (mut who, mut unsettled) = (Vec::new(), 0);
    for e in list(r, "from").iter().chain(list(r, "links")).chain(list(r, "waivers")) {
        if word(e, "status") != Some("ok") {
            unsettled += 1;
            continue;
        }
        if let Some(rc) = e.get("reviewed").filter(|x| !x.is_null()).or_else(|| e.get("approved").filter(|x| !x.is_null())) {
            who.push((word(rc, "date").unwrap_or_default().to_string(), word(rc, "by").unwrap_or_default().to_string()));
        }
    }
    who.sort();
    who.dedup();
    (who, unsettled)
}

/// The requirements that moved: those of the new side in its order, then those only the old one has.
pub(super) fn whys(o: &Side<'_>, n: &Side<'_>) -> Vec<Why> {
    let key = |r: &Json| (owned(r, "name"), r.get("version").and_then(Json::as_int));
    let (olds, news) = (o.requirements(), n.requirements());
    let mut out = Vec::new();
    for r in news {
        let old = olds.iter().find(|x| key(x) == key(r));
        if old.is_some_and(|x| bare(x).compact() == bare(r).compact()) {
            continue;
        }
        let before = old.map(decided).unwrap_or_default();
        let (who, unsettled) = looked(r);
        out.push(Why {
            file: owned(r, "file").unwrap_or_default(),
            line: r.get("line").and_then(Json::as_int),
            name: shown_name(news, r),
            how: if old.is_some() { How::Changed } else { How::Added },
            text: (old.and_then(|x| owned(x, "text")), owned(r, "text")),
            in_force: (old.and_then(period), period(r)),
            owner: (old.and_then(|x| owned(x, "owner")), owned(r, "owner")),
            decided: decided(r).into_iter().filter(|d| !before.contains(d)).collect(),
            from: from_of(o, n, old, r),
            links: links_of(old, r),
            waived: list(r, "waivers").iter().map(|w| Waived { role: word(w, "role").unwrap_or_default().to_string(), why: word(w, "why").unwrap_or_default().to_string() }).collect(),
            looked: who,
            unsettled,
        });
    }
    for r in olds.iter().filter(|x| !news.iter().any(|y| key(y) == key(x))) {
        out.push(Why {
            file: owned(r, "file").unwrap_or_default(),
            line: None,
            name: shown_name(olds, r),
            how: How::Removed,
            text: (owned(r, "text"), None),
            in_force: (period(r), None),
            owner: (owned(r, "owner"), None),
            decided: Vec::new(),
            from: Vec::new(),
            links: Vec::new(),
            waived: Vec::new(),
            looked: Vec::new(),
            unsettled: 0,
        });
    }
    out
}

// ── Saying ──────────────────────────────────────────────────────────────

/// What the reasons are, said once under them.
pub fn own_words() -> Text {
    tr!(
        "これはモジュールの要件のファイルの言葉です。ritsu が確かめるのは、書かれた相手が確認のあとで動いていないことで、書かれたことが正しいかどうかではありません。",
        "These are the words of the module's requirements file. ritsu checks that what they were written against has not moved since someone looked, not that they are right."
    )
}

/// `a → b`, or the one value when the sides do not differ or only one has it.
fn moved(p: &Pair, q: &str) -> Option<String> {
    match p {
        (Some(a), Some(b)) if a != b => Some(format!("{q}{a}{q} → {q}{b}{q}")),
        (_, Some(x)) | (Some(x), None) => Some(format!("{q}{x}{q}")),
        (None, None) => None,
    }
}

pub fn decided_label(d: &Decided) -> Text {
    let (date, by) = (&d.date, &d.by);
    tr!("決めたこと（{date}、{by}）", "decided {date} by {by}")
}

pub fn looked_label(date: &str, by: &str) -> Text {
    tr!("確認（{date}、{by}）", "looked at {date} by {by}")
}

pub fn in_force_label() -> Text {
    tr!("効く期間", "in force")
}

pub fn owner_label() -> Text {
    tr!("持ち主", "owner")
}

pub fn from_label() -> Text {
    tr!("出どころ", "from")
}

pub fn unsettled_label(k: usize) -> Text {
    let w = if k == 1 { "link has" } else { "links have" };
    tr!("{k} 本のリンクに、いまの中身を確認した記録がありません。`ritsu check` はこれで止まります", "{k} {w} no record of someone looking at it as it is, which stops `ritsu check`")
}

/// What is said of a thing a requirement is read from, after its name: `29 CFR 1903, as of a → b`.
fn cited_tail(c: &Cited, lang: Lang, q: &str) -> String {
    let mut parts: Vec<String> = c.id.iter().map(|i| format!("{q}{i}{q}")).collect();
    if let Some(d) = moved(&c.asof, q) {
        parts.push(tr!("{d} 時点", "as of {d}").get(lang).to_string());
    }
    parts.join(if lang == Lang::Ja { "、" } else { ", " })
}

/// How a thing of a requirement changed, after a colon; empty when it did not.
fn how_tail(how: Option<How>, lang: Lang) -> String {
    how.map_or(String::new(), |h| format!("{}{}", colon(lang), change_word(h, lang)))
}

/// The requirements, as `ritsu mod diff` writes text.
pub(super) fn text(ws: &[Why], lang: Lang) -> String {
    let c = colon(lang);
    let mut o = String::new();
    for w in ws {
        o.push_str(&format!("  {} ({}){c}{}\n", w.name, w.file, change_word(w.how, lang)));
        match &w.text {
            (Some(a), Some(b)) if a != b => o.push_str(&format!("    - {a}\n    + {b}\n")),
            (_, Some(x)) | (Some(x), None) => o.push_str(&format!("    {x}\n")),
            (None, None) => {}
        }
        if let Some(v) = moved(&w.in_force, "") {
            o.push_str(&format!("    {}{c}{v}\n", in_force_label().get(lang)));
        }
        if matches!(&w.owner, (Some(a), Some(b)) if a != b) {
            o.push_str(&format!("    {}{c}{}\n", owner_label().get(lang), moved(&w.owner, "").unwrap_or_default()));
        }
        for d in &w.decided {
            o.push_str(&format!("    {}{c}{}\n", decided_label(d).get(lang), d.why));
        }
        for f in &w.from {
            let tail = cited_tail(f, lang, "");
            let tail = if tail.is_empty() { tail } else { format!(" ({tail})") };
            o.push_str(&format!("    {} {}{tail}{}\n", from_label().get(lang), f.what, how_tail(f.how, lang)));
            for (op, l) in f.lines.iter().filter(|(op, _)| *op == '-' || *op == '+') {
                o.push_str(&format!("      {op} {l}\n"));
            }
            if f.more > 0 {
                let more = f.more;
                o.push_str(&format!("      {}\n", tr!("ほか {more} 行", "and {more} more lines").get(lang)));
            }
        }
        for l in &w.links {
            o.push_str(&format!("    {} {}{}\n", l.label().get(lang), l.artifact, how_tail(l.how, lang)));
        }
        for x in &w.waived {
            o.push_str(&format!("    {}{c}{}\n", x.label().get(lang), x.why));
        }
        for (date, by) in &w.looked {
            o.push_str(&format!("    {}\n", looked_label(date, by).get(lang)));
        }
        if w.unsettled > 0 {
            o.push_str(&format!("    {}\n", unsettled_label(w.unsettled).get(lang)));
        }
    }
    o.push_str(&format!("  {}\n", own_words().get(lang)));
    o
}

/// A fence no line of `lines` closes.
fn fence_for(lines: &[(char, String)]) -> String {
    let longest = lines.iter().flat_map(|(_, l)| l.split(|c| c != '`')).map(str::len).max().unwrap_or(0);
    "`".repeat((longest + 1).max(3))
}

/// The requirements, as `ritsu mod diff` writes Markdown: the words of the `.req` have their tags
/// escaped, as a module's words go into a pull request's comment.
pub(super) fn markdown(ws: &[Why], lang: Lang) -> String {
    let c = colon(lang);
    let cap = ritsu_base::text::capitalize;
    let mut o = String::new();
    for w in ws {
        o.push_str(&format!("\n- **`{}`** (`{}`){c}{}\n", w.name, w.file, change_word(w.how, lang)));
        match &w.text {
            (Some(a), Some(b)) if a != b => {
                o.push_str(&format!("  - {}{c}{}\n", tr!("前", "Was").get(lang), html(a)));
                o.push_str(&format!("  - {}{c}{}\n", tr!("今", "Now").get(lang), html(b)));
            }
            (_, Some(x)) | (Some(x), None) => o.push_str(&format!("  - {}\n", html(x))),
            (None, None) => {}
        }
        if let Some(v) = moved(&w.in_force, "`") {
            o.push_str(&format!("  - {}{c}{v}\n", cap(in_force_label().get(lang))));
        }
        if matches!(&w.owner, (Some(a), Some(b)) if a != b) {
            o.push_str(&format!("  - {}{c}{}\n", cap(owner_label().get(lang)), moved(&w.owner, "`").unwrap_or_default()));
        }
        for d in &w.decided {
            o.push_str(&format!("  - {}{c}{}\n", cap(decided_label(d).get(lang)), html(&d.why)));
        }
        for f in &w.from {
            let tail = cited_tail(f, lang, "`");
            let tail = if tail.is_empty() { tail } else { format!(" ({tail})") };
            o.push_str(&format!("  - {} `{}`{tail}{}\n", cap(from_label().get(lang)), f.what, how_tail(f.how, lang)));
        }
        for l in &w.links {
            o.push_str(&format!("  - {} `{}`{}\n", cap(l.label().get(lang)), l.artifact, how_tail(l.how, lang)));
        }
        for x in &w.waived {
            o.push_str(&format!("  - {}{c}{}\n", cap(x.label().get(lang)), html(&x.why)));
        }
        for (date, by) in &w.looked {
            o.push_str(&format!("  - {}\n", cap(looked_label(date, by).get(lang))));
        }
        if w.unsettled > 0 {
            o.push_str(&format!("  - {}\n", cap(unsettled_label(w.unsettled).get(lang))));
        }
        for f in w.from.iter().filter(|f| !f.lines.is_empty()) {
            let shown: Vec<(char, String)> = f.lines.iter().filter(|(op, _)| *op == '-' || *op == '+').cloned().collect();
            let fence = fence_for(&shown);
            o.push_str(&format!("\n<details><summary><code>{}</code>{c}{}</summary>\n\n{fence}diff\n", html(&f.what), tr!("本文の違い", "what its text says differently").get(lang)));
            for (op, l) in &shown {
                o.push_str(&format!("{op} {l}\n"));
            }
            o.push_str(&format!("{fence}\n\n</details>\n"));
        }
    }
    o.push_str(&format!("\n{}\n", own_words().get(lang)));
    o
}

/// The requirements as the JSON holds them (DESIGN 17.21).
pub(super) fn json(ws: &[Why]) -> Json {
    let pair = |p: &Pair| Json::obj([("old", Json::opt_str(p.0.clone())), ("new", Json::opt_str(p.1.clone()))]);
    let how = |h: Option<How>| h.map_or(Json::Null, |h| Json::str(h.word()));
    Json::arr(ws.iter().map(|w| {
        Json::obj([
            ("file", Json::str(w.file.as_str())),
            ("line", w.line.map_or(Json::Null, Json::int)),
            ("requirement", Json::str(w.name.as_str())),
            ("change", Json::str(w.how.word())),
            ("text", pair(&w.text)),
            ("in_force", pair(&w.in_force)),
            ("owner", pair(&w.owner)),
            ("decided", Json::arr(w.decided.iter().map(|d| Json::obj([("date", Json::str(d.date.as_str())), ("by", Json::str(d.by.as_str())), ("why", Json::str(d.why.as_str()))])))),
            (
                "from",
                Json::arr(w.from.iter().map(|f| {
                    Json::obj([
                        ("what", Json::str(f.what.as_str())),
                        ("id", Json::opt_str(f.id.clone())),
                        ("asof", pair(&f.asof)),
                        ("change", how(f.how)),
                        ("lines", Json::arr(f.lines.iter().map(|(op, t)| Json::obj([("op", Json::str(op.to_string())), ("text", Json::str(t.as_str()))])))),
                        ("more", Json::int(f.more as i128)),
                    ])
                })),
            ),
            (
                "links",
                Json::arr(w.links.iter().map(|l| Json::obj([("role", Json::str(l.role.as_str())), ("artifact", Json::str(l.artifact.as_str())), ("path", Json::opt_str(l.path.clone())), ("change", how(l.how))]))),
            ),
            ("waivers", Json::arr(w.waived.iter().map(|x| Json::obj([("role", Json::str(x.role.as_str())), ("why", Json::str(x.why.as_str()))])))),
            ("looked", Json::arr(w.looked.iter().map(|(d, b)| Json::obj([("date", Json::str(d.as_str())), ("by", Json::str(b.as_str()))])))),
            ("unsettled", Json::int(w.unsettled as i128)),
        ])
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at() -> At {
        At { read: PathBuf::new(), files: None, root: PathBuf::from("/nowhere"), rel: String::new(), label: "side".to_string(), shown: String::new() }
    }

    fn parsed(s: &str) -> Json {
        ritsu_base::json::parse(s).unwrap()
    }

    fn side<'a>(at: &'a At, api: &'a Json) -> Side<'a> {
        Side { at, api: Some(api) }
    }

    /// The JSON of a project of one requirement, as `yuen api` prints it.
    fn project_of(text: &str, line: u32, status: &str, date: &str, up: &str, down: &str) -> Json {
        parsed(&format!(
            r#"{{"sources":[{{"file":"r.req","name":"law","kind":"law","id":"29 CFR 1","asof":"2024-01-01"}}],
            "requirements":[{{"name":"a","version":1,"file":"r.req","line":{line},"text":"{text}","in_force":{{"from":"2024-01-01","to":null}},"owner":"m","sha256":"{up}",
              "from":[{{"line":2,"sources":[{{"source":"law","fragment":"§1","sha256":"{up}"}}],"requirement":null,"reviewed":{{"date":"{date}","by":"m"}},"status":"{status}"}}],
              "decided":[{{"date":"2024-01-01","by":"m","why":"because"}}],
              "links":[{{"line":3,"role":"satisfied","artifact":{{"text":"rulec \"r.rule\" table t","path":"r.rule"}},"sha256":"{down}","reviewed":{{"date":"{date}","by":"m"}},"status":"{status}"}}],
              "waivers":[]}}]}}"#
        ))
    }

    /// Where a requirement is written and who looked at it say nothing of what moved.
    #[test]
    fn a_new_look_at_the_same_ends_moves_nothing() {
        let a = at();
        let old = project_of("the same", 10, "ok", "2024-01-01", "aaaa", "bbbb");
        let new = project_of("the same", 12, "ok", "2025-01-01", "aaaa", "bbbb");
        assert!(whys(&side(&a, &old), &side(&a, &new)).is_empty());
    }

    /// What moved is said by what changed, and a decision the old side had is not said again.
    #[test]
    fn what_a_changed_requirement_says() {
        let a = at();
        let old = project_of("old words", 10, "ok", "2024-01-01", "aaaa", "bbbb");
        let new = project_of("new words", 10, "ok", "2025-01-01", "cccc", "bbbb");
        let w = whys(&side(&a, &old), &side(&a, &new));
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].how, How::Changed);
        assert_eq!(w[0].text, (Some("old words".to_string()), Some("new words".to_string())));
        assert!(w[0].decided.is_empty());
        assert_eq!(w[0].from[0].how, Some(How::Changed));
        assert_eq!(w[0].from[0].id.as_deref(), Some("29 CFR 1"));
        assert_eq!(w[0].links[0].how, None);
        assert_eq!(w[0].looked, [("2025-01-01".to_string(), "m".to_string())]);
        assert_eq!(w[0].unsettled, 0);
    }

    /// A record that no longer holds is no one having looked.
    #[test]
    fn a_link_no_record_holds_for_is_counted() {
        let a = at();
        let old = project_of("old", 10, "ok", "2024-01-01", "aaaa", "bbbb");
        let new = project_of("new", 10, "down_changed", "2024-01-01", "aaaa", "cccc");
        let w = whys(&side(&a, &old), &side(&a, &new));
        assert_eq!(w[0].unsettled, 2);
        assert!(w[0].looked.is_empty());
    }

    /// A requirement only one side has is said as added or removed, and nothing of it as changed.
    #[test]
    fn a_requirement_one_side_has() {
        let a = at();
        let new = project_of("words", 10, "ok", "2025-01-01", "aaaa", "bbbb");
        let none = parsed(r#"{"sources":[],"requirements":[]}"#);
        let added = whys(&side(&a, &none), &side(&a, &new));
        assert_eq!((added[0].how, added[0].from[0].how, added[0].links[0].how, added[0].decided.len()), (How::Added, None, None, 1));
        let removed = whys(&side(&a, &new), &side(&a, &none));
        assert_eq!((removed[0].how, removed[0].from.len(), removed[0].text.0.as_deref()), (How::Removed, 0, Some("words")));
    }

    /// A fence is longer than any run of backticks a module's text holds.
    #[test]
    fn a_fence_outlasts_the_text() {
        assert_eq!(fence_for(&[('-', "plain".to_string())]), "```");
        assert_eq!(fence_for(&[('-', "a ```` b".to_string())]), "`````");
    }
}
