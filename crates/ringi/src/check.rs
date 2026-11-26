//! A `.ringi` from its text to its diagnostics (DESIGN 16.1, `check`): the syntax (`parse.rs`), the
//! files its `use` lines read and its names (`names.rs`), every expression's type (`types.rs`), the
//! form (`form.rs`) and the route (`route.rs`). What `ringi check` prints for a file, as text and as
//! JSON, and what `ritsu check` prints of it, is here too.
//!
//! A file of ringi's another one reads (an organization, masters, a shared route) is checked as a
//! file of its own, once in a run: [`Run`] keeps what each came to, with the data an organization
//! or masters read ([`crate::org`]), which the files that read them read from there. A request is
//! checked against that data too: the days it can be used on ([`crate::usable`]), and who answers
//! each stage in each version of the organization ([`crate::org::listing`]).

use crate::ast::{File, FileKind, Route, UseKind};
use crate::diag::{self, Diag};
use crate::names::{self, Scope};
use crate::suite::Suite;
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// What the check of one file comes to.
pub struct Outcome {
    /// The file, as it was given.
    pub path: String,
    pub diags: Vec<Diag>,
    /// The file, when nothing is wrong with it.
    pub file: Option<Rc<File>>,
    /// What the line that says the file passes says after the file.
    pub summary: Option<Text>,
    /// The files the request (or the shared route) is lowered to, when ringi's own checks pass and
    /// they could be laid (DESIGN 6.4).
    pub lowered: Option<crate::lower::Lowered>,
    /// The data the file read: an organization's versions (or why there are none), the rows of
    /// masters; for a shared route, the data of the organization it reads.
    pub data: Option<Rc<crate::org::Data>>,
    /// What the lines under the `ok` line say: the data an organization or masters read, the days a
    /// request can be used on, who may submit it, what the shape of the organization does to its
    /// stages.
    pub facts: Vec<Fact>,
    /// The days a request (or a shared route) can be used on, when its checks pass.
    pub usable: Option<crate::usable::Usable>,
    /// What the checks across the borders and `api` read of a request's ticket, when its checks
    /// pass ([`crate::ticket`]).
    pub ticket: Option<ritsu_ports::RequestTicket>,
    /// The UTC offset a request's days are read in, in minutes east of UTC (DESIGN 7.5), when its
    /// checks pass: the ticket machine's ([`crate::engine::offset`]), which `api` says.
    pub offset: Option<i32>,
    /// The ASCII names the JSON takes for what the file names (DESIGN 3.4, 16.2).
    pub names: crate::engine::Names,
    /// What the approvers and the requesters of each version of the organization come to (E502–E504,
    /// E508, E509 and the warnings of the same lists), when the run keeps them apart from what the
    /// file is checked to ([`Run::keep_listing_apart`]): `ritsu gen`'s, whose code takes the
    /// organization at each event (DESIGN 12.3).
    pub listing: Vec<Diag>,
}

/// A line under the `ok` line, and what the JSON's `facts` says of it (DESIGN 16.2): its text, and,
/// for what the shape of the organization does to the stages of a request, each case a page lists.
#[derive(Clone, Debug)]
pub struct Fact {
    pub text: Text,
    pub cases: Vec<crate::org::listing::Shaped>,
}

impl Fact {
    /// A line with no cases.
    pub fn line(text: Text) -> Fact {
        Fact { text, cases: Vec::new() }
    }

    pub fn to_json(&self, names: &crate::engine::Names, lang: Lang) -> Json {
        Json::obj([("text", Json::str(self.text.get(lang))), ("cases", Json::arr(self.cases.iter().map(|c| c.to_json(names, lang))))])
    }
}

impl Outcome {
    /// A file that did not come as far as its names: what is wrong with it, and nothing else. Also
    /// what `ritsu gen` says of a request whose laid code kaname says something of
    /// ([`crate::ports::Engine::glue_refused`]).
    pub(crate) fn failed(path: &str, diags: Vec<Diag>) -> Outcome {
        Outcome { path: path.to_string(), diags, file: None, summary: None, lowered: None, data: None, facts: Vec::new(), usable: None, ticket: None, offset: None, names: Default::default(), listing: Vec::new() }
    }

    pub fn has_errors(&self) -> bool {
        diag::has_errors(&self.diags)
    }

