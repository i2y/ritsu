//! `yuen doc` (DESIGN 10, PLAN D.1): the page of a project, for whoever has to understand and
//! check what the code is meant to do — the people who run the business, accounting or legal,
//! operations, the developers who read the code — and for whoever audits where it came from.
//!
//! The page is built once, as blocks (headings, paragraphs, tables, quotes of the law, the text
//! of a diagnostic), and `markdown.rs` and `html.rs` only write the blocks out, so the two forms
//! cannot say different things (koyomi's `src/doc/` works the same way). What the page says is
//! the `.req` files, the result of the check and what the copies of the sources say: the graph is
//! read from what `yuen api` prints (DESIGN 11), the articles from the pinned copies. What an
//! artifact holds (a rule's tables, a calendar's dates) is the page of its own language; this
//! page names the artifact and its file, and goes no further (DESIGN 10, what was set aside).

use crate::ast::FromWhat;
use crate::check::Checked;
use crate::sources::Resolved;
use ritsu_base::diag::Severity;
use ritsu_base::text::{Lang, Text};
use serde_json::Value;
use std::collections::BTreeSet;

pub mod html;
pub mod markdown;

/// A run of text: words, code (a name, a naming, a hash), or words in bold.
#[derive(Clone, Debug, PartialEq)]
pub enum Inline {
    T(String),
    C(String),
    B(String),
}

pub type Para = Vec<Inline>;

