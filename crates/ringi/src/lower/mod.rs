//! What ringi lowers a request to (DESIGN 6.4): the route's table and the conditions of its stages
//! to `route.rule`, the form's computations and conditions to `form.rule` (a computation of days to
//! `form.cal`, and a copy the generated code reads to `values.rule`), each named table to
//! `<name>.rule`, the deadlines to `terms.cal`, and the values of the choices to `choices.rule`,
//! which the others read. The files are laid beside the request in
//! `<file>.lowered/`, which is not on the disk (the check lays them over the files it reads,
//! [`crate::mapped`]), and each line of them keeps the line of the request it comes from.
//!
//! Names are the ASCII names of the request (DESIGN 3.4), out of the way of the words rulec and
//! koyomi reserve ([`words::Names`]). The side of a condition where it does not hold, and `none` for
//! an empty cell, are written here, so that rulec's check of completeness reads a whole table.

pub mod form;
pub mod glue;
pub mod lines;
pub mod route;
pub mod shape;
pub mod terms;
pub mod words;

use crate::ast::{Expr, ExprKind, Field, TypeExpr, UseKind};
use crate::names::{Got, Scope};
use ritsu_base::text::Text;
use ritsu_ports::{LineFrom, LoweredTool, RuleFacts};
use std::path::{Path, PathBuf};

/// What a lowered file is in the request: what decides how its diagnostics are said again.
#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    /// The values of the choices.
    Choices,
    /// The form's computations and conditions.
    Form,
    /// The copy of the form's computations and conditions the code ringi generates reads
    /// (`values.rule`, [`form::values`]): what the check and the reference interpreter read is
    /// `Form`.
    Values,
    /// The form's computations of days.
    FormDates,
    /// A named table, by its name.
    Table(String),
    /// The route.
    Route,
    /// The deadlines counted in a calendar, by the calendar's alias.
    Terms(String),
    /// The question whether a required field may be hidden (E105), by the field: not a part of the
    /// request, and not written out by `extract`.
    Hidden(String),
}

/// One lowered file.
#[derive(Clone, Debug)]
pub struct LFile {
    /// Its name in the lowered directory (`route.rule`).
    pub name: String,
    pub tool: LoweredTool,
    pub part: Part,
    pub text: String,
    pub lines: Vec<LineFrom>,
    /// Each name the file gives that is not the request's (`days_value` for `days`, a word koyomi
    /// reserves), with the request's: what the file's language says of it is said with the
    /// request's names ([`LFile::named_back`]).
    pub renamed: Vec<(String, String)>,
    /// Each input of a lowered rule of the form that says whether a field is filled in (a condition's
    /// `cover is none`), by its name in the file, with the field's ASCII name: what the reader of the
    /// file gives as yes or no, from the values of the form.
    pub given: Vec<(String, String)>,
    /// For the lowered rule of the form and its copy (`form.rule`, `values.rule`), what each output
    /// reads, by the file's names, through the names it is worked out from: the inputs, and the
    /// fields of the elements (`lines.price`). What the screens give each output of `values.rule`
    /// apart (DESIGN 13.7). Empty for every other file.
    pub reads: Vec<(String, Vec<String>)>,
}

impl LFile {
    /// A text the file's language wrote, each name the file renamed written as the request names
    /// it: a whole name only, not a part of a longer one.
    pub fn named_back(&self, t: &str) -> String {
        let mut out = t.to_string();
        for (given, want) in &self.renamed {
            out = replace_name(&out, given, want);
        }
        out
    }

    /// Where the line `n` (from 1) of the file comes from: the line of the request (0 for none) and
    /// what it is there.
    pub fn from(&self, n: usize) -> Option<&LineFrom> {
        lines::Out::from_line(&self.lines, n)
    }
}

/// What the route's rule names each thing of the request by: each input with what it reads (a
/// field, a field's presence `reviewer?`, a rule's output `authority.board`) and its column, and
/// each stage's output with its values and how each reads.
#[derive(Clone, Debug, Default)]
pub struct RouteNames {
    /// Each column the route reads, by what it reads in the request (`amount`, `reviewer?`,
    /// `authority.board`), with its name in the file and its kind.
    pub inputs: Vec<(Vec<String>, String, words::ColKind)>,
    /// The columns of `inputs` the route works out itself, which it does not take: the outputs of a
    /// rule it brings in with `apply` (DESIGN 6.4).
    pub worked: Vec<String>,
    pub stages: Vec<(String, String, Vec<(String, String)>)>,
    /// Each value only a `possible` or an example names, by its output.
    pub named_only: Vec<(String, String)>,
    /// The enum of each stage's output that has values, by the output: `<output>_by`, unless an enum
    /// the rule imports has that name (a choice `first_by`), when `_value` follows it.
    pub enums: Vec<(String, String)>,
    /// The rules the route brings in with `apply`, by the alias the request reads them as
    /// (`authority`). Their outputs are columns of the route's rule, not `fed by` inputs, so the
    /// enum of one is not written in `choices.rule` (DESIGN 6.4).
    pub applied: Vec<String>,
}

