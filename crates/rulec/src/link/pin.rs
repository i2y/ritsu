//! The pins of the merging of files (§15.230).
//!
//! - **`signature:`** on a contribution's `to` and `after` lines: the shape of the file it relies
//!   on — the rule's name, its inputs and outputs, its open values, the outputs it opens, its open
//!   enums and blocks, and the declarations of the names this contribution reads. A comment, the
//!   `description`, a row of a table or a definition the contribution does not read are not in it:
//!   the rows of the rule are the rule's own to approve, and moving them does not ask every
//!   contribution to be read again. What the contribution relies on moving does (E040, which says
//!   what moved).
//! - **`meaning:`** on an `apply` line: the callee's lines with its comments, its `description` and
//!   its citations left out. An apply relies on every row of the callee, so a row moves it; a
//!   comment does not.

use super::{Contrib, range_text, type_text};
use crate::ast::*;
use crate::diag::Diag;
use crate::lex::Kind;

/// A token as the meaning reads it: a name, a number or a string as written, and a mark in its one
/// spelling, so that `→` and `->`, `、` and `,` are one.
fn token_text(k: &Kind) -> String {
    match k {
        Kind::Ident(s) => s.clone(),
        Kind::Num(n) => n.raw.clone(),
        Kind::Str(s) => format!("{s:?}"),
        Kind::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02}"),
        Kind::Hash(h) => format!("sha256:{h}"),
        Kind::Signature(h) => format!("signature:{h}"),
        Kind::Meaning(h) => format!("meaning:{h}"),
        Kind::Pipe => "|".into(),
        Kind::Arrow => "->".into(),
        Kind::Colon => ":".into(),
        Kind::Eq => "=".into(),
        Kind::LParen => "(".into(),
        Kind::RParen => ")".into(),
        Kind::LBracket => "[".into(),
        Kind::RBracket => "]".into(),
        Kind::Comma => ",".into(),
        Kind::Le => "<=".into(),
        Kind::Ge => ">=".into(),
        Kind::Lt => "<".into(),
        Kind::Gt => ">".into(),
        Kind::Plus => "+".into(),
        Kind::Minus => "-".into(),
        Kind::Star => "*".into(),
        Kind::Slash => "/".into(),
        Kind::Question => "?".into(),
        Kind::DotDot => "..".into(),
        Kind::Dot => ".".into(),
        Kind::At => "@".into(),
    }
}

/// The lines of a rule as its meaning reads them (§15.230): each line's tokens with one space
/// between them, the lines that are only a comment and the `description` line left out, and a
/// citation (`@…` to the end of the line) taken off. Two files that `rulec fmt` makes the same, or
/// that differ only in comments, have the same lines.
pub fn meaning_lines(src: &str) -> Vec<String> {
    meaning_pairs(src).into_iter().map(|(n, _)| n).collect()
}

/// [`meaning_lines`], each beside the line as it is written — its tokens as they are spelled, one
/// space where the file has any, the comment and the citation left out — which is what E040 shows
/// of a line that moved.
pub fn meaning_pairs(src: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (n, text) in src.lines().enumerate() {
        let Ok((ts, _)) = crate::lex::lex_line_soft(n + 1, text) else {
            // A line that does not lex is kept as written: it is still part of what the file says.
            out.push((text.trim().to_string(), text.trim().to_string()));
            continue;
        };
        if ts.is_empty() || ts.first().and_then(|t| t.ident()) == Some(crate::kw::DESCRIPTION) {
            continue;
        }
        let kept: Vec<&crate::lex::Token> = ts.iter().take_while(|t| !t.is(&Kind::At)).collect();
        if kept.is_empty() {
            continue;
        }
        let words: Vec<String> = kept.iter().map(|t| token_text(&t.kind)).collect();
        let mut shown = String::new();
        let mut end: Option<usize> = None;
        for t in &kept {
            if end.is_some_and(|e| e < t.span.col) {
                shown.push(' ');
            }
            match text.get(t.span.col..t.span.col + t.span.len) {
                Some(w) => shown.push_str(w),
                None => shown.push_str(&token_text(&t.kind)),
            }
            end = Some(t.span.col + t.span.len);
        }
        out.push((words.join(" "), shown));
    }
    out
}

/// `meaning:<16 hex>` of a rule's text.
pub fn meaning(src: &str) -> String {
    crate::sha256::short(meaning_lines(src).join("\n").as_bytes())
}

/// The lines of the file at `path` as its meaning reads them, beside the lines as written, at the
/// revision of the repository whose meaning is `pinned`: what an `apply` pinned, for E040 to say
/// what moved since.
pub fn meaning_at(path: &str, pinned: &str) -> Option<Vec<(String, String)>> {
    history(path).into_iter().find(|src| meaning(src) == pinned).map(|src| meaning_pairs(&src))
}

