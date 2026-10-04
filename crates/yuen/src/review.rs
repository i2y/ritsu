//! `yuen review` (DESIGN 4.2, 4.4, PLAN B.10): what a person has looked at, written down.
//!
//! For every chosen link or waiver that is marked, the record under it is written anew — the
//! day, the role, and the hashes of the ends now — and what was looked at goes into
//! `reviewed/<hash>` beside the `.req`, so the next change can be shown as a diff. Nothing
//! else of the file changes, not even its line endings. `check` never writes; this is the one
//! command that writes hashes into a `.req` (DESIGN P3).

use crate::check::{self, Checked};
use crate::date::Day;
use crate::diag::{self, Diag};
use ritsu_base::text::{Lang, Text};
use crate::marks::{LinkKind, LinkState, Status};
use crate::project::{Project, Refusal};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// What to look at: the lines, the requirements, or everything.
pub struct Choice<'a> {
    pub at: Vec<&'a str>,
    pub requirements: Vec<&'a str>,
    pub all: bool,
}

/// The largest content kept in `reviewed/` (DESIGN 4.4).
pub const MAX_KEPT: usize = 1 << 20;

pub struct Outcome {
    /// What `review` says, a line each.
    pub lines: Vec<Text>,
    /// Diagnostics, when the project has errors that stop it (stages 1 and 2).
    pub diags: Vec<Diag>,
    pub exit: u8,
}

/// The record a link gets now.
fn record_text(st: &LinkState, date: Day, by: &str) -> String {
    let up: Vec<String> = st.up.iter().flatten().map(|e| format!("sha256:{}", e.end.hash)).collect();
    match st.kind {
        LinkKind::Waiver(_) => format!("approved {date} by {by} {}", up.join(", ")),
        _ => format!("reviewed {date} by {by} {} -> sha256:{}", up.join(", "), st.down.as_ref().map(|e| e.end.hash.as_str()).unwrap_or("")),
    }
}

/// What a code says, short, for the line `review` writes per record.
fn what_it_was(st: &LinkState) -> Text {
    match &st.status {
        Status::Unreviewed => tr!("まだ確かめていなかった", "not looked at yet"),
        Status::UpChanged(_) => tr!("リンク元が変わっていた", "the upper end had changed"),
        Status::DownChanged => tr!("リンク先が変わっていた", "the lower end had changed"),
        Status::Unapproved => tr!("まだ承認していなかった", "not approved yet"),
        Status::WaiverChanged => tr!("承認のあとで要件が変わっていた", "the requirement had changed since the approval"),
        Status::BadRecord(..) => tr!("記録の形が崩れていた", "the record was not written right"),
        Status::Ok | Status::Unreadable => Text::default(),
    }
}

/// A file's lines with their endings, as bytes are kept.
fn split_lines(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = src;
    while let Some(i) = rest.find('\n') {
        out.push(rest[..=i].to_string());
        rest = &rest[i + 1..];
    }
    if !rest.is_empty() {
        out.push(rest.to_string());
    }
    out
}

fn ending(line: &str) -> &str {
    if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        ""
    }
}

/// One edit of a file: replace line `line`, or put a line after it.
struct Edit {
    line: usize,
    replace: bool,
    text: String,
}

fn apply(src: &str, mut edits: Vec<Edit>) -> String {
    let mut lines = split_lines(src);
    edits.sort_by(|a, b| b.line.cmp(&a.line));
    for e in edits {
        let i = e.line - 1;
        if e.replace {
            let old = lines[i].clone();
            let end = ending(&old).to_string();
            let body = &old[..old.len() - end.len()];
            let indent: String = body.chars().take_while(|c| *c == ' ').collect();
            // A comment at the end of the record stays.
            let comment = body.find('#').map(|k| {
                let before = body[..k].trim_end().len();
                body[before..].to_string()
            });
            lines[i] = format!("{indent}{}{}{end}", e.text, comment.unwrap_or_default());
        } else {
            let link = lines[i].clone();
            let mut end = ending(&link).to_string();
            if end.is_empty() {
                // The link is the last line, without a line break: it gets one.
                lines[i].push('\n');
                end = String::new();
            }
            let indent: String = link.chars().take_while(|c| *c == ' ').collect();
            lines.insert(i + 1, format!("{indent}  {}{end}", e.text));
        }
    }
    lines.concat()
}

