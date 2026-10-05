//! The third stage (DESIGN 1.4, 5.1, PLAN B.4): every source's copies against their pins, and
//! every citation against the sources (E101–E105, W101). `check` never reads the network: a
//! copy that is not there is for `yuen source fetch` to bring.

use crate::ast::*;
use crate::copies;
use crate::diag::{Diag, DiagExt};
use ritsu_base::text::Text;
use crate::names::Name;
use crate::project::Project;
use ritsu_base::openspec;
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

/// One requirement of an OpenSpec spec a `.req` pins (DESIGN 20).
#[derive(Clone, Debug)]
pub struct SpecPin {
    /// The requirement's name, as OpenSpec's archive matches it.
    pub name: String,
    pub pin: Option<String>,
    /// Its block, when the spec reads, holds the requirement, and the block matches the pin.
    pub block: Option<String>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Resolved {
    /// A law, copied an article at a time. `borrowed` is the source of a rule or a calendar it is
    /// borrowed from (DESIGN 1.4, 3.3): then the copies and the pins are that file's, and its
    /// language's check holds them.
    Law { db: LawDb, id: String, asof: String, dir: PathBuf, revision: Option<String>, articles: Vec<Article>, borrowed: Option<Name> },
    File { name: Name, abs: PathBuf, url: Option<String>, pin: Option<String>, bytes: Option<Vec<u8>>, borrowed: Option<Name> },
    /// An OpenSpec spec, read a requirement at a time (DESIGN 20): the file, the requirements
    /// pinned, and the spec as read, when it reads.
    OpenSpec { name: Name, abs: PathBuf, pins: Vec<SpecPin>, spec: Option<openspec::Spec> },
    /// A borrowed source whose language is not handed to yuen: `ritsu yuen` reads it (exit 2).
    NoPort { name: Name },
    /// The naming of a borrowed source or the path of a file source did not resolve (stage 2),
    /// or the source could not be borrowed (E106, E203).
    Broken,
}

impl Resolved {
    /// The source of a rule or a calendar it is borrowed from.
    pub fn borrowed(&self) -> Option<&Name> {
        match self {
            Resolved::Law { borrowed, .. } | Resolved::File { borrowed, .. } => borrowed.as_ref(),
            _ => None,
        }
    }
}

/// The sources of every file, in the order they are declared.
pub struct Sources {
    pub files: Vec<Vec<(String, Resolved)>>,
}

/// An upper end of a `from @…` line: what the requirement's end writes for it (DESIGN 4.1),
/// and the copy it is.
#[derive(Clone, Debug)]
pub struct Cited {
    /// `from law egov 129AC0000000089 第140条 sha256:…`, `from file docs/x.md sha256:…` or
    /// `from openspec openspec/specs/x/spec.md <requirement> sha256:…`.
    pub line: String,
    pub hash: String,
    pub bytes: Vec<u8>,
    /// The source and the article as a person reads them: `民法 第140条`, `約款`,
    /// `greeting "Greeting by name"`.
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
            Resolved::OpenSpec { name, pins, .. } => {
                if fragments.is_empty() {
                    return None;
                }
                let mut out = Vec::new();
                for (fr, _) in fragments {
                    let bytes = pins.iter().find(|x| &x.name == fr)?.block.clone()?.into_bytes();
                    let hash = sha256::short(&bytes);
                    out.push(Cited {
                        line: format!("from openspec {} {fr} sha256:{hash}", name.path),
                        hash,
                        bytes,
                        label: format!("{source} {}", crate::names::word_or_quote(fr)),
                        law: None,
                    });
                }
                Some(out)
            }
            Resolved::NoPort { .. } | Resolved::Broken => None,
        }
    }
}

