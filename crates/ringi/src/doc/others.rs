//! The pages of the files that are not requests (DESIGN 11.1): a shared route (what it takes, its
//! stages, how their approvers are chosen and their deadlines; which requests use it is in the
//! requests, not in it), an organization (its positions and kinds of team, the versions of its data
//! with their days and people, and the proxies), and masters (each master's fields, and its rows
//! with the days each may be chosen on).

use super::request::{self, head};
use super::words::{Say, day_text, grouped};
use super::{Block, h2, h3, id_of};
use crate::check::{Outcome, Run};
use crate::org::{Data, Org, ProxyKind};

/// The page of a shared route. `project` is the root of the project it is in, when there is one:
/// the requests of the project that take it are listed.
pub fn route(o: &Outcome, run: &Run, say: &Say, shown: &str, sha: &str, project: Option<&std::path::Path>) -> Vec<Block> {
    let mut out = head(say, shown, sha);
    if o.usable.is_some() {
        request::usable(o, say, &mut out);
    }
    out.push(h2(say.t("What it takes", "受け取る物"), "inputs"));
    let inputs = &say.s.file.inputs;
    if inputs.is_empty() {
        out.push(Block::Para(say.t("It takes nothing: a request uses it as it is.", "受け取る物はありません。申請の種類は、そのまま使います。")));
    } else {
        out.push(Block::Para(say.t(
            "A request that takes the route gives it each of these, as a condition on its own form (`route manager_approval(long: days > 5)`):",
            "経路を使う申請の種類は、どれにも自分のフォームについての条件を渡します（`route manager_approval(long: days > 5)` の形）。",
        )));
        let rows = inputs.iter().map(|f| vec![format!("`{}`", f.name.text), say.ty(f, &[f.name.text.as_str()])]).collect();
        out.push(Block::Table { head: vec![say.t("Input", "受け取る物"), say.t("Type", "型")], rows });
    }
    request::route_section(say, &mut out);
    request::ways(say, &mut out);
    request::deadlines(o, run, say, &mut out);
    out.push(h2(say.t("The requests that take it", "使う申請の種類"), "requests"));
    match project.and_then(|root| takers(say, root, &o.path)) {
        None => out.push(Block::Para(say.t(
            "This file does not say which requests take the route: each request names the route it takes, on its `route` line. Their pages show the route with what each gives it.",
            "どの申請の種類がこの経路を使うかは、このファイルには書いてありません。使う経路は、申請の種類の `route` の行に書いてあります。その申請の種類のページが、渡す条件と一緒にこの経路を見せます。",
        ))),
        Some(rows) if rows.is_empty() => out.push(Block::Para(say.t("No request of the project takes the route.", "プロジェクトに、この経路を使う申請の種類はありません。"))),
        Some(rows) => {
            out.push(Block::Para(say.t(
                "The requests of the project that take the route, on their `route` line: a change of the route changes their tickets. Each request's page shows the route with what it gives it.",
                "プロジェクトの中で、`route` の行でこの経路を使う申請の種類です。経路を変えると、これらのチケットが変わります。渡す条件は、それぞれの申請の種類のページが経路と一緒に見せます。",
            )));
            out.push(Block::Table { head: vec![say.t("Request", "申請の種類"), say.t("File", "ファイル")], rows });
        }
    }
    out
}

/// The requests of the project under `root` whose `route` line takes the shared route at `path`,
/// each read as the project's index reads what a file uses ([`crate::ports::uses_of`]): its title in
/// the page's language and its name, and its file from the root. None when the route is not under
/// the root.
fn takers(say: &Say, root: &std::path::Path, path: &str) -> Option<Vec<Vec<String>>> {
    let me = ritsu_base::paths::from_root(root, std::path::Path::new(path))?;
    let how = format!("use {}", crate::ast::UseKind::Route.word());
    let mut rows = Vec::new();
    for rel in crate::ports::files_under(root) {
        let Ok(uses) = crate::ports::uses_of(root, &rel) else { continue };
        if !uses.iter().any(|u| u.how == how && u.target.path == me) {
            continue;
        }
        let Ok((f, _)) = crate::ports::parsed(root, &rel) else { continue };
        if f.kind != crate::ast::FileKind::Request {
            continue;
        }
        // named as the page of a request names it in its head: by its ASCII name
        let ascii = f.name.ascii();
        let name = match say.title_in(&f) {
            Some(t) => say.t(format!("{t} (`{ascii}`)"), format!("{t}（`{ascii}`）")),
            None => format!("`{ascii}`"),
        };
        rows.push(vec![name, format!("`{rel}`")]);
    }
    Some(rows)
}

