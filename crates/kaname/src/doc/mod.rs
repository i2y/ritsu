//! `kaname doc` (DESIGN 13): the page for people. Two people read it. The person who approves the
//! decisions reads where they are: in the tables the module calls, or in its code. The engineer reads
//! how far each function was checked: total, or checked at some places when it runs, and why. This
//! module puts together what the page says, in one language, as blocks; `markdown.rs` and `html.rs`
//! write the blocks out.
//!
//! What the page says is what the check found of a module whose names and types pass (the analysis
//! of ranges and the effects, with the verdicts of the checks across the borders when whoever draws
//! it has them: `ritsu kaname doc`), what `examples` answer in the reference interpreter, and what
//! the other languages answer through ritsu's ports: the page `rulec doc` draws for a rule the module
//! calls, and the page `koyomi doc` draws for a dates file. The page holds, in order:
//!
//! 1. the head: the `.kn` with its digest, kaname's version, the files the module reads, and what
//!    the check found, in a few sentences;
//! 2. the decisions in tables: each rule and each dates file it calls, with the page its language
//!    draws, and each transfer it builds, with the reasons the book can refuse it with;
//! 3. the functions in three parts: those that are total, those checked at some places when they run
//!    (each place, and why the check could not show it), and those that may not stop;
//! 4. the decisions written in code (W501), or that there are none;
//! 5. the `examples`, with what each answers.

pub mod html;
pub mod markdown;

use crate::check::{Checked, Outcome};
use crate::names::Target;
use crate::ranges::{Outcome as Came, Place, SiteKind};
use crate::suite::Suite;
use crate::value::Program;
use ritsu_base::text::{Lang, Text};
use std::path::Path;

/// An operation of a transfer the module builds, on the page: its name, the transfer, where it is
/// built (the line and the function), and the reasons it can be refused with (`None` when ritsu's
/// X4 has not answered in this run, `Some(None)` when it cannot be decided).
type Operation = (String, crate::ty::TransferId, Vec<(usize, usize)>, Option<Option<Vec<String>>>);

/// A part of the functions: its heading, its id, and which effects it takes.
type Group = (Text, &'static str, fn(&crate::effects::Effect) -> bool);

/// The two forms of the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// What GitHub shows as it is; the pages of the rules and the dates are folded sections.
    Markdown,
    /// One HTML file that reads nothing from outside; the pages of the rules and the dates open
    /// over it.
    Html,
}

/// A piece of the page. Text in a block may hold `code` in backquotes; the writers turn it into
/// their own form.
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    /// A heading of level 2 to 4, and the id the HTML gives it.
    Heading { level: u8, text: String, id: String },
    Para(String),
    /// A paragraph set apart (GitHub's `[!NOTE]`).
    Note(String),
    List(Vec<String>),
    /// A table: its head and its rows.
    Table { head: Vec<String>, rows: Vec<Vec<String>> },
    /// Lines of a file as they are (`kn`, `text`), under a caption.
    Code { lang: &'static str, caption: String, text: String },
    /// The page of a rule or a dates file, by its index in [`Page::embedded`].
    Embed(usize),
}

/// The page of another language that the page holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Embedded {
    /// `rulec` or `koyomi`: the language whose `doc` drew it.
    pub tool: &'static str,
    /// The file, from the root.
    pub file: String,
    /// The page, in the form of the page that holds it, or what the language said when it could
    /// not draw one.
    pub page: Result<String, String>,
}

/// The page: its language, its title, the `.kn` it is drawn from (from the root) with the first
/// sixteen hex digits of its SHA-256, the blocks in order, and the pages of the other languages it
/// holds.
#[derive(Clone, Debug)]
pub struct Page {
    pub lang: Lang,
    pub title: String,
    pub description: Option<String>,
    pub file: String,
    pub sha: String,
    pub blocks: Vec<Block>,
    pub embedded: Vec<Embedded>,
}

/// The page of a module whose names and types pass, drawn with the languages `suite` joins, its
/// paths written from `root`, in `lang`, for `format`. None for a module whose names or types do
/// not pass: it has no analysis to show.
pub fn page(o: &Outcome, suite: &Suite, root: &Path, lang: Lang, format: Format) -> Option<Page> {
    let c = o.analysed.as_ref()?;
    c.analysis.get()?;
    let mut b = Builder { o, c, p: Program::of(c), suite, root, lang, format, blocks: Vec::new(), embedded: Vec::new() };
    b.head();
    b.tables();
    b.functions();
    b.in_code();
    b.examples();
    Some(b.done())
}

/// The page written out in its form.
pub fn write(p: &Page, format: Format) -> String {
    match format {
        Format::Markdown => markdown::write(p),
        Format::Html => html::write(p),
    }
}

/// `s` in backquotes, as code in the page's text.
fn code(s: &str) -> String {
    format!("`{s}`")
}

