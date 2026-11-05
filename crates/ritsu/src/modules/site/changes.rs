//! The tab of what changed since the version before (DESIGN 17.17): what `ritsu mod diff` found, said
//! first in a sentence, then in three tiles, then file by file: a table of what the two versions
//! answer when the rule is small enough to have one, and the regions where they differ. The body of
//! `ritsu mod diff --format markdown` stays under it, whole, as the same report on a machine reads.

use super::holds::number;
use super::markdown::md_html;
use crate::modules::diff::{self, Behavior, How, ModDiff};
use ritsu_base::docpage::esc;
use ritsu_base::json::Json;
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use rulec::vdiff::{Map, MapCoord, MapSide};

/// The most lines of a difference the page shows of one table (as `ritsu mod diff`).
const ROWS: usize = 200;

/// How many of each language's files were compared and how many of them answer differently.
fn differing(d: &ModDiff) -> Vec<&Behavior> {
    d.behavior.iter().filter(|b| b.result.as_ref().is_ok_and(|s| s.differs)).collect()
}

/// A file's name in a sentence: the name its language's diff gives it, else its path.
fn name_of(b: &Behavior) -> String {
    let Ok(s) = &b.result else { return b.file.clone() };
    if let Some(n) = s.json.get("rule").and_then(Json::as_str) {
        return n.to_string();
    }
    if let Some(n) = s.json.get("new").and_then(|h| h.get("name")).and_then(Json::as_str) {
        return n.to_string();
    }
    b.file.rsplit('/').next().unwrap_or(&b.file).split('.').next().unwrap_or(&b.file).to_string()
}

/// How many places a rule's answers differ in, or how many days of a calendar change.
fn places_of(b: &Behavior) -> Option<i128> {
    let s = b.result.as_ref().ok()?;
    match b.tool {
        Tool::Rulec => s.json.get("changes").and_then(Json::as_arr).map(|c| c.len() as i128),
        Tool::Koyomi => s.json.get("total").and_then(|t| t.get("changed_days")).and_then(Json::as_int),
        _ => None,
    }
}

/// The words for a set of compared files: rules, calendars, gates, or files when they are mixed.
fn word_of(d: &ModDiff) -> Text {
    let tools: Vec<Tool> = d.behavior.iter().map(|b| b.tool).collect();
    let all = |t: Tool| tools.iter().all(|x| *x == t);
    let one = d.behavior.len() == 1;
    if all(Tool::Rulec) {
        if one { tr!("規則", "rule") } else { tr!("規則", "rules") }
    } else if all(Tool::Koyomi) {
        if one { tr!("カレンダー", "calendar") } else { tr!("カレンダー", "calendars") }
    } else if all(Tool::Sekisho) {
        if one { tr!("ゲート", "gate") } else { tr!("ゲート", "gates") }
    } else if one {
        tr!("ファイル", "file")
    } else {
        tr!("ファイル", "files")
    }
}

/// One change of an interface as the page lists it: the places that say the same for each output,
/// put together.
struct Grouped {
    how: How,
    may_break: bool,
    /// The place, after the output.
    place: String,
    /// The outputs it holds for; none for what a file says of itself.
    outputs: Vec<String>,
    /// The values, when each is a word or a number.
    old: Option<String>,
    new: Option<String>,
}

/// The changes of a file's interface, as the page lists and counts them: a change to every output
/// of a rule is one change, not one for each language, and Go's `output_fields` are the `outputs` of
/// the others.
fn grouped(i: &diff::Iface) -> Vec<Grouped> {
    let short = |v: &Option<Json>| {
        v.as_ref().and_then(|j| match j.as_str() {
            Some(s) => Some(s.to_string()),
            None => Some(j.compact()).filter(|t| !t.starts_with(['{', '['])),
        })
    };
    let mut out: Vec<Grouped> = Vec::new();
    for c in &i.changes {
        // `/python/enums/region/values/eu`: the part after the output, with the outputs it holds for
        let at = c.at.trim_start_matches('/');
        let (head, rest) = match at.split_once('/') {
            Some(("sources", _)) | None => ("", at),
            Some(parts) => parts,
        };
        let place = match rest.strip_prefix("output_fields/") {
            Some(tail) if head == "go" => format!("outputs/{tail}"),
            _ => rest.to_string(),
        };
        let (old, new) = (short(&c.old), short(&c.new));
        let same = |g: &&mut Grouped| g.how == c.how && g.place == place && g.old == old && g.new == new;
        let group = match out.iter_mut().find(same) {
            Some(g) => g,
            None => {
                out.push(Grouped { how: c.how, may_break: c.may_break(), place, outputs: Vec::new(), old, new });
                out.last_mut().expect("just pushed")
            }
        };
        if !head.is_empty() {
            group.outputs.push(head.to_string());
        }
    }
    out
}

