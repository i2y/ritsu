//! `koyomi doc` (DESIGN 7): the page whoever approves a `.cal` reads (accounting, legal, the
//! people who keep the company calendar), as Markdown or as one HTML file.
//!
//! The page is built once, as blocks of text ([`Page`]), from the `.cal` and from what the
//! check found; `markdown.rs` and `html.rs` only draw it. Every sentence on it goes back to
//! a line of the `.cal` (the operations are said in words by `paraphrase.rs`, and nowhere
//! else) or to what the check computed. Nothing on it depends on the day it is made.

pub mod edges;
pub mod html;
pub mod markdown;
pub mod months;

use crate::ast::{Cite, File, RuleKind, Ty};
use crate::calendar::{Calendar, Info, clean};
use crate::check::{Checked, Outcome, Report};
use crate::date::{self, Day};
use crate::diag::{Diag, Step, day_with_weekday};
use ritsu_base::text::{Lang, Text, count};
use crate::interp;
use crate::paraphrase;
use crate::resolve::{CK, Model, ROp};
use crate::sources::{self, Law, Origin};
use months::{Grid, Marks, Ym};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub use edges::{Survey, ja_name, survey};

/// Markdown, which GitHub shows in a pull request, or one HTML file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Html,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    /// `--months`: the months the tables show, instead of the ones the file decides.
    pub months: Option<(Ym, Ym)>,
}

/// The most months a page shows when nothing decides otherwise (DESIGN 7).
pub const MONTHS: i64 = 24;

/// A piece of running text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inline {
    /// Words.
    T(String),
    /// Code: a line of the `.cal`, a name as written.
    C(String),
    /// A sentence from `paraphrase.rs` (DESIGN 7.1).
    Say(String),
    /// Words in bold.
    B(String),
    /// A link: the words and where it goes.
    A(String, String),
}

pub type Para = Vec<Inline>;

fn t(s: impl Into<String>) -> Inline {
    Inline::T(s.into())
}

fn c(s: impl Into<String>) -> Inline {
    Inline::C(s.into())
}

/// An article of a law the `.cal` cites, quoted from its copy.
#[derive(Clone, Debug)]
pub struct Quote {
    pub head: String,
    pub lines: Vec<String>,
}

/// An item of a list, with what is said under it.
#[derive(Clone, Debug, Default)]
pub struct Item {
    pub body: Para,
    pub more: Vec<Para>,
    pub quotes: Vec<Quote>,
}

fn item(body: Para) -> Item {
    Item { body, ..Default::default() }
}

/// What the month tables mark, for the legend.
#[derive(Clone, Copy, Debug, Default)]
pub struct Legend {
    pub closed: bool,
    pub fails: bool,
    pub edges: bool,
    pub unknown: bool,
}

#[derive(Clone, Debug)]
pub enum Block {
    H2(String),
    H3(String),
    P(Para),
    /// The page's verdict, at its top: everything held, or what did not.
    Alert { ok: bool, lines: Vec<Para> },
    Ol(Vec<Item>),
    Ul(Vec<Item>),
    Table { head: Vec<String>, rows: Vec<Vec<Para>> },
    /// A computation, a step a line, as `koyomi check` shows it under a diagnostic.
    Trace(Vec<Step>),
    Quote(Quote),
    /// The files the page was made from, and their digests.
    Stamp(Vec<(String, Para)>),
    Months { grids: Vec<Grid>, legend: Legend },
}

#[derive(Clone, Debug)]
pub struct Page {
    pub lang: Lang,
    pub title: String,
    pub description: Option<String>,
    pub blocks: Vec<Block>,
}

/// The page of a checked file, or the errors that keep it from being made. A file whose
/// claims or examples fail still has a page, which shows where (DESIGN 7); a file with any
/// other error has none: a broken file drawn neatly would mislead.
pub fn page(o: &Outcome, lang: Lang, opts: &Options) -> Result<Page, Vec<Diag>> {
    let blocking: Vec<Diag> = o.diags.iter().filter(|d| d.is_error() && !matches!(d.code, "E301" | "E302" | "E303")).cloned().collect();
    if !blocking.is_empty() {
        return Err(blocking);
    }
    match &o.checked {
        Some(Checked::Calendar(c)) => Ok(calendar_page(c, lang, opts)),
        Some(Checked::Dates(m, rep)) => Ok(dates_page(m, rep, &o.diags, o.ok.as_ref(), lang, opts)),
        None => Err(o.diags.clone()),
    }
}

/// The page, drawn.
pub fn render(p: &Page, f: Format) -> String {
    match f {
        Format::Markdown => markdown::render(p),
        Format::Html => html::render(p),
    }
}

// ── Pieces both pages use ──────────────────────────────────────────────────

fn say(t: Text, lang: Lang) -> String {
    t.get(lang).to_string()
}

/// The comment of a line of the `.cal`, after `#` outside a string.
fn comment_of(src: &str, line: usize) -> Option<String> {
    let l = src.lines().nth(line.checked_sub(1)?)?;
    let mut in_str = false;
    for (i, ch) in l.char_indices() {
        match ch {
            '"' => in_str = !in_str,
            '#' if !in_str => {
                let c = l[i + 1..].trim();
                return (!c.is_empty()).then(|| c.to_string());
            }
            _ => {}
        }
    }
    None
}

/// An operation as the `.cal` writes it, with its comment: `+ 1 day  # 初日は算入しない`.
fn code_of(src: &str, line: usize, text: &str) -> String {
    match comment_of(src, line) {
        Some(cm) => format!("{text}  # {cm}"),
        None => text.to_string(),
    }
}

/// The articles a citation names, quoted from the copies the check read.
fn quotes(laws: &[Law], cite: &Cite, lang: Lang) -> Vec<Quote> {
    let mut out = Vec::new();
    let Some(law) = laws.iter().find(|l| l.name == cite.source) else { return out };
    for (fr, _) in &cite.fragments {
        let Some(p) = law.pins.iter().find(|p| &p.fragment == fr) else { continue };
        let Ok(xml) = ritsu_base::fs::read_to_string(&p.path) else { continue };
        let rev = law.revision.clone().unwrap_or_default();
        let head = match lang {
            Lang::Ja => format!("{} {fr}（e-Gov 法令検索、{} 時点、版 {rev}）", law.name, law.asof),
            Lang::En => format!("{} {fr} (e-Gov, as of {}, revision {rev})", law.name, law.asof),
        };
        out.push(Quote { head, lines: ritsu_base::sources::article_lines(&xml) });
    }
    out
}

