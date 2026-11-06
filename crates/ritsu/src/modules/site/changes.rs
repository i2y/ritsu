//! The tab of what changed since the version before (DESIGN 17.17): what `ritsu mod diff` found, said
//! first in a sentence, then in tiles, then why, in the words of the module's requirements when it has
//! them, then file by file: a table of what the two versions answer when the rule is small enough to
//! have one, and the regions where they differ. The body of `ritsu mod diff --format markdown` stays
//! under it, whole, as the same report on a machine reads.

use super::holds::number;
use super::markdown::md_html;
use crate::modules::diff::{self, Behavior, How, ModDiff};
use crate::modules::why::{self, Why};
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
    let named = |l: &[String]| l.iter().filter(|x| !diff::is_record(x)).count();
    let (added, removed) = (named(&f.added), named(&f.removed));
    let records = f.added.len() + f.removed.len() - added - removed;
    let mut parts: Vec<Text> = Vec::new();
    if added > 0 {
        parts.push(tr!("{added} 増えた", "{added} added"));
    }
    if removed > 0 {
        parts.push(tr!("{removed} 消えた", "{removed} removed"));
    }
    parts.push(tr!("{changed} 変わった", "{changed} changed"));
    parts.push(tr!("{same} 同じ", "{same} the same"));
    if records > 0 {
        parts.push(tr!("reviewed/ の入れ替わり {records}", "{records} moved in reviewed/"));
    }
    let list = Text::join(&parts, "、", ", ");
    let req = &d.requires;
    let moved_req = req.added.len() + req.removed.len() + req.changed.len();
    let require = if moved_req == 0 { tr!("require は変わりません", "require: no change") } else { tr!("require は {moved_req} 行変わります", "require: {moved_req} changed") };
    let files = list.then(&tr!("。", ". ")).then(&require);
    let tile = |label: Text, v: Text| format!("<div><dt>{}</dt><dd>{}</dd></div>\n", esc(label.get(lang)), esc(v.get(lang)));
    let requirements = if d.why.is_empty() { String::new() } else { tile(tr!("要件", "Requirements"), requirements_tile(d)) };
    format!(
        "<dl class=\"tiles\">\n{}{}{requirements}{}</dl>\n",
        tile(tr!("振る舞い", "Behavior"), behavior),
        tile(tr!("インターフェース", "Interface"), interface),
        tile(tr!("ファイル", "Files"), files)
    )
}

/// How many requirements moved, and how many decisions came with them.
fn requirements_tile(d: &ModDiff) -> Text {
    let (moved, decided) = (d.why.len(), d.why.iter().map(|w| w.decided.len()).sum::<usize>());
    let r = if decided == 1 { "decision" } else { "decisions" };
    match decided {
        0 => tr!("{moved} 件変わり、新しく決めたことは書いていません", "{moved} moved, no new decision written"),
        _ => tr!("{moved} 件変わり、新しく決めたこと {decided} 件", "{moved} moved, {decided} new {r}"),
    }
}

/// Whether the words of a module are in Japanese: a module may keep its requirements in English and
/// in Japanese, as it keeps its rules, and a page puts first what is in its own language.
pub(super) fn in_japanese(text: &str) -> bool {
    text.chars().any(|c| matches!(c, '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{9fff}'))
}

/// Whether a text is in the language of the page.
fn in_language(text: &str, lang: Lang) -> bool {
    in_japanese(text) == matches!(lang, Lang::Ja)
}

/// The first reason a version gives for what moved, for a line of the list, and how many more: of the
/// reasons in the page's language when there are any (the same reason in the other language is not
/// one more), and else of all of them.
pub(super) fn reason_of(d: &ModDiff, lang: Lang) -> Option<(String, usize)> {
    let all: Vec<&str> = d.why.iter().flat_map(|w| w.decided.iter().map(|x| x.why.as_str())).collect();
    let here: Vec<&str> = all.iter().copied().filter(|r| in_language(r, lang)).collect();
    let (first, more) = if here.is_empty() { all.split_first()? } else { here.split_first()? };
    Some((first.to_string(), more.len()))
}

