//! The lowered files checked by their languages, and what those say, said again at the request's
//! lines in ringi's words (DESIGN 6.5, 9.2).
//!
//! The lowered files are laid over the files the check reads (`ritsu_base::fs::Overlay`), at
//! `<file>.lowered/` beside the request, for as long as the languages are asked about them: rulec
//! and koyomi read them as files of their own, through the ports (`Rules::findings`,
//! `Dates::findings`), and nothing is written to the disk. A file that is there already on the disk
//! is not laid over: E201 says so.
//!
//! Each diagnostic comes back with its line in the lowered file, which the map of lines takes to the
//! request's line; its code is said as ringi's: a row no request takes E301, two rows that take one
//! E302, a row no request reaches E303, an approver no request reaches W303, a `possible` that does
//! not happen E306, an example that does not hold E601, a reminder after its deadline E403, a
//! deadline outside the calendar's data E404; any other E201 (W201 for a warning), with the
//! language's own code and words. A mistake in the route's table makes the examples and the
//! claims of the table fail with it: those are counted under it, not said. A row rulec shows no
//! request reaches in a table ringi cuts a condition into (`form.rule`, `values.rule`, the question
//! of E105) is no mistake of the request: the file is laid again without it and asked again
//! ([`unreached`]).

use crate::ast::Route;
use crate::diag::{Diag, Spaced};
use crate::lower::words::ColKind;
use crate::lower::{self, Ctx, LFile, Lowered, Part};
use crate::suite::Suite;
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text};
use ritsu_ports::{Finding, Found, LoweredTool};

/// A diagnostic of a lowered file in both languages.
struct Said {
    en: Finding,
    ja: Finding,
}

impl Said {
    fn code(&self) -> &str {
        &self.en.code
    }

    fn is_error(&self) -> bool {
        self.en.severity == ritsu_base::diag::Severity::Error
    }

    /// Its headline, as the language writes it (rulec's `title`, koyomi's `message`), with the
    /// names of the request where the lowered file `lf` renamed them.
    fn title(&self, lf: &LFile) -> Text {
        let t = |f: &Finding| lf.named_back(f.json.get("title").or_else(|| f.json.get("message")).and_then(|j| j.as_str()).unwrap_or_default());
        Text::new(t(&self.ja), t(&self.en))
    }

    /// Its notes, the same way.
    fn notes(&self, lf: &LFile) -> Vec<Text> {
        let ns = |f: &Finding| f.json.get("notes").and_then(|j| j.as_arr()).map(|a| a.iter().filter_map(|n| n.as_str().map(|n| lf.named_back(n))).collect::<Vec<_>>()).unwrap_or_default();
        let (ja, en) = (ns(&self.ja), ns(&self.en));
        en.into_iter().enumerate().map(|(i, e)| Text::new(ja.get(i).cloned().unwrap_or_else(|| e.clone()), e)).collect()
    }
}

/// The request lowered, its files checked by their languages, and what they say said in ringi's
/// words, pushed to `out`; the claims rulec answers for the route (E306, E105) too. The lowered
/// request, for the port, when nothing stopped it from being laid.
pub fn check(cx: &Ctx, suite: &Suite, out: &mut Vec<Diag>) -> Option<Lowered> {
    let s = cx.s;
    let lowered = match lower::lower(cx) {
        Ok(l) => l,
        Err(unread) => {
            out.extend(unread.iter().map(|u| e309(cx, u)));
            return None;
        }
    };
    if lowered.files.is_empty() {
        return Some(lowered);
    }
    // a file already on the disk where a lowered file is laid, named as the run writes its paths
    for f in &lowered.files {
        let p = lowered.path(f);
        if ritsu_base::fs::exists(&p) {
            let shown = ritsu_base::paths::show(&p);
            out.push(
                s.err("E201", s.file.name.span, tr!("下ろしたファイルを置く `{shown}` に、ファイルがあります", "There is a file at `{shown}`, where ringi lays what it lowers"))
                    .note(tr!(
                        "ringi は、下ろしたファイルをディスクに書かずに、このパスに置いて確かめます。ディスクのファイルを消すか、別の名前にしてください（`ritsu ringi extract` で書き出したファイルなら、`--out` に別のディレクトリを渡してください）。",
                        "The files ringi lowers are laid at this path and checked there, not written to the disk. Remove the file on the disk or rename it (for files `ritsu ringi extract` wrote, pass another directory to `--out`)."
                    )),
            );
            return None;
        }
    }
    // the lowered files laid once for the run (`Lowered::laid`): what the check asks here, the
    // claims after it and the ticket machine ask of one set of files, and each lowered file is
    // checked once (ritsu's DESIGN 4.15). A row rulec shows no request reaches, in a table ringi
    // cuts a condition into, is a part of the condition no request is in: it is left out, and the
    // file laid again without it and asked again ([`unreached`], DESIGN 6.4)
    let mut lowered = lowered;
    let all: Vec<usize> = (0..lowered.files.len()).collect();
    let mut said = lowered.fs(|| ask(cx, suite, &lowered, &all, out));
    for _ in 0..4 {
        let drops = unreached(&lowered, &said);
        if drops.is_empty() {
            break;
        }
        lowered = lowered.without_lines(&drops);
        let again: Vec<usize> = drops.iter().map(|(i, _)| *i).collect();
        let more = lowered.fs(|| ask(cx, suite, &lowered, &again, out));
        said.retain(|(i, _)| !again.contains(i));
        said.extend(more);
        said.sort_by_key(|(i, _)| *i);
    }
    lowered.fs(|| {
        say(cx, &lowered, &said, out);
        if let Some(r) = &suite.rules
            && !crate::diag::has_errors(out)
        {
            crate::claims::only_when(cx, &lowered, r.as_ref(), out);
            crate::claims::hidden(cx, &lowered, r.as_ref(), out);
        }
    });
    Some(lowered)
}

/// What the languages say of the lowered files `which`, with the files laid (`Lowered::fs`): in the
/// run's language (`cx.lang`), and in the other too only when it says anything, since a check is
/// made in one language and asked again for the other: a file it says nothing of is checked once,
/// in the language the run asks of it later too. A file that cannot be read is E201, pushed to `out`.
fn ask(cx: &Ctx, suite: &Suite, lowered: &Lowered, which: &[usize], out: &mut Vec<Diag>) -> Vec<(usize, Vec<Said>)> {
    let s = cx.s;
    let mut said: Vec<(usize, Vec<Said>)> = Vec::new();
    for &i in which {
        let f = &lowered.files[i];
        let p = lowered.path(f);
        let ask = |lang: Lang| match f.tool {
            LoweredTool::Rulec => suite.rules.as_ref().map(|r| r.findings(&p, lang)),
            LoweredTool::Koyomi => suite.dates.as_ref().map(|d| d.findings(&p, lang)),
        };
        let other = if cx.lang == Lang::Ja { Lang::En } else { Lang::Ja };
        let Some(first) = ask(cx.lang) else { continue };
        let second = match &first {
            Ok(Found::Value(v)) if v.is_empty() => Ok(Found::Value(Vec::new())),
            _ => ask(other).unwrap_or(Ok(Found::Value(Vec::new()))),
        };
        let (en, ja) = if cx.lang == Lang::Ja { (second, first) } else { (first, second) };
        match (en, ja) {
            (Ok(Found::Value(en)), Ok(Found::Value(ja))) => {
                let pairs = en.into_iter().zip(ja.into_iter().map(Some).chain(std::iter::repeat_with(|| None))).map(|(e, j)| Said { ja: j.unwrap_or_else(|| e.clone()), en: e }).collect();
                said.push((i, pairs));
            }
            (Err(why), _) => {
                // the lowered file is no file anyone sees: named by its name (DESIGN 6.5)
                let w = why.first().map(|w| w.message.clone()).unwrap_or_default();
                let name = &f.name;
                let (ja, en) = (w.ja.clone(), w.en.clone());
                out.push(s.err("E201", s.file.name.span, tr!("下ろした `{name}` を読めません: {ja}", "The lowered `{name}` cannot be read: {en}")));
            }
            _ => {}
        }
    }
    said
}