    /// Whether the file needs a language this run does not join (E008): the command is run where a
    /// language is not joined, which is not the file's fault, and exits 2.
    pub fn unjoined(&self) -> bool {
        self.diags.iter().any(|d| d.code == "E008")
    }

    /// The exit code of `ringi check` for this file alone: 0, 1, or 2 for E008.
    pub fn exit(&self) -> u8 {
        if self.unjoined() {
            2
        } else if self.has_errors() {
            1
        } else {
            0
        }
    }
}

/// What a run reads besides the files it is given (DESIGN 16.1): `--org`, `--at`, `--through`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    /// Where the data of every organization of the run is read from, instead of its `data` line.
    pub org: Option<String>,
    /// The day whose version of the organization is checked, instead of every version.
    pub at: Option<i64>,
    /// The day every request of the run is to be usable through (E107, E404).
    pub through: Option<i64>,
    /// Whether the approvers of each version are left unlisted (E502–E504, W502, W506, W507, W510,
    /// and the lines under `ok` of what the shape of the organization does): `simulate` solves them
    /// for the requester it is asked of, and shows a stage with nobody as it comes.
    pub unlisted: bool,
    /// Whether the scenarios of a request are left unrun (E602–E604): `run` runs them itself, to
    /// print each row as far as it goes, and `simulate` and `extract` need none of them.
    pub unrun: bool,
}

/// A run of the check: the languages it joins, what it reads besides the files, and what each file
/// came to.
pub struct Run {
    pub suite: Suite,
    /// What the run reads besides the files ([`Run::set_options`]).
    opts: RefCell<Options>,
    kept: RefCell<BTreeMap<(PathBuf, String), Rc<Outcome>>>,
    /// Whether the lists of each version of the organization are kept apart from what a file is
    /// checked to ([`Outcome::listing`]).
    apart: std::cell::Cell<bool>,
    /// Whether flows and gates are taken to pass, unread ([`Run::set_unread`]), and the files
    /// checked while they were, by their keys in `kept`.
    unread: std::cell::Cell<bool>,
    kept_unread: RefCell<std::collections::BTreeSet<(PathBuf, String)>>,
    /// The root of the project the run is of, when whoever runs it says it ([`Run::set_root`]):
    /// what a gate a `use gate` line reads is named from, as the project names it.
    root: RefCell<Option<PathBuf>>,
    /// The language the run says what it finds in ([`Run::set_lang`]): the one the files the
    /// requests are lowered to are asked of first.
    lang: std::cell::Cell<Lang>,
    /// The command the run is reached through ([`Run::set_through`]), which the commands its facts
    /// name are named with.
    through: std::cell::Cell<Option<&'static str>>,
}

impl Run {
    pub fn new(suite: Suite) -> Run {
        Run::with(suite, Options::default())
    }

    pub fn with(suite: Suite, opts: Options) -> Run {
        Run {
            suite,
            opts: RefCell::new(opts),
            kept: RefCell::default(),
            apart: std::cell::Cell::new(false),
            unread: std::cell::Cell::new(false),
            kept_unread: RefCell::default(),
            root: RefCell::default(),
            lang: std::cell::Cell::new(Lang::En),
            through: std::cell::Cell::new(None),
        }
    }

    /// From now on, the run is reached through the command `outer` (`ritsu`, for `ritsu check` and
    /// `ritsu gen`), whatever runs it: a command a fact names (`ritsu ringi simulate …`) is named
    /// the way the person ran the one that checks (ritsu's DESIGN 6.2, item 10). Said before any
    /// file is checked.
    pub fn set_through(&self, outer: &'static str) {
        self.through.set(Some(outer));
    }