/// The page of an organization.
pub fn organization(o: &Outcome, say: &Say, shown: &str, sha: &str) -> Vec<Block> {
    let f = say.s.file;
    let mut out = head(say, shown, sha);
    out.push(h2(say.t("Positions and kinds of team", "役職とチームの種類"), "shape"));
    let titled = |t: &crate::ast::Titled| match (&t.title, say.own(f)) {
        (Some(title), true) => say.t(format!("{title} (`{}`)", t.name.text), format!("{title}（`{}`）", t.name.text)),
        _ => format!("`{}`", t.name.text),
    };
    let positions: Vec<String> = f.positions.iter().map(titled).collect();
    let kinds: Vec<String> = f.team_kinds.iter().map(titled).collect();
    out.push(Block::List(vec![
        say.t(format!("Positions, from the lowest: {}.", positions.join(", ")), format!("役職（低い順）：{}", positions.join("、"))),
        say.t(format!("Kinds of team, from the widest: {}.", kinds.join(", ")), format!("チームの種類（広い順）：{}", kinds.join("、"))),
    ]));
    out.push(h2(say.t("Versions of the data", "データの版"), "versions"));
    let data = o.data.as_deref();
    let org = match data {
        Some(Data::Org(org)) => org,
        Some(Data::NoOrg(n)) => {
            let (line, fix) = (n.line.get(say.lang), n.fix.get(say.lang));
            out.push(Block::Note(say.t(format!("No data is read: {line}. {fix}"), format!("データを読んでいません。{line}。{fix}"))));
            return out;
        }
        _ => {
            out.push(Block::Para(say.t("The file names no data (`data`), and none was given (`--org`).", "ファイルにデータの置き場（`data`）が無く、`--org` も渡されていません。")));
            return out;
        }
    };
    versions(say, org, &mut out);
    proxies(say, org, &mut out);
    out
}

/// The versions of an organization's data: the days each is in effect, its teams and its people.
fn versions(say: &Say, org: &Org, out: &mut Vec<Block>) {
    let w = &org.written;
    let text = match org.at {
        Some(at) => {
            let at = crate::lex::date_text(at);
            say.t(format!("The data is read from `{w}`, in the version in effect on {at}."), format!("データは `{w}` から、{at} に効いている版を読んでいます。"))
        }
        None => say.t(format!("The data is read from `{w}`. Each version is in effect from its day until the day before the next version's."), format!("データは `{w}` から読んでいます。どの版も、その日から、次の版の日の前の日まで効きます。")),
    };
    out.push(Block::Para(text));
    let rows = org
        .versions
        .iter()
        .map(|v| {
            let (a, b) = org.period(v);
            let through = b.map(|b| day_text(b, say.lang)).unwrap_or_else(|| say.t("no end", "終わり無し"));
            vec![day_text(a, say.lang), through, grouped(v.teams.len() as i128), grouped(v.people.len() as i128)]
        })
        .collect();
    out.push(Block::Table { head: vec![say.t("From", "から"), say.t("Through", "まで"), say.t("Teams", "チーム"), say.t("People", "人")], rows });
    for v in &org.versions {
        teams(say, v, out);
        people(say, org, v, out);
    }
}

