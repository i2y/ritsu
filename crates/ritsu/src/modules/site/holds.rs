//! What a version of a module holds, file by file, as the pages of `ritsu mod site` show it
//! (DESIGN 17.17): the kind of each file, its name and its description (its first lines, which every
//! language writes alike), and what its language's `api` says of it, in a few facts.

use crate::modules::diff;
use ritsu_base::json::Json;
use ritsu_base::modules::lex;
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_project::Joined;

/// What a file is, to a reader of the site. The order is the order of the filters and the groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Kind {
    Calendar,
    Dates,
    Rule,
    Workflow,
    Book,
    Gate,
    Request,
    Route,
    Organization,
    Masters,
    Functions,
    Requirements,
    ContextMap,
    Claims,
    Contract,
}

impl Kind {
    /// The word for one of them.
    pub(super) fn one(self) -> Text {
        match self {
            Kind::Calendar => tr!("カレンダー", "calendar"),
            Kind::Dates => tr!("日付の計算", "date computation"),
            Kind::Rule => tr!("規則", "rule"),
            Kind::Workflow => tr!("ワークフロー", "workflow"),
            Kind::Book => tr!("帳簿", "book"),
            Kind::Gate => tr!("ゲート", "gate"),
            Kind::Request => tr!("申請の種類", "request type"),
            Kind::Route => tr!("共有の経路", "shared route"),
            Kind::Organization => tr!("組織", "organization"),
            Kind::Masters => tr!("マスタのファイル", "masters file"),
            Kind::Functions => tr!("kaname のモジュール", "kaname module"),
            Kind::Requirements => tr!("要件のファイル", "requirements file"),
            Kind::ContextMap => tr!("コンテキストの地図", "context map"),
            Kind::Claims => tr!("主張のファイル", "claims file"),
            Kind::Contract => tr!("protobuf の契約", "protobuf contract"),
        }
    }

    /// The word for some of them, in a heading or on a filter.
    pub(super) fn many(self) -> Text {
        match self {
            Kind::Calendar => tr!("カレンダー", "Calendars"),
            Kind::Dates => tr!("日付の計算", "Date computations"),
            Kind::Rule => tr!("規則", "Rules"),
            Kind::Workflow => tr!("ワークフロー", "Workflows"),
            Kind::Book => tr!("帳簿", "Books"),
            Kind::Gate => tr!("ゲート", "Gates"),
            Kind::Request => tr!("申請の種類", "Request types"),
            Kind::Route => tr!("共有の経路", "Shared routes"),
            Kind::Organization => tr!("組織", "Organizations"),
            Kind::Masters => tr!("マスタ", "Masters files"),
            Kind::Functions => tr!("kaname のモジュール", "kaname modules"),
            Kind::Requirements => tr!("要件", "Requirements"),
            Kind::ContextMap => tr!("コンテキストの地図", "Context maps"),
            Kind::Claims => tr!("主張", "Claims"),
            Kind::Contract => tr!("protobuf の契約", "Protobuf contracts"),
        }
    }

    /// How many of them, as a list line says it: `5 calendars`, `カレンダー 5 個`.
    pub(super) fn counted(self, n: usize) -> Text {
        let (one, many) = (self.one(), self.many());
        let en = if n == 1 { one.en } else { many.en.to_lowercase() };
        let unit = match self {
            Kind::Rule | Kind::Workflow | Kind::Dates => "本",
            Kind::Book | Kind::Calendar | Kind::Functions | Kind::Gate | Kind::Contract => "個",
            _ => "つ",
        };
        let ja = many.ja;
        tr!("{ja} {n} {unit}", "{n} {en}")
    }

    /// The word a filter carries in the page's data.
    pub(super) fn word(self) -> &'static str {
        match self {
            Kind::Calendar => "calendar",
            Kind::Dates => "dates",
            Kind::Rule => "rule",
            Kind::Workflow => "workflow",
            Kind::Book => "book",
            Kind::Gate => "gate",
            Kind::Request => "request",
            Kind::Route => "route",
            Kind::Organization => "organization",
            Kind::Masters => "masters",
            Kind::Functions => "functions",
            Kind::Requirements => "requirements",
            Kind::ContextMap => "contexts",
            Kind::Claims => "claims",
            Kind::Contract => "contract",
        }
    }
}