    /// The command the run is reached through ([`Run::set_through`]).
    pub fn reached_through(&self) -> Option<&'static str> {
        self.through.get()
    }

    /// From now on, the files a request is lowered to are asked of the languages that check them in
    /// `lang` first (`--lang`): the language the run says what it finds in, and asks of those files
    /// again in (the page of a rule, its code), so that a file that is found to say nothing is
    /// checked once in the run (ritsu's DESIGN 4.15). Said before any file is checked; what a file
    /// is checked to is the same in either language.
    pub fn set_lang(&self, lang: Lang) {
        self.lang.set(lang);
    }

    /// What the run reads besides the files.
    pub fn opts(&self) -> Options {
        self.opts.borrow().clone()
    }

    /// From now on, the run reads `opts` besides the files (`ritsu check --through`): said before
    /// any file is checked, so that what each file is checked to with them is the one check of the
    /// file in the run, which every question of the run is answered from (ritsu's DESIGN 4.15). A
    /// file checked before with other options is checked again.
    pub fn set_options(&self, opts: &Options) {
        if *self.opts.borrow() != *opts {
            *self.opts.borrow_mut() = opts.clone();
            self.kept.borrow_mut().clear();
        }
    }

    /// The root of the project the run is of (`ritsu check`, `ritsu gen`): a gate a request reads
    /// is asked of sekisho from it, as the project's own check of the gate asks it, so that the
    /// gate is checked once in the run (ritsu's DESIGN 4.15). Said before any file is checked; a
    /// run that is not told finds the root above each file.
    pub fn set_root(&self, root: &Path) {
        self.root.borrow_mut().get_or_insert_with(|| ritsu_base::paths::opened(root));
    }

    /// The root a file of the run is read from: the project's, when the run was told it, else the
    /// one above the file ([`ritsu_base::paths::find_root`]).
    pub fn root_for(&self, path: &str) -> PathBuf {
        self.root.borrow().clone().unwrap_or_else(|| ritsu_base::paths::find_root(Path::new(path)))
    }

    /// From now on, what the approvers and the requesters of each version of the organization come
    /// to is kept apart from what a file is checked to (DESIGN 12.3): a request whose lists say
    /// something passes, its scenarios are run, and [`Outcome::listing`] holds what the lists say.
    /// `ritsu gen` reads a run so: the code it writes takes the organization at each event, and is
    /// the same whatever the versions hold; `ritsu check` says the lists.
    pub fn keep_listing_apart(&self) {
        self.apart.set(true);
        self.kept.borrow_mut().clear();
    }

    /// From now on until it is said again, every flow and every gate a file reads (`use flow`,
    /// `use gate`) is taken to pass, unread: `ritsu gen --target requests` makes the server's API
    /// from the requests so before the check, as a flow or a gate may read the API the run writes
    /// (ritsu's DESIGN 9.3, ringi's 12.2). What a file is checked to so is kept: asked of again once
    /// they are read, it is kept where the flows and the gates it reads pass, as a check that reads
    /// them comes to the same, and checked again where one does not (E202). Each file is checked
    /// once in such a run that passes (ritsu's DESIGN 4.15).
    pub fn set_unread(&self, on: bool) {
        self.unread.set(on);
    }

    /// Whether flows and gates are taken to pass, unread ([`Run::set_unread`]).
    pub fn unread(&self) -> bool {
        self.unread.get()
    }

    /// The check of the file at `path`, named the way the run writes its paths (ritsu's DESIGN
    /// 4.15), however `path` is written: `./expense.ringi`, `x/../expense.ringi` and
    /// `expense.ringi` are one file, checked once in the run and named alike. Kept by the file the
    /// system opens (`paths::opened`); Err, why, when it cannot be read.
    pub fn file(&self, path: &str) -> Result<Rc<Outcome>, std::io::Error> {
        let shown = ritsu_base::paths::show_given(path);
        let key = (ritsu_base::paths::opened(Path::new(&shown)), shown.clone());
        let kept = self.kept.borrow().get(&key).cloned();
        if let Some(o) = kept {
            if self.unread() || !self.kept_unread.borrow_mut().remove(&key) {
                return Ok(o);
            }
            // checked with the flows and the gates it reads taken to pass: the same check where they do
            if o.file.as_deref().is_none_or(|f| names::flows_and_gates_pass(f, &shown, self)) {
                return Ok(o);
            }
        }
        let src = ritsu_base::fs::read_to_string(&shown)?;
        let o = Rc::new(self.text(&shown, &src));
        if self.unread() {
            self.kept_unread.borrow_mut().insert(key.clone());
        }
        self.kept.borrow_mut().insert(key, o.clone());
        Ok(o)
    }

    /// The check of `src`, the text of the file `path`, its commands named through the command the
    /// run is reached through ([`Run::set_through`]).
    pub fn text(&self, path: &str, src: &str) -> Outcome {
        match self.through.get() {
            Some(outer) => ritsu_base::cli::through(outer, || self.checked_text(path, src)),
            None => self.checked_text(path, src),
        }
    }

    fn checked_text(&self, path: &str, src: &str) -> Outcome {
        ritsu_base::tally::count("ringi", Path::new(path));
        let opts = self.opts();
        // a key written in the file is in the repository whether the file reads or not (W901)
        let keys = crate::security::keys(path, src);
        let parsed = crate::parse::parse(path, src);
        let Some(file) = parsed.file else {
            let mut diags = parsed.diags;
            diags.extend(keys);
            diag::sort(&mut diags);
            return Outcome::failed(path, diags);
        };
        let unjoined = self.unjoined(&file, path, src);
        if !unjoined.is_empty() {
            let mut diags = unjoined;
            diags.extend(keys);
            diag::sort(&mut diags);
            return Outcome::failed(path, diags);
        }
        let mut diags = parsed.diags;
        let got = names::read_uses(&file, path, src, self, &mut diags);
        let scope = Scope { file: &file, path, src, got };
        names::declarations(&scope, &mut diags);
        names::references(&scope, &mut diags);
        crate::types::check(&scope, &mut diags);
        crate::form::check(&scope, &mut diags);
        crate::route::check(&scope, &self.suite, &mut diags);
        let mut data = None;
        let mut facts = Vec::new();
        let mut usable = None;
        let mut lowered = None;
        let mut listing = Vec::new();
        if !diag::has_errors(&diags) {
            match file.kind {
                FileKind::Organization => {
                    let read = crate::org::read(&file, path, &opts);
                    diags.extend(read.diags);
                    if let Some(o) = read.org {
                        facts.push(Fact::line(o.facts()));
                        data = Some(Rc::new(crate::org::Data::Org(o)));
                    } else if let Some(none) = read.none {
                        facts.push(Fact::line(tr!("データ無し: {}", "no data: {}", none.line.ja; none.line.en)));
                        data = Some(Rc::new(crate::org::Data::NoOrg(none)));
                    }
                }
                FileKind::Masters => {
                    let md = crate::org::masters::read(&file, path, src, scope.org_data(), &mut diags);
                    crate::org::listing::masters_people(&scope, &mut diags);
                    facts.push(Fact::line(md.facts()));
                    data = Some(Rc::new(crate::org::Data::Masters(md)));
                }
                FileKind::Request | FileKind::Route => {
                    data = scope.org_rc();
                    let (l, u) = lowered_and_usable(&scope, &self.suite, &opts, self.lang.get(), &mut diags);
                    lowered = l;
                    let listed = match (file.kind == FileKind::Request && !opts.unlisted, self.apart.get()) {
                        (true, false) => crate::org::listing::check(&scope, u.as_ref(), &mut diags),
                        (true, true) => crate::org::listing::check(&scope, u.as_ref(), &mut listing),
                        _ => Vec::new(),
                    };
                    // whether a ticket ends, and within how long (E401, W402); what its endings
                    // do with its holds (E405–E407), and what the holds are keyed by (E408)
                    let ends = crate::ends::check(&scope, &mut diags);
                    if file.kind == FileKind::Request {
                        crate::ends::holds(&scope, &mut diags);
                        // what the holds are keyed by (E408)
                        crate::ends::keyed(&scope, &crate::engine::books(&scope, &self.suite), &mut diags);
                    }
                    // the scenarios, each run on the ticket machine (E602–E604). They are written
                    // for the days of their rows: with `--at`, which reads one version of the
                    // organization, they are left to a run without it
                    if file.kind == FileKind::Request
                        && !opts.unrun
                        && opts.at.is_none()
                        && !file.scenarios.is_empty()
                        && !diag::has_errors(&diags)
                        && let (Some(l), Some(org)) = (lowered.as_ref(), scope.org_data())
                    {
                        let m = crate::engine::Machine::new(&scope, &self.suite, l, u.as_ref());
                        crate::scenario::check(&m, org, &mut diags);
                    }
                    if !diag::has_errors(&diags)
                        && let Some(u) = u
                    {
                        facts.extend(u.lines().into_iter().map(Fact::line));
                        facts.extend(ends.filter(|_| file.kind == FileKind::Request).map(Fact::line));
                        facts.extend(listed);
                        usable = Some(u);
                    }
                    // a label the file's screens show that a language it declares labels in lacks,
                    // or that has no title in the file's language (E703): the file says what its pages
                    // are in, and that is said where the file is checked, not first where its screens
                    // are made. A range a screen cannot hold (E704) is the screens' own limit, which
                    // the package of a language does not have, and is left to `--target requests`
                    if file.kind == FileKind::Request {
                        let said = crate::screens::labels_checked(&scope);
                        diags.extend(said.into_iter().map(|x| x.notes.into_iter().fold(Diag::error(x.code, path, x.line, 1, x.message).source(src), |d, n| d.note(n))));
                    }
                }
            }
        }
        let summary = (!diag::has_errors(&diags)).then(|| summary(&scope));
        let ticket = (file.kind == FileKind::Request && !diag::has_errors(&diags)).then(|| crate::ticket::facts(&scope, &self.suite));
        let offset = (file.kind == FileKind::Request && !diag::has_errors(&diags)).then(|| crate::engine::offset(&crate::engine::calendars(&scope, &self.suite)));
        let names = crate::engine::Names::of(&scope);
        drop(scope);
        diags.extend(keys);
        diag::sort(&mut diags);
        let ok = !diag::has_errors(&diags);
        if !ok {
            facts.clear();
            usable = None;
        }
        diag::sort(&mut listing);
        Outcome { path: path.to_string(), diags, file: ok.then(|| Rc::new(file)), summary, lowered, data, facts, usable, ticket: ticket.filter(|_| ok), offset: offset.filter(|_| ok), names, listing }
    }

    /// E008 for each language the file needs that this run does not join, at the first line that
    /// needs it.
    fn unjoined(&self, f: &File, path: &str, src: &str) -> Vec<Diag> {
        let cmd = crate::run::with_ritsu();
        needs(f)
            .into_iter()
            .filter(|(tool, ..)| !self.suite.reads(tool))
            .map(|(tool, line, why)| {
                Diag::error("E008", path, line, 1, tr!("このファイルには {tool} が要りますが、この ringi には入っていません（{}）", "The file needs {tool}, which this ringi does not hold ({})", why.ja; why.en))
                    .source(src)
                    .note(tr!(
                        "ringi 単独のバイナリには、ほかの言語が入っていません。同じコマンドを、すべての言語をつないだ ritsu で、`{cmd}` のように走らせてください。",
                        "The binary of ringi's own crate holds no other language; run the same command with every language joined, through ritsu: `{cmd}`."
                    ))
            })
            .collect()
    }
}