impl RouteNames {
    /// The enum of the stage's output `output`.
    pub fn enum_of(&self, output: &str) -> String {
        self.enums.iter().find(|(o, _)| o == output).map(|(_, e)| e.clone()).unwrap_or_else(|| format!("{output}_by"))
    }
}

/// A request, lowered.
#[derive(Clone, Debug)]
pub struct Lowered {
    /// The lowered directory, beside the request, as the run writes the request's path
    /// (`examples/expense.lowered`).
    pub dir: PathBuf,
    pub files: Vec<LFile>,
    /// What `route.rule` names things by, when there is one.
    pub route: Option<RouteNames>,
    /// The files laid over the files the program reads ([`Lowered::laid`]).
    laid: Laid,
}

/// The lowered files laid over the files the program reads, made the first time they are asked for.
#[derive(Clone, Default)]
struct Laid(std::cell::OnceCell<std::rc::Rc<dyn ritsu_base::fs::Files>>);

impl std::fmt::Debug for Laid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.get().is_some() { "Laid" } else { "Laid(not yet)" })
    }
}

impl Lowered {
    pub fn path(&self, f: &LFile) -> PathBuf {
        self.dir.join(&f.name)
    }

    /// The lowered files laid over the files the program reads, where the request's lowered
    /// directory would be (`ritsu_base::fs::Overlay`): made over the files the thread reads the
    /// first time they are asked for, and the same files after. Every question the run asks of
    /// them (the check, the claims, the ticket machine, `simulate`, the glue) is asked of one set
    /// of files, by which the languages keep what they read (ritsu's DESIGN 4.15): a lowered file
    /// is checked once in the run.
    pub fn laid(&self) -> std::rc::Rc<dyn ritsu_base::fs::Files> {
        self.laid
            .0
            .get_or_init(|| {
                let mut ov = ritsu_base::fs::Overlay::new(ritsu_base::fs::current());
                for f in &self.files {
                    ov.put(&ritsu_base::paths::opened(&self.path(f)), f.text.clone());
                }
                std::rc::Rc::new(ov)
            })
            .clone()
    }

    /// `f`, with the lowered files laid where the languages read them ([`Lowered::laid`]).
    pub fn fs<R>(&self, f: impl FnOnce() -> R) -> R {
        ritsu_base::fs::with(self.laid(), f)
    }

    /// The request lowered the same, with the lines `drops` left out (each a file's index and its
    /// lines, from 1) and the map of lines with them, to be laid anew: the rows rulec shows no
    /// request reaches in a table ringi cuts a condition into ([`crate::mapped`], DESIGN 6.4).
    pub fn without_lines(&self, drops: &[(usize, Vec<usize>)]) -> Lowered {
        let mut files = self.files.clone();
        for (i, lines) in drops {
            let Some(f) = files.get_mut(*i) else { continue };
            if f.text.lines().count() != f.lines.len() {
                continue;
            }
            let mut text = String::new();
            let mut map = Vec::new();
            for (n, (l, from)) in f.text.lines().zip(f.lines.iter()).enumerate() {
                if lines.contains(&(n + 1)) {
                    continue;
                }
                text.push_str(l);
                text.push('\n');
                map.push(from.clone());
            }
            f.text = text;
            f.lines = map;
        }
        Lowered { dir: self.dir.clone(), files, route: self.route.clone(), laid: Laid::default() }
    }

    /// The files as the port hands them over (`Requests::lowered`): the parts of the request, not
    /// the questions.
    pub fn handed(&self) -> Vec<ritsu_ports::LoweredFile> {
        self.files
            .iter()
            .filter(|f| !matches!(f.part, Part::Hidden(_)))
            .map(|f| ritsu_ports::LoweredFile { path: self.path(f), tool: f.tool, text: f.text.clone(), lines: f.lines.clone() })
            .collect()
    }
}