/// How many changes the interface has, and how many of them may break a user.
fn interface_counts(d: &ModDiff) -> (usize, usize) {
    let changes = d.interface.iter().map(|i| grouped(i).len()).sum();
    let breaks = d.interface.iter().map(|i| grouped(i).iter().filter(|g| g.may_break).count() + i.breaks.len()).sum();
    (changes, breaks)
}

/// The sentence at the top: what changes, for whoever depends on the module.
fn headline(d: &ModDiff) -> Text {
    let moved = differing(d);
    let whole = d.whole();
    let rest = if whole {
        tr!("両方の版が受け取る入力のうち、それ以外は同じ答えです。", "Of the inputs both versions accept, everything else gets the same answer.")
    } else {
        tr!("調べきれなかった入力があります。下に書きました。", "Some inputs could not be settled, as it says below.")
    };
    let (changes, breaks) = interface_counts(d);
    match moved.as_slice() {
        [] if d.behavior.is_empty() && changes > 0 => {
            let w = if changes == 1 { "change" } else { "changes" };
            match breaks {
                0 => tr!("インターフェースの変更が {changes} 件あります。使う側を壊すものはありません。", "{changes} {w} to the interface, none that may break a user."),
                _ => tr!("インターフェースの変更が {changes} 件あり、そのうち {breaks} 件は使う側を壊しえます。", "{changes} {w} to the interface, {breaks} that may break a user."),
            }
        }
        [] if d.behavior.is_empty() => tr!("答えを出すファイルは変わっていません。", "No file that gives answers changed."),
        [] if whole => tr!("どの答えも変わりません。", "Every answer stays the same."),
        [] => tr!("違いは見つかりませんでしたが、調べきれなかった入力があります。", "No difference was shown, but some inputs could not be settled."),
        [b] => {
            let name = name_of(b);
            let first = match (b.tool, places_of(b)) {
                (Tool::Rulec, Some(0)) => tr!("{name} が受け取る入力か出力の形が変わります。", "What {name} accepts or returns changes."),
                (Tool::Rulec, Some(1)) => tr!("{name} の答えが 1 か所で変わります。", "{name} answers differently in one place."),
                (Tool::Rulec, Some(n)) => tr!("{name} の答えが {n} か所で変わります。", "{name} answers differently in {n} places."),
                (Tool::Koyomi, Some(0)) => tr!("{name} の知っている日の範囲か名前が変わります。", "The days {name} knows, or their names, change."),
                (Tool::Koyomi, Some(1)) => tr!("{name} の 1 日が変わります。", "One day of {name} changes."),
                (Tool::Koyomi, Some(n)) => {
                    let n = number(&n.to_string());
                    tr!("{name} の {n} 日が変わります。", "{n} days of {name} change.")
                }
                (Tool::Sekisho, _) => tr!("{name} で、だれが何をしてよいかが変わります。", "Who may do what changes in {name}."),
                _ => tr!("{name} の答えが変わります。", "{name} answers differently."),
            };
            first.then(&tr!("", " ")).then(&rest)
        }
        many => {
            let n = many.len();
            let names = many.iter().map(|b| name_of(b)).collect::<Vec<_>>();
            let names_ja = names.join("、");
            let names_en = names.join(", ");
            tr!("{n} つのファイルで答えが変わります（{names_ja}）。", "{n} files answer differently: {names_en}.").then(&tr!("", " ")).then(&rest)
        }
    }
}

/// The line of a module on the list: what the newest version changed.
pub(super) fn latest_line(d: &ModDiff, version: &str) -> Text {
    let moved = differing(d);
    let (changes, _) = interface_counts(d);
    match moved.as_slice() {
        [] if d.behavior.is_empty() && changes > 0 => {
            let w = if changes == 1 { "place" } else { "places" };
            tr!("{version} でインターフェースが {changes} か所変わります", "{version} changes the interface in {changes} {w}")
        }
        [] if d.behavior.is_empty() => tr!("{version} で答えを出すファイルは変わっていません", "{version} changes no file that gives answers"),
        [] => tr!("{version} でどの答えも変わりません", "{version} keeps every answer"),
        [b] => {
            let name = name_of(b);
            match (b.tool, places_of(b)) {
                (Tool::Rulec, Some(n)) if n > 0 => {
                    let w = if n == 1 { "place" } else { "places" };
                    tr!("{version} で {name} の答えが {n} か所で変わります", "{version} changes the answers of {name} in {n} {w}")
                }
                (Tool::Koyomi, Some(n)) if n > 0 => {
                    let w = if n == 1 { "day" } else { "days" };
                    let n = number(&n.to_string());
                    tr!("{version} で {name} の {n} 日が変わります", "{version} changes {n} {w} of {name}")
                }
                _ => tr!("{version} で {name} が変わります", "{version} changes {name}"),
            }
        }
        many => {
            let n = many.len();
            tr!("{version} で {n} つのファイルの答えが変わります", "{version} changes the answers of {n} files")
        }
    }
}