/// The two texts, word by word: what is the same as it is, what went in `<del>`, what came in `<ins>`.
fn words_html(old: &str, new: &str) -> String {
    let (a, b) = (tokens(old), tokens(new));
    let (n, m) = (a.len(), b.len());
    // a text this long differs throughout, or is no sentence to read word by word
    if n * m > 1_000_000 {
        return format!("<del>{}</del><ins>{}</ins>", esc(old), esc(new));
    }
    let mut t = vec![0u32; (n + 1) * (m + 1)];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i * (m + 1) + j] = if a[i] == b[j] { t[(i + 1) * (m + 1) + j + 1] + 1 } else { t[(i + 1) * (m + 1) + j].max(t[i * (m + 1) + j + 1]) };
        }
    }
    let mut o = String::new();
    let (mut del, mut ins) = (String::new(), String::new());
    let flush = |del: &mut String, ins: &mut String, o: &mut String| {
        if !del.is_empty() {
            o.push_str(&format!("<del>{}</del>", esc(del)));
        }
        if !ins.is_empty() {
            o.push_str(&format!("<ins>{}</ins>", esc(ins)));
        }
        del.clear();
        ins.clear();
    };
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            flush(&mut del, &mut ins, &mut o);
            o.push_str(&esc(a[i]));
            i += 1;
            j += 1;
        } else if j == m || (i < n && t[(i + 1) * (m + 1) + j] >= t[i * (m + 1) + j + 1]) {
            del.push_str(a[i]);
            i += 1;
        } else {
            ins.push_str(b[j]);
            j += 1;
        }
    }
    flush(&mut del, &mut ins, &mut o);
    o
}

/// A text's words: a run of ASCII letters and digits, with the commas and points inside a number, and
/// any other character alone (Japanese has no spaces to split at).
fn tokens(s: &str) -> Vec<&str> {
    let cs: Vec<(usize, char)> = s.char_indices().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let mut j = i + 1;
        if cs[i].1.is_ascii_alphanumeric() {
            while j < cs.len() {
                let c = cs[j].1;
                let inside = (c == ',' || c == '.') && cs[j - 1].1.is_ascii_digit() && cs.get(j + 1).is_some_and(|x| x.1.is_ascii_digit());
                if c.is_ascii_alphanumeric() || inside {
                    j += 1;
                } else {
                    break;
                }
            }
        }
        let end = cs.get(j).map_or(s.len(), |x| x.0);
        out.push(&s[cs[i].0..end]);
        i = j;
    }
    out
}

/// `a → b` of two values, the one that went struck out: the one value when they do not differ.
fn pair_html(p: &why::Pair) -> Option<String> {
    match p {
        (Some(a), Some(b)) if a != b => Some(format!("<del>{}</del><ins>{}</ins>", esc(a), esc(b))),
        (_, Some(x)) | (Some(x), None) => Some(esc(x)),
        (None, None) => None,
    }
}

/// What a thing a requirement stands on did, in a few words.
fn how_note(how: Option<How>, lang: Lang) -> String {
    let t = match how {
        Some(How::Changed) => tr!("中身が変わりました", "its text changed"),
        Some(How::Added) => tr!("新しく足しました", "new here"),
        Some(How::Removed) => tr!("外しました", "no longer here"),
        None => return String::new(),
    };
    format!(" <span class=\"small dim\">{}</span>", esc(t.get(lang)))
}