/// A request or a shared route that passes ringi's own checks, lowered, and its lowered files
/// checked by their languages (DESIGN 6.4, 6.5, 9.2): first a computed field's range that no request
/// can meet (E103), then the days it can be used on, which the lowered dates are counted over
/// ([`crate::usable`]), then what rulec and koyomi say of the lowered files, in ringi's words, and
/// the claims ringi asks rulec of them ([`crate::mapped`]), and the claim that a reminder comes
/// before its deadline where the two are counted in two calendars ([`crate::apart`]).
fn lowered_and_usable(s: &Scope, suite: &Suite, opts: &Options, lang: Lang, diags: &mut Vec<Diag>) -> (Option<crate::lower::Lowered>, Option<crate::usable::Usable>) {
    crate::claims::unsatisfiable(s, diags);
    if diag::has_errors(diags) {
        return (None, None);
    }
    let mut cx = crate::lower::context(s, suite);
    cx.lang = lang;
    let usable = crate::usable::find(&mut cx, suite, opts, diags);
    let lowered = crate::mapped::check(&cx, suite, diags);
    crate::apart::claim(&cx, suite, diags);
    (lowered, Some(usable))
}

/// The languages a file needs (DESIGN 16.3), each with the first line that needs it and why: the
/// ones its `use` lines read, and the ones its parts are lowered to (a request to a module of
/// kaname's, a form and a route to rules, deadlines and business days to koyomi's dates). An
/// organization and masters need none.
pub fn needs(f: &File) -> Vec<(&'static str, usize, Text)> {
    let mut v: Vec<(&'static str, usize, Text)> = Vec::new();
    let mut add = |tool: &'static str, line: usize, why: Text| match v.iter_mut().find(|x| x.0 == tool) {
        Some(x) if line < x.1 => *x = (tool, line, why),
        Some(_) => {}
        None => v.push((tool, line, why)),
    };
    if f.kind == FileKind::Request {
        add("kaname", f.name.span.line, tr!("申請の種類は kaname のモジュールに下ろします", "a request is lowered to a module of kaname's"));
    }
    for u in &f.uses {
        let Some(tool) = u.kind.tool() else { continue };
        let name = u.alias.as_ref().map(|a| a.text.clone()).unwrap_or_else(|| u.path.clone());
        let why = match u.kind {
            UseKind::Rule => tr!("規則 `{name}` を読みます", "it reads the rule `{name}`"),
            UseKind::Calendar => tr!("カレンダー `{name}` を読みます", "it reads the calendar `{name}`"),
            UseKind::Dates => tr!("日付のファイル `{name}` を読みます", "it reads the dates file `{name}`"),
            UseKind::Book => tr!("帳簿 `{name}` を読みます", "it reads the book `{name}`"),
            UseKind::Flow => tr!("flow `{name}` を読みます", "it reads the flow `{name}`"),
            _ => tr!("gate `{name}` を読みます", "it reads the gate `{name}`"),
        };
        add(tool, u.line, why);
    }
    if !matches!(f.kind, FileKind::Request | FileKind::Route) {
        return v;
    }
    let block = |w: &str| f.blocks.iter().find(|(b, _)| *b == w).map(|(_, l)| *l);
    if let Some(l) = block("form").filter(|_| !f.form.is_empty()) {
        add("rulec", l, tr!("フォームを rulec の規則に下ろします", "its form is lowered to a rule"));
    }
    if let (Some(l), Some(Route::Table(_) | Route::Stages(_))) = (block("route"), &f.route) {
        add("rulec", l, tr!("経路を rulec の規則に下ろします", "its route is lowered to a rule"));
    }
    let deadlines = tr!("期限を koyomi の日付のファイルに下ろします", "its deadlines are lowered to a dates file");
    if let Some(l) = block("every stage") {
        add("koyomi", l, deadlines.clone());
    }
    if let Some(Route::Stages(lines)) = &f.route {
        for s in lines.iter().filter(|s| s.deadlines.is_some()) {
            add("koyomi", s.line, deadlines.clone());
        }
    }
    let days = tr!("営業日を koyomi のカレンダーで数えます", "it counts business days in a calendar of koyomi's");
    for fd in f.form.iter().chain(&f.inputs) {
        let counts = fd.open_in.is_some() || fd.computed.as_ref().is_some_and(|e| {
            let mut all = Vec::new();
            e.walk(&mut all);
            all.iter().any(|x| matches!(x.kind, crate::ast::ExprKind::BusinessDays(..)))
        });
        if counts {
            add("koyomi", fd.line, days.clone());
        }
    }
    if let Some(t) = &f.ticket {
        for l in &t.lines {
            if let crate::ast::TicketLine::Resubmit { line, .. } | crate::ast::TicketLine::Pause { line, .. } = l {
                add("koyomi", *line, days.clone());
            }
        }
    }
    v.sort_by_key(|x| x.1);
    v
}