/// The text of an article, quoted from its pinned copy a paragraph to a line.
#[derive(Clone, Debug)]
pub struct Quote {
    pub head: String,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum Block {
    /// A section's heading and its anchor.
    H2(String, String),
    /// A heading inside a section and its anchor.
    H3(String, String),
    P(Para),
    List(Vec<Para>),
    /// The heads of the columns, and the rows.
    Table(Vec<String>, Vec<Vec<Para>>),
    Quote(Quote),
    /// A diagnostic as `yuen check` prints it (with its diff).
    Pre(String),
}

pub struct Page {
    pub title: String,
    pub blocks: Vec<Block>,
}

fn t(s: impl Into<String>) -> Inline {
    Inline::T(s.into())
}
fn c(s: impl Into<String>) -> Inline {
    Inline::C(s.into())
}
fn b(s: impl Into<String>) -> Inline {
    Inline::B(s.into())
}
fn say(x: Text, lang: Lang) -> String {
    x.get(lang).to_string()
}
fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

/// Whether the page cannot be made: errors in the words, names, sources or artifacts (stages 1
/// to 4). A page made over copies or artifacts that cannot be read would mislead whoever reads
/// it (DESIGN 10); the check's diagnostics go to standard error instead.
pub fn blocked(c: &Checked) -> bool {
    c.model.is_none()
        || c.diags.iter().any(|d| {
            d.severity == Severity::Error && (d.code.starts_with("E0") || d.code.starts_with("E1") || d.code.starts_with("E2") || d.code == "E403")
        })
}

/// What a link's or a waiver's state says to a person.
fn state_words(st: &str, lang: Lang) -> String {
    say(
        match st {
            "ok" => tr!("確かめたまま", "as looked at"),
            "unreviewed" => tr!("まだ確かめていない", "not looked at yet"),
            "up_changed" => tr!("確かめたあとで、リンク元が変わった", "what it comes from changed after it was looked at"),
            "down_changed" => tr!("確かめたあとで、成果物が変わった", "the artifact changed after it was looked at"),
            "unapproved" => tr!("承認されていない", "not approved"),
            "bad_record" => tr!("記録の形が崩れている", "its record is broken"),
            "unreadable" => tr!("リンクの両端を読めない", "its ends cannot be read"),
            _ => Text::default(),
        },
        lang,
    )
}

/// A requirement as its heading and the table write it: its name, its version when it has
/// several, its alias.
fn req_label(r: &Value, many: bool) -> String {
    let mut out = s(&r["name"]);
    if many {
        out.push_str(&format!(" v{}", r["version"].as_u64().unwrap_or(1)));
    }
    if let Some(a) = r["alias"].as_str() {
        out.push_str(&format!(" ({a})"));
    }
    out
}

fn anchor(prefix: &str, r: &Value, many: bool) -> String {
    let base = r["alias"].as_str().map(str::to_string).unwrap_or_else(|| s(&r["name"]));
    let mut a: String = base.chars().map(|ch| if ch.is_alphanumeric() || ch == '_' || ch == '-' { ch } else { '-' }).collect();
    if many {
        a.push_str(&format!("-v{}", r["version"].as_u64().unwrap_or(1)));
    }
    format!("{prefix}-{a}")
}

fn period(v: &Value) -> String {
    if v.is_null() {
        return "—".into();
    }
    format!("{}..{}", v["from"].as_str().unwrap_or(""), v["to"].as_str().unwrap_or(""))
}

/// `@民法 第141条, 第143条`, `@holidays`, or the requirement it is read from.
fn from_words(f: &Value) -> String {
    if !f["requirement"].is_null() {
        let r = &f["requirement"];
        return format!("{} v{}", s(&r["name"]), r["version"].as_u64().unwrap_or(1));
    }
    let srcs = f["sources"].as_array().cloned().unwrap_or_default();
    let name = srcs.first().map(|x| s(&x["source"])).unwrap_or_default();
    let frags: Vec<String> = srcs.iter().filter_map(|x| x["fragment"].as_str().map(str::to_string)).collect();
    if frags.is_empty() { format!("@{name}") } else { format!("@{name} {}", frags.join(", ")) }
}

fn short(h: &str) -> String {
    format!("sha256:{h}")
}

/// Who looked, when, and the hashes, or that no one has.
fn reviewed_words(r: &Value, lang: Lang) -> Para {
    if r.is_null() {
        return vec![t(say(tr!("確かめた記録は無い", "no record of anyone looking"), lang))];
    }
    let (by, date) = (s(&r["by"]), s(&r["date"]));
    let up: Vec<String> = r["up"].as_array().map(|a| a.iter().map(|h| short(&s(h))).collect()).unwrap_or_default();
    let mut p = vec![t(say(tr!("{by} が {date} に確かめた（", "looked at by {by} on {date} ("), lang)), c(up.join(", "))];
    if let Some(d) = r["down"].as_str() {
        p.push(t(" → "));
        p.push(c(short(d)));
    }
    p.push(t(say(tr!("）", ")"), lang)));
    p
}

/// The page of a checked project (stages 1 to 4 without errors: see [`blocked`]).
pub fn page(ch: &Checked, label: &str, lang: Lang) -> Page {
    let p = ch.project.as_ref().unwrap();
    let m = ch.model.as_ref().unwrap();
    let api = crate::api::api(ch, label, lang).unwrap_or(Value::Null);
    let reqs = api["requirements"].as_array().cloned().unwrap_or_default();
    let names: Vec<String> = api["files"].as_array().map(|a| a.iter().map(|f| s(&f["name"])).collect()).unwrap_or_default();
    let many_versions = |name: &str| reqs.iter().filter(|r| r["name"].as_str() == Some(name)).count() > 1;
    let name_list = names.join(", ");
    let title = say(tr!("{name_list} — 要件の出どころ", "{name_list} — where the requirements come from"), lang);
    let mut out: Vec<Block> = Vec::new();

    // ── 1. The head: the files, their hashes, yuen's version, the check ──
    let version = crate::api::VERSION;
    let mut files: Vec<Para> = Vec::new();
    for f in api["files"].as_array().cloned().unwrap_or_default() {
        let (path, name, v, h) = (s(&f["path"]), s(&f["name"]), s(&f["version"]), s(&f["sha256"]));
        files.push(vec![c(path), t(say(tr!("（{name} v{v}、", " ({name} v{v}, "), lang)), c(short(&h)), t(say(tr!("）", ")"), lang))]);
    }
    out.push(Block::P(vec![t(say(
        tr!(
            "このページは、下の .req のファイルと、出典の写しと、ほかの言語から読んだ成果物の定義をもとに、yuen {version} が作った。",
            "yuen {version} made this page from the .req files below, the copies of their sources and what the other languages say of the artifacts."
        ),
        lang,
    ))]));
    out.push(Block::List(files));
    let summary = crate::check::summary(ch, label);
    if ch.diags.is_empty() {
        out.push(Block::P(vec![t(say(tr!("検査の結果：", "The check: "), lang)), c(summary.get(lang).trim_end())]));
    } else {
        out.push(Block::P(vec![t(say(tr!("検査の結果：", "The check: "), lang)), c(summary.get(lang).trim_end()), t(say(tr!("。止めている診断：", ". What stops it:"), lang))]));
        out.push(Block::List(ch.diags.iter().map(|d| vec![c(d.render(lang).lines().next().unwrap_or("").to_string())]).collect()));
    }

    // ── 2. The traceability table ──
    out.push(Block::H2(say(tr!("トレーサビリティ", "Traceability"), lang), "traceability".into()));
    let heads = [
        tr!("要件", "Requirement"),
        tr!("期間", "In force"),
        tr!("出どころ", "Comes from"),
        tr!("持ち主", "Owner"),
        tr!("満たすもの", "Met by"),
        tr!("確かめるもの", "Checked by"),
        tr!("状態", "State"),
    ]
    .into_iter()
    .map(|x| say(x, lang))
    .collect();
    let mut rows = Vec::new();
    for r in &reqs {
        let many = many_versions(&s(&r["name"]));
        let mut from: Para = Vec::new();
        for f in r["from"].as_array().cloned().unwrap_or_default() {
            if !from.is_empty() {
                from.push(t(say(tr!("、", "; "), lang)));
            }
            from.push(c(from_words(&f)));
        }
        for d in r["decided"].as_array().cloned().unwrap_or_default() {
            if !from.is_empty() {
                from.push(t(say(tr!("、", "; "), lang)));
            }
            let (by, date) = (s(&d["by"]), s(&d["date"]));
            from.push(t(say(tr!("{date} に {by} が決めた", "decided by {by} on {date}"), lang)));
        }
        let mut met: Para = Vec::new();
        let mut checked: Para = Vec::new();
        for l in r["links"].as_array().cloned().unwrap_or_default() {
            let side = if s(&l["role"]) == "satisfied" { &mut met } else { &mut checked };
            if !side.is_empty() {
                side.push(t(say(tr!("、", "; "), lang)));
            }
            side.push(c(s(&l["artifact"]["text"])));
        }
        let mut waived = 0;
        for w in r["waivers"].as_array().cloned().unwrap_or_default() {
            waived += 1;
            let side = if s(&w["role"]) == "satisfied" { &mut met } else { &mut checked };
            if !side.is_empty() {
                side.push(t(say(tr!("、", "; "), lang)));
            }
            side.push(t(say(tr!("見送り", "waived"), lang)));
        }
        let mut marked = 0;
        for k in ["from", "links", "waivers"] {
            for x in r[k].as_array().cloned().unwrap_or_default() {
                if x["status"].as_str().is_some_and(|st| st != "ok") {
                    marked += 1;
                }
            }
        }
        let state = if marked > 0 {
            say(tr!("印あり（{marked} 本）", "marked ({marked})"), lang)
        } else if waived > 0 {
            say(tr!("確かめたまま（見送り {waived} 件）", "as looked at ({waived} waived)"), lang)
        } else {
            say(tr!("確かめたまま", "as looked at"), lang)
        };
        rows.push(vec![
            vec![c(req_label(r, many))],
            vec![t(period(&r["in_force"]))],
            from,
            vec![t(s(&r["owner"]))],
            met,
            checked,
            vec![if marked > 0 { b(state) } else { t(state) }],
        ]);
    }
    out.push(Block::Table(heads, rows));

    // ── 3. The sources ──
    out.push(Block::H2(say(tr!("出典", "Sources"), lang), "sources".into()));
    let srcs = api["sources"].as_array().cloned().unwrap_or_default();
    if srcs.is_empty() {
        out.push(Block::P(vec![t(say(tr!("出典の宣言は無い。どの要件も、決めたことか、ほかの要件から来ている。", "No source is declared: every requirement comes from a decision or from another requirement."), lang))]));
    }
    let several_files = api["files"].as_array().map(|a| a.len() > 1).unwrap_or(false);
    for src in &srcs {
        let name = s(&src["name"]);
        let head = if several_files { format!("{} — {name}", s(&src["file"])) } else { name.clone() };
        out.push(Block::H3(head, format!("source-{}", name.chars().map(|ch| if ch.is_alphanumeric() { ch } else { '-' }).collect::<String>())));
        let mut p: Para = Vec::new();
        match src["kind"].as_str() {
            Some("law") => {
                let (db, id, asof) = (s(&src["db"]), s(&src["id"]), s(&src["asof"]));
                let dbw = if db == "ecfr" { "eCFR" } else { "e-Gov" };
                p.push(t(say(tr!("{dbw} の法令 ", "The law "), lang)));
                p.push(c(id.clone()));
                p.push(t(say(tr!("、{asof} 時点", " in {dbw}, as of {asof}"), lang)));
                if let Some(rev) = src["revision"].as_str() {
                    p.push(t(say(tr!("、版 ", ", version "), lang)));
                    p.push(c(rev.to_string()));
                    p.push(t(say(tr!("（写しの隣の revision.txt）", " (revision.txt beside the copies)"), lang)));
                }
                p.push(t(say(tr!("。", "."), lang)));
            }
            Some("file") => {
                p.push(t(say(tr!("ファイル ", "The file "), lang)));
                p.push(c(s(&src["path"])));
                if let Some(u) = src["url"].as_str() {
                    p.push(t(say(tr!("（元は ", " (from "), lang)));
                    p.push(c(u.to_string()));
                    p.push(t(say(tr!("）", ")"), lang)));
                }
                if let Some(h) = src["sha256"].as_str() {
                    p.push(t(say(tr!("、固定は ", ", pinned at "), lang)));
                    p.push(c(short(h)));
                }
                p.push(t(say(tr!("。", "."), lang)));
            }
            _ => {}
        }
        if !src["borrowed"].is_null() {
            p.push(t(say(tr!("借りた先：", " Borrowed from "), lang)));
            p.push(c(s(&src["borrowed"]["text"])));
            p.push(t(say(tr!("（写しと固定はそのファイルのもので、その言語の検査が確かめる）。", " (the copies and the pins are that file's, and its language's check holds them)."), lang)));
        }
        out.push(Block::P(p));
        let file = s(&src["file"]);
        let citing = |frag: Option<&str>| -> Para {
            let mut by: Para = Vec::new();
            for r in &reqs {
                if s(&r["file"]) != file {
                    continue;
                }
                let cites = r["from"].as_array().cloned().unwrap_or_default().iter().any(|f| {
                    f["sources"].as_array().cloned().unwrap_or_default().iter().any(|x| s(&x["source"]) == name && (frag.is_none() || x["fragment"].as_str() == frag))
                });
                if cites {
                    if !by.is_empty() {
                        by.push(t(", "));
                    }
                    by.push(c(req_label(r, many_versions(&s(&r["name"])))));
                }
            }
            by
        };
        if src["kind"].as_str() == Some("law") {
            let heads = [tr!("条", "Article"), tr!("固定", "Pin"), tr!("引く要件", "Cited by"), tr!("固定している成果物", "Pinned by the artifacts")].into_iter().map(|x| say(x, lang)).collect();
            let mut rows = Vec::new();
            for pin in src["pins"].as_array().cloned().unwrap_or_default() {
                let frag = s(&pin["fragment"]);
                // a rule or a calendar pins an article for the whole file: the file, once
                let mut pinned: Para = Vec::new();
                let mut seen: Vec<String> = Vec::new();
                for a in api["artifacts"].as_array().cloned().unwrap_or_default() {
                    let hit = a["pins"].as_array().cloned().unwrap_or_default().iter().any(|x| s(&x["id"]) == s(&src["id"]) && s(&x["asof"]) == s(&src["asof"]) && s(&x["fragment"]) == frag);
                    let whole = format!("{} \"{}\"", s(&a["tool"]), s(&a["path"]));
                    if hit && !seen.contains(&whole) {
                        if !pinned.is_empty() {
                            pinned.push(t(", "));
                        }
                        pinned.push(c(whole.clone()));
                        seen.push(whole);
                    }
                }
                rows.push(vec![vec![c(frag.clone())], vec![c(short(&s(&pin["sha256"])))], citing(Some(&frag)), pinned]);
            }
            out.push(Block::Table(heads, rows));
        } else {
            let by = citing(None);
            if !by.is_empty() {
                let mut p = vec![t(say(tr!("引く要件：", "Cited by: "), lang))];
                p.extend(by);
                out.push(Block::P(p));
            }
        }
    }

    // ── 4. Why each requirement is so ──
    out.push(Block::H2(say(tr!("要件ごとの「なぜ」", "Why each requirement is so"), lang), "why".into()));
    let mut quoted: BTreeSet<(usize, String, String)> = BTreeSet::new();
    for (ri, r) in reqs.iter().enumerate() {
        let many = many_versions(&s(&r["name"]));
        let fi = p.reqs[ri].file;
        let rel = p.files[fi].rel.clone();
        out.push(Block::H3(req_label(r, many), anchor("why", r, many)));
        out.push(Block::P(vec![b(s(&r["text"]))]));
        let mut facts: Vec<Para> = Vec::new();
        facts.push(vec![t(say(tr!("期間：", "In force: "), lang)), t(period(&r["in_force"]))]);
        facts.push(vec![t(say(tr!("持ち主：", "Owner: "), lang)), t(s(&r["owner"]))]);
        facts.push(vec![t(say(tr!("ファイル：", "File: "), lang)), c(format!("{}:{}", p.shown(&rel), r["line"].as_u64().unwrap_or(0)))]);
        if let Some(h) = r["sha256"].as_str() {
            facts.push(vec![t(say(tr!("要件のハッシュ：", "The requirement's end: "), lang)), c(short(h))]);
        }
        let replaces: Vec<String> = r["replaces"].as_array().cloned().unwrap_or_default().iter().map(|x| format!("{} v{}", s(&x["name"]), x["version"].as_u64().unwrap_or(1))).collect();
        if !replaces.is_empty() {
            facts.push(vec![t(say(tr!("置き換える要件：", "Replaces: "), lang)), c(replaces.join(", "))]);
        }
        if many {
            let others: Vec<String> = reqs
                .iter()
                .filter(|o| o["name"] == r["name"] && o["version"] != r["version"])
                .map(|o| format!("v{} ({})", o["version"].as_u64().unwrap_or(1), period(&o["in_force"])))
                .collect();
            facts.push(vec![t(say(tr!("ほかの版：", "Other versions: "), lang)), t(others.join(", "))]);
        }
        out.push(Block::List(facts));
        // where it comes from, with the articles quoted from the pinned copies
        let decl = p.decl(ri);
        for (i, f) in r["from"].as_array().cloned().unwrap_or_default().iter().enumerate() {
            let mut line = vec![t(say(tr!("出どころ ", "Comes from "), lang)), c(from_words(f)), t(" — ")];
            line.extend(reviewed_words(&f["reviewed"], lang));
            line.push(t(say(tr!("。状態：", ". State: "), lang)));
            line.push(t(state_words(&s(&f["status"]), lang)));
            out.push(Block::P(line));
            push_diags(&mut out, ch, &rel, decl.from.get(i).map(|x| (x.span.line, x.record.as_ref().map(|r| r.line))), lang);
            let Some(FromWhat::Cite { source, fragments, .. }) = decl.from.get(i).map(|x| &x.what) else { continue };
            let Some((_, Resolved::Law { db, id, asof, articles, .. })) = m.sources.files[fi].iter().find(|(n, _)| n == source) else { continue };
            for (frag, _) in fragments {
                let Some(a) = articles.iter().find(|a| &a.fragment == frag) else { continue };
                let key = (fi, source.clone(), frag.clone());
                if quoted.contains(&key) {
                    out.push(Block::P(vec![t(say(tr!("（{source} {frag} の条文は、上に載せた）", "(The text of {source} {frag} is quoted above.)"), lang))]));
                    continue;
                }
                let bytes = a.bytes.clone().or_else(|| std::fs::read(&a.abs).ok());
                let Some(bytes) = bytes else { continue };
                quoted.insert(key);
                let dbw = if db.word() == "ecfr" { "eCFR" } else { "e-Gov" };
                let head = say(tr!("{source} {frag}（{dbw} {id}、{asof} 時点の写し）", "{source} {frag} ({dbw} {id}, the copy as of {asof})"), lang);
                // quoted as `yuen trace` quotes it (ritsu-base's `quote_lines`): an e-Gov article as
                // the law prints it, a table or an eCFR section a line for each line of its text
                let file = a.abs.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                out.push(Block::Quote(Quote { head, lines: crate::copies::quote_lines(*db, &file, &String::from_utf8_lossy(&bytes)) }));
            }
        }
        for d in r["decided"].as_array().cloned().unwrap_or_default() {
            let (by, date, why) = (s(&d["by"]), s(&d["date"]), s(&d["why"]));
            out.push(Block::P(vec![t(say(tr!("{date} に {by} が決めた：", "Decided by {by} on {date}: "), lang)), t(why)]));
        }
        for (i, l) in r["links"].as_array().cloned().unwrap_or_default().iter().enumerate() {
            let side = if s(&l["role"]) == "satisfied" { tr!("満たすもの ", "Met by ") } else { tr!("確かめるもの ", "Checked by ") };
            let mut line = vec![t(say(side, lang)), c(s(&l["artifact"]["text"])), t(" — ")];
            line.extend(reviewed_words(&l["reviewed"], lang));
            line.push(t(say(tr!("。状態：", ". State: "), lang)));
            line.push(t(state_words(&s(&l["status"]), lang)));
            out.push(Block::P(line));
            push_diags(&mut out, ch, &rel, decl.links.get(i).map(|x| (x.span.line, x.record.as_ref().map(|r| r.line))), lang);
        }
        for (i, w) in r["waivers"].as_array().cloned().unwrap_or_default().iter().enumerate() {
            let side = if s(&w["role"]) == "satisfied" { tr!("満たすものを置かない（見送り）：", "Nothing meets it (waived): ") } else { tr!("確かめるものを置かない（見送り）：", "Nothing checks it (waived): ") };
            let mut line = vec![t(say(side, lang)), t(s(&w["why"])), t(" — ")];
            let a = &w["approved"];
            if a.is_null() {
                line.push(t(say(tr!("承認の記録は無い", "no record of an approval"), lang)));
            } else {
                let (by, date) = (s(&a["by"]), s(&a["date"]));
                line.push(t(say(tr!("{by} が {date} に承認した（", "approved by {by} on {date} ("), lang)));
                line.push(c(short(&s(&a["sha256"]))));
                line.push(t(say(tr!("）", ")"), lang)));
            }
            line.push(t(say(tr!("。状態：", ". State: "), lang)));
            line.push(t(state_words(&s(&w["status"]), lang)));
            out.push(Block::P(line));
            push_diags(&mut out, ch, &rel, decl.waivers.get(i).map(|x| (x.span.line, x.record.as_ref().map(|r| r.line))), lang);
        }
    }

    // ── 5. The scope ──
    out.push(Block::H2(say(tr!("範囲", "Scope"), lang), "scope".into()));
    let scopes = api["scopes"].as_array().cloned().unwrap_or_default();
    if scopes.is_empty() {
        out.push(Block::P(vec![t(say(tr!("範囲の宣言は無い。", "No scope is declared."), lang))]));
    }
    for sc in &scopes {
        let n = sc["artifacts"].as_u64().unwrap_or(0);
        let untraced: Vec<String> = sc["untraced"].as_array().cloned().unwrap_or_default().iter().map(|x| s(&x["text"])).collect();
        let mut p = vec![c(format!("scope {}", s(&sc["text"]))), t(" — ")];
        if untraced.is_empty() && n == 1 {
            p.push(t(say(tr!("成果物は 1 個で、要件に辿れる。", "1 artifact in it, which traces to a requirement."), lang)));
            out.push(Block::P(p));
        } else if untraced.is_empty() {
            p.push(t(say(tr!("成果物は {n} 個で、どれも要件に辿れる。", "{n} artifacts in it, every one tracing to a requirement."), lang)));
            out.push(Block::P(p));
        } else {
            let k = untraced.len();
            p.push(t(say(tr!("成果物 {n} 個のうち、{k} 個が要件に辿れない：", "{n} artifacts in it, {k} of them tracing to no requirement:"), lang)));
            out.push(Block::P(p));
            out.push(Block::List(untraced.into_iter().map(|u| vec![c(u)]).collect()));
        }
    }

    // ── 6. The records, by day ──
    out.push(Block::H2(say(tr!("確かめた記録", "Records"), lang), "records".into()));
    let mut recs: Vec<(String, usize, Vec<Para>)> = Vec::new();
    for (ri, r) in reqs.iter().enumerate() {
        let label = req_label(r, many_versions(&s(&r["name"])));
        for f in r["from"].as_array().cloned().unwrap_or_default() {
            let rv = &f["reviewed"];
            if rv.is_null() {
                continue;
            }
            let ups: Vec<String> = rv["up"].as_array().cloned().unwrap_or_default().iter().map(|h| short(&s(h))).collect();
            recs.push((s(&rv["date"]), ri, vec![vec![t(s(&rv["date"]))], vec![t(s(&rv["by"]))], vec![c(from_words(&f)), t(" → "), c(label.clone())], vec![c(format!("{} -> {}", ups.join(", "), short(&s(&rv["down"]))))]]));
        }
        for l in r["links"].as_array().cloned().unwrap_or_default() {
            let rv = &l["reviewed"];
            if rv.is_null() {
                continue;
            }
            let ups: Vec<String> = rv["up"].as_array().cloned().unwrap_or_default().iter().map(|h| short(&s(h))).collect();
            recs.push((s(&rv["date"]), ri, vec![vec![t(s(&rv["date"]))], vec![t(s(&rv["by"]))], vec![c(label.clone()), t(" → "), c(s(&l["artifact"]["text"]))], vec![c(format!("{} -> {}", ups.join(", "), short(&s(&rv["down"]))))]]));
        }
        for w in r["waivers"].as_array().cloned().unwrap_or_default() {
            let a = &w["approved"];
            if a.is_null() {
                continue;
            }
            let what = if s(&w["role"]) == "satisfied" { tr!(" の見送り（満たすもの）を承認", ": nothing meets it, approved") } else { tr!(" の見送り（確かめるもの）を承認", ": nothing checks it, approved") };
            recs.push((s(&a["date"]), ri, vec![vec![t(s(&a["date"]))], vec![t(s(&a["by"]))], vec![c(label.clone()), t(say(what, lang))], vec![c(short(&s(&a["sha256"])))]]));
        }
    }
    recs.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
    if recs.is_empty() {
        out.push(Block::P(vec![t(say(tr!("確かめた記録は、まだ無い。", "No one has recorded looking at anything yet."), lang))]));
    } else {
        let heads = [tr!("日付", "Date"), tr!("誰が", "By"), tr!("何を", "What"), tr!("ハッシュ", "Hashes")].into_iter().map(|x| say(x, lang)).collect();
        out.push(Block::Table(heads, recs.into_iter().map(|(_, _, row)| row).collect()));
    }
    Page { title, blocks: out }
}

/// The diagnostics the check gave at a line of a requirement (the line of a link, a waiver or a
/// `from`, or the line of its record), as `yuen check` prints them: what changed and the diff.
fn push_diags(out: &mut Vec<Block>, ch: &Checked, rel: &str, at: Option<(usize, Option<usize>)>, lang: Lang) {
    let Some((line, record)) = at else { return };
    for d in &ch.diags {
        if d.rel == rel && (d.line == Some(line) || (record.is_some() && d.line == record)) {
            out.push(Block::Pre(d.render(lang)));
        }
    }
}