/// The files of the request as `ringi extract` writes them into the directory `out`: the parts of
/// the request (not the questions), by their names, each path a line names to a file outside the
/// lowered directory written from `out` instead (`use calendar`, `apply`, `fed by`), so that the files
/// written read what the laid ones read.
pub fn extracted(l: &Lowered, out: &Path) -> Vec<(String, String)> {
    let from = ritsu_base::paths::opened(&l.dir);
    let to = ritsu_base::paths::opened(out);
    l.files
        .iter()
        .filter(|f| !matches!(f.part, Part::Hidden(_)))
        .map(|f| {
            let text: String = f.text.lines().map(|line| format!("{}\n", repath(line, &from, &to))).collect();
            (f.name.clone(), text)
        })
        .collect()
}

/// A line of a lowered file with each path it names written from `to` instead of `from`: only the
/// lines that name a file, and a path to another lowered file (`choices.rule`) as it is.
fn repath(line: &str, from: &Path, to: &Path) -> String {
    let t = line.trim_start();
    if !(t.starts_with("use calendar ") || t.starts_with("import enum ") || t.starts_with("apply ") || t.contains(" fed by ")) {
        return line.to_string();
    }
    let mut out = String::new();
    let mut rest = line;
    while let Some(a) = rest.find('"') {
        out.push_str(&rest[..=a]);
        let after = &rest[a + 1..];
        let Some(b) = after.find('"') else {
            out.push_str(after);
            return out;
        };
        let p = &after[..b];
        let target = ritsu_base::paths::opened(&from.join(p));
        let moved = if p.starts_with('@') || ritsu_base::paths::is_absolute(p) || target.starts_with(from) { p.to_string() } else { ritsu_base::paths::between(to, &target) };
        out.push_str(&moved);
        out.push('"');
        rest = &after[b + 1..];
    }
    out.push_str(rest);
    out
}

/// `text` with each whole `name` in it (not a part of a longer name) replaced by `with`.
fn replace_name(text: &str, name: &str, with: &str) -> String {
    let is_name = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(name) {
        let before = rest[..i].chars().next_back();
        let after = rest[i + name.len()..].chars().next();
        out.push_str(&rest[..i]);
        if before.is_some_and(is_name) || after.is_some_and(is_name) {
            out.push_str(name);
        } else {
            out.push_str(with);
        }
        rest = &rest[i + name.len()..];
    }
    out.push_str(rest);
    out
}

/// The lowered directory of the request at `path`: beside it, named after the file without
/// `.ringi` (`expense.lowered`, `expense.ja.lowered`).
pub fn dir_of(path: &str) -> PathBuf {
    let p = Path::new(path);
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let stem = name.strip_suffix(".ringi").unwrap_or(&name);
    p.with_file_name(format!("{stem}.lowered"))
}

/// A rule a `use rule` reads, with what rulec knows of it.
pub struct UsedRule {
    /// The alias as written.
    pub alias: String,
    /// The path a lowered file in the lowered directory writes for it.
    pub path: String,
    pub facts: std::rc::Rc<RuleFacts>,
    /// The pin of its meaning (`meaning:…`), which the route's `apply` of it holds (DESIGN 6.4).
    pub meaning: Option<String>,
}

/// A calendar a `use calendar` of the request or of the shared route it uses reads.
pub struct UsedCal {
    pub alias: String,
    /// The alias in ASCII, which the JSON takes.
    pub ascii: String,
    /// The word the lowered files of koyomi's of the calendar are named by, after the kind of file
    /// (`terms_<word>.cal`, `open_<word>.cal`, `opened_<word>.cal`, and the code generated from each,
    /// `<request>_opened_<word>`): its ASCII alias, else, where that names the same file of the
    /// generated code as another calendar's ([`calendar_words`]), the alias with a number after it.
    pub file: String,
    /// The path a lowered file writes for it.
    pub path: String,
    /// The days its data covers, when koyomi says.
    pub data: Option<(i64, i64)>,
    /// The UTC offset its times are read in, in minutes east of UTC, when it says one.
    pub offset: Option<i32>,
    /// The calendar's file, as the program reaches it.
    pub target: Option<std::path::PathBuf>,
}

/// What lowering reads besides the request: the rules it uses, its calendars, and the days its dates
/// are counted over ([`crate::usable`]).
pub struct Ctx<'c, 'a> {
    pub s: &'c Scope<'a>,
    pub rules: Vec<UsedRule>,
    pub calendars: Vec<UsedCal>,
    /// The days a stage is taken to start on, by the alias of the calendar its deadlines count in:
    /// the days the request can be used on, within the calendar's data. A calendar not in it is
    /// counted over a whole turn of the Gregorian calendar from 2000-01-01, within its data.
    pub windows: Vec<(String, (i64, i64))>,
    /// The days the date a computation of the form counts business days from is taken over, when
    /// the calendar's data ends before the field's range does: the field's range up to the last day
    /// it can be counted from.
    pub form_window: Option<(i64, i64)>,
    /// The language the lowered files are asked of first: the run's ([`crate::check::Run::set_lang`]),
    /// in which the run asks of them again, so that a file asked once is checked once.
    pub lang: ritsu_base::text::Lang,
}