/// One fact of a file: what is said, and how it reads.
pub(super) enum Value {
    Text(String),
    /// Written as code.
    Code(String),
    /// Some names, each written as code.
    Names(Vec<String>),
    Link(String),
}

/// A source of data a file pins by its digest.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Pinned {
    pub file: String,
    pub url: Option<String>,
    pub sha256: String,
}

/// One file a module holds.
pub(super) struct Held {
    pub rel: String,
    pub kind: Kind,
    pub name: String,
    pub description: Option<String>,
    pub facts: Vec<(Text, Value)>,
    /// The days a calendar knows, the first and the last.
    pub known: Option<(String, String)>,
    pub pinned: Vec<Pinned>,
    /// Whether its page has a section to try it in.
    pub can_try: bool,
    /// Whether its language has a page of it.
    pub has_page: bool,
}

/// The name and the description a file's first lines give: `rule shipping_fee v1`, then its
/// `description "…"` line. The name is None for a file whose first line names nothing.
fn head_of(text: &str) -> (Option<String>, Option<String>) {
    let mut name = None;
    let mut description = None;
    let mut seen = 0;
    for src in text.lines() {
        let t = src.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with("//") {
            continue;
        }
        seen += 1;
        if seen == 1 {
            let mut words = t.split_whitespace();
            let _ = words.next();
            name = words.next().map(|w| w.split('(').next().unwrap_or(w).to_string());
        }
        if t.starts_with("description") {
            description = lex::line(t).ok().and_then(|l| l.tokens.get(1).map(|x| x.text.clone()));
            break;
        }
        if seen > 12 {
            break;
        }
    }
    (name, description)
}

/// A number the way the site writes it: its thousands apart when it is a whole number.
pub(super) fn number(s: &str) -> String {
    match s.parse::<u64>() {
        Ok(n) => ritsu_base::text::count(n),
        Err(_) => s.to_string(),
    }
}

/// The range an input or an output of a rule says: `1 to 30,000 g`.
fn range_of(v: &Json) -> Option<Text> {
    let unit = v.get("unit").and_then(Json::as_str).unwrap_or("");
    let r = v.get("range")?;
    let end = |k: &str| r.get(k).and_then(|x| x.as_int().map(|n| number(&n.to_string())).or_else(|| x.as_str().map(number)));
    let (lo, hi) = (end("min")?, end("max")?);
    let u = if unit.is_empty() { String::new() } else { format!(" {unit}") };
    Some(tr!("{lo}〜{hi}{u}", "{lo} to {hi}{u}"))
}

/// The inputs and the outputs of a rule, from `rulec api`.
fn rule_facts(api: &Json, lang: Lang) -> Vec<(Text, Value)> {
    let Some(py) = api.get("python") else { return Vec::new() };
    let enums = py.get("enums").and_then(Json::as_arr).unwrap_or(&[]);
    let read = |v: &Json| -> String {
        let name = v.get("name").and_then(Json::as_str).unwrap_or("");
        if let Some(r) = range_of(v) {
            return format!("{name} ({})", r.get(lang));
        }
        let ty = v.get("type").and_then(Json::as_str).unwrap_or("");
        let e = enums.iter().find(|e| e.get("alias").and_then(Json::as_str) == Some(ty) || e.get("name").and_then(Json::as_str) == Some(name));
        if let Some(values) = e.and_then(|e| e.get("values")).and_then(Json::as_arr) {
            let vs: Vec<&str> = values.iter().filter_map(|x| x.get("name").and_then(Json::as_str)).collect();
            return format!("{name} ({})", vs.join(", "));
        }
        let unit = v.get("unit").and_then(Json::as_str).unwrap_or("");
        if unit.is_empty() { name.to_string() } else { format!("{name} ({unit})") }
    };
    let list = |k: &str| -> Vec<String> { py.get(k).and_then(Json::as_arr).unwrap_or(&[]).iter().map(&read).collect() };
    let mut out = Vec::new();
    let (ins, outs) = (list("params"), list("outputs"));
    if !ins.is_empty() {
        out.push((tr!("入力", "Inputs"), Value::Text(ins.join("; "))));
    }
    if !outs.is_empty() {
        out.push((tr!("出力", "Outputs"), Value::Text(outs.join("; "))));
    }
    out
}

