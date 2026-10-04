//! The third stage (DESIGN 1.4, 5.1, PLAN B.4): every source's copies against their pins, and
//! every citation against the sources (E101–E105, W101). `check` never reads the network: a
//! copy that is not there is for `yuen source fetch` to bring.

use crate::ast::*;
use crate::copies;
use crate::diag::{Diag, DiagExt};
use ritsu_base::text::Text;
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
    /// A law, copied an article at a time. `borrowed` is the source of a rule or a calendar it is
    /// borrowed from (DESIGN 1.4, 3.3): then the copies and the pins are that file's, and its
    /// language's check holds them.
    Law { db: LawDb, id: String, asof: String, dir: PathBuf, revision: Option<String>, articles: Vec<Article>, borrowed: Option<Name> },
    File { name: Name, abs: PathBuf, url: Option<String>, pin: Option<String>, bytes: Option<Vec<u8>>, borrowed: Option<Name> },
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
    if !abs.is_file() {
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
                let bytes = std::fs::read(&copy).ok().filter(|b| sha256::short(b) == *pin);
                if bytes.is_none() {
                    let (shown, fr) = (p.shown(&rel), fragment);
                    diags.push(e106(tr!("{t} の {fr} の写し {shown} を読めないか、固定と違います", "the copy {shown} of {fr} of {t} cannot be read, or does not match its pin")));
                }
                articles.push(Article { fragment: fragment.clone(), rel, abs: copy, pin: Some(pin.clone()), bytes, span: s.span });
            }
            Resolved::Law { db, id: id.clone(), asof: asof.clone(), revision: copies::revision(&dir), dir, articles, borrowed: Some(n.clone()) }
        }
        ritsu_ports::SourceKind::File { path, url, pin } => {
            let Ok(rel) = ritsu_base::paths::join(&dir_rel, path) else {
                diags.push(e106(tr!("{t} の写しのパス `{path}` はルートの外に出ます", "the path `{path}` of the copy of {t} goes outside the root")));
                return Resolved::Broken;
            };
            let copy = p.root.join(&rel);
            let bytes = std::fs::read(&copy).ok().filter(|b| pin.as_deref() == Some(sha256::short(b).as_str()));
            if bytes.is_none() {
                let shown = p.shown(&rel);
                diags.push(e106(tr!("{t} の写し {shown} を読めないか、固定と違います", "the copy {shown} of {t} cannot be read, or does not match its pin")));
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
                    Resolved::Law { db: *db, id: id.clone(), asof, revision: copies::revision(&dir), dir, articles, borrowed: None }
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
                        Resolved::File { name: name.clone(), abs, url: url.clone(), pin: pin.clone(), bytes, borrowed: None }
                    }
                },
                SourceKind::Borrowed { .. } => match &p.names.sources[fi][si] {
                    Some(n) => borrow(p, fi, s, n, &mut diags),
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
                    SourceKind::Borrowed { .. } => match sources.get(fi, source) {
                        Some(Resolved::Law { articles, borrowed: Some(n), .. }) => {
                            if fragments.is_empty() {
                                diags.push(p.err(fi, "E105", *source_span, tr!(
                                    "法令は条の単位で写すので、`@{source} 第140条` のように、どこを引いたかを書きます",
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
                                        tr!("固定している条: {}。ほかの条を引くなら、そのファイルに固定の行を足します。", "The articles it pins: {}. To cite another, add its pin to that file.", pinned.join("、"); pinned.join(", "))
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
                    let texts: Vec<(&Pinned, String)> = theirs.iter().filter_map(|x| std::fs::read(&x.abs).ok().map(|b| (*x, copies::xml_text(&String::from_utf8_lossy(&b))))).collect();
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
                        .note(tr!("要件の写しは {mine_shown}、{t} の写しは {theirs_shown}（{} 時点）です。", "The requirement's copy is {mine_shown}; the copy of {t} is {theirs_shown} (as of {}).", first.asof; first.asof))
                        .diff(tr!("要件の写しから {t} の写しへの差分", "from the requirement's copy to the copy of {t}"), dl);
                    if more > 0 {
                        dg = dg.note(tr!("差分はほかに {more} 行あります。", "{more} more lines of the diff are not shown."));
                    }
                    dg = dg.note(tr!(
                        "どちらかの写しが古いということです。改正を確かめ、古いほうを取り直して固定し直します（yuen source fetch と yuen source pin、または規則やカレンダーの source fetch と source pin）。",
                        "One of the copies is old: check the amendments, and fetch and pin the older one again (yuen source fetch and yuen source pin, or the rule's or the calendar's source fetch and source pin)."
                    ));
                    diags.push(dg);
                }
            }
        }
    }
    diags
}