/// Words listed in a Japanese sentence: `土曜と日曜`, `月曜、水曜と金曜`.
pub fn ja_list(words: &[String]) -> String {
    match words {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{}と{last}", rest.join("、")),
    }
}

/// The space a Japanese sentence puts before a number that follows a word: `条件は 60 日後まで`,
/// but `条件は同じ日から`.
pub fn digit_space(s: &str) -> &'static str {
    if s.starts_with(|c: char| c.is_ascii_digit()) { " " } else { "" }
}

/// `受領日 689 日のうち 75 日` and `75 of the 689 days of 受領日`; the input combinations when
/// there are integer inputs too.
fn of_total(m: &Model, part: u64, total: u64) -> Text {
    let (p, n) = (count(part), count(total));
    if m.inputs.len() == 1 {
        let din = &m.date_in().name;
        tr!("{din} {n} 日のうち {p} 日", "{p} of the {n} days of {din}")
    } else {
        tr!("{n} 通りのうち {p} 通り", "{p} of the {n} input combinations")
    }
}

/// `受領日 689 日のすべて` and `all 689 days of 受領日`.
fn all_of(m: &Model, total: u64) -> Text {
    let n = count(total);
    if m.inputs.len() == 1 {
        let din = &m.date_in().name;
        tr!("{din} {n} 日のすべて", "all {n} days of {din}")
    } else {
        tr!("{n} 通りのすべて", "all {n} input combinations")
    }
}

/// The inputs as a sentence says them: `受領日 2026-07-21` or `起点 2026-01-22、月数 1`.
fn inputs_text(m: &Model, vals: &[i64]) -> Text {
    let parts: Vec<String> = m.inputs.iter().enumerate().map(|(k, i)| format!("{} {}", i.name, i.show(vals[k]))).collect();
    Text::new(parts.join("、"), parts.join(", "))
}

/// A cell's value: a date with its day of the week, or an integer.
fn value_cell(ty: Ty, v: i64, lang: Lang) -> Para {
    match ty {
        Ty::Date => vec![t(day_with_weekday(Day(v as i32), lang))],
        Ty::Int => vec![t(v.to_string())],
    }
}

/// `2026-12-29〜2027-01-03 の 6 日` / `2026-12-29..2027-01-03, 6 days`.
fn run_text(a: Day, b: Day) -> Text {
    let n = (b.0 - a.0) as i64 + 1;
    tr!("{a}〜{b} の {n} 日", "{a}..{b}, {n} days")
}

fn ym_span(a: Ym, b: Ym) -> Text {
    if a == b { Text::same(a.to_string()) } else { tr!("{a}〜{b}", "{a}..{b}") }
}

const MONTHS_EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// `12 月 29 日` / `29 December`.
fn month_day(m: u32, d: u32) -> Text {
    tr!("{m} 月 {d} 日", "{d} {}", ; MONTHS_EN[(m - 1) as usize])
}

/// What one `closed` or `open` line of a calendar says.
fn rule_sentence(r: &RuleKind) -> Text {
    match r {
        RuleKind::Weekly(ds) => {
            let ja: Vec<String> = ds.iter().map(|d| format!("{}曜", date::WEEKDAY_JA[*d as usize])).collect();
            let en: Vec<Text> = ds.iter().map(|d| Text::same(date::WEEKDAY_EN_LONG[*d as usize])).collect();
            tr!("毎週{}", "every {}", ja_list(&ja); Text::list(&en).en)
        }
        RuleKind::Table(name, _) => tr!("表「{name}」に載っている日", "the days the table {name} lists"),
        RuleKind::Every { from, to, name } => {
            let span = if from == to {
                month_day(from.0, from.1)
            } else {
                let (a, b) = (month_day(from.0, from.1), month_day(to.0, to.1));
                tr!("{}〜{}", "from {} to {}", a.ja, b.ja; a.en, b.en)
            };
            match name {
                Some(n) => tr!("毎年 {}（{n}）", "every year, {} ({n})", span.ja; span.en),
                None => tr!("毎年 {}", "every year, {}", span.ja; span.en),
            }
        }
        RuleKind::Days { from, to, name } => {
            let span = if from == to { Text::same(from.to_string()) } else { tr!("{from}〜{to}", "{from} to {to}") };
            match name {
                Some(n) => tr!("{}（{n}）", "{} ({n})", span.ja; span.en),
                None => span,
            }
        }
        RuleKind::Open { from, to, name } => {
            let span = if from == to { Text::same(from.to_string()) } else { tr!("{from}〜{to}", "{from} to {to}") };
            match name {
                Some(n) => tr!("営業する日: {}（{n}）", "open on {} ({n})", span.ja; span.en),
                None => tr!("営業する日: {}", "open on {}", span.ja; span.en),
            }
        }
    }
}

/// A calendar file as the page reads it: its parsed lines and the laws it cites.
struct CalFile {
    info: Info,
    file: File,
    laws: Vec<Law>,
}

fn read_calendar_files(c: &Calendar) -> Vec<CalFile> {
    let mut out = Vec::new();
    for info in std::iter::once(&c.info).chain(c.used.iter()) {
        let Ok(src) = ritsu_base::fs::read_to_string(&info.path) else { continue };
        let parsed = crate::parse::parse(&info.shown, &src);
        let Some(file) = parsed.file else { continue };
        let dir = info.path.parent().unwrap_or(Path::new("")).to_path_buf();
        let (laws, _) = sources::check_laws(&Origin { file: &file, dir: &dir });
        out.push(CalFile { info: info.clone(), file, laws });
    }
    out
}

/// The rows of the stamp for the calendar files and their sources, paths from `base`.
fn stamp_calendar(cal: &Calendar, base: &Path, include_self: bool, lang: Lang) -> Vec<(String, Para)> {
    let mut rows = Vec::new();
    let infos: Vec<&Info> = if include_self { std::iter::once(&cal.info).chain(cal.used.iter()).collect() } else { cal.used.iter().collect() };
    for i in infos {
        let p = crate::api::relative(base, &i.path);
        let tail = match lang {
            Lang::Ja => format!("（calendar {} v{}、sha256:{}）", i.name, i.version, &i.sha256[..16]),
            Lang::En => format!(" (calendar {} v{}, sha256:{})", i.name, i.version, &i.sha256[..16]),
        };
        rows.push((say(tr!("カレンダー", "Calendar"), lang), vec![c(p), t(tail)]));
    }
    for tb in &cal.tables {
        let p = crate::api::relative(base, &clean(&tb.path));
        let covers = if tb.listed_years { format!("covers listed years = {}..{}", tb.covers.0, tb.covers.1) } else { format!("covers {}..{}", tb.covers.0, tb.covers.1) };
        let mut cell = vec![t(format!("{} = ", tb.name)), c(p)];
        cell.push(t(match (&tb.url, lang) {
            (Some(u), Lang::Ja) => format!("（sha256:{}、{u} の写し、{covers}）", tb.pin),
            (Some(u), Lang::En) => format!(" (sha256:{}, a copy of {u}, {covers})", tb.pin),
            (None, Lang::Ja) => format!("（sha256:{}、{covers}）", tb.pin),
            (None, Lang::En) => format!(" (sha256:{}, {covers})", tb.pin),
        }));
        rows.push((say(tr!("表", "Table"), lang), cell));
    }
    rows
}