/// The rows rulec shows no request reaches (its E102) in the tables ringi cuts a condition into —
/// a `shown when` or a `required when` in `form.rule` and its copy `values.rule`, and the question
/// of E105 — by file, as the lines of the file (from 1). ringi cuts a condition into rows over the
/// columns it reads, both sides of it: a row no request is in is a part of the condition that
/// never comes, which is an answer, not a mistake of the request (DESIGN 6.4). A row of a table the
/// request writes (the route's, a named table) is the request's, and is said (E303, E201).
fn unreached(lowered: &Lowered, said: &[(usize, Vec<Said>)]) -> Vec<(usize, Vec<usize>)> {
    let mut out = Vec::new();
    for (i, list) in said {
        let f = &lowered.files[*i];
        if f.tool != LoweredTool::Rulec || !matches!(f.part, Part::Form | Part::Values | Part::Hidden(_)) {
            continue;
        }
        let mut lines: Vec<usize> = list
            .iter()
            .filter(|x| x.code() == "E102" && x.is_error())
            .filter_map(|x| x.en.line)
            .filter(|l| {
                let t = line_text(f, Some(*l));
                let t = t.trim_start();
                t.starts_with('|') && !t.contains("->")
            })
            .collect();
        lines.sort_unstable();
        lines.dedup();
        if !lines.is_empty() {
            out.push((*i, lines));
        }
    }
    out
}

/// E309: a part of a condition the table it is lowered to cannot hold (DESIGN 5.3), said where it is
/// written; a part written in the shared route the request uses, at the request's `route` line.
pub fn e309(cx: &Ctx, u: &lower::route::Unread) -> Diag {
    use crate::lower::route::Unreadable as U;
    let s = cx.s;
    let shared = match &s.file.route {
        Some(Route::Shared { alias, line, .. }) => Some((alias.text.clone(), crate::ast::Span { line: *line, col: alias.span.col, end: alias.span.end })),
        _ => None,
    };
    let (at, alias) = match (&shared, u.here) {
        (Some((a, at)), false) => (*at, a.clone()),
        (Some((a, _)), true) => (u.span, a.clone()),
        (None, _) => (u.span, String::new()),
    };
    let w = crate::types::written(s.src, u.span);
    let (of_ja, of_en) = if u.form { ("フォーム", "form") } else { ("経路", "route") };
    let title = match &u.why {
        _ if !u.here => tr!("この申請が渡す物では、経路は共有の経路 `{alias}` の条件を読めません", "With what the request gives it, the route cannot read a condition of the shared route `{alias}`"),
        U::Field { name, ty, computed: true } => tr!("{of_ja}は、計算する `{ty}` のフィールド `{name}` を読めません", "The {of_en} cannot read `{name}`, a computed `{ty}`"),
        U::Field { name, ty, .. } => tr!("{of_ja}は `{ty}` のフィールド `{name}` を読めません", "The {of_en} cannot read `{name}`, a field of `{ty}`"),
        U::Two { a, b, .. } => tr!("{of_ja}は `{a}` と `{b}` を比べられません", "The {of_en} cannot compare `{a}` with `{b}`"),
        U::Computed { .. } => tr!("{of_ja}は `{w}` を計算できません", "The {of_en} cannot compute `{w}`"),
        U::Requester | U::Ticket | U::Other => tr!("{of_ja}は `{w}` を読めません", "The {of_en} cannot read `{w}`"),
    };
    let mut d = s.err("E309", at, title);
    if !u.here {
        let l = u.span.line;
        d = d.note(tr!("共有の経路の {l} 行目の条件です。", "It is the condition on line {l} of the shared route."));
    }
    d = d.note(if u.form {
        tr!(
            "フォームの条件（`shown when`、`required when`）が読めるのは、表に値を書けるフォームのフィールド（自分の値か規則の値から選ぶ選択、数、金額、日付、真偽）と、申請が使う規則の出力です。条件は、それらを列にした表に下ろして、rulec が確かめます。",
            "A condition of the form (`shown when`, `required when`) reads the fields of the form whose values a table can hold (a choice of its own values or of a rule's values, a number, an amount, a date, yes or no) and the outputs of the rules the request uses: it is lowered to a table of them, which rulec checks."
        )
    } else {
        tr!(
            "経路の条件が読めるのは、表に値を書けるフォームのフィールド（自分の値か規則の値から選ぶ選択、数、金額、日付、真偽）、申請が使う規則の出力、共有の経路の入力です。経路は、それらを列にした表に下ろして、rulec が確かめます。",
            "A condition of the route reads the fields of the form whose values a table can hold (a choice of its own values or of a rule's values, a number, an amount, a date, yes or no), the outputs of the rules the request uses, and the inputs of a shared route: the route is lowered to a table of them, which rulec checks."
        )
    });
    let more = match &u.why {
        U::Requester if !u.form => Some(tr!(
            "申請を出せる人を絞るなら、`ticket` に `submitted by` を書いてください。組織のいちばん上の人の申請をだれかに承認させるなら、上に人がいないときの回し先を書いてください（`who` に `board = team board` と宣言して `when empty escalate, then board`）。",
            "To let only some people submit the request, write `submitted by` under `ticket`. To have someone approve the requests of the person at the top of the organization, write where a stage goes when there is no one above (`when empty escalate, then board`, with `board = team board` in `who`)."
        )),
        U::Field { computed: true, .. } => Some(tr!(
            "フォームが計算するのは、数、金額、日付のフィールドです。`margin : money[JPY] = limit - amount` のようにそのどれかを計算し、条件では `when margin < 0JPY` のように比べてください。",
            "The form computes a field when it is a number, an amount or a date: compute one of those, as in `margin : money[JPY] = limit - amount`, and compare it in the condition, as in `when margin < 0JPY`."
        )),
        U::Field { ty, .. } if ty == "text" || ty == "long text" => Some(tr!("これで分けるなら、{of_ja}が分ける値を並べた `choice` にしてください。", "To decide by it, make it a `choice` of the values the {of_en} tells apart.")),
        U::Two { rewrite: Some((field, cond)), .. } => Some(tr!("差を計算するフィールドを足して、それを値と比べてください（`{field}` と書き、条件は `{cond}`）。", "Add a computed field for the difference, and compare it with a value: `{field}`, then `{cond}`.")),
        U::Computed { rewrite: Some((field, cond)) } => Some(tr!("計算はフォームのフィールドに置いて、そのフィールドを比べてください（`{field}` と書き、条件は `{cond}`）。", "Put the computation in a field of the form, and compare that field: `{field}`, then `{cond}`.")),
        _ => None,
    };
    if let Some(m) = more {
        d = d.note(m);
    }
    d
}

/// The line of the request a diagnostic of the lowered file `f` at `line` is said at: the line the
/// map gives, else the request's first.
fn at(cx: &Ctx, f: &LFile, line: Option<usize>) -> (usize, Text) {
    match line.and_then(|l| f.from(l)) {
        Some(lf) if lf.line > 0 => (lf.line, lf.what.clone()),
        _ => (cx.s.file.name.span.line, Text::default()),
    }
}

fn diag(cx: &Ctx, code: &'static str, error: bool, line: usize, msg: Text) -> Diag {
    let s = cx.s;
    let d = if error { Diag::error(code, s.path, line, 1, msg) } else { Diag::warning(code, s.path, line, 1, msg) };
    d.source(s.src)
}