/// A file's path in the module, from the directory of the file that names it: a path of a dependency
/// (`@holidays/…`) is its own.
fn beside(dir: &str, file: &str) -> String {
    if dir.is_empty() || file.is_empty() || file.starts_with('@') { file.to_string() } else { format!("{dir}/{file}") }
}

/// What a rule cites and the copies it pins, from `rulec api`.
fn rule_sources(api: &Json, lang: Lang, held: &mut Held) {
    let dir = held.rel.rsplit_once('/').map_or("", |(d, _)| d).to_string();
    for s in api.get("sources").and_then(Json::as_arr).unwrap_or(&[]) {
        let text = |k: &str| s.get(k).and_then(Json::as_str).unwrap_or("");
        if text("kind") == "law" {
            let (id, asof) = (text("id"), text("asof"));
            held.facts.push((tr!("出典", "Source"), Value::Text(say(&tr!("{id}（{asof} 時点）", "{id}, as of {asof}"), lang))));
            let Some(db) = ritsu_base::sources::LawDb::from_word(text("db")) else { continue };
            for p in s.get("pins").and_then(Json::as_arr).unwrap_or(&[]) {
                let of = |k: &str| p.get(k).and_then(Json::as_str);
                let (Some(fragment), Some(sha)) = (of("fragment"), of("sha256")) else { continue };
                let Some(file) = ritsu_base::sources::fragment_file(db, fragment) else { continue };
                let file = format!("{}/{file}", ritsu_base::sources::copy_dir(id, asof));
                held.pinned.push(Pinned { file: beside(&dir, &file), url: None, sha256: sha.to_string() });
            }
        } else if !text("path").is_empty() {
            let (path, url) = (text("path"), text("url"));
            held.facts.push((tr!("出典", "Source"), if url.is_empty() { Value::Code(path.to_string()) } else { Value::Link(url.to_string()) }));
            held.pinned.push(Pinned { file: beside(&dir, path), url: Some(url.to_string()).filter(|u| !u.is_empty()), sha256: text("sha256").to_string() });
        }
    }
}

/// A calendar's days, tables and sources, from `koyomi api`.
fn calendar_facts(api: &Json, lang: Lang, held: &mut Held) {
    let Some(cal) = api.get("calendar") else { return };
    if let Some(r) = cal.get("data_range")
        && let (Some(a), Some(z)) = (r.get("from").and_then(Json::as_str), r.get("to").and_then(Json::as_str))
    {
        held.known = Some((a.to_string(), z.to_string()));
        held.facts.push((tr!("知っている日", "Knows"), Value::Code(say(&tr!("{a} 〜 {z}", "{a} to {z}"), lang))));
    }
    if let Some(off) = cal.get("offset").and_then(Json::as_str) {
        held.facts.push((tr!("UTC オフセット", "UTC offset"), Value::Code(off.to_string())));
    }
    let dir = held.rel.rsplit_once('/').map_or("", |(d, _)| d).to_string();
    for s in cal.get("sources").and_then(Json::as_arr).unwrap_or(&[]) {
        let file = s.get("file").and_then(Json::as_str).unwrap_or("");
        let rows = s.get("rows").and_then(Json::as_int).unwrap_or(0);
        let rows_said = number(&rows.to_string());
        if !file.is_empty() {
            held.facts.push((tr!("表", "Table"), Value::Text(say(&tr!("{file} の {rows_said} 行", "{rows_said} rows of {file}"), lang))));
        }
        let url = s.get("url").and_then(Json::as_str);
        if let Some(u) = url {
            held.facts.push((tr!("出典", "Source"), Value::Link(u.to_string())));
        }
        if let Some(sha) = s.get("sha256").and_then(Json::as_str) {
            held.facts.push((tr!("固定した版", "Pinned at"), Value::Code(format!("sha256:{sha}"))));
            held.pinned.push(Pinned { file: beside(&dir, file), url: url.map(str::to_string), sha256: sha.to_string() });
        }
    }
    let uses: Vec<String> = cal.get("uses").and_then(Json::as_arr).unwrap_or(&[]).iter().filter_map(|u| u.get("name").and_then(Json::as_str).map(str::to_string)).collect();
    if !uses.is_empty() {
        held.facts.push((tr!("読むカレンダー", "Reads"), Value::Names(uses)));
    }
}