/// The people of a version, each in each team they are in: their name, the team, their position in
/// it, and whether it is the team they are mainly in (where their manager is found).
/// What an administrator checks a version by; the rest the data holds of a person (an address) is
/// not on the page.
fn people(say: &Say, org: &Org, v: &crate::org::Version, out: &mut Vec<Block>) {
    let day = day_text(v.from, say.lang);
    out.push(h3(say.t(format!("The people from {day}"), format!("{day}からの人")), id_of("people", &v.day())));
    let team = |id: &str| match v.team(id) {
        Some(t) => {
            let name = if say.ja() && !t.name_ja.is_empty() { &t.name_ja } else { &t.name };
            if name.is_empty() || name == id { format!("`{id}`") } else { say.t(format!("{name} (`{id}`)"), format!("{name}（`{id}`）")) }
        }
        None => format!("`{id}`"),
    };
    let mut rows = Vec::new();
    for p in &v.people {
        let of: Vec<&crate::org::Member> = v.members.iter().filter(|m| m.person == p.id).collect();
        let who = v.who(&p.id).get(say.lang).to_string();
        if of.is_empty() {
            rows.push(vec![who, "—".to_string(), "—".to_string(), "—".to_string()]);
            continue;
        }
        for m in of {
            let position = org.positions.get(m.position).map_or_else(|| "—".to_string(), |(name, _)| say.position(name));
            rows.push(vec![who.clone(), team(&m.team), position, if m.primary { say.t("yes", "はい") } else { "—".to_string() }]);
        }
    }
    out.push(Block::Table { head: vec![say.t("Person", "人"), say.t("Team", "チーム"), say.t("Position", "役職"), say.t("Mainly in it", "主に属する")], rows });
}

/// The teams of a version: each with its kind, the team above, its head and how many are in it; in
/// the order the data writes them.
fn teams(say: &Say, v: &crate::org::Version, out: &mut Vec<Block>) {
    let day = day_text(v.from, say.lang);
    out.push(h3(say.t(format!("The teams from {day}"), format!("{day}からのチーム")), id_of("teams", &v.day())));
    let team = |id: &str| match v.team(id) {
        Some(t) => {
            let name = if say.ja() && !t.name_ja.is_empty() { &t.name_ja } else { &t.name };
            if name.is_empty() || name == id { format!("`{id}`") } else { say.t(format!("{name} (`{id}`)"), format!("{name}（`{id}`）")) }
        }
        None => format!("`{id}`"),
    };
    let rows = v
        .teams
        .iter()
        .map(|t| {
            let members = v.members.iter().filter(|m| m.team == t.id).count();
            vec![
                team(&t.id),
                say.team_kind(&t.kind),
                t.parent.as_deref().map_or_else(|| "—".to_string(), team),
                t.head.as_deref().map_or_else(|| "—".to_string(), |h| v.who(h).get(say.lang).to_string()),
                grouped(members as i128),
            ]
        })
        .collect();
    out.push(Block::Table { head: vec![say.t("Team", "チーム"), say.t("Kind", "種類"), say.t("Above", "上のチーム"), say.t("Head", "長"), say.t("Members", "人数")], rows });
}

/// The proxies of an organization's data (DESIGN 5.8).
fn proxies(say: &Say, org: &Org, out: &mut Vec<Block>) {
    out.push(h2(say.t("Proxies", "代理"), "proxies"));
    if org.proxies.is_empty() {
        out.push(Block::Para(say.t("The data has no proxy.", "データに代理はありません。")));
        return;
    }
    // written from where the file writes the data, as the versions are, so that the page does not
    // change with where it is made from
    let at = org.proxies_file.as_ref().map(|_| format!("{}/proxies.csv", org.written.trim_end_matches('/')));
    let file = at.as_deref().map(|f| format!(" (`{f}`)")).unwrap_or_default();
    let file_ja = at.as_deref().map(|f| format!("（`{f}`）")).unwrap_or_default();
    out.push(Block::Para(say.t(
        format!("The proxies of the data{file}. A proxy for approving answers the stages of the person they stand in for, beside them; a proxy for submitting submits requests in that person's name. Whatever a proxy does is kept in the history with whose place they did it in."),
        format!("データにある代理です{file_ja}。承認の代理は、代わる人のステップに並んで答えます。申請の代理は、代わる人の名前で申請を出します。代理がしたことは、だれの代わりにしたかと一緒に、履歴に残ります。"),
    )));
    let rows = org
        .proxies
        .iter()
        .map(|p| {
            let kind = match p.kind {
                ProxyKind::Approve => say.t("approving", "承認"),
                ProxyKind::Submit => say.t("submitting", "申請"),
            };
            let requests = if p.requests.is_empty() { say.t("every request", "どの申請の種類も") } else { p.requests.iter().map(|r| format!("`{r}`")).collect::<Vec<_>>().join(if say.ja() { "、" } else { ", " }) };
            vec![say.person(&p.person), say.person(&p.proxy), kind, requests, p.days().get(say.lang).to_string()]
        })
        .collect();
    out.push(Block::Table { head: vec![say.t("Person", "代わられる人"), say.t("Proxy", "代わる人"), say.t("For", "種類"), say.t("Requests", "申請の種類"), say.t("Days", "期間")], rows });
}

