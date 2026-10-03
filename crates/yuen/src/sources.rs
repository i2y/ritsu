//! The third stage (DESIGN 1.4, 5.1, PLAN B.4): every source's copies against their pins, and
//! every citation against the sources (E101–E105, W101). `check` never reads the network: a
//! copy that is not there is for `yuen source fetch` to bring.

use crate::ast::*;
use crate::copies;
use crate::diag::{Diag, DiagExt};
use crate::names::Name;
use crate::project::Project;
use ritsu_base::sha256;
use std::path::PathBuf;

/// One article of a law a `.req` pins.
#[derive(Clone, Debug)]
pub struct Article {
    pub fragment: String,
    /// The copy's path from the root (DESIGN 2.2); `Project::shown` writes it for a person.
    pub rel: String,
    pub abs: PathBuf,
    pub pin: Option<String>,
    /// The copy, when it is there, reads as a copy of the article, and matches its pin.
    pub bytes: Option<Vec<u8>>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Resolved {
    Law { db: LawDb, id: String, asof: String, dir: PathBuf, revision: Option<String>, articles: Vec<Article> },
    File { name: Name, abs: PathBuf, url: Option<String>, pin: Option<String>, bytes: Option<Vec<u8>> },
    /// A source a rule or a calendar pins (DESIGN 1.4, 3.3), read through the tool's JSON.
    Borrowed { name: Name },
    /// The naming of a borrowed source or the path of a file source did not resolve (stage 2).
    Broken,
}

/// The sources of every file, in the order they are declared.
pub struct Sources {
    pub files: Vec<Vec<(String, Resolved)>>,
}

/// An upper end of a `from @…` line: what the requirement's end writes for it (DESIGN 4.1),
/// and the copy it is.
#[derive(Clone, Debug)]
pub struct Cited {
    /// `from law egov 129AC0000000089 第140条 sha256:…` or `from file docs/x.md sha256:…`.
    pub line: String,
    pub hash: String,
    pub bytes: Vec<u8>,
    /// The source and the article as a person reads them: `民法 第140条`, `約款`.
    pub label: String,
    /// For a law: the database and the copy's file, to quote it.
    pub law: Option<(LawDb, String)>,
}

impl Sources {
    pub fn get(&self, fi: usize, name: &str) -> Option<&Resolved> {
        self.files[fi].iter().find(|(n, _)| n == name).map(|(_, r)| r)
    }