/// Every name a contribution writes that may be another file's: what its items read and write, the
/// tables and clauses it takes precedence over, its `into` lines and terms, its open values' terms.
fn referenced(c: &RuleFile) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for it in &c.items {
        out.extend(crate::types::item_reads(it));
        match it {
            Item::Table(t) => {
                out.extend(t.outputs.iter().map(|o| o.name.text.clone()));
                out.extend(t.overrides.iter().map(|o| o.table.clone()));
            }
            Item::Agg(d) => out.push(d.over.clone()),
            Item::Fold(d) => out.push(d.over.clone()),
            _ => {}
        }
    }
    if let Some(h) = &c.contrib {
        for d in &h.intos {
            out.push(d.target.text.clone());
            out.extend(d.terms.iter().map(|t| t.text.clone()));
        }
    }
    for o in &c.opens {
        out.extend(o.terms.iter().map(|t| t.name.text.clone()));
    }
    out.sort();
    out.dedup();
    out
}

/// How a name the reader relies on is declared in the target, as one line, or `None` when the target
/// does not declare it (or declares it in a line the shape has already).
fn declaration(t: &RuleFile, n: &str) -> Option<String> {
    for it in &t.items {
        match it {
            Item::Derived(d) if d.name.text == n => {
                return Some(format!("derive {n} : {} range {}", type_text(&d.ty), d.range.as_ref().map(range_text).unwrap_or_default()));
            }
            Item::Define(d) if d.name.text == n => return Some(format!("define {n} : {}", type_text(&d.ty))),
            Item::Agg(d) if d.name.text == n => {
                return Some(format!("{} {n} over {} range {}", d.kind.word(), d.over, d.range.as_ref().map(range_text).unwrap_or_default()));
            }
            Item::Fold(d) if d.answer.as_ref().is_some_and(|a| a.name.text == n) => {
                let ty = d.answer.as_ref().and_then(|a| a.ty.as_ref()).map(type_text).unwrap_or_default();
                return Some(format!("fold {} over {} -> {n} : {ty}", d.verdict, d.over));
            }
            Item::Table(tb) if tb.name.as_ref().is_some_and(|x| x.text == n) => {
                return Some(format!("{} {n}", if tb.clause { crate::kw::CLAUSE } else { crate::kw::TABLE }));
            }
            Item::Table(tb) => {
                if let Some(o) = tb.outputs.iter().find(|o| o.name.text == n) {
                    if t.outputs.iter().any(|x| x.name.text == n) {
                        return None;
                    }
                    return Some(format!("column {n} : {}", o.ty.as_ref().map(type_text).unwrap_or_default()));
                }
            }
            _ => {}
        }
    }
    for el in &t.elements {
        if el.name.text == n {
            return Some(format!("elements {n}"));
        }
        if let Some(fd) = el.fields.iter().find(|x| x.name.text == n) {
            return Some(format!("field {n} : {} range {}", type_text(&fd.ty), fd.range.as_ref().map(range_text).unwrap_or_default()));
        }
    }
    None
}

/// The shape of `t` that the contribution `reader` relies on, one line each (§15.230).
pub fn shape_lines(t: &RuleFile, reader: &RuleFile) -> Vec<String> {
    let mut out = Vec::new();
    let kind = match t.kind {
        FileKind::Rule => crate::kw::RULE,
        FileKind::Enums => crate::kw::ENUMS,
        FileKind::Contribution => crate::kw::CONTRIBUTION,
    };
    out.push(format!("{kind} {}", t.name.text));
    for i in &t.inputs {
        out.push(format!("input {} : {} range {}", i.name.text, type_text(&i.ty), i.range.as_ref().map(range_text).unwrap_or_default()));
    }
    for o in &t.outputs {
        let r = o.rounding.as_ref().map(|r| format!(" round {}({})", r.mode, r.grid.raw)).unwrap_or_default();
        out.push(format!("output {} : {}{r}", o.name.text, type_text(&o.ty)));
    }
    for o in &t.opens {
        out.push(super::open_text(o));
    }
    for it in &t.items {
        if let Item::Table(tb) = it {
            if tb.open.is_some() {
                for o in &tb.outputs {
                    out.push(format!("opened {}", o.name.text));
                }
            }
        }
    }
    for e in &t.enums {
        if e.open.is_some() && e.from.is_none() {
            let vs: Vec<&str> = e.values.iter().map(|v| v.text.as_str()).collect();
            if e.ordered {
                out.push(format!("enum {} open = {}", e.name.text, vs.join(" < ")));
            } else {
                out.push(format!("enum {} open", e.name.text));
            }
        }
    }
    for p in &t.prerequisites {
        if p.open.is_some() {
            out.push(format!("prerequisites {} over {} open", p.name.text, p.over_name()));
        }
    }
    if let Some(h) = &t.contrib {
        for e in &h.extends {
            out.push(format!("extend {} with {}", e.enum_name.text, e.value.text));
        }
    }
    let mut reads: Vec<String> = referenced(reader).iter().filter_map(|n| declaration(t, n)).collect();
    reads.sort();
    reads.dedup();
    out.extend(reads.into_iter().map(|l| format!("reads {l}")));
    out
}