/// `1 field`, `2 fields`.
fn many(n: usize, one: &str, more: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {more}") }
}

/// What the line that says a file passes says after the file: its kind and what it holds.
fn summary(s: &Scope) -> Text {
    let f = s.file;
    let stages = s.stages().map(|v| v.len()).unwrap_or(0);
    let st = many(stages, "stage", "stages");
    match f.kind {
        FileKind::Request => {
            let n = f.form.len();
            let fields = many(n, "field", "fields");
            tr!("申請の種類、フィールド {n} 個、ステップ {stages} 個", "a request, {fields}, {st}")
        }
        FileKind::Route => {
            let n = f.inputs.len();
            let inputs = many(n, "input", "inputs");
            tr!("経路、入力 {n} 個、ステップ {stages} 個", "a route, {inputs}, {st}")
        }
        FileKind::Organization => {
            let (p, k) = (f.positions.len(), f.team_kinds.len());
            let (positions, kinds) = (many(p, "position", "positions"), many(k, "kind of team", "kinds of team"));
            tr!("組織、役職 {p} 個、チームの種類 {k} 個", "an organization, {positions}, {kinds}")
        }
        FileKind::Masters => {
            let n = f.masters.len();
            let masters = many(n, "master", "masters");
            tr!("マスタ {n} 個", "{masters}")
        }
    }
}