/// The names under `key` of an `api` JSON's list.
fn names_of(api: &Json, key: &str) -> Vec<String> {
    api.get(key).and_then(Json::as_arr).unwrap_or(&[]).iter().filter_map(|x| x.get("name").and_then(Json::as_str).map(str::to_string)).collect()
}

fn say(t: &Text, lang: Lang) -> String {
    t.get(lang).to_string()
}

/// What `ringi api` says of a `.ringi`: the department that owns a request, the shared route it
/// takes, the first day it can be used on, and the names its kind of file declares (the stages of a
/// route, the positions and kinds of team of an organization, the masters of a masters file), each as
/// the file writes it.
fn request_facts(api: &Json, held: &mut Held) {
    if let Some(owner) = api.get("owner").and_then(Json::as_str) {
        held.facts.push((tr!("受け持つ部門", "Owned by"), Value::Code(owner.to_string())));
    }
    let routes: Vec<String> = api
        .get("uses")
        .and_then(Json::as_arr)
        .unwrap_or(&[])
        .iter()
        .filter(|u| u.get("kind").and_then(Json::as_str) == Some("route"))
        .filter_map(|u| u.get("file").and_then(Json::as_str).map(str::to_string))
        .collect();
    if !routes.is_empty() {
        held.facts.push((tr!("共有の経路", "Shared route"), Value::Names(routes)));
    }
    let items = api.get("items").and_then(Json::as_arr).unwrap_or(&[]);
    let written = |kind: &str| -> Vec<String> {
        items.iter().filter(|i| i.get("kind").and_then(Json::as_str) == Some(kind)).filter_map(|i| i.get("written").or_else(|| i.get("name")).and_then(Json::as_str).map(str::to_string)).collect()
    };
    for (kind, label) in [("stage", tr!("ステップ", "Stages")), ("position", tr!("役職", "Positions")), ("team_kind", tr!("チームの種類", "Kinds of team")), ("master", tr!("マスタ", "Masters"))] {
        let ns = written(kind);
        if !ns.is_empty() {
            held.facts.push((label, Value::Names(ns)));
        }
    }
    if let Some(from) = api.get("usable").and_then(|u| u.get("from")).and_then(Json::as_str) {
        held.facts.push((tr!("使える最初の日", "Usable from"), Value::Code(from.to_string())));
    }
}

/// What a file is, from its language and what its `api` says.
fn kind_of(tool: Tool, api: Option<&Json>) -> Option<Kind> {
    Some(match tool {
        Tool::Rulec => Kind::Rule,
        Tool::Koyomi => {
            if api.and_then(|a| a.get("kind")).and_then(Json::as_str) == Some("dates") {
                Kind::Dates
            } else {
                Kind::Calendar
            }
        }
        Tool::Dandori => Kind::Workflow,
        Tool::Chobo => Kind::Book,
        Tool::Sekisho => Kind::Gate,
        // the first word of a `.ringi` says its kind; a file that does not pass is read as a request
        Tool::Ringi => match api.and_then(|a| a.get("kind")).and_then(Json::as_str) {
            Some("route") => Kind::Route,
            Some("organization") => Kind::Organization,
            Some("masters") => Kind::Masters,
            _ => Kind::Request,
        },
        Tool::Kaname => Kind::Functions,
        Tool::Yuen => Kind::Requirements,
        Tool::Sakai => Kind::ContextMap,
        Tool::Geas => Kind::Claims,
        Tool::Proto => Kind::Contract,
        _ => return None,
    })
}