/// `signature:<16 hex>` of `t` as `reader` relies on it.
pub fn signature(t: &RuleFile, reader: &RuleFile) -> String {
    crate::sha256::short(shape_lines(t, reader).join("\n").as_bytes())
}

/// The file at `path`, parsed, as the pins read it.
fn parsed(path: &str) -> Option<RuleFile> {
    let src = crate::vfs::read_to_string(std::path::Path::new(path)).ok()?;
    crate::parse::parse(&src, path).file
}

/// The revisions of the file at `path` in git, newest first, with its text at each: where the shape
/// a pin was taken from is looked for, to say what has moved since.
fn history(path: &str) -> Vec<String> {
    let Ok(out) = std::process::Command::new("git").args(["log", "--format=%H", "-n", "60", "--", path]).output() else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    let mut texts = Vec::new();
    for rev in String::from_utf8_lossy(&out.stdout).lines() {
        let spec = if std::path::Path::new(path).is_absolute() { format!("{rev}:{path}") } else { format!("{rev}:./{path}") };
        if let Ok(o) = std::process::Command::new("git").args(["show", &spec]).output() {
            if o.status.success() {
                texts.push(String::from_utf8_lossy(&o.stdout).into_owned());
            }
        }
    }
    texts
}

/// What moved between two lists of lines, as notes: a line both have under one head (its first
/// two words) and differs in, a line only the old has, and one only the new has.
pub fn moved(old: &[String], new: &[String]) -> Vec<String> {
    let pair = |ls: &[String]| ls.iter().map(|l| (l.clone(), l.clone())).collect::<Vec<_>>();
    moved_pairs(&pair(old), &pair(new))
}

/// [`moved`] over the lines of [`meaning_pairs`]: compared as the meaning reads them, shown as
/// they are written.
pub fn moved_pairs(old: &[(String, String)], new: &[(String, String)]) -> Vec<String> {
    let head = |l: &str| l.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
    let has = |ls: &[(String, String)], l: &str| ls.iter().any(|(k, _)| k == l);
    let mut out = Vec::new();
    for (ok, os) in old {
        if has(new, ok) {
            continue;
        }
        match new.iter().find(|(nk, _)| !has(old, nk) && head(nk) == head(ok)) {
            Some((_, ns)) => out.push(tr!("`{os}` が `{ns}` になりました", "`{os}` is now `{ns}`")),
            None => out.push(tr!("`{os}` が無くなりました", "`{os}` is gone")),
        }
    }
    for (nk, ns) in new {
        if has(old, nk) || old.iter().any(|(ok, _)| !has(new, ok) && head(ok) == head(nk)) {
            continue;
        }
        out.push(tr!("`{ns}` が増えました", "`{ns}` is new"));
    }
    out
}

/// The line `rulec source pin` writes for a `to` or `after` line with `sig`: the line as written,
/// the digest replaced or added.
pub fn target_line(word: &str, t: &Target, sig: &str) -> String {
    format!("{word} \"{}\" signature:{sig}", t.file)
}