fn stamp_laws(laws: &[Law], lang: Lang) -> Vec<(String, Para)> {
    let mut rows = Vec::new();
    for l in laws {
        let pins: Vec<String> = l.pins.iter().map(|p| format!("{} sha256:{}", p.fragment, p.pin)).collect();
        let rev = l.revision.clone().unwrap_or_default();
        let text = match lang {
            Lang::Ja => format!("{} = e-Gov 法令検索の {}、{} 時点（版 {rev}）。{}", l.name, l.id, l.asof, pins.join("、")),
            Lang::En => format!("{} = law {} on e-Gov as of {} (revision {rev}): {}", l.name, l.id, l.asof, pins.join(", ")),
        };
        rows.push((say(tr!("法令", "Law"), lang), vec![t(text)]));
    }
    rows
}

fn made_by(lang: Lang) -> Para {
    let v = crate::api::VERSION;
    vec![t(say(
        tr!(
            "上のファイルを koyomi {v} で検査して作ったページです。ファイルのハッシュが今のものと違えば、このページは古くなっています。",
            "koyomi {v} made this page by checking the files above. If a file's digest is no longer what it says here, the page is out of date."
        ),
        lang,
    ))]
}

/// The calendar's closing lines, file by file, with the laws they cite.
fn rules_blocks(files: &[CalFile], base: &Path, lang: Lang, out: &mut Vec<Block>) {
    let any_open = files.iter().any(|f| f.file.rules.iter().any(|r| matches!(r.what, RuleKind::Open { .. })));
    out.push(Block::P(vec![t(say(
        if any_open {
            tr!(
                "次の日を休みにします。`open` の行の日は、休みの決まりに当たっても営業日です。",
                "These days are closed. A day on an `open` line is a business day even when a closing line names it."
            )
        } else {
            tr!("次の日を休みにします。", "These days are closed.")
        },
        lang,
    ))]));
    for (i, f) in files.iter().enumerate() {
        if files.len() > 1 {
            let p = crate::api::relative(base, &f.info.path);
            let head = match lang {
                Lang::Ja => format!("{}（{p}）", f.info.name),
                Lang::En => format!("{} ({p})", f.info.name),
            };
            let head = if i == 0 { head } else { say(tr!("{head}から読んだ分", "{head}, read with use calendar"), lang) };
            out.push(Block::P(vec![Inline::B(head)]));
        }
        let mut items = Vec::new();
        for r in &f.file.rules {
            let line = f.file.src.lines().nth(r.span.line - 1).unwrap_or("").trim();
            let code = match line.split_once(" @") {
                Some((a, _)) => a.trim_end().to_string(),
                None => line.to_string(),
            };
            let mut it = item(vec![t(say(rule_sentence(&r.what), lang)), t(" "), c(code)]);
            if let Some(ci) = &r.cite {
                it.quotes = quotes(&f.laws, ci, lang);
            }
            items.push(it);
        }
        if items.is_empty() {
            items.push(item(vec![t(say(tr!("休みの決まりはありません", "no closing lines"), lang))]));
        }
        out.push(Block::Ul(items));
    }
}

/// The tables a calendar reads, and the days it knows.
fn sources_blocks(cal: &Calendar, lang: Lang, out: &mut Vec<Block>) {
    let mut items = Vec::new();
    for tb in &cal.tables {
        let rows = count(tb.rows.len() as u64);
        let (a, b) = tb.covers;
        let from = match &tb.url {
            Some(u) => tr!("{u} の写しを、", "a copy of {u}, "),
            None => tr!("", ""),
        };
        let listed = if tb.listed_years { tr!("（行のある最初の年から最後の年まで）", " (from the first year with a row to the last)") } else { tr!("", "") };
        let text = tr!(
            "{}: {rows} 行。{}sha256:{} で固定しています。休みを全部載せているのは {a}〜{b}{} です。",
            "{}: {rows} rows, {}pinned at sha256:{}; it lists every closed day from {a} to {b}{}.",
            tb.name,
            from.ja,
            tb.pin,
            listed.ja;
            tb.name,
            from.en,
            tb.pin,
            listed.en
        );
        items.push(item(vec![t(say(text, lang))]));
    }
    if !items.is_empty() {
        out.push(Block::Ul(items));
    }
    let (lo, hi) = cal.data;
    out.push(Block::P(vec![t(say(
        if cal.tables.is_empty() {
            tr!(
                "表を読まないので、このカレンダーはどの日が休みかを全部知っています（{lo}〜{hi}）。",
                "It reads no table, so it knows every day ({lo}..{hi})."
            )
        } else {
            tr!(
                "このカレンダーが休みかどうかを知っているのは {lo}〜{hi} です。その外の日が営業日かを問う計算は、検査でも生成したコードでも止まります。",
                "The calendar knows {lo}..{hi}. A computation that asks whether a day outside it is a business day stops, in the check and in the generated code alike."
            )
        },
        lang,
    ))]));
}

fn months_block(grids: Vec<Grid>, has_cal: bool) -> Block {
    let mut l = Legend { closed: false, fails: false, edges: false, unknown: false };
    for g in &grids {
        for cell in g.cells() {
            l.closed |= cell.closed && has_cal;
            l.fails |= !cell.fails.is_empty();
            l.edges |= !cell.edges.is_empty();
            l.unknown |= !cell.known;
        }
    }
    Block::Months { grids, legend: l }
}

// ── The page of a dates file ───────────────────────────────────────────────