impl<'c, 'a> Ctx<'c, 'a> {
    /// The request's ASCII name, the first word of every lowered file's name (`expense_route`).
    pub fn name(&self) -> String {
        self.s.file.name.ascii().to_string()
    }

    pub fn rule(&self, alias: &str) -> Option<&UsedRule> {
        self.rules.iter().find(|r| r.alias == alias)
    }

    /// The calendar a count of business days is counted in: the one named, else the first.
    pub fn calendar(&self, named: Option<&str>) -> Option<&UsedCal> {
        match named {
            Some(n) => self.calendars.iter().find(|c| c.alias == n),
            None => self.calendars.first(),
        }
    }

    /// The file's name in the words of a description: `expense.ringi`.
    pub fn shown(&self) -> String {
        Path::new(self.s.path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
    }
}

/// A path written from the lowered directory: up one to the request's directory, then the path the
/// request writes (a path into a module, `@…`, as it is), its `..` folded.
pub fn from_lowered(parts: &[&str]) -> String {
    if let Some(m) = parts.iter().find(|p| p.starts_with('@')) {
        return m.to_string();
    }
    let mut out: Vec<String> = vec!["..".into()];
    for p in parts {
        for c in p.split('/') {
            match c {
                "" | "." => {}
                ".." if out.last().is_some_and(|l| l != "..") => {
                    out.pop();
                }
                c => out.push(c.to_string()),
            }
        }
    }
    out.join("/")
}

/// What lowering needs from the ports for the request in `s`: the facts of each rule it uses, the
/// path of each calendar. Err with the alias of a rule rulec cannot answer for (which is said as
/// E202 where the `use` line is).
pub fn context<'c, 'a>(s: &'c Scope<'a>, suite: &crate::suite::Suite) -> Ctx<'c, 'a> {
    let mut rules = Vec::new();
    let mut calendars = Vec::new();
    for (u, g) in s.file.uses.iter().zip(&s.got) {
        let Some(alias) = &u.alias else { continue };
        if let Got::Rule(target, facts) = g {
            let meaning = suite.rules.as_ref().and_then(|r| r.meaning(target));
            rules.push(UsedRule { alias: alias.text.clone(), path: from_lowered(&[&u.path]), facts: facts.clone(), meaning });
            continue;
        }
        let Got::Other(target) = g else { continue };
        match u.kind {
            UseKind::Calendar => {
                let facts = suite.dates.as_ref().and_then(|d| d.calendar(target).ok());
                let (data, offset) = (facts.as_ref().map(|c| c.data), facts.and_then(|c| c.offset));
                calendars.push(UsedCal { alias: alias.text.clone(), ascii: alias.ascii().to_string(), file: String::new(), path: from_lowered(&[&u.path]), data, offset, target: Some(target.clone()) });
            }
            _ => {}
        }
    }
    // the calendars of the shared route the request uses, written from the request's directory
    if let Some(crate::ast::Route::Shared { alias, .. }) = &s.file.route
        && let Some(u) = s.file.uses.iter().find(|u| u.kind == UseKind::Route && u.alias.as_ref().is_some_and(|a| a.text == alias.text))
        && let Some(shared) = s.ringi(&alias.text, UseKind::Route)
    {
        let dir = Path::new(&u.path).parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
        let shared_path = crate::names::beside(s.path, &u.path).ok();
        for cu in shared.uses.iter().filter(|cu| cu.kind == UseKind::Calendar) {
            let Some(a) = &cu.alias else { continue };
            if calendars.iter().any(|c: &UsedCal| c.alias == a.text) {
                continue;
            }
            let target = shared_path.as_ref().and_then(|sp| crate::names::beside(&sp.to_string_lossy(), &cu.path).ok());
            let facts = match (&suite.dates, &target) {
                (Some(d), Some(t)) => d.calendar(t).ok(),
                _ => None,
            };
            let (data, offset) = (facts.as_ref().map(|c| c.data), facts.and_then(|c| c.offset));
            calendars.push(UsedCal { alias: a.text.clone(), ascii: a.ascii().to_string(), file: String::new(), path: from_lowered(&[&dir, &cu.path]), data, offset, target });
        }
    }
    let words = calendar_words(&calendars.iter().map(|c| c.ascii.clone()).collect::<Vec<_>>());
    for (c, w) in calendars.iter_mut().zip(words) {
        c.file = w;
    }
    Ctx { s, rules, calendars, windows: Vec::new(), form_window: None, lang: ritsu_base::text::Lang::En }
}

/// The word each calendar's lowered files of koyomi's are named by ([`UsedCal::file`]), the calendars
/// in the order read: its ASCII alias, written as a module's name takes it, where that is free, else
/// with `_2`, `_3`, … after it. An alias that starts with `_` has `n` before it (`_office` is
/// `n_office`), and two `_` or more in a row are one: the file's name joins the word with `_`
/// (`<request>_opened_<word>`), and rustc warns of a module's name with two `_` in a row (koyomi's
/// E016 when the code is generated). The code generated from a file of koyomi's is named by the
/// file's name in small letters (the modules of TypeScript, Python and Rust), and Go's package by
/// the same without `_`, so two calendars whose aliases are one name there (`a_b` and `aB`; `ab` and
/// `aB`, which are one file on a file system that sets case aside) would write one file of the
/// generated code. A word is free when none of the names its files may take (`terms_<word>`,
/// `open_<word>`, `opened_<word>`) is, in Go's spelling, one of the files of the request's other
/// calendars or one of the files with no calendar's word (`form`, `terms`, `open`): `ed_x` and `x`
/// are free of each other only so, as `open_ed_x` and `opened_x` are one package in Go. A request
/// whose calendars are free of each other, and none of whose aliases starts with `_` or has two `_`
/// in a row, keeps the names it had.
pub fn calendar_words(asciis: &[String]) -> Vec<String> {
    let go = |n: &str| n.chars().filter(|c| *c != '_').flat_map(char::to_lowercase).collect::<String>();
    let files = |w: &str| ["terms", "open", "opened"].map(|k| go(&format!("{k}_{w}")));
    let mut taken: std::collections::BTreeSet<String> = ["form", "terms", "open"].map(String::from).into_iter().collect();
    let mut out = Vec::new();
    for a in asciis {
        let mut base = if a.starts_with('_') { format!("n{a}") } else { a.clone() };
        while base.contains("__") {
            base = base.replace("__", "_");
        }
        let mut w = base.clone();
        let mut i = 2;
        let sep = if base.ends_with('_') { "" } else { "_" };
        while files(&w).iter().any(|f| taken.contains(f)) {
            w = format!("{base}{sep}{i}");
            i += 1;
        }
        taken.extend(files(&w));
        out.push(w);
    }
    out
}

/// The request lowered: every file of it, in the order written (`choices.rule`, `form.rule`,
/// `form.cal`, the named tables, `route.rule`, `terms.cal`), the copy of `form.rule` the code ringi
/// generates reads (`values.rule`), then the questions. Err with each part
/// of the route's conditions its table cannot hold (E309): a request whose route is not lowered is
/// not lowered at all.
pub fn lower(cx: &Ctx) -> Result<Lowered, Vec<route::Unread>> {
    // the conditions of the form and of the claims, then the route's
    let mut unread = route::other_conditions(cx);
    let route = match route::route(cx) {
        Ok(r) => r,
        Err(u) => {
            unread.extend(u);
            None
        }
    };
    if !unread.is_empty() {
        unread.sort_by_key(|u| (u.span.line, u.span.col));
        return Err(unread);
    }
    let applied: &[String] = route.as_ref().map(|(_, n)| n.applied.as_slice()).unwrap_or(&[]);
    let mut files = Vec::new();
    if let Some(f) = form::choices(cx, applied) {
        files.push(f);
    }
    files.extend(form::form(cx));
    for t in &cx.s.file.tables {
        files.push(route::named_table(cx, t));
    }
    let mut names = None;
    if let Some((f, n)) = route {
        files.push(f);
        names = Some(n);
    }
    files.extend(terms::terms(cx));
    if let Some(f) = form::values(cx) {
        files.push(f);
    }
    files.extend(form::hidden(cx));
    // the enums and the values of the choices a rule writes under other names, which what rulec
    // says of any of the rules is said back in
    let chosen = choices_renamed(cx);
    for f in files.iter_mut().filter(|f| f.tool == LoweredTool::Rulec && f.part != Part::Choices) {
        for r in &chosen {
            if !f.renamed.iter().any(|(g, _)| *g == r.0) {
                f.renamed.push(r.clone());
            }
        }
    }
    Ok(Lowered { dir: dir_of(cx.s.path), files, route: names, laid: Laid::default() })
}

/// A field's choice enum's name in the lowered files, when it has one: a `choice` and a `choice
/// from` a rule's enum are enums of `choices.rule`, named after the field ([`choice_enums`]).
pub fn enum_of(cx: &Ctx, f: &Field) -> Option<String> {
    if !enumerated(cx, f) {
        return None;
    }
    Some(choice_enums(cx).into_iter().find(|(n, _)| *n == f.name.text).map(|(_, e)| e).unwrap_or_else(|| f.name.ascii().to_string()))
}

/// Whether a field is an enum of `choices.rule`: a `choice`, or a `choice from` a rule's enum.
fn enumerated(cx: &Ctx, f: &Field) -> bool {
    match &f.ty {
        TypeExpr::Choice => true,
        TypeExpr::ChoiceFrom(p) => p.len() == 2 && cx.rule(&p[0].text).is_some(),
        _ => false,
    }
}

/// The enum of `choices.rule` of each field of the form that has one, by the field's name as
/// written: its ASCII name, unless rulec reserves the word or reads it as a type (`date`,
/// `string`), the code rulec generates makes a type of that name (`option`, `output`: the enum's type
/// is `Option`, `Output`), or the enum of a rule's output the route reads, or another field's enum,
/// has it or has it in Pascal case, when `_value` follows it.
pub fn choice_enums(cx: &Ctx) -> Vec<(String, String)> {
    let mut names = words::Names::enums().avoiding(&words::declared(cx.s));
    for (.., n) in form::output_enums(cx) {
        names.keep(&n);
    }
    cx.s.file.form.iter().filter(|f| enumerated(cx, f)).map(|f| (f.name.text.clone(), names.take(f.name.ascii()))).collect()
}

/// Each name the request's enums of `choices.rule` and the values of its choices are written under
/// in place of their ASCII names, with those names.
pub fn choices_renamed(cx: &Ctx) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = choice_enums(cx).into_iter().filter_map(|(n, e)| cx.s.field(&n).filter(|f| f.name.ascii() != e).map(|f| (e, f.name.ascii().to_string()))).collect();
    out.extend(words::values_renamed(cx.s));
    out
}