/// The root of a run (ritsu's DESIGN 6.2, item 3; 17.6): `--root`, else the nearest directory above
/// the first path given that holds `.git` or `ritsu.mod`, else the directory of that file.
pub fn root_of(root: Option<&Path>, first: &str) -> PathBuf {
    match root {
        Some(r) => ritsu_base::paths::opened(r),
        None => ritsu_base::paths::find_root(Path::new(first)),
    }
}

/// The line that says a file passes, `ok expense.ringi: a request, 8 fields, 4 stages`, and the
/// lines under it, each two spaces in: the data the file read, the days a request can be used on.
pub fn ok_line(o: &Outcome, lang: Lang) -> Option<String> {
    if o.has_errors() {
        return None;
    }
    o.summary.as_ref().map(|s| {
        let mut out = format!("ok {}: {}\n", o.path, s.get(lang));
        for f in &o.facts {
            out.push_str(&format!("  {}\n", f.text.get(lang)));
        }
        out
    })
}

/// What `ringi check` prints for one file: each diagnostic, then the line that says it passes.
pub fn render(o: &Outcome, lang: Lang) -> String {
    let mut s: String = o.diags.iter().map(|d| d.render(lang)).collect();
    if let Some(ok) = ok_line(o, lang) {
        s.push_str(&ok);
    }
    s
}