fn dates_page(m: &Model, rep: &Report, diags: &[Diag], ok: Option<&Text>, lang: Lang, opts: &Options) -> Page {
    let s = |x: Text| say(x, lang);
    let f = &m.file;
    let sv = survey(m, rep);
    let mut b: Vec<Block> = Vec::new();
    let base: PathBuf = clean(m.path.parent().unwrap_or(Path::new("")));
    let file = m.path.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();

    // The heading: what the page was made from.
    let mut stamp = vec![(
        s(tr!("ファイル", "File")),
        vec![c(file.clone()), t(match lang {
            Lang::Ja => format!("（dates {} v{}、sha256:{}）", f.name.text, f.version, &m.sha256[..16]),
            Lang::En => format!(" (dates {} v{}, sha256:{})", f.name.text, f.version, &m.sha256[..16]),
        })],
    )];
    if let Some(cal) = &m.cal {
        stamp.extend(stamp_calendar(cal, &base, true, lang));
        stamp.extend(stamp_laws(&cal.laws, lang));
    }
    stamp.extend(stamp_laws(&m.laws, lang));
    stamp.push(("koyomi".into(), vec![t(crate::api::VERSION)]));
    b.push(Block::Stamp(stamp));
    b.push(Block::P(made_by(lang)));

    // The verdict.
    let failing: Vec<&Diag> = diags.iter().filter(|d| d.is_error()).collect();
    if failing.is_empty() {
        let line = ok.map(|x| ritsu_base::text::spaced(x, lang)).unwrap_or_default();
        b.push(Block::Alert { ok: true, lines: vec![vec![t(format!("{line}{}", if lang == Lang::Ja { "。" } else { "." }))]] });
    } else {
        let mut lines: Vec<Para> = failing.iter().map(|d| vec![t(format!("{}{}", ritsu_base::text::spaced(&d.message, lang), if lang == Lang::Ja { "。" } else { "." }))]).collect();
        if failing.iter().any(|d| d.code != "E303") {
            lines.push(vec![t(s(tr!(
                "成り立たない入力の日は、下の月の表で太字にしてあります。",
                "The input days it fails on are in bold in the month tables below."
            )))]);
        }
        b.push(Block::Alert { ok: false, lines });
    }

    // How the dates are computed.
    b.push(Block::H2(s(tr!("計算のしかた", "How the dates are computed"))));
    b.push(Block::P(vec![
        t(s(tr!(
            "日付ごとに、.cal に書いた操作を上から順に普通の言葉で書きます。後ろのコードは .cal の行そのままです（",
            "Each date, operation by operation from the top, in words. The code after each is the line as the .cal writes it ("
        ))),
        c("#"),
        t(s(tr!(" から後ろはコメント）。", " starts a comment)."))),
    ]));
    let combos = rep.combinations;
    for (k, d) in m.dates.iter().enumerate() {
        let ast = &f.dates[k];
        b.push(Block::H3(if d.alias == d.name { d.name.clone() } else { s(tr!("{}（{}）", "{} ({})", d.name, d.alias)) }));
        if d.ops.is_empty() {
            b.push(Block::P(vec![t(s(tr!("{}と同じ日です。", "The same day as {}.", ja_name(&d.start_name); d.start_name)))]));
        } else {
            b.push(Block::P(vec![t(s(tr!("{}から、次の順に計算します。", "From {}, in this order:", ja_name(&d.start_name); d.start_name)))]));
        }
        let mut items = Vec::new();
        for (i, o) in d.ops.iter().enumerate() {
            let mut it = item(vec![Inline::Say(s(paraphrase::sentence(m, &o.op))), t(" "), c(code_of(&f.src, o.span.line, &o.text))]);
            let or = &rep.ops[k][i];
            match &o.op {
                ROp::Roll(_) => {
                    if or.moved == 0 {
                        it.more.push(vec![t(s(tr!(
                            "範囲のどの入力でも休みに当たらず、日付は動きません。",
                            "No input of the range lands on a closed day here, so the day never moves."
                        )))]);
                    } else {
                        let most = sv.moves.iter().find(|x| x.date == k && x.op == i).and_then(|x| x.most.as_ref()).map(|x| x.0).unwrap_or(0);
                        let part = of_total(m, or.moved, combos);
                        it.more.push(vec![t(s(tr!(
                            "{}で休みに当たり、日付が動きます（いちばん大きく動くのは {most} 日）。",
                            "The day is closed, and moves, on {}; by {most} days at most.",
                            part.ja;
                            part.en
                        )))]);
                    }
                }
                ROp::IfClosed(_) => {
                    let part = of_total(m, or.if_closed, combos);
                    it.more.push(vec![t(s(tr!("{}で休みに当たり、この行が効きます。", "The day is closed, and the line acts, on {}.", part.ja; part.en)))]);
                }
                _ => {}
            }
            if let Some(ci) = &ast.ops[i].cite {
                it.quotes = quotes(&m.laws, ci, lang);
            }
            items.push(it);
        }
        if let (Some((at, sp)), Some(cal)) = (d.at, m.cal.as_ref())
            && cal.offset.is_some()
        {
            let at_text = crate::vectors::at_minutes(at);
            let code = match at {
                crate::ast::At::Time(_) => format!("at {:02}:{:02}", at_text / 60, at_text % 60),
                crate::ast::At::EndOfDay => "at end of day".to_string(),
            };
            let mut it = item(vec![Inline::Say(s(paraphrase::at_sentence(at, cal.offset_text().as_deref()))), t(" "), c(code_of(&f.src, sp.line, &code))]);
            if let Some((_, _, Some(ci))) = &ast.at {
                it.quotes = quotes(&m.laws, ci, lang);
            }
            items.push(it);
        }
        if !items.is_empty() {
            b.push(Block::Ol(items));
        }
        if let Some(ci) = &ast.cite {
            for q in quotes(&m.laws, ci, lang) {
                b.push(Block::Quote(q));
            }
        }
    }

    // What was checked.
    b.push(Block::H2(s(tr!("確かめたこと", "What was checked"))));
    let ranges: Vec<String> = m
        .inputs
        .iter()
        .map(|i| match lang {
            Lang::Ja => format!("{} {}〜{}", i.name, i.show(i.lo), i.show(i.hi)),
            Lang::En => format!("{} ({}..{})", i.name, i.show(i.lo), i.show(i.hi)),
        })
        .collect();
    let n = count(combos);
    b.push(Block::P(vec![t(if m.inputs.len() == 1 {
        s(tr!(
            "{} の {n} 日のすべてで日付を計算し、条件を確かめました。",
            "Every date was computed, and every claim checked, on all {n} days of {}.",
            ranges[0];
            ranges[0]
        ))
    } else {
        s(tr!(
            "{} の {n} 通りのすべてで日付を計算し、条件を確かめました。",
            "Every date was computed, and every claim checked, on all {n} combinations of {}.",
            ranges.join("、");
            Text::list(&ranges.iter().map(|r| Text::same(r.clone())).collect::<Vec<_>>()).en
        ))
    })]));
    if m.claims.is_empty() {
        b.push(Block::P(vec![t(s(tr!("条件は書かれていません。", "The file states no claims.")))]));
    } else {
        let mut rows = Vec::new();
        for (ci, cl) in m.claims.iter().enumerate() {
            let cr = &rep.claims[ci];
            let mut res: Para = Vec::new();
            let mono = matches!(cl.kind, CK::Monotonic(_));
            if cr.fails == 0 {
                let all = if mono { tr!("隣り合う {} 組のすべて", "all {} pairs of adjacent days", count(cr.checked)) } else { all_of(m, cr.checked) };
                res.push(t(s(tr!("成り立つ（{}）", "holds on {}", all.ja; all.en))));
                if let (CK::Compare(a, cmp, bb), Some((_, vals, l, r))) = (&cl.kind, &cr.tightest)
                    && *cmp != crate::ast::Cmp::Eq
                {
                    let who = inputs_text(m, vals);
                    let (is, limit) = crate::check::compare_parts(m, a, *cmp, bb, *l, *r, vals, true);
                    let lim = limit.map(|x| tr!("（条件は{}{}）", " (the claim allows {})", digit_space(&x.ja), x.ja; x.en)).unwrap_or_default();
                    res.push(t(s(tr!(
                        "。余裕がいちばん少ないのは{} のときで、{}{}",
                        ". The least room is at {}, where {}{}",
                        who.ja,
                        is.ja,
                        lim.ja;
                        who.en,
                        is.en,
                        lim.en
                    ))));
                }
            } else {
                let part = if mono {
                    tr!("隣り合う {} 組のうち {} 組", "{} of the {} pairs of adjacent days", count(cr.checked), count(cr.fails); count(cr.fails), count(cr.checked))
                } else {
                    of_total(m, cr.fails, cr.checked)
                };
                res.push(Inline::B(s(tr!("成り立たない", "fails"))));
                res.push(t(s(tr!("（{}）", " on {}", part.ja; part.en))));
                if let (CK::Compare(a, cmp, bb), Some((_, vals, l, r))) = (&cl.kind, &cr.farthest) {
                    let who = inputs_text(m, vals);
                    let (is, limit) = crate::check::compare_parts(m, a, *cmp, bb, *l, *r, vals, true);
                    let lim = limit.map(|x| tr!("（条件は{}{}）", " (the claim allows {})", digit_space(&x.ja), x.ja; x.en)).unwrap_or_default();
                    res.push(t(s(tr!(
                        "。いちばん外れるのは{} のときで、{}{}",
                        ". The farthest is at {}, where {}{}",
                        who.ja,
                        is.ja,
                        lim.ja;
                        who.en,
                        is.en,
                        lim.en
                    ))));
                }
            }
            rows.push(vec![vec![t(cl.name.clone())], vec![c(cl.text.clone())], res]);
        }
        b.push(Block::Table { head: vec![s(tr!("条件", "Claim")), s(tr!("書いてあること", "As written")), s(tr!("結果", "Result"))], rows });
        for (ci, cl) in m.claims.iter().enumerate() {
            if let Some(ac) = &f.claims[ci].cite {
                let qs = quotes(&m.laws, ac, lang);
                if !qs.is_empty() {
                    b.push(Block::P(vec![t(s(tr!("条件「{}」が引いている条文:", "What the claim {} cites:", cl.name)))]));
                    for q in qs {
                        b.push(Block::Quote(q));
                    }
                }
            }
        }
        // Where each claim fails.
        for (ci, cl) in m.claims.iter().enumerate() {
            let cr = &rep.claims[ci];
            if cr.fails == 0 {
                continue;
            }
            b.push(Block::H3(s(tr!("「{}」が成り立たない入力", "Where {} fails", cl.name))));
            b.extend(runs_blocks(m, &cr.runs, cr.more_runs, lang));
            if let Some(d) = diags.iter().find(|d| matches!(d.code, "E301" | "E302") && d.line == Some(cl.span.line))
                && !d.extra.steps.is_empty()
            {
                let head = d.extra.heading.as_ref().map(|h| h.get(lang).to_string()).unwrap_or_default();
                b.push(Block::P(vec![t(format!("{}:", ritsu_base::text::capitalize(&head)))]));
                b.push(Block::Trace(d.extra.steps.clone()));
            }
        }
    }

    // Days a month does not have.
    b.push(Block::H2(s(tr!("無い日の扱い", "Days a month does not have"))));
    if sv.elses.is_empty() {
        b.push(Block::P(vec![t(s(tr!(
            "この決まりには、その月に無い日（2 月 30 日など）に当たりうる操作がありません。",
            "No operation of this file can land on a day its month does not have, such as 30 February."
        )))]));
    } else {
        b.push(Block::P(vec![t(s(tr!(
            "月を足す、何か月後の何日にする、何日で締める、の三つの操作は、その月に無い日（2 月 30 日など）に当たることがあります。そのときどうするかは .cal に書いてあります。範囲のすべての入力について、その扱いを使う数と、ほかの扱いに替えたら結果が変わる数を数えました。",
            "Adding months, taking a day of a month some months away, and closing on a day of the month can land on a day the month does not have, such as 30 February. The .cal says what to do then. For every input of the range, koyomi counted how often that is used, and how often another way would change the result."
        )))]));
        let mut items = Vec::new();
        for e in &sv.elses {
            let d = &m.dates[e.date];
            let o = &d.ops[e.op];
            let mut it = item(vec![c(o.text.clone()), t(s(tr!("（{}）", " ({})", d.name)))]);
            if e.used == 0 {
                it.more.push(vec![t(s(tr!("範囲のどの入力でも使われません。", "No input of the range uses it.")))]);
            } else {
                let part = of_total(m, e.used, combos);
                let how = edges::missing_word(e.how);
                let mut p = vec![t(s(tr!("{}で無い日に当たり、", "It lands on a day the month does not have on {}, and ", part.ja; part.en))), c(format!("else {how}")), t(s(tr!(" を使います。", " is used.")))];
                if let Some((vals, (y, mm, dd), gave)) = &e.first {
                    let who = inputs_text(m, vals);
                    let missing = interp::ymd(*y, *mm, *dd);
                    p.push(t(s(tr!(
                        "最初は{} のときで、{missing} が無いので {gave} にします。",
                        " The first is {}: {missing} does not exist, and {gave} is taken.",
                        who.ja;
                        who.en
                    ))));
                }
                it.more.push(p);
            }
            let mut sw: Para = Vec::new();
            for (p, n) in &e.swaps {
                let word = edges::missing_word(*p);
                if !sw.is_empty() {
                    sw.push(t(s(tr!("", " "))));
                }
                if *p == date::Missing::Reject {
                    let part = of_total(m, *n, combos);
                    sw.push(t(s(tr!("扱いを ", "With "))));
                    sw.push(c(format!("else {word}")));
                    sw.push(t(if *n == 0 {
                        s(tr!(" に替えても、計算が止まる入力はありません。", " instead, no input would be refused."))
                    } else {
                        s(tr!(" に替えると、{}で計算が止まります。", " instead, {} would be refused.", part.ja; part.en))
                    }));
                } else {
                    let part = of_total(m, *n, combos);
                    sw.push(t(s(tr!("扱いを ", "With "))));
                    sw.push(c(format!("else {word}")));
                    sw.push(t(if *n == 0 {
                        s(tr!(" に替えても、{}は変わりません。", " instead, {} would be the same on every input.", ja_name(&d.name); d.name))
                    } else {
                        s(tr!(" に替えると、{}で{}が変わります。", " instead, {} would differ on {}.", part.ja, ja_name(&d.name); d.name, part.en))
                    }));
                }
            }
            if !sw.is_empty() {
                it.more.push(sw);
            }
            items.push(it);
        }
        b.push(Block::Ul(items));
    }

    // Edge cases.
    b.push(Block::H2(s(tr!("エッジケース", "Edge cases"))));
    b.push(Block::P(vec![t(s(tr!(
        "範囲の入力から koyomi が選んだものです。月末の日、休みの日とその前後、無い日に当たる入力、条件の余裕がいちばん少ない入力などを選びます。どの行も、右の列に書いたことに当たる入力のうち、検査が最初に計算したものです。",
        "Inputs koyomi picked from the range: month ends, closed days and the days around them, inputs that land on a missing day, the least room a claim has, and so on. Each row is the first input the check computed of those that do what its last column says."
    )))]));
    let mut head: Vec<String> = m.inputs.iter().map(|i| i.name.clone()).collect();
    head.extend(m.dates.iter().map(|d| d.name.clone()));
    head.push(s(tr!("なぜ選んだか", "Why")));
    let mut rows = Vec::new();
    let mut out = vec![Day(0); m.dates.len()];
    for e in &sv.edges {
        let mut row: Vec<Para> = m.inputs.iter().enumerate().map(|(k, i)| value_cell(i.ty, e.vals[k], lang)).collect();
        let ok = interp::run(m, &e.vals, &mut out, &mut Vec::new()).is_ok();
        for d in &out {
            row.push(if ok { value_cell(Ty::Date, d.0 as i64, lang) } else { vec![t("-")] });
        }
        let why = Text::join(&e.why, "。", "; ");
        row.push(vec![t(why.get(lang))]);
        rows.push(row);
    }
    b.push(Block::Table { head, rows });

    // The examples.
    if !m.examples.is_empty() {
        b.push(Block::H2(s(tr!("例", "Examples"))));
        b.push(Block::P(vec![t(s(tr!(
            ".cal の examples に書いた行と、計算した結果です。",
            "The rows the .cal writes under examples, and what the computation gives."
        )))]));
        let mut head: Vec<String> = m.inputs.iter().map(|i| i.name.clone()).collect();
        head.extend(m.dates.iter().map(|d| format!("→ {}", d.name)));
        head.push(s(tr!("結果", "Result")));
        let mut rows = Vec::new();
        for ex in &m.examples {
            let mut vals: Vec<i64> = m.inputs.iter().map(|i| i.lo).collect();
            let mut outside = false;
            for (k, v) in &ex.inputs {
                vals[*k] = *v;
                outside |= *v < m.inputs[*k].lo || *v > m.inputs[*k].hi;
            }
            let mut row: Vec<Para> = m.inputs.iter().enumerate().map(|(k, i)| value_cell(i.ty, vals[k], lang)).collect();
            let mut want: Vec<Option<Day>> = vec![None; m.dates.len()];
            for (k, d, _) in &ex.outputs {
                want[*k] = Some(*d);
            }
            for w in &want {
                row.push(w.map(|d| value_cell(Ty::Date, d.0 as i64, lang)).unwrap_or_default());
            }
            let verdict = if outside {
                vec![Inline::B(s(tr!("範囲の外", "outside the range")))]
            } else {
                match interp::run(m, &vals, &mut out, &mut Vec::new()) {
                    Err(_) => vec![Inline::B(s(tr!("計算が止まる", "does not compute")))],
                    Ok(()) => {
                        let wrong: Vec<String> = want
                            .iter()
                            .enumerate()
                            .filter_map(|(k, w)| w.filter(|d| *d != out[k]).map(|_| format!("{} {}", m.dates[k].name, out[k])))
                            .collect();
                        if wrong.is_empty() {
                            vec![t(s(tr!("合っている", "matches")))]
                        } else {
                            let list = wrong.join(if lang == Lang::Ja { "、" } else { ", " });
                            vec![Inline::B(s(tr!("違う", "differs"))), t(s(tr!("（計算すると {list}）", " (it computes to {list})")))]
                        }
                    }
                }
            };
            row.push(verdict);
            rows.push(row);
        }
        b.push(Block::Table { head, rows });
    }

    // The calendar, and the months.
    b.push(Block::H2(s(tr!("カレンダー", "Calendar"))));
    let mut marks = Marks::default();
    let (lo_day, hi_day) = {
        let din = m.date_in();
        let mut lo = Day(din.lo as i32);
        let mut hi = Day(din.hi as i32);
        for dr in &rep.dates {
            if let Some(e) = dr.earliest {
                lo = lo.min(e);
            }
            if let Some(l) = dr.latest {
                hi = hi.max(l);
            }
        }
        (lo, hi)
    };
    let all_months = Ym::of(lo_day).through(Ym::of(hi_day));
    let edge_days: Vec<Day> = sv.edges.iter().map(|e| Day(e.vals[m.date_input] as i32)).collect();
    let (shown, left_out): (Vec<Ym>, usize) = match opts.months {
        Some((a, z)) => (a.through(z), 0),
        None if all_months.len() as i64 <= MONTHS => (all_months.clone(), 0),
        None => {
            // The months with an input a claim fails on, and the months of the edge cases:
            // their inputs and the dates they compute, so that the holiday a date moved off
            // is on the page too.
            let mut want: std::collections::BTreeSet<Ym> = edge_days.iter().map(|d| Ym::of(*d)).collect();
            let mut buf = vec![Day(0); m.dates.len()];
            for e in &sv.edges {
                if interp::run(m, &e.vals, &mut buf, &mut Vec::new()).is_ok() {
                    want.extend(buf.iter().map(|d| Ym::of(*d)));
                }
            }
            for cr in &rep.claims {
                for (_, a, z) in &cr.runs {
                    want.extend(Ym::of(*a).through(Ym::of(*z)));
                }
            }
            let v: Vec<Ym> = all_months.iter().copied().filter(|x| want.contains(x)).collect();
            let left = all_months.len() - v.len();
            (v, left)
        }
    };
    if let (Some(first), Some(last)) = (shown.first(), shown.last()) {
        let (from, to) = (first.first(), last.last());
        marks.fails = fail_marks(m, rep, from, to);
        for e in &sv.edges {
            let d = Day(e.vals[m.date_input] as i32);
            if d >= from && d <= to {
                marks.edges.entry(d).or_default().extend(e.why.iter().cloned());
            }
        }
    }
    match &m.cal {
        None => b.push(Block::P(vec![t(s(tr!(
            "このファイルはカレンダーを読まないので、休みの日はありません。",
            "The file reads no calendar, so no day is closed."
        )))])),
        Some(cal) => {
            let files = read_calendar_files(cal);
            let desc = files.first().and_then(|f| f.file.description.clone());
            let shown_path = files.first().map(|f| crate::api::relative(&base, &f.info.path)).unwrap_or_default();
            b.push(Block::P(vec![
                t(s(tr!("このファイルは、カレンダー「{}」v{}（", "The file reads the calendar {} v{} (", cal.info.name, cal.info.version))),
                c(shown_path),
                t(s(tr!("）を読みます。", ")."))),
            ]));
            if let Some(dsc) = desc {
                b.push(Block::P(vec![t(s(tr!("カレンダーの説明: {dsc}", "Its description: {dsc}")))]));
            }
            b.push(Block::H3(s(tr!("休みの決まり", "Closed days"))));
            rules_blocks(&files, &base, lang, &mut b);
            b.push(Block::H3(s(tr!("出典とデータの範囲", "Sources and the days the calendar knows"))));
            sources_blocks(cal, lang, &mut b);
            let (a, z) = (lo_day.max(cal.data.0), hi_day.min(cal.data.1));
            if a <= z
                && let Some((ra, rz)) = cal.longest_closed_run(a, z)
            {
                let din = &m.date_in().name;
                let r = run_text(ra, rz);
                b.push(Block::P(vec![t(s(tr!(
                    "{din}と、そこから計算した日付が入る {a}〜{z} で、いちばん長い連休は {}です。",
                    "In {a}..{z}, the days of {din} and of the dates computed from it, the longest run of closed days is {}.",
                    r.ja;
                    r.en
                )))]));
            }
        }
    }
    b.push(Block::H3(s(tr!("月の表", "Month by month"))));
    let grids: Vec<Grid> = shown.iter().map(|ym| months::grid(*ym, m.cal.as_ref(), &marks)).collect();
    let blk = months_block(grids, m.cal.is_some());
    if left_out > 0 {
        let total = all_months.len();
        let span = ym_span(all_months[0], *all_months.last().unwrap());
        let n = shown.len();
        b.push(Block::P(vec![t(s(tr!(
            "入力と計算した日付が入る月は {} の {total} か月あります。そのうち、成り立たない入力のある月と、エッジケースの入力か計算した日付のある月の {n} か月だけを載せ、{left_out} か月は省きました。",
            "The inputs and the dates computed from them fall in the {total} months of {}. Only the {n} months that hold an input a claim fails on, or an edge case's input or dates, are shown; {left_out} are left out.",
            span.ja;
            span.en
        )))]));
    }
    b.push(blk);

    let title = format!("{} v{}", f.name.text, f.version);
    Page { lang, title, description: f.description.clone(), blocks: b }
}