/// What each lowered file's language said, said in ringi's words.
fn say(cx: &Ctx, lowered: &Lowered, said: &[(usize, Vec<Said>)], out: &mut Vec<Diag>) {
    // a lowered file the others read that does not pass (choices.rule) makes their reading lines
    // fail: those are counted under it
    let choices_fail = said.iter().any(|(i, v)| lowered.files[*i].part == Part::Choices && v.iter().any(Said::is_error));
    // the copy of `form.rule` the generated code reads says again what `form.rule` says: it is
    // heard only when `form.rule` passes, for what the copy has of its own (DESIGN 9.2)
    let form_fails = said.iter().any(|(i, v)| matches!(lowered.files[*i].part, Part::Form | Part::Choices) && v.iter().any(Said::is_error));
    for (i, list) in said {
        let f = &lowered.files[*i];
        if f.part == Part::Values && form_fails {
            continue;
        }
        let mut roots: Vec<Diag> = Vec::new();
        let mut rest: Vec<Diag> = Vec::new();
        let mut following = 0usize;
        let root_codes = ["E101", "E105", "E102"];
        let table_broken = f.part == Part::Route && list.iter().any(|x| x.is_error() && root_codes.contains(&x.code()));
        for x in list {
            // a question rule of E105 is ringi's own question: what rulec warns of it (a row it
            // cannot show reached, W128) is no word on the request; nor is a warning of the copy of
            // `form.rule`, which `form.rule` gives too
            if matches!(f.part, Part::Hidden(_) | Part::Values) && !x.is_error() {
                continue;
            }
            if choices_fail && f.part != Part::Choices && matches!(x.code(), "E066" | "E013") {
                following += 1;
                continue;
            }
            let is_root = f.part == Part::Route && root_codes.contains(&x.code()) && x.is_error();
            if table_broken && !is_root {
                following += 1;
                continue;
            }
            if named_only(lowered, f, x) {
                continue;
            }
            let d = one(cx, lowered, f, x);
            if is_root { roots.push(d) } else { rest.push(d) }
        }
        if following > 0 {
            let target = roots.first_mut().or(rest.first_mut());
            if let Some(d) = target {
                let n = following;
                *d = d.clone().note(tr!("この誤りから出た診断が、ほかに {n} 件あります（例と主張が、この誤りのために成り立たないなど）。この誤りを直してから、もう一度確かめてください。", "{n} more diagnostics follow from this mistake (the examples and the claims that cannot hold because of it); correct it, then check again."));
            }
        }
        out.extend(roots);
        out.extend(rest);
    }
}

/// One diagnostic of a lowered file in ringi's words.
fn one(cx: &Ctx, lowered: &Lowered, f: &LFile, x: &Said) -> Diag {
    let s = cx.s;
    let line = x.en.line;
    let (ln, what) = at(cx, f, line);
    let from = format!("{} {}", tool_word(f.tool), x.code());
    let witness = x.en.json.get("witness").filter(|w| w.as_obj().is_some_and(|o| !o.is_empty())).cloned().or_else(|| x.en.json.get("inputs").map(|i| Json::obj([("inputs", i.clone())])));
    let extra = |rows: Option<Json>| Spaced { witness: witness.clone(), rows, from: Some(from.clone()) };
    let names = lowered.route.clone().unwrap_or_default();
    let inputs_shown = || {
        let inputs = x.en.json.get("witness").and_then(|w| w.get("inputs")).or_else(|| x.en.json.get("inputs")).and_then(|i| i.as_obj()).map(|o| o.to_vec()).unwrap_or_default();
        let vals: ritsu_ports::Values = inputs.iter().map(|(n, j)| (n.clone(), json_value(j))).collect();
        crate::claims::shown_inputs(s, &names, &vals)
    };
    match (f.tool, x.code(), &f.part) {
        (LoweredTool::Rulec, "E101", Part::Route) => e301(cx, &names, x, &inputs_shown(), ln).with(extra(None)),
        (LoweredTool::Rulec, "E105", Part::Route) => {
            let ins = inputs_shown();
            let (ija, ien) = (ins.ja.clone(), ins.en.clone());
            let rows = route_rows(cx, x);
            let lines: Vec<String> = rows.iter().map(|l| l.to_string()).collect();
            let first = rows.first().copied().unwrap_or(ln);
            let ls = lines.join(", ");
            let mut d = diag(cx, "E302", true, first, tr!("経路の表の二つの行に当てはまる申請があります（{ls} 行目）", "Two rows of the route take one request (lines {ls})"));
            if let Some(both) = both_take(cx, &names, x) {
                d = d.note(both);
            }
            d.note(tr!("どちらの行にも当てはまる申請: {ija}", "A request both rows take: {ien}"))
                .note(tr!("どちらの行が正しいかを決め、区切りが重ならないように直してください。", "Decide which row is right, and correct the bounds so that they do not overlap."))
                .with(extra(Some(Json::arr(rows.iter().map(|l| Json::from(*l))))))
        }
        (LoweredTool::Rulec, "E102", Part::Route) => {
            let why = x.title(f);
            let (ja, en) = (why.ja.clone(), why.en.clone());
            diag(cx, "E303", true, ln, tr!("経路の表のこの行には、どの申請も当てはまりません", "No request reaches this row of the route"))
                .note(tr!("rulec の言い方: {ja}", "As rulec says it: {en}"))
                .note(tr!("行の区切りを直すか、行を消してください。", "Correct the row's bounds, or remove the row."))
                .with(extra(None))
        }
        (LoweredTool::Rulec, "W111", Part::Route) if line_text(f, line).trim_start().starts_with("enum ") => {
            let stage = names.stages.iter().find(|(_, o, _)| line_text(f, line).contains(&format!("enum {} ", names.enum_of(o)))).cloned();
            // the values as the rule names them, which the stage's values are looked up by
            let first_note = x.en.json.get("notes").and_then(|j| j.as_arr()).and_then(|a| a.first()).and_then(|n| n.as_str()).unwrap_or_default().to_string();
            let output = stage.as_ref().map(|(_, o, _)| o.clone()).unwrap_or_default();
            let never: Vec<String> = first_note
                .rsplit(": ")
                .next()
                .unwrap_or_default()
                .split(", ")
                .filter(|v| !v.is_empty() && !names.named_only.iter().any(|(o, x)| *o == output && x == v))
                .map(str::to_string)
                .collect();

            let shown: Vec<String> = never.iter().map(|v| stage.as_ref().and_then(|(_, _, vs)| vs.iter().find(|(x, _)| x == v).map(|(_, sh)| sh.clone())).unwrap_or_else(|| v.clone())).collect();
            let st = stage.map(|(n, ..)| n).unwrap_or_default();
            let vs = shown.join(", ");
            diag(cx, "W303", false, ln, tr!("ステップ {st} の `{vs}` には、どの申請も届きません", "No request reaches `{vs}` at the stage {st}"))
                .note(tr!("行やステップの条件を直すか、どの申請も届かない物を経路から消してください。", "Correct the conditions of the rows or of the stage, or take out of the route what no request reaches."))
                .with(extra(None))
        }
        (LoweredTool::Rulec, "E104", _) => {
            let why = x.title(f);
            let (ja, en) = (why.ja.clone(), why.en.clone());
            diag(cx, "E104", true, ln, tr!("丸め方の書かれていない計算です（{ja}）", "A computed amount that does not say how it rounds ({en})")).note(tr!("フィールドに `round down(1JPY)` のように丸め方を書いてください。", "Write how the field rounds, such as `round down(1JPY)`.")).with(extra(None))
        }
        (LoweredTool::Rulec, "E146", Part::Route) => {
            let name = claim_at(cx, ln).unwrap_or_default();
            let why: Vec<Text> = x.notes(f);
            let mut d = diag(cx, "E306", true, ln, tr!("主張 {name} が成り立ちません。そうなる申請がありません", "The claim {name} does not hold: no request comes to it"));
            if let Some(w) = why.first() {
                let (ja, en) = (w.ja.clone(), w.en.clone());
                d = d.note(tr!("rulec の言い方: {ja}", "As rulec says it: {en}"));
            }
            d.note(tr!("経路を直すか、主張を消してください。", "Correct the route, or remove the claim.")).with(extra(None))
        }
        (LoweredTool::Rulec, "E107" | "E019", Part::Route) => {
            let why = x.title(f);
            let (ja, en) = (why.ja.clone(), why.en.clone());
            diag(cx, "E601", true, ln, tr!("経路の例が成り立ちません", "An example of the route does not hold")).note(tr!("rulec の言い方: {ja}", "As rulec says it: {en}")).with(extra(None))
        }
        (LoweredTool::Koyomi, "E301", Part::Terms(_)) if is_reminder_claim(cx, ln) => {
            let why = x.title(f);
            let (ja, en) = (why.ja.clone(), why.en.clone());
            let mut d = diag(cx, "E403", true, ln, tr!("期限より前に来ない催促があります", "A reminder does not come before its deadline")).note(tr!("koyomi の言い方: {ja}", "As koyomi says it: {en}"));
            for n in x.notes(f) {
                let (ja, en) = (n.ja.clone(), n.en.clone());
                d = d.note(tr!("{ja}", "{en}"));
            }
            d.note(tr!("催促の日数を期限より短くしてください。", "Make the reminder come after fewer business days than the deadline.")).with(extra(None))
        }
        // rulec counts the rounding and the list's length as the generated code does: a range of a
        // computed field it finds no value in is E103, as ringi's own check says the ones it
        // finds itself (DESIGN 4.1, 9.2)
        (LoweredTool::Rulec, "E150", Part::Form) if ranged_at(cx, ln).is_some() => {
            let (field, at) = ranged_at(cx, ln).expect("matched above");
            let reach = x.en.json.get("spans").and_then(|j| j.as_arr()).and_then(|a| a.first()).and_then(|j| j.get("label")).and_then(|j| j.as_str()).and_then(|label| reach_as_written(label, field));
            crate::claims::no_value_in_range(s, field, at, reach).with(extra(None))
        }
        // the copy the generated code reads works a computation out before any range holds it: what
        // it reaches past int64 is said as `form.rule`'s E108 is, with how far and what to narrow
        (LoweredTool::Rulec, "E108", Part::Values) if computed_at(cx, ln).is_some() => {
            let field = computed_at(cx, ln).expect("matched above");
            past_int64(cx, f, x, field, line).with(extra(None))
        }
        (LoweredTool::Koyomi, "E203", Part::Terms(_) | Part::FormDates) => {
            let why = x.title(f);
            let (ja, en) = (why.ja.clone(), why.en.clone());
            diag(cx, "E404", true, ln, tr!("営業日で数えた日が、カレンダーのデータの外に出ることがあります", "A day counted in business days can fall outside the calendar's data"))
                .note(tr!("koyomi の言い方: {ja}", "As koyomi says it: {en}"))
                .note(tr!("カレンダーの休みの表を、数えた日が出る年まで足すか、数え始める日付の範囲を狭めてください。", "Extend the calendar's table of closed days to the years the days counted fall in, or narrow the range of the date counted from.")).with(extra(None))
        }
        _ => {
            let why = x.title(f);
            let (ja, en) = (why.ja.clone(), why.en.clone());
            // the lowered file is no file anyone sees, and its path is not said: it is named by
            // its name, which `ritsu ringi extract` writes it by (DESIGN 6.5)
            let name = &f.name;
            let fl = line.unwrap_or(0);
            let tool = tool_word(f.tool);
            let code = x.code().to_string();
            let part = if what.en.is_empty() { tr!("下ろしたファイル", "a lowered file") } else { what.clone() };
            // a name in backquotes at the end is followed by a space, as the Japanese of the suite writes it
            let pja = if part.ja.ends_with('`') { format!("{} ", part.ja) } else { part.ja.clone() };
            let pen = part.en.clone();
            let from = tr!("{tool} の {code}（下ろした `{name}` の {fl} 行目）", "From {tool}: {code} at line {fl} of the lowered `{name}`");
            let mut d = if x.is_error() {
                diag(cx, "E201", true, ln, tr!("{pja}を下ろした先が、{tool} の検査を通りません: {ja}", "What {pen} is lowered to does not pass {tool}'s check: {en}"))
            } else {
                diag(cx, "W201", false, ln, tr!("{pja}を下ろした先に、{tool} が警告を出します: {ja}", "What {pen} is lowered to has a warning from {tool}: {en}"))
            }
            .note(from);
            // a name a target language keeps (rulec's W121): rulec's advice is written in rulec's
            // words, so ringi says how to rename it in the request
            let keyword = f.tool == LoweredTool::Rulec && code == "W121";
            // a note's list (rulec's parts of a gap, the rows to add) is the lines under it, which
            // start with two spaces: they go with it, and are not counted apart
            let mut taken = 0;
            for n in x.notes(f) {
                let under = n.en.starts_with("  ");
                if !under {
                    if taken == if keyword { 1 } else { 3 } {
                        break;
                    }
                    taken += 1;
                } else if taken == 0 {
                    continue;
                }
                let (ja, en) = (n.ja.clone(), n.en.clone());
                d = d.note(tr!("{ja}", "{en}"));
            }
            if keyword {
                d = d.note(tr!(
                    "生成するコードで名前をそのまま使うなら、別の ASCII の名前にしてください（日本語の名前なら、括弧の中の名前です）。",
                    "For the generated code to use the name as it is, choose another ASCII name (for a name in Japanese, the one in brackets)."
                ));
            }
            d.with(extra(None))
        }
    }
}