/// A name as the page shows it: the name, and its alias when the generated code calls it otherwise
/// (`遅れの請求`（`claim`）).
fn named(lang: Lang, n: &crate::ast::Name) -> String {
    match &n.alias {
        Some((a, _)) if *a != n.text => match lang {
            Lang::Ja => format!("{}（{}）", code(&n.text), code(a)),
            Lang::En => format!("{} ({})", code(&n.text), code(a)),
        },
        _ => code(&n.text),
    }
}

/// Items as a sentence lists them: `a と b`, `a、b、c`; `a and b`, `a, b and c`.
fn and_list(lang: Lang, items: &[String]) -> String {
    match (lang, items.len()) {
        (Lang::Ja, 2) => format!("{} と {}", items[0], items[1]),
        (Lang::Ja, _) => items.join("、"),
        (Lang::En, _) => Text::list(&items.iter().map(Text::same).collect::<Vec<_>>()).en,
    }
}

/// The kinds of failure, as code: `` `rule` ``, `` `range` ``.
fn kinds(fails: &std::collections::BTreeSet<crate::ast::FailKind>) -> Vec<String> {
    fails.iter().map(|k| code(k.word())).collect()
}

/// An id for the HTML from a name: its ASCII letters and digits, the rest `-`.
fn id_of(prefix: &str, name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    format!("{prefix}-{}", s.trim_matches('-'))
}

/// The function a place of the analysis is in: the one whose lines hold it.
struct Builder<'a> {
    o: &'a Outcome,
    c: &'a std::rc::Rc<Checked>,
    p: Program,
    suite: &'a Suite,
    root: &'a Path,
    lang: Lang,
    format: Format,
    blocks: Vec<Block>,
    embedded: Vec<Embedded>,
}

impl<'a> Builder<'a> {
    fn done(self) -> Page {
        let m = &self.c.module;
        let title = match &m.name.alias {
            Some((a, _)) if *a != m.name.text => match self.lang {
                Lang::Ja => format!("{}（{a}）", m.name.text),
                Lang::En => format!("{} ({a})", m.name.text),
            },
            _ => m.name.text.clone(),
        };
        Page {
            lang: self.lang,
            title,
            description: m.description.as_ref().map(|(d, _)| d.clone()),
            file: self.shown_from_root(&self.c.canon, &self.c.path),
            sha: ritsu_base::sha256::short(self.c.src.as_bytes()),
            blocks: self.blocks,
            embedded: self.embedded,
        }
    }

    /// `tr!` in the page's language.
    fn t(&self, t: Text) -> String {
        t.get(self.lang).to_string()
    }

    fn ja(&self) -> bool {
        self.lang == Lang::Ja
    }

    fn push(&mut self, b: Block) {
        self.blocks.push(b);
    }

    fn heading(&mut self, level: u8, text: String, id: String) {
        self.push(Block::Heading { level, text, id });
    }

    /// A file as the page names it: from the root, else as given.
    fn shown_from_root(&self, file: &Path, given: &str) -> String {
        ritsu_base::paths::from_root(self.root, file).unwrap_or_else(|| given.to_string())
    }

    /// A description as a sentence: ending with the full stop of the language it is written in. One
    /// with kana or kanji in it is Japanese, whatever it ends with (`…払われていれば none`).
    fn sentence(&self, d: &str) -> String {
        let d = d.trim_end();
        let japanese = d.chars().any(|c| matches!(c, 'ぁ'..='ヿ' | '㐀'..='鿿' | '！'..='ﾟ'));
        if d.ends_with(['.', '!', '?', '。', '！', '？']) {
            d.to_string()
        } else if japanese {
            format!("{d}。")
        } else {
            format!("{d}.")
        }
    }

    /// Sentences one after another: a space between them in English, and in Japanese after one that
    /// ends in English.
    fn sentences(&self, parts: &[String]) -> String {
        let mut s = String::new();
        for p in parts.iter().filter(|p| !p.is_empty()) {
            if !s.is_empty() && (!self.ja() || s.chars().last().is_some_and(|c| c.is_ascii())) {
                s.push(' ');
            }
            s.push_str(p);
        }
        s
    }

    /// "line 23 (`claim`)", for each place a thing is called: the lines and the functions.
    fn lines_of(&self, at: &[(usize, usize)]) -> String {
        let items: Vec<String> = at
            .iter()
            .map(|(line, f)| {
                let fun = code(&self.c.decls.fns[*f].name.text);
                match self.lang {
                    Lang::Ja => format!("{line} 行目（{fun}）"),
                    Lang::En => format!("line {line} ({fun})"),
                }
            })
            .collect();
        and_list(self.lang, &items)
    }


    // -----------------------------------------------------------------------------------------
    // 1. The head