/// The runs of inputs a claim fails on, for the page: every run, by the integer inputs when
/// there are some.
fn runs_blocks(m: &Model, runs: &[(Vec<i64>, Day, Day)], more: u64, lang: Lang) -> Vec<Block> {
    const SHOWN: usize = 40;
    let span = |a: &Day, b: &Day| -> String {
        let n = (b.0 - a.0) as i64 + 1;
        match (a == b, lang) {
            (true, _) => a.to_string(),
            (false, Lang::Ja) => format!("{a}〜{b}（{n} 日）"),
            (false, Lang::En) => format!("{a}..{b} ({n} days)"),
        }
    };
    let sep = if lang == Lang::Ja { "、" } else { ", " };
    let ints = m.int_inputs();
    let mut out = Vec::new();
    if ints.is_empty() {
        let shown: Vec<String> = runs.iter().take(SHOWN).map(|(_, a, b)| span(a, b)).collect();
        let rest = runs.len().saturating_sub(SHOWN) as u64 + more;
        let mut text = shown.join(sep);
        if rest > 0 {
            text.push_str(&say(tr!("、ほか {rest} か所", ", and {rest} more runs"), lang));
        }
        out.push(Block::P(vec![t(say(tr!("成り立たない日: {text}", "The days it fails on: {text}"), lang))]));
        return out;
    }
    let mut groups: Vec<(Vec<i64>, Vec<String>, u64)> = Vec::new();
    for (ps, a, b) in runs {
        let n = (b.0 - a.0) as u64 + 1;
        match groups.last_mut() {
            Some((p, v, c)) if p == ps => {
                v.push(span(a, b));
                *c += n;
            }
            _ => groups.push((ps.clone(), vec![span(a, b)], n)),
        }
    }
    let mut items = Vec::new();
    let mut left = 0usize;
    for (i, (ps, v, n)) in groups.iter().enumerate() {
        if i >= SHOWN {
            left += 1;
            continue;
        }
        let who: Vec<String> = ints.iter().zip(ps).map(|(k, x)| format!("{} {x}", m.inputs[*k].name)).collect();
        let who = who.join(sep);
        let days = v.join(sep);
        let n = count(*n);
        let unit = if n == "1" { "input" } else { "inputs" };
        items.push(item(vec![t(say(tr!("{who} の {n} 通り: {days}", "{who}, {n} {unit}: {days}"), lang))]));
    }
    out.push(Block::P(vec![t(say(tr!("成り立たない入力（整数の入力の値ごと）:", "The inputs it fails on, by the values of the integer inputs:"), lang))]));
    out.push(Block::Ul(items));
    if left > 0 || more > 0 {
        let rest = left as u64 + more;
        out.push(Block::P(vec![t(say(tr!("ほか {rest} 組は省きました（全部は `koyomi check --format json` で出ます）。", "{rest} more are left out (`koyomi check --format json` lists them all)."), lang))]));
    }
    out
}