/// The diagnostics with their files written from the root, for the JSON.
fn rooted(o: &Outcome, root: &Path) -> Vec<Diag> {
    o.diags
        .iter()
        .map(|d| {
            let rel = ritsu_base::paths::from_root(root, Path::new(&d.file)).unwrap_or_else(|| d.file.clone());
            d.clone().rel(&rel)
        })
        .collect()
}

/// The `--format json` of one file: `ringi` (the version), `file`, `ok`, `summary`, `facts` (the
/// lines under the `ok` line, each with its text and its cases), `usable` (the days a request can be
/// used on), `diagnostics`, in that order.
pub fn to_json(o: &Outcome, root: &Path, lang: Lang) -> Json {
    let file = ritsu_base::paths::from_root(root, Path::new(&o.path)).unwrap_or_else(|| o.path.clone());
    let passes = !o.has_errors();
    Json::obj([
        ("ringi", Json::str(env!("CARGO_PKG_VERSION"))),
        ("file", Json::str(file)),
        ("ok", Json::from(passes)),
        ("summary", Json::opt_str(if passes { o.summary.as_ref().map(|t| t.get(lang).to_string()) } else { None })),
        ("facts", Json::arr(o.facts.iter().filter(|_| passes).map(|f| f.to_json(&o.names, lang)))),
        ("usable", match &o.usable {
            Some(u) if passes => u.to_json(&o.names, root, lang),
            _ => Json::Null,
        }),
        ("diagnostics", Json::arr(rooted(o, root).iter().map(|d| d.to_json(lang)))),
    ])
}

/// What `ritsu check` prints of one file (`f`, as the person gave it) that the check came to, or
/// could not read (ritsu's DESIGN 8.3): every finding, as `ringi check` prints it and as its
/// `--format json` prints it, then the line that says the file passes; a file that cannot be read,
/// as every command of ritsu says it (ritsu's DESIGN 6.2, item 10).
pub fn unit(root: &Path, f: &str, checked: Result<&Outcome, &std::io::Error>, lang: Lang) -> ritsu_ports::Checked {
    use ritsu_ports::{Checked as Unit, Finding, Part, Verdict};
    match checked {
        Ok(o) => {
            let mut parts: Vec<Part> = rooted(o, root).iter().map(|d| Part::Finding(Finding::of(d, Some(d.rel.clone()), lang))).collect();
            if let Some(ok) = ok_line(o, lang) {
                parts.push(Part::Text(ok));
            }
            let verdict = if o.unjoined() {
                Verdict::Unchecked
            } else if o.has_errors() {
                Verdict::Fails
            } else {
                Verdict::Passes
            };
            Unit { label: f.to_string(), parts, verdict, json: to_json(o, root, lang) }
        }
        Err(e) => Unit::unchecked(f, format!("{}\n", ritsu_base::cli::refusal(ritsu_base::cli::cannot_read(f, e).get(lang), lang))),
    }
}

/// Shorthand for a test or a tool: the file at `path` with the languages `suite` joins.
pub fn check(path: &str, suite: Suite) -> Result<Rc<Outcome>, std::io::Error> {
    Run::new(suite).file(path)
}