    fn head(&mut self) {
        let m = &self.c.module;
        let version = env!("CARGO_PKG_VERSION");
        let file = self.shown_from_root(&self.c.canon, &self.c.path);
        let sha = ritsu_base::sha256::short(self.c.src.as_bytes());
        let (name, v) = (&m.name.text, &m.version);
        let mut facts = vec![self.t(tr!("ファイル：{}（module {name} {v}、sha256:{sha}）", "File: {} (module {name} {v}, sha256:{sha})", code(&file)))];
        let reads: Vec<String> = m
            .uses
            .iter()
            .zip(&self.c.uses)
            .map(|(u, t)| {
                let f = code(&self.shown_from_root(t.canon(), &u.path));
                let n = code(&u.name);
                match t {
                    Target::Rule { .. } => self.t(tr!("規則 {n}（{f}）", "the rule {n} ({f})")),
                    Target::Dates { .. } => self.t(tr!("日付のファイル {n}（{f}）", "the dates file {n} ({f})")),
                    Target::Book { .. } => self.t(tr!("帳簿 {n}（{f}）", "the book {n} ({f})")),
                    Target::Module(_) => self.t(tr!("モジュール {n}（{f}）", "the module {n} ({f})")),
                }
            })
            .collect();
        facts.push(if reads.is_empty() {
            self.t(tr!("読むファイル：なし", "Reads: nothing"))
        } else {
            let list = if self.ja() { reads.join("、") } else { reads.join("; ") };
            self.t(tr!("読むファイル：{list}", "Reads: {list}"))
        });
        facts.push(self.t(tr!("kaname：{version}", "kaname: {version}")));
        self.push(Block::List(facts));
        self.push(Block::Para(self.t(tr!(
            "上のファイルと、それが読むファイルを kaname {version} で検査して作ったページです。ファイルのハッシュが今のものと違えば、このページは古くなっています。",
            "kaname {version} made this page by checking the file above and the files it reads. If the file's digest is no longer what it says here, the page is out of date."
        ))));
        self.push(Block::Note(self.summary()));
    }

    /// What the check found, in a few sentences.
    fn summary(&self) -> String {
        let stop = if self.ja() { "。" } else { "." };
        let mut out = vec![format!("{}{stop}", self.t(crate::effects::summary(self.c)))];
        let c = self.c.clone();
        let sites = sites_of(&c);
        let count = |pick: fn(&SiteKind) -> Option<String>| -> usize {
            let mut seen: Vec<String> = sites.iter().filter_map(|(_, s)| pick(&s.kind)).collect();
            seen.sort();
            seen.dedup();
            seen.len()
        };
        let rules = count(|k| match k {
            SiteKind::Rule { canon, .. } => Some(canon.to_string_lossy().into_owned()),
            _ => None,
        });
        let dates = count(|k| match k {
            SiteKind::Date { canon, date, .. } => Some(format!("{}#{date}", canon.to_string_lossy())),
            _ => None,
        });
        let transfers = count(|k| match k {
            SiteKind::Transfer { id, .. } => Some(format!("{}#{}.{}", id.book.to_string_lossy(), id.transfer, id.op)),
            _ => None,
        });
        let en = |n: usize, one: &str, many: &str| match n {
            0 => format!("no {one}"),
            1 => format!("1 {one}"),
            _ => format!("{n} {many}"),
        };
        let ja_calls = match (rules, dates) {
            (0, 0) => "規則も koyomi の日付も呼びません。".to_string(),
            (r, 0) => format!("呼ぶのは規則 {r} 個です。"),
            (0, d) => format!("呼ぶのは koyomi の日付 {d} 個です。"),
            (r, d) => format!("呼ぶのは規則 {r} 個と、koyomi の日付 {d} 個です。"),
        };
        let ja_transfers = if transfers == 0 { "振替は組み立てません。".to_string() } else { format!("振替を {transfers} 個組み立てます。") };
        out.push(self.t(Text::new(
            format!("{ja_calls}{ja_transfers}"),
            format!("It calls {} and {}. It builds {}.", en(rules, "rule", "rules"), en(dates, "date of koyomi", "dates of koyomi"), en(transfers, "transfer", "transfers")),
        )));
        let decisions = self.o.diags.iter().filter(|d| d.code == "W501").count();
        out.push(if decisions == 0 {
            self.t(tr!("コードに書いた判断はありません。", "No decision is written in its code."))
        } else {
            self.t(Text::new(
                format!("コードに書いた判断が {decisions} 個あります（下に挙げます）。"),
                if decisions == 1 { "One decision is written in its code; it is listed below.".to_string() } else { format!("{decisions} decisions are written in its code; they are listed below.") },
            ))
        });
        let errors = self.o.diags.iter().filter(|d| d.is_error()).count();
        if errors > 0 {
            out.push(self.t(Text::new(
                format!("このモジュールは `kaname check` を通りません（エラー {errors} 個）。関数がどこで失敗しうるかを、このページで示します。"),
                format!("The module does not pass `kaname check` ({errors} {}); the page shows where its functions can fail.", if errors == 1 { "error" } else { "errors" }),
            )));
        }
        self.sentences(&out)
    }