/// The three tiles under the sentence.
fn tiles(d: &ModDiff, lang: Lang) -> String {
    let moved = differing(d).len();
    let total = d.behavior.len();
    let word = word_of(d);
    let verb = if moved == 1 { "answers" } else { "answer" };
    let (wj, we) = (word.ja, word.en);
    let unsettled = d.behavior.iter().filter(|b| b.result.as_ref().map_or(true, |s| !s.total)).count();
    let behavior = if total == 0 {
        tr!("比べるものはありません", "Nothing to compare")
    } else if unsettled > 0 {
        tr!("{wj} {total} のうち {moved} で答えが変わります。{unsettled} は調べきれませんでした", "{moved} of {total} {we} {verb} differently; {unsettled} not settled")
    } else {
        tr!("{wj} {total} のうち {moved} で答えが変わります", "{moved} of {total} {we} {verb} differently")
    };
    let (changes, breaks) = interface_counts(d);
    let w = if changes == 1 { "change" } else { "changes" };
    let interface = if changes == 0 && breaks == 0 {
        tr!("変わりません", "No change")
    } else {
        tr!("変更 {changes} 件、そのうち使う側を壊しうるもの {breaks} 件", "{changes} {w}, {breaks} of them may break a user")
    };
    let f = &d.files;
    let (changed, same) = (f.changed.len(), f.same);
    let mut parts: Vec<Text> = Vec::new();
    if !f.added.is_empty() {
        let n = f.added.len();
        parts.push(tr!("{n} 増えた", "{n} added"));
    }
    if !f.removed.is_empty() {
        let n = f.removed.len();
        parts.push(tr!("{n} 消えた", "{n} removed"));
    }
    parts.push(tr!("{changed} 変わった", "{changed} changed"));
    parts.push(tr!("{same} 同じ", "{same} the same"));
    let list = Text::join(&parts, "、", ", ");
    let req = &d.requires;
    let moved_req = req.added.len() + req.removed.len() + req.changed.len();
    let require = if moved_req == 0 { tr!("require は変わりません", "require: no change") } else { tr!("require は {moved_req} 行変わります", "require: {moved_req} changed") };
    let files = list.then(&tr!("。", ". ")).then(&require);
    let tile = |label: Text, v: Text| format!("<div><dt>{}</dt><dd>{}</dd></div>\n", esc(label.get(lang)), esc(v.get(lang)));
    format!(
        "<dl class=\"tiles\">\n{}{}{}</dl>\n",
        tile(tr!("振る舞い", "Behavior"), behavior),
        tile(tr!("インターフェース", "Interface"), interface),
        tile(tr!("ファイル", "Files"), files)
    )
}

/// A row's or a column's head in a map.
fn coord_text(c: &MapCoord, unit: &str, date: bool, lang: Lang) -> String {
    match c {
        MapCoord::Word(w, _) => esc(w),
        MapCoord::Band(a, b) => {
            let n = |s: &String| if date { s.clone() } else { number(s) };
            let u = if unit.is_empty() { String::new() } else { format!(" {unit}") };
            let t = match (a, b) {
                (Some(a), Some(b)) if a == b => format!("{}{u}", n(a)),
                (Some(a), Some(b)) => {
                    let (x, y) = (n(a), n(b));
                    tr!("{x}〜{y}{u}", "{x} to {y}{u}").get(lang).to_string()
                }
                (Some(a), None) => {
                    let x = n(a);
                    tr!("{x}{u} 以上", "{x}{u} or more").get(lang).to_string()
                }
                (None, Some(b)) => {
                    let y = n(b);
                    tr!("{y}{u} 以下", "{y}{u} or less").get(lang).to_string()
                }
                (None, None) => tr!("すべて", "all").get(lang).to_string(),
            };
            esc(&t)
        }
    }
}

/// The note a row or a column of a value only one version takes carries.
fn side_note(c: &MapCoord, lang: Lang) -> String {
    match c {
        MapCoord::Word(_, MapSide::NewOnly) => format!(" <span class=\"note\">{}</span>", esc(tr!("（新しい値）", "(new)").get(lang))),
        MapCoord::Word(_, MapSide::OldOnly) => format!(" <span class=\"note\">{}</span>", esc(tr!("（消えた値）", "(removed)").get(lang))),
        _ => String::new(),
    }
}