/// The computed number written on the line `line` of the request.
fn computed_at<'a>(cx: &'a Ctx, line: usize) -> Option<&'a crate::ast::Field> {
    let all = cx.s.fields().iter().flat_map(|f| std::iter::once(f).chain(f.children.iter()));
    all.filter(|f| f.line == line && f.computed.is_some() && matches!(f.ty, crate::ast::TypeExpr::Num(_))).last()
}

/// E201 for a computation the code ringi generates cannot hold in int64 (rulec's E108 of
/// `values.rule`): the field, what it can come to from the ranges of the fields it reads, the
/// fields to narrow, and rulec's code and words.
fn past_int64(cx: &Ctx, f: &LFile, x: &Said, field: &crate::ast::Field, line: Option<usize>) -> Diag {
    use crate::lower::words;
    let s = cx.s;
    let fd = &field.name.text;
    // the end farther from zero
    let abs = |r: ritsu_units::Rat| if r.cmp_to(ritsu_units::Rat::int(0)) == std::cmp::Ordering::Less { ritsu_units::Rat::int(0).sub(r) } else { r };
    let reach = crate::claims::reach_filling(s, field).map(|(lo, hi)| words::written(words::Val::Num(if abs(lo).cmp_to(abs(hi)) == std::cmp::Ordering::Greater { lo } else { hi }), &field.ty));
    let leaves = read_fields(s, field);
    let (lja, len) = (leaves.iter().map(|l| format!("`{l}`")).collect::<Vec<_>>().join("、"), leaves.iter().map(|l| format!("`{l}`")).collect::<Vec<_>>().join(", "));
    let why = x.title(f);
    let (ja, en) = (why.ja.clone(), why.en.clone());
    let headline = match &reach {
        Some(r) => tr!("フィールド `{fd}` の計算を生成するコードのために下ろした先が、rulec の検査を通りません: `{fd}` は読むフィールドの範囲から {r} まで届き、int64 に収まりません", "What the computation of the field `{fd}` is lowered to for the generated code does not pass rulec's check: from the ranges of the fields it reads, `{fd}` can come to {r}, past int64"),
        None => tr!("フィールド `{fd}` の計算を生成するコードのために下ろした先が、rulec の検査を通りません: {ja}", "What the computation of the field `{fd}` is lowered to for the generated code does not pass rulec's check: {en}"),
    };
    let name = &f.name;
    let fl = line.unwrap_or(0);
    diag(cx, "E201", true, field.line, headline)
        .note(tr!(
            "生成するコードは、フォームが計算する値を、各フィールドに書いた範囲と比べる前に求めます。検査と `ritsu ringi run` は、各値を書いた範囲の中で読みます。",
            "The generated code works out what the form computes before it holds each value to the range its field writes; the check and `ritsu ringi run` read each value within that range."
        ))
        .note(tr!("`{fd}` が読むフィールド（{lja}）の範囲を狭めて、届く値を int64 に収めてください。", "Narrow the ranges of the fields `{fd}` reads ({len}) so that what it can come to fits in int64."))
        .note(tr!("rulec の E108（下ろした `{name}` の {fl} 行目）: {ja}", "From rulec: E108 at line {fl} of the lowered `{name}`: {en}"))
}