    // -----------------------------------------------------------------------------------------
    // 2. The decisions in tables

    fn tables(&mut self) {
        self.heading(2, self.t(tr!("表にある判断", "Decisions in tables")), "tables".into());
        let m = &self.c.module;
        let others = m.uses.iter().zip(&self.c.uses).filter(|(_, t)| !matches!(t, Target::Module(_))).count();
        if others == 0 {
            self.push(Block::Para(self.t(tr!(
                "このモジュールは、規則も日付のファイルも帳簿も読みません。計算する値は、どれも引数から来ます。",
                "The module reads no rule, dates file or book: every value it computes comes from its arguments."
            ))));
            return;
        }
        self.push(Block::Para(self.t(tr!(
            "モジュールが使う判断は、読むファイルにあります。規則は rulec が、日付は koyomi が、帳簿は chobo が、それぞれ検査しています。モジュールは、その答えを運んでつなぎます。",
            "The decisions the module uses are in the files it reads, each checked by its own language: the rules by rulec, the dates by koyomi, the books by chobo. The module carries their answers and joins them."
        ))));
        let c = self.c.clone();
        let sites = sites_of(&c);
        let uses: Vec<(crate::ast::Use, Target)> = m.uses.iter().cloned().zip(self.c.uses.iter().cloned()).collect();
        for (u, t) in &uses {
            let n = code(&u.name);
            match t {
                Target::Rule { file, canon, .. } => {
                    let shown = self.shown_from_root(canon, &u.path);
                    let at: Vec<(usize, usize)> = sites.iter().filter(|(_, s)| matches!(&s.kind, SiteKind::Rule { canon: c, .. } if c == canon)).map(|(f, s)| (s.span.line, *f)).collect();
                    self.heading(3, self.t(tr!("規則 {n}", "The rule {n}")), id_of("rule", &u.name));
                    let f = code(&shown);
                    self.push(Block::Para(if at.is_empty() {
                        self.t(tr!("{f}。このモジュールは、この規則を呼んでいません。", "{f}. The module does not call it."))
                    } else {
                        let lines = self.lines_of(&at);
                        self.t(tr!("{f}。{lines}で呼んでいます。", "{f}, called on {lines}."))
                    }));
                    let page = match &self.suite.rules {
                        Some(r) => r.doc(file, &shown, self.format == Format::Html, self.lang).map_err(|s| said(&s, self.lang)),
                        None => Err(self.t(tr!("rulec がつながっていません", "rulec is not joined"))),
                    };
                    self.embedded.push(Embedded { tool: "rulec", file: shown, page });
                    self.push(Block::Embed(self.embedded.len() - 1));
                }
                Target::Dates { file, canon, .. } => {
                    let shown = self.shown_from_root(canon, &u.path);
                    let at: Vec<(String, usize, usize)> = sites
                        .iter()
                        .filter_map(|(f, s)| match &s.kind {
                            SiteKind::Date { canon: c, date, .. } if c == canon => Some((date.clone(), s.span.line, *f)),
                            _ => None,
                        })
                        .collect();
                    self.heading(3, self.t(tr!("日付のファイル {n}", "The dates file {n}")), id_of("dates", &u.name));
                    let f = code(&shown);
                    let mut dates: Vec<String> = at.iter().map(|(d, _, _)| d.clone()).collect();
                    dates.dedup();
                    let mut said_dates = Vec::new();
                    for d in &dates {
                        let lines: Vec<(usize, usize)> = at.iter().filter(|(x, _, _)| x == d).map(|(_, l, f)| (*l, *f)).collect();
                        let (dd, ll) = (code(d), self.lines_of(&lines));
                        said_dates.push(self.t(tr!("日付 {dd} を {ll}で", "the date {dd} on {ll}")));
                    }
                    self.push(Block::Para(if said_dates.is_empty() {
                        self.t(tr!("{f}。このモジュールは、この日付のファイルの日付を呼んでいません。", "{f}. The module calls none of its dates."))
                    } else {
                        let list = and_list(self.lang, &said_dates);
                        self.t(tr!("{f}。{list}呼んでいます。", "{f}: it calls {list}."))
                    }));
                    let page = match &self.suite.dates {
                        Some(d) => d.doc(file, &shown, self.format == Format::Html, self.lang).map_err(|s| said(&s, self.lang)),
                        None => Err(self.t(tr!("koyomi がつながっていません", "koyomi is not joined"))),
                    };
                    self.embedded.push(Embedded { tool: "koyomi", file: shown, page });
                    self.push(Block::Embed(self.embedded.len() - 1));
                }
                Target::Book { canon, .. } => {
                    let shown = self.shown_from_root(canon, &u.path);
                    self.heading(3, self.t(tr!("帳簿 {n}", "The book {n}")), id_of("book", &u.name));
                    let f = code(&shown);
                    self.push(Block::Para(self.t(tr!(
                        "{f}。モジュールは、次の振替を値として組み立てます。振替を当てるのはホストで、kaname は帳簿に書きません。拒否されうる理由は、モジュールが渡す額の範囲で chobo が探したものです。",
                        "{f}. The module builds these transfers as values; the host makes them, and kaname writes no book. The reasons each can be refused with are the ones chobo finds at the amounts the module gives."
                    ))));
                    self.transfers(canon, &sites);
                }
                Target::Module(_) => {}
            }
        }
    }