/// A source a rule or a calendar pins, borrowed (DESIGN 1.4, 3.3): its pins and its copies are
/// that file's, as its language answers for it (`Sources`, which answers only for a file that
/// passes its check). The copies sit beside the file, where the language keeps them.
fn borrow(p: &Project, fi: usize, s: &SourceDecl, n: &Name, diags: &mut Vec<Diag>) -> Resolved {
    let tool = n.tool;
    let wanted = n.items.first().map(|(_, v)| v.clone()).unwrap_or_default();
    let abs = p.root.join(&n.path);
    let t = n.text();
    let e106 = |msg: Text| p.err(fi, "E106", s.span, msg);
    if !ritsu_base::fs::is_file(&abs) {
        diags.push(e106(tr!("{t} の {} がありません", "{} of {t} is not there", p.shown(&n.path))));
        return Resolved::Broken;
    }
    if !p.suite.reads(tool) {
        return Resolved::NoPort { name: n.clone() };
    }
    let declared = match p.suite.sources(tool, &abs) {
        Some(Ok(d)) => d,
        Some(Err(said)) => {
            diags.push(crate::check::refused(p, fi, s.span, n, &said));
            return Resolved::Broken;
        }
        None => return Resolved::NoPort { name: n.clone() },
    };
    let Some(src) = declared.iter().find(|x| x.name == wanted) else {
        let have: Vec<&str> = declared.iter().map(|x| x.name.as_str()).collect();
        let (ja, en) = (have.join("、"), have.join(", "));
        let shown = p.shown(&n.path);
        let mut d = e106(tr!("{shown} は出典「{wanted}」を宣言していません", "{shown} declares no source {wanted}"));
        d = d.note(if have.is_empty() {
            tr!("{shown} が宣言している出典はありません。", "{shown} declares no source at all.")
        } else {
            tr!("{shown} が宣言している出典: {ja}", "The sources {shown} declares: {en}")
        });
        diags.push(d);
        return Resolved::Broken;
    };
    let dir_rel = ritsu_base::paths::parent(&n.path);
    match &src.kind {
        ritsu_ports::SourceKind::Law { db, id, asof, pins } => {
            let db = if db == "ecfr" { LawDb::Ecfr } else { LawDb::Egov };
            let copy_dir = copies::copy_dir(id, asof);
            let dir = abs.parent().map(|d| d.join(&copy_dir)).unwrap_or_default();
            let mut articles = Vec::new();
            for (fragment, pin) in pins {
                let Some(file) = copies::fragment_file(db, fragment) else { continue };
                let rel = ritsu_base::paths::join(&dir_rel, &format!("{copy_dir}/{file}")).unwrap_or_else(|_| format!("{copy_dir}/{file}"));
                let copy = dir.join(&file);
                // the language's check holds the copy to its pin; one that does not match is not read
                let bytes = ritsu_base::fs::read(&copy).ok().filter(|b| sha256::short(b) == *pin);
                if bytes.is_none() {
                    let (shown, fr) = (p.shown(&rel), fragment);
                    diags.push(e106(tr!("{t} の {fr} のコピー {shown} を読めないか、固定と違います", "the copy {shown} of {fr} of {t} cannot be read, or does not match its pin")));
                }
                articles.push(Article { fragment: fragment.clone(), rel, abs: copy, pin: Some(pin.clone()), bytes, span: s.span });
            }
            Resolved::Law { db, id: id.clone(), asof: asof.clone(), revision: copies::revision(&dir), dir, articles, borrowed: Some(n.clone()) }
        }
        ritsu_ports::SourceKind::File { path, url, pin } => {
            let Ok(rel) = ritsu_base::paths::join(&dir_rel, path) else {
                diags.push(e106(tr!("{t} のコピーのパス `{path}` はルートの外に出ます", "the path `{path}` of the copy of {t} goes outside the root")));
                return Resolved::Broken;
            };
            let copy = p.root.join(&rel);
            let bytes = ritsu_base::fs::read(&copy).ok().filter(|b| pin.as_deref() == Some(sha256::short(b).as_str()));
            if bytes.is_none() {
                let shown = p.shown(&rel);
                diags.push(e106(tr!("{t} のコピー {shown} を読めないか、固定と違います", "the copy {shown} of {t} cannot be read, or does not match its pin")));
            }
            Resolved::File { name: Name { tool: crate::names::Tool::File, path: rel, items: vec![] }, abs: copy, url: url.clone(), pin: pin.clone(), bytes, borrowed: Some(n.clone()) }
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
                        match ritsu_base::fs::read(&abs) {
                            Err(_) => diags.push(p.err(fi, "E101", pl.span, tr!("{name} {fr} のコピー {shown} がありません", "The copy of {name} {fr} is not there: {shown}")).note(tr!(
                                "`yuen source fetch` が {} から取ってきて、そこに書きます。check は通信しません。",
                                "`yuen source fetch` takes it from {} and writes it there; check never reads the network.",
                                db.title()
                            ))),
                            Ok(bytes) => {
                                let actual = sha256::short(&bytes);
                                if let Err(why) = copies::readable(*db, &file, &bytes) {
                                    diags.push(p.err(fi, "E104", pl.span, tr!("{name} {fr} のコピー {shown} が読めません: {}", "The copy {shown} of {name} {fr} cannot be read: {}", why.ja; why.en)).note(tr!(
                                        "コピーは、{} が配る XML のままにしておいてください。手で直さず、`yuen source fetch` で取り直してください。",
                                        "A copy is the XML {} serves, as served; fetch it again with `yuen source fetch` rather than editing it.",
                                        db.title()
                                    )));
                                } else {
                                    match &pl.pin {
                                        None => diags.push(
                                            p.err(fi, "E102", pl.span, tr!("{name} {fr} が固定されていません（`sha256:` がありません）", "{name} {fr} is not pinned (it has no `sha256:`)"))
                                                .note(tr!("いまのコピーなら sha256:{actual} です（`yuen source pin` でも書けます）。", "For the copy as it is, that is sha256:{actual} (`yuen source pin` writes it too)."))
                                                .fix_trimmed(ritsu_base::sources::fixed_pin_line(&line_of(pl.span.line), &actual)),
                                        ),
                                        Some(pin) if *pin != actual => diags.push(
                                            p.err(fi, "E103", pl.span, tr!(
                                                "{name} {fr} のコピーが固定と違います（固定は sha256:{pin}、コピーは sha256:{actual}）",
                                                "The copy of {name} {fr} does not match its pin (pinned sha256:{pin}, the copy is sha256:{actual})"
                                            ))
                                            .note(tr!(
                                                "固定したあとでコピーが変わりました。条文の何が変わったかを読んでから、固定を書き換えてください。",
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
                    Resolved::Law { db: *db, id: id.clone(), asof, revision: copies::revision(&dir), dir, articles, borrowed: None }
                }
                SourceKind::File { url, pin, .. } => match &p.names.sources[fi][si] {
                    None => Resolved::Broken,
                    Some(name) => {
                        let abs = p.root.join(&name.path);
                        let shown = p.shown(&name.path);
                        let sname = &s.name;
                        let mut bytes = None;
                        match ritsu_base::fs::read(&abs) {
                            Err(_) => {
                                let mut d = p.err(fi, "E101", s.span, tr!("出典「{sname}」のコピー {shown} がありません", "The copy of the source {sname} is not there: {shown}"));
                                d = match url {
                                    Some(_) => d.note(tr!(
                                        "`yuen source fetch` が url から取ってきて、そこに書きます。check は通信しません。",
                                        "`yuen source fetch` takes it from the url and writes it there; check never reads the network."
                                    )),
                                    None => d.note(tr!("yuen はコピーを、.req からの相対パスで探します。ファイルを置くか、パスを直してください。", "The copy is looked for from the directory of the .req; put the file there or correct the path.")),
                                };
                                diags.push(d);
                            }
                            Ok(b) => {
                                let actual = sha256::short(&b);
                                match pin {
                                    None => diags.push(
                                        p.err(fi, "E102", s.span, tr!("出典「{sname}」が固定されていません（`sha256:` がありません）", "The source {sname} is not pinned (it has no `sha256:`)"))
                                            .note(tr!("いまのコピーなら sha256:{actual} です（`yuen source pin` でも書けます）。", "For the copy as it is, that is sha256:{actual} (`yuen source pin` writes it too)."))
                                            .fix_trimmed(ritsu_base::sources::fixed_pin_line(&line_of(s.span.line), &actual)),
                                    ),
                                    Some(pn) if *pn != actual => diags.push(
                                        p.err(fi, "E103", s.span, tr!(
                                            "出典「{sname}」のコピーが固定と違います（固定は sha256:{pn}、コピーは sha256:{actual}）",
                                            "The copy of the source {sname} does not match its pin (pinned sha256:{pn}, the copy is sha256:{actual})"
                                        ))
                                        .note(tr!(
                                            "固定したあとでコピーが変わりました。何が変わったかを読んでから（`yuen source outdated`）、固定を書き換えてください。",
                                            "The copy changed after it was pinned. Read what changed (`yuen source outdated`), then pin it again."
                                        ))
                                        .fix_trimmed(ritsu_base::sources::fixed_pin_line(&line_of(s.span.line), &actual)),
                                    ),
                                    Some(_) => bytes = Some(b),
                                }
                            }
                        }
                        Resolved::File { name: name.clone(), abs, url: url.clone(), pin: pin.clone(), bytes, borrowed: None }
                    }
                },
                SourceKind::Borrowed { .. } => match &p.names.sources[fi][si] {
                    Some(n) => borrow(p, fi, s, n, &mut diags),
                    None => Resolved::Broken,
                },
                SourceKind::OpenSpec { pins, .. } => match &p.names.sources[fi][si] {
                    Some(n) => spec_source(p, fi, s, n, pins, &mut diags),
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
                        "引く出典は、同じファイルに `source {source} = law \"<法令ID>\" asof <日付>` のように宣言してください。出典の名前は、ファイルごとに別々です。",
                        "Declare the source in the same file, like `source {source} = law \"<law id>\" asof <date>`; source names belong to a file."
                    )));
                    continue;
                };
                match &decl.kind {
                    SourceKind::Law { db, id, asof, pins } => {
                        if fragments.is_empty() {
                            diags.push(p.err(fi, "E105", *source_span, tr!(
                                "法令は条の単位で保存するので、`@{source} 第140条` のように、どこを引いたかを書いてください",
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
                                let fix = match ritsu_base::fs::read(&path) {
                                    Ok(b) => format!("{indent}{shown} sha256:{}", sha256::short(&b)),
                                    Err(_) => format!("{indent}{shown} sha256:<yuen source fetch, then yuen source pin>"),
                                };
                                diags.push(
                                    p.err(fi, "E102", *sp, tr!("{source} {fr} を引いていますが、固定されていません", "{source} {fr} is cited but not pinned"))
                                        .note(tr!(
                                            "引く条には、出典の下に固定の行を書いてください。どの版の条文を読んで要件を書いたかを、固定で残すためです。",
                                            "Every article cited has a pin line under its source: it records which version of the text the requirement was read from."
                                        ))
                                        .fix_trimmed(fix),
                                );
                            }
                        }
                    }
                    SourceKind::OpenSpec { pins, .. } => {
                        if fragments.is_empty() {
                            diags.push(p.err(fi, "E105", *source_span, tr!(
                                "OpenSpec の仕様は要件の単位で読むので、`@{source} \"<要件の名前>\"` のように、どの要件を引いたかを書いてください",
                                "An OpenSpec spec is read a requirement at a time, so say which one: `@{source} \"<requirement>\"`"
                            )));
                            continue;
                        }
                        let spec = match sources.get(fi, source) {
                            Some(Resolved::OpenSpec { spec: Some(spec), .. }) => Some(spec),
                            _ => None,
                        };
                        for (fr, sp) in fragments {
                            cited.push((source.clone(), fr.clone()));
                            if pins.iter().any(|pl| &pl.fragment == fr) {
                                continue;
                            }
                            let shown = crate::names::word_or_quote(fr);
                            let indent = pins.first().map(|pl| " ".repeat(pl.span.col - 1)).unwrap_or_else(|| "  ".into());
                            match spec.map(|s| s.get(fr)) {
                                Some(None) => {
                                    let spec_shown = match sources.get(fi, source) {
                                        Some(Resolved::OpenSpec { name, .. }) => p.shown(&name.path),
                                        _ => String::new(),
                                    };
                                    diags.push(no_such_requirement(p, fi, *sp, &spec_shown, fr, spec.unwrap()));
                                }
                                found => {
                                    let fix = match found.flatten() {
                                        Some(r) => format!("{indent}{shown} sha256:{}", sha256::short(r.block.as_bytes())),
                                        None => format!("{indent}{shown} sha256:<yuen source pin>"),
                                    };
                                    diags.push(
                                        p.err(fi, "E102", *sp, tr!("{source} {shown} を引いていますが、固定されていません", "{source} {shown} is cited but not pinned"))
                                            .note(tr!(
                                                "引く要件には、出典の下に固定の行を書いてください。どの版の要件を読んで書いたかを、固定で残すためです。",
                                                "Every requirement cited has a pin line under its source: it records which version of the requirement was read."
                                            ))
                                            .fix_trimmed(fix),
                                    );
                                }
                            }
                        }
                    }
                    SourceKind::File { .. } => {
                        if let Some((fr, sp)) = fragments.first() {
                            diags.push(p.err(fi, "E105", *sp, tr!("出典「{source}」はファイルを丸ごと固定しているので、`{fr}` のように箇所を引けません", "The source {source} pins a whole file, so no part of it such as `{fr}` can be cited")).note(tr!(
                                "`from @{source}` と丸ごと引き、どの段落かは要件の文に書いてください。",
                                "Cite it whole, `from @{source}`, and say which paragraph in the requirement's text."
                            )));
                        }
                    }
                    SourceKind::Borrowed { .. } => match sources.get(fi, source) {
                        Some(Resolved::Law { articles, borrowed: Some(n), .. }) => {
                            if fragments.is_empty() {
                                diags.push(p.err(fi, "E105", *source_span, tr!(
                                    "法令は条の単位で保存するので、`@{source} 第140条` のように、どこを引いたかを書いてください",
                                    "A law is copied an article at a time, so say which one: `@{source} 第140条`"
                                )));
                                continue;
                            }
                            for (fr, sp) in fragments {
                                if !articles.iter().any(|a| &a.fragment == fr) {
                                    let t = n.text();
                                    let pinned: Vec<&str> = articles.iter().map(|a| a.fragment.as_str()).collect();
                                    diags.push(p.err(fi, "E106", *sp, tr!("{t} は {fr} を固定していません", "{t} does not pin {fr}")).note(if pinned.is_empty() {
                                        tr!("借りた出典の条は、それを宣言しているファイルが固定しているものだけを引けます。", "Only the articles the file that declares the source pins can be cited.")
                                    } else {
                                        tr!("固定している条: {}。ほかの条を引くなら、そのファイルに固定の行を足してください。", "The articles it pins: {}. To cite another, add its pin to that file.", pinned.join("、"); pinned.join(", "))
                                    }));
                                }
                            }
                        }
                        Some(Resolved::File { .. }) => {
                            if let Some((fr, sp)) = fragments.first() {
                                diags.push(p.err(fi, "E105", *sp, tr!("出典「{source}」はファイルを丸ごと固定しているので、`{fr}` のように箇所を引けません", "The source {source} pins a whole file, so no part of it such as `{fr}` can be cited")));
                            }
                        }
                        _ => {}
                    },
                }
            }
        }
        for s in &f.ast.sources {
            let (SourceKind::Law { pins, .. } | SourceKind::OpenSpec { pins, .. }) = &s.kind else { continue };
            for pl in pins {
                if !cited.iter().any(|(n, fr)| n == &s.name && fr == &pl.fragment) {
                    let fr = &if matches!(s.kind, SourceKind::OpenSpec { .. }) { crate::names::word_or_quote(&pl.fragment) } else { pl.fragment.clone() };
                    let name = &s.name;
                    diags.push(p.warn(fi, "W101", pl.span, tr!("{name} {fr} は固定されていますが、どの要件からも引かれていません", "{name} {fr} is pinned but no requirement cites it")).note(tr!(
                        "引用を消したあとの残りなら、固定の行を消してください。",
                        "If it is what is left after a citation was removed, delete the pin line."
                    )));
                }
            }
        }
    }
    diags.extend(unpinned(p, &sources));
    diags.sort_by(|a, b| (a.file.as_str(), a.line, a.col).cmp(&(b.file.as_str(), b.line, b.col)));
    (sources, diags)
}

/// E108: a spec without a requirement of a name, with the near name or the names it has.
fn no_such_requirement(p: &Project, fi: usize, at: Span, shown: &str, name: &str, spec: &openspec::Spec) -> Diag {
    let n = crate::names::word_or_quote(name);
    let mut d = p.err(fi, "E108", at, tr!("仕様 {shown} に要件 {n} がありません", "The spec {shown} has no requirement {n}"));
    let near = spec.near(name);
    d = if let Some(m) = near.first() {
        let m = crate::names::word_or_quote(m);
        d.note(tr!(
            "近い名前の要件 {m} があります。OpenSpec は名前を、大文字と小文字や空白も含めて、書いたとおりに比べます。",
            "The spec has a requirement of a near name, {m}; OpenSpec compares names as written, case and spaces included."
        ))
    } else if spec.requirements.is_empty() {
        d.note(tr!("この仕様には要件がありません。", "The spec has no requirement."))
    } else {
        let names: Vec<String> = spec.requirements.iter().map(|r| crate::names::word_or_quote(&r.name)).collect();
        d.note(tr!("仕様にある要件: {}", "The requirements of the spec: {}", names.join("、"); names.join(", ")))
    };
    d.note(tr!(
        "変更を archive して名前が変わった（RENAMED）か、無くなった（REMOVED）のなら、その要件を読む要件を見直してから、固定と引用を直してください。",
        "If an archived change renamed the requirement (RENAMED) or removed it (REMOVED), look again at the requirements that read it, then correct the pin and the citations."
    ))
}

/// An OpenSpec spec a `.req` names (DESIGN 20.4): the spec against the pins of its requirements.
fn spec_source(p: &Project, fi: usize, s: &SourceDecl, name: &Name, pins: &[PinLine], diags: &mut Vec<Diag>) -> Resolved {
    let f = &p.files[fi];
    let line_of = |n: usize| f.src.lines().nth(n.saturating_sub(1)).unwrap_or("").to_string();
    let abs = p.root.join(&name.path);
    let shown = p.shown(&name.path);
    let sname = &s.name;
    let unread = |pins: &[PinLine]| pins.iter().map(|pl| SpecPin { name: pl.fragment.clone(), pin: pl.pin.clone(), block: None, span: pl.span }).collect();
    let bytes = match ritsu_base::fs::read(&abs) {
        Ok(b) => b,
        Err(_) => {
            diags.push(p.err(fi, "E101", s.span, tr!("出典「{sname}」の仕様 {shown} がありません", "The spec of the source {sname} is not there: {shown}")).note(tr!(
                "OpenSpec の仕様は取ってくるコピーではなく、プロジェクトのファイルです。パスを直してください。変更を archive して capability が無くなったのなら、それを読む要件を見直してください。",
                "An OpenSpec spec is a file of the project, not a copy to fetch: correct the path. If an archived change retired the capability, look again at the requirements that read it."
            )));
            return Resolved::OpenSpec { name: name.clone(), abs, pins: unread(pins), spec: None };
        }
    };
    let spec = match openspec::read_spec(&bytes) {
        Ok(spec) => spec,
        Err(e) => {
            let why = match e {
                openspec::SpecError::NotUtf8 => tr!("UTF-8 ではありません", "it is not UTF-8"),
                openspec::SpecError::NoRequirements { delta: true } => tr!(
                    "`## Requirements` の節がありません。これは変更の提案の差分（`## ADDED Requirements` など）です",
                    "it has no `## Requirements` section: it is a change's delta spec (`## ADDED Requirements` and the like)"
                ),
                openspec::SpecError::NoRequirements { delta: false } => tr!("`## Requirements` の節がありません", "it has no `## Requirements` section"),
                openspec::SpecError::Twice { name, first, line } => tr!("要件「{name}」が {first} 行目と {line} 行目の二か所にあります", "the requirement {name} is written twice, on lines {first} and {line}"),
            };
            diags.push(p.err(fi, "E104", s.span, tr!("出典「{sname}」の仕様 {shown} が読めません: {}", "The spec {shown} of the source {sname} cannot be read: {}", why.ja; why.en)).note(tr!(
                "出典に書くのは `openspec/specs/<capability>/spec.md` です。仕様の形の誤りは `openspec validate --specs` が言います。",
                "A source names `openspec/specs/<capability>/spec.md`; `openspec validate --specs` says what is wrong with the form of a spec."
            )));
            return Resolved::OpenSpec { name: name.clone(), abs, pins: unread(pins), spec: None };
        }
    };
    let mut out = Vec::new();
    for pl in pins {
        let rname = &pl.fragment;
        let n = crate::names::word_or_quote(rname);
        let mut sp = SpecPin { name: rname.clone(), pin: pl.pin.clone(), block: None, span: pl.span };
        match spec.get(rname) {
            None => diags.push(no_such_requirement(p, fi, pl.span, &shown, rname, &spec)),
            Some(req) => {
                let actual = sha256::short(req.block.as_bytes());
                match &pl.pin {
                    None => diags.push(
                        p.err(fi, "E102", pl.span, tr!("{sname} {n} が固定されていません（`sha256:` がありません）", "{sname} {n} is not pinned (it has no `sha256:`)"))
                            .note(tr!("いまの要件なら sha256:{actual} です（`yuen source pin` でも書けます）。", "For the requirement as it is, that is sha256:{actual} (`yuen source pin` writes it too)."))
                            .fix_trimmed(ritsu_base::sources::fixed_pin_line(&line_of(pl.span.line), &actual)),
                    ),
                    Some(pin) if *pin != actual => {
                        let mut d = p
                            .err(fi, "E103", pl.span, tr!(
                                "{sname} {n} が固定と違います（固定は sha256:{pin}、いまの要件は sha256:{actual}）",
                                "{sname} {n} does not match its pin (pinned sha256:{pin}, the requirement is sha256:{actual})"
                            ))
                            .note(tr!(
                                "固定したあとで、仕様の要件が変わりました（変更の archive か、仕様の書き直し）。何が変わったかを読んでから、固定を書き換えてください（`yuen source pin`）。",
                                "The requirement changed in the spec after it was pinned (an archived change, or an edit). Read what changed, then pin it again (`yuen source pin`)."
                            ));
                        if let Some(old) = crate::marks::reviewed_content(p, fi, pin) {
                            let (dl, more) = crate::diff::unified(&String::from_utf8_lossy(&old), &req.block);
                            d = d.diff(tr!("固定したときの要件からの差分（仕様は {shown}）", "what changed in the requirement since it was pinned (the spec {shown})"), dl);
                            if more > 0 {
                                d = d.note(tr!("差分はほかに {more} 行あります。", "{more} more lines of the diff are not shown."));
                            }
                        }
                        diags.push(d.fix_trimmed(ritsu_base::sources::fixed_pin_line(&line_of(pl.span.line), &actual)));
                    }
                    Some(_) => sp.block = Some(req.block.clone()),
                }
            }
        }
        out.push(sp);
    }
    Resolved::OpenSpec { name: name.clone(), abs, pins: out, spec: Some(spec) }
}

/// W102 (DESIGN 20.4): the requirements of a spec no source of the project pins, said once for a
/// spec, at the first source that names it.
fn unpinned(p: &Project, sources: &Sources) -> Vec<Diag> {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for (fi, f) in p.files.iter().enumerate() {
        for (s, (_, r)) in f.ast.sources.iter().zip(&sources.files[fi]) {
            let Resolved::OpenSpec { name, spec: Some(spec), .. } = r else { continue };
            if seen.contains(&name.path.as_str()) {
                continue;
            }
            seen.push(&name.path);
            let pinned = |rn: &str| {
                p.files.iter().enumerate().any(|(fj, g)| {
                    g.ast.sources.iter().zip(&sources.files[fj]).any(|(t, (_, x))| {
                        matches!(x, Resolved::OpenSpec { name: m, .. } if m.path == name.path) && matches!(&t.kind, SourceKind::OpenSpec { pins, .. } if pins.iter().any(|pl| pl.fragment == rn))
                    })
                })
            };
            let left: Vec<String> = spec.requirements.iter().filter(|q| !pinned(&q.name)).map(|q| crate::names::word_or_quote(&q.name)).collect();
            if left.is_empty() {
                continue;
            }
            let shown = p.shown(&name.path);
            let k = left.len();
            let (ja, en) = (left.join("、"), left.join(", "));
            let msg = if k == 1 {
                tr!("仕様 {shown} の要件のうち 1 件を、プロジェクトのどの出典も固定していません: {ja}", "A requirement of the spec {shown} is pinned by no source of the project: {en}")
            } else {
                tr!("仕様 {shown} の要件のうち {k} 件を、プロジェクトのどの出典も固定していません: {ja}", "{k} requirements of the spec {shown} are pinned by no source of the project: {en}")
            };
            out.push(
                p.warn(fi, "W102", s.span, msg)
                .note(tr!(
                    "要件ごとに、固定の行と、それを引く要件を書いてください。読まないと決めた要件も、引く要件を書いて `not satisfied` と `not verified` に理由を書けば、外したことが承認とともに残ります。",
                    "Pin each one and read it with a requirement. To leave one out, read it with a requirement all the same and write `not satisfied` and `not verified` with the reasons: leaving it out is then on record, with its approval."
                )),
            );
        }
    }
    out
}

/// An article a rule or a calendar pins (DESIGN 3.3): the source's name in that file, the law,
/// the article, its pin, and its copy beside that file.
#[derive(Clone, Debug)]
pub struct Pinned {
    pub source: String,
    pub db: LawDb,
    pub id: String,
    pub asof: String,
    pub fragment: String,
    pub pin: String,
    /// The copy, from the root.
    pub rel: String,
    pub abs: PathBuf,
}

/// The articles the file of an artifact pins, when it is a rule or a calendar whose language
/// answers for it (`Sources`); none otherwise.
pub fn pinned_by(p: &Project, n: &Name) -> Vec<Pinned> {
    use crate::names::Tool;
    if !matches!(n.tool, Tool::Rulec | Tool::Koyomi) {
        return vec![];
    }
    let abs = p.root.join(&n.path);
    let Some(Ok(declared)) = p.suite.sources(n.tool, &abs) else { return vec![] };
    let dir_rel = ritsu_base::paths::parent(&n.path);
    let mut out = Vec::new();
    for src in declared {
        let ritsu_ports::SourceKind::Law { db, id, asof, pins } = &src.kind else { continue };
        let db = if db == "ecfr" { LawDb::Ecfr } else { LawDb::Egov };
        let copy_dir = copies::copy_dir(id, asof);
        for (fragment, pin) in pins {
            let Some(file) = copies::fragment_file(db, fragment) else { continue };
            let Ok(rel) = ritsu_base::paths::join(&dir_rel, &format!("{copy_dir}/{file}")) else { continue };
            out.push(Pinned { source: src.name.clone(), db, id: id.clone(), asof: asof.clone(), fragment: fragment.clone(), pin: pin.clone(), abs: p.root.join(&rel), rel });
        }
    }
    out
}

/// E107 (DESIGN 3.3): a requirement cites an article, and a rule or a calendar that meets it pins
/// the same article (the same database, law and article) in copies none of which has the text of
/// the requirement's copy — what the requirement was read from and what the rule copied differ,
/// and one of the two is old. The texts are compared, not the bytes: e-Gov rewrites attributes
/// of an article that did not change.
pub fn mismatches(p: &Project, s: &Sources) -> Vec<Diag> {
    use crate::names::Tool;
    let mut diags = Vec::new();
    for r in 0..p.reqs.len() {
        let fi = p.reqs[r].file;
        let d = p.decl(r);
        let mut metby: Vec<Name> = Vec::new();
        for (li, l) in d.links.iter().enumerate() {
            if l.side != Side::Satisfied {
                continue;
            }
            if let Some(n) = &p.names.links[r][li]
                && matches!(n.tool, Tool::Rulec | Tool::Koyomi)
            {
                let file = Name { tool: n.tool, path: n.path.clone(), items: vec![] };
                if !metby.contains(&file) {
                    metby.push(file);
                }
            }
        }
        if metby.is_empty() {
            continue;
        }
        let pins: Vec<(Name, Vec<Pinned>)> = metby.iter().map(|n| (n.clone(), pinned_by(p, n))).collect();
        for f in &d.from {
            let FromWhat::Cite { source, fragments, .. } = &f.what else { continue };
            let Some(Resolved::Law { db, id, articles, .. }) = s.get(fi, source) else { continue };
            for (fr, sp) in fragments {
                let Some(a) = articles.iter().find(|a| &a.fragment == fr) else { continue };
                let Some(mine) = &a.bytes else { continue };
                let mine_text = copies::xml_text(&String::from_utf8_lossy(mine));
                for (n, ps) in &pins {
                    let theirs: Vec<&Pinned> = ps.iter().filter(|x| x.db == *db && x.id == *id && x.fragment == *fr).collect();
                    if theirs.is_empty() || theirs.iter().any(|x| x.abs == a.abs) {
                        continue;
                    }
                    let texts: Vec<(&Pinned, String)> = theirs.iter().filter_map(|x| ritsu_base::fs::read(&x.abs).ok().map(|b| (*x, copies::xml_text(&String::from_utf8_lossy(&b))))).collect();
                    if texts.iter().any(|(_, t)| *t == mine_text) {
                        continue;
                    }
                    let Some((first, other)) = texts.first() else { continue };
                    let me = p.req_label(r);
                    let t = n.text();
                    let (mine_shown, theirs_shown) = (p.shown(&a.rel), p.shown(&first.rel));
                    let (dl, more) = crate::diff::unified(&mine_text, other);
                    let mut dg = p
                        .err(fi, "E107", *sp, tr!("{me} が読んだ {source} {fr} と、{t} が固定している {fr} の本文が違います", "{me} reads {source} {fr}, and {t} pins {fr} with another text"))
                        .note(tr!("要件のコピーは {mine_shown}、{t} のコピーは {theirs_shown}（{} 時点）です。", "The requirement's copy is {mine_shown}; the copy of {t} is {theirs_shown} (as of {}).", first.asof; first.asof))
                        .diff(tr!("要件のコピーから {t} のコピーへの差分", "from the requirement's copy to the copy of {t}"), dl);
                    if more > 0 {
                        dg = dg.note(tr!("差分はほかに {more} 行あります。", "{more} more lines of the diff are not shown."));
                    }
                    dg = dg.note(tr!(
                        "どちらかのコピーが古いということです。改正を確かめ、古いほうを取り直して固定し直してください（yuen source fetch と yuen source pin、または規則やカレンダーの source fetch と source pin）。",
                        "One of the copies is old: check the amendments, and fetch and pin the older one again (yuen source fetch and yuen source pin, or the rule's or the calendar's source fetch and source pin)."
                    ));
                    diags.push(dg);
                }
            }
        }
    }
    diags
}
