//! `sakai doc` (DESIGN 10): the page for the people who have to understand what the code is to
//! do and check it — those who run the business, those who run the systems, the developers who
//! read the code. This module puts together what the page says, in one language, as blocks;
//! `markdown.rs` and `html.rs` write the blocks out, and `draw.rs` draws the HTML page's map.
//!
//! What the page says is what the check found (`Checked`) and what the other languages answer
//! through ritsu's ports: a rule's inputs and outputs (rulec), a book's accounts and transfers
//! (chobo), the dates of a dates file and the days its calendar's data covers (koyomi). A page
//! is written for a map that passes `check`, as `api` and `export cml` are.

pub mod draw;
pub mod html;
pub mod markdown;

use crate::api;
use crate::ast::{Target, ValueTo};
use crate::check::{Checked, Outcome};
use crate::elements::At;
use crate::model::{Model, RelK};
use crate::naming::{Name, Tool};
use crate::suite::Suite;
use ritsu_base::text::{Lang, spaced};
use serde_json::Value;

/// A piece of the page. Text in a block may hold `code` in backquotes and a link to a context's
/// part of the page as `[[alias|text]]`; the writers turn both into their own form.
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    /// A heading, its level (1 to 3), and for a context, the context's alias.
    Heading(u8, String, Option<String>),
    Para(String),
    Table(Vec<String>, Vec<Vec<String>>),
    List(Vec<String>),
    /// The context map, drawn from the page's nodes and edges.
    Map,
}

/// A context on the map.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub alias: String,
    pub name: String,
}

/// How a relationship is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Line {
    /// From the upstream to the downstream.
    Arrow,
    /// A shared kernel or a partnership.
    Both,
    /// Separate ways.
    Dotted,
}

/// A relationship on the map: the two contexts (upstream first for an arrow), the lines of its
/// label, and how it is drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub label: Vec<String>,
    pub line: Line,
}