    /// The table of the transfers of a book the module builds.
    fn transfers(&mut self, book: &Path, sites: &[(usize, &crate::ranges::CallSite)]) {
        let c = self.c.clone();
        let a = c.analysis.get().expect("analysed");
        let mut ops: Vec<Operation> = Vec::new();
        for (f, s) in sites {
            let SiteKind::Transfer { id, name } = &s.kind else { continue };
            if id.book != book {
                continue;
            }
            let reasons = a.fns[*f].refusals.iter().find(|r| r.span == s.span).map(|r| r.reasons.clone());
            match ops.iter_mut().find(|(n, ..)| n == name) {
                Some(op) => {
                    op.2.push((s.span.line, *f));
                    op.3 = match (op.3.take(), reasons) {
                        (Some(Some(mut x)), Some(Some(y))) => {
                            for r in y {
                                if !x.contains(&r) {
                                    x.push(r);
                                }
                            }
                            Some(Some(x))
                        }
                        (Some(None), _) | (_, Some(None)) => Some(None),
                        (x, None) | (None, x) => x,
                    }
                }
                None => ops.push((name.clone(), id.clone(), vec![(s.span.line, *f)], reasons)),
            }
        }
        if ops.is_empty() {
            self.push(Block::Para(self.t(tr!("このモジュールは、この帳簿の振替を組み立てていません。", "The module builds no transfer of this book."))));
            return;
        }
        let head = vec![self.t(tr!("振替", "Transfer")), self.t(tr!("引数", "Arguments")), self.t(tr!("組み立てる行", "Built on")), self.t(tr!("拒否されうる理由", "Can be refused with"))];
        let mut rows = Vec::new();
        for (name, id, at, reasons) in ops {
            let args = match self.p.transfer(&id) {
                Some((_, params)) => params.iter().map(|(n, t)| format!("{}: {}", code(n), self.p.show_ty(self.c, t))).collect::<Vec<_>>().join(", "),
                None => String::new(),
            };
            let refusable = id.op == "do" || id.op == "hold";
            let why = match (refusable, reasons) {
                (false, _) => "-".to_string(),
                (true, Some(Some(rs))) if rs.is_empty() => self.t(tr!("なし", "none")),
                (true, Some(Some(rs))) => rs.iter().map(|r| code(r)).collect::<Vec<_>>().join(", "),
                (true, Some(None)) => self.t(tr!(
                    "決められません（生成する型は、帳簿の境界の理由を全部とります）",
                    "undecided (the type generated for it takes every reason of the book's bounds)"
                )),
                (true, None) => self.t(tr!("この実行では、ritsu の X4 の答えがありません", "ritsu's check X4 has not answered in this run")),
            };
            rows.push(vec![code(&name), args, self.lines_of(&at), why]);
        }
        self.push(Block::Table { head, rows });
    }

    // -----------------------------------------------------------------------------------------
    // 3. The functions

    fn functions(&mut self) {
        self.heading(2, self.t(tr!("関数", "Functions")), "functions".into());
        let a = self.c.analysis.get().expect("analysed");
        let n = self.c.decls.fns.len();
        if n == 0 {
            self.push(Block::Para(self.t(tr!("このモジュールには関数がありません。", "The module has no function."))));
            return;
        }
        self.push(Block::Para(self.t(tr!(
            "関数は、エフェクトで三つに分かれます。total な関数は、宣言した範囲のどの入力にも答えます。実行時に確かめる関数には、検査が示せなかった場所があります。生成したコードはそこで値を確かめ、外れれば、その場所の失敗の種類（`rule` など）で失敗します。止まらないかもしれない関数は、再帰を持ちます。",
            "A function's effect puts it in one of three parts. A total function answers every input in the ranges it declares. A function checked when it runs has places the checks could not show: the generated code checks the value there, and when it is outside, fails with the kind of the place (`rule` and so on). A function that may not stop recurses."
        ))));
        let groups: [Group; 3] = [
            (tr!("total な関数", "Total"), "total", |e| e.total()),
            (tr!("実行時に確かめる関数", "Checked when they run"), "checked", |e| !e.fails.is_empty() && !e.diverges),
            (tr!("止まらないかもしれない関数", "May not stop"), "recursive", |e| e.diverges),
        ];
        for (title, id, pick) in groups {
            self.heading(3, self.t(title), id.into());
            let fs: Vec<usize> = (0..n).filter(|f| pick(&a.fns[*f].effect)).collect();
            if fs.is_empty() {
                self.push(Block::Para(self.t(tr!("ありません。", "None."))));
            }
            for f in fs {
                self.function(f);
            }
        }
    }