/// What a line is, in both languages, for the map of lines.
pub fn what(ja: impl Into<String>, en: impl Into<String>) -> Text {
    Text { ja: ja.into(), en: en.into() }
}

/// Whether a field a person fills in may have no value where the lowered rules read it (DESIGN
/// 4.9): one written `optional`, shown or asked for on a condition, or the approvers' (empty until
/// an approver fills it in). A field the form asks for on every request has a value wherever the
/// form is worked out: without one, the request is refused first. A computed field may have none
/// through what it reads ([`empty_reads`]).
pub fn may_be_empty(f: &Field) -> bool {
    f.computed.is_none() && (f.optional.is_some() || f.shown_when.is_some() || f.required_when.is_some() || f.approvers_only.is_some())
}

/// The fields that may be empty ([`may_be_empty`]) a computed field reads, itself or through the
/// computations it reads, each once in the form's order: the computation has no value where one of
/// them has none (DESIGN 4.9). A total reads rows, and counts no row whose cell it reads is empty:
/// it always has a value. Empty for a field that is not computed.
pub fn empty_reads<'s>(s: &'s Scope, f: &Field) -> Vec<&'s Field> {
    fn walk<'s>(s: &'s Scope, e: &Expr, depth: usize, out: &mut Vec<&'s Field>) {
        let mut all = Vec::new();
        e.walk(&mut all);
        for x in all {
            let ExprKind::Path(p) = &x.kind else { continue };
            if p.len() != 1 {
                continue;
            }
            let Some(g) = s.field(&p[0].text) else { continue };
            match &g.computed {
                Some(c) if depth < 32 => walk(s, c, depth + 1, out),
                Some(_) => {}
                None if may_be_empty(g) && !out.iter().any(|y| y.name.text == g.name.text) => out.push(g),
                None => {}
            }
        }
    }
    let mut out = Vec::new();
    if let Some(c) = &f.computed {
        walk(s, c, 0, &mut out);
    }
    let order = |g: &&Field| s.fields().iter().position(|y| y.name.text == g.name.text).unwrap_or(usize::MAX);
    out.sort_by_key(order);
    out
}