/// The page: its language, title, the map file it is written from (from the map's directory, its
/// name), the blocks in order, and the map's nodes and edges.
#[derive(Clone, Debug)]
pub struct Page {
    pub lang: Lang,
    pub title: String,
    pub file: String,
    pub blocks: Vec<Block>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

fn t(lang: Lang, ja: &str, en: &str) -> String {
    if lang == Lang::Ja { ja.to_string() } else { en.to_string() }
}

/// The items of a list, joined as the language joins them.
fn join(lang: Lang, items: &[String]) -> String {
    items.join(if lang == Lang::Ja { "、" } else { ", " })
}

/// `Name (alias)`, as a heading or a cell says a context.
fn named(lang: Lang, name: &str, alias: &str) -> String {
    if lang == Lang::Ja { format!("{name}（{alias}）") } else { format!("{name} ({alias})") }
}

/// A path from the root, as a path from the map's directory: the page reads the same wherever
/// `doc` runs.
fn rel(m: &Model, p: &str) -> String {
    let dir = &m.map.dir;
    if dir.is_empty() || dir == "." {
        return p.to_string();
    }
    if p == dir {
        return ".".to_string();
    }
    if let Some(rest) = p.strip_prefix(&format!("{dir}/")) {
        return rest.to_string();
    }
    let ups = dir.split('/').count();
    format!("{}{p}", "../".repeat(ups))
}

/// A naming as the page shows it: the tool, the file from the map's directory, the pairs.
fn naming(m: &Model, n: &Name) -> String {
    let mut s = format!("{} \"{}\"", n.tool.word(), rel(m, &n.path));
    for (k, v) in &n.items {
        s.push_str(&format!(" {k} {}", ritsu_base::naming::word_or_quote(v)));
    }
    s
}

/// A naming in `api`'s JSON, as a `Name`.
fn name_of(v: &Value) -> Option<Name> {
    let tool = Tool::from_word(v["tool"].as_str()?)?;
    let mut n = Name::file(tool, v["path"].as_str()?.to_string());
    for pair in v["items"].as_array()? {
        n = n.with(pair[0].as_str()?, pair[1].as_str()?.to_string());
    }
    Some(n)
}

fn role(lang: Lang, key: &str) -> String {
    let (ja, en) = match key {
        "open_host_service" => ("公開ホストサービス", "open host service"),
        "published_language" => ("公表された言語", "published language"),
        "supplier" => ("供給者", "supplier"),
        "conformist" => ("順応者", "conformist"),
        "anticorruption_layer" => ("腐敗防止層", "anticorruption layer"),
        "customer" => ("顧客", "customer"),
        other => (other, other),
    };
    t(lang, ja, en)
}

fn roles(lang: Lang, v: &Value) -> String {
    let rs: Vec<String> = v.as_array().map(|a| a.iter().filter_map(|r| r.as_str()).map(|r| role(lang, r)).collect()).unwrap_or_default();
    rs.join(if lang == Lang::Ja { "・" } else { ", " })
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
}

/// The link to a context's part of the page.
fn link(m: &Model, name: &str) -> String {
    match m.contexts.iter().find(|c| c.name == name) {
        Some(c) => format!("[[{}|{}]]", c.alias, c.name),
        None => name.to_string(),
    }
}

fn code(s: &str) -> String {
    format!("`{s}`")
}

/// The page of a map that passed `check` (`o.checked` is there), in `lang`.
pub fn page(o: &Outcome, suite: &Suite, lang: Lang) -> Page {
    let c = o.checked.as_ref().expect("a page is written for a map that passes check");
    let m = &c.model;
    let a = api::api(c);
    let h = &m.map.ast.heading;
    let file = m.map.file.rsplit('/').next().unwrap_or(&m.map.file).to_string();
    let mut b = Vec::new();
    let alias = h.alias.clone().unwrap_or_default();
    let title = if lang == Lang::Ja {
        format!("コンテキストマップ：{}（{alias}）v{}", h.name, h.version)
    } else {
        format!("Context map: {} ({alias}) v{}", h.name, h.version)
    };
    b.push(Block::Heading(1, title.clone(), None));
    if let Some(d) = &m.map.ast.description {
        b.push(Block::Para(d.value.clone()));
    }
    if let Some(s) = &o.summary {
        let s = spaced(s, lang);
        b.push(Block::Para(if lang == Lang::Ja {
            format!("{} から書いた。`sakai check` の結果：{s}。", code(&file))
        } else {
            format!("Written from {}. `sakai check` says: {s}.", code(&file))
        }));
    }

    // 1. the map
    let rels: Vec<Value> = a["relationships"].as_array().cloned().unwrap_or_default();
    let nodes: Vec<Node> = m.contexts.iter().map(|x| Node { alias: x.alias.clone(), name: x.name.clone() }).collect();
    let idx = |name: &str| m.contexts.iter().position(|x| x.name == name).unwrap_or(0);
    let mut edges = Vec::new();
    for r in &rels {
        match r["kind"].as_str().unwrap_or("") {
            "upstream_downstream" => {
                let through = strs(&r["through"]).join(" ");
                let mut label: Vec<String> = strs(&r["roles"]["upstream"]).iter().map(|x| role(lang, x)).collect();
                if let Some(last) = label.last_mut() {
                    last.push_str(&format!(" {through}"));
                }
                label.push(format!("→ {}", roles(lang, &r["roles"]["downstream"])));
                edges.push(Edge { from: idx(r["upstream"].as_str().unwrap_or("")), to: idx(r["downstream"].as_str().unwrap_or("")), label, line: Line::Arrow });
            }
            k => {
                let between = strs(&r["between"]);
                let (label, line) = match k {
                    "shared_kernel" => (t(lang, "共有カーネル", "shared kernel"), Line::Both),
                    "partnership" => (t(lang, "パートナーシップ", "partnership"), Line::Both),
                    _ => (t(lang, "別々の道", "separate ways"), Line::Dotted),
                };
                edges.push(Edge { from: idx(&between[0]), to: idx(&between[1]), label: vec![label], line });
            }
        }
    }
    b.push(Block::Map);
    b.push(Block::Para(t(
        lang,
        "矢印は上流から下流へ向かい、上流の役割と、通る公表された言語と、下流の役割を書く。両向きの線は共有カーネルかパートナーシップ、点線は別々の道。",
        "An arrow goes from the upstream to the downstream, with the upstream's roles, the published language the relationship goes through, and the downstream's role. A line with two heads is a shared kernel or a partnership; a dotted line is separate ways.",
    )));

    // 2. the contexts
    b.push(Block::Heading(2, t(lang, "コンテキスト", "Contexts"), None));
    let count = |ci: usize| c.artifacts.iter().filter(|x| x.owner.map(|o| o.0) == Some(ci)).count();
    b.push(Block::Table(
        vec![t(lang, "コンテキスト", "Context"), t(lang, "別名", "Alias"), t(lang, "持ち主", "Owner"), t(lang, "成果物", "Artifacts"), t(lang, "説明", "Description")],
        m.contexts
            .iter()
            .enumerate()
            .map(|(ci, x)| {
                vec![
                    format!("[[{}|{}]]", x.alias, x.name),
                    code(&x.alias),
                    x.ast.owner.as_ref().map(|s| s.value.clone()).unwrap_or_default(),
                    count(ci).to_string(),
                    x.ast.description.as_ref().map(|s| s.value.clone()).unwrap_or_default(),
                ]
            })
            .collect(),
    ));

    // 3. each context
    let crossings: Vec<Value> = a["crossings"].as_array().cloned().unwrap_or_default();
    for (ci, x) in m.contexts.iter().enumerate() {
        b.push(Block::Heading(2, named(lang, &x.name, &x.alias), Some(x.alias.clone())));
        if let Some(d) = &x.ast.description {
            b.push(Block::Para(d.value.clone()));
        }
        let mut facts = Vec::new();
        if let Some(o) = &x.ast.owner {
            facts.push(format!("{}{}", t(lang, "持ち主：", "Owner: "), o.value));
        }
        if !x.ast.also.is_empty() {
            let also: Vec<String> = x.ast.also.iter().map(|s| s.value.clone()).collect();
            facts.push(format!("{}{}", t(lang, "ほかの呼び名：", "Also called: "), join(lang, &also)));
        }
        facts.push(format!("{}{}", t(lang, "書いたファイル：", "Written in: "), code(&rel(m, &x.file))));
        b.push(Block::List(facts));

        b.push(Block::Heading(3, t(lang, "成果物", "Artifacts"), None));
        b.push(artifacts(c, suite, ci, lang));

        if !x.published.is_empty() {
            b.push(Block::Heading(3, t(lang, "公表された言語", "Published language"), None));
            b.push(Block::List(x.published.iter().map(|p| published(c, p, lang)).collect()));
        }

        if !x.ast.terms.is_empty() {
            b.push(Block::Heading(3, t(lang, "用語集", "Glossary"), None));
            b.push(Block::Table(
                vec![t(lang, "語", "Term"), t(lang, "定義", "Definition"), t(lang, "指すもの", "Means"), t(lang, "越えていく先", "Crosses into")],
                (0..x.ast.terms.len()).map(|ti| term_row(c, &rels, &crossings, ci, ti, lang)).collect(),
            ));
        }

        b.push(Block::Heading(3, t(lang, "関係", "Relationships"), None));
        let mine: Vec<String> = rels.iter().filter_map(|r| relationship(m, r, &crossings, &x.name, lang)).collect();
        if mine.is_empty() {
            b.push(Block::Para(t(lang, "関係はない。", "It has no relationship.")));
        } else {
            b.push(Block::List(mine));
        }
    }

    // 4. the glossary index
    b.push(Block::Heading(2, t(lang, "用語集の索引", "Glossary index"), None));
    let mut all: Vec<(String, usize, usize)> = Vec::new();
    for (ci, x) in m.contexts.iter().enumerate() {
        for (ti, term) in x.ast.terms.iter().enumerate() {
            all.push((term.name.clone(), ci, ti));
        }
    }
    all.sort();
    let rows: Vec<Vec<String>> = all
        .iter()
        .map(|(name, ci, ti)| {
            let x = &m.contexts[*ci];
            let term = &x.ast.terms[*ti];
            let others: Vec<String> = all.iter().filter(|(n, cj, _)| n == name && cj != ci).map(|(_, cj, _)| link(m, &m.contexts[*cj].name)).collect();
            let row = term_row(c, &rels, &crossings, *ci, *ti, lang);
            let mut across = row[3].clone();
            if !others.is_empty() {
                let also = if lang == Lang::Ja { format!("同じ名前の語が {} にもある", join(lang, &others)) } else { format!("a term of the same name is in {}", join(lang, &others)) };
                across = if across == "—" { also } else if lang == Lang::Ja { format!("{across}。{also}") } else { format!("{across}; {also}") };
            }
            vec![code(name), link(m, &x.name), term.definition.as_ref().map(|s| s.value.clone()).unwrap_or_default(), across]
        })
        .collect();
    if rows.is_empty() {
        b.push(Block::Para(t(lang, "どのコンテキストも語を書いていない。", "No context writes a term.")));
    } else {
        b.push(Block::Table(vec![t(lang, "語", "Term"), t(lang, "コンテキスト", "Context"), t(lang, "定義", "Definition"), t(lang, "境界を越えるとき", "Across a boundary")], rows));
    }

    // 5. the mappings
    b.push(Block::Heading(2, t(lang, "対応", "Mappings"), None));
    let before = b.len();
    mappings(c, &mut b, lang);
    if b.len() == before {
        b.push(Block::Para(t(lang, "列挙を読み替える腐敗防止層はない。", "No anticorruption layer maps an enum.")));
    }

    // 6. what is not checked
    b.push(Block::Heading(2, t(lang, "確かめていないこと", "What is not checked"), None));
    b.push(Block::List(vec![
        t(lang, "腐敗防止層のコードが、書いた対応のとおりに読み替えているか。sakai が確かめるのは、対応が上流の列挙を網羅していることと、対応の先が下流の列挙にあることまで（規則が先のときは、規則の表を rulec が確かめる）。", "Whether the code of an anticorruption layer maps as the mapping says. sakai checks that the mapping covers the upstream's enum and that what it maps to is in the downstream's enum (where a rule is the target, rulec checks the rule's tables)."),
        t(lang, "契約に書いていない呼び出し：OpenAPI の文書の無い HTTP（URL を文字列で持つもの）、AsyncAPI の文書の無いメッセージのキュー、データベースの共有、リフレクションと動的な import。OpenAPI と AsyncAPI の文書に書いた HTTP の操作とチャネルは、ほかの成果物と同じく確かめる。", "Calls no contract writes: HTTP to a URL held in a string with no OpenAPI document, a message queue with no AsyncAPI document, a shared database, reflection and dynamic imports. The HTTP operations and the channels written in OpenAPI and AsyncAPI documents are checked like any other artifact."),
        t(lang, "コードが、OpenAPI と AsyncAPI の文書のとおりに HTTP を呼び、チャネルに送り、チャネルから受けているか。sakai が確かめるのは、文書どうしと、文書と地図が合っていることまで。", "Whether the code calls HTTP, and sends to and receives from the channels, as the OpenAPI and AsyncAPI documents say: sakai checks the documents against each other and against the map."),
        t(lang, "生成したコードの置き場所のコードが、本当にその公表された言語から生成したものか。", "Whether the code where generated code is kept was really generated from the published language."),
        t(lang, "語の定義の文の中身。", "What a term's definition says."),
        t(lang, "コードの import。これは sakai build が書く設定で、import-linter、dependency-cruiser、ArchUnit、go-arch-lint が CI で確かめる。", "The imports of the code: the settings `sakai build` writes have import-linter, dependency-cruiser, ArchUnit and go-arch-lint check them in CI."),
    ]));
    b.push(Block::Para(if lang == Lang::Ja {
        format!("sakai {} が {} から書いた。", env!("CARGO_PKG_VERSION"), code(&file))
    } else {
        format!("Written by sakai {} from {}.", env!("CARGO_PKG_VERSION"), code(&file))
    }));
    Page { lang, title, file, blocks: b, nodes, edges }
}

/// The table of a context's artifacts: each artifact of a language with what its language says it
/// holds, then the code, counted.
fn artifacts(c: &Checked, suite: &Suite, ci: usize, lang: Lang) -> Block {
    let m = &c.model;
    let mine: Vec<&crate::owners::Artifact> = c.artifacts.iter().filter(|x| x.owner.map(|o| o.0) == Some(ci)).collect();
    let mut rows = Vec::new();
    for x in mine.iter().filter(|x| x.tool != Tool::File && x.contract.is_none()) {
        rows.push(vec![code(x.tool.word()), code(&rel(m, &x.path)), holds(c, suite, x, lang)]);
    }
    for x in mine.iter().filter(|x| x.contract.is_some()) {
        let (k, _) = x.contract.unwrap();
        rows.push(vec![code(k.word()), code(&rel(m, &x.path)), contract_holds(c, &x.path, lang)]);
    }
    let files = mine.iter().filter(|x| x.tool == Tool::File && x.contract.is_none()).count();
    if files > 0 {
        let what = if lang == Lang::Ja { format!("{files} 個のファイル（コード）") } else { format!("{files} files (code)") };
        rows.push(vec![code("file"), what, String::new()]);
    }
    if rows.is_empty() {
        return Block::Para(t(lang, "成果物はない。", "It owns no artifact."));
    }
    Block::Table(vec![t(lang, "ツール", "Tool"), t(lang, "ファイル", "File"), t(lang, "持っているもの", "What it holds")], rows)
}

/// What a language says an artifact holds: a rule's inputs and outputs, a book's accounts and
/// transfers, a dates file's dates and its calendar's data, a calendar's data, a `.proto`'s
/// messages, enums and services.
fn holds(c: &Checked, suite: &Suite, x: &crate::owners::Artifact, lang: Lang) -> String {
    let m = &c.model;
    let names = |v: Vec<String>| join(lang, &v.iter().map(|n| code(n)).collect::<Vec<_>>());
    match x.tool {
        Tool::Rulec => match c.read.facts.get(&x.path) {
            Some(f) => {
                let ins = names(f.inputs.iter().map(|i| i.name.clone()).collect());
                let outs = names(f.outputs.iter().map(|o| o.name.clone()).collect());
                if lang == Lang::Ja { format!("入力 {ins}／出力 {outs}") } else { format!("inputs {ins}; outputs {outs}") }
            }
            None => String::new(),
        },
        Tool::Chobo => match crate::suite::book_names(suite, &m.root.join(&x.path)) {
            Some(Ok((accounts, transfers))) => {
                let (a, tr) = (names(accounts), names(transfers));
                if lang == Lang::Ja { format!("勘定 {a}／振替 {tr}") } else { format!("accounts {a}; transfers {tr}") }
            }
            _ => String::new(),
        },
        Tool::Koyomi => calendar(c, suite, &x.path, lang),
        Tool::Proto => match c.protos.files.get(&x.path) {
            Some(f) => {
                let mut parts = Vec::new();
                let msgs: Vec<String> = f.messages.iter().map(|x| x.name.clone()).collect();
                let enums: Vec<String> = f.enums.iter().map(|x| x.name.clone()).collect();
                let svcs: Vec<String> = f.services.iter().map(|x| x.name.clone()).collect();
                for (v, ja, en) in [(msgs, "メッセージ", "messages"), (enums, "列挙", "enums"), (svcs, "サービス", "services")] {
                    if !v.is_empty() {
                        parts.push(format!("{} {}", t(lang, ja, en), names(v)));
                    }
                }
                parts.join(if lang == Lang::Ja { "／" } else { "; " })
            }
            None => String::new(),
        },
        Tool::Dandori => {
            let refs = c.read.refs_of(&x.path);
            let mut parts = Vec::new();
            let services: Vec<String> = refs.iter().filter(|r| r.how == "implements").filter_map(|r| r.target.items.first().map(|(_, s)| s.clone())).collect();
            if !services.is_empty() {
                parts.push(format!("{} {}", t(lang, "実装するサービス", "implements"), names(services)));
            }
            let flows: Vec<String> = refs.iter().filter(|r| r.how == "flow").map(|r| rel(m, &r.target.path)).collect();
            if !flows.is_empty() {
                parts.push(format!("{} {}", t(lang, "子のフロー", "child flows"), names(flows)));
            }
            parts.join(if lang == Lang::Ja { "／" } else { "; " })
        }
        _ => String::new(),
    }
}

/// What an OpenAPI or AsyncAPI document holds (DESIGN 15.7): the specification and the API it
/// describes, its HTTP operations, its channels and its operations on them, and its schemas and
/// enums; for a part of a document, the schemas in it.
fn contract_holds(c: &Checked, path: &str, lang: Lang) -> String {
    let cs = &c.contracts;
    let Some(d) = cs.docs.get(path) else { return String::new() };
    let names = |v: Vec<String>| join(lang, &v.iter().map(|n| code(n)).collect::<Vec<_>>());
    let sep = if lang == Lang::Ja { "／" } else { "; " };
    let mut parts = Vec::new();
    if d.part {
        let keys: Vec<String> = d.root.as_map().unwrap_or_default().iter().map(|(k, _)| k.name.clone()).collect();
        parts.push(if lang == Lang::Ja { format!("文書の一部 {}", names(keys)) } else { format!("a part of a document: {}", names(keys)) });
        return parts.join(sep);
    }
    let title = if d.title.is_empty() { String::new() } else if lang == Lang::Ja { format!("「{}」", d.title) } else { format!(" \"{}\"", d.title) };
    let version = match (d.version.is_empty(), lang) {
        (true, _) => String::new(),
        (false, Lang::Ja) => format!("、バージョン {}", d.version),
        (false, _) => format!(", version {}", d.version),
    };
    parts.push(format!("{} {}{title}{version}", d.kind.title(), d.spec));
    let ops: Vec<String> = cs.operations(path).iter().map(|o| if o.id.is_empty() { code(&format!("{} {}", o.method, o.path)) } else if lang == Lang::Ja { format!("{}（{} {}）", code(&o.id), o.method, o.path) } else { format!("{} ({} {})", code(&o.id), o.method, o.path) }).collect();
    if !ops.is_empty() {
        parts.push(format!("{} {}", t(lang, "操作", "operations"), join(lang, &ops)));
    }
    let chans: Vec<String> = cs
        .channels(path)
        .iter()
        .map(|ch| {
            let what = match &ch.elsewhere {
                Some(f) => {
                    let m = &c.model;
                    if lang == Lang::Ja { format!("{} のもの", code(&rel(m, f))) } else { format!("from {}", code(&rel(m, f))) }
                }
                None => code(&ch.address),
            };
            if lang == Lang::Ja { format!("{}（{what}）", code(&ch.id)) } else { format!("{} ({what})", code(&ch.id)) }
        })
        .collect();
    if !chans.is_empty() {
        parts.push(format!("{} {}", t(lang, "チャネル", "channels"), join(lang, &chans)));
    }
    let acts: Vec<String> = cs
        .actions(path)
        .iter()
        .map(|a| {
            let on = a.channel.clone().unwrap_or_default();
            if lang == Lang::Ja { format!("{}（{} {}）", code(&a.id), a.action, code(&on)) } else { format!("{} ({} {})", code(&a.id), a.action, code(&on)) }
        })
        .collect();
    if !acts.is_empty() {
        parts.push(format!("{} {}", t(lang, "送受信", "sends and receives"), join(lang, &acts)));
    }
    let schemas: Vec<(String, bool)> = d
        .root
        .get("components")
        .and_then(|x| x.get("schemas"))
        .and_then(|x| x.as_map())
        .unwrap_or_default()
        .iter()
        .map(|(k, _)| (k.name.clone(), cs.enum_values(path, &format!("/components/schemas/{}", crate::contracts::escape(&k.name))).is_some()))
        .collect();
    let (enums, others): (Vec<_>, Vec<_>) = schemas.into_iter().partition(|(_, e)| *e);
    if !others.is_empty() {
        parts.push(format!("{} {}", t(lang, "スキーマ", "schemas"), names(others.into_iter().map(|(n, _)| n).collect())));
    }
    if !enums.is_empty() {
        parts.push(format!("{} {}", t(lang, "列挙", "enums"), names(enums.into_iter().map(|(n, _)| n).collect())));
    }
    parts.join(sep)
}

/// What koyomi says of a `.cal`: a dates file's dates, and the calendar it uses with the days its
/// data covers; a calendar, the days its data covers, as a dates file that uses it is told.
fn calendar(c: &Checked, suite: &Suite, path: &str, lang: Lang) -> String {
    let m = &c.model;
    let Some(port) = &suite.dates else { return String::new() };
    let range = |cal: &ritsu_ports::DateCalendar| {
        let (from, to) = (ritsu_ports::day_text(cal.data.0), ritsu_ports::day_text(cal.data.1));
        if lang == Lang::Ja { format!("カレンダー {} のデータ {from}..{to}", code(&cal.name)) } else { format!("calendar {}, its data {from}..{to}", code(&cal.name)) }
    };
    if let Ok(f) = port.facts(&m.root.join(path)) {
        let dates: Vec<String> = f.functions.iter().map(|d| code(&d.name)).collect();
        let mut s = format!("{} {}", t(lang, "日付", "dates"), join(lang, &dates));
        if let Some(cal) = &f.calendar {
            s.push_str(if lang == Lang::Ja { "／" } else { "; " });
            s.push_str(&range(cal));
        }
        return s;
    }
    // a calendar: what a dates file of the map that uses it is told of it
    for (file, tool, refs) in &c.read.refs {
        if *tool != Tool::Koyomi || !refs.iter().any(|r| r.how == "use calendar" && r.target.path == path) {
            continue;
        }
        if let Ok(f) = port.facts(&m.root.join(file))
            && let Some(cal) = &f.calendar
        {
            return range(cal);
        }
    }
    String::new()
}

/// One published language: its package, what it is written in, its open host services with their
/// methods, the workflows that implement them, and where its generated code is.
fn published(c: &Checked, p: &crate::model::Pub, lang: Lang) -> String {
    let m = &c.model;
    let mut parts = Vec::new();
    let from: Vec<String> = p
        .protos
        .iter()
        .map(|(f, _)| code(&rel(m, f)))
        .chain(p.rulec.iter().map(|(f, _)| code(&rel(m, f))))
        .chain(p.krate.iter().map(|(d, _)| code(&rel(m, d))))
        .chain(p.contracts.iter().map(|(k, f, _)| format!("{} {}", k.title(), code(&rel(m, f)))))
        .collect();
    if !from.is_empty() {
        parts.push(format!("{}{}", t(lang, "書いたもの ", "written in "), join(lang, &from)));
    }
    // the open host services of OpenAPI and AsyncAPI documents: HTTP operations and channels
    if !p.contracts.is_empty() {
        let cs = &c.contracts;
        for (s, _) in &p.services {
            let mut what = None;
            for (_, f, _) in &p.contracts {
                if let Some(o) = cs.operations(f).into_iter().find(|o| o.name() == *s) {
                    what = Some(if lang == Lang::Ja { format!("HTTP の操作 {} {}", o.method, o.path) } else { format!("the HTTP operation {} {}", o.method, o.path) });
                }
                if let Some(ch) = cs.channels(f).into_iter().find(|ch| ch.id == *s) {
                    what = Some(if lang == Lang::Ja { format!("チャネル {}", ch.address) } else { format!("the channel {}", ch.address) });
                }
            }
            let mut one = format!("{}{}", t(lang, "公開ホストサービス ", "open host service "), code(s));
            if let Some(w) = what {
                one.push_str(&if lang == Lang::Ja { format!("（{w}）") } else { format!(" ({w})") });
            }
            parts.push(one);
        }
        if !p.generated.is_empty() {
            let g: Vec<String> = p.generated.iter().map(|(d, _)| code(&rel(m, d))).collect();
            parts.push(format!("{}{}", t(lang, "生成したコード ", "generated code in "), join(lang, &g)));
        }
        return if lang == Lang::Ja { format!("{}：{}", code(&p.package), parts.join("。")) } else { format!("{}: {}", code(&p.package), parts.join("; ")) };
    }
    for (s, _) in &p.services {
        let methods: Vec<String> = p
            .protos
            .iter()
            .filter_map(|(f, _)| c.protos.files.get(f))
            .flat_map(|f| f.services.iter().filter(|x| x.name == *s).flat_map(|x| x.methods.iter().map(|mm| mm.name.clone())))
            .collect();
        let by: Vec<String> = c
            .read
            .refs
            .iter()
            .filter(|(_, tool, refs)| *tool == Tool::Dandori && refs.iter().any(|r| r.how == "implements" && r.target.items.first().is_some_and(|(_, n)| n == s) && p.protos.iter().any(|(f, _)| *f == r.target.path)))
            .map(|(f, _, _)| code(&rel(m, f)))
            .collect();
        let mut one = format!("{}{}", t(lang, "公開ホストサービス ", "open host service "), code(s));
        if !methods.is_empty() {
            one.push_str(&if lang == Lang::Ja { format!("（{}）", join(lang, &methods.iter().map(|x| code(x)).collect::<Vec<_>>())) } else { format!(" ({})", join(lang, &methods.iter().map(|x| code(x)).collect::<Vec<_>>())) });
        }
        if !by.is_empty() {
            one.push_str(&if lang == Lang::Ja { format!("。{} が実装する", join(lang, &by)) } else { format!(", implemented by {}", join(lang, &by)) });
        }
        parts.push(one);
    }
    if !p.generated.is_empty() {
        let g: Vec<String> = p.generated.iter().map(|(d, _)| code(&rel(m, d))).collect();
        parts.push(format!("{}{}", t(lang, "生成したコード ", "generated code in "), join(lang, &g)));
    }
    if lang == Lang::Ja { format!("{}：{}", code(&p.package), parts.join("。")) } else { format!("{}: {}", code(&p.package), parts.join("; ")) }
}

/// The row of a term: the term, its definition, what it means, and where it crosses into (the
/// contexts whose artifacts refer to what it means, and what it is read as there).
fn term_row(c: &Checked, rels: &[Value], crossings: &[Value], ci: usize, ti: usize, lang: Lang) -> Vec<String> {
    let m = &c.model;
    let x = &m.contexts[ci];
    let term = &x.ast.terms[ti];
    let means: Vec<Name> = (0..term.means.len()).filter_map(|mi| c.elements.get(At::Means(ci, ti, mi)).cloned()).collect();
    // the contexts whose artifacts refer to what the term means, in the order of the crossings
    let mut across: Vec<String> = Vec::new();
    for cr in crossings {
        if cr["to_context"].as_str() != Some(x.name.as_str()) {
            continue;
        }
        // what crosses, a document's elements among them, as references
        let mut reached: Vec<Name> = cr["elements"].as_array().map(|a| a.iter().filter_map(name_of).collect()).unwrap_or_default();
        if let Some(to) = name_of(&cr["to"]) {
            reached.push(to);
        }
        let into = cr["from_context"].as_str().unwrap_or("").to_string();
        if means.iter().any(|e| reached.iter().any(|r| r.is_or_contains(e) || e.is_or_contains(r))) && !across.contains(&into) {
            across.push(into);
        }
    }
    // what it is read as there: a value across an anticorruption layer, or a term the layer maps
    // it to
    let mut read_as: Vec<(String, String)> = Vec::new();
    for e in &means {
        let Some(("value", v)) = e.items.get(1).map(|(k, v)| (k.as_str(), v.as_str())) else { continue };
        for (down, value) in values_across(c, rels, e, v) {
            read_as.push((down, code(&value)));
        }
    }
    for r in rels {
        if r["kind"] != "upstream_downstream" || r["upstream"].as_str() != Some(x.name.as_str()) {
            continue;
        }
        for tm in r["terms"].as_array().into_iter().flatten() {
            if tm["from"].as_str() == Some(term.name.as_str()) {
                let down = r["downstream"].as_str().unwrap_or("").to_string();
                let to = tm["to"].as_str().unwrap_or("");
                read_as.push((down, if lang == Lang::Ja { format!("語 {}", code(to)) } else { format!("the term {}", code(to)) }));
            }
        }
    }
    for (down, _) in &read_as {
        if !across.contains(down) {
            across.push(down.clone());
        }
    }
    let cells: Vec<String> = across
        .iter()
        .map(|ctx| {
            let as_: Vec<String> = read_as.iter().filter(|(d, _)| d == ctx).map(|(_, v)| v.clone()).collect();
            match (as_.is_empty(), lang) {
                (true, _) => link(m, ctx),
                (false, Lang::Ja) => format!("{}（{} として）", link(m, ctx), join(lang, &as_)),
                (false, _) => format!("{} (as {})", link(m, ctx), join(lang, &as_)),
            }
        })
        .collect();
    let cell = if cells.is_empty() { "—".to_string() } else { join(lang, &cells) };
    let means_text: Vec<String> = means.iter().map(|n| code(&naming(m, n))).collect();
    vec![code(&term.name), term.definition.as_ref().map(|s| s.value.clone()).unwrap_or_default(), if means_text.is_empty() { "—".to_string() } else { join(lang, &means_text) }, cell]
}

/// The downstream contexts and values an upstream enum's value `v` is mapped to: by the values
/// the `.ctx` writes, or by the rule that takes the enum in.
fn values_across(c: &Checked, _rels: &[Value], e: &Name, v: &str) -> Vec<(String, String)> {
    let m = &c.model;
    let mut out = Vec::new();
    // the enum the value is of: a document's schema, a `.proto`'s or a rule's enum
    let enum_of = Name { items: e.items[..1].to_vec(), ..e.clone() };
    for (ci, x) in m.contexts.iter().enumerate() {
        for (ri, r) in x.rels.iter().enumerate() {
            let RelK::Upstream { enums, .. } = &r.kind else { continue };
            for (ei, em) in enums.iter().enumerate() {
                if c.elements.get(At::From(ci, ri, ei)) != Some(&enum_of) {
                    continue;
                }
                for (from, to) in pairs(c, ci, ri, ei, em) {
                    if from == v
                        && let Some(to) = to
                    {
                        out.push((x.name.clone(), to));
                    }
                }
            }
        }
    }
    out
}

/// The value lines of an enum mapping, each upstream value and the downstream value (None when
/// refused): what the `.ctx` writes, else what the rule it maps to takes the enum in as.
fn pairs(c: &Checked, ci: usize, ri: usize, ei: usize, em: &crate::ast::EnumMap) -> Vec<(String, Option<String>)> {
    if !em.values.is_empty() {
        return em
            .values
            .iter()
            .map(|v| match &v.to {
                ValueTo::Value(x, _) => (v.from.clone(), Some(x.clone())),
                ValueTo::Refuse(..) => (v.from.clone(), None),
            })
            .collect();
    }
    rule_pairs(c, ci, ri, ei).unwrap_or_default().into_iter().map(|(a, b)| (a, Some(b))).collect()
}

/// The pairs a rule's `import proto` makes of an enum mapping whose target is the rule's enum.
fn rule_pairs(c: &Checked, ci: usize, ri: usize, ei: usize) -> Option<Vec<(String, String)>> {
    let from = c.elements.get(At::From(ci, ri, ei))?;
    let to = c.elements.get(At::To(ci, ri, ei))?;
    if to.tool != Tool::Rulec {
        return None;
    }
    let (_, name) = to.items.first()?;
    let f = c.protos.files.get(&from.path)?;
    let full = f.full(&from.items.first()?.1);
    crate::mapping::rule_import(&c.read, &to.path, name, &from.path, &full)
}

/// The tables of the mappings: for each anticorruption layer's enum mapping, the upstream's
/// values and what each becomes downstream, and where that is decided.
fn mappings(c: &Checked, b: &mut Vec<Block>, lang: Lang) {
    let m = &c.model;
    for (ci, x) in m.contexts.iter().enumerate() {
        for (ri, r) in x.rels.iter().enumerate() {
            let RelK::Upstream { enums, layer, .. } = &r.kind else { continue };
            let up = &m.contexts[r.partner];
            for (ei, em) in enums.iter().enumerate() {
                let from = c.elements.get(At::From(ci, ri, ei)).map(|n| naming(m, n)).unwrap_or_else(|| em.from.clone());
                let to = match &em.target {
                    Target::Name(n, _) => n.clone(),
                    Target::Element(_) => c.elements.get(At::To(ci, ri, ei)).map(|n| naming(m, n)).unwrap_or_default(),
                };
                let head = if lang == Lang::Ja { format!("{} ← {}：{}", x.name, up.name, em.from) } else { format!("{} ← {}: {}", x.name, up.name, em.from) };
                b.push(Block::Heading(3, head, None));
                let lay: Vec<String> = layer.iter().map(|o| code(&format!("{} \"{}\"", o.contract.map(|k| k.word()).or(o.tool.map(|t| t.word())).unwrap_or("dir"), rel(m, &o.path)))).collect();
                let mut says = vec![if lang == Lang::Ja { format!("{} を {} に読み替える。", code(&from), code(&to)) } else { format!("Maps {} to {}.", code(&from), code(&to)) }];
                if !lay.is_empty() {
                    says.push(if lang == Lang::Ja { format!("層は {}。", join(lang, &lay)) } else { format!("The layer is {}.", join(lang, &lay)) });
                }
                let rule = rule_pairs(c, ci, ri, ei);
                let rows: Vec<Vec<String>> = if em.values.is_empty() && rule.is_some() {
                    says.push(t(lang, "対応は、規則の `import proto` から rulec が読んだもの（規則の表は rulec が確かめる）。", "The mapping is read from the rule's `import proto` by rulec (rulec checks the rule's tables)."));
                    rule.unwrap().into_iter().map(|(a, v)| vec![code(&a), code(&v)]).collect()
                } else {
                    let checked = match &em.target {
                        Target::Name(..) => false,
                        Target::Element(_) => c.elements.get(At::To(ci, ri, ei)).is_some(),
                    };
                    says.push(if checked {
                        if lang == Lang::Ja { format!("対応は {} に書いたもの。下流の値は、対応の先の列挙にあることを確かめた。", code(&rel(m, &x.file))) } else { format!("The mapping is written in {}; the downstream values are checked to be in the target enum.", code(&rel(m, &x.file))) }
                    } else if lang == Lang::Ja {
                        format!("対応は {} に書いたもの。対応の先は名前だけなので、下流の値は確かめていない。", code(&rel(m, &x.file)))
                    } else {
                        format!("The mapping is written in {}. Its target is only a name, so the downstream values are not checked.", code(&rel(m, &x.file)))
                    });
                    em.values
                        .iter()
                        .map(|v| match &v.to {
                            ValueTo::Value(x, _) => vec![code(&v.from), code(x)],
                            ValueTo::Refuse(why, _) => {
                                let why = why.as_ref().map(|s| s.value.clone()).unwrap_or_default();
                                vec![code(&v.from), if lang == Lang::Ja { format!("拒否：{why}") } else { format!("refused: {why}") }]
                            }
                        })
                        .collect()
                };
                b.push(Block::Para(says.join(" ")));
                b.push(Block::Table(vec![t(lang, "上流の値", "Upstream value"), t(lang, "下流の値", "Downstream value")], rows));
            }
        }
    }
}

/// One relationship as a context's part of the page says it, with the references that cross
/// through it; None when the context is not in it.
fn relationship(m: &Model, r: &Value, crossings: &[Value], me: &str, lang: Lang) -> Option<String> {
    let declared = strs(&r["declared"]);
    let through_it = |cr: &&Value| cr["allowed_by"]["declared"].as_str().is_some_and(|d| declared.iter().any(|x| x == d));
    let refs: Vec<String> = crossings
        .iter()
        .filter(through_it)
        .filter(|cr| cr["from_context"].as_str() == Some(me) || cr["to_context"].as_str() == Some(me))
        .map(|cr| {
            let from = name_of(&cr["from"]).map(|n| rel(m, &n.path)).unwrap_or_default();
            let to = name_of(&cr["to"]).map(|n| naming(m, &n)).unwrap_or_default();
            format!("{} ({}) → {}", code(&format!("{from}:{}", cr["line"])), cr["via"].as_str().unwrap_or(""), code(&to))
        })
        .collect();
    let kind = r["kind"].as_str()?;
    let head = match kind {
        "upstream_downstream" => {
            let (up, down) = (r["upstream"].as_str()?, r["downstream"].as_str()?);
            let through = strs(&r["through"]).iter().map(|x| code(x)).collect::<Vec<_>>().join(" ");
            let (ur, dr) = (roles(lang, &r["roles"]["upstream"]), roles(lang, &r["roles"]["downstream"]));
            if down == me {
                if lang == Lang::Ja { format!("上流 {}：{ur} {through} → {dr}", link(m, up)) } else { format!("Upstream {}: {ur} {through} → {dr}", link(m, up)) }
            } else if up == me {
                if lang == Lang::Ja { format!("下流 {}：{ur} {through} → {dr}", link(m, down)) } else { format!("Downstream {}: {ur} {through} → {dr}", link(m, down)) }
            } else {
                return None;
            }
        }
        _ => {
            let between = strs(&r["between"]);
            if !between.iter().any(|x| x == me) {
                return None;
            }
            let other = between.iter().find(|x| *x != me).cloned().unwrap_or_default();
            match kind {
                "shared_kernel" => {
                    let side = r["sides"].as_array().and_then(|s| s.iter().find(|s| s["context"].as_str() == Some(me)).or(s.first())).cloned().unwrap_or(Value::Null);
                    let items: Vec<String> = side["items"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|i| match i.get("dir") {
                            Some(d) => code(&format!("dir \"{}\"", rel(m, d.as_str().unwrap_or("")))),
                            None => name_of(&i["name"]).map(|n| code(&naming(m, &n))).unwrap_or_default(),
                        })
                        .collect();
                    if lang == Lang::Ja { format!("{} と共有カーネル：{}", link(m, &other), join(lang, &items)) } else { format!("Shared kernel with {}: {}", link(m, &other), join(lang, &items)) }
                }
                "partnership" => {
                    if lang == Lang::Ja { format!("{} とパートナーシップ", link(m, &other)) } else { format!("Partnership with {}", link(m, &other)) }
                }
                _ => {
                    if lang == Lang::Ja { format!("{} と別々の道（越える参照が無いことを確かめた）", link(m, &other)) } else { format!("Separate ways from {} (no reference crosses, as checked)", link(m, &other)) }
                }
            }
        }
    };
    if refs.is_empty() || kind == "separate_ways" {
        return Some(head);
    }
    let sep = if lang == Lang::Ja { "。越える参照：" } else { ". References that cross: " };
    Some(format!("{head}{sep}{}", refs.join("; ")))
}

/// The id of a context's part of the HTML page.
pub fn anchor(alias: &str) -> String {
    format!("ctx-{alias}")
}

/// The anchor GitHub gives a heading: lower case, spaces as `-`, and what is not a letter, a
/// digit, `-` or `_` left out.
pub fn slug(heading: &str) -> String {
    heading.chars().filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == ' ').map(|c| if c == ' ' { '-' } else { c }).collect::<String>().to_lowercase()
}

/// The heading of a context's part of the page, in a language.
pub fn context_heading(lang: Lang, n: &Node) -> String {
    named(lang, &n.name, &n.alias)
}

/// `Markdown` or `HTML`, the two forms `doc` writes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Format {
    Markdown,
    Html,
}

/// The page of a map that passed check, written in a form.
pub fn write(o: &Outcome, suite: &Suite, lang: Lang, format: Format) -> String {
    let p = page(o, suite, lang);
    match format {
        Format::Markdown => markdown::write(&p),
        Format::Html => html::write(&p),
    }
}