    /// What one citation of a `from` line resolves to: one end per article (one for a file
    /// source), or None when any of them cannot be read (that was said in this stage).
    pub fn cited(&self, fi: usize, source: &str, fragments: &[(String, Span)]) -> Option<Vec<Cited>> {
        match self.get(fi, source)? {
            Resolved::Law { db, id, articles, .. } => {
                if fragments.is_empty() {
                    return None;
                }
                let mut out = Vec::new();
                for (fr, _) in fragments {
                    let a = articles.iter().find(|a| &a.fragment == fr)?;
                    let bytes = a.bytes.clone()?;
                    let hash = sha256::short(&bytes);
                    let file = a.abs.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    out.push(Cited {
                        line: format!("from law {} {id} {fr} sha256:{hash}", db.word()),
                        hash,
                        bytes,
                        label: format!("{source} {fr}"),
                        law: Some((*db, file)),
                    });
                }
                Some(out)
            }
            Resolved::File { name, bytes, .. } => {
                if !fragments.is_empty() {
                    return None;
                }
                let bytes = bytes.clone()?;
                let hash = sha256::short(&bytes);
                Some(vec![Cited { line: format!("from file {} sha256:{hash}", name.path), hash, bytes, label: source.to_string(), law: None }])
            }
            Resolved::Borrowed { .. } | Resolved::Broken => None,
        }
    }
}

/// Check every source and every citation of the project.
pub fn check_sources(p: &Project) -> (Sources, Vec<Diag>) {
    let mut diags = Vec::new();
    let mut files = Vec::new();
    for (fi, f) in p.files.iter().enumerate() {
        let req_dir = f.abs.parent().map(|d| d.to_path_buf()).unwrap_or_default();
        let line_of = |n: usize| f.src.lines().nth(n.saturating_sub(1)).unwrap_or("").to_string();
        let mut out = Vec::new();
        for (si, s) in f.ast.sources.iter().enumerate() {
            let r = match &s.kind {
                SourceKind::Law { db, id, asof, pins } => {
                    let asof = asof.to_string();
                    let dir_shown = copies::copy_dir(id, &asof);
                    let dir = req_dir.join(&dir_shown);
                    let mut articles = Vec::new();
                    for pl in pins {
                        let Some(file) = copies::fragment_file(*db, &pl.fragment) else {
                            let fr = &pl.fragment;
                            diags.push(p.err(fi, "E105", pl.span, tr!("`{fr}` は条の書き方になっていません", "`{fr}` is not written as an article")).note(copies::fragment_shapes(*db)));
                            continue;
                        };
                        let rel = ritsu_base::paths::join(&f.dir, &format!("{dir_shown}/{file}")).ok().unwrap_or_else(|| format!("{dir_shown}/{file}"));
                        let shown = p.shown(&rel);
                        let abs = dir.join(&file);
                        let mut a = Article { fragment: pl.fragment.clone(), rel, abs: abs.clone(), pin: pl.pin.clone(), bytes: None, span: pl.span };
                        let name = &s.name;
                        let fr = &pl.fragment;
                        match std::fs::read(&abs) {
                            Err(_) => diags.push(p.err(fi, "E101", pl.span, tr!("{name} {fr} の写し {shown} がありません", "The copy of {name} {fr} is not there: {shown}")).note(tr!(
                                "`yuen source fetch` が {} から取ってきて、そこに書きます。check は通信しません。",
                                "`yuen source fetch` takes it from {} and writes it there; check never reads the network.",
                                db.title()
                            ))),
                            Ok(bytes) => {
                                let actual = sha256::short(&bytes);
                                if let Err(why) = copies::readable(*db, &file, &bytes) {
                                    diags.push(p.err(fi, "E104", pl.span, tr!("{name} {fr} の写し {shown} が読めません: {}", "The copy {shown} of {name} {fr} cannot be read: {}", why.ja; why.en)).note(tr!(
                                        "写しは {} が配る XML のままにします。手で直さず、`yuen source fetch` で取り直します。",
                                        "A copy is the XML {} serves, as served; fetch it again with `yuen source fetch` rather than editing it.",
                                        db.title()
                                    )));
                                } else {
                                    match &pl.pin {
                                        None => diags.push(
                                            p.err(fi, "E102", pl.span, tr!("{name} {fr} が固定されていません（`sha256:` がありません）", "{name} {fr} is not pinned (it has no `sha256:`)"))
                                                .note(tr!("いまの写しなら sha256:{actual} です（`yuen source pin` でも書けます）。", "For the copy as it is, that is sha256:{actual} (`yuen source pin` writes it too)."))
                                                .fix_trimmed(ritsu_base::sources::fixed_pin_line(&line_of(pl.span.line), &actual)),
                                        ),
                                        Some(pin) if *pin != actual => diags.push(
                                            p.err(fi, "E103", pl.span, tr!(
                                                "{name} {fr} の写しが固定と違います（固定は sha256:{pin}、写しは sha256:{actual}）",
                                                "The copy of {name} {fr} does not match its pin (pinned sha256:{pin}, the copy is sha256:{actual})"
                                            ))
                                            .note(tr!(
                                                "固定したあとで写しが変わりました。条文の何が変わったかを読んでから、固定を書き換えます。",
                                                "The copy changed after it was pinned. Read what changed in the text, then pin it again."
                                            ))
                                            .fix_trimmed(ritsu_base::sources::fixed_pin_line(&line_of(pl.span.line), &actual)),
                                        ),
                                        Some(_) => a.bytes = Some(bytes),
                                    }
                                }
                            }
                        }
                        articles.push(a);
                    }
                    Resolved::Law { db: *db, id: id.clone(), asof, revision: copies::revision(&dir), dir, articles }
                }
                SourceKind::File { url, pin, .. } => match &p.names.sources[fi][si] {
                    None => Resolved::Broken,
                    Some(name) => {
                        let abs = p.root.join(&name.path);
                        let shown = p.shown(&name.path);
                        let sname = &s.name;
                        let mut bytes = None;
                        match std::fs::read(&abs) {
                            Err(_) => {
                                let mut d = p.err(fi, "E101", s.span, tr!("出典「{sname}」の写し {shown} がありません", "The copy of the source {sname} is not there: {shown}"));
                                d = match url {
                                    Some(_) => d.note(tr!(
                                        "`yuen source fetch` が url から取ってきて、そこに書きます。check は通信しません。",
                                        "`yuen source fetch` takes it from the url and writes it there; check never reads the network."
                                    )),
                                    None => d.note(tr!("写しは .req からの相対パスで探します。ファイルを置くか、パスを直します。", "The copy is looked for from the directory of the .req; put the file there or correct the path.")),
                                };
                                diags.push(d);
                            }
                            Ok(b) => {
                                let actual = sha256::short(&b);
                                match pin {
                                    None => diags.push(
                                        p.err(fi, "E102", s.span, tr!("出典「{sname}」が固定されていません（`sha256:` がありません）", "The source {sname} is not pinned (it has no `sha256:`)"))
                                            .note(tr!("いまの写しなら sha256:{actual} です（`yuen source pin` でも書けます）。", "For the copy as it is, that is sha256:{actual} (`yuen source pin` writes it too)."))
                                            .fix_trimmed(ritsu_base::sources::fixed_pin_line(&line_of(s.span.line), &actual)),
                                    ),
                                    Some(pn) if *pn != actual => diags.push(
                                        p.err(fi, "E103", s.span, tr!(
                                            "出典「{sname}」の写しが固定と違います（固定は sha256:{pn}、写しは sha256:{actual}）",
                                            "The copy of the source {sname} does not match its pin (pinned sha256:{pn}, the copy is sha256:{actual})"
                                        ))
                                        .note(tr!(
                                            "固定したあとで写しが変わりました。何が変わったかを読んでから（`yuen source outdated`）、固定を書き換えます。",
                                            "The copy changed after it was pinned. Read what changed (`yuen source outdated`), then pin it again."
                                        ))
                                        .fix_trimmed(ritsu_base::sources::fixed_pin_line(&line_of(s.span.line), &actual)),
                                    ),
                                    Some(_) => bytes = Some(b),
                                }
                            }
                        }
                        Resolved::File { name: name.clone(), abs, url: url.clone(), pin: pin.clone(), bytes }
                    }
                },
                SourceKind::Borrowed { .. } => match &p.names.sources[fi][si] {
                    Some(n) => Resolved::Borrowed { name: n.clone() },
                    None => Resolved::Broken,
                },
            };
            out.push((s.name.clone(), r));
        }
        files.push(out);
    }
    let sources = Sources { files };
    // Citations (E105, E102 for an article cited and not pinned) and what is pinned and not
    // cited (W101).
    for (fi, f) in p.files.iter().enumerate() {
        let mut cited: Vec<(String, String)> = Vec::new();
        for r in &f.ast.requirements {
            for fl in &r.from {
                let FromWhat::Cite { source, source_span, fragments } = &fl.what else { continue };
                let Some(decl) = f.ast.sources.iter().find(|s| &s.name == source) else {
                    diags.push(p.err(fi, "E105", *source_span, tr!("出典「{source}」はこのファイルで宣言されていません", "The source {source} is not declared in this file")).note(tr!(
                        "引く出典は、同じファイルに `source {source} = law \"<法令ID>\" asof <日付>` のように宣言します。出典の名前はファイルごとに分かれます。",
                        "Declare the source in the same file, like `source {source} = law \"<law id>\" asof <date>`; source names belong to a file."
                    )));
                    continue;
                };
                match &decl.kind {
                    SourceKind::Law { db, id, asof, pins } => {
                        if fragments.is_empty() {
                            diags.push(p.err(fi, "E105", *source_span, tr!(
                                "法令は条の単位で写すので、`@{source} 第140条` のように、どこを引いたかを書きます",
                                "A law is copied an article at a time, so say which one: `@{source} 第140条`"
                            )));
                            continue;
                        }
                        for (fr, sp) in fragments {
                            cited.push((source.clone(), fr.clone()));
                            let Some(file) = copies::fragment_file(*db, fr) else {
                                diags.push(p.err(fi, "E105", *sp, tr!("`{fr}` は条の書き方になっていません", "`{fr}` is not written as an article")).note(copies::fragment_shapes(*db)));
                                continue;
                            };
                            if !pins.iter().any(|pl| &pl.fragment == fr) {
                                let path = f.abs.parent().unwrap().join(copies::copy_dir(id, &asof.to_string())).join(&file);
                                let indent = pins.first().map(|pl| " ".repeat(pl.span.col - 1)).unwrap_or_else(|| "  ".into());
                                let shown = if fr.chars().all(|c| c.is_alphanumeric() || c == '_') { fr.clone() } else { crate::names::quote(fr) };
                                let fix = match std::fs::read(&path) {
                                    Ok(b) => format!("{indent}{shown} sha256:{}", sha256::short(&b)),
                                    Err(_) => format!("{indent}{shown} sha256:<yuen source fetch, then yuen source pin>"),
                                };
                                diags.push(
                                    p.err(fi, "E102", *sp, tr!("{source} {fr} を引いていますが、固定されていません", "{source} {fr} is cited but not pinned"))
                                        .note(tr!(
                                            "引く条は、出典の下に固定の行を書きます。どの版の条文を読んで要件を書いたかを、固定で残すためです。",
                                            "Every article cited has a pin line under its source: it records which version of the text the requirement was read from."
                                        ))
                                        .fix_trimmed(fix),
                                );
                            }
                        }
                    }
                    SourceKind::File { .. } => {
                        if let Some((fr, sp)) = fragments.first() {
                            diags.push(p.err(fi, "E105", *sp, tr!("出典「{source}」はファイルを丸ごと固定しているので、`{fr}` のように箇所を引けません", "The source {source} pins a whole file, so no part of it such as `{fr}` can be cited")).note(tr!(
                                "`from @{source}` と丸ごと引き、どの段落かは要件の文に書きます。",
                                "Cite it whole, `from @{source}`, and say which paragraph in the requirement's text."
                            )));
                        }
                    }
                    SourceKind::Borrowed { .. } => {}
                }
            }
        }
        for s in &f.ast.sources {
            let SourceKind::Law { pins, .. } = &s.kind else { continue };
            for pl in pins {
                if !cited.iter().any(|(n, fr)| n == &s.name && fr == &pl.fragment) {
                    let fr = &pl.fragment;
                    let name = &s.name;
                    diags.push(p.warn(fi, "W101", pl.span, tr!("{name} {fr} は固定されていますが、どの要件からも引かれていません", "{name} {fr} is pinned but no requirement cites it")).note(tr!(
                        "引用を消したあとの残りなら、固定の行を消します。",
                        "If it is what is left after a citation was removed, delete the pin line."
                    )));
                }
            }
        }
    }
    diags.sort_by(|a, b| (a.file.as_str(), a.line, a.col).cmp(&(b.file.as_str(), b.line, b.col)));
    (sources, diags)
}