/// Where a `--at` points: the file and the line.
fn parse_at(at: &str) -> Option<(PathBuf, usize)> {
    let (f, l) = at.rsplit_once(':')?;
    let line = l.parse().ok()?;
    let abs = std::fs::canonicalize(f).ok()?;
    Some((abs, line))
}

fn choose<'a>(p: &Project, states: &'a [LinkState], c: &Choice) -> Result<Vec<(&'a LinkState, bool)>, Text> {
    let mut out: Vec<(&LinkState, bool)> = Vec::new();
    let push = |st: &'a LinkState, named: bool, out: &mut Vec<(&'a LinkState, bool)>| {
        if let Some(e) = out.iter_mut().find(|(s, _)| std::ptr::eq(*s, st)) {
            e.1 |= named;
        } else {
            out.push((st, named));
        }
    };
    for at in &c.at {
        let Some((abs, line)) = parse_at(at) else {
            return Err(tr!("`--at {at}` は `<file.req>:<行>` の形ではないか、そのファイルがありません", "`--at {at}` is not `<file.req>:<line>`, or there is no such file"));
        };
        let found: Vec<&LinkState> = states
            .iter()
            .filter(|st| {
                let f = p.file_of(st.req);
                f.abs == abs && (st.line == line || st.record(p).is_some_and(|r| r.line == line))
            })
            .collect();
        if found.is_empty() {
            return Err(tr!("{at} にはリンクも見送りもありません", "There is no link or waiver at {at}"));
        }
        for st in found {
            push(st, true, &mut out);
        }
    }
    for spec in &c.requirements {
        let rs = p.find_req(spec)?;
        for st in states.iter().filter(|s| rs.contains(&s.req)) {
            push(st, false, &mut out);
        }
    }
    if c.all {
        for st in states {
            push(st, false, &mut out);
        }
    }
    out.sort_by_key(|(s, _)| (p.reqs[s.req].file, s.line));
    Ok(out)
}

pub fn review(args: &[String], root: Option<&str>, c: &Choice, by: &str, date: Option<&str>) -> Result<Outcome, Refusal> {
    review_with(args, root, c, by, date, crate::suite::Suite::default())
}