/// The page of masters.
pub fn masters(o: &Outcome, say: &Say, shown: &str, sha: &str) -> Vec<Block> {
    let f = say.s.file;
    let mut out = head(say, shown, sha);
    let data = match o.data.as_deref() {
        Some(Data::Masters(m)) => Some(m),
        _ => None,
    };
    for m in &f.masters {
        let title = match (&m.title, say.own(f)) {
            (Some(t), true) => say.t(format!("{t} (`{}`)", m.name.text), format!("{t}（`{}`）", m.name.text)),
            _ => format!("`{}`", m.name.text),
        };
        out.push(h2(title, &id_of("master", m.name.ascii())));
        let titled = |x: &crate::ast::Field| match (&x.title, say.own(f)) {
            (Some(t), true) => say.t(format!("{t} (`{}`)", x.name.text), format!("{t}（`{}`）", x.name.text)),
            _ => format!("`{}`", x.name.text),
        };
        if let Some(k) = &m.key {
            let key = m.fields.iter().find(|x| x.name.text == k.text).map(titled).unwrap_or_else(|| format!("`{}`", k.text));
            out.push(Block::Para(say.t(format!("A row is named by {key}: a request that chooses a row keeps it."), format!("行を見分けるキーは {key} です。行を選んだ申請は、その値を持ちます。"))));
        }
        let rows = m
            .fields
            .iter()
            .map(|x| {
                let accepts = request::accepts(say, x);
                vec![titled(x), say.ty(x, &[x.name.text.as_str()]), accepts]
            })
            .collect();
        out.push(Block::Table { head: vec![say.t("Field", "フィールド"), say.t("Type", "型"), say.t("Accepts", "受け付ける値")], rows });
        let Some(md) = data.and_then(|d| d.master(&m.name.text)) else {
            out.push(Block::Para(say.t("No rows are read for it: it has no `data` line.", "行を読んでいません。`data` の行がありません。")));
            continue;
        };
        let n = md.rows.len();
        out.push(Block::Para(say.t(
            format!("{} {} from `{}`. A row may be chosen from its first day through its last.", grouped(n as i128), if n == 1 { "row" } else { "rows" }, md.written),
            format!("`{}` から {} 行を読んでいます。行は、始まりの日から終わりの日まで選べます。", md.written, grouped(n as i128)),
        )));
        let mut head: Vec<String> = m.fields.iter().map(titled).collect();
        head.push(say.t("From", "から"));
        head.push(say.t("Through", "まで"));
        let rows = md
            .rows
            .iter()
            .map(|r| {
                let mut cells: Vec<String> = m
                    .fields
                    .iter()
                    .map(|x| {
                        let v = md.value(r, &x.name.text).or_else(|| md.value(r, x.name.ascii())).unwrap_or("");
                        match x.ty {
                            crate::ast::TypeExpr::Person if !v.is_empty() => say.person(v),
                            _ => v.to_string(),
                        }
                    })
                    .collect();
                cells.push(r.from.map(|d| day_text(d, say.lang)).unwrap_or_else(|| say.t("any day", "いつでも")));
                cells.push(r.to.map(|d| day_text(d, say.lang)).unwrap_or_else(|| say.t("no end", "終わり無し")));
                cells
            })
            .collect();
        out.push(Block::Table { head, rows });
    }
    out
}