/// The pins of one contribution (§15.230): its `to` and each `after`, each held to the shape of the
/// file it names as this contribution relies on it. E040 says a pin that is missing or no longer
/// matches, and what moved since.
pub(crate) fn check_pins(c: &Contrib, _cs: &[Contrib], _root: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    let Some(h) = &c.file.contrib else { return out };
    let mut lines: Vec<(&str, &Target)> = Vec::new();
    if let Some(t) = &h.to {
        lines.push((crate::kw::TO, t));
    }
    for a in &h.afters {
        lines.push((crate::kw::AFTER, a));
    }
    for (word, t) in lines {
        let p = super::join(&c.shown, &t.file);
        let Some(target) = parsed(&p) else {
            out.push(
                Diag::error("E067", tr!("`{}` を読めません", "`{}` cannot be read", t.file))
                    .at(format!("{}:{}", c.shown, t.span.line))
                    .mark(t.span.clone(), "")
                    .note(tr!("パスは寄与のファイルのある場所からたどります（探した先: {p}）。", "The path is followed from the directory of the contribution (looked for: {p}).")),
            );
            continue;
        };
        if word == crate::kw::AFTER && target.kind != FileKind::Contribution {
            out.push(
                Diag::error("E067", tr!("`after` の先 `{}` は寄与のファイルではありません", "`after` names `{}`, which is not a contribution", t.file))
                    .at(format!("{}:{}", c.shown, t.span.line))
                    .mark(t.span.clone(), "")
                    .note(tr!(
                        "`after` は、この寄与より先に合わせる寄与を言います。足す先の規則は `to` で言ってください。",
                        "`after` names a contribution merged before this one; the rule it adds to is named with `to`."
                    )),
            );
            continue;
        }
        let now = signature(&target, &c.file);
        let at = format!("{}:{}", c.shown, t.span.line);
        let fixed = target_line(word, t, &now);
        match &t.signature {
            None => out.push(
                Diag::error("E040", tr!("`{}` の形が固定されていません", "The shape of `{}` is not pinned", t.file))
                    .at(at)
                    .mark(t.span.clone(), "")
                    .note(tr!(
                        "いまの形は signature:{now} です。この形を確かめたなら、行を次のとおりにしてください（`rulec source pin` でも書けます）。",
                        "The shape is signature:{now} now. Once this contribution is checked against it, make the line the following (`rulec source pin` writes it too)."
                    ))
                    .fix(crate::diag::FixKind::PinSource, fixed),
            ),
            Some(pinned) if *pinned != now => {
                let mut d = Diag::error("E040", tr!("この寄与が頼る `{}` の形が変わっています", "The shape of `{}` this contribution relies on has changed", t.file))
                    .at(at)
                    .mark(t.span.clone(), tr!("固定: signature:{pinned}", "pinned: signature:{pinned}"))
                    .note(tr!("いまの形: signature:{now}", "The shape now: signature:{now}"));
                let then = history(&p).into_iter().find_map(|src| {
                    let f = crate::parse::parse(&src, &p).file?;
                    (signature(&f, &c.file) == *pinned).then(|| shape_lines(&f, &c.file))
                });
                match then {
                    Some(old) => {
                        for m in moved(&old, &shape_lines(&target, &c.file)) {
                            d = d.note(m);
                        }
                    }
                    None => {
                        d = d.note(tr!(
                            "固定したときの形は、このリポジトリの履歴に見つかりませんでした。形に入るのは、名前、入力と出力、開いた値と開いた出力、開いた列挙と塊、この寄与が読む名前の宣言です。",
                            "The shape it was pinned at is not in this repository's history. What the shape holds: the name, the inputs and outputs, the open values and outputs, the open enums and blocks, and the declarations of the names this contribution reads."
                        ));
                    }
                }
                out.push(
                    d.note(tr!(
                        "この寄与を新しい形で読み直し、それでよければ、行を `fix.text` のとおりにしてください（`rulec source pin` でも書けます）。",
                        "Read this contribution against the new shape; once it holds, make the line as `fix.text` says (`rulec source pin` writes it too)."
                    ))
                    .fix(crate::diag::FixKind::PinSource, fixed),
                );
            }
            _ => {}
        }
    }
    out
}

/// `rulec source pin` of a contribution: its `to` and `after` lines with the signatures of what they
/// name now. Returns the text and what it says.
pub fn pin_contribution(f: &RuleFile, path: &str, src: &str) -> Result<(String, Vec<String>), String> {
    let mut lines: Vec<String> = src.lines().map(str::to_string).collect();
    let mut said = Vec::new();
    let Some(h) = &f.contrib else { return Ok((src.to_string(), said)) };
    let mut targets: Vec<(&str, &Target)> = Vec::new();
    if let Some(t) = &h.to {
        targets.push((crate::kw::TO, t));
    }
    for a in &h.afters {
        targets.push((crate::kw::AFTER, a));
    }
    for (word, t) in targets {
        let p = super::join(path, &t.file);
        let target = parsed(&p).ok_or_else(|| tr!("`{}` を読めません", "cannot read `{}`", t.file))?;
        let sig = signature(&target, f);
        if t.signature.as_deref() == Some(sig.as_str()) {
            said.push(tr!("{word} {}: signature:{sig} に固定済みです", "{word} {}: already pinned at signature:{sig}", t.file));
            continue;
        }
        if let Some(l) = lines.get_mut(t.span.line - 1) {
            let comment = l.find('#').map(|k| l[k..].to_string());
            let mut new = target_line(word, t, &sig);
            if let Some(c) = comment {
                new.push_str("  ");
                new.push_str(&c);
            }
            *l = new;
        }
        said.push(tr!("{word} {}: signature:{sig} に固定しました", "{word} {}: pinned at signature:{sig}", t.file));
    }
    let mut text = lines.join("\n");
    text.push('\n');
    Ok((text, said))
}