/// The fields a person fills that a computation reads in the end, through other computations and
/// totals: `lines.quantity`, `lines.price`.
fn read_fields(s: &crate::names::Scope, f: &crate::ast::Field) -> Vec<String> {
    use crate::ast::ExprKind;
    fn walk(s: &crate::names::Scope, row: Option<&crate::ast::Field>, e: &crate::ast::Expr, depth: usize, out: &mut Vec<String>) {
        let mut all = Vec::new();
        e.walk(&mut all);
        for x in all {
            let (g, within): (Option<&crate::ast::Field>, Option<&crate::ast::Field>) = match &x.kind {
                ExprKind::Path(p) if p.len() == 1 => match row.and_then(|r| r.children.iter().find(|c| c.name.text == p[0].text)) {
                    Some(c) => (Some(c), row),
                    None => (s.field(&p[0].text), None),
                },
                ExprKind::Sum(p) if p.len() == 2 => {
                    let rows = s.field(&p[0].text);
                    (rows.and_then(|r| r.children.iter().find(|c| c.name.text == p[1].text)), rows)
                }
                _ => (None, None),
            };
            let Some(g) = g else { continue };
            match &g.computed {
                Some(c) if depth < 32 => walk(s, within, c, depth + 1, out),
                _ => {
                    let n = match within {
                        Some(r) => format!("{}.{}", r.name.text, g.name.text),
                        None => g.name.text.clone(),
                    };
                    if !out.contains(&n) {
                        out.push(n);
                    }
                }
            }
        }
    }
    let row = s.fields().iter().find(|r| r.children.iter().any(|c| c.name.text == f.name.text && c.line == f.line));
    let mut out = Vec::new();
    if let Some(e) = &f.computed {
        walk(s, row, e, 0, &mut out);
    }
    out
}

/// The computed field written on the line `line` of the request that writes a range, and where the
/// range is written.
fn ranged_at<'a>(cx: &'a Ctx, line: usize) -> Option<(&'a crate::ast::Field, crate::ast::Span)> {
    let all = cx.s.fields().iter().flat_map(|f| std::iter::once(f).chain(f.children.iter()));
    all.filter(|f| f.line == line && f.computed.is_some()).find_map(|f| Some((f, f.range.as_ref()?.span)))
}

/// The values rulec says a definition reaches (the mark of its E150, `fee can only be >=0JPY
/// <=30JPY`), as ringi writes values of the field (`>=0JPY <=30JPY`, `<=1_000_000JPY`); None when
/// the mark is not a range of numbers in the field's unit.
fn reach_as_written(label: &str, field: &crate::ast::Field) -> Option<String> {
    use crate::lower::words;
    let crate::ast::TypeExpr::Num(unit) = &field.ty else { return None };
    let ends: Vec<&str> = label.split_whitespace().filter(|w| w.starts_with(['>', '<', '='])).collect();
    if ends.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for e in ends {
        let mark = e.trim_end_matches(|c: char| c != '>' && c != '<' && c != '=');
        let rest = &e[mark.len()..];
        let digits = rest.trim_end_matches(|c: char| !(c.is_ascii_digit()));
        if rest[digits.len()..] != unit.unit {
            return None;
        }
        let (whole, frac) = digits.split_once('.').unwrap_or((digits, ""));
        let (negative, whole) = (whole.starts_with('-'), whole.trim_start_matches('-'));
        if whole.is_empty() || !whole.chars().chain(frac.chars()).all(|c| c.is_ascii_digit()) {
            return None;
        }
        let den = 10i128.checked_pow(frac.len() as u32)?;
        let num = format!("{whole}{frac}").parse::<i128>().ok()?;
        let value = ritsu_units::Rat::checked_new(if negative { -num } else { num }, den)?;
        out.push(format!("{mark}{}", words::written(words::Val::Num(value), &field.ty)));
    }
    Some(out.join(" "))
}

/// Whether rulec's word is that no row gives values only a `possible` or an example names: the
/// claim or the example that does not hold says it (E306, E601).
fn named_only(lowered: &Lowered, f: &LFile, x: &Said) -> bool {
    if !(f.part == Part::Route && x.code() == "W111") {
        return false;
    }
    let line = line_text(f, x.en.line);
    let Some(names) = &lowered.route else { return false };
    let Some((_, output, _)) = names.stages.iter().find(|(_, o, _)| line.trim_start().starts_with(&format!("enum {} ", names.enum_of(o)))) else { return false };
    let note = x.en.json.get("notes").and_then(|j| j.as_arr()).and_then(|a| a.first()).and_then(|n| n.as_str()).unwrap_or_default().to_string();
    let values: Vec<&str> = note.rsplit(": ").next().unwrap_or_default().split(", ").filter(|v| !v.is_empty()).collect();
    !values.is_empty() && values.iter().all(|v| names.named_only.iter().any(|(o, x)| o == output && x == v))
}

/// E201 for the code ringi lays for the request at `path` (its source `src`), at `at`, when kaname
/// says something of it (DESIGN 9.2; [`crate::ports::Engine::glue_refused`]): ringi's bug, not the
/// request's. The notes say kaname's code and the line of the first three things it says, in the
/// request's glue or in the machine of the tickets, without kaname's words, which are of code the
/// person never wrote.
pub(crate) fn glue_bug(path: &str, src: &str, at: crate::ast::Span, said: &[crate::ports::GlueSaid]) -> Diag {
    let mut d = Diag::error("E201", path, at.line, at.col, tr!("この申請のために ringi が書くコードが、kaname の検査を通りません。ringi の不具合です", "The code ringi writes for this request does not pass kaname's check: a bug in ringi")).source(src);
    for s in said.iter().take(3) {
        let code = &s.code;
        let line = s.line.map(|l| l.to_string()).unwrap_or_else(|| "?".to_string());
        d = d.note(match (s.machine, s.not_total) {
            (false, false) => tr!("kaname の {code}（申請のコードの {line} 行目）", "From kaname: {code} at line {line} of the request's code"),
            (true, false) => tr!("kaname の {code}（チケットの機械のコードの {line} 行目）", "From kaname: {code} at line {line} of the machine of the tickets"),
            (false, true) => tr!("申請のコードの {line} 行目の関数が total ではありません（{code}）", "A function at line {line} of the request's code is not total ({code})"),
            (true, true) => tr!("チケットの機械のコードの {line} 行目の関数が total ではありません（{code}）", "A function at line {line} of the machine of the tickets is not total ({code})"),
        });
    }
    if said.len() > 3 {
        let n = said.len() - 3;
        d = d.note(tr!("ほかに {n} 件あります。", "{n} more besides."));
    }
    let from = said.first().filter(|s| !s.not_total).map(|s| format!("kaname {}", s.code));
    d.note(tr!(
        "申請を直す必要はありません。この申請と ritsu の版（`ritsu --version`）を添えて、ringi の不具合として知らせてください。",
        "Nothing in the request is to be corrected for it: report it as a bug in ringi, with this request and the version of ritsu (`ritsu --version`)."
    ))
    .with(Spaced { witness: None, rows: None, from })
}