/// What a version answers in a cell of a map: the value, or each output and its value.
fn answer_text(a: &[(String, String)]) -> String {
    if a.len() == 1 {
        return esc(&number(&a[0].1));
    }
    a.iter().map(|(k, v)| format!("{}: {}", esc(k), esc(&number(v)))).collect::<Vec<_>>().join("<br>")
}

/// What a cell of a map says of two versions that answer differently: where it holds more than one
/// output, each on a line, with only those that moved marked.
fn moved_text(a: &[(String, String)], b: &[(String, String)]) -> String {
    if a.len() == 1 && b.len() == 1 {
        return format!("<s>{}</s> {}", answer_text(a), answer_text(b));
    }
    b.iter()
        .map(|(k, v)| match a.iter().find(|(x, _)| x == k) {
            Some((_, old)) if old != v => format!("{}: <s>{}</s> {}", esc(k), esc(&number(old)), esc(&number(v))),
            _ => format!("<span class=\"same\">{}: {}</span>", esc(k), esc(&number(v))),
        })
        .collect::<Vec<_>>()
        .join("<br>")
}

/// The table of a rule of one input: a row for each of its values and a column for each output, so
/// that each column reads down and a row says which of its answers moved.
fn by_output_html(m: &Map, lang: Lang) -> String {
    let mut o = String::from("<div class=\"scroll\"><table class=\"map\">\n<thead><tr>");
    o.push_str(&format!("<th scope=\"col\">{}</th>", esc(&m.cols.col)));
    for out in &m.outputs {
        let unit = out.unit.as_ref().map_or(String::new(), |u| format!(" <span class=\"note\">{}</span>", esc(u)));
        o.push_str(&format!("<th scope=\"col\">{}{unit}</th>", esc(&out.name)));
    }
    o.push_str("</tr></thead>\n<tbody>\n");
    let cells = m.cells.first().map_or(&[][..], Vec::as_slice);
    for (c, cell) in m.cols.coords.iter().zip(cells) {
        o.push_str(&format!("<tr><th scope=\"row\">{}{}</th>", coord_text(c, &m.cols.unit, m.cols.date, lang), side_note(c, lang)));
        for out in &m.outputs {
            // `-` is what a version without the output says
            let of = |a: &Option<Vec<(String, String)>>| a.as_ref().and_then(|a| a.iter().find(|(k, v)| *k == out.name && v != "-")).map(|(_, v)| esc(&number(v)));
            let html = match (of(&cell.old), of(&cell.new)) {
                (Some(a), Some(b)) if cell.moved && a != b => format!("<td class=\"chg\"><s>{a}</s> {b}</td>"),
                (Some(_), Some(b)) => format!("<td>{b}</td>"),
                (None, Some(b)) => format!("<td class=\"new\">{b}</td>"),
                (Some(a), None) => format!("<td class=\"gone\"><s>{a}</s></td>"),
                (None, None) => "<td class=\"none\">—</td>".to_string(),
            };
            o.push_str(&html);
        }
        o.push_str("</tr>\n");
    }
    o.push_str("</tbody>\n</table></div>\n");
    o
}

/// The table of what the two versions answer.
fn map_html(m: &Map, lang: Lang) -> String {
    let Some(rows) = &m.rows else { return by_output_html(m, lang) };
    let mut o = String::from("<div class=\"scroll\"><table class=\"map\">\n<thead><tr>");
    o.push_str(&format!("<th scope=\"col\">{}</th>", esc(&rows.col)));
    for c in &m.cols.coords {
        o.push_str(&format!("<th scope=\"col\">{}{}</th>", coord_text(c, &m.cols.unit, m.cols.date, lang), side_note(c, lang)));
    }
    o.push_str("</tr></thead>\n<tbody>\n");
    for (y, row) in m.cells.iter().enumerate() {
        o.push_str(&format!("<tr><th scope=\"row\">{}{}</th>", coord_text(&rows.coords[y], &rows.unit, rows.date, lang), side_note(&rows.coords[y], lang)));
        for cell in row {
            let html = match (&cell.old, &cell.new) {
                (Some(a), Some(b)) if cell.moved => format!("<td class=\"chg\">{}</td>", moved_text(a, b)),
                (Some(_), Some(b)) => format!("<td>{}</td>", answer_text(b)),
                (None, Some(b)) => format!("<td class=\"new\">{}</td>", answer_text(b)),
                (Some(a), None) => format!("<td class=\"gone\"><s>{}</s></td>", answer_text(a)),
                (None, None) => "<td class=\"none\">—</td>".to_string(),
            };
            o.push_str(&html);
        }
        o.push_str("</tr>\n");
    }
    o.push_str("</tbody>\n</table></div>\n");
    o
}