/// The lines of what a source's text says differently: a line that changed, word by word.
fn lines_html(lines: &[(char, String)], more: usize, lang: Lang) -> String {
    let mut o = String::new();
    let mut k = 0;
    while k < lines.len() {
        let (op, text) = &lines[k];
        match op {
            '@' => k += 1,
            ' ' => {
                o.push_str(&format!("<p class=\"same\">{}</p>\n", esc(text)));
                k += 1;
            }
            _ => {
                let gone: Vec<&String> = lines[k..].iter().take_while(|(x, _)| *x == '-').map(|(_, t)| t).collect();
                let came: Vec<&String> = lines[k + gone.len()..].iter().take_while(|(x, _)| *x == '+').map(|(_, t)| t).collect();
                if gone.len() == came.len() {
                    for (a, b) in gone.iter().zip(&came) {
                        o.push_str(&format!("<p>{}</p>\n", words_html(a, b)));
                    }
                } else {
                    for a in &gone {
                        o.push_str(&format!("<p><del>{}</del></p>\n", esc(a)));
                    }
                    for b in &came {
                        o.push_str(&format!("<p><ins>{}</ins></p>\n", esc(b)));
                    }
                }
                k += gone.len() + came.len();
            }
        }
    }
    if more > 0 {
        o.push_str(&format!("<p class=\"same\">{}</p>\n", esc(tr!("ほか {more} 行", "and {more} more lines").get(lang))));
    }
    o
}

/// One requirement that moved: its text word by word, then what it says of itself. A link goes to
/// the file's page, given what `code` makes of a path from the module's root.
fn requirement_html(w: &Why, lang: Lang, code: &dyn Fn(&str) -> Option<String>) -> String {
    let (tag, word) = match w.how {
        How::Changed => ("warnish", tr!("変わった", "Changed")),
        How::Added => ("same", tr!("新しい", "New")),
        How::Removed => ("unsettled", tr!("消えた", "Removed")),
    };
    let name = match (code(&w.file), w.line) {
        (Some(h), Some(l)) => format!("<a href=\"{}#L{l}\">{}</a>", esc(&h), esc(&w.name)),
        (Some(h), None) => format!("<a href=\"{}\">{}</a>", esc(&h), esc(&w.name)),
        _ => esc(&w.name),
    };
    let mut o = format!("<div class=\"whyreq\">\n<div class=\"filehead\"><h4 class=\"mono\">{name}</h4><span class=\"tag {tag}\">{}</span></div>\n", esc(word.get(lang)));
    match &w.text {
        (Some(a), Some(b)) if a != b => o.push_str(&format!("<p class=\"whytext\">{}</p>\n", words_html(a, b))),
        (_, Some(x)) | (Some(x), None) => o.push_str(&format!("<p class=\"whytext\">{}</p>\n", esc(x))),
        (None, None) => {}
    }
    let mut rows = String::new();
    let mut row = |label: &str, value: String| rows.push_str(&format!("<dt>{}</dt><dd>{value}</dd>\n", esc(&ritsu_base::text::capitalize(label))));
    if let Some(v) = pair_html(&w.in_force) {
        row(why::in_force_label().get(lang), format!("<span class=\"mono\">{v}</span>"));
    }
    if matches!(&w.owner, (Some(a), Some(b)) if a != b) {
        row(why::owner_label().get(lang), pair_html(&w.owner).unwrap_or_default());
    }
    if w.how != How::Removed {
        let label = tr!("決めたこと", "Decided");
        if w.decided.is_empty() {
            row(label.get(lang), format!("<span class=\"dim\">{}</span>", esc(tr!("この版では、新しく決めたことを書いていません", "This version writes no new decision").get(lang))));
        } else {
            let list: Vec<String> = w.decided.iter().map(|d| format!("{} <span class=\"small dim\">{}</span>", esc(&d.why), esc(&format!("{} · {}", d.date, d.by)))).collect();
            row(label.get(lang), list.join("<br>"));
        }
    }
    for f in &w.from {
        let mut v = format!("<code>{}</code>", esc(&f.what));
        if let Some(id) = &f.id {
            v.push_str(&format!(" {}", esc(id)));
        }
        if let Some(a) = pair_html(&f.asof) {
            let asof = match lang {
                Lang::Ja => format!(" <span class=\"mono\">{a}</span> 時点"),
                Lang::En => format!(" as of <span class=\"mono\">{a}</span>"),
            };
            v.push_str(&asof);
        }
        v.push_str(&how_note(f.how, lang));
        if !f.lines.is_empty() {
            v.push_str(&format!("<details class=\"whydiff\"><summary>{}</summary>\n{}</details>", esc(tr!("本文の違い", "What its text says differently").get(lang)), lines_html(&f.lines, f.more, lang)));
        }
        row(why::from_label().get(lang), v);
    }
    for l in &w.links {
        let shown = format!("<code>{}</code>", esc(&l.artifact));
        let v = match l.path.as_deref().and_then(code) {
            Some(h) => format!("<a href=\"{}\">{shown}</a>", esc(&h)),
            None => shown,
        };
        row(l.label().get(lang), format!("{v}{}", how_note(l.how, lang)));
    }
    for x in &w.waived {
        row(x.label().get(lang), esc(&x.why));
    }
    if !w.looked.is_empty() {
        let list: Vec<String> = w.looked.iter().map(|(d, b)| format!("<span class=\"mono\">{}</span> {}", esc(d), esc(b))).collect();
        row(tr!("確認", "Looked at").get(lang), list.join("、"));
    }
    if w.unsettled > 0 {
        row(tr!("確認", "Looked at").get(lang), format!("<span class=\"warn\">{}</span>", super::markdown::inline(why::unsettled_label(w.unsettled).get(lang))));
    }
    if !rows.is_empty() {
        o.push_str(&format!("<dl class=\"whylines\">\n{rows}</dl>\n"));
    }
    o.push_str("</div>\n");
    o
}