/// The cells of a row that may be empty the computation of a row `c` reads, itself or through the
/// row's other computations (DESIGN 4.9): a row counts for it where they are all filled in.
pub fn empty_reads_of_row<'f>(rows: &'f Field, c: &'f Field) -> Vec<&'f Field> {
    form::row_empty_reads(rows, c)
}

/// Whether a computed field may have no value: it reads a field that may be empty (DESIGN 4.9).
pub fn may_be_none(s: &Scope, f: &Field) -> bool {
    !empty_reads(s, f).is_empty()
}

/// What the lowered files take in place of the value of `f`, a field that may be empty a
/// computation reads, where it is empty (DESIGN 4.9): a value of its range, which they take with
/// whether the field is filled in and no answer of them reads. Its least, else its most, else
/// zero; for a number with no unit, a value other than zero where its range has one, since the
/// glue's Rust holds such a number as a whole number, whose zero is the default of what it
/// unwraps, and clippy asks for that to be written otherwise (`manual_unwrap_or_default`). The
/// glue and the reference both take this one.
pub fn stand_in(f: &Field) -> words::Val {
    let (lo, hi) = words::ends(f);
    let at = match &f.ty {
        TypeExpr::Num(u) if u.dim == ritsu_units::Dim::Number => {
            let st = words::step(u);
            let inside = |r: &ritsu_units::Rat| lo.is_none_or(|l| r.cmp_to(l) != std::cmp::Ordering::Less) && hi.is_none_or(|h| r.cmp_to(h) != std::cmp::Ordering::Greater);
            [lo, hi, Some(st), Some(ritsu_units::Rat::int(0).sub(st))].into_iter().flatten().find(|r| r.num != 0 && inside(r)).unwrap_or(ritsu_units::Rat::int(0))
        }
        _ => lo.or(hi).unwrap_or(ritsu_units::Rat::int(0)),
    };
    match &f.ty {
        TypeExpr::Date => words::Val::Day(at.num as i64),
        _ => words::Val::Num(at),
    }
}