    fn function(&mut self, f: usize) {
        let a = self.c.analysis.get().expect("analysed");
        let facts = &a.fns[f];
        let info = &self.c.decls.fns[f];
        let decl = &self.c.module.fns[f];
        self.heading(4, named(self.lang, &info.name), id_of("fn", info.name.ascii()));
        let e = &facts.effect;
        let written = decl.effect.as_ref().map(|w| {
            let ks: std::collections::BTreeSet<crate::ast::FailKind> = w.fails.iter().map(|(k, _)| *k).collect();
            if w.total { "total".to_string() } else { crate::effects::written(&ks, w.diverges) }
        });
        let holds = !self.o.diags.iter().any(|d| d.code == "E401" && d.line == Some(info.line));
        let mut said = Vec::new();
        if e.total() {
            said.push(self.t(tr!("total な関数です。宣言した範囲のどの入力にも答えます。", "Total: it answers every input in the ranges it declares.")));
        } else {
            if !e.fails.is_empty() {
                let ks = and_list(self.lang, &kinds(&e.fails));
                let places = facts.places.iter().filter(|p| p.failing()).count();
                said.push(if places == 0 {
                    self.t(tr!("呼ぶ関数を通して、{ks} で失敗することがあります。", "Through the functions it calls, it can fail with {ks}."))
                } else {
                    self.t(Text::new(
                        format!("実行時に {places} か所を確かめ、{ks} で失敗することがあります。"),
                        format!("It checks {places} {} when it runs, and can fail with {ks}.", if places == 1 { "place" } else { "places" }),
                    ))
                });
            }
            if e.diverges {
                said.push(self.ring(f));
            }
        }
        let waits = facts.places.iter().any(|p| p.outcome == Came::Pending);
        match (&written, holds) {
            (Some(w), true) if waits => {
                let w = code(w);
                said.push(self.t(tr!(
                    "見出しには {w} と書いてあります。成り立つかは、言語をまたぐ確かめの答え（`ritsu kaname doc` が尋ねます）で決まります。",
                    "Its head writes {w}; whether it holds turns on the answers of the checks across the languages, which `ritsu kaname doc` asks."
                )));
            }
            (Some(w), true) => {
                let w = code(w);
                said.push(self.t(tr!("見出しに {w} と書いてあり、kaname がそれを確かめています。", "Its head writes {w}, and kaname holds it to that.")));
            }
            (Some(w), false) => {
                let w = code(w);
                said.push(self.t(tr!("見出しには {w} と書いてありますが、成り立ちません（E401）。", "Its head writes {w}, which does not hold (E401).")));
            }
            (None, _) => said.push(self.t(tr!("見出しにエフェクトは書いてありません。kaname が推論したものです。", "Its head writes no effect: this is what kaname infers."))),
        }
        // an inner function (DESIGN 21.3): a generic one, one that takes functions (26.5, 27.5), a
        // `private` one (28.2)
        let (generic, takes) = (!info.tparams.is_empty(), info.takes_fn());
        if generic {
            said.push(self.t(tr!(
                "総称の関数で、ホストは呼ばず、モジュールの関数が呼びます。呼び出しの答えとエフェクトは、呼ぶところで、渡す値で読み直して決まります。",
                "A generic function: no host calls it, the functions of the modules do, and what a call answers and its effect are read again with the values the call gives."
            )));
        }
        if takes && generic {
            said.push(self.t(tr!("関数を取ります。", "It takes functions.")));
        } else if takes {
            said.push(self.t(tr!(
                "関数を取る関数で、ホストは呼ばず、モジュールの関数が呼びます。呼び出しの答えとエフェクトは、呼ぶところで、渡す値と関数で読み直して決まります。",
                "A function that takes functions: no host calls it, the functions of the modules do, and what a call answers and its effect are read again with the values and the functions the call gives."
            )));
        }
        // what the function given does (DESIGN 27.5): the effect of one given where the type writes none
        // goes on to the caller; one given where it writes one keeps within it
        if takes {
            let through = crate::effects::through(info);
            if through.is_empty() {
                said.push(self.t(tr!("渡す関数は、引数の型に書いたエフェクトに収まります。", "A function given keeps within the effect its parameter's type writes.")));
            } else {
                let names: Vec<Text> = through.iter().map(|n| Text::same(format!("`{n}`"))).collect();
                let shown = Text::list(&names);
                said.push(self.t(tr!("{} に渡した関数のエフェクトは、呼ぶ側に伝わります。", "The effect of a function given to {} goes on to the caller.", shown.ja; shown.en)));
            }
        }
        if info.private {
            said.push(self.t(if generic || takes {
                tr!("`private` で、このモジュールの関数だけが呼びます。", "It is `private`: only the functions of its module call it.")
            } else {
                tr!("`private` の関数で、ホストは呼ばず、このモジュールの関数だけが呼びます。", "A `private` function: no host calls it, only the functions of its module do.")
            }));
        }
        self.push(Block::Para(self.sentences(&said)));
        if let Some((d, _)) = &decl.description {
            self.push(Block::Para(self.sentence(d)));
        }
        let lines: Vec<&str> = self.c.src.lines().collect();
        // to the last line of its answer: a record or a list goes on over lines
        let end = decl.last.max(ritsu_base::definition::block_end(&lines, info.line)).min(lines.len());
        let body = lines.get(info.line.saturating_sub(1)..end).map(|ls| ls.join("\n")).unwrap_or_default();
        self.push(Block::Code { lang: "kn", caption: String::new(), text: body });
        let mut items: Vec<String> = facts.places.iter().filter(|p| p.failing()).map(|p| self.place(p)).collect();
        for k in &facts.calls {
            let callee = a.fns.iter().zip(&self.c.decls.fns).find(|(_, i)| i.name.text == k.function && k.module == self.c.canon).map(|(x, _)| x.effect.clone());
            let other = self.c.world.modules.get(&k.module).and_then(|m| {
                let i = m.decls.fns.iter().position(|i| i.name.text == k.function)?;
                m.analysis.get().map(|a| a.fns[i].effect.clone())
            });
            let Some(effect) = callee.or(other) else { continue };
            if effect.fails.is_empty() || k.module == self.c.canon && e.diverges {
                continue;
            }
            let (line, fun, ks) = (k.line, code(&k.function), and_list(self.lang, &kinds(&effect.fails)));
            items.push(self.t(tr!("{line} 行目: {ks} で失敗することのある関数 {fun} を呼んでいます。", "Line {line}: it calls {fun}, which can fail with {ks}.")));
        }
        if !items.is_empty() {
            self.push(Block::Para(self.t(tr!("失敗しうる場所：", "Where it can fail:"))));
            self.push(Block::List(items));
        }
    }