fn tool_word(t: LoweredTool) -> &'static str {
    match t {
        LoweredTool::Rulec => "rulec",
        LoweredTool::Koyomi => "koyomi",
    }
}

/// The text of the line `line` (from 1) of a lowered file.
fn line_text(f: &LFile, line: Option<usize>) -> String {
    line.and_then(|l| f.text.lines().nth(l.wrapping_sub(1))).unwrap_or_default().to_string()
}

/// The line of the route's table, where a gap in it is said.
fn route_line(cx: &Ctx) -> Option<usize> {
    match &cx.s.file.route {
        Some(Route::Table(t)) => Some(t.line),
        _ => None,
    }
}

/// The request's lines of the rows rulec names (`rows: [{table, row}]`), in the route's table.
fn route_rows(cx: &Ctx, x: &Said) -> Vec<usize> {
    let Some(Route::Table(t)) = &cx.s.file.route else { return Vec::new() };
    x.en.json
        .get("rows")
        .and_then(|r| r.as_arr())
        .map(|a| a.iter().filter_map(|r| r.get("row").and_then(|n| n.as_int())).filter_map(|n| t.rows.get((n as usize).wrapping_sub(1)).map(|r| r.line)).collect())
        .unwrap_or_default()
}

/// The name of the claim at a line of the request.
fn claim_at(cx: &Ctx, line: usize) -> Option<String> {
    cx.s.file.claims.iter().find(|c| c.line == line).map(|c| c.name.text.clone())
}

/// Whether the line is the claim that every stage reminds before it is due.
fn is_reminder_claim(cx: &Ctx, line: usize) -> bool {
    cx.s.file.claims.iter().any(|c| c.line == line && matches!(c.body, crate::ast::ClaimBody::RemindsBeforeDue))
}

/// A value of rulec's JSON as the ports carry it.
fn json_value(j: &Json) -> ritsu_ports::Value {
    match j {
        Json::Bool(b) => ritsu_ports::Value::Bool(*b),
        Json::Int(n) => ritsu_ports::Value::Int(*n),
        Json::Str(t) => ritsu_ports::Value::Enum(t.clone()),
        _ => ritsu_ports::Value::None,
    }
}