/// [`review`], reading what another language holds through the ports `suite` joins.
pub fn review_with(args: &[String], root: Option<&str>, c: &Choice, by: &str, date: Option<&str>, suite: crate::suite::Suite) -> Result<Outcome, Refusal> {
    if c.at.is_empty() && c.requirements.is_empty() && !c.all {
        return Err(Refusal(tr!("何を確かめたかを `--at`、`--requirement`、`--all` のどれかで選んでください", "Choose what was looked at with `--at`, `--requirement` or `--all`")));
    }
    let date = match date {
        Some(d) => Day::parse(d).ok_or_else(|| Refusal(tr!("`--date {d}` は日付（`2026-10-04` の形）ではありません", "`--date {d}` is not a date (`2026-10-04`)")))?,
        None => crate::date::today(),
    };
    let checked: Checked = check::check_with(args, root, suite)?;
    let Some(m) = checked.model.as_ref() else {
        let exit = if check::has_unjoined(&checked.diags) { 2 } else { 1 };
        return Ok(Outcome { lines: vec![tr!("構文か名前にエラーがあるので、何も書きませんでした", "Nothing was written: the words or the names have errors")], diags: checked.diags, exit });
    };
    let p = checked.project.as_ref().unwrap();
    if !p.files.iter().any(|f| f.ast.roles.iter().any(|r| r.name == by)) {
        let d = diag::error("E008", "--by", "--by", 0, 0, tr!("役割「{by}」はこのプロジェクトで宣言されていません", "The role {by} is not declared in this project"))
            .note(tr!("確かめた人の役割には、`role {by}` のように宣言した役割を書いてください。", "Whoever looked is named by a declared role (`role {by}`)."));
        return Ok(Outcome { lines: vec![tr!("何も書きませんでした", "Nothing was written")], diags: vec![d], exit: 1 });
    }
    let chosen = choose(p, &m.states, c).map_err(Refusal)?;
    let mut edits: BTreeMap<usize, Vec<Edit>> = BTreeMap::new();
    let mut keep: BTreeMap<usize, Vec<(String, Vec<u8>)>> = BTreeMap::new();
    let mut lines = Vec::new();
    let mut refused = false;
    let mut unmarked = 0;
    for (st, named) in chosen {
        let f = p.file_of(st.req);
        let at = format!("{}:{}", f.display, st.line);
        match &st.status {
            Status::Unreadable => {
                lines.push(tr!(
                    "{at}: リンクの両端を読めないので、書きませんでした（先に check の診断を直してください）",
                    "{at}: not written, since the ends of the link cannot be read (correct what check says first)"
                ));
                refused = true;
                continue;
            }
            Status::Ok => {
                if named {
                    lines.push(tr!("{at}: 印が無いので、書きませんでした（確かめたときのままです）", "{at}: not written, since nothing is marked (it is as it was looked at)"));
                }
                unmarked += 1;
                continue;
            }
            _ => {}
        }
        let text = record_text(st, date, by);
        let code = st.status.code().unwrap_or("");
        let what = what_it_was(st);
        lines.push(tr!("{at}: {code}（{}）— {text}", "{at}: {code} ({}) — {text}", what.ja; what.en));
        let fi = p.reqs[st.req].file;
        let (line, replace) = match st.record(p) {
            Some(r) => (r.line, true),
            None => (st.line, false),
        };
        edits.entry(fi).or_default().push(Edit { line, replace, text });
        for e in st.up.iter().flatten().chain(st.down.iter()) {
            if e.end.bytes.len() <= MAX_KEPT && std::str::from_utf8(&e.end.bytes).is_ok() {
                keep.entry(fi).or_default().push((e.end.hash.clone(), e.end.bytes.clone()));
            }
        }
    }
    // Write the files, then what was looked at, then clear what no record points at.
    let mut wrote = 0;
    for (fi, es) in edits {
        let f = &p.files[fi];
        wrote += es.len();
        let new = apply(&f.src, es);
        std::fs::write(&f.abs, new).map_err(|e| {
            let d = &f.display;
            Refusal(tr!("`{d}` を書けません: {e}", "Cannot write `{d}`: {e}"))
        })?;
    }
    let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();
    for (fi, items) in keep {
        let dir = p.files[fi].abs.parent().unwrap().join("reviewed");
        std::fs::create_dir_all(&dir).map_err(|e| Refusal(tr!("reviewed/ を作れません: {e}", "Cannot make reviewed/: {e}")))?;
        for (h, b) in items {
            let path = dir.join(&h);
            if !path.exists() {
                std::fs::write(&path, &b).map_err(|e| Refusal(tr!("reviewed/{h} を書けません: {e}", "Cannot write reviewed/{h}: {e}")))?;
            }
        }
        dirs.insert(dir);
    }
    if wrote > 0 {
        for f in &p.files {
            dirs.insert(f.abs.parent().unwrap().join("reviewed"));
        }
        for dir in dirs {
            clear(&dir);
        }
    }
    if unmarked > 0 && wrote == 0 && !refused {
        lines.push(tr!("選んだリンクと見送りには印が無いので、何も書きませんでした", "Nothing chosen was marked, so nothing was written"));
    } else {
        let n = wrote;
        lines.push(tr!("記録を {n} 件書きました", "Wrote {}", ; ritsu_base::text::plural(n, "record", "records")));
    }
    Ok(Outcome { lines, diags: vec![], exit: if refused { 1 } else { 0 } })
}

/// Remove from `reviewed/` what no `.req` beside it points at: only files named with sixteen
/// hex digits (DESIGN 4.4).
fn clear(dir: &Path) {
    let Some(parent) = dir.parent() else { return };
    let Ok(rd) = std::fs::read_dir(parent) else { return };
    let mut wanted = BTreeSet::new();
    for e in rd.filter_map(|e| e.ok()) {
        let path = e.path();
        if path.extension().is_some_and(|x| x == "req")
            && let Ok(src) = std::fs::read_to_string(&path)
        {
            wanted.extend(crate::marks::hashes_in(&src));
        }
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().to_string();
        if name.len() == 16 && name.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) && !wanted.contains(&name) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// What `review` prints.
pub fn render(o: &Outcome, lang: Lang) -> String {
    let mut s = String::new();
    for d in &o.diags {
        s.push_str(&d.render(lang));
    }
    for l in &o.lines {
        s.push_str(l.get(lang));
        s.push('\n');
    }
    s
}