/// The regions where a rule's two versions differ, from `rulec diff --format json`.
fn regions_html(j: &Json, lang: Lang) -> String {
    let changes = j.get("changes").and_then(Json::as_arr).unwrap_or(&[]);
    let unknown = j.get("unknown").and_then(Json::as_arr).unwrap_or(&[]);
    if changes.is_empty() && unknown.is_empty() {
        return String::new();
    }
    let mut o = String::from("<div class=\"scroll\"><table class=\"t\">\n<thead><tr>");
    for h in [tr!("違うところ", "Where they differ"), tr!("答え", "Answer"), tr!("例", "Example")] {
        o.push_str(&format!("<th scope=\"col\">{}</th>", esc(h.get(lang))));
    }
    o.push_str("</tr></thead>\n<tbody>\n");
    for c in changes {
        let region = c.get("text").and_then(Json::as_str).unwrap_or("");
        // the outputs that moved: a region says all of them
        let outs: Vec<String> = c
            .get("outputs")
            .and_then(Json::as_arr)
            .unwrap_or(&[])
            .iter()
            .filter_map(|x| {
                let s = |k: &str| x.get(k).and_then(Json::as_str).unwrap_or("").to_string();
                let (a, b) = (s("old"), s("new"));
                (a != b).then(|| format!("{}: {a} → {b}", s("output")))
            })
            .collect();
        let witness: Vec<String> = c
            .get("witness")
            .and_then(Json::as_arr)
            .unwrap_or(&[])
            .iter()
            .map(|x| {
                let s = |k: &str| x.get(k).and_then(Json::as_str).unwrap_or("").to_string();
                format!("{}={}", s("input"), s("value"))
            })
            .collect();
        o.push_str(&format!(
            "<tr><td class=\"mono\">{}</td><td class=\"mono nowrap\">{}</td><td class=\"mono\">{}</td></tr>\n",
            esc(region),
            outs.iter().map(|x| esc(x)).collect::<Vec<_>>().join("<br>"),
            esc(&witness.join(", "))
        ));
    }
    let not_settled = tr!("決まりませんでした", "Not settled");
    for u in unknown {
        let region = u.get("text").and_then(Json::as_str).unwrap_or("");
        let why = u.get("why").and_then(Json::as_str).unwrap_or("");
        o.push_str(&format!("<tr><td class=\"mono\">{}</td><td>{}</td><td>{}</td></tr>\n", esc(region), esc(not_settled.get(lang)), esc(why)));
    }
    o.push_str("</tbody>\n</table></div>\n");
    o
}

/// What a rule's two versions changed about what it takes and gives.
fn domain_html(j: &Json, lang: Lang) -> String {
    let rl = if lang == Lang::Ja { rulec::i18n::Lang::Ja } else { rulec::i18n::Lang::En };
    let items: Vec<String> = j
        .get("domain")
        .and_then(Json::as_arr)
        .unwrap_or(&[])
        .iter()
        .map(|x| {
            let s = |k: &str| x.get(k).and_then(Json::as_str).map(str::to_string);
            let (what, name) = (s("what").unwrap_or_default(), s("name").unwrap_or_default());
            let word = rulec::i18n::with(rl, || rulec::vdiff::domain_word(&what));
            let values = match (s("old"), s("new")) {
                (Some(a), Some(b)) => format!(" <code>{}</code> → <code>{}</code>", esc(&a), esc(&b)),
                _ => String::new(),
            };
            let colon = if lang == Lang::Ja { "：" } else { ": " };
            format!("<li><code>{}</code>{colon}{}{values}</li>", esc(&name), esc(&word))
        })
        .collect();
    if items.is_empty() { String::new() } else { format!("<ul class=\"domain\">\n{}\n</ul>\n", items.join("\n")) }
}