/// For each input day in `from..=to`, the claims that fail on it, with the values of the
/// integer inputs they fail for.
fn fail_marks(m: &Model, rep: &Report, from: Day, to: Day) -> BTreeMap<Day, Vec<Text>> {
    let ints = m.int_inputs();
    // For each day, each failing claim with the values of the integer inputs it fails for.
    type ByDay = BTreeMap<Day, Vec<(usize, Vec<Vec<i64>>)>>;
    let mut by_day: ByDay = BTreeMap::new();
    for (ci, cr) in rep.claims.iter().enumerate() {
        for (ps, a, b) in &cr.runs {
            let (a, b) = ((*a).max(from), (*b).min(to));
            let mut d = a;
            while d <= b {
                let v = by_day.entry(d).or_default();
                match v.iter_mut().find(|(c, _)| *c == ci) {
                    Some((_, list)) => {
                        if list.len() < 12 && !list.contains(ps) {
                            list.push(ps.clone());
                        }
                    }
                    None => v.push((ci, vec![ps.clone()])),
                }
                d = Day(d.0 + 1);
            }
        }
    }
    by_day
        .into_iter()
        .map(|(d, v)| {
            let texts = v
                .into_iter()
                .map(|(ci, list)| {
                    let name = &m.claims[ci].name;
                    if ints.is_empty() {
                        tr!("条件「{name}」が成り立たない", "the claim {name} fails")
                    } else {
                        let who: Vec<String> = list
                            .iter()
                            .map(|ps| ints.iter().zip(ps).map(|(k, x)| format!("{} {x}", m.inputs[*k].name)).collect::<Vec<_>>().join(" "))
                            .collect();
                        tr!("条件「{name}」が成り立たない（{}）", "the claim {name} fails ({})", who.join("、"); who.join(", "))
                    }
                })
                .collect();
            (d, texts)
        })
        .collect()
}