/// What the file `rel` of the module `m` holds, for a file in one of the languages; None for any
/// other. What its language cannot say of it (a file that does not pass its check, say) is left
/// out: the page of the file and the module's `ritsu check` say why.
pub(super) fn held(m: &diff::Module, rel: &str, lang: Lang, joined: &Joined) -> Option<Held> {
    let tool = ritsu_project::project::kind_of(rel)?;
    let text = m.files.get(rel).map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
    let (name, description) = head_of(&text);
    let api = diff::api_of(tool, &m.root.join(rel), rel, joined).ok();
    let kind = kind_of(tool, api.as_ref())?;
    let stem = rel.rsplit('/').next().unwrap_or(rel).split('.').next().unwrap_or(rel).to_string();
    let mut h = Held {
        rel: rel.to_string(),
        kind,
        name: if matches!(tool, Tool::Proto | Tool::Geas) { stem.clone() } else { name.unwrap_or(stem) },
        description,
        facts: Vec::new(),
        known: None,
        pinned: Vec::new(),
        can_try: false,
        has_page: false,
    };
    if let Some(api) = &api {
        match tool {
            Tool::Rulec => {
                h.facts = rule_facts(api, lang);
                rule_sources(api, lang, &mut h);
            }
            Tool::Koyomi => calendar_facts(api, lang, &mut h),
            Tool::Chobo => {
                for (key, label) in [("accounts", tr!("勘定", "Accounts")), ("transfers", tr!("振替", "Transfers"))] {
                    let ns = names_of(api, key);
                    if !ns.is_empty() {
                        h.facts.push((label, Value::Names(ns)));
                    }
                }
            }
            Tool::Sekisho => {
                for (key, label) in [("roles", tr!("役割", "Roles")), ("actions", tr!("操作", "Actions"))] {
                    let ns = names_of(api, key);
                    if !ns.is_empty() {
                        h.facts.push((label, Value::Names(ns)));
                    }
                }
            }
            Tool::Kaname => {
                let fs: Vec<String> = api.get("wasm").and_then(|w| w.get("functions")).and_then(Json::as_arr).unwrap_or(&[]).iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
                if !fs.is_empty() {
                    h.facts.push((tr!("関数", "Functions"), Value::Names(fs)));
                }
            }
            Tool::Yuen => {
                let ns = names_of(api, "requirements");
                if !ns.is_empty() {
                    h.facts.push((tr!("要件", "Requirements"), Value::Names(ns)));
                }
            }
            Tool::Ringi => request_facts(api, &mut h),
            _ => {}
        }
    }
    Some(h)
}

/// The kinds a module holds, each with how many, in the order of [`Kind`].
pub(super) fn tally(holds: &[Held]) -> Vec<(Kind, usize)> {
    let mut out: Vec<(Kind, usize)> = Vec::new();
    for h in holds {
        match out.iter_mut().find(|(k, _)| *k == h.kind) {
            Some((_, n)) => *n += 1,
            None => out.push((h.kind, 1)),
        }
    }
    out.sort();
    out
}

/// The sources of data a module pins, each once.
pub(super) fn pinned(holds: &[Held]) -> Vec<Pinned> {
    let mut out: Vec<Pinned> = holds.iter().flat_map(|h| h.pinned.iter().cloned()).collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_head_of_a_file() {
        assert_eq!(
            head_of("# A comment.\n\nrule shipping_fee v1\ndescription \"The fee\"\n\ninputs\n"),
            (Some("shipping_fee".to_string()), Some("The fee".to_string()))
        );
        assert_eq!(head_of("map Shop(shop) v1\ndescription \"A \\\"small\\\" shop\"\n"), (Some("Shop".to_string()), Some("A \"small\" shop".to_string())));
        assert_eq!(head_of("calendar 国民の祝日 v1\n"), (Some("国民の祝日".to_string()), None));
        assert_eq!(head_of(""), (None, None));
    }

    #[test]
    fn how_many_of_a_kind() {
        assert_eq!(Kind::Calendar.counted(5).en, "5 calendars");
        assert_eq!(Kind::Rule.counted(1).en, "1 rule");
        assert_eq!(Kind::Rule.counted(2).ja, "規則 2 本");
        assert_eq!(Kind::Calendar.counted(5).ja, "カレンダー 5 個");
        assert_eq!(Kind::Request.counted(2).en, "2 request types");
        assert_eq!(Kind::Request.counted(2).ja, "申請の種類 2 つ");
        assert_eq!(Kind::Route.counted(1).en, "1 shared route");
        assert_eq!(number("30000"), "30,000");
        assert_eq!(number("2.5"), "2.5");
    }
}