/// The days of a calendar that change, from `koyomi diff --format json`.
fn days_html(j: &Json, lang: Lang) -> String {
    let word = |w: &str| if w == "open" { tr!("営業日", "open") } else { tr!("休み", "closed") };
    let reasons = |x: &Json, k: &str| -> String { x.get(k).and_then(Json::as_arr).unwrap_or(&[]).iter().filter_map(|r| r.as_str().map(str::to_string).or_else(|| r.get("text").and_then(Json::as_str).map(str::to_string))).collect::<Vec<_>>().join(", ") };
    let mut o = String::new();
    if let Some(k) = j.get("known")
        && let (Some(a), Some(b)) = (k.get("old"), k.get("new"))
        && a.compact() != b.compact()
    {
        let side = |x: &Json| {
            let s = |f: &str| x.get(f).and_then(Json::as_str).unwrap_or("").to_string();
            format!("{} .. {}", s("from"), s("to"))
        };
        let (x, y) = (side(a), side(b));
        o.push_str(&format!("<p>{}</p>\n", super::markdown::inline(tr!("知っている日：`{x}` → `{y}`", "The days it knows: `{x}` → `{y}`").get(lang))));
    }
    let changes = j.get("changes").and_then(Json::as_arr).unwrap_or(&[]);
    if changes.is_empty() {
        return o;
    }
    o.push_str("<div class=\"scroll\"><table class=\"t\">\n<thead><tr>");
    for h in [tr!("日", "Days"), tr!("答え", "Answer"), tr!("理由", "Why")] {
        o.push_str(&format!("<th scope=\"col\">{}</th>", esc(h.get(lang))));
    }
    o.push_str("</tr></thead>\n<tbody>\n");
    for c in changes.iter().take(ROWS) {
        let s = |k: &str| c.get(k).and_then(Json::as_str).unwrap_or("").to_string();
        let (from, to) = (s("from"), s("to"));
        let span = if from == to { from } else { format!("{from} .. {to}") };
        let (a, b) = (word(&s("old")), word(&s("new")));
        let why = [reasons(c, "new_reasons"), reasons(c, "old_reasons")].into_iter().find(|x| !x.is_empty()).unwrap_or_default();
        o.push_str(&format!("<tr><td class=\"mono nowrap\">{}</td><td class=\"nowrap\">{} → {}</td><td>{}</td></tr>\n", esc(&span), esc(a.get(lang)), esc(b.get(lang)), esc(&why)));
    }
    if changes.len() > ROWS {
        let n = changes.len() - ROWS;
        o.push_str(&format!("<tr><td colspan=\"3\">{}</td></tr>\n", esc(tr!("ほかに {n} 件", "and {n} more").get(lang))));
    }
    o.push_str("</tbody>\n</table></div>\n");
    o
}

/// One file's article: what its language's diff found.
fn behavior_html(b: &Behavior, lang: Lang) -> String {
    let mut o = String::from("<article class=\"fileblock\">\n<div class=\"filehead\">");
    o.push_str(&format!("<h3 class=\"mono\">{}</h3>", esc(&b.file)));
    let (cls, word) = match &b.result {
        Ok(s) if s.differs => ("differ", tr!("答えが変わります", "Answers differ")),
        Ok(s) if !s.total => ("unsettled", tr!("調べきれません", "Not settled")),
        Ok(s) if s.more => ("same", tr!("答えは同じ（ほかの変わりがあります）", "Same answers, with other changes")),
        Ok(_) => ("same", tr!("同じ答え", "Same answers")),
        Err(_) => ("unsettled", tr!("比べられません", "Not compared")),
    };
    o.push_str(&format!("<span class=\"tag {cls}\">{}</span></div>\n", esc(word.get(lang))));
    match &b.result {
        Err(t) => o.push_str(&format!("<p>{}</p>\n", super::markdown::inline(t.get(lang)))),
        Ok(s) => match b.tool {
            Tool::Rulec => {
                o.push_str(&domain_html(&s.json, lang));
                if let Some(m) = &s.map {
                    o.push_str(&map_html(m, lang));
                }
                // a table says what the regions say, all but what could not be settled
                if s.map.is_none() || s.json.get("unknown").and_then(Json::as_arr).is_some_and(|u| !u.is_empty()) {
                    o.push_str(&regions_html(&s.json, lang));
                }
                if s.json.get("blocked").is_some_and(|x| !x.is_null()) || s.json.get("machine").is_some_and(|x| !x.is_null()) {
                    o.push_str(&format!("<details><summary>{}</summary>\n{}</details>\n", esc(tr!("規則の言語の出力", "What rulec said").get(lang)), md_html(&s.markdown)));
                }
            }
            Tool::Koyomi if s.json.get("kind").and_then(Json::as_str) == Some("calendar") => o.push_str(&days_html(&s.json, lang)),
            _ if s.differs || s.more || !s.total => o.push_str(&format!("<details open><summary>{}</summary>\n{}</details>\n", esc(tr!("言語の出力", "What its language said").get(lang)), md_html(&s.markdown))),
            _ => {}
        },
    }
    o.push_str("</article>\n");
    o
}