    /// What says a function may not stop: the ring of calls it is in, or the call that may not stop.
    fn ring(&self, f: usize) -> String {
        let rings = crate::effects::rings(self.c);
        match rings.iter().find(|(fs, recursive)| *recursive && fs.contains(&f)) {
            Some((fs, _)) if fs.len() == 1 => self.t(tr!("止まらないかもしれません。自分を呼んでいます。", "It may not stop: it calls itself.")),
            Some((fs, _)) => {
                let names: Vec<String> = fs.iter().map(|g| code(&self.c.decls.fns[*g].name.text)).collect();
                let list = and_list(self.lang, &names);
                self.t(tr!("止まらないかもしれません。{list} が呼び合っています。", "It may not stop: {list} call each other."))
            }
            None => self.t(tr!("止まらないかもしれません。止まらないかもしれない関数を呼んでいます。", "It may not stop: it calls a function that may not stop.")),
        }
    }

    /// One place the generated code checks when it runs, as an item of the list.
    fn place(&self, p: &Place) -> String {
        let (line, kind) = (p.span.line, code(p.kind.word()));
        let lead = self.t(tr!("{line} 行目（{kind}）", "Line {line} ({kind})"));
        let said = match (&p.outcome, &p.across) {
            (Came::Pending, _) => {
                let x = p.what.across().unwrap_or("");
                self.t(tr!("この実行では、ritsu の {x} の答えがありません。", "ritsu's check {x} has not answered in this run."))
            }
            (_, Some((c, headline))) => self.coded(headline, &format!("ritsu {c}")),
            // no code says it (DESIGN 22.3): what it is, and that the code checks it
            (Came::Unbounded, _) => {
                let head = crate::ranges::unbounded_head(&crate::ranges::place_src(self.c, p.span));
                let parts = vec![self.sentence_of(&head), self.t(tr!("生成したコードが、走らせたときにこの場所で確かめます。", "The generated code checks it there when it runs."))];
                self.sentences(&parts)
            }
            _ => match self.diag_at(p) {
                Some(d) => {
                    let mut parts = vec![self.coded(&d.message, d.code)];
                    if let Some(n) = d.notes.first() {
                        parts.push(n.get(self.lang).to_string());
                    }
                    self.sentences(&parts)
                }
                None => String::new(),
            },
        };
        match self.lang {
            Lang::Ja => format!("{lead}: {said}"),
            Lang::En => format!("{lead}: {said}"),
        }
    }