/// Whether a field that may be empty is read by a computation of the form that is not a total (a
/// computation of a row reads its row's fields): the rules that compute it take its value and
/// whether it is filled in as two inputs (DESIGN 4.9), since rulec computes nothing of a value that
/// may not be there (rulec's E103).
pub fn computed_from(s: &Scope, f: &Field) -> bool {
    may_be_empty(f)
        && s.file.form.iter().filter(|g| g.computed.is_some() && matches!(g.ty, TypeExpr::Num(_))).any(|g| {
            let mut all = Vec::new();
            g.computed.as_ref().unwrap().walk(&mut all);
            all.iter().any(|x| matches!(&x.kind, ExprKind::Path(p) if p.len() == 1 && p[0].text == f.name.text))
        })
}

/// What the output `output` (by the request's name of it: `trip_to_shown`, `amount`) of a lowered
/// rule of the form (`values.rule`) reads ([`LFile::reads`]), by the request's names of the fields: a
/// field by its ASCII name (`category`), a field of a row by the rows' and its (`items.price`), each
/// once in the form's order; whether a field is filled in is read of the field. What its answer
/// changes with, and nothing else (DESIGN 4.9, 12.2): a condition the rule reads as the table of
/// fewer fields than it names (`over is none`, where `over` is worked out from a field that may be
/// empty and one that is never empty, reads the one that may be) reads those fields. Empty for an
/// output the file does not have.
pub fn reads_of(s: &Scope, lf: &LFile, output: &str) -> Vec<String> {
    let in_file = lf.renamed.iter().find(|(_, w)| w == output).map(|(g, _)| g.as_str()).unwrap_or(output);
    let Some((_, reads)) = lf.reads.iter().find(|(n, _)| n == in_file) else { return Vec::new() };
    // the request's name of a name the file gives
    let back = |given: &str| lf.renamed.iter().find(|(g, _)| g == given).map(|(_, w)| w.clone()).unwrap_or_else(|| given.to_string());
    let mut at: Vec<(usize, usize)> = Vec::new();
    for r in reads {
        let (field, cell) = match r.split_once('.') {
            Some((el, col)) => {
                let cell = match lf.given.iter().find(|(g, _)| g == col) {
                    Some((_, w)) => w.rsplit('.').next().unwrap_or(w).to_string(),
                    None => back(col),
                };
                (back(el), Some(cell))
            }
            None => match lf.given.iter().find(|(g, _)| g == r) {
                Some((_, w)) => (w.clone(), None),
                None => (back(r), None),
            },
        };
        let Some(i) = s.file.form.iter().position(|f| f.name.ascii() == field) else { continue };
        let c = match &cell {
            Some(c) => match s.file.form[i].children.iter().position(|x| x.name.ascii() == c) {
                Some(j) => j + 1,
                None => continue,
            },
            None => 0,
        };
        if !at.contains(&(i, c)) {
            at.push((i, c));
        }
    }
    at.sort();
    at.into_iter()
        .map(|(i, c)| {
            let f = &s.file.form[i];
            match c {
                0 => f.name.ascii().to_string(),
                c => format!("{}.{}", f.name.ascii(), f.children[c - 1].name.ascii()),
            }
        })
        .collect()
}