/// The interface: what changed, the places that say the same for each output put together.
fn interface_html(d: &ModDiff, lang: Lang) -> String {
    let shown: Vec<&diff::Iface> = d.interface.iter().filter(|i| !i.quiet()).collect();
    if shown.is_empty() {
        return String::new();
    }
    let mut o = String::from("<article class=\"fileblock\">\n");
    o.push_str(&format!("<h3>{}</h3>\n", esc(tr!("インターフェース", "Interface").get(lang))));
    for i in shown {
        let file = format!("<span class=\"mono\">{}</span>", esc(&i.file));
        if let Some(t) = &i.note {
            o.push_str(&format!("<p>{file}: {}</p>\n", super::markdown::inline(t.get(lang))));
        }
        if let Some((a, b)) = &i.version {
            o.push_str(&format!("<p>{file}: <code>{}</code> → <code>{}</code></p>\n", esc(a), esc(b)));
        }
        for (tool, f) in &i.breaks {
            o.push_str(&format!("<p>{file} ({tool}): {}</p>\n", esc(&f.text)));
        }
        let mut groups = grouped(i);
        groups.sort_by_key(|g| !g.may_break);
        // the outputs are named only where a change holds for some of them
        let mut every: Vec<&str> = groups.iter().flat_map(|g| g.outputs.iter().map(String::as_str)).collect();
        every.sort_unstable();
        every.dedup();
        for g in groups.iter().take(ROWS) {
            let verb = match g.how {
                How::Added => tr!("足されました", "was added"),
                How::Removed => tr!("消えました", "was removed"),
                How::Changed => tr!("変わりました", "changed"),
            };
            let some = g.outputs.len() < every.len();
            let list = if some { g.outputs.iter().map(|h| format!("<span class=\"lang mono\">{}</span>", esc(h))).collect::<Vec<_>>().join(" ") } else { String::new() };
            let code = esc(&g.place);
            let colon = if lang == Lang::Ja { "：" } else { ": " };
            let values = match (&g.old, &g.new) {
                (Some(a), Some(b)) => format!("{colon}<code>{}</code> → <code>{}</code>", esc(a), esc(b)),
                (Some(x), None) | (None, Some(x)) => format!("{colon}<code>{}</code>", esc(x)),
                (None, None) => String::new(),
            };
            let warns = if g.may_break { format!(" <span class=\"small warn\">{}</span>", esc(tr!("（使う側を壊しえます）", "(may break a user)").get(lang))) } else { String::new() };
            let line = match lang {
                Lang::Ja => format!("{file} の <code>{code}</code> が{}{values}{warns}", esc(verb.get(lang))),
                Lang::En => format!("In {file}, <code>{code}</code> {}{values}{warns}", esc(verb.get(lang))),
            };
            if list.is_empty() {
                o.push_str(&format!("<p>{line}</p>\n"));
            } else {
                o.push_str(&format!("<p>{line}</p>\n<p class=\"langs\">{list}</p>\n"));
            }
        }
    }
    o.push_str("</article>\n");
    o
}