// ── The page of a calendar file ───────────────────────────────────────────

fn calendar_page(cal: &Calendar, lang: Lang, opts: &Options) -> Page {
    let s = |x: Text| say(x, lang);
    let mut b: Vec<Block> = Vec::new();
    let base = clean(cal.info.path.parent().unwrap_or(Path::new("")));
    let file = cal.info.path.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
    let files = read_calendar_files(cal);
    let mut stamp = vec![(
        s(tr!("ファイル", "File")),
        vec![c(file), t(match lang {
            Lang::Ja => format!("（calendar {} v{}、sha256:{}）", cal.info.name, cal.info.version, &cal.info.sha256[..16]),
            Lang::En => format!(" (calendar {} v{}, sha256:{})", cal.info.name, cal.info.version, &cal.info.sha256[..16]),
        })],
    )];
    stamp.extend(stamp_calendar(cal, &base, false, lang));
    for f in &files {
        stamp.extend(stamp_laws(&f.laws, lang));
    }
    stamp.push(("koyomi".into(), vec![t(crate::api::VERSION)]));
    b.push(Block::Stamp(stamp));
    b.push(Block::P(made_by(lang)));
    if let Some(off) = cal.offset_text() {
        b.push(Block::P(vec![t(s(tr!(
            "時刻を出すときの UTC オフセットは {off} です。",
            "Times are given at the UTC offset {off}."
        )))]));
    }

    b.push(Block::H2(s(tr!("休みの決まり", "Closed days"))));
    rules_blocks(&files, &base, lang, &mut b);
    b.push(Block::H2(s(tr!("出典とデータの範囲", "Sources and the days the calendar knows"))));
    sources_blocks(cal, lang, &mut b);

    // The months: the last 24 of the data range, or what --months says. A calendar without
    // a table knows every day, and the page does not pick months for it.
    let shown: Vec<Ym> = match opts.months {
        Some((a, z)) => a.through(z),
        None if cal.tables.is_empty() => vec![],
        None => {
            let end = Ym::of(cal.data.1);
            let mut start = Ym::of(cal.data.0);
            while start.count(end) > MONTHS {
                start = start.next();
            }
            start.through(end)
        }
    };
    if shown.is_empty() {
        b.push(Block::H2(s(tr!("月の表", "Month by month"))));
        b.push(Block::P(vec![
            t(s(tr!(
                "表を読まないカレンダーは、どの日のことも知っているので、月の表は月を指定したときだけ出します（",
                "A calendar that reads no table knows every day, so the month tables come only when the months are given ("
            ))),
            c("--months 2026-01..2026-12"),
            t(s(tr!("）。", ")."))),
        ]));
        return Page { lang, title: format!("{} v{}", cal.info.name, cal.info.version), description: files.first().and_then(|f| f.file.description.clone()), blocks: b };
    }
    let (first, last) = (shown[0].first(), shown.last().unwrap().last());

    b.push(Block::H2(s(tr!("営業日の数", "Business days"))));
    let mut rows = Vec::new();
    let (y0, y1) = (shown[0].y, shown.last().unwrap().y);
    for y in y0..=y1 {
        if let Ok(open) = cal.open_days_in_year(y) {
            let days = if date::is_leap(y as i64) { 366 } else { 365 };
            rows.push(vec![vec![t(y.to_string())], vec![t(open.to_string())], vec![t((days - open).to_string())]]);
        }
    }
    if rows.is_empty() {
        b.push(Block::P(vec![t(s(tr!(
            "表の月に、カレンダーが一年まるごと知っている年はありません。",
            "No year of the months shown is one the calendar knows all of."
        )))]));
    } else {
        b.push(Block::Table { head: vec![s(tr!("年", "Year")), s(tr!("営業日", "Business days")), s(tr!("休み", "Closed days"))], rows });
    }
    let (a, z) = (first.max(cal.data.0), last.min(cal.data.1));
    if a <= z
        && let Some((ra, rz)) = cal.longest_closed_run(a, z)
    {
        let span = ym_span(shown[0], *shown.last().unwrap());
        let r = run_text(ra, rz);
        b.push(Block::P(vec![t(s(tr!(
            "{} でいちばん長い連休は {}です。",
            "The longest run of closed days in {} is {}.",
            span.ja,
            r.ja;
            span.en,
            r.en
        )))]));
    }

    b.push(Block::H2(s(tr!("月の表", "Month by month"))));
    let grids: Vec<Grid> = shown.iter().map(|ym| months::grid(*ym, Some(cal), &Marks::default())).collect();
    b.push(months_block(grids, true));
    let desc = files.first().and_then(|f| f.file.description.clone());
    Page { lang, title: format!("{} v{}", cal.info.name, cal.info.version), description: desc, blocks: b }
}