/// What the computed field `f` (of the rows `rows`, for a computation of a row) is worked out from,
/// as the kind's description says it (`reads`) and the refusal of a value of it outside its range
/// names it: as the request's lowered `values.rule` (`values`) reads a computation of the form,
/// else as the computation is written, through the computations it reads ([`reads_of`],
/// [`form_reads`]). Empty for a field that is not computed.
pub fn computed_reads(s: &Scope, values: Option<&LFile>, f: &Field, rows: Option<&Field>) -> Vec<String> {
    let Some(e) = &f.computed else { return Vec::new() };
    let lowered = values.filter(|_| rows.is_none()).map(|lf| reads_of(s, lf, f.name.ascii())).filter(|r| !r.is_empty());
    lowered.unwrap_or_else(|| form_reads(s, e, rows))
}

/// The fields of the form an expression of it reads (a condition, a computation; of the row `rows`
/// for a computation of a row), through the computations it reads, each once in the form's order:
/// a field by its ASCII name (`category`), a field of a row by the rows' and its (`items.price`),
/// which a total reads in each row. What its answer changes with, and nothing else (DESIGN 4.9,
/// 12.2).
pub fn form_reads(s: &Scope, e: &Expr, rows: Option<&Field>) -> Vec<String> {
    fn in_row(rows: &Field, c: &Field, depth: usize, out: &mut Vec<(usize, usize)>, at: usize) {
        let Some(i) = rows.children.iter().position(|x| x.name.text == c.name.text) else { return };
        match &c.computed {
            None if !out.contains(&(at, i + 1)) => out.push((at, i + 1)),
            Some(e) if depth < 32 => {
                let mut all = Vec::new();
                e.walk(&mut all);
                for x in all {
                    if let ExprKind::Path(p) = &x.kind
                        && p.len() == 1
                        && let Some(g) = rows.children.iter().find(|g| g.name.text == p[0].text)
                    {
                        in_row(rows, g, depth + 1, out, at);
                    }
                }
            }
            _ => {}
        }
    }
    fn walk(s: &Scope, e: &Expr, depth: usize, out: &mut Vec<(usize, usize)>) {
        let at = |name: &str| s.file.form.iter().position(|f| f.name.text == name);
        let mut all = Vec::new();
        e.walk(&mut all);
        for x in all {
            match &x.kind {
                ExprKind::Path(p) if p.len() == 1 => {
                    let (Some(g), Some(i)) = (s.field(&p[0].text), at(&p[0].text)) else { continue };
                    match &g.computed {
                        Some(c) if depth < 32 => walk(s, c, depth + 1, out),
                        Some(_) => {}
                        None if !out.contains(&(i, 0)) => out.push((i, 0)),
                        None => {}
                    }
                }
                ExprKind::Lookup(_, by) => {
                    if let Some(i) = at(&by.text).filter(|i| !out.contains(&(*i, 0))) {
                        out.push((i, 0));
                    }
                }
                ExprKind::Sum(p) if p.len() == 2 => {
                    let (Some(rows), Some(i)) = (s.field(&p[0].text), at(&p[0].text)) else { continue };
                    if let Some(c) = rows.children.iter().find(|c| c.name.text == p[1].text) {
                        in_row(rows, c, 0, out, i);
                    }
                }
                _ => {}
            }
        }
    }
    let mut out: Vec<(usize, usize)> = Vec::new();
    match rows {
        Some(r) => {
            let at = s.file.form.iter().position(|f| f.name.text == r.name.text).unwrap_or(usize::MAX);
            let mut all = Vec::new();
            e.walk(&mut all);
            for x in all {
                if let ExprKind::Path(p) = &x.kind
                    && p.len() == 1
                    && let Some(g) = r.children.iter().find(|g| g.name.text == p[0].text)
                {
                    in_row(r, g, 0, &mut out, at);
                }
            }
        }
        None => walk(s, e, 0, &mut out),
    }
    out.sort();
    out.into_iter()
        .filter_map(|(i, c)| {
            let f = s.file.form.get(i)?;
            Some(match c {
                0 => f.name.ascii().to_string(),
                c => format!("{}.{}", f.name.ascii(), f.children.get(c - 1)?.name.ascii()),
            })
        })
        .collect()
}