/// A row rulec says to add, laid out in the request's table: the same columns, a value of a choice
/// by the name the file writes, `none` an empty cell, an approver as the cell writes it
/// (`dept_head and accounting`). The input cells are rulec's, cut at the bounds the table writes
/// and at the ends of the ranges; the cells of the stages (or of the route) are `?`, which no check
/// passes, for whoever writes the table to answer.
fn ringi_row(cx: &Ctx, names: &lower::RouteNames, row: &str) -> String {
    let s = cx.s;
    let cells: Vec<&str> = row.trim().trim_matches('|').split('|').map(str::trim).collect();
    let Some(Route::Table(t)) = &s.file.route else { return row.to_string() };
    let n_in = t.inputs.len();
    let mut out = Vec::new();
    for (k, c) in cells.iter().enumerate() {
        if k < n_in {
            let field = t.inputs.get(k).filter(|p| p.len() == 1).and_then(|p| s.field(&p[0].text));
            let shown = match field {
                Some(f) if matches!(f.ty, crate::ast::TypeExpr::Choice) => c
                    .split(", ")
                    .map(|v| {
                        let (neg, v) = v.strip_prefix("not: ").map(|r| ("not: ", r)).unwrap_or(("", v));
                        let w = crate::lower::words::choice_written(s, f, v).map(|o| o.name.text.clone()).unwrap_or_else(|| v.to_string());
                        format!("{neg}{w}")
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
                _ => c.to_string(),
            };
            out.push(shown);
        } else {
            let stage = names.stages.get(k - n_in);
            let shown = if *c == "none" {
                String::new()
            } else {
                stage.and_then(|(_, _, vs)| vs.iter().find(|(v, _)| v == c).map(|(_, sh)| sh.clone())).unwrap_or_else(|| c.to_string())
            };
            out.push(shown);
        }
    }
    format!("| {} |", out.join(" | "))
}

/// E301 (DESIGN 6.5): the requests no row of the route's table takes, in the request's words, part
/// by part (from the rows rulec says to add for them), and one of them, which `witness` carries;
/// then the rows to add, laid out in the table, their stages (or their route) left as `?`. rulec
/// lists eight parts at most: the rows close the parts listed, and the next check names the rest.
fn e301(cx: &Ctx, names: &lower::RouteNames, x: &Said, example: &Text, ln: usize) -> Diag {
    let s = cx.s;
    let table = match &s.file.route {
        Some(Route::Table(t)) => Some(t),
        _ => None,
    };
    let chooses = table.is_some_and(|t| t.chooses_route);
    let fix = x.en.json.get("fix").and_then(|j| j.get("text")).and_then(|j| j.as_str()).unwrap_or_default().to_string();
    let rows: Vec<&str> = fix.lines().map(str::trim).filter(|r| !r.is_empty()).collect();
    // after the parts it lists, rulec says how many more there are (`  and 3 more`)
    let more = notes_of(&x.en).iter().any(|n| n.starts_with("  and "));
    let parts: Vec<(Text, bool)> = table.map(|t| rows.iter().map(|r| part_words(cx, t, names, r)).collect()).unwrap_or_default();
    let mut d = diag(cx, "E301", true, route_line(cx).unwrap_or(ln), tr!("経路の表に当てはまらない申請があります", "No row of the route takes some requests"));
    match parts.as_slice() {
        // one request, which the example says
        [(_, true)] if !more => {}
        [(w, false)] if !more => {
            let (ja, en) = (w.ja.clone(), w.en.clone());
            d = d.note(tr!("どの行にも当てはまらない申請の範囲: {ja}", "The requests no row takes: {en}"));
        }
        [] => {}
        _ => {
            let n = parts.len();
            d = d.note(if more {
                tr!("どの行にも当てはまらない申請の範囲（次のほかにもあります）:", "The requests no row takes, in these parts and more:")
            } else {
                tr!("どの行にも当てはまらない申請の範囲（{n} か所）:", "The requests no row takes, in {n} parts:")
            });
            for (w, _) in &parts {
                let (ja, en) = (w.ja.clone(), w.en.clone());
                d = d.note(tr!("  {ja}", "  {en}"));
            }
        }
    }
    let (eja, een) = (example.ja.clone(), example.en.clone());
    d = d.note(tr!("当てはまる行の無い申請: {eja}", "A request no row takes: {een}"));
    let shown: Vec<String> = rows.iter().map(|r| ringi_row(cx, names, r)).collect();
    let (wja, wen) = if chooses {
        ("`?` に、その申請が使う共有の経路（`use route` の別名）を書いてください", "writing in place of `?` the shared route those requests take (the alias of a `use route` line)")
    } else {
        ("`?` に、そのステップに答える承認者（`who` の名前）を書いてください（そのステップを飛ばす申請なら空のセル）", "writing in place of each `?` who answers that stage (a name in `who`; a blank cell where those requests skip it)")
    };
    match shown.as_slice() {
        [] => return d.note(tr!("その申請が当てはまる行を、表に足してください。", "Add a row those requests fall in to the table.")),
        [one] => d = d.note(tr!("表に次の行を足し、{wja}: {one}", "Add this row to the table, {wen}: {one}")),
        many => {
            d = d.note(tr!("表に次の行を足し、{wja}:", "Add these rows to the table, {wen}:"));
            for r in many {
                d = d.note(tr!("  {r}", "  {r}"));
            }
        }
    }
    d.note(match (more, chooses) {
        (false, false) => tr!(
            "セルは、表がすでに書いている区切りと範囲の端だけで切ってあるので、このまま足せば穴は埋まります。行の途中で承認者が変わるなら、そこで行を分けてください。",
            "The cells are cut only at the bounds the table already writes and at the ends of the ranges, so the rows close the gap as they are; where the approvers change inside a row, split it there."
        ),
        (false, true) => tr!(
            "セルは、表がすでに書いている区切りと範囲の端だけで切ってあるので、このまま足せば穴は埋まります。行の途中で経路が変わるなら、そこで行を分けてください。",
            "The cells are cut only at the bounds the table already writes and at the ends of the ranges, so the rows close the gap as they are; where the route changes inside a row, split it there."
        ),
        (true, false) => tr!(
            "セルは、表がすでに書いている区切りと範囲の端だけで切ってあります。埋まるのは挙げた所だけで、残りは次の検査で出ます。行の途中で承認者が変わるなら、そこで行を分けてください。",
            "The cells are cut only at the bounds the table already writes and at the ends of the ranges. The rows close the parts listed, and the next check names the rest; where the approvers change inside a row, split it there."
        ),
        (true, true) => tr!(
            "セルは、表がすでに書いている区切りと範囲の端だけで切ってあります。埋まるのは挙げた所だけで、残りは次の検査で出ます。行の途中で経路が変わるなら、そこで行を分けてください。",
            "The cells are cut only at the bounds the table already writes and at the ends of the ranges. The rows close the parts listed, and the next check names the rest; where the route changes inside a row, split it there."
        ),
    })
    .fix_in_notes(shown.join("\n"))
}

/// The notes of a finding, as its language wrote them.
fn notes_of(f: &Finding) -> Vec<String> {
    f.json.get("notes").and_then(|j| j.as_arr()).map(|a| a.iter().filter_map(|n| n.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

/// What a cell of the route's table takes of its column, read from the lowered table's spelling
/// of it (rulec's rows to add, and the rows ringi lowers): an ordered column between two ends,
/// both in (None where the cell leaves the end to the column's range); some values of a column of
/// values (as the lowered rule names them); or every value.
#[derive(Clone, Debug, PartialEq)]
enum Takes {
    Every,
    Between(Option<ritsu_units::Rat>, Option<ritsu_units::Rat>),
    Values(Vec<String>),
}

impl Takes {
    /// A cell, in the lowered table's spelling (`>100_000JPY <=200_000JPY`, `travel, meals`,
    /// `not: travel`, `none`, `-`), of a column of the kind `kind`: an open end is the next value
    /// on the column's step (`>100_000JPY` is from 100_001JPY).
    fn read(cell: &str, kind: &ColKind) -> Takes {
        use crate::lex::Tok;
        let c = cell.trim();
        if c == "-" || c.is_empty() {
            return Takes::Every;
        }
        match kind {
            ColKind::Ordered { step, ty, .. } if c != "none" => {
                let (lines, _) = crate::lex::lex("cell", c);
                let toks: Vec<&Tok> = lines.iter().flat_map(|l| l.tokens.iter().map(|t| &t.tok)).collect();
                let value = |t: &Tok| match t {
                    Tok::Num(n) => {
                        let e = crate::ast::Expr { kind: crate::ast::ExprKind::Num(n.clone()), span: crate::ast::Span { line: 0, col: 0, end: 0 } };
                        lower::words::constant_in(&e, ty).map(|v| v.rat())
                    }
                    Tok::Date(d) => Some(ritsu_units::Rat::int(*d as i128)),
                    _ => None,
                };
                if let [one] = toks.as_slice() {
                    return value(one).map_or(Takes::Every, |v| Takes::Between(Some(v), Some(v)));
                }
                // the values of the step, counted from 0: the first above `v` (or at it), the last
                // below it (or at it)
                let on = |v: ritsu_units::Rat, up: bool, strict: bool| {
                    let q = v.div(*step);
                    let (fl, exact) = (q.num.div_euclid(q.den), q.num.rem_euclid(q.den) == 0);
                    let k = match (up, strict, exact) {
                        (true, true, _) => fl + 1,
                        (true, false, true) => fl,
                        (true, false, false) => fl + 1,
                        (false, true, true) => fl - 1,
                        (false, _, _) => fl,
                    };
                    ritsu_units::Rat::int(k).mul(*step)
                };
                let (mut lo, mut hi) = (None, None);
                for pair in toks.chunks(2) {
                    let [op, v] = pair else { return Takes::Every };
                    let Some(v) = value(v) else { return Takes::Every };
                    match op {
                        Tok::Ge => lo = Some(on(v, true, false)),
                        Tok::Gt => lo = Some(on(v, true, true)),
                        Tok::Le => hi = Some(on(v, false, false)),
                        Tok::Lt => hi = Some(on(v, false, true)),
                        _ => return Takes::Every,
                    }
                }
                Takes::Between(lo, hi)
            }
            ColKind::Values(all) => {
                let (not, rest) = match c.strip_prefix("not:") {
                    Some(r) => (true, r),
                    None => (false, c),
                };
                let vs: Vec<String> = rest.split(',').map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).collect();
                if not { Takes::Values(all.iter().filter(|v| !vs.contains(v)).cloned().collect()) } else { Takes::Values(vs) }
            }
            _ => Takes::Values(vec![c.to_string()]),
        }
    }

    /// The same with every end and value written out, inside the column: what two cells are
    /// compared by.
    fn whole(&self, kind: &ColKind) -> Takes {
        use std::cmp::Ordering::*;
        let pick = |a: Option<ritsu_units::Rat>, b: Option<ritsu_units::Rat>, o: std::cmp::Ordering| match (a, b) {
            (Some(x), Some(y)) => Some(if x.cmp_to(y) == o { x } else { y }),
            (x, y) => x.or(y),
        };
        match (self, kind) {
            (Takes::Every, ColKind::Ordered { lo, hi, .. }) => Takes::Between(*lo, *hi),
            (Takes::Between(a, b), ColKind::Ordered { lo, hi, .. }) => Takes::Between(pick(*a, *lo, Greater), pick(*b, *hi, Less)),
            (Takes::Every, ColKind::Values(all)) => Takes::Values(all.clone()),
            (Takes::Every, ColKind::Bool) => Takes::Values(vec!["true".into(), "false".into()]),
            (t, _) => t.clone(),
        }
    }

    /// Whether every value it takes, `other` takes too: both written out ([`Takes::whole`]).
    fn within(&self, other: &Takes) -> bool {
        use std::cmp::Ordering::*;
        // an end None is the open end, below (or above) every value
        let lo_ok = |a: &Option<ritsu_units::Rat>, b: &Option<ritsu_units::Rat>| match (a, b) {
            (_, None) => true,
            (None, Some(_)) => false,
            (Some(x), Some(y)) => x.cmp_to(*y) != Less,
        };
        let hi_ok = |a: &Option<ritsu_units::Rat>, b: &Option<ritsu_units::Rat>| match (a, b) {
            (_, None) => true,
            (None, Some(_)) => false,
            (Some(x), Some(y)) => x.cmp_to(*y) != Greater,
        };
        match (self, other) {
            (Takes::Between(a, b), Takes::Between(c, d)) => lo_ok(a, c) && hi_ok(b, d),
            (Takes::Values(a), Takes::Values(b)) => a.iter().all(|v| b.contains(v)),
            (_, Takes::Every) => true,
            _ => false,
        }
    }

    /// What it and `other` both take: both written out ([`Takes::whole`]). None where they take
    /// nothing together.
    fn meet(&self, other: &Takes) -> Option<Takes> {
        use std::cmp::Ordering::*;
        let pick = |a: Option<ritsu_units::Rat>, b: Option<ritsu_units::Rat>, o: std::cmp::Ordering| match (a, b) {
            (Some(x), Some(y)) => Some(if x.cmp_to(y) == o { x } else { y }),
            (x, y) => x.or(y),
        };
        match (self, other) {
            (Takes::Between(a, b), Takes::Between(c, d)) => {
                let (lo, hi) = (pick(*a, *c, Greater), pick(*b, *d, Less));
                match (lo, hi) {
                    (Some(x), Some(y)) if x.cmp_to(y) == Greater => None,
                    _ => Some(Takes::Between(lo, hi)),
                }
            }
            (Takes::Values(a), Takes::Values(b)) => {
                let both: Vec<String> = a.iter().filter(|v| b.contains(v)).cloned().collect();
                (!both.is_empty()).then_some(Takes::Values(both))
            }
            (Takes::Every, t) | (t, Takes::Every) => Some(t.clone()),
            _ => None,
        }
    }
}

/// A column of the route's table as the request writes it (`amount`, `authority.board`): its name,
/// the field it reads (None for a rule's output), and what the lowered rule reads in it.
fn column<'a>(cx: &'a Ctx, names: &'a lower::RouteNames, path: &[crate::ast::Word]) -> Option<(String, Option<&'a crate::ast::Field>, &'a ColKind)> {
    let reads: Vec<String> = path.iter().map(|w| w.text.clone()).collect();
    let (_, _, kind) = names.inputs.iter().find(|i| i.0 == reads)?;
    let field = (path.len() == 1).then(|| cx.s.field(&path[0].text)).flatten();
    Some((reads.join("."), field, kind))
}

/// What a cell takes of its column, in the request's words: the column's name as the request
/// writes it, the values in the field's unit and by the names the file writes (`amount from
/// 100_001JPY to 1_000_000JPY`, `kind one of travel, meals`); and whether it is one value. None
/// for every value of the column.
fn said(s: &crate::names::Scope, name: &str, field: Option<&crate::ast::Field>, kind: &ColKind, t: &Takes) -> Option<(Text, bool)> {
    use std::cmp::Ordering::*;
    let ga = ritsu_base::text::ja_spacing(&format!("{name}が"));
    match (t.whole(kind), kind) {
        (Takes::Between(a, b), ColKind::Ordered { lo, hi, ty, .. }) => {
            let same = |x: Option<ritsu_units::Rat>, y: Option<ritsu_units::Rat>| match (x, y) {
                (Some(x), Some(y)) => x.cmp_to(y) == Equal,
                (x, y) => x.is_none() && y.is_none(),
            };
            if same(a, *lo) && same(b, *hi) {
                return None;
            }
            let day = matches!(ty, crate::ast::TypeExpr::Date);
            let w = |r: ritsu_units::Rat| if day { crate::lex::date_text(r.num as i64) } else { lower::words::written(lower::words::Val::Num(r), ty) };
            Some(match (a, b) {
                (Some(x), Some(y)) if x.cmp_to(y) == Equal => (tr!("{ga} {}", "{name} = {}", w(x)), true),
                (Some(x), Some(y)) => (tr!("{ga} {} から {}", "{name} from {} to {}", w(x), w(y)), false),
                (Some(x), None) if day => (tr!("{ga} {} 以降", "{name} on or after {}", w(x)), false),
                (Some(x), None) => (tr!("{ga} {} 以上", "{name} {} or more", w(x)), false),
                (None, Some(y)) if day => (tr!("{ga} {} 以前", "{name} on or before {}", w(y)), false),
                (None, Some(y)) => (tr!("{ga} {} 以下", "{name} {} or less", w(y)), false),
                (None, None) => return None,
            })
        }
        (Takes::Values(vs), _) => {
            let every = match kind {
                ColKind::Values(all) => all.iter().all(|v| vs.contains(v)) && !vs.iter().any(|v| v == "none"),
                ColKind::Bool => ["true", "false"].iter().all(|b| vs.iter().any(|v| v == b)),
                ColKind::Ordered { .. } => false,
            };
            if every {
                return None;
            }
            let shown: Vec<String> = vs
                .iter()
                .map(|v| match field {
                    Some(f) if matches!(f.ty, crate::ast::TypeExpr::Choice) => crate::lower::words::choice_written(s, f, v).map(|o| o.name.text.clone()).unwrap_or_else(|| v.clone()),
                    _ => v.clone(),
                })
                .collect();
            Some(match shown.as_slice() {
                [one] => (tr!("{ga} {one}", "{name} = {one}"), true),
                // in English the values are joined with `or`, not with the comma the columns are
                [first @ .., last] => (tr!("{ga} {} のどれか", "{name} = {} or {last}", shown.join(", "); first.join(", ")), false),
                [] => return None,
            })
        }
        _ => None,
    }
}

/// A part of the input no row takes, in the request's words, from the row rulec says to add for
/// it: each input cell that is not every value; and whether the part is one request.
fn part_words(cx: &Ctx, t: &crate::ast::RouteTable, names: &lower::RouteNames, row: &str) -> (Text, bool) {
    let cells: Vec<&str> = row.trim().trim_matches('|').split('|').map(str::trim).collect();
    let (mut ja, mut en, mut one) = (Vec::new(), Vec::new(), true);
    for (path, cell) in t.inputs.iter().zip(&cells) {
        let Some((name, field, kind)) = column(cx, names, path) else {
            one = false;
            continue;
        };
        match said(cx.s, &name, field, kind, &Takes::read(cell, kind)) {
            Some((w, single)) => {
                ja.push(w.ja);
                en.push(w.en);
                one &= single;
            }
            None => one = false,
        }
    }
    if en.is_empty() {
        return (tr!("すべての申請", "every request"), false);
    }
    (Text::new(ja.join("、"), en.join(", ")), one)
}

/// E302's note of all the two rows share, as rulec says it of its rows (E105): that the two take
/// the same requests, that one takes every request the other does, or the requests both take, in
/// the request's words. None where both take one request alone, which the example says.
fn both_take(cx: &Ctx, names: &lower::RouteNames, x: &Said) -> Option<Text> {
    let s = cx.s;
    let Some(Route::Table(t)) = &s.file.route else { return None };
    let picked: Vec<&crate::ast::RouteRow> = x.en.json.get("rows").and_then(|r| r.as_arr())?.iter().filter_map(|r| r.get("row").and_then(|n| n.as_int())).filter_map(|n| t.rows.get((n as usize).wrapping_sub(1))).collect();
    let [a, b] = picked.as_slice() else { return None };
    // each column: its name, the field, its kind, and the two cells, written out
    let mut cols = Vec::new();
    for (k, path) in t.inputs.iter().enumerate() {
        let (name, field, kind) = column(cx, names, path)?;
        let cell = |r: &crate::ast::RouteRow| r.tests.get(k).map(|(test, _)| Takes::read(&lower::words::test_text(s, field, test), kind).whole(kind));
        cols.push((name, field, kind, cell(a)?, cell(b)?));
    }
    let (la, lb) = (a.line, b.line);
    match (cols.iter().all(|c| c.3.within(&c.4)), cols.iter().all(|c| c.4.within(&c.3))) {
        (true, true) => return Some(tr!("二つの行は、同じ申請を取ります。", "The two rows take the same requests.")),
        (true, false) => return Some(tr!("{la} 行目に当てはまる申請は、どれも {lb} 行目にも当てはまります。", "Every request line {la} takes, line {lb} takes too.")),
        (false, true) => return Some(tr!("{lb} 行目に当てはまる申請は、どれも {la} 行目にも当てはまります。", "Every request line {lb} takes, line {la} takes too.")),
        (false, false) => {}
    }
    let (mut ja, mut en, mut one) = (Vec::new(), Vec::new(), true);
    for (name, field, kind, ca, cb) in &cols {
        let both = ca.meet(cb)?;
        if let Some((w, single)) = said(s, name, *field, kind, &both) {
            ja.push(w.ja);
            en.push(w.en);
            one &= single;
        } else {
            one = false;
        }
    }
    let (ja, en) = (ja.join("、"), en.join(", "));
    (!one && !en.is_empty()).then(|| tr!("どちらの行にも当てはまる申請の範囲: {ja}", "The requests both rows take: {en}"))
}