/// Why it changed, in the words of the module's requirements.
fn why_html(d: &ModDiff, lang: Lang, code: &dyn Fn(&str) -> Option<String>) -> String {
    if d.why.is_empty() {
        return String::new();
    }
    let mut o = format!("<article class=\"fileblock why\">\n<h3>{}</h3>\n", esc(tr!("変わった理由", "Why it changed").get(lang)));
    // what is in the page's language first, each language in the order of the files
    let mut shown: Vec<&Why> = d.why.iter().collect();
    shown.sort_by_key(|w| !in_language(w.text.1.as_deref().or(w.text.0.as_deref()).unwrap_or(&w.name), lang));
    for w in shown {
        o.push_str(&requirement_html(w, lang, code));
    }
    o.push_str(&format!("<p class=\"small dim whynote\">{}</p>\n</article>\n", esc(why::own_words().get(lang))));
    o
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
/// another version, and `code` what a link to a file of the new version says, from the file's path.
pub(super) fn pane(d: &ModDiff, from: &str, to_select: &str, lang: Lang, code: &dyn Fn(&str) -> Option<String>) -> String {
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
    o.push_str(&why_html(d, lang, code));
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

    /// A sentence read word by word: a number moves whole, and the rest stays as it is.
    #[test]
    fn two_texts_read_word_by_word() {
        assert_eq!(
            words_html("not exceed $161,323. Proposed after January 15, 2024", "not exceed $165,514. Proposed after January 15, 2025"),
            "not exceed $<del>161,323</del><ins>165,514</ins>. Proposed after January 15, <del>2024</del><ins>2025</ins>"
        );
        assert_eq!(words_html("a <b>", "a <c>"), "a &lt;<del>b</del><ins>c</ins>&gt;");
        assert_eq!(words_html("税率は十パーセント", "税率は八パーセント"), "税率は<del>十</del><ins>八</ins>パーセント");
    }

    /// What a requirement says of itself, with a link to its line and to what meets it; what is not in
    /// the module has no link, and the words of the file are escaped.
    #[test]
    fn a_requirement_that_moved_is_said_in_the_words_of_its_file() {
        let w = Why {
            file: "r.req".to_string(),
            line: Some(9),
            name: "amounts".to_string(),
            how: How::Changed,
            text: (Some("At most $1.".to_string()), Some("At most $2.".to_string())),
            in_force: (Some("2024-01-16..".to_string()), Some("2025-01-16..".to_string())),
            owner: (Some("m".to_string()), Some("m".to_string())),
            decided: vec![why::Decided { date: "2026-11-05".to_string(), by: "m".to_string(), why: "Follows <the> amendment".to_string() }],
            from: vec![why::Cited {
                what: "@law §1".to_string(),
                id: Some("29 CFR 1".to_string()),
                asof: (Some("2024-02-01".to_string()), Some("2025-02-01".to_string())),
                how: Some(How::Changed),
                lines: vec![('@', "@@ -1,1 +1,1 @@".to_string()), (' ', "(c) Kept.".to_string()), ('-', "(d) Up to $1.".to_string()), ('+', "(d) Up to $2.".to_string())],
                more: 0,
            }],
            links: vec![
                why::Linked { role: "satisfied".to_string(), artifact: "rulec \"r.rule\" table t".to_string(), path: Some("r.rule".to_string()), how: Some(How::Changed) },
                why::Linked { role: "verified".to_string(), artifact: "rulec \"@dep/x.rule\"".to_string(), path: Some("@dep/x.rule".to_string()), how: None },
            ],
            waived: Vec::new(),
            looked: vec![("2026-11-05".to_string(), "m".to_string())],
            unsettled: 0,
        };
        let code = |rel: &str| if rel.starts_with('@') { None } else { Some(format!("code/{rel}.html")) };
        let html = requirement_html(&w, Lang::En, &code);
        assert!(html.contains("<h4 class=\"mono\"><a href=\"code/r.req.html#L9\">amounts</a></h4><span class=\"tag warnish\">Changed</span>"), "{html}");
        assert!(html.contains("<p class=\"whytext\">At most $<del>1</del><ins>2</ins>.</p>"), "{html}");
        assert!(html.contains("<dt>In force</dt><dd><span class=\"mono\"><del>2024-01-16..</del><ins>2025-01-16..</ins></span></dd>"), "{html}");
        assert!(html.contains("<dt>Decided</dt><dd>Follows &lt;the&gt; amendment <span class=\"small dim\">2026-11-05 · m</span></dd>"), "{html}");
        assert!(html.contains("<code>@law §1</code> 29 CFR 1 as of <span class=\"mono\"><del>2024-02-01</del><ins>2025-02-01</ins></span>"), "{html}");
        assert!(html.contains("<p class=\"same\">(c) Kept.</p>\n<p>(d) Up to $<del>1</del><ins>2</ins>.</p>"), "{html}");
        assert!(html.contains("<dt>Met by</dt><dd><a href=\"code/r.rule.html\"><code>rulec &quot;r.rule&quot; table t</code></a> <span class=\"small dim\">its text changed</span></dd>"), "{html}");
        assert!(html.contains("<dt>Checked by</dt><dd><code>rulec &quot;@dep/x.rule&quot;</code></dd>"), "{html}");
        assert!(html.contains("<dt>Looked at</dt><dd><span class=\"mono\">2026-11-05</span> m</dd>"), "{html}");
        assert!(!html.contains("<dt>Owner</dt>"), "{html}");
        let none = Why { decided: Vec::new(), ..w.clone() };
        assert!(requirement_html(&none, Lang::En, &code).contains("This version writes no new decision"));
        let gone = Why { how: How::Removed, text: (Some("At most $1.".to_string()), None), decided: Vec::new(), from: Vec::new(), links: Vec::new(), looked: Vec::new(), ..w };
        let html = requirement_html(&gone, Lang::Ja, &code);
        assert!(html.contains("<span class=\"tag unsettled\">消えた</span>") && html.contains("<p class=\"whytext\">At most $1.</p>") && !html.contains("決めたこと"), "{html}");
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