    /// A headline as a sentence of the page, its stop added.
    fn sentence_of(&self, s: &Text) -> String {
        self.sentence(&ritsu_base::text::spaced(s, self.lang))
    }

    /// A headline as a sentence that ends with the code it is said with: `… cannot be decided
    /// (W301).`, `…決められません（W301）。`
    fn coded(&self, s: &Text, code: &str) -> String {
        let h = ritsu_base::text::spaced(s, self.lang);
        let h = h.trim_end().trim_end_matches(['.', '。']);
        match self.lang {
            Lang::Ja => format!("{h}（{code}）。"),
            Lang::En => format!("{h} ({code})."),
        }
    }

    /// The diagnostic kaname said of a place it holds itself: the one at its line and column, with its
    /// code.
    fn diag_at(&self, p: &Place) -> Option<&crate::diag::Diag> {
        let code = crate::ranges::code_of(p)?;
        self.o.diags.iter().find(|d| d.code == code && d.line == Some(p.span.line) && d.col == Some(p.span.col)).or_else(|| self.o.diags.iter().find(|d| d.code == code && d.line == Some(p.span.line)))
    }

    // -----------------------------------------------------------------------------------------
    // 4. The decisions written in code

    fn in_code(&mut self) {
        self.heading(2, self.t(tr!("コードにある判断", "Decisions in code")), "code".into());
        let found: Vec<String> = self
            .o
            .diags
            .iter()
            .filter(|d| d.code == "W501")
            .map(|d| {
                let line = d.line.unwrap_or(0);
                let m = self.sentence_of(&d.message);
                self.t(tr!("{line} 行目: {m}", "Line {line}: {m}"))
            })
            .collect();
        if found.is_empty() {
            self.push(Block::Para(self.t(tr!("このモジュールの判断は、どれも表にあります。", "The module's decisions are all in tables."))));
            return;
        }
        self.push(Block::Para(self.t(tr!(
            "kaname は次を警告しています（W501）。どれも、比べた値によって業務の定数を選ぶ判断をコードに書いています。表に書けば、完全性と重なりを rulec が確かめ、承認する人がページで読めます。",
            "kaname warns of these (W501). Each writes in code a decision that chooses a constant of the business by the values it compares. Written in a table instead, rulec checks it is complete and without overlap, and the person who approves it reads it on its page."
        ))));
        self.push(Block::List(found));
    }

    // -----------------------------------------------------------------------------------------
    // 5. The examples

    fn examples(&mut self) {
        self.heading(2, self.t(tr!("例", "Examples")), "examples".into());
        let m = &self.c.module;
        if m.examples.is_empty() {
            self.push(Block::Para(self.t(tr!("このモジュールには `examples` がありません。", "The module writes no `examples`."))));
            return;
        }
        self.push(Block::Para(self.t(tr!(
            "`kaname check` は、`examples` の行ごとに、ホストが呼ぶときと同じように関数を参照インタプリタで流し、書いてある答えと比べます。参照インタプリタの答えは次のとおりです。",
            "`kaname check` runs each line of `examples` in the reference interpreter, as a host would call the function, and compares the answer with the one written. The reference interpreter answers these."
        ))));
        let mut lines = Vec::new();
        for x in &m.examples {
            let Some((f, args, came)) = crate::examples::run_one(&self.p, self.suite, self.c, x) else { continue };
            let info = &self.c.decls.fns[f];
            let shown: Vec<String> = info.params.iter().zip(&args).map(|(prm, v)| format!("{}: {}", prm.name.text, self.p.show(self.c, v, &prm.slot.ty))).collect();
            lines.push(format!("{}({})", info.name.text, shown.join(", ")));
            lines.push(match came {
                crate::examples::Came::Answer(v) => format!("  = {}", self.p.show(self.c, &v, &info.ret.ty)),
                crate::examples::Came::Stopped(crate::interp::Stop::Failed(fl)) => format!("  fails {}", fl.kind.word()),
                crate::examples::Came::Stopped(crate::interp::Stop::Diverged(_)) => self.t(tr!("  # 止まりません", "  # does not stop")),
            });
        }
        self.push(Block::Code { lang: "kn", caption: String::new(), text: lines.join("\n") });
    }
}

/// Every call into another language of the module, with the function it is in.
fn sites_of(c: &Checked) -> Vec<(usize, &crate::ranges::CallSite)> {
    let a = c.analysis.get().expect("analysed");
    a.fns.iter().enumerate().flat_map(|(f, x)| x.sites.iter().map(move |s| (f, s))).collect()
}

/// What a port said when it could not draw a page, as lines of text.
fn said(s: &[ritsu_ports::Said], lang: Lang) -> String {
    s.iter()
        .map(|x| match x.line {
            Some(l) => format!("{}:{l}: {}", x.file, x.message.get(lang)),
            None => format!("{}: {}", x.file, x.message.get(lang)),
        })
        .collect::<Vec<_>>()
        .join("\n")
}