/// The tab's content: `from` and `to` are the two versions, `to_select` the control that moves to
/// another version.
pub(super) fn pane(d: &ModDiff, from: &str, to_select: &str, lang: Lang) -> String {
    let mut o = String::new();
    o.push_str(&format!(
        "<div class=\"fromto\"><div><span class=\"small dim\">{}</span><div class=\"ctl mono fixed\">{}</div></div>{to_select}</div>\n",
        esc(tr!("比べる元", "From").get(lang)),
        esc(from)
    ));
    o.push_str(&format!("<h2 class=\"headline\">{}</h2>\n", esc(headline(d).get(lang))));
    o.push_str(&format!(
        "<p class=\"dim\">{}</p>\n",
        esc(tr!("ritsu はすべての入力を比べるか、決められないと言います。一部を試すことはしません。", "ritsu compares every input, or says it cannot decide. It does not sample.").get(lang))
    ));
    o.push_str(&tiles(d, lang));
    for b in d.behavior.iter().filter(|b| !matches!(&b.result, Ok(s) if !s.differs && s.total && !s.more)) {
        o.push_str(&behavior_html(b, lang));
    }
    let same: Vec<&Behavior> = d.behavior.iter().filter(|b| matches!(&b.result, Ok(s) if !s.differs && s.total && !s.more)).collect();
    if !same.is_empty() {
        o.push_str("<article class=\"fileblock sames\">\n");
        for b in same {
            o.push_str(&format!("<div class=\"filehead\"><h3 class=\"mono\">{}</h3><span class=\"tag same\">{}</span></div>\n", esc(&b.file), esc(tr!("同じ答え", "Same answers").get(lang))));
        }
        o.push_str("</article>\n");
    }
    o.push_str(&interface_html(d, lang));
    o.push_str(&format!(
        "<details class=\"whole\"><summary>{}</summary>\n{}</details>\n",
        super::markdown::inline(tr!("報告の全文（`ritsu mod diff --format markdown` と同じ）", "The whole report, as `ritsu mod diff --format markdown` writes it").get(lang)),
        md_html(&diff::body_markdown(d, lang, 4, ROWS))
    ));
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::diff::{IChange, Iface};
    use rulec::vdiff::{MapAxis, MapCell, MapOutput};

    fn answers(a: &[(&str, &str)]) -> Option<Vec<(String, String)>> {
        Some(a.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
    }

    fn word(w: &str, s: MapSide) -> MapCoord {
        MapCoord::Word(w.to_string(), s)
    }

    fn axis(col: &str, coords: Vec<MapCoord>) -> MapAxis {
        MapAxis { col: col.to_string(), unit: String::new(), date: false, coords }
    }

    fn outputs() -> Vec<MapOutput> {
        vec![MapOutput { name: "most".to_string(), unit: Some("USD".to_string()) }, MapOutput { name: "per".to_string(), unit: None }]
    }

    /// A rule of one input: a row for each value, a column for each output with its unit, and the mark
    /// only on the answers that moved; a value only one version takes says which.
    #[test]
    fn a_rule_of_one_input_reads_down_its_outputs() {
        let cell = |old: Option<Vec<(String, String)>>, new: Option<Vec<(String, String)>>, moved| MapCell { old, new, moved };
        let m = Map {
            rows: None,
            cols: axis("violation", vec![word("willful", MapSide::Both), word("posting", MapSide::Both), word("added", MapSide::NewOnly), word("gone", MapSide::OldOnly)]),
            outputs: outputs(),
            cells: vec![vec![
                cell(answers(&[("most", "161323"), ("per", "violation")]), answers(&[("most", "165514"), ("per", "violation")]), true),
                cell(answers(&[("most", "100"), ("per", "day")]), answers(&[("most", "100"), ("per", "day")]), false),
                cell(None, answers(&[("most", "5"), ("per", "day")]), true),
                cell(answers(&[("most", "7"), ("per", "day")]), None, true),
            ]],
        };
        let html = map_html(&m, Lang::En);
        assert!(html.contains("<thead><tr><th scope=\"col\">violation</th><th scope=\"col\">most <span class=\"note\">USD</span></th><th scope=\"col\">per</th></tr></thead>"), "{html}");
        assert!(html.contains("<tr><th scope=\"row\">willful</th><td class=\"chg\"><s>161,323</s> 165,514</td><td>violation</td></tr>"), "{html}");
        assert!(html.contains("<tr><th scope=\"row\">posting</th><td>100</td><td>day</td></tr>"), "{html}");
        assert!(html.contains("added <span class=\"note\">(new)</span></th><td class=\"new\">5</td><td class=\"new\">day</td></tr>"), "{html}");
        assert!(html.contains("gone <span class=\"note\">(removed)</span></th><td class=\"gone\"><s>7</s></td><td class=\"gone\"><s>day</s></td></tr>"), "{html}");
    }

    /// A rule of two inputs: the cell of two outputs marks the one that moved and leaves the other faint.
    #[test]
    fn a_cell_of_two_outputs_marks_the_one_that_moved() {
        let m = Map {
            rows: Some(axis("region", vec![word("uk", MapSide::Both)])),
            cols: axis("kind", vec![word("a", MapSide::Both)]),
            outputs: outputs(),
            cells: vec![vec![MapCell { old: answers(&[("most", "1000"), ("per", "day")]), new: answers(&[("most", "2000"), ("per", "day")]), moved: true }]],
        };
        let html = map_html(&m, Lang::En);
        assert!(html.contains("<td class=\"chg\">most: <s>1,000</s> 2,000<br><span class=\"same\">per: day</span></td>"), "{html}");
    }

    /// A change a rule says for each language it generates is one change, and what a file says of its
    /// sources is said whole, and no candidate for what breaks a user.
    #[test]
    fn the_changes_of_an_interface_are_counted_as_they_are_listed() {
        let change = |at: &str, old: &str, new: &str| IChange { at: at.to_string(), how: How::Changed, old: Some(Json::str(old)), new: Some(Json::str(new)) };
        let i = Iface {
            file: "a.rule".to_string(),
            tool: Some(Tool::Rulec),
            how: "api",
            version: None,
            changes: vec![
                change("/sources/dol/asof", "2024-02-01", "2025-02-01"),
                change("/python/outputs/most/range/max", "1", "2"),
                change("/rust/outputs/most/range/max", "1", "2"),
                change("/go/output_fields/most/range/max", "1", "2"),
                change("/sql/outputs/most/range/max", "1", "3"),
            ],
            breaks: Vec::new(),
            note: None,
        };
        let g = grouped(&i);
        let got: Vec<(&str, bool, Vec<&str>)> = g.iter().map(|g| (g.place.as_str(), g.may_break, g.outputs.iter().map(String::as_str).collect())).collect();
        assert_eq!(
            got,
            [
                ("sources/dol/asof", false, vec![]),
                ("outputs/most/range/max", true, vec!["python", "rust", "go"]),
                ("outputs/most/range/max", true, vec!["sql"]),
            ]
        );
        assert_eq!((g[0].old.as_deref(), g[0].new.as_deref()), (Some("2024-02-01"), Some("2025-02-01")));
    }
}
